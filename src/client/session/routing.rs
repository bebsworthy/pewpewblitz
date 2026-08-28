use super::{
    AppExit, Client, ClientJoinPhase, ClientJoinStatus, ClientMatchResultContext,
    ClientMatchResultState, ClientNetworkConfig, Commands, Disconnect, Disconnected, Entity,
    Fighter, MatchPhase, MatchRoot, MatchRouteGrant, MatchState, MessageReceiver, MessageWriter,
    NetworkTransport, PlayerId, Query, Real, Res, ResMut, Result, RoutedClientLifecycle,
    RoutedClientPhase, RoutedClientSession, RoutedClientSessionKind, RoutedTransitionDeadline,
    RoutedUdpIo, SelectedGameType, Time, ToOwned, Unlink, UnlinkReason, Unlinked, With, hud, info,
    spawn_client_entity, warn,
};

/// Finish the routed process smoke only after the match session has completed, its routes have
/// been revoked, and the client has authenticated one fresh lobby generation. Generation one is
/// the initial lobby and generation two is the match; a generation of three or greater therefore
/// cannot be satisfied by an initial lobby retry.
#[allow(clippy::needless_pass_by_value)]
pub(in crate::client) fn observe_fresh_lobby_return(
    config: Res<ClientNetworkConfig>,
    mut app_exit: MessageWriter<AppExit>,
    query: Query<(&RoutedClientSession, &ClientJoinStatus), With<Client>>,
) {
    if !config.exit_after_lobby_return {
        return;
    }
    if query.iter().any(|(session, status)| {
        session.kind == RoutedClientSessionKind::Lobby
            && session.generation >= 3
            && matches!(status.phase, ClientJoinPhase::LobbyActive { .. })
    }) {
        info!("brawler client authenticated a fresh lobby after match completion");
        app_exit.write(AppExit::Success);
    }
}

/// Accept one authenticated lobby grant and begin the explicit unlink boundary. A stale or
/// duplicate grant is ignored; it cannot replace the capability already selected for this
/// request.
pub(super) fn process_match_route_grant(
    mut lifecycle: ResMut<RoutedClientLifecycle>,
    mut receivers: Query<
        (
            &RoutedClientSession,
            Option<&mut MessageReceiver<MatchRouteGrant>>,
        ),
        With<Client>,
    >,
) {
    if lifecycle.phase != RoutedClientPhase::Lobby {
        return;
    }
    for (session, receiver) in &mut receivers {
        if session.kind != RoutedClientSessionKind::Lobby
            || session.generation != lifecycle.generation
        {
            continue;
        }
        let Some(mut receiver) = receiver else {
            continue;
        };
        for grant in receiver.receive() {
            if lifecycle.accept_grant(grant) {
                info!(
                    request_id = grant.request_id.get(),
                    allocation_id = grant.allocation_id.get(),
                    match_id = grant.match_id.get(),
                    "brawler client accepted authenticated match route grant"
                );
                // The capability is intentionally absent from this log record.
                return;
            }
        }
    }
}

/// A replicated authoritative completion is the only gameplay signal that starts the routed
/// match-to-lobby transition. The client intentionally does not wait for a worker Result or
/// terminal disconnect: the supervisor owns that control-plane fact while the client closes its
/// current session and then creates one fresh lobby session.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy resources are schedule-owned system parameters"
)]
pub(in crate::client) fn observe_completed_match(
    config: Res<ClientNetworkConfig>,
    mut lifecycle: ResMut<RoutedClientLifecycle>,
    roots: Query<
        (
            &MatchState,
            Option<&crate::matchplay::WipeoutState>,
            Option<&crate::matchplay::HotZoneState>,
            Option<&crate::matchplay::MatchClock>,
        ),
        With<MatchRoot>,
    >,
    statuses: Query<&ClientJoinStatus, With<Client>>,
    fighters: Query<(&PlayerId, &crate::combat::TeamId), With<Fighter>>,
    selection: Option<Res<SelectedGameType>>,
    result_state: Option<ResMut<ClientMatchResultState>>,
) {
    if config.transport != NetworkTransport::RoutedUdp
        || lifecycle.phase != RoutedClientPhase::Match
    {
        return;
    }
    let Some((result, final_score)) =
        roots
            .iter()
            .find_map(|(state, wipeout, hot_zone, clock)| match state.phase {
                MatchPhase::Completed { result, .. } => Some((
                    result,
                    hud::build_mode_score_view(Some((state, wipeout)), hot_zone, clock)
                        .filter(|score| !matches!(score, hud::ModeScoreView::Syncing)),
                )),
                _ => None,
            })
    else {
        return;
    };
    if let Some(mut result_state) = result_state
        && result_state.context.is_none()
    {
        let local_player = statuses.iter().find_map(|status| match status.phase {
            ClientJoinPhase::Active { player_id, .. } => Some(player_id),
            _ => None,
        });
        let local_team = local_player.and_then(|local_player| {
            fighters
                .iter()
                .find_map(|(player_id, team)| (*player_id == local_player).then_some(*team))
        });
        let game_type_id = selection
            .as_ref()
            .and_then(|value| value.game_type_id.clone())
            .or_else(|| result_state.last_accepted_game_type_id.clone());
        result_state.context = Some(ClientMatchResultContext {
            result,
            local_team,
            game_type_id,
            game_name: None,
            final_score,
        });
    }
    if lifecycle.request_return_to_lobby() {
        info!("brawler client observed authoritative match completion; returning to lobby");
    }
}

/// Issue the one deliberate disconnect required by a routed session transition. The new entity
/// is not spawned here: `advance_routed_transition` waits for the old entity to be despawned by
/// the deferred `Unlinked` observation.
#[allow(clippy::needless_pass_by_value)]
pub(in crate::client) fn drive_routed_transition(
    mut commands: Commands,
    config: Res<ClientNetworkConfig>,
    time: Res<Time<Real>>,
    mut query: Query<
        (Entity, &mut ClientJoinStatus, Option<&Disconnected>),
        With<RoutedClientSession>,
    >,
    lifecycle: Res<RoutedClientLifecycle>,
) {
    if config.transport != NetworkTransport::RoutedUdp
        || !matches!(
            lifecycle.phase,
            RoutedClientPhase::AwaitingLobbyUnlink
                | RoutedClientPhase::AwaitingLobbyRetryUnlink
                | RoutedClientPhase::AwaitingMatchUnlink
        )
    {
        return;
    }
    for (entity, mut status, disconnected) in &mut query {
        if disconnected.is_none() && !status.disconnect_requested {
            status.disconnect_requested = true;
            commands.entity(entity).insert(RoutedTransitionDeadline(
                time.elapsed().saturating_add(config.connect_timeout),
            ));
            commands.trigger(Disconnect { entity });
        } else if disconnected.is_some() && status.disconnect_requested {
            // Netcode's `Disconnect` deliberately leaves the underlying Link established. Wait
            // one frame so its disconnect datagrams can flush through PostUpdate, then close the
            // routed socket and expose `Unlinked` to the deferred teardown observer below.
            commands.trigger(Unlink {
                entity,
                reason: UnlinkReason::UserRequested(None),
            });
        }
    }
}

/// Observe the transport's deferred `Unlinked` marker before any replacement is requested. This
/// is the ownership boundary that guarantees a fixed local address never has two live sockets.
#[allow(clippy::needless_pass_by_value)]
pub(super) fn observe_routed_transition(
    mut commands: Commands,
    config: Res<ClientNetworkConfig>,
    lifecycle: Res<RoutedClientLifecycle>,
    query: Query<(Entity, Option<&Unlinked>), With<RoutedClientSession>>,
) {
    if config.transport != NetworkTransport::RoutedUdp {
        return;
    }
    for (entity, unlinked) in &query {
        let Some(_unlinked) = unlinked else {
            continue;
        };
        if !matches!(
            lifecycle.phase,
            RoutedClientPhase::AwaitingLobbyUnlink
                | RoutedClientPhase::AwaitingLobbyRetryUnlink
                | RoutedClientPhase::AwaitingMatchUnlink
        ) {
            // Unexpected routed disconnects stay visible to the normal lifecycle observer, which
            // classifies them as terminal failures. Only an explicitly requested transition may
            // consume Unlinked and replace the entity.
            continue;
        }
        commands.entity(entity).despawn();
    }
}

/// Spawn the next generation only after the previous routed entity's deferred despawn has been
/// applied. This keeps the Lightyear topology at exactly one `Client` entity.
#[allow(clippy::needless_pass_by_value)]
pub(super) fn advance_routed_transition(
    mut commands: Commands,
    config: Res<ClientNetworkConfig>,
    time: Res<Time<Real>>,
    mut lifecycle: ResMut<RoutedClientLifecycle>,
    clients: Query<(), With<Client>>,
) -> Result {
    if config.transport != NetworkTransport::RoutedUdp || clients.iter().next().is_some() {
        return Ok(());
    }
    match lifecycle.phase {
        RoutedClientPhase::AwaitingLobbyUnlink => {
            let Some(grant) = lifecycle.accepted_grant else {
                lifecycle.phase = RoutedClientPhase::AwaitingLobbyRetryUnlink;
                return Ok(());
            };
            let generation = lifecycle.generation.saturating_add(1).max(1);
            let io = RoutedUdpIo::with_match_capability(
                config.server_addr,
                grant.capability.to_routing_capability(),
            );
            spawn_client_entity(
                &mut commands,
                &config,
                time.elapsed(),
                Some((
                    io,
                    RoutedClientSession {
                        generation,
                        kind: RoutedClientSessionKind::Match,
                    },
                )),
            )?;
            lifecycle.generation = generation;
            // This transition is required in release builds too. Never hide the mutating call
            // inside `debug_assert_eq!`, whose expression is compiled out without debug checks.
            let accepted_grant = lifecycle.begin_match();
            debug_assert_eq!(accepted_grant, Some(grant));
        }
        RoutedClientPhase::AwaitingLobbyRetryUnlink | RoutedClientPhase::AwaitingMatchUnlink => {
            lifecycle.begin_lobby_after_match();
            let generation = lifecycle.generation;
            spawn_client_entity(
                &mut commands,
                &config,
                time.elapsed(),
                Some((
                    RoutedUdpIo::lobby(config.server_addr),
                    RoutedClientSession {
                        generation,
                        kind: RoutedClientSessionKind::Lobby,
                    },
                )),
            )?;
            info!(generation, "brawler client starting fresh lobby session");
        }
        RoutedClientPhase::Disabled | RoutedClientPhase::Lobby | RoutedClientPhase::Match => {}
    }
    Ok(())
}

/// Routed failures recover through a fresh lobby attempt. Direct UDP keeps its existing terminal
/// timeout behavior below.
#[allow(clippy::needless_pass_by_value)]
pub(in crate::client) fn enforce_routed_timeout(
    mut commands: Commands,
    config: Res<ClientNetworkConfig>,
    time: Res<Time<Real>>,
    mut lifecycle: ResMut<RoutedClientLifecycle>,
    mut query: Query<
        (
            Entity,
            &mut ClientJoinStatus,
            &RoutedClientSession,
            Option<&RoutedTransitionDeadline>,
        ),
        With<Client>,
    >,
) {
    if !owns_automatic_routed_recovery(&config) {
        return;
    }
    let now = time.elapsed();
    for (entity, mut status, session, transition_deadline) in &mut query {
        if status.disconnect_requested {
            let deadline = transition_deadline.map_or_else(
                || {
                    let deadline = now.saturating_add(config.connect_timeout);
                    commands
                        .entity(entity)
                        .insert(RoutedTransitionDeadline(deadline));
                    deadline
                },
                |deadline| deadline.0,
            );
            if now >= deadline {
                // The normal path has already had a full timeout window to produce Disconnected
                // and Unlinked. Force the transport boundary and remove the stale root so the
                // next schedule can spawn the appropriate fresh generation.
                warn!(
                    ?session.kind,
                    "routed session teardown deadline expired; forcing recovery"
                );
                commands.trigger(Unlink {
                    entity,
                    reason: UnlinkReason::TransportError(
                        "routed session teardown deadline expired".to_owned(),
                    ),
                });
                commands.entity(entity).despawn();
            }
            continue;
        }
        if matches!(
            status.phase,
            ClientJoinPhase::Active { .. } | ClientJoinPhase::LobbyActive { .. }
        ) || now < status.started_at.saturating_add(config.connect_timeout)
        {
            continue;
        }
        match session.kind {
            RoutedClientSessionKind::Lobby if lifecycle.phase == RoutedClientPhase::Lobby => {
                warn!("routed lobby session timed out; retrying with a fresh request");
                lifecycle.phase = RoutedClientPhase::AwaitingLobbyRetryUnlink;
            }
            RoutedClientSessionKind::Match if lifecycle.phase == RoutedClientPhase::Match => {
                warn!("routed match session timed out; returning to a fresh lobby request");
                lifecycle.phase = RoutedClientPhase::AwaitingMatchUnlink;
            }
            _ => continue,
        }
        status.disconnect_requested = true;
        commands.entity(entity).insert(RoutedTransitionDeadline(
            now.saturating_add(config.connect_timeout),
        ));
        commands.trigger(Disconnect { entity });
    }
}

pub(super) fn owns_automatic_routed_recovery(config: &ClientNetworkConfig) -> bool {
    config.transport == NetworkTransport::RoutedUdp && !config.presents_product_shell()
}

pub(super) fn disconnect_rejected_client(
    mut commands: Commands,
    mut query: Query<(Entity, &mut ClientJoinStatus), With<Client>>,
) {
    for (entity, mut status) in query.iter_mut() {
        if matches!(status.phase, ClientJoinPhase::Rejected(_)) && !status.disconnect_requested {
            status.disconnect_requested = true;
            commands.trigger(Disconnect { entity });
        }
    }
}
