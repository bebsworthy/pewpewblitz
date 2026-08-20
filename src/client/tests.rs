//! Focused client composition and behavior tests.

use super::*;

#[test]
fn build_editor_stick_has_independent_vertical_and_horizontal_edges() {
    let mut x_ready = true;
    let mut y_ready = true;
    assert_eq!(
        editor_axis_edges(Vec2::new(0.0, 0.8), &mut x_ready, &mut y_ready),
        (false, false, true, false),
    );
    assert_eq!(
        editor_axis_edges(Vec2::new(0.0, 0.8), &mut x_ready, &mut y_ready),
        (false, false, false, false),
        "a held stick must not repeat without crossing the neutral hysteresis",
    );
    let _ = editor_axis_edges(Vec2::ZERO, &mut x_ready, &mut y_ready);
    assert_eq!(
        editor_axis_edges(Vec2::new(0.8, 0.0), &mut x_ready, &mut y_ready),
        (false, true, false, false),
    );
    let _ = editor_axis_edges(Vec2::ZERO, &mut x_ready, &mut y_ready);
    assert_eq!(
        editor_axis_edges(Vec2::new(0.0, -0.8), &mut x_ready, &mut y_ready),
        (false, false, false, true),
    );
}

#[test]
fn automatic_match_ready_waits_for_the_requested_roster() {
    let mut config = ClientNetworkConfig::new(1);
    config.headless = true;
    config.headless_simulation_ticks = Some(2_000);
    config.exit_after_roster = Some(4);
    assert!(!automatic_match_command_enabled(&config, 2));
    assert!(automatic_match_command_enabled(&config, 4));
}

#[test]
fn headless_match_command_rearms_during_countdown_for_waiting_retry() {
    let match_id = crate::matchplay::MatchId(7);
    assert!(should_rearm_headless_match_command(
        true,
        Some((match_id, MatchPhase::Waiting)),
        match_id,
        MatchPhase::Countdown { starts_at_tick: 90 },
    ));
    assert!(!should_rearm_headless_match_command(
        false,
        Some((match_id, MatchPhase::Waiting)),
        match_id,
        MatchPhase::Countdown { starts_at_tick: 90 },
    ));
}

#[test]
fn client_config_defaults_to_loopback_and_validates_roster_target() {
    let mut config = ClientNetworkConfig::new(1);
    assert!(config.validate().is_ok());
    config.exit_after_roster = Some(0);
    assert!(config.validate().is_err());
}

#[test]
fn headless_custom_build_and_cover_lane_movement_are_bounded() {
    let mut config = ClientNetworkConfig::new(1);
    config.headless = true;
    config.build_preset = Some(5);
    assert!(config.validate().is_ok());
    config.build_preset = Some(6);
    assert!(config.validate().is_err());
    config.build_preset = Some(5);
    config.window_size = Some((960, 540));
    assert!(config.validate().is_ok());
    config.window_size = Some((639, 540));
    assert!(config.validate().is_err());

    assert_eq!(
        headless_navigation_delta(Vec2::new(-768.0, 0.0), Vec2::new(768.0, 0.0)),
        Some(Vec2::Y)
    );
    assert_eq!(
        headless_navigation_delta(Vec2::new(-500.0, 180.0), Vec2::new(500.0, 180.0)),
        Some(Vec2::X)
    );
    assert_eq!(
        headless_combat_move_axis(Vec2::X, Some(Vec2::new(350.0, 0.0)), Some(Vec2::X), true),
        Vec2::ZERO
    );
    assert_eq!(
        headless_combat_move_axis(Vec2::X, Some(Vec2::new(100.0, 0.0)), Some(Vec2::X), false),
        Vec2::ZERO
    );
}

#[test]
fn render_profiles_keep_fixed_simulation_and_select_expected_window_path() {
    assert_eq!(
        render_profile_settings(RenderProfile::Native).0,
        PresentMode::Fifo
    );
    assert_eq!(
        render_profile_settings(RenderProfile::HighRefresh).0,
        PresentMode::AutoNoVsync
    );
    let (_, settings) = render_profile_settings(RenderProfile::ThirtyFps);
    assert!(matches!(
        settings.focused_mode,
        UpdateMode::Reactive { wait, .. } if wait == RENDER_30_INTERVAL
    ));
    let (_, settings) = render_profile_settings(RenderProfile::SixtyFps);
    assert!(matches!(
        settings.focused_mode,
        UpdateMode::Reactive { wait, .. } if wait == RENDER_60_INTERVAL
    ));
}

#[test]
fn app_exit_is_forwarded_after_update_producers_run() {
    fn request_exit(mut app_exit: MessageWriter<AppExit>) {
        app_exit.write(AppExit::Success);
    }

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_message::<AppExit>()
        .init_resource::<ClientShutdown>()
        .add_systems(Update, request_exit)
        .add_systems(
            Last,
            (
                forward_app_exit_to_client_disconnect,
                finish_client_shutdown,
            )
                .chain(),
        )
        .add_observer(|trigger: On<Disconnect>, mut commands: Commands| {
            commands
                .entity(trigger.entity)
                .insert(Disconnected::default());
        });
    let client = app.world_mut().spawn(Client).id();

    app.update();

    assert!(
        app.world()
            .resource::<ClientShutdown>()
            .requested_exit
            .is_none()
    );
    assert!(app.world().get::<Disconnected>(client).is_some());
    assert!(app.should_exit().is_some_and(|exit| exit.is_success()));
}

#[test]
fn routed_unexpected_disconnect_is_terminal_but_expected_handoff_is_not() {
    fn app_for(phase: RoutedClientPhase) -> App {
        let mut config = ClientNetworkConfig::new(1);
        config.transport = NetworkTransport::RoutedUdp;
        config.auto_connect = true;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<AppExit>()
            .insert_resource(config)
            .insert_resource(RoutedClientLifecycle {
                phase,
                generation: 1,
                ..default()
            })
            .init_resource::<crate::diagnostics::ProcessExitClassification>()
            .add_systems(Update, observe_client_lifecycle);
        app.world_mut().spawn((
            Client,
            RoutedClientSession {
                generation: 1,
                kind: RoutedClientSessionKind::Lobby,
            },
            ClientJoinStatus {
                phase: ClientJoinPhase::LobbyActive {
                    player_id: PlayerId(1),
                },
                started_at: Duration::ZERO,
                disconnect_requested: false,
            },
            Disconnected::default(),
        ));
        app
    }

    let mut unexpected = app_for(RoutedClientPhase::Lobby);
    unexpected.update();
    assert!(unexpected.should_exit().is_some_and(|exit| exit.is_error()));

    let mut expected = app_for(RoutedClientPhase::AwaitingLobbyUnlink);
    expected.update();
    assert!(expected.should_exit().is_none());
}

#[test]
fn routed_handoff_disconnects_netcode_before_unlinking_transport() {
    use lightyear::link::{LinkPlugin, Linked};

    let mut config = ClientNetworkConfig::new(1);
    config.transport = NetworkTransport::RoutedUdp;
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_plugins(LinkPlugin)
        .insert_resource(config)
        .insert_resource(RoutedClientLifecycle {
            phase: RoutedClientPhase::AwaitingLobbyUnlink,
            generation: 1,
            ..default()
        })
        .add_systems(Update, drive_routed_transition)
        .add_observer(|trigger: On<Disconnect>, mut commands: Commands| {
            commands
                .entity(trigger.entity)
                .insert(Disconnected::default());
        });
    let client = app
        .world_mut()
        .spawn((
            Client,
            Link::default(),
            Linked,
            RoutedClientSession {
                generation: 1,
                kind: RoutedClientSessionKind::Lobby,
            },
            ClientJoinStatus {
                phase: ClientJoinPhase::LobbyActive {
                    player_id: PlayerId(1),
                },
                started_at: Duration::ZERO,
                disconnect_requested: false,
            },
        ))
        .id();

    app.update();
    assert!(app.world().get::<Disconnected>(client).is_some());
    assert!(app.world().get::<Unlinked>(client).is_none());

    app.update();
    assert!(app.world().get::<Unlinked>(client).is_some());
    assert!(app.world().get::<Linked>(client).is_none());
}

#[test]
fn routed_connect_timeout_never_expires_an_active_lobby_or_match_session() {
    let mut config = ClientNetworkConfig::new(1);
    config.transport = NetworkTransport::RoutedUdp;
    config.auto_connect = true;
    config.connect_timeout = Duration::from_secs(1);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_secs(2),
        ))
        .insert_resource(config)
        .insert_resource(RoutedClientLifecycle {
            phase: RoutedClientPhase::Match,
            generation: 1,
            ..default()
        })
        .add_systems(Update, enforce_routed_timeout)
        .add_observer(|trigger: On<Disconnect>, mut commands: Commands| {
            commands
                .entity(trigger.entity)
                .insert(Disconnected::default());
        });
    let client = app
        .world_mut()
        .spawn((
            Client,
            RoutedClientSession {
                generation: 1,
                kind: RoutedClientSessionKind::Match,
            },
            ClientJoinStatus {
                phase: ClientJoinPhase::Active {
                    player_id: PlayerId(1),
                    network_entity_id: NetworkEntityId(1),
                },
                started_at: Duration::ZERO,
                disconnect_requested: false,
            },
        ))
        .id();

    app.update();
    assert!(app.world().get::<Disconnected>(client).is_none());
    assert_eq!(
        app.world().resource::<RoutedClientLifecycle>().phase,
        RoutedClientPhase::Match
    );

    app.world_mut()
        .get_mut::<ClientJoinStatus>(client)
        .unwrap()
        .phase = ClientJoinPhase::AwaitingOutcome;
    app.update();
    assert!(app.world().get::<Disconnected>(client).is_some());
    assert_eq!(
        app.world().resource::<RoutedClientLifecycle>().phase,
        RoutedClientPhase::AwaitingMatchUnlink
    );
}

#[test]
fn routed_teardown_deadline_forces_stale_session_recovery() {
    let mut config = ClientNetworkConfig::new(1);
    config.transport = NetworkTransport::RoutedUdp;
    config.auto_connect = true;
    config.connect_timeout = Duration::from_secs(1);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_secs(2),
        ))
        .insert_resource(config)
        .insert_resource(RoutedClientLifecycle {
            phase: RoutedClientPhase::AwaitingMatchUnlink,
            generation: 2,
            ..default()
        })
        .add_systems(Update, enforce_routed_timeout);
    let client = app
        .world_mut()
        .spawn((
            Client,
            RoutedClientSession {
                generation: 2,
                kind: RoutedClientSessionKind::Match,
            },
            ClientJoinStatus {
                phase: ClientJoinPhase::AwaitingOutcome,
                started_at: Duration::ZERO,
                disconnect_requested: true,
            },
            RoutedTransitionDeadline(Duration::ZERO),
        ))
        .id();

    app.update();

    assert!(app.world().get_entity(client).is_err());
}

#[test]
fn input_sampling_schedule_runs_before_fixed_update() {
    #[derive(Resource, Default)]
    struct Trace(Vec<&'static str>);

    fn sample(mut trace: ResMut<Trace>) {
        trace.0.push("sample");
    }
    fn fixed(mut trace: ResMut<Trace>) {
        trace.0.push("fixed");
    }
    fn render(mut trace: ResMut<Trace>) {
        trace.0.push("render");
    }

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            Duration::from_secs_f32(1.0 / 60.0),
        ))
        .init_resource::<Trace>()
        .add_systems(
            RunFixedMainLoop,
            sample.in_set(RunFixedMainLoopSystems::BeforeFixedMainLoop),
        )
        .add_systems(FixedUpdate, fixed)
        .add_systems(Update, render);
    app.update();
    app.update();

    let trace = &app.world().resource::<Trace>().0;
    let sample_index = trace.iter().rposition(|value| *value == "sample");
    let fixed_index = trace.iter().rposition(|value| *value == "fixed");
    let render_index = trace.iter().rposition(|value| *value == "render");
    assert!(sample_index.is_some_and(|sample| {
        fixed_index.is_some_and(|fixed| {
            render_index.is_some_and(|render| sample < fixed && fixed < render)
        })
    }));
}

#[test]
fn controlled_entity_gets_input_marker_before_fighter_replication_arrives() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_observer(add_controlled_input_marker);

    let entity = app.world_mut().spawn(Controlled).id();
    app.update();

    assert!(
        app.world()
            .get::<InputMarker<FighterInput>>(entity)
            .is_some()
    );
    assert!(
        app.world()
            .get::<ActionState<FighterInput>>(entity)
            .is_some()
    );
}

#[test]
fn keyboard_movement_is_sampled_from_the_window_input_resource() {
    let mut app = App::new();
    let mut keyboard = ButtonInput::<KeyCode>::default();
    keyboard.press(KeyCode::KeyD);
    app.add_plugins(MinimalPlugins)
        .insert_resource(keyboard)
        .init_resource::<PendingLocalActions>()
        .init_resource::<ClientInputContext>()
        .insert_resource(ClientPlayableGate(true))
        .init_resource::<InputDeviceActivity>()
        .init_resource::<ClientInputSettings>()
        .add_systems(Update, sample_local_input);
    app.world_mut().spawn((Window::default(), PrimaryWindow));

    app.update();

    assert_eq!(
        app.world().resource::<PendingLocalActions>().move_axis,
        Vec2::X
    );
}

#[test]
fn logical_keyboard_movement_supports_non_us_layouts() {
    let mut app = App::new();
    let mut logical_keyboard = ButtonInput::<Key>::default();
    logical_keyboard.press(Key::Character("d".into()));
    app.add_plugins(MinimalPlugins)
        .insert_resource(ButtonInput::<KeyCode>::default())
        .insert_resource(logical_keyboard)
        .init_resource::<PendingLocalActions>()
        .init_resource::<ClientInputContext>()
        .init_resource::<InputDeviceActivity>()
        .init_resource::<ClientInputSettings>()
        .add_systems(Update, sample_local_input);
    app.world_mut().spawn((Window::default(), PrimaryWindow));

    app.update();

    assert_eq!(
        app.world().resource::<PendingLocalActions>().move_axis,
        Vec2::X
    );
}

#[test]
fn activity_thresholds_are_strict_and_require_nonzero_progress() {
    // The default move deadzone is 0.0, so a centered stick must not count as activity.
    assert!(!exceeds_activity_threshold(0.0, 0.0));
    assert!(exceeds_activity_threshold(0.01, 0.0));
    assert!(!exceeds_activity_threshold(0.25, 0.25));
    assert!(exceeds_activity_threshold(0.26, 0.25));
}

#[test]
fn idle_gamepad_does_not_become_the_active_input_device() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(ButtonInput::<KeyCode>::default())
        .init_resource::<PendingLocalActions>()
        .insert_resource(ClientPlayableGate(true))
        .init_resource::<ClientInputContext>()
        .init_resource::<InputDeviceActivity>()
        .init_resource::<ClientInputSettings>()
        .add_systems(Update, sample_local_input);
    let gamepad_entity = app.world_mut().spawn(Gamepad::default()).id();

    // A connected-but-resting gamepad must never register meaningful activity, even on
    // its first sample: the default move deadzone is 0.0 and a centered stick reads 0.0.
    for _ in 0..5 {
        app.update();
    }
    assert_eq!(
        app.world().resource::<PendingLocalActions>().active_device,
        ActiveInputDevice::KeyboardMouse
    );
    assert!(
        app.world()
            .resource::<InputDeviceActivity>()
            .recent_gamepads
            .is_empty()
    );
    assert_eq!(
        app.world().resource::<PendingLocalActions>().move_axis,
        Vec2::ZERO
    );

    // Real stick input beyond the (zero) deadzone still adopts the gamepad.
    let mut gamepads = app.world_mut().query::<&mut Gamepad>();
    gamepads
        .single_mut(app.world_mut())
        .expect("idle test gamepad")
        .analog_mut()
        .set(GamepadAxis::LeftStickX, 0.6);
    app.update();
    assert_eq!(
        app.world().resource::<PendingLocalActions>().active_device,
        ActiveInputDevice::Gamepad(gamepad_entity)
    );
}

/// Resolve the Arc Launcher loadout through the real build pipeline, so input shaping
/// tests observe the same `ResolvedMatchLoadout` a joined client receives by replication.
fn resolved_arc_launcher_loadout() -> crate::builds::ResolvedMatchLoadout {
    let build_catalog = crate::builds::BuildCatalog::embedded().expect("embedded build catalog");
    let weapons = crate::combat::WeaponCatalog::embedded().expect("embedded weapon catalog");
    let fighter = crate::combat::FighterDefinitions::default().entries[0];
    crate::builds::resolve_build_recipe(
        &build_catalog,
        &weapons,
        &fighter,
        crate::builds::BrawlerBuildRecipe {
            weapon: crate::builds::WeaponChoice::Preset(crate::combat::WeaponPresetId(3)),
            ultimate: crate::builds::UltimateDefinitionId(1),
            passives: [
                crate::builds::PassiveDefinitionId(1),
                crate::builds::PassiveDefinitionId(6),
            ],
        },
        None,
    )
    .expect("arc launcher loadout resolves")
}

#[test]
fn gamepad_sample_maps_sticks_triggers_and_start_to_native_actions() {
    let mut gamepad = Gamepad::default();
    gamepad.analog_mut().set(GamepadAxis::LeftStickX, 0.75);
    gamepad.analog_mut().set(GamepadAxis::RightStickY, -0.8);
    gamepad.analog_mut().set(GamepadButton::RightTrigger2, 1.0);
    gamepad.digital_mut().press(GamepadButton::Start);

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(ButtonInput::<KeyCode>::default())
        .init_resource::<PendingLocalActions>()
        .insert_resource(ClientPlayableGate(true))
        .init_resource::<ClientInputContext>()
        .init_resource::<InputDeviceActivity>()
        .init_resource::<ClientInputSettings>()
        .add_systems(Update, sample_local_input);
    let gamepad_entity = app.world_mut().spawn(gamepad).id();
    // The controlled fighter carries the replicated loadout; a standalone
    // `ResolvedWeapon` never arrives in network play, so the lob range must come from
    // the loadout's primary weapon exactly as a joined client observes it.
    let loadout = resolved_arc_launcher_loadout();
    app.world_mut()
        .spawn((Fighter, Controlled, Position::default(), loadout));

    app.update();

    let pending = app.world().resource::<PendingLocalActions>();
    assert_eq!(
        pending.active_device,
        ActiveInputDevice::Gamepad(gamepad_entity)
    );
    assert_eq!(pending.move_axis, Vec2::new(0.75, 0.0));
    // Default calibration commits the normalized aim direction; facing is identical to the
    // raw-axis contract because the authoritative decoder normalizes positive multiples.
    assert_eq!(pending.aim_axis, Some(Vec2::new(0.0, -1.0)));
    assert!(
        pending
            .aim_distance
            .is_some_and(|distance| (distance - 381.333_34).abs() < 0.001)
    );
    assert_ne!(pending.held_buttons & FighterInput::PRIMARY_FIRE, 0);
    assert_eq!(
        *app.world().resource::<ClientInputContext>(),
        ClientInputContext::Paused
    );
    assert_ne!(pending.action_indicator & ACTION_PAUSE, 0);
}

#[test]
fn controller_sample_reaches_native_fighter_action_buffer() {
    let mut gamepad = Gamepad::default();
    gamepad.analog_mut().set(GamepadAxis::LeftStickX, 0.75);
    gamepad.analog_mut().set(GamepadAxis::RightStickY, -0.8);
    gamepad.analog_mut().set(GamepadButton::RightTrigger2, 1.0);

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(ButtonInput::<KeyCode>::default())
        .init_resource::<PendingLocalActions>()
        .insert_resource(ClientPlayableGate(true))
        .init_resource::<ClientInputContext>()
        .init_resource::<InputDeviceActivity>()
        .init_resource::<ClientInputSettings>()
        .add_systems(Update, (sample_local_input, write_client_input).chain());
    app.world_mut().spawn((
        ActionState::<FighterInput>::default(),
        InputMarker::<FighterInput>::default(),
    ));
    let gamepad_entity = app.world_mut().spawn(gamepad).id();

    app.update();

    let mut actions = app.world_mut().query::<&ActionState<FighterInput>>();
    let action = actions
        .single(app.world())
        .expect("one controller input target");
    assert_eq!(
        action.0,
        FighterInput::from_axes(
            Vec2::new(0.75, 0.0),
            Some(Vec2::new(0.0, -1.0)),
            FighterInput::PRIMARY_FIRE,
        )
    );
    // The calibrated aim axis and the previous raw axis are positive scalar multiples, so
    // the authoritative facing decode produces the identical rotation.
    let tuning = crate::movement::InputTuning::default();
    let calibrated_facing = crate::movement::committed_aim(Vec2::new(0.0, -1.0), tuning);
    let raw_facing = crate::movement::committed_aim(Vec2::new(0.0, -0.8), tuning);
    assert_eq!(calibrated_facing, raw_facing);
    assert_eq!(
        app.world().resource::<PendingLocalActions>().active_device,
        ActiveInputDevice::Gamepad(gamepad_entity)
    );
}

#[test]
fn interpolated_fighter_pose_is_written_to_render_transform() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .add_systems(Update, write_interpolated_fighter_pose_to_transform);
    let entity = app
        .world_mut()
        .spawn((
            Fighter,
            Remote,
            Position::from_xy(120.0, -45.0),
            Rotation::radians(core::f32::consts::FRAC_PI_2),
            Transform::from_xyz(0.0, 0.0, -10.0),
        ))
        .id();

    app.update();

    let transform = app.world().get::<Transform>(entity).expect("transform");
    assert_eq!(transform.translation, Vec3::new(120.0, -45.0, -10.0));
    assert!(
        (transform.rotation.to_euler(EulerRot::ZYX).0 - core::f32::consts::FRAC_PI_2).abs() < 0.001
    );
}

#[test]
fn disconnected_gamepad_falls_back_to_most_recent_connected_device() {
    let first = Entity::from_raw_u32(1).expect("valid test entity index");
    let second = Entity::from_raw_u32(2).expect("valid test entity index");
    assert_eq!(
        select_active_gamepad(
            ActiveInputDevice::Gamepad(first),
            &[first, second],
            &[],
            &[second],
        ),
        Some(second)
    );
    assert_eq!(
        select_active_gamepad(
            ActiveInputDevice::Gamepad(first),
            &[first, second],
            &[second],
            &[first, second],
        ),
        Some(second)
    );
    assert_eq!(
        select_active_gamepad(ActiveInputDevice::Gamepad(first), &[first], &[], &[],),
        None
    );
}

#[test]
fn keyboard_mouse_selection_persists_until_gamepad_activity() {
    let gamepad = Entity::from_raw_u32(3).expect("valid test entity index");
    assert_eq!(
        select_active_input_device(ActiveInputDevice::KeyboardMouse, false, Some(gamepad), None,),
        ActiveInputDevice::KeyboardMouse
    );
    assert_eq!(
        select_active_input_device(
            ActiveInputDevice::KeyboardMouse,
            false,
            Some(gamepad),
            Some(gamepad),
        ),
        ActiveInputDevice::Gamepad(gamepad)
    );
    assert_eq!(
        select_active_input_device(
            ActiveInputDevice::Gamepad(gamepad),
            true,
            Some(gamepad),
            None,
        ),
        ActiveInputDevice::KeyboardMouse
    );
}

#[test]
fn controller_cancel_does_not_toggle_pause() {
    let mut context = ClientInputContext::Gameplay;
    let mut pending = PendingLocalActions {
        cancel_pressed: true,
        latched_buttons: FighterInput::INTERACT,
        ..default()
    };
    apply_pause_request(&mut context, &mut pending, false);
    assert_eq!(context, ClientInputContext::Gameplay);
    assert_eq!(pending.latched_buttons, FighterInput::INTERACT);

    apply_pause_request(&mut context, &mut pending, true);
    assert_eq!(context, ClientInputContext::Paused);
    assert_eq!(pending.latched_buttons, 0);

    context = ClientInputContext::Shell;
    pending.latched_buttons = FighterInput::INTERACT;
    apply_pause_request(&mut context, &mut pending, true);
    assert_eq!(context, ClientInputContext::Shell);
    assert_eq!(pending.latched_buttons, 0);
}

#[test]
fn camera_clamp_uses_viewport_aspect_and_centers_oversized_axes() {
    let bounds = crate::map::AxisAlignedMapRect {
        min: Vec2::new(-896.0, -576.0),
        max: Vec2::new(896.0, 576.0),
    };
    let landscape = clamp_camera_center(Vec2::new(900.0, 0.0), bounds, Vec2::new(16.0, 9.0));
    assert!((landscape.x - 256.0).abs() < 0.001);

    let portrait = clamp_camera_center(Vec2::new(900.0, 0.0), bounds, Vec2::new(9.0, 16.0));
    assert!(portrait.x > landscape.x);
    assert!(portrait.x <= bounds.max.x);

    let oversized = clamp_camera_center(Vec2::new(900.0, 400.0), bounds, Vec2::new(4000.0, 100.0));
    assert!(oversized.x.abs() < 0.001);
    assert!((oversized.y - 216.0).abs() < 0.001);
}

#[test]
fn paused_input_writes_neutral_and_clears_latched_actions() {
    #[derive(Resource, Default)]
    struct FixedTickCount(u32);

    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(bevy::time::TimeUpdateStrategy::ManualDuration(
            crate::timing::SIMULATION_TICK,
        ))
        .init_resource::<PendingLocalActions>()
        .init_resource::<FixedTickCount>()
        .insert_resource(ClientInputContext::Paused)
        .insert_resource(ClientPlayableGate(true))
        .add_systems(Update, write_client_input)
        .add_systems(FixedUpdate, |mut ticks: ResMut<FixedTickCount>| {
            ticks.0 = ticks.0.saturating_add(1);
        });
    app.world_mut().spawn((
        ActionState::<FighterInput>::default(),
        InputMarker::<FighterInput>::default(),
    ));
    {
        let mut pending = app.world_mut().resource_mut::<PendingLocalActions>();
        pending.move_axis = Vec2::X;
        pending.held_buttons = FighterInput::PRIMARY_FIRE;
        pending.latched_buttons = FighterInput::INTERACT;
    }

    app.update();
    app.update();

    let world = app.world_mut();
    let mut query = world.query::<&ActionState<FighterInput>>();
    assert_eq!(
        query.single(world).expect("one input marker").0,
        FighterInput::default()
    );
    assert_eq!(world.resource::<PendingLocalActions>().latched_buttons, 0);
    assert!(world.resource::<FixedTickCount>().0 > 0);
}

#[test]
fn hud_reports_connection_device_actions_and_scoreboard_state() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<PendingLocalActions>()
        .init_resource::<ClientInputContext>()
        .add_systems(Update, update_client_hud);
    let status = app.world_mut().spawn((InputStatusText, Text::new(""))).id();
    let scoreboard = app
        .world_mut()
        .spawn((ScoreboardOverlay, Visibility::Hidden))
        .id();
    app.world_mut().spawn((
        Client,
        ClientJoinStatus {
            phase: ClientJoinPhase::Active {
                player_id: PlayerId(1),
                network_entity_id: NetworkEntityId(1),
            },
            started_at: Duration::ZERO,
            disconnect_requested: false,
        },
    ));
    {
        let mut pending = app.world_mut().resource_mut::<PendingLocalActions>();
        pending.active_device = ActiveInputDevice::Gamepad(Entity::from_raw_u32(7).unwrap());
        pending.action_indicator = ACTION_PRIMARY_FIRE | ACTION_SCOREBOARD;
        pending.scoreboard_held = true;
    }

    app.update();

    let text = app.world().get::<Text>(status).expect("HUD status text");
    assert!(text.0.contains("Connection: connected"));
    assert!(text.0.contains("Input: gamepad | gameplay"));
    assert!(text.0.contains("Actions: fire,scoreboard"));
    assert_eq!(
        app.world().get::<Visibility>(scoreboard),
        Some(&Visibility::Inherited)
    );
}

#[test]
#[allow(clippy::float_cmp)]
fn pause_keys_adjust_calibration_only_while_paused() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ClientInputContext>()
        .init_resource::<ClientInputSettings>()
        .init_resource::<InputSettingsSelection>()
        .add_systems(Update, adjust_input_settings_from_pause_keys);

    // Gameplay context ignores the adjustment keys entirely.
    let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keyboard.press(KeyCode::BracketRight);
    keyboard.press(KeyCode::KeyI);
    app.update();
    let settings = *app.world().resource::<ClientInputSettings>();
    // Exact zero is representable; the default never applied a radial remap.
    assert_eq!(settings.move_deadzone, 0.0);
    assert!(!settings.invert_move_y);

    *app.world_mut().resource_mut::<ClientInputContext>() = ClientInputContext::Paused;
    let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keyboard.reset_all();
    keyboard.press(KeyCode::BracketRight);
    keyboard.press(KeyCode::KeyI);
    app.update();
    let settings = *app.world().resource::<ClientInputSettings>();
    assert!((settings.move_deadzone - 0.05).abs() < 1e-6);
    assert!(settings.invert_move_y);

    // Tab cycles the selected field, then reset restores validated defaults.
    let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keyboard.reset_all();
    keyboard.press(KeyCode::Tab);
    keyboard.press(KeyCode::BracketRight);
    app.update();
    let settings = *app.world().resource::<ClientInputSettings>();
    assert!((settings.aim_deadzone - 0.30).abs() < 1e-6);
    assert_eq!(
        app.world().resource::<InputSettingsSelection>().field,
        InputSettingsField::Calibration(CalibrationField::AimDeadzone)
    );

    let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keyboard.reset_all();
    keyboard.press(KeyCode::KeyR);
    app.update();
    // Two calibration adjustments and one inversion preceded the reset; the reset bumps the
    // revision from that previous value so consumers never miss the change.
    assert_eq!(
        *app.world().resource::<ClientInputSettings>(),
        ClientInputSettings {
            revision: 4,
            ..ClientInputSettings::default()
        }
    );
}

#[test]
fn pause_trigger_calibration_keeps_the_hysteresis_band() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(ClientInputContext::Paused)
        .init_resource::<ClientInputSettings>()
        .init_resource::<InputSettingsSelection>()
        .add_systems(Update, adjust_input_settings_from_pause_keys);

    // Cycle to Trigger press (three fields after Move deadzone) and lower it repeatedly.
    for _ in 0..3 {
        let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keyboard.reset_all();
        keyboard.press(KeyCode::Tab);
        app.update();
    }
    assert_eq!(
        app.world().resource::<InputSettingsSelection>().field,
        InputSettingsField::Calibration(CalibrationField::TriggerPress)
    );
    for _ in 0..8 {
        let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
        keyboard.reset_all();
        keyboard.press(KeyCode::BracketLeft);
        app.update();
    }
    let settings = *app.world().resource::<ClientInputSettings>();
    assert!(
        (settings.trigger_press - (settings.trigger_release + MIN_TRIGGER_HYSTERESIS)).abs() < 1e-6
    );
    assert!(settings.validate().is_ok());
}

#[test]
fn pause_rebind_flow_captures_the_next_physical_key_press() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(ClientInputContext::Paused)
        .init_resource::<ClientInputSettings>()
        .init_resource::<InputSettingsSelection>()
        .add_systems(Update, adjust_input_settings_from_pause_keys);
    app.world_mut()
        .resource_mut::<InputSettingsSelection>()
        .field = InputSettingsField::Keyboard(KeyboardAction::Ultimate);

    // B arms rebind listening for the selected binding row.
    let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keyboard.press(KeyCode::KeyB);
    app.update();
    assert!(app.world().resource::<InputSettingsSelection>().listening);

    // The next non-modifier key commits the rebind and ends listening.
    let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keyboard.reset_all();
    keyboard.press(KeyCode::KeyP);
    app.update();
    let settings = *app.world().resource::<ClientInputSettings>();
    assert_eq!(settings.keyboard.ultimate, KeyCode::KeyP);
    assert!(!app.world().resource::<InputSettingsSelection>().listening);
    assert_eq!(settings.revision, 1);

    // While listening, modifiers are refused and B remains a valid gameplay binding.
    let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keyboard.reset_all();
    keyboard.press(KeyCode::KeyB);
    app.update();
    assert!(app.world().resource::<InputSettingsSelection>().listening);
    let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keyboard.reset_all();
    keyboard.press(KeyCode::ShiftLeft);
    app.update();
    assert!(
        app.world().resource::<InputSettingsSelection>().listening,
        "a modifier press must not commit a binding"
    );
    let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keyboard.reset_all();
    keyboard.press(KeyCode::KeyB);
    app.update();
    assert!(!app.world().resource::<InputSettingsSelection>().listening);
    assert_eq!(
        app.world()
            .resource::<ClientInputSettings>()
            .keyboard
            .ultimate,
        KeyCode::KeyB
    );
}

#[test]
fn pause_rebind_captures_mouse_and_gamepad_buttons() {
    let mut gamepad = Gamepad::default();
    gamepad.digital_mut().press(GamepadButton::North);
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ButtonInput<MouseButton>>()
        .insert_resource(ClientInputContext::Paused)
        .init_resource::<ClientInputSettings>()
        .init_resource::<InputSettingsSelection>()
        .add_systems(Update, adjust_input_settings_from_pause_keys);

    // Mouse primary rebinds from the next mouse button press.
    app.world_mut()
        .resource_mut::<InputSettingsSelection>()
        .field = InputSettingsField::MousePrimary;
    let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keyboard.press(KeyCode::KeyB);
    app.update();
    // Clear the arming key's just-pressed edge so only the mouse press can commit or cancel.
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    let mut mouse = app.world_mut().resource_mut::<ButtonInput<MouseButton>>();
    mouse.press(MouseButton::Right);
    app.update();
    assert_eq!(
        app.world().resource::<ClientInputSettings>().mouse_primary,
        MouseButton::Right
    );

    // A gamepad row armed from the keyboard captures the next controller button press.
    app.world_mut()
        .resource_mut::<InputSettingsSelection>()
        .field = InputSettingsField::Gamepad(GamepadAction::Ultimate);
    let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keyboard.reset_all();
    keyboard.press(KeyCode::KeyB);
    app.update();
    app.world_mut().spawn(gamepad);
    // Clear the arming key's just-pressed edge so only the pad press can commit or cancel.
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.update();
    assert_eq!(
        app.world()
            .resource::<ClientInputSettings>()
            .gamepad
            .ultimate,
        GamepadButton::North
    );
    assert!(!app.world().resource::<InputSettingsSelection>().listening);
}

#[test]
fn capturing_a_rebind_suppresses_pause_cancel_and_latched_actions() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<ButtonInput<KeyCode>>()
        .insert_resource(ClientInputContext::Paused)
        .init_resource::<PendingLocalActions>()
        .init_resource::<InputDeviceActivity>()
        .init_resource::<ClientInputSettings>()
        .insert_resource(InputSettingsSelection {
            field: InputSettingsField::Keyboard(KeyboardAction::Ultimate),
            listening: true,
        })
        .add_systems(Update, sample_local_input);
    app.world_mut().spawn((Window::default(), PrimaryWindow));

    // Escape is the default pause binding; while listening it must neither unpause nor
    // register as a gameplay cancel. The settings capture system consumes it as local cancel.
    let mut keyboard = app.world_mut().resource_mut::<ButtonInput<KeyCode>>();
    keyboard.press(KeyCode::Escape);
    keyboard.press(KeyCode::Space);
    app.update();
    assert_eq!(
        *app.world().resource::<ClientInputContext>(),
        ClientInputContext::Paused
    );
    let pending = app.world().resource::<PendingLocalActions>();
    assert!(!pending.cancel_pressed);
    assert_eq!(pending.action_indicator & ACTION_PAUSE, 0);
    assert_eq!(pending.latched_buttons, 0);
}

#[test]
fn settings_revision_change_clears_held_and_latched_actions() {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .init_resource::<ButtonInput<KeyCode>>()
        .init_resource::<ClientInputContext>()
        .init_resource::<InputDeviceActivity>()
        .init_resource::<PendingLocalActions>()
        .init_resource::<ClientInputSettings>()
        .add_systems(Update, sample_local_input);
    app.world_mut().spawn((Window::default(), PrimaryWindow));

    {
        let mut pending = app.world_mut().resource_mut::<PendingLocalActions>();
        pending.held_buttons = FighterInput::PRIMARY_FIRE;
        pending.latched_buttons = FighterInput::INTERACT;
        pending.input_settings_revision = 0;
    }
    app.update();
    // Held buttons are recomputed from device state each sample, but a latched action must
    // survive while the settings revision is unchanged.
    let pending = app.world().resource::<PendingLocalActions>();
    assert_eq!(pending.latched_buttons, FighterInput::INTERACT);
    assert_eq!(pending.input_settings_revision, 0);

    app.world_mut()
        .resource_mut::<ClientInputSettings>()
        .toggle_inversion(true);
    app.update();
    let pending = app.world().resource::<PendingLocalActions>();
    assert_eq!(pending.held_buttons, 0);
    assert_eq!(pending.latched_buttons, 0);
    assert_eq!(pending.input_settings_revision, 1);
}

#[test]
fn settings_overlay_reports_calibration_bindings_and_conflicts() {
    let mut settings = ClientInputSettings::default();
    let selection = InputSettingsSelection::default();
    let lines = compose_input_settings_lines(&settings, selection);
    assert!(lines.len() <= 8);
    assert!(
        lines[0].replace("move=[0.00]", "").contains("aim=0.25")
            && lines[0].contains("trigger=0.55/0.45")
    );
    assert!(lines.iter().any(|line| line.contains("Bindings OK")));
    let mouse_selected = InputSettingsSelection {
        field: InputSettingsField::MousePrimary,
        listening: false,
    };
    assert!(
        compose_input_settings_lines(&settings, mouse_selected)
            .iter()
            .any(|line| line.contains("Mouse [Left]"))
    );

    settings
        .rebind(KeyboardAction::Ultimate, KeyCode::KeyQ)
        .expect("rebind applies");
    let lines = compose_input_settings_lines(&settings, selection);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("Conflict: Active item, Ultimate"))
    );

    // A listening selection replaces the hint row with the rebind capture prompt and marks
    // the target field.
    let listening = InputSettingsSelection {
        field: InputSettingsField::Keyboard(KeyboardAction::Ultimate),
        listening: true,
    };
    let lines = compose_input_settings_lines(&settings, listening);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("Rebind Ultimate: press a key"))
    );
    assert!(
        lines
            .iter()
            .any(|line| line.contains("ult=[E]") || line.contains("ult=[Q]"))
    );

    settings.rebind_gamepad(GamepadAction::Cancel, GamepadButton::South);
    let lines = compose_input_settings_lines(&settings, selection);
    assert!(
        lines
            .iter()
            .any(|line| line.contains("Conflict: Active item, Ultimate, Interact, Cancel"))
    );
}

#[test]
fn join_rejections_map_to_stable_failure_categories() {
    use crate::diagnostics::FailureCategory;
    use crate::protocol::MatchJoinRejection;

    let cases = [
        (
            MatchJoinRejection::ProtocolVersionMismatch,
            FailureCategory::ProtocolMismatch,
        ),
        (
            MatchJoinRejection::BuildVersionMismatch,
            FailureCategory::ProtocolMismatch,
        ),
        (
            MatchJoinRejection::RegistryMismatch,
            FailureCategory::ProtocolMismatch,
        ),
        (
            MatchJoinRejection::ContentMismatch,
            FailureCategory::ContentMismatch,
        ),
        (
            MatchJoinRejection::HandshakeTimeout,
            FailureCategory::Timeout,
        ),
        (
            MatchJoinRejection::ServerFull,
            FailureCategory::ShutdownIncomplete,
        ),
        (
            MatchJoinRejection::MatchFull,
            FailureCategory::ShutdownIncomplete,
        ),
        (
            MatchJoinRejection::MatchInProgress,
            FailureCategory::ShutdownIncomplete,
        ),
        (
            MatchJoinRejection::IdentifierExhausted,
            FailureCategory::ShutdownIncomplete,
        ),
    ];
    for (reason, expected) in cases {
        assert_eq!(join_rejection_category(&reason), expected);
    }
}

#[test]
fn routed_grant_is_bound_to_the_current_session_and_accepted_once() {
    use crate::{config::GameMode, protocol::RouteCapability};
    use brawler_routing::{AllocationId, MatchId, PeerId, RequestId, RouteId};

    let mut lifecycle = RoutedClientLifecycle::default();
    lifecycle.start_lobby();
    let request_id = RequestId::new(41).expect("non-zero request ID");
    let grant = MatchRouteGrant {
        request_id,
        allocation_id: AllocationId::new(2).expect("non-zero allocation ID"),
        match_id: MatchId::new(3).expect("non-zero match ID"),
        route_id: RouteId::new(4).expect("non-zero route ID"),
        peer_id: PeerId::new(5).expect("non-zero peer ID"),
        game_mode: GameMode::Wipeout,
        capability: RouteCapability::from_bytes([7; RouteCapability::BYTES])
            .expect("non-zero route capability"),
        activation_expiry_unix_ms: 10,
        route_expiry_unix_ms: 20,
    };
    assert!(lifecycle.accept_grant(grant));
    assert_eq!(lifecycle.phase, RoutedClientPhase::AwaitingLobbyUnlink);
    assert_eq!(lifecycle.current_request_id, Some(request_id));
    assert!(
        !lifecycle.accept_grant(grant),
        "duplicate grants must not replace a route"
    );
}

#[test]
fn routed_return_to_lobby_is_an_intentional_nonterminal_transition() {
    let mut lifecycle = RoutedClientLifecycle {
        phase: RoutedClientPhase::Match,
        generation: 2,
        ..default()
    };
    assert!(lifecycle.request_return_to_lobby());
    assert_eq!(
        lifecycle.phase,
        RoutedClientPhase::AwaitingMatchUnlink,
        "disconnect is issued only after this state is observed"
    );
    assert!(!lifecycle.request_return_to_lobby());
}

#[test]
fn replicated_completed_match_requests_fresh_lobby_without_terminal_exit() {
    let mut config = ClientNetworkConfig::new(1);
    config.transport = NetworkTransport::RoutedUdp;
    let mut app = App::new();
    app.insert_resource(config)
        .insert_resource(RoutedClientLifecycle {
            phase: RoutedClientPhase::Match,
            generation: 2,
            ..default()
        })
        .init_resource::<ClientMatchResultState>()
        .init_resource::<SelectedGameType>();
    app.add_systems(Update, observe_completed_match);
    app.world_mut().spawn((
        MatchRoot,
        MatchState {
            match_id: crate::matchplay::MatchId(9),
            mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
            phase: MatchPhase::Completed {
                completed_at_tick: 12,
                restart_unlocked_at_tick: 72,
                result: crate::matchplay::MatchResult::Draw,
            },
            rules_revision: 1,
        },
    ));
    app.update();
    assert_eq!(
        app.world().resource::<RoutedClientLifecycle>().phase,
        RoutedClientPhase::AwaitingMatchUnlink
    );
    assert_eq!(
        app.world()
            .resource::<ClientMatchResultState>()
            .context
            .as_ref()
            .map(|context| context.result),
        Some(crate::matchplay::MatchResult::Draw)
    );
}

#[test]
fn routed_headless_return_exit_requires_a_fresh_lobby_generation() {
    fn app_for(generation: u64) -> App {
        let mut config = ClientNetworkConfig::new(1);
        config.headless = true;
        config.transport = NetworkTransport::RoutedUdp;
        config.exit_after_roster = Some(2);
        config.exit_after_lobby_return = true;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<AppExit>()
            .insert_resource(config)
            .add_systems(Update, observe_fresh_lobby_return);
        app.world_mut().spawn((
            Client,
            RoutedClientSession {
                generation,
                kind: RoutedClientSessionKind::Lobby,
            },
            ClientJoinStatus {
                phase: ClientJoinPhase::LobbyActive {
                    player_id: PlayerId(1),
                },
                started_at: Duration::ZERO,
                disconnect_requested: false,
            },
        ));
        app
    }

    let mut initial_retry = app_for(2);
    initial_retry.update();
    assert!(initial_retry.should_exit().is_none());
    let mut returned = app_for(3);
    returned.update();
    assert!(returned.should_exit().is_some_and(|exit| exit.is_success()));
}

#[test]
fn routed_lobby_generations_and_request_ids_are_fresh() {
    let mut lifecycle = RoutedClientLifecycle::default();
    lifecycle.start_lobby();
    let first_generation = lifecycle.generation;
    lifecycle.start_lobby();
    assert_eq!(lifecycle.generation, first_generation + 1);
    assert_eq!(lifecycle.current_request_id, None);
    assert_eq!(lifecycle.accepted_grant, None);
}
