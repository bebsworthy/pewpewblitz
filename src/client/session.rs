//! Client connection, compatibility handshake, selection, roster, and shutdown lifecycle.
#![allow(clippy::wildcard_imports)]

use super::*;

/// Installs the client Lightyear group, protocol, connection, and status systems.
pub struct ClientNetworkPlugin;

/// Deadline for an intentional routed session teardown. A normal teardown reaches `Unlinked`
/// quickly; if lifecycle markers never arrive, this bounded fallback lets the next generation
/// recover instead of leaving the client permanently in an awaiting phase.
#[derive(Component, Clone, Copy, Debug)]
pub(super) struct RoutedTransitionDeadline(pub(super) Duration);

/// Defers `Connect` until the freshly spawned client entity and all transport components are
/// materialized in the `World`. Product-shell connections are created from an `Update` system;
/// triggering `Connect` in that same deferred spawn boundary can run transport observers before
/// their query can see the new `RoutedUdpIo`, leaving the first attempt without a bound socket.
#[derive(Component)]
pub(super) struct PendingClientConnect;

impl Plugin for ClientNetworkPlugin {
    #[allow(clippy::too_many_lines)]
    fn build(&self, app: &mut App) {
        if app.world().resource::<ClientNetworkConfig>().transport == NetworkTransport::RoutedUdp {
            app.add_plugins(RoutedUdpPlugin);
        }
        configure_client_settings_ui(app);
        app.insert_resource(FallbackErrorHandler(error))
            .add_plugins(ClientCombatPlugin)
            .add_plugins(crate::terrain::ClientTerrainPlugin)
            .add_plugins(ClientQueuePlugin)
            .init_resource::<RosterLogState>()
            .init_resource::<ClientShutdown>()
            .init_resource::<PendingLocalActions>()
            .init_resource::<LiveInputTrace>()
            .init_resource::<HeadlessAutomation>()
            .init_resource::<InputDeviceActivity>()
            .init_resource::<ClientInputContext>()
            .init_resource::<ClientPlayableGate>()
            .init_resource::<BuildSelectionState>()
            .init_resource::<MatchCommandState>()
            .init_resource::<MatchLoadingCommandState>()
            .init_resource::<ClientInputSettings>()
            .init_resource::<InputSettingsSelection>()
            .init_resource::<RoutedClientLifecycle>()
            .init_resource::<ClientMatchResultState>()
            .init_resource::<crate::diagnostics::ProcessExitClassification>()
            .add_systems(
                Startup,
                (spawn_client_connection, spawn_controller_demo_gamepad).chain(),
            )
            .add_systems(
                RunFixedMainLoop,
                (
                    update_controller_demo_gamepad,
                    sample_local_input,
                    apply_headless_input.after(sample_local_input),
                )
                    .chain()
                    .in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
            )
            .add_systems(
                FixedPreUpdate,
                write_client_input.in_set(InputSystems::WriteClientInputs),
            )
            .add_systems(
                FixedUpdate,
                advance_headless_automation.in_set(crate::gameplay::GameplaySet::Finalize),
            )
            .add_systems(
                Update,
                (
                    connect_spawned_clients,
                    finish_spawned_client_connect,
                    send_client_hello,
                    process_join_outcome,
                    process_match_route_grant,
                    drive_routed_transition,
                    observe_routed_transition,
                    advance_routed_transition,
                    enforce_routed_timeout,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    process_build_selection_outcomes,
                    send_build_selection_request,
                    drive_match_loading_check_in,
                    process_match_command_outcomes,
                    send_match_command,
                    finish_product_match_smoke,
                    update_build_selection_overlay,
                    disconnect_rejected_client,
                    observe_client_lifecycle,
                    log_replicated_roster,
                    enforce_client_timeout,
                    trace_client_interpolation_sync,
                    trace_client_interpolation_history,
                )
                    .chain()
                    .after(enforce_routed_timeout),
            )
            .add_systems(
                Last,
                (
                    forward_app_exit_to_client_disconnect,
                    finish_client_shutdown,
                )
                    .chain()
                    // Order before the terminal observation set so closeout observations and
                    // the final report see post-shutdown counts and the re-emitted exit.
                    .before(crate::diagnostics::TerminalObservationSet),
            );
        // Keep this observer separate from the large lifecycle tuple (Bevy's tuple system
        // implementations are deliberately bounded), while preserving the grant->match and
        // completion->disconnect ordering explicitly.
        app.add_systems(
            Update,
            observe_fresh_lobby_return
                .after(process_join_outcome)
                .before(process_match_route_grant),
        );
        app.add_systems(
            Update,
            observe_completed_match
                .after(process_match_route_grant)
                .before(drive_routed_transition),
        );
        app.add_observer(add_controlled_input_marker);
    }
}

fn configure_client_settings_ui(app: &mut App) {
    app.configure_sets(
        Update,
        (
            ClientSettingsUiSet::Capture,
            ClientSettingsUiSet::Shell,
            ClientSettingsUiSet::Present,
        )
            .chain(),
    )
    .add_systems(
        Update,
        adjust_input_settings_from_pause_keys.in_set(ClientSettingsUiSet::Capture),
    )
    .add_systems(
        Update,
        update_input_settings_overlay.in_set(ClientSettingsUiSet::Present),
    );
}

fn process_match_command_outcomes(
    mut state: ResMut<MatchCommandState>,
    mut receivers: Query<Option<&mut MessageReceiver<MatchCommandOutcome>>, With<Client>>,
) {
    for receiver in &mut receivers {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for outcome in receiver.receive() {
            state.last_outcome = Some(outcome);
        }
    }
}

#[derive(Resource, Default)]
struct MatchLoadingCommandState {
    ready_sent_for: Option<(u64, u128, u128)>,
    ready_last_sent_at: Option<Duration>,
    cancel_sent_for: Option<(u64, u128, u128)>,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
fn drive_match_loading_check_in(
    config: Res<ClientNetworkConfig>,
    time: Res<Time<Real>>,
    mut routed: ResMut<RoutedClientLifecycle>,
    playable: Res<ClientPlayableGate>,
    mut loading: ResMut<ClientMatchLoadingModel>,
    mut state: ResMut<MatchLoadingCommandState>,
    roots: Query<&MatchState, With<MatchRoot>>,
    controlled: Query<
        &MatchParticipant,
        (With<Fighter>, With<Controlled>, Without<SelectingBuild>),
    >,
    mut clients: Query<
        (
            &mut MessageSender<MatchLoadingClientMessage>,
            Option<&mut MessageReceiver<MatchLoadingServerMessage>>,
            Option<&mut MessageReceiver<MatchLoadingStatus>>,
        ),
        With<Client>,
    >,
) {
    if config.transport != NetworkTransport::RoutedUdp
        || (!config.presents_product_shell() && !config.product_match_smoke)
        || routed.phase != RoutedClientPhase::Match
    {
        return;
    }
    let Some(grant) = routed.accepted_grant else {
        return;
    };
    let correlation = (
        grant.request_id.get(),
        grant.allocation_id.get(),
        grant.match_id.get(),
    );
    let cancel_requested = loading.take_match_cancel_requested();
    for (mut sender, receiver, status_receiver) in &mut clients {
        if let Some(mut status_receiver) = status_receiver {
            for status in status_receiver.receive() {
                if (status.request_id, status.allocation_id, status.match_id) == correlation {
                    loading.observe_status(status);
                }
            }
        }
        if let Some(mut receiver) = receiver {
            for outcome in receiver.receive() {
                if (outcome.request_id, outcome.allocation_id, outcome.match_id) != correlation {
                    continue;
                }
                match outcome.outcome {
                    crate::protocol::MatchLoadingServerOutcome::CancellationAccepted
                    | crate::protocol::MatchLoadingServerOutcome::TerminalFailure => {
                        loading.observe_match_cancellation(true);
                        let _ = routed.request_return_to_lobby();
                    }
                    crate::protocol::MatchLoadingServerOutcome::CancellationTooLate => {
                        loading.observe_match_cancellation(false);
                    }
                }
            }
        }
        if cancel_requested && state.cancel_sent_for != Some(correlation) {
            sender.send::<SessionChannel>(MatchLoadingClientMessage {
                request_id: correlation.0,
                allocation_id: correlation.1,
                match_id: correlation.2,
                action: MatchLoadingClientAction::CancelMatchStart,
            });
            state.cancel_sent_for = Some(correlation);
            continue;
        }
        let retry_due = state.ready_sent_for != Some(correlation)
            || state.ready_last_sent_at.is_none_or(|sent_at| {
                time.elapsed().saturating_sub(sent_at) >= Duration::from_millis(500)
            });
        if !playable.0
            || loading.phase() == Some(crate::lobby::MatchLoadingPhase::Cancelling)
            || !retry_due
            || !roots.single().is_ok_and(|root| {
                root.match_id.0 == correlation.2 && matches!(root.phase, MatchPhase::Waiting)
            })
            || controlled.single().is_err()
        {
            continue;
        }
        sender.send::<SessionChannel>(MatchLoadingClientMessage {
            request_id: correlation.0,
            allocation_id: correlation.1,
            match_id: correlation.2,
            action: MatchLoadingClientAction::Ready,
        });
        if config.render_measurement.is_some() && state.ready_sent_for != Some(correlation) {
            eprintln!(
                "brawler-client timing match-ready client_id={} request_id={} ts_ms={}",
                config.client_id,
                correlation.0,
                crate::diagnostics::unix_micros_now() / 1_000
            );
        }
        state.ready_sent_for = Some(correlation);
        state.ready_last_sent_at = Some(time.elapsed());
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
#[allow(clippy::too_many_arguments)]
fn send_match_command(
    config: Res<ClientNetworkConfig>,
    routed: Res<RoutedClientLifecycle>,
    playable: Res<ClientPlayableGate>,
    mut state: ResMut<MatchCommandState>,
    pending: Res<PendingLocalActions>,
    roots: Query<&MatchState, With<MatchRoot>>,
    controlled: Query<
        &MatchParticipant,
        (With<Fighter>, With<Controlled>, Without<SelectingBuild>),
    >,
    authoritative_ticks: Query<&AuthoritativeTick, (With<Fighter>, With<Controlled>)>,
    roster: Query<(), (With<Remote>, With<Fighter>)>,
    mut senders: Query<&mut MessageSender<MatchCommandRequest>, With<Client>>,
) {
    if config.transport == NetworkTransport::RoutedUdp && routed.phase != RoutedClientPhase::Match {
        return;
    }
    let Ok(match_state) = roots.single() else {
        return;
    };
    let Ok(participant) = controlled.single() else {
        return;
    };
    let pressed = pending.action_indicator & ACTION_INTERACT != 0;
    let product_routed =
        config.transport == NetworkTransport::RoutedUdp && config.presents_product_shell();
    if product_routed {
        return;
    }
    let automatic = if product_routed {
        playable.0
    } else {
        automatic_match_command_enabled(&config, roster.iter().count())
    };
    if should_rearm_headless_match_command(
        config.headless_simulation_ticks.is_some(),
        state.sent_for_phase,
        match_state.match_id,
        match_state.phase,
    ) {
        // A countdown departure returns the same match ID to Waiting. Re-arm automation while the
        // countdown is still observable so that the unchanged Waiting key can be sent again.
        state.sent_for_phase = None;
    }
    let command = match match_state.phase {
        MatchPhase::Waiting
            if !participant.ready && ((!product_routed && pressed) || automatic) =>
        {
            Some(MatchCommand::SetReady(true))
        }
        MatchPhase::Completed {
            restart_unlocked_at_tick,
            ..
        } if !product_routed
            && !participant.restart_ready
            && authoritative_ticks
                .iter()
                .next()
                .is_some_and(|tick| tick.0 >= restart_unlocked_at_tick)
            && (pressed || automatic) =>
        {
            Some(MatchCommand::ReadyForRestart)
        }
        _ => None,
    };
    let Some(command) = command else {
        return;
    };
    if config.headless_simulation_ticks.is_some()
        && state.sent_for_phase == Some((match_state.match_id, match_state.phase))
    {
        return;
    }
    state.next_request_id = state.next_request_id.saturating_add(1).max(1);
    for mut sender in &mut senders {
        sender.send::<SessionChannel>(MatchCommandRequest {
            request_id: state.next_request_id,
            match_id: match_state.match_id,
            command,
        });
    }
    state.sent_for_phase = Some((match_state.match_id, match_state.phase));
}

#[allow(clippy::needless_pass_by_value)]
fn finish_product_match_smoke(
    config: Res<ClientNetworkConfig>,
    roots: Query<&MatchState, With<MatchRoot>>,
    mut exit: MessageWriter<AppExit>,
) {
    if config.product_match_smoke
        && config.render_measurement.is_none()
        && !config.product_requeue_smoke
        && roots
            .iter()
            .any(|state| matches!(state.phase, MatchPhase::Active { .. }))
    {
        exit.write(AppExit::Success);
    }
}

pub(super) fn should_rearm_headless_match_command(
    automation_enabled: bool,
    sent_for_phase: Option<(crate::matchplay::MatchId, MatchPhase)>,
    match_id: crate::matchplay::MatchId,
    phase: MatchPhase,
) -> bool {
    automation_enabled
        && matches!(phase, MatchPhase::Countdown { .. })
        && sent_for_phase == Some((match_id, MatchPhase::Waiting))
}

pub(super) fn automatic_match_command_enabled(
    config: &ClientNetworkConfig,
    roster_count: usize,
) -> bool {
    (config.headless_simulation_ticks.is_some()
        || config.windowed_combat_demo.is_some()
        || config.windowed_controller_demo.is_some()
        || config.render_measurement.is_some())
        && config
            .exit_after_roster
            .is_none_or(|target| roster_count >= target)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn spawn_client_connection(
    mut commands: Commands,
    config: Res<ClientNetworkConfig>,
    time: Res<Time<Real>>,
    mut routed: ResMut<RoutedClientLifecycle>,
) -> Result {
    if !config.connects_on_startup() {
        info!(
            mode = "client",
            "brawler client awaiting product-shell action"
        );
        return Ok(());
    }
    if config.transport == NetworkTransport::RoutedUdp {
        routed.start_lobby();
        let generation = routed.generation;
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
        info!(
            mode = "client",
            transport = "routed-udp",
            version = VERSION,
            tick_hz = crate::timing::SIMULATION_TICK_HZ,
            client_id = config.client_id,
            server = %config.server_addr,
            generation,
            "brawler client connecting to lobby selector"
        );
        return Ok(());
    }
    if config.transport != NetworkTransport::Udp {
        return Ok(());
    }
    spawn_client_entity(&mut commands, &config, time.elapsed(), None)?;
    info!(
        mode = "client",
        version = VERSION,
        tick_hz = crate::timing::SIMULATION_TICK_HZ,
        client_id = config.client_id,
        server = %config.server_addr,
        "brawler client connecting"
    );
    Ok(())
}

/// Spawn exactly one fresh Lightyear client entity. `routed` is either a lobby/match adapter and
/// generation marker or `None` for the unchanged direct-UDP baseline.
fn spawn_client_entity(
    commands: &mut Commands,
    config: &ClientNetworkConfig,
    started_at: Duration,
    routed: Option<(RoutedUdpIo, RoutedClientSession)>,
) -> Result<Entity> {
    let auth = Authentication::Manual {
        server_addr: config.server_addr,
        client_id: config.client_id,
        private_key: crate::protocol::DEVELOPMENT_PRIVATE_KEY,
        protocol_id: config.network_protocol_id,
    };
    let netcode_config =
        NetcodeConfig {
            client_timeout_secs: config.connect_timeout.as_secs().try_into().map_err(
                |_| "client connect timeout does not fit in Netcode's i32 seconds field",
            )?,
            token_expire_secs: -1,
            ..default()
        };
    let netcode = NetcodeClient::new(auth, netcode_config)?;
    let status = ClientJoinStatus {
        phase: ClientJoinPhase::Connecting,
        started_at,
        disconnect_requested: false,
    };
    let entity = if let Some((io, session)) = routed {
        commands
            .spawn((
                status,
                session,
                PingManager::default(),
                ReplicationReceiver,
                Link::default()
                    .with_mtu(LinkMtu::new(ROUTED_LINK_MTU))
                    .with_conditioner(config.impairment_profile.receive_conditioner()),
                netcode,
                LocalAddr(config.local_addr),
                PeerAddr(config.server_addr),
                io,
                PendingClientConnect,
                Name::new(format!("Brawler routed client {}", config.client_id)),
            ))
            .id()
    } else {
        commands
            .spawn((
                status,
                PingManager::default(),
                ReplicationReceiver,
                Link::default().with_conditioner(config.impairment_profile.receive_conditioner()),
                netcode,
                LocalAddr(config.local_addr),
                PeerAddr(config.server_addr),
                UdpIo::default(),
                PendingClientConnect,
                Name::new(format!("Brawler client {}", config.client_id)),
            ))
            .id()
    };
    Ok(entity)
}

/// Start a client only after its deferred spawn has reached the world. This system is first in
/// the session chain, so connection observers complete before later session observation while
/// preserving the normal Lightyear receive/send schedules.
#[allow(clippy::needless_pass_by_value)]
fn connect_spawned_clients(
    mut commands: Commands,
    clients: Query<Entity, Added<PendingClientConnect>>,
) {
    for entity in &clients {
        // NetcodeClient's required initial lifecycle markers are useful for a statically spawned
        // endpoint, but a product-shell entity lives for one deferred boundary before this
        // system runs. Clear those initial markers first so they cannot be observed as a real
        // failed attempt before `Connect` installs `Connecting` and the routed socket's `Linked`.
        commands.entity(entity).remove::<(Unlinked, Disconnected)>();
        commands.trigger(Connect { entity });
    }
}

#[allow(clippy::needless_pass_by_value)]
fn finish_spawned_client_connect(
    mut commands: Commands,
    clients: Query<Entity, (With<PendingClientConnect>, With<Connecting>)>,
) {
    for entity in &clients {
        commands.entity(entity).remove::<PendingClientConnect>();
    }
}

pub(super) struct ProductLobbyAttempt {
    pub started_at: Duration,
    pub server_addr: std::net::SocketAddr,
    pub logical_address: String,
    pub proposed_display_name: String,
    pub netcode_timeout: Duration,
}

pub(super) fn spawn_product_lobby_connection(
    commands: &mut Commands,
    config: &ClientNetworkConfig,
    routed: &mut RoutedClientLifecycle,
    attempt: ProductLobbyAttempt,
) -> Result<Entity> {
    let mut attempt_config = config.clone();
    attempt_config.server_addr = attempt.server_addr;
    attempt_config.local_addr = match attempt.server_addr {
        std::net::SocketAddr::V4(_) => "0.0.0.0:0".parse().expect("wildcard IPv4 address is valid"),
        std::net::SocketAddr::V6(_) => "[::]:0".parse().expect("wildcard IPv6 address is valid"),
    };
    attempt_config.connect_timeout = attempt.netcode_timeout;
    routed.start_lobby();
    let generation = routed.generation;
    let entity = spawn_client_entity(
        commands,
        &attempt_config,
        attempt.started_at,
        Some((
            RoutedUdpIo::lobby(attempt.server_addr),
            RoutedClientSession {
                generation,
                kind: RoutedClientSessionKind::Lobby,
            },
        )),
    )?;
    commands.entity(entity).insert(RuntimeLobbyTarget {
        logical_address: attempt.logical_address,
        proposed_display_name: attempt.proposed_display_name,
    });
    Ok(entity)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn spawn_controller_demo_gamepad(
    mut commands: Commands,
    config: Res<ClientNetworkConfig>,
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
    config: Res<ClientNetworkConfig>,
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

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn send_client_hello(
    config: Res<ClientNetworkConfig>,
    fingerprint: Res<ProtocolFingerprint>,
    content_fingerprint: Res<crate::content::GameplayContentFingerprint>,
    time: Res<Time<Real>>,
    routed: Res<RoutedClientLifecycle>,
    mut query: Query<
        (
            &mut ClientJoinStatus,
            Option<&mut MessageSender<MatchHello>>,
            Option<&mut MessageSender<LobbyHello>>,
            Option<&RoutedClientSession>,
            Option<&RuntimeLobbyTarget>,
        ),
        (With<Client>, With<Connected>),
    >,
) {
    for (mut status, match_sender, lobby_sender, routed_session, runtime_target) in query.iter_mut()
    {
        if matches!(status.phase, ClientJoinPhase::Connecting) {
            if routed_session.is_some_and(|session| session.kind == RoutedClientSessionKind::Lobby)
            {
                let Some(mut sender) = lobby_sender else {
                    continue;
                };
                sender.send::<SessionChannel>(LobbyHello {
                    protocol_version: config.expected_protocol_version,
                    build_version: config.expected_build_version.clone(),
                    registry_fingerprint: fingerprint.0,
                    content_fingerprint: *content_fingerprint,
                    proposed_display_name: runtime_target.map_or_else(
                        || crate::lobby::generated_display_name(config.client_id),
                        |target| target.proposed_display_name.clone(),
                    ),
                });
            } else {
                let Some(mut sender) = match_sender else {
                    continue;
                };
                sender.send::<SessionChannel>(MatchHello {
                    protocol_version: config.expected_protocol_version,
                    build_version: config.expected_build_version.clone(),
                    registry_fingerprint: fingerprint.0,
                    content_fingerprint: *content_fingerprint,
                });
            }
            status.phase = ClientJoinPhase::AwaitingOutcome;
            status.started_at = time.elapsed();
            info!("brawler client connected; awaiting compatibility outcome");
            if routed_session.is_some_and(|session| session.kind == RoutedClientSessionKind::Match)
                && let Some(request_id) = routed.current_request_id
            {
                // This marker intentionally contains only stable correlation IDs. It is emitted
                // at the Lightyear Connected boundary for the fresh match session; capabilities,
                // player identities, and manifests are never logged.
                let timestamp_ms = std::time::SystemTime::now()
                    .duration_since(std::time::UNIX_EPOCH)
                    .map_or(0, |duration| duration.as_millis());
                let marker = format!(
                    "brawler-client timing handoff-connected client_id={} request_id={} ts_ms={}\n",
                    config.client_id,
                    request_id.get(),
                    timestamp_ms,
                );
                // Both verification clients inherit one stderr file descriptor. Format the whole
                // bounded marker first and issue one write so process output cannot splice stable
                // IDs into an unparsable half-line.
                let _ = std::io::Write::write_all(&mut std::io::stderr().lock(), marker.as_bytes());
            }
        }
    }
}

pub(super) fn process_build_selection_outcomes(
    mut state: ResMut<BuildSelectionState>,
    mut receivers: Query<Option<&mut MessageReceiver<BuildSelectionOutcome>>, With<Client>>,
) {
    for receiver in &mut receivers {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for outcome in receiver.receive() {
            state.last_outcome = Some(outcome);
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
#[allow(clippy::too_many_arguments)]
#[allow(clippy::too_many_lines)]
pub(super) fn send_build_selection_request(
    config: Res<ClientNetworkConfig>,
    routed: Res<RoutedClientLifecycle>,
    mut state: ResMut<BuildSelectionState>,
    catalog: Res<crate::builds::BuildCatalogResource>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    gamepads: Query<&Gamepad>,
    fighters: Query<(), (With<Fighter>, With<Controlled>, With<SelectingBuild>)>,
    matches: Query<&MatchState, With<MatchRoot>>,
    statuses: Query<&ClientJoinStatus, With<Client>>,
    mut senders: Query<&mut MessageSender<BuildSelectionRequest>, With<Client>>,
) {
    if config.presents_product_shell() {
        return;
    }
    if config.transport == NetworkTransport::RoutedUdp && routed.phase != RoutedClientPhase::Match {
        return;
    }
    if !statuses
        .iter()
        .any(|status| matches!(status.phase, ClientJoinPhase::Active { .. }))
        || fighters.iter().next().is_none()
    {
        return;
    }
    let Ok(match_state) = matches.single() else {
        return;
    };
    if state.last_match_id != Some(match_state.match_id) {
        state.last_match_id = Some(match_state.match_id);
        state.last_sent = None;
        state.last_outcome = None;
    }
    let keyboard = keyboard.as_deref();
    let left = keyboard.is_some_and(|keys| {
        keys.just_pressed(KeyCode::ArrowLeft) || keys.just_pressed(KeyCode::KeyA)
    }) || gamepads
        .iter()
        .any(|gamepad| gamepad.just_pressed(GamepadButton::DPadLeft));
    let right = keyboard.is_some_and(|keys| {
        keys.just_pressed(KeyCode::ArrowRight) || keys.just_pressed(KeyCode::KeyD)
    }) || gamepads
        .iter()
        .any(|gamepad| gamepad.just_pressed(GamepadButton::DPadRight));
    let up = keyboard.is_some_and(|keys| {
        keys.just_pressed(KeyCode::ArrowUp) || keys.just_pressed(KeyCode::KeyW)
    }) || gamepads
        .iter()
        .any(|gamepad| gamepad.just_pressed(GamepadButton::DPadUp));
    let down = keyboard.is_some_and(|keys| {
        keys.just_pressed(KeyCode::ArrowDown) || keys.just_pressed(KeyCode::KeyS)
    }) || gamepads
        .iter()
        .any(|gamepad| gamepad.just_pressed(GamepadButton::DPadDown));
    let cancel = keyboard.is_some_and(|keys| keys.just_pressed(KeyCode::Escape))
        || gamepads
            .iter()
            .any(|gamepad| gamepad.just_pressed(GamepadButton::East));
    if cancel && state.current_index == 4 {
        state.current_index = 0;
        state.custom_field = 0;
        return;
    }
    let stick_x = gamepads
        .iter()
        .find_map(|gamepad| gamepad.get(GamepadAxis::LeftStickX))
        .unwrap_or(0.0);
    let stick_y = gamepads
        .iter()
        .find_map(|gamepad| gamepad.get(GamepadAxis::LeftStickY))
        .unwrap_or(0.0);
    let BuildSelectionState {
        analog_x_ready,
        analog_y_ready,
        ..
    } = &mut *state;
    let (analog_left, analog_right, analog_up, analog_down) =
        editor_axis_edges(Vec2::new(stick_x, stick_y), analog_x_ready, analog_y_ready);
    if left || analog_left {
        if state.current_index == 4 {
            edit_custom_recipe(&mut state, -1);
        } else {
            state.current_index = (state.current_index + 4) % 5;
        }
    } else if right || analog_right {
        if state.current_index == 4 {
            edit_custom_recipe(&mut state, 1);
        } else {
            state.current_index = (state.current_index + 1) % 5;
        }
    } else if state.current_index == 4 && (up || analog_up) {
        state.custom_field = (state.custom_field + 5) % 6;
    } else if state.current_index == 4 && (down || analog_down) {
        state.custom_field = (state.custom_field + 1) % 6;
    }
    let confirm = keyboard
        .is_some_and(|keys| keys.just_pressed(KeyCode::Space) || keys.just_pressed(KeyCode::Enter))
        || gamepads
            .iter()
            .any(|gamepad| gamepad.just_pressed(GamepadButton::South));
    let automatic = config.headless
        || crossbeam_transport(&config)
        || cfg!(feature = "network-test")
        || config.windowed_combat_demo.is_some()
        || config.windowed_controller_demo.is_some()
        || config.render_measurement.is_some();
    let should_send = automatic && state.last_sent.is_none() || confirm;
    if !should_send {
        return;
    }
    if state.last_sent.is_some() && !confirm {
        return;
    }
    if let Some(preset) = config.build_preset {
        state.current_index = usize::from(preset.saturating_sub(1).min(4));
    }
    state.next_request_id = state.next_request_id.saturating_add(1).max(1);
    let request = BuildSelectionRequest {
        request_id: state.next_request_id,
        match_id: match_state.match_id,
        selection: catalog
            .0
            .presets
            .get(state.current_index)
            .map_or(BuildSelection::Custom(state.custom_recipe), |preset| {
                BuildSelection::Preset(preset.id)
            }),
    };
    for mut sender in &mut senders {
        sender.send::<SessionChannel>(request);
    }
    state.last_sent = Some(request.request_id);
}

pub(super) fn editor_axis_edges(
    stick: Vec2,
    x_ready: &mut bool,
    y_ready: &mut bool,
) -> (bool, bool, bool, bool) {
    if stick.x.abs() < 0.3 {
        *x_ready = true;
    }
    if stick.y.abs() < 0.3 {
        *y_ready = true;
    }
    let horizontal = stick.x.abs() >= stick.y.abs();
    let left = horizontal && stick.x < -0.6 && *x_ready;
    let right = horizontal && stick.x > 0.6 && *x_ready;
    let up = !horizontal && stick.y > 0.6 && *y_ready;
    let down = !horizontal && stick.y < -0.6 && *y_ready;
    if left || right {
        *x_ready = false;
    }
    if up || down {
        *y_ready = false;
    }
    (left, right, up, down)
}

fn edit_custom_recipe(state: &mut BuildSelectionState, delta: i8) {
    use crate::builds::{
        PassiveDefinitionId, PulseMagazine, PulsePower, PulseReach, UltimateDefinitionId,
        WeaponChoice,
    };
    let WeaponChoice::CustomPulse {
        mut power,
        mut reach,
        mut magazine,
    } = state.custom_recipe.weapon
    else {
        return;
    };
    let step = |current: u16, count: u16| {
        u16::try_from(
            ((i32::from(current) - 1 + i32::from(delta)).rem_euclid(i32::from(count))) + 1,
        )
        .expect("bounded editor field index fits u16")
    };
    match state.custom_field {
        0 => {
            power = [PulsePower::Light, PulsePower::Balanced, PulsePower::Heavy][usize::from(
                step(
                    match power {
                        PulsePower::Light => 1,
                        PulsePower::Balanced => 2,
                        PulsePower::Heavy => 3,
                    },
                    3,
                ) - 1,
            )];
        }
        1 => {
            reach = [PulseReach::Compact, PulseReach::Standard, PulseReach::Long][usize::from(
                step(
                    match reach {
                        PulseReach::Compact => 1,
                        PulseReach::Standard => 2,
                        PulseReach::Long => 3,
                    },
                    3,
                ) - 1,
            )];
        }
        2 => {
            magazine = [
                PulseMagazine::Quick,
                PulseMagazine::Standard,
                PulseMagazine::Expanded,
            ][usize::from(
                step(
                    match magazine {
                        PulseMagazine::Quick => 1,
                        PulseMagazine::Standard => 2,
                        PulseMagazine::Expanded => 3,
                    },
                    3,
                ) - 1,
            )];
        }
        3 => {
            state.custom_recipe.ultimate =
                UltimateDefinitionId(step(state.custom_recipe.ultimate.0, 2));
        }
        4 => {
            state.custom_recipe.passives[0] =
                PassiveDefinitionId(step(state.custom_recipe.passives[0].0, 6));
        }
        _ => {
            state.custom_recipe.passives[1] =
                PassiveDefinitionId(step(state.custom_recipe.passives[1].0, 6));
        }
    }
    state.custom_recipe.weapon = WeaponChoice::CustomPulse {
        power,
        reach,
        magazine,
    };
}

pub(super) fn crossbeam_transport(config: &ClientNetworkConfig) -> bool {
    #[cfg(feature = "network-test")]
    {
        matches!(config.transport, NetworkTransport::Crossbeam)
    }
    #[cfg(not(feature = "network-test"))]
    {
        let _ = config;
        false
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
#[allow(clippy::too_many_lines)]
pub(super) fn update_build_selection_overlay(
    config: Res<ClientNetworkConfig>,
    state: Res<BuildSelectionState>,
    catalog: Res<crate::builds::BuildCatalogResource>,
    weapons: Res<crate::combat::WeaponCatalogResource>,
    fighter_definitions: Res<crate::combat::FighterDefinitions>,
    fighters: Query<(), (With<Fighter>, With<Controlled>, With<SelectingBuild>)>,
    mut overlay: Query<(&mut Text, &mut Visibility), With<BuildSelectionText>>,
) {
    if config.presents_product_shell() {
        for (_, mut visibility) in &mut overlay {
            *visibility = Visibility::Hidden;
        }
        return;
    }
    let selecting = fighters.iter().next().is_some();
    let preset = catalog.0.presets.get(state.current_index);
    let current = preset.map_or("Custom Pulse", |preset| preset.display_name.as_str());
    let recipe = preset.map_or(state.custom_recipe, |preset| preset.recipe);
    let ultimate = catalog
        .0
        .ultimates
        .iter()
        .find(|definition| definition.id == recipe.ultimate)
        .map_or("Unknown ultimate", |definition| {
            definition.display_name.as_str()
        });
    let passive_name = |id| {
        catalog
            .0
            .passives
            .iter()
            .find(|definition| definition.id == id)
            .map_or("Unknown passive", |definition| {
                definition.display_name.as_str()
            })
    };
    let loadout_line = format!(
        "{ultimate} | {} + {}",
        passive_name(recipe.passives[0]),
        passive_name(recipe.passives[1])
    );
    let custom_line = match recipe.weapon {
        crate::builds::WeaponChoice::CustomPulse {
            power,
            reach,
            magazine,
        } => {
            let fields = [
                "Power",
                "Reach",
                "Magazine",
                "Ultimate",
                "Passive 1",
                "Passive 2",
            ];
            format!(
                "Custom Pulse: {power:?} / {reach:?} / {magazine:?} | editing {}",
                fields[state.custom_field]
            )
        }
        crate::builds::WeaponChoice::Preset(id) => format!("Weapon preset {}", id.0),
    };
    let profile = match preset.map_or(0, |preset| preset.id.0) {
        1 => "steady mid-range pressure; cover and rushes counter it",
        2 => "close burst; cone, falloff, and reload punish misses",
        3 => "cover/group punish; telegraphed landing and dead zone",
        4 => "close displacement burst; kite outside danger range",
        _ => "custom fields: power, reach, magazine, ultimate, passive 1, passive 2",
    };
    let preview = crate::builds::resolve_build_recipe(
        &catalog.0,
        &weapons.0,
        &fighter_definitions.entries[0],
        recipe,
        preset.map(|preset| preset.id),
    )
    .map_or_else(
        |error| format!("PROVISIONAL: invalid ({error:?})"),
        |loadout| format!("PROVISIONAL: {}/12 points", loadout.total_points),
    );
    let status = state
        .last_outcome
        .map_or("Awaiting server".to_string(), |outcome| {
            match outcome.decision {
                BuildSelectionDecision::Accepted => {
                    "Accepted; waiting for replicated state".to_string()
                }
                BuildSelectionDecision::UnknownId => {
                    "Server rejected: unknown content ID".to_string()
                }
                BuildSelectionDecision::WrongMatch => "Server rejected: wrong match".to_string(),
                BuildSelectionDecision::WrongPhase => {
                    "Server rejected: selection phase closed".to_string()
                }
                BuildSelectionDecision::ReadyLocked => {
                    "Server rejected: ready locks the build".to_string()
                }
                BuildSelectionDecision::Stale => "Server rejected: stale request".to_string(),
                BuildSelectionDecision::InvalidSlots => {
                    "Server rejected: invalid slots".to_string()
                }
                BuildSelectionDecision::InvalidCombination => {
                    "Server rejected: invalid combination".to_string()
                }
                BuildSelectionDecision::OverBudget => "Server rejected: over 12 points".to_string(),
                BuildSelectionDecision::CandidateTooLarge => {
                    "Server rejected: candidate too large".to_string()
                }
                BuildSelectionDecision::ResolutionFailed => {
                    "Server rejected: recipe failed validation".to_string()
                }
            }
        });
    for (mut text, mut visibility) in &mut overlay {
        *visibility = if selecting {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        **text = format!(
            "Select build: Left/Right or A/D changes choice | Up/Down changes custom field | Space / South confirms | Esc / East returns\n{current} | {custom_line}\n{loadout_line}\n{profile}\n{preview}\n{status}"
        );
    }
}

/// Map one server join rejection to the stable failure category its evidence belongs to.
pub(super) fn join_rejection_category(
    reason: &MatchJoinRejection,
) -> crate::diagnostics::FailureCategory {
    match reason {
        MatchJoinRejection::ProtocolVersionMismatch
        | MatchJoinRejection::BuildVersionMismatch
        | MatchJoinRejection::RegistryMismatch => {
            crate::diagnostics::FailureCategory::ProtocolMismatch
        }
        MatchJoinRejection::ContentMismatch => crate::diagnostics::FailureCategory::ContentMismatch,
        MatchJoinRejection::HandshakeTimeout => crate::diagnostics::FailureCategory::Timeout,
        MatchJoinRejection::ServerFull
        | MatchJoinRejection::MatchFull
        | MatchJoinRejection::MatchInProgress
        | MatchJoinRejection::IdentifierExhausted => {
            crate::diagnostics::FailureCategory::ShutdownIncomplete
        }
    }
}

/// Classify a client error exit and append the bounded local failure record when the
/// `BRAWLER_FAILURE_REPORT` control selects one, so client failures keep the same stable
/// categories the dedicated server already records.
fn record_client_failure(
    diagnostics: Option<&Res<crate::diagnostics::ProcessDiagnosticsSettings>>,
    classification: &mut crate::diagnostics::ProcessExitClassification,
    category: crate::diagnostics::FailureCategory,
    message: String,
) {
    classification.record_error_exit(category.into());
    if let Some(settings) = diagnostics
        && let Some(path) = settings.failure_record_path()
    {
        crate::diagnostics::write_failure_record(
            &path,
            &crate::diagnostics::ProcessFailureRecordV1::new(category, message),
        );
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_lines,
    reason = "the role-specific ordered outcome decoder keeps conflict and terminal policy visible"
)]
pub(super) fn process_join_outcome(
    mut commands: Commands,
    mut query: Query<
        (
            Entity,
            &mut ClientJoinStatus,
            Option<&mut MessageReceiver<MatchJoinOutcome>>,
            Option<&mut MessageReceiver<LobbyJoinOutcome>>,
            Option<&RoutedClientSession>,
            Option<&ClientLobbyMembership>,
        ),
        (With<Client>, Without<PendingClientConnect>),
    >,
    config: Res<ClientNetworkConfig>,
    routed: Res<RoutedClientLifecycle>,
    diagnostics: Option<Res<crate::diagnostics::ProcessDiagnosticsSettings>>,
    mut classification: ResMut<crate::diagnostics::ProcessExitClassification>,
    mut app_exit: MessageWriter<AppExit>,
) {
    for (entity, mut status, match_receiver, lobby_receiver, routed_session, membership) in
        query.iter_mut()
    {
        if let Some(mut receiver) = match_receiver {
            for outcome in receiver.receive() {
                match outcome {
                    MatchJoinOutcome::Accepted {
                        player_id,
                        network_entity_id,
                    } => {
                        info!(
                            player_id = player_id.0,
                            network_entity_id = network_entity_id.0,
                            "brawler client accepted"
                        );
                        if config.render_measurement.is_some() {
                            eprintln!(
                                "brawler-client timing match-accepted client_id={} player_id={} ts_ms={}",
                                config.client_id,
                                player_id.0,
                                crate::diagnostics::unix_micros_now() / 1_000
                            );
                        }
                        status.phase = ClientJoinPhase::Active {
                            player_id,
                            network_entity_id,
                        };
                    }
                    MatchJoinOutcome::Rejected { reason } => {
                        if config.transport == NetworkTransport::RoutedUdp
                            && matches!(
                                routed.phase,
                                RoutedClientPhase::AwaitingLobbyUnlink
                                    | RoutedClientPhase::AwaitingLobbyRetryUnlink
                                    | RoutedClientPhase::AwaitingMatchUnlink
                            )
                        {
                            warn!(
                                ?reason,
                                "ignoring join rejection during routed session teardown"
                            );
                            continue;
                        }
                        warn!(?reason, "brawler client rejected");
                        record_client_failure(
                            diagnostics.as_ref(),
                            &mut classification,
                            join_rejection_category(&reason),
                            format!("join rejected: {reason:?}"),
                        );
                        status.phase = ClientJoinPhase::Rejected(reason);
                        app_exit.write(AppExit::error());
                    }
                }
            }
        }
        if !routed_session.is_some_and(|session| session.kind == RoutedClientSessionKind::Lobby) {
            continue;
        }
        let Some(mut receiver) = lobby_receiver else {
            continue;
        };
        let mut accepted_this_batch: Option<ClientLobbyMembership> = None;
        let mut rejected_this_batch = false;
        for outcome in receiver.receive() {
            match outcome {
                LobbyJoinOutcome::Accepted {
                    player_id,
                    accepted_display_name,
                    server_name,
                    catalog_revision,
                    game_types,
                } => {
                    let accepted = ClientLobbyMembership {
                        player_id,
                        accepted_display_name,
                        server_name,
                        catalog_revision,
                        game_types,
                    };
                    if crate::lobby::validate_catalog(&accepted.game_types).is_err()
                        || crate::lobby::catalog_revision(&accepted.game_types).ok()
                            != Some(catalog_revision)
                        || rejected_this_batch
                        || membership.is_some_and(|previous| previous != &accepted)
                        || accepted_this_batch
                            .as_ref()
                            .is_some_and(|previous| previous != &accepted)
                    {
                        if config.presents_product_shell() {
                            commands
                                .entity(entity)
                                .insert(ClientLobbyFailure::InvalidWelcome);
                            status.phase = ClientJoinPhase::Disconnected;
                        } else {
                            record_client_failure(
                                diagnostics.as_ref(),
                                &mut classification,
                                crate::diagnostics::FailureCategory::ProtocolMismatch,
                                "lobby advertised an invalid or conflicting welcome".to_string(),
                            );
                            app_exit.write(AppExit::error());
                        }
                        continue;
                    }
                    if membership == Some(&accepted)
                        || accepted_this_batch.as_ref() == Some(&accepted)
                    {
                        continue;
                    }
                    commands.entity(entity).insert(accepted.clone());
                    accepted_this_batch = Some(accepted);
                    status.phase = ClientJoinPhase::LobbyActive { player_id };
                    info!(player_id = player_id.0, "brawler lobby client accepted");
                    if config.exit_after_lobby_welcome {
                        app_exit.write(AppExit::Success);
                    }
                }
                LobbyJoinOutcome::Rejected { reason } => {
                    if membership.is_some() || accepted_this_batch.is_some() {
                        if config.presents_product_shell() {
                            commands
                                .entity(entity)
                                .insert(ClientLobbyFailure::InvalidWelcome);
                            status.phase = ClientJoinPhase::Disconnected;
                        } else {
                            app_exit.write(AppExit::error());
                        }
                        continue;
                    }
                    rejected_this_batch = true;
                    warn!(?reason, "brawler lobby client rejected");
                    if config.presents_product_shell() {
                        commands
                            .entity(entity)
                            .insert(ClientLobbyFailure::Rejected(reason));
                        status.phase = ClientJoinPhase::Disconnected;
                    } else {
                        record_client_failure(
                            diagnostics.as_ref(),
                            &mut classification,
                            crate::diagnostics::FailureCategory::ProtocolMismatch,
                            format!("lobby join rejected: {reason:?}"),
                        );
                        app_exit.write(AppExit::error());
                    }
                }
            }
        }
    }
}

/// Finish the routed process smoke only after the match session has completed, its routes have
/// been revoked, and the client has authenticated one fresh lobby generation. Generation one is
/// the initial lobby and generation two is the match; a generation of three or greater therefore
/// cannot be satisfied by an initial lobby retry.
#[allow(clippy::needless_pass_by_value)]
pub(super) fn observe_fresh_lobby_return(
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
fn process_match_route_grant(
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
pub(super) fn observe_completed_match(
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
pub(super) fn drive_routed_transition(
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
fn observe_routed_transition(
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
fn advance_routed_transition(
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
pub(super) fn enforce_routed_timeout(
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

fn owns_automatic_routed_recovery(config: &ClientNetworkConfig) -> bool {
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

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn observe_client_lifecycle(
    config: Res<ClientNetworkConfig>,
    routed: Res<RoutedClientLifecycle>,
    mut query: Query<
        (
            &mut ClientJoinStatus,
            Option<&Disconnected>,
            Has<Connecting>,
        ),
        (With<Client>, Without<PendingClientConnect>),
    >,
    diagnostics: Option<Res<crate::diagnostics::ProcessDiagnosticsSettings>>,
    mut classification: ResMut<crate::diagnostics::ProcessExitClassification>,
    mut app_exit: MessageWriter<AppExit>,
) {
    // Routed sessions recover through their explicit Unlinked/deferred-teardown state machine;
    // treating the deliberate lobby->match disconnect as a terminal direct-UDP failure would
    // incorrectly exit the client.
    if config.transport == NetworkTransport::RoutedUdp
        && matches!(
            routed.phase,
            RoutedClientPhase::AwaitingLobbyUnlink
                | RoutedClientPhase::AwaitingLobbyRetryUnlink
                | RoutedClientPhase::AwaitingMatchUnlink
        )
    {
        return;
    }
    for (mut status, disconnected, connecting) in query.iter_mut() {
        if disconnected.is_some()
            && !connecting
            && !matches!(
                status.phase,
                ClientJoinPhase::Rejected(_) | ClientJoinPhase::Disconnected
            )
        {
            let reason = disconnected.map(|disconnected| disconnected.reason.to_string());
            warn!(?reason, "brawler client disconnected");
            record_client_failure(
                diagnostics.as_ref(),
                &mut classification,
                crate::diagnostics::FailureCategory::ShutdownIncomplete,
                format!("client disconnected: {reason:?}"),
            );
            status.phase = ClientJoinPhase::Disconnected;
            if !config.presents_product_shell() {
                app_exit.write(AppExit::error());
            }
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn log_replicated_roster(
    config: Res<ClientNetworkConfig>,
    routed: Res<RoutedClientLifecycle>,
    automation: Res<HeadlessAutomation>,
    combat_evidence: Option<Res<ClientCombatEvidenceStatus>>,
    mut roster_state: ResMut<RosterLogState>,
    status_query: Query<&ClientJoinStatus, With<Client>>,
    roster: Query<(&PlayerId, &NetworkEntityId), (With<Remote>, With<Fighter>)>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if config.transport == NetworkTransport::RoutedUdp && routed.phase != RoutedClientPhase::Match {
        return;
    }
    if config.exit_after_lobby_return {
        return;
    }
    let mut current: Vec<_> = roster
        .iter()
        .map(|(player, entity)| (*player, *entity))
        .collect();
    current.sort_by_key(|(player, entity)| (player.0, entity.0));
    if current != roster_state.0 {
        info!(
            roster = ?current.iter().map(|(player, entity)| (player.0, entity.0)).collect::<Vec<_>>(),
            "brawler replicated roster changed"
        );
        roster_state.0.clone_from(&current);
    }
    if let Some(target) = config.exit_after_roster
        && status_query
            .iter()
            .any(|status| matches!(status.phase, ClientJoinPhase::Active { .. }))
        && current.len() >= target
        && automation
            .simulation_ticks
            .is_none_or(|limit| automation.elapsed_ticks >= limit)
        && combat_evidence.is_none_or(|status| status.permits_exit())
    {
        app_exit.write(AppExit::Success);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn enforce_client_timeout(
    config: Res<ClientNetworkConfig>,
    time: Res<Time<Real>>,
    status_query: Query<&ClientJoinStatus, With<Client>>,
    roster: Query<(), (With<Remote>, With<Fighter>)>,
    diagnostics: Option<Res<crate::diagnostics::ProcessDiagnosticsSettings>>,
    mut classification: ResMut<crate::diagnostics::ProcessExitClassification>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if config.transport == NetworkTransport::RoutedUdp {
        return;
    }
    let now = time.elapsed();
    let roster_count = roster.iter().count();
    for status in status_query.iter() {
        let connection_timed_out = matches!(
            status.phase,
            ClientJoinPhase::Connecting | ClientJoinPhase::AwaitingOutcome
        ) && now
            >= status.started_at.saturating_add(config.connect_timeout);
        let roster_timed_out = config.exit_after_roster.is_some_and(|target| {
            matches!(status.phase, ClientJoinPhase::Active { .. })
                && roster_count < target
                && now >= status.started_at.saturating_add(config.connect_timeout)
        });
        if connection_timed_out || roster_timed_out {
            error!("brawler client connection timed out");
            record_client_failure(
                diagnostics.as_ref(),
                &mut classification,
                crate::diagnostics::FailureCategory::Timeout,
                "client connection timed out".to_string(),
            );
            app_exit.write(AppExit::error());
        }
    }
}

pub(super) fn forward_app_exit_to_client_disconnect(
    mut app_exits: ResMut<Messages<AppExit>>,
    mut shutdown: ResMut<ClientShutdown>,
    mut commands: Commands,
    query: Query<(Entity, Option<&Disconnected>), With<Client>>,
) {
    if shutdown.requested_exit.is_some() {
        return;
    }
    let exits: Vec<_> = app_exits.drain().collect();
    let Some(exit) = exits
        .iter()
        .find(|exit| exit.is_error())
        .or_else(|| exits.first())
        .cloned()
    else {
        return;
    };
    shutdown.requested_exit = Some(exit);
    for (entity, disconnected) in query.iter() {
        if disconnected.is_none() {
            commands.trigger(Disconnect { entity });
        }
    }
}

pub(super) fn finish_client_shutdown(
    mut app_exits: ResMut<Messages<AppExit>>,
    mut shutdown: ResMut<ClientShutdown>,
    query: Query<Option<&Disconnected>, With<Client>>,
) {
    let mut any_client = false;
    let all_disconnected = query.iter().all(|disconnected| {
        any_client = true;
        disconnected.is_some()
    });
    if any_client
        && all_disconnected
        && let Some(exit) = shutdown.requested_exit.take()
    {
        app_exits.write(exit);
    }
}

#[cfg(test)]
mod connection_start_tests {
    use super::*;

    #[derive(Component)]
    struct CompleteConnectionFixture;

    #[derive(Resource, Default)]
    struct ObservedConnect(bool);

    #[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
    fn observe_connect_after_materialization(
        trigger: On<Connect>,
        fixtures: Query<
            (),
            (
                With<CompleteConnectionFixture>,
                Without<Unlinked>,
                Without<Disconnected>,
            ),
        >,
        mut commands: Commands,
        mut observed: ResMut<ObservedConnect>,
    ) {
        observed.0 = fixtures.get(trigger.entity).is_ok();
        commands.entity(trigger.entity).insert(Connecting);
    }

    #[test]
    fn deferred_client_connect_runs_after_the_complete_entity_is_materialized() {
        let mut app = App::new();
        app.init_resource::<ObservedConnect>()
            .add_observer(observe_connect_after_materialization)
            .add_systems(
                Update,
                (connect_spawned_clients, finish_spawned_client_connect).chain(),
            );
        let entity = app
            .world_mut()
            .spawn((
                PendingClientConnect,
                CompleteConnectionFixture,
                Unlinked::default(),
                Disconnected::default(),
            ))
            .id();

        app.update();

        assert!(app.world().resource::<ObservedConnect>().0);
        assert!(app.world().get::<PendingClientConnect>(entity).is_none());
    }

    #[test]
    fn product_flow_is_the_only_owner_of_its_attempt_timeout() {
        let mut config = ClientNetworkConfig::new(1);
        config.transport = NetworkTransport::RoutedUdp;
        assert!(!owns_automatic_routed_recovery(&config));

        config.auto_connect = true;
        assert!(owns_automatic_routed_recovery(&config));
    }
}
