//! Automation-only session drivers: the windowed synthetic-controller demo and the headless
//! product-match smoke exit. Compiled only with the `automation` feature.

use super::{
    AppExit, Client, ClientJoinPhase, ClientJoinStatus, Commands, Component, Controlled, Fighter,
    Gamepad, GamepadAxis, GamepadButton, MatchPhase, MatchRoot, MatchState, MessageWriter,
    NetworkEntityId, Position, Query, Res, RoutedClientSession, RoutedClientSessionKind, Vec2,
    With, info,
};

/// Marker for the reproducible, windowed controller smoke path.
#[derive(Component)]
pub(super) struct ControllerDemoGamepad;

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn spawn_controller_demo_gamepad(
    mut commands: Commands,
    config: Res<crate::ClientAutomationConfig>,
) {
    if config.windowed_controller_demo.is_some() {
        commands.spawn((Gamepad::default(), ControllerDemoGamepad));
        info!("windowed synthetic controller demo enabled");
    }
}

/// Keep the synthetic controller aimed at the server-owned neutral dummy while preserving the
/// normal gamepad sampling path. This is only a visual/input smoke aid; it is not gameplay logic.
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn update_controller_demo_gamepad(
    config: Res<crate::ClientAutomationConfig>,
    mut gamepads: Query<&mut Gamepad, With<ControllerDemoGamepad>>,
    controlled: Query<&Position, (With<Fighter>, With<Controlled>)>,
    fighters: Query<(&NetworkEntityId, &Position), With<Fighter>>,
) {
    if config.windowed_controller_demo.is_none() {
        return;
    }
    let aim = controlled
        .iter()
        .next()
        .and_then(|controlled| {
            fighters
                .iter()
                .find(|(network_id, _)| network_id.0 == 0)
                .map(|(_, dummy)| dummy.0 - controlled.0)
        })
        .filter(|delta| delta.is_finite() && delta.length_squared() > f32::EPSILON)
        .map_or(Vec2::X, Vec2::normalize);

    for mut gamepad in &mut gamepads {
        gamepad.analog_mut().set(GamepadAxis::LeftStickX, 0.0);
        gamepad.analog_mut().set(GamepadAxis::LeftStickY, 0.0);
        gamepad.analog_mut().set(GamepadAxis::RightStickX, aim.x);
        gamepad.analog_mut().set(GamepadAxis::RightStickY, aim.y);
        gamepad.analog_mut().set(GamepadButton::RightTrigger2, 1.0);
    }
}

/// Headless M05 evidence: exit successfully once the joined product match reaches Active.
/// With `--expect-published-map` the replicated map must also be a published recipe
/// (BRL-w9z1), so the run proves the community map reached this client.
#[allow(clippy::needless_pass_by_value)]
pub(super) fn finish_product_match_smoke(
    config: Res<crate::ClientAutomationConfig>,
    roots: Query<&MatchState, With<MatchRoot>>,
    maps: Query<&brawler::map::ResolvedMapIdentity, With<brawler::map::MapRoot>>,
    mut exit: MessageWriter<AppExit>,
) {
    if config.product_match_smoke
        && config.render_measurement.is_none()
        && !config.product_requeue_smoke
        && !config.product_network_change_smoke
        && !config.product_rejoin_smoke
        && config.product_completed_rejoin_cue.is_none()
        && config.leave_match_cue.is_none()
        && roots
            .iter()
            .any(|state| matches!(state.phase, MatchPhase::Active { .. }))
    {
        let identity = maps.iter().next();
        if let Some(identity) = identity {
            let marker = format!(
                "brawler-client match-map recipe_id={} recipe_fingerprint={}\n",
                identity.recipe_id.0, identity.recipe_fingerprint.0
            );
            let _ = std::io::Write::write_all(&mut std::io::stderr().lock(), marker.as_bytes());
        }
        if config.expect_published_map
            && identity.is_none_or(|identity| {
                identity.recipe_id.0 <= brawler::lobby::PUBLISHED_MAP_RECIPE_ID_BASE
            })
        {
            bevy::log::error!("brawler product match smoke expected a published community map");
            exit.write(AppExit::error());
            return;
        }
        exit.write(AppExit::Success);
    }
}

/// Headless BRL-0162 network-change rehearsal phases. Durations are wall-clock because the
/// supervisor's capability idle bound and Netcode's keep-alive bound are wall-clock too.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum NetworkChangeStage {
    #[default]
    AwaitingActive,
    Settling {
        until: core::time::Duration,
    },
    Outage {
        until: core::time::Duration,
        generation: u64,
    },
    Recovering {
        deadline: core::time::Duration,
        generation: u64,
        tick_at_rebind: u64,
    },
    Finished,
}

#[derive(bevy::prelude::Resource, Debug, Default)]
pub(super) struct NetworkChangeSmoke {
    stage: NetworkChangeStage,
    /// The source port the match session started on. The first outage moves to a new port; the
    /// second returns to this one, as a Wi-Fi -> cellular -> Wi-Fi flap would (BRL-sqz0).
    first_source: Option<std::net::SocketAddr>,
    returned: bool,
}

/// Settle time after Active before the rehearsed outage starts.
const NETWORK_CHANGE_SETTLE: core::time::Duration = core::time::Duration::from_secs(1);
/// Rehearsed outage: long enough to exceed every sub-second retransmission and the old 3 s
/// server-side default, comfortably inside the 10 s session bound.
const NETWORK_CHANGE_OUTAGE: core::time::Duration = core::time::Duration::from_secs(4);
/// Authoritative ticks the resumed session must deliver after the source-port change (1 s).
const NETWORK_CHANGE_RESUMED_TICKS: u64 = brawler::timing::SIMULATION_TICK_HZ;

/// Drive two mid-match outages, the first followed by a new source port and the second by a
/// return to the original port, and require the same routed match session to resume after each:
/// the session generation must not change, the client must stay Active, and fresh authoritative
/// ticks must arrive from the server after each rebind.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "one automation coordinator reads the schedule-owned session state it verifies"
)]
pub(super) fn drive_network_change_smoke(
    config: Res<crate::ClientAutomationConfig>,
    time: Res<bevy::prelude::Time<bevy::prelude::Real>>,
    mut smoke: bevy::prelude::ResMut<NetworkChangeSmoke>,
    lifecycle: Res<crate::RoutedClientLifecycle>,
    roots: Query<&MatchState, With<MatchRoot>>,
    ticks: Query<&brawler::combat::AuthoritativeTick>,
    mut clients: Query<
        (
            &RoutedClientSession,
            &ClientJoinStatus,
            &mut crate::routed_udp::RoutedUdpIo,
            &lightyear::prelude::LocalAddr,
        ),
        With<Client>,
    >,
    mut exit: MessageWriter<AppExit>,
) {
    if !config.product_network_change_smoke {
        return;
    }
    let now = time.elapsed();
    let newest_tick = ticks.iter().map(|tick| tick.0).max().unwrap_or(0);
    let Some((session, status, mut io, local_addr)) = clients.iter_mut().next() else {
        if !matches!(smoke.stage, NetworkChangeStage::AwaitingActive) {
            bevy::log::error!("network-change smoke lost its routed client entity");
            exit.write(AppExit::error());
        }
        return;
    };
    let match_session = session.kind == RoutedClientSessionKind::Match
        && lifecycle.phase == crate::RoutedClientPhase::Match;
    match smoke.stage {
        NetworkChangeStage::AwaitingActive => {
            if match_session
                && roots
                    .iter()
                    .any(|state| matches!(state.phase, MatchPhase::Active { .. }))
            {
                smoke.stage = NetworkChangeStage::Settling {
                    until: now.saturating_add(NETWORK_CHANGE_SETTLE),
                };
            }
        }
        NetworkChangeStage::Settling { until } if now >= until => {
            io.set_simulated_outage(true);
            info!(
                generation = session.generation,
                outage_ms = NETWORK_CHANGE_OUTAGE.as_millis(),
                "network-change smoke: dropping all routed traffic"
            );
            smoke.stage = NetworkChangeStage::Outage {
                until: now.saturating_add(NETWORK_CHANGE_OUTAGE),
                generation: session.generation,
            };
        }
        NetworkChangeStage::Outage { until, generation } if now >= until => {
            match restore_traffic(&mut smoke, &mut io, local_addr.0) {
                Ok(()) => {
                    smoke.stage = NetworkChangeStage::Recovering {
                        deadline: now.saturating_add(super::ROUTED_SESSION_TIMEOUT),
                        generation,
                        tick_at_rebind: newest_tick,
                    };
                }
                Err(error) => {
                    bevy::log::error!(%error, "network-change smoke could not rebind its socket");
                    exit.write(AppExit::error());
                    smoke.stage = NetworkChangeStage::Finished;
                }
            }
        }
        NetworkChangeStage::Recovering {
            deadline,
            generation,
            tick_at_rebind,
        } => {
            let same_session = match_session
                && session.generation == generation
                && matches!(status.phase, ClientJoinPhase::Active { .. });
            if !same_session {
                bevy::log::error!(
                    generation = session.generation,
                    ?status.phase,
                    "network-change smoke: the match session did not survive"
                );
                exit.write(AppExit::error());
                smoke.stage = NetworkChangeStage::Finished;
            } else if newest_tick >= tick_at_rebind.saturating_add(NETWORK_CHANGE_RESUMED_TICKS) {
                info!(
                    generation,
                    tick_at_rebind,
                    newest_tick,
                    returned = smoke.returned,
                    "network-change smoke: the same match session resumed after the source change"
                );
                if smoke.returned {
                    exit.write(AppExit::Success);
                    smoke.stage = NetworkChangeStage::Finished;
                } else {
                    // Second outage, then a return to the original source port.
                    smoke.stage = NetworkChangeStage::Settling { until: now };
                }
            } else if now >= deadline {
                bevy::log::error!(
                    tick_at_rebind,
                    newest_tick,
                    "network-change smoke: no authoritative progress after the source change"
                );
                exit.write(AppExit::error());
                smoke.stage = NetworkChangeStage::Finished;
            }
        }
        NetworkChangeStage::Settling { .. }
        | NetworkChangeStage::Outage { .. }
        | NetworkChangeStage::Finished => {}
    }
}

/// End a rehearsed outage: the first moves the socket to a new source port (from the configured
/// local address), the second returns it to the port the session started on.
fn restore_traffic(
    smoke: &mut NetworkChangeSmoke,
    io: &mut crate::routed_udp::RoutedUdpIo,
    configured: std::net::SocketAddr,
) -> std::io::Result<()> {
    let previous = io.local_addr();
    let returning = smoke.first_source.is_some();
    let target = if let Some(first) = smoke.first_source {
        first
    } else {
        let started = previous
            .ok_or_else(|| std::io::Error::other("the routed socket has no local address"))?;
        smoke.first_source = Some(started);
        configured
    };
    let rebound = io.rebind_socket(target)?;
    io.set_simulated_outage(false);
    smoke.returned = returning;
    if returning {
        info!(?previous, %rebound, "network-change smoke: traffic restored from the original source port");
    } else {
        info!(?previous, %rebound, "network-change smoke: traffic restored from a new source port");
    }
    Ok(())
}

/// Headless BRL-yyeg rejoin rehearsal phases. Wall-clock, like the routed bounds it exceeds.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum RejoinStage {
    #[default]
    AwaitingActive,
    Settling {
        until: core::time::Duration,
    },
    Outage {
        deadline: core::time::Duration,
    },
    /// Completed variant: the session is lost and the opponent was cued to leave; stay away
    /// while the match completes with this seat held.
    Offline {
        until: core::time::Duration,
    },
    Rejoining {
        deadline: core::time::Duration,
    },
    /// Completed variant: the forfeit result was captured; wait for the fresh lobby after it.
    AwaitingFreshLobby {
        deadline: core::time::Duration,
    },
    Resumed {
        deadline: core::time::Duration,
        tick_at_rejoin: u64,
    },
    Finished,
}

#[derive(bevy::prelude::Resource, Debug, Default)]
pub(super) struct RejoinSmoke {
    stage: RejoinStage,
    /// The match session that is lost: its generation, accepted player and match.
    original: Option<(
        u64,
        brawler::protocol::PlayerId,
        brawler::matchplay::MatchId,
    )>,
}

/// Rehearsed outage: longer than every routed session bound (10 s capability idle, 10 s Netcode
/// keep-alive), so the match session is declared lost by every party.
const REJOIN_OUTAGE_DEADLINE: core::time::Duration = core::time::Duration::from_secs(16);
/// Bound for the fresh lobby, the automatic rejoin, and the rejoined match session.
const REJOIN_DEADLINE: core::time::Duration = core::time::Duration::from_secs(30);
/// Completed variant: time to stay offline after cueing the opponent, so its Leave Match and
/// the forfeit land while this seat is held (BRL-jxfy). It stays below the 10 s automation
/// connect timeout after which the routed recovery would return to the lobby on its own.
const COMPLETED_REJOIN_OFFLINE: core::time::Duration = core::time::Duration::from_secs(6);

/// Drop all traffic of a Practice match session until it is lost, return to a fresh lobby the
/// way the product shell does, and require the automatic rejoin to reclaim the same player in
/// the same running match with fresh authoritative ticks. Runs before the lifecycle observer so
/// the expected loss is not classified as a terminal disconnect.
///
/// The completed variant (BRL-jxfy, a player-versus-player 1v1 client) writes its cue file once the session is
/// lost so the opponent leaves, stays offline while the match completes by forfeit with this
/// seat held, and then requires the automatic rejoin to land in the completed match: the rejoined
/// session must capture the forfeit result with its own team as the winner, never see the match
/// running again, and return to a fresh lobby.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one automation coordinator reads the schedule-owned session state it verifies"
)]
pub(super) fn drive_rejoin_smoke(
    config: Res<crate::ClientAutomationConfig>,
    time: Res<bevy::prelude::Time<bevy::prelude::Real>>,
    mut smoke: bevy::prelude::ResMut<RejoinSmoke>,
    mut lifecycle: bevy::prelude::ResMut<crate::RoutedClientLifecycle>,
    roots: Query<&MatchState, With<MatchRoot>>,
    ticks: Query<&brawler::combat::AuthoritativeTick>,
    result_state: Res<crate::ClientMatchResultState>,
    mut clients: Query<
        (
            &RoutedClientSession,
            &mut ClientJoinStatus,
            &mut crate::routed_udp::RoutedUdpIo,
            bevy::prelude::Has<lightyear::prelude::client::Disconnected>,
        ),
        With<Client>,
    >,
    mut exit: MessageWriter<AppExit>,
) {
    let completed_cue = config.product_completed_rejoin_cue.as_deref();
    if !config.product_rejoin_smoke && completed_cue.is_none() {
        return;
    }
    let now = time.elapsed();
    let newest_tick = ticks.iter().map(|tick| tick.0).max().unwrap_or(0);
    let match_id = roots.iter().next().map(|state| state.match_id);
    let match_active = roots
        .iter()
        .any(|state| matches!(state.phase, MatchPhase::Active { .. }));
    let client = clients.iter_mut().next();
    let mut fail = |smoke: &mut RejoinSmoke, message: &str| {
        bevy::log::error!(message, "rejoin smoke failed");
        exit.write(AppExit::error());
        smoke.stage = RejoinStage::Finished;
    };
    match smoke.stage {
        RejoinStage::AwaitingActive => {
            let Some((session, status, _, _)) = client else {
                return;
            };
            if let (RoutedClientSessionKind::Match, ClientJoinPhase::Active { player_id, .. }) =
                (session.kind, &status.phase)
                && lifecycle.phase == crate::RoutedClientPhase::Match
                && match_active
                && let Some(match_id) = match_id
            {
                smoke.original = Some((session.generation, *player_id, match_id));
                smoke.stage = RejoinStage::Settling {
                    until: now.saturating_add(NETWORK_CHANGE_SETTLE),
                };
            }
        }
        RejoinStage::Settling { until } if now >= until => {
            let Some((session, _, mut io, _)) = client else {
                return fail(&mut smoke, "the routed client entity disappeared");
            };
            io.set_simulated_outage(true);
            info!(
                generation = session.generation,
                "rejoin smoke: dropping all routed traffic beyond the session bound"
            );
            smoke.stage = RejoinStage::Outage {
                deadline: now.saturating_add(REJOIN_OUTAGE_DEADLINE),
            };
        }
        RejoinStage::Outage { deadline } => {
            let Some((session, mut status, _, disconnected)) = client else {
                return fail(&mut smoke, "the routed client entity disappeared");
            };
            let lost = session.kind == RoutedClientSessionKind::Match && disconnected;
            if lost && let Some(cue) = completed_cue {
                // An offline app has not noticed the loss yet: keep the lost session in place
                // while the opponent leaves and the match completes with this seat held. Marked
                // disconnected it is not a terminal failure, and restarting its clock keeps the
                // automatic routed recovery (one connect timeout) from returning to the lobby
                // before this smoke does.
                status.phase = ClientJoinPhase::Disconnected;
                status.started_at = now;
                if let Err(error) = std::fs::write(cue, b"leave\n") {
                    bevy::log::error!(%error, path = %cue.display(), "completed rejoin smoke could not write its cue");
                    return fail(&mut smoke, "the leave cue could not be written");
                }
                info!(
                    path = %cue.display(),
                    "completed rejoin smoke: the match session was lost; cued the opponent to leave"
                );
                smoke.stage = RejoinStage::Offline {
                    until: now.saturating_add(COMPLETED_REJOIN_OFFLINE),
                };
            } else if lost && lifecycle.request_return_to_lobby() {
                info!("rejoin smoke: the match session was lost; returning to a fresh lobby");
                smoke.stage = RejoinStage::Rejoining {
                    deadline: now.saturating_add(REJOIN_DEADLINE),
                };
            } else if now >= deadline {
                fail(&mut smoke, "the match session outlived the outage");
            }
        }
        RejoinStage::Offline { until } if now >= until => {
            if lifecycle.request_return_to_lobby() {
                info!("completed rejoin smoke: coming back; returning to a fresh lobby");
                smoke.stage = RejoinStage::Rejoining {
                    deadline: now.saturating_add(REJOIN_DEADLINE),
                };
            } else {
                fail(
                    &mut smoke,
                    "the lost match session could not return to the lobby",
                );
            }
        }
        RejoinStage::Rejoining { deadline } if completed_cue.is_some() => {
            let Some((original_generation, original_player, _)) = smoke.original else {
                return fail(&mut smoke, "no original match session was recorded");
            };
            let rejoined_session = client.is_some_and(|(session, status, _, _)| {
                session.kind == RoutedClientSessionKind::Match
                    && session.generation > original_generation
                    && matches!(
                        status.phase,
                        ClientJoinPhase::Active { player_id, .. } if player_id == original_player
                    )
            });
            if rejoined_session && match_active {
                return fail(&mut smoke, "the rejoin found the match still running");
            }
            if let Some(context) = result_state.context.as_ref() {
                let own_forfeit_victory = matches!(
                    context.result,
                    brawler::matchplay::MatchResult::Forfeit { winner, .. }
                        if context.local_team == Some(winner)
                );
                if context.local_player != Some(original_player) || !own_forfeit_victory {
                    bevy::log::error!(?context.result, ?context.local_team, ?context.local_player, "completed rejoin smoke captured an unexpected result");
                    return fail(
                        &mut smoke,
                        "the captured result is not this player's forfeit victory",
                    );
                }
                info!(
                    player_id = original_player.0,
                    result = ?context.result,
                    "completed rejoin smoke: the rejoined session captured the completed match's forfeit result"
                );
                smoke.stage = RejoinStage::AwaitingFreshLobby {
                    deadline: now.saturating_add(REJOIN_DEADLINE),
                };
            } else if now >= deadline {
                fail(
                    &mut smoke,
                    "the rejoin into the completed match did not complete",
                );
            }
        }
        RejoinStage::AwaitingFreshLobby { deadline } => {
            let Some((original_generation, _, _)) = smoke.original else {
                return fail(&mut smoke, "no original match session was recorded");
            };
            // Generations: original match, rejoin lobby, rejoined match, then this fresh lobby.
            let fresh_lobby = client.is_some_and(|(session, status, _, _)| {
                session.kind == RoutedClientSessionKind::Lobby
                    && session.generation >= original_generation.saturating_add(3)
                    && matches!(status.phase, ClientJoinPhase::LobbyActive { .. })
            });
            if fresh_lobby && !lifecycle.rejoin_in_progress() {
                info!("completed rejoin smoke: returned to a fresh lobby after the Results");
                exit.write(AppExit::Success);
                smoke.stage = RejoinStage::Finished;
            } else if now >= deadline {
                fail(&mut smoke, "no fresh lobby after the completed match");
            }
        }
        RejoinStage::Rejoining { deadline } => {
            let Some((original_generation, original_player, original_match)) = smoke.original
            else {
                return fail(&mut smoke, "no original match session was recorded");
            };
            let rejoined = client.is_some_and(|(session, status, _, _)| {
                session.kind == RoutedClientSessionKind::Match
                    && session.generation > original_generation
                    && matches!(
                        status.phase,
                        ClientJoinPhase::Active { player_id, .. } if player_id == original_player
                    )
            }) && lifecycle.phase == crate::RoutedClientPhase::Match
                && match_id == Some(original_match)
                && match_active;
            if rejoined {
                info!(
                    player_id = original_player.0,
                    "rejoin smoke: the same player rejoined the same running match"
                );
                smoke.stage = RejoinStage::Resumed {
                    deadline: now.saturating_add(REJOIN_DEADLINE),
                    tick_at_rejoin: newest_tick,
                };
            } else if now >= deadline {
                fail(&mut smoke, "the automatic rejoin did not complete");
            }
        }
        RejoinStage::Resumed {
            deadline,
            tick_at_rejoin,
        } => {
            if newest_tick >= tick_at_rejoin.saturating_add(NETWORK_CHANGE_RESUMED_TICKS) {
                info!(
                    tick_at_rejoin,
                    newest_tick,
                    "rejoin smoke: the rejoined match session receives authoritative ticks"
                );
                exit.write(AppExit::Success);
                smoke.stage = RejoinStage::Finished;
            } else if now >= deadline {
                fail(&mut smoke, "no authoritative progress after the rejoin");
            }
        }
        RejoinStage::Settling { .. } | RejoinStage::Offline { .. } | RejoinStage::Finished => {}
    }
}

/// Headless opponent for rejoin checks (BRL-jxfy): once the cue file exists while the routed
/// match is Active, leave the match through the product Leave Match path. The cue is polled at
/// most four times a second.
#[allow(clippy::needless_pass_by_value)]
pub(super) fn drive_leave_match_cue(
    config: Res<crate::ClientAutomationConfig>,
    time: Res<bevy::prelude::Time<bevy::prelude::Real>>,
    mut lifecycle: bevy::prelude::ResMut<crate::RoutedClientLifecycle>,
    roots: Query<&MatchState, With<MatchRoot>>,
    mut next_poll: bevy::prelude::Local<core::time::Duration>,
    mut left: bevy::prelude::Local<bool>,
) {
    let Some(cue) = config.leave_match_cue.as_deref() else {
        return;
    };
    let now = time.elapsed();
    if *left
        || now < *next_poll
        || lifecycle.phase != crate::RoutedClientPhase::Match
        || !roots
            .iter()
            .any(|state| matches!(state.phase, MatchPhase::Active { .. }))
    {
        return;
    }
    *next_poll = now.saturating_add(core::time::Duration::from_millis(250));
    if cue.exists() && lifecycle.request_leave_match() {
        *left = true;
        info!(path = %cue.display(), "leave-match cue: leaving the match");
    }
}

/// Finish the routed process smoke only after the match session has completed, its routes have
/// been revoked, and the client has authenticated one fresh lobby generation. Generation one is
/// the initial lobby and generation two is the match; a generation of three or greater therefore
/// cannot be satisfied by an initial lobby retry.
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn observe_fresh_lobby_return(
    config: Res<crate::ClientAutomationConfig>,
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
