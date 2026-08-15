//! Focused client composition and behavior tests.

use super::*;

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
fn client_config_defaults_to_loopback_and_validates_roster_target() {
    let mut config = ClientNetworkConfig::new(1);
    assert!(config.validate().is_ok());
    config.exit_after_roster = Some(0);
    assert!(config.validate().is_err());
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
        .init_resource::<InputTuning>()
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
        .init_resource::<InputTuning>()
        .add_systems(Update, sample_local_input);
    app.world_mut().spawn((Window::default(), PrimaryWindow));

    app.update();

    assert_eq!(
        app.world().resource::<PendingLocalActions>().move_axis,
        Vec2::X
    );
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
        .init_resource::<InputTuning>()
        .add_systems(Update, sample_local_input);
    let gamepad_entity = app.world_mut().spawn(gamepad).id();
    let catalog = crate::combat::WeaponCatalog::embedded().expect("embedded weapon catalog");
    let fighter = crate::combat::FighterDefinitions::default().entries[0];
    let launcher = catalog
        .resolve_preset(crate::combat::WeaponPresetId(3), &fighter)
        .expect("arc launcher preset");
    app.world_mut()
        .spawn((Fighter, Controlled, Position::default(), launcher));

    app.update();

    let pending = app.world().resource::<PendingLocalActions>();
    assert_eq!(
        pending.active_device,
        ActiveInputDevice::Gamepad(gamepad_entity)
    );
    assert_eq!(pending.move_axis, Vec2::new(0.75, 0.0));
    assert_eq!(pending.aim_axis, Some(Vec2::new(0.0, -0.8)));
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
        .init_resource::<InputTuning>()
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
            Some(Vec2::new(0.0, -0.8)),
            FighterInput::PRIMARY_FIRE,
        )
    );
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
