//! Client connection, compatibility handshake, selection, roster, and shutdown lifecycle.
#![allow(clippy::wildcard_imports)]

use super::*;

/// Installs the client Lightyear group, protocol, connection, and status systems.
pub struct ClientNetworkPlugin;

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FallbackErrorHandler(error))
            .add_plugins(ClientCombatPlugin)
            .init_resource::<RosterLogState>()
            .init_resource::<ClientShutdown>()
            .init_resource::<PendingLocalActions>()
            .init_resource::<LiveInputTrace>()
            .init_resource::<HeadlessAutomation>()
            .init_resource::<InputDeviceActivity>()
            .init_resource::<ClientInputContext>()
            .init_resource::<WeaponSelectionState>()
            .init_resource::<GreyboxArenaDefinition>()
            .init_resource::<InputTuning>()
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
                    send_client_hello,
                    process_join_outcome,
                    process_weapon_selection_outcomes,
                    send_weapon_selection_request,
                    update_weapon_selection_overlay,
                    disconnect_rejected_client,
                    observe_client_lifecycle,
                    log_replicated_roster,
                    enforce_client_timeout,
                    trace_client_interpolation_sync,
                    trace_client_interpolation_history,
                )
                    .chain(),
            )
            .add_systems(
                Last,
                (
                    forward_app_exit_to_client_disconnect,
                    finish_client_shutdown,
                )
                    .chain(),
            );
        app.add_observer(add_controlled_input_marker);
    }
}

pub(super) fn spawn_client_connection(
    mut commands: Commands,
    config: Res<ClientNetworkConfig>,
    time: Res<Time<Real>>,
) -> Result {
    if config.transport != NetworkTransport::Udp {
        return Ok(());
    }
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
    let entity = commands
        .spawn((
            ClientJoinStatus {
                phase: ClientJoinPhase::Connecting,
                started_at: time.elapsed(),
                disconnect_requested: false,
            },
            PingManager::default(),
            ReplicationReceiver,
            Link::default().with_conditioner(config.impairment_profile.receive_conditioner()),
            NetcodeClient::new(auth, netcode_config)?,
            LocalAddr(config.local_addr),
            PeerAddr(config.server_addr),
            UdpIo::default(),
            Name::new(format!("Brawler client {}", config.client_id)),
        ))
        .id();
    commands.trigger(Connect { entity });
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

pub(super) fn send_client_hello(
    config: Res<ClientNetworkConfig>,
    fingerprint: Res<ProtocolFingerprint>,
    content_fingerprint: Res<crate::combat::GameplayContentFingerprint>,
    time: Res<Time<Real>>,
    mut query: Query<
        (&mut ClientJoinStatus, &mut MessageSender<ClientHello>),
        (With<Client>, With<Connected>),
    >,
) {
    for (mut status, mut sender) in query.iter_mut() {
        if matches!(status.phase, ClientJoinPhase::Connecting) {
            sender.send::<SessionChannel>(ClientHello {
                protocol_version: config.expected_protocol_version,
                build_version: config.expected_build_version.clone(),
                registry_fingerprint: fingerprint.0,
                content_fingerprint: *content_fingerprint,
            });
            status.phase = ClientJoinPhase::AwaitingOutcome;
            status.started_at = time.elapsed();
            info!("brawler client connected; awaiting compatibility outcome");
        }
    }
}

pub(super) fn process_weapon_selection_outcomes(
    mut state: ResMut<WeaponSelectionState>,
    mut receivers: Query<Option<&mut MessageReceiver<WeaponSelectionOutcome>>, With<Client>>,
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

#[allow(clippy::too_many_arguments)]
pub(super) fn send_weapon_selection_request(
    config: Res<ClientNetworkConfig>,
    mut state: ResMut<WeaponSelectionState>,
    catalog: Res<WeaponCatalogResource>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    gamepads: Query<&Gamepad>,
    fighters: Query<(), (With<Fighter>, With<Controlled>, With<SelectingWeapon>)>,
    statuses: Query<&ClientJoinStatus, With<Client>>,
    mut senders: Query<&mut MessageSender<WeaponSelectionRequest>, With<Client>>,
) {
    if !statuses
        .iter()
        .any(|status| matches!(status.phase, ClientJoinPhase::Active { .. }))
        || fighters.iter().next().is_none()
    {
        return;
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
    let stick_x = gamepads
        .iter()
        .find_map(|gamepad| gamepad.get(GamepadAxis::LeftStickX))
        .unwrap_or(0.0);
    let analog_left = stick_x < -0.6 && state.analog_ready;
    let analog_right = stick_x > 0.6 && state.analog_ready;
    if stick_x.abs() < 0.3 {
        state.analog_ready = true;
    }
    if left || analog_left {
        state.analog_ready = false;
        state.current_index = (state.current_index + 3) % 4;
    } else if right || analog_right {
        state.analog_ready = false;
        state.current_index = (state.current_index + 1) % 4;
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
        || config.windowed_controller_demo.is_some();
    let should_send = automatic && state.last_sent.is_none() || confirm;
    if !should_send {
        return;
    }
    if state.last_sent.is_some() && !confirm {
        return;
    }
    if let Some(preset) = config.weapon_preset {
        state.current_index = usize::from(preset.saturating_sub(1).min(3));
    }
    let Some(preset) = catalog.0.presets.get(state.current_index) else {
        return;
    };
    state.next_request_id = state.next_request_id.saturating_add(1).max(1);
    let request = WeaponSelectionRequest {
        request_id: state.next_request_id,
        preset_id: preset.id,
    };
    for mut sender in &mut senders {
        sender.send::<SessionChannel>(request);
    }
    state.last_sent = Some(request.request_id);
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

pub(super) fn update_weapon_selection_overlay(
    state: Res<WeaponSelectionState>,
    catalog: Res<WeaponCatalogResource>,
    fighters: Query<(), (With<Fighter>, With<Controlled>, With<SelectingWeapon>)>,
    mut overlay: Query<(&mut Text, &mut Visibility), With<WeaponSelectionText>>,
) {
    let selecting = fighters.iter().next().is_some();
    let Some(preset) = catalog.0.presets.get(state.current_index) else {
        return;
    };
    let current = preset.display_name.as_str();
    let recipe = &preset.configuration.recipe;
    let pattern = match recipe.firing {
        crate::combat::FiringPattern::Single => "single",
        crate::combat::FiringPattern::Spread { delivery_count, .. } => {
            if delivery_count == 7 {
                "7-pellet spread"
            } else {
                "spread"
            }
        }
    };
    let range = match recipe.delivery {
        crate::combat::DeliveryMethod::Straight { range, .. } => format!("range {range:.0}"),
        crate::combat::DeliveryMethod::Lobbed {
            distance,
            max_flight_ticks,
            ..
        } => format!("aimed landing up to {distance:.0} / up to {max_flight_ticks}t flight"),
        crate::combat::DeliveryMethod::MeleeArc {
            reach,
            angle_degrees,
        } => format!("reach {reach:.0} / {angle_degrees:.0}°"),
    };
    let recovery = format!("{}t recovery", recipe.economy.refill_ticks());
    let profile = match preset.id.0 {
        1 => "steady mid-range pressure; cover and rushes counter it",
        2 => "close burst; cone, falloff, and reload punish misses",
        3 => "cover/group punish; telegraphed landing and dead zone",
        4 => "close displacement burst; kite outside danger range",
        _ => "server-authored preset",
    };
    let status = state
        .last_outcome
        .map_or("Awaiting server".to_string(), |outcome| {
            match outcome.decision {
                WeaponSelectionDecision::Accepted => {
                    "Accepted; waiting for replicated state".to_string()
                }
                WeaponSelectionDecision::UnknownPreset => {
                    "Server rejected: unknown preset".to_string()
                }
                WeaponSelectionDecision::NotSelecting => {
                    "Server rejected: selection is locked".to_string()
                }
                WeaponSelectionDecision::StaleRequest => {
                    "Server rejected: stale request".to_string()
                }
                WeaponSelectionDecision::ResolutionFailed => {
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
            "Select weapon: A/D or arrows • D-pad/stick • Space / South\n{current} • {pattern} • {range} • {recovery}\n{profile}\n{status}"
        );
    }
}

pub(super) fn process_join_outcome(
    mut query: Query<
        (
            &mut ClientJoinStatus,
            Option<&mut MessageReceiver<JoinOutcome>>,
        ),
        With<Client>,
    >,
    mut app_exit: MessageWriter<AppExit>,
) {
    for (mut status, receiver) in query.iter_mut() {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for outcome in receiver.receive() {
            match outcome {
                JoinOutcome::Accepted {
                    player_id,
                    network_entity_id,
                } => {
                    info!(
                        player_id = player_id.0,
                        network_entity_id = network_entity_id.0,
                        "brawler client accepted"
                    );
                    status.phase = ClientJoinPhase::Active {
                        player_id,
                        network_entity_id,
                    };
                }
                JoinOutcome::Rejected { reason } => {
                    warn!(?reason, "brawler client rejected");
                    status.phase = ClientJoinPhase::Rejected(reason);
                    app_exit.write(AppExit::error());
                }
            }
        }
    }
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

pub(super) fn observe_client_lifecycle(
    mut query: Query<
        (
            &mut ClientJoinStatus,
            Option<&Disconnected>,
            Has<Connecting>,
        ),
        With<Client>,
    >,
    mut app_exit: MessageWriter<AppExit>,
) {
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
            status.phase = ClientJoinPhase::Disconnected;
            app_exit.write(AppExit::error());
        }
    }
}

pub(super) fn log_replicated_roster(
    config: Res<ClientNetworkConfig>,
    automation: Res<HeadlessAutomation>,
    combat_evidence: Option<Res<ClientCombatEvidenceStatus>>,
    mut roster_state: ResMut<RosterLogState>,
    status_query: Query<&ClientJoinStatus, With<Client>>,
    roster: Query<(&PlayerId, &NetworkEntityId), (With<Remote>, With<Fighter>)>,
    mut app_exit: MessageWriter<AppExit>,
) {
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

pub(super) fn enforce_client_timeout(
    config: Res<ClientNetworkConfig>,
    time: Res<Time<Real>>,
    status_query: Query<&ClientJoinStatus, With<Client>>,
    roster: Query<(), (With<Remote>, With<Fighter>)>,
    mut app_exit: MessageWriter<AppExit>,
) {
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
