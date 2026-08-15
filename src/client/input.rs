//! Local device sampling, render-to-fixed input bridging, and input diagnostics.
#![allow(clippy::wildcard_imports)]

use super::*;

pub(super) fn logical_key_pressed(keyboard: Option<&ButtonInput<Key>>, expected: &str) -> bool {
    keyboard.is_some_and(|keyboard| {
        keyboard.get_pressed().any(|key| {
            matches!(key, Key::Character(character) if character
                .as_str()
                .eq_ignore_ascii_case(expected))
        })
    })
}

/// Converts controller, keyboard, and mouse state to the shared action representation.
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub(super) fn sample_local_input(
    mut pending: ResMut<PendingLocalActions>,
    trace: Option<ResMut<LiveInputTrace>>,
    mut context: ResMut<ClientInputContext>,
    mut activity: ResMut<InputDeviceActivity>,
    tuning: Res<InputTuning>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    logical_keyboard: Option<Res<ButtonInput<Key>>>,
    mouse_buttons: Option<Res<ButtonInput<MouseButton>>>,
    mouse_motion: Option<Res<AccumulatedMouseMotion>>,
    gamepads: Query<(Entity, &Gamepad)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    fighters: Query<(&Position, Option<&ResolvedWeapon>), (With<Fighter>, With<Controlled>)>,
) {
    let Some(keyboard) = keyboard else {
        return;
    };
    if windows.is_empty() && gamepads.is_empty() {
        // Headless clients can still drive PendingLocalActions from a test/script. Do not
        // replace that scripted state with a synthetic zero-device sample.
        return;
    }
    let mouse_buttons = mouse_buttons.as_deref();
    let logical_keyboard = logical_keyboard.as_deref();
    let mut keyboard_move = Vec2::ZERO;
    if keyboard.pressed(KeyCode::KeyA) || logical_key_pressed(logical_keyboard, "a") {
        keyboard_move.x -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyD) || logical_key_pressed(logical_keyboard, "d") {
        keyboard_move.x += 1.0;
    }
    if keyboard.pressed(KeyCode::KeyS) || logical_key_pressed(logical_keyboard, "s") {
        keyboard_move.y -= 1.0;
    }
    if keyboard.pressed(KeyCode::KeyW) || logical_key_pressed(logical_keyboard, "w") {
        keyboard_move.y += 1.0;
    }
    let keyboard_active = keyboard_move != Vec2::ZERO
        || keyboard.any_just_pressed([
            KeyCode::KeyQ,
            KeyCode::KeyE,
            KeyCode::Space,
            KeyCode::Enter,
            KeyCode::Escape,
        ]);
    let keyboard_scoreboard = keyboard.pressed(KeyCode::Tab);
    let mouse_active = mouse_motion.is_some_and(|motion| motion.delta.length_squared() > 0.0);

    let mut meaningful_gamepads = Vec::new();
    for (entity, gamepad) in &gamepads {
        let left = gamepad.left_stick();
        let right = gamepad.right_stick();
        let trigger = gamepad.get(GamepadButton::RightTrigger2).unwrap_or(0.0);
        let meaningful = left.length() >= tuning.move_deadzone
            || right.length() >= tuning.aim_deadzone
            || trigger >= tuning.trigger_release
            || gamepad.any_pressed([
                GamepadButton::LeftTrigger,
                GamepadButton::RightTrigger,
                GamepadButton::South,
                GamepadButton::East,
                GamepadButton::Select,
                GamepadButton::Start,
            ]);
        let changed = activity
            .last_samples
            .iter()
            .find(|(id, _, _, _)| *id == entity)
            .is_none_or(|(_, previous_left, previous_right, previous_trigger)| {
                previous_left.distance_squared(left) > 0.0001
                    || previous_right.distance_squared(right) > 0.0001
                    || (trigger >= tuning.trigger_release
                        && *previous_trigger < tuning.trigger_release)
            });
        if meaningful
            && (changed
                || gamepad.any_just_pressed([
                    GamepadButton::LeftTrigger,
                    GamepadButton::RightTrigger,
                    GamepadButton::South,
                    GamepadButton::East,
                    GamepadButton::Select,
                    GamepadButton::Start,
                ]))
        {
            activity.recent_gamepads.retain(|id| *id != entity);
            activity.recent_gamepads.push(entity);
            meaningful_gamepads.push(entity);
        }
        activity.last_samples.retain(|(id, _, _, _)| *id != entity);
        activity.last_samples.push((entity, left, right, trigger));
    }
    activity
        .recent_gamepads
        .retain(|id| gamepads.get(*id).is_ok());
    activity
        .last_samples
        .retain(|(id, _, _, _)| gamepads.get(*id).is_ok());
    let connected_gamepads: Vec<_> = gamepads.iter().map(|(entity, _)| entity).collect();
    let active_gamepad = select_active_gamepad(
        pending.active_device,
        &activity.recent_gamepads,
        &meaningful_gamepads,
        &connected_gamepads,
    );
    let gamepad_sample = active_gamepad.and_then(|entity| {
        gamepads
            .get(entity)
            .ok()
            .map(|(_, gamepad)| (entity, gamepad.left_stick(), gamepad.right_stick(), gamepad))
    });

    let keyboard_mouse_active = keyboard_active
        || mouse_active
        || keyboard_scoreboard
        || mouse_buttons
            .is_some_and(|buttons| buttons.any_pressed([MouseButton::Left, MouseButton::Right]));
    let meaningful_gamepad = if meaningful_gamepads.is_empty() {
        None
    } else {
        active_gamepad.or_else(|| meaningful_gamepads.last().copied())
    };
    pending.active_device = select_active_input_device(
        pending.active_device,
        keyboard_mouse_active,
        active_gamepad,
        meaningful_gamepad,
    );

    let (move_axis, aim_axis, aim_distance, gamepad_buttons, gamepad_pause, gamepad_interact) =
        match (pending.active_device, gamepad_sample) {
            (ActiveInputDevice::Gamepad(active), Some((entity, left, right, gamepad)))
                if active == entity =>
            {
                let mut buttons = 0;
                let trigger = gamepad.get(GamepadButton::RightTrigger2).unwrap_or(0.0);
                if trigger_pressed(
                    pending.held_buttons & FighterInput::PRIMARY_FIRE != 0,
                    trigger,
                    *tuning,
                ) {
                    buttons |= FighterInput::PRIMARY_FIRE;
                }
                if gamepad.pressed(GamepadButton::LeftTrigger) {
                    buttons |= FighterInput::ACTIVE_ITEM;
                }
                if gamepad.pressed(GamepadButton::RightTrigger) {
                    buttons |= FighterInput::ULTIMATE;
                }
                (
                    left,
                    right.is_finite().then_some(right),
                    committed_aim(right, *tuning).and_then(|_| {
                        controlled_lob_range(&fighters).map(|range| {
                            radial_deadzone(right, tuning.aim_deadzone).length() * range
                        })
                    }),
                    buttons,
                    gamepad.just_pressed(GamepadButton::Start),
                    gamepad.just_pressed(GamepadButton::South),
                )
            }
            _ => {
                let mut buttons = 0;
                if mouse_buttons.is_some_and(|buttons| buttons.pressed(MouseButton::Left)) {
                    buttons |= FighterInput::PRIMARY_FIRE;
                }
                if keyboard.pressed(KeyCode::KeyQ) {
                    buttons |= FighterInput::ACTIVE_ITEM;
                }
                if keyboard.pressed(KeyCode::KeyE) {
                    buttons |= FighterInput::ULTIMATE;
                }
                let mouse_aim = mouse_aim(&windows, &cameras, &fighters);
                (
                    keyboard_move,
                    mouse_aim.map(|(direction, _)| direction),
                    mouse_aim.map(|(_, distance)| distance),
                    buttons,
                    keyboard.just_pressed(KeyCode::Escape),
                    keyboard.just_pressed(KeyCode::Space) || keyboard.just_pressed(KeyCode::Enter),
                )
            }
        };

    pending.cancel_pressed = gamepad_sample
        .is_some_and(|(_, _, _, gamepad)| gamepad.just_pressed(GamepadButton::East))
        || keyboard.just_pressed(KeyCode::Escape);
    pending.scoreboard_held = gamepad_sample
        .is_some_and(|(_, _, _, gamepad)| gamepad.pressed(GamepadButton::Select))
        || keyboard.pressed(KeyCode::Tab);
    pending.action_indicator = u16::from(gamepad_buttons);
    if gamepad_interact
        || keyboard.just_pressed(KeyCode::Space)
        || keyboard.just_pressed(KeyCode::Enter)
    {
        pending.action_indicator |= ACTION_INTERACT;
    }
    if pending.cancel_pressed {
        pending.action_indicator |= ACTION_CANCEL;
    }
    if gamepad_pause {
        pending.action_indicator |= ACTION_PAUSE;
    }
    if pending.scoreboard_held {
        pending.action_indicator |= ACTION_SCOREBOARD;
    }
    apply_pause_request(&mut context, &mut pending, gamepad_pause);
    if gamepad_interact
        || keyboard.just_pressed(KeyCode::Space)
        || keyboard.just_pressed(KeyCode::Enter)
    {
        pending.latched_buttons |= FighterInput::INTERACT;
    }
    pending.move_axis = move_axis;
    pending.aim_axis = aim_axis;
    pending.aim_distance = aim_distance;
    pending.held_buttons = gamepad_buttons;

    if let Some(mut trace) = trace.filter(|trace| trace.enabled) {
        let focused = windows.iter().next().is_some_and(|window| window.focused);
        let wasd = [
            keyboard.pressed(KeyCode::KeyW),
            keyboard.pressed(KeyCode::KeyA),
            keyboard.pressed(KeyCode::KeyS),
            keyboard.pressed(KeyCode::KeyD),
        ];
        let sample = (focused, wasd, pending.move_axis, pending.active_device);
        if trace.last_sample != Some(sample) {
            info!(
                window_focused = focused,
                physical_w = wasd[0],
                physical_a = wasd[1],
                physical_s = wasd[2],
                physical_d = wasd[3],
                logical_w = logical_key_pressed(logical_keyboard, "w"),
                logical_a = logical_key_pressed(logical_keyboard, "a"),
                logical_s = logical_key_pressed(logical_keyboard, "s"),
                logical_d = logical_key_pressed(logical_keyboard, "d"),
                active_device = ?pending.active_device,
                move_axis = ?pending.move_axis,
                "live client input sample changed"
            );
            trace.last_sample = Some(sample);
        }
        if trace.last_aim != pending.aim_axis {
            info!(aim_axis = ?pending.aim_axis, "live client aim sample changed");
            trace.last_aim = pending.aim_axis;
        }
    }
}

pub(super) fn apply_headless_input(
    automation: Res<HeadlessAutomation>,
    mut pending: ResMut<PendingLocalActions>,
    statuses: Query<&ClientJoinStatus, With<Client>>,
    controlled: Query<&Position, (With<Fighter>, With<Controlled>)>,
    fighters: Query<(&NetworkEntityId, &Position), With<Fighter>>,
) {
    if !statuses
        .iter()
        .any(|status| matches!(status.phase, ClientJoinPhase::Active { .. }))
    {
        return;
    }
    if automation
        .simulation_ticks
        .is_some_and(|limit| automation.elapsed_ticks >= limit)
    {
        return;
    }
    let controlled_position = controlled.iter().next().map(|position| position.0);
    let dummy_position = fighters
        .iter()
        .find(|(network_id, _)| network_id.0 == 0)
        .map(|(_, target)| target.0);
    let aim_delta = controlled_position
        .zip(dummy_position)
        .map(|(position, target)| target - position)
        .filter(|delta| delta.is_finite() && delta.length_squared() > f32::EPSILON);
    let aim_axis = if automation.aim_at_dummy {
        aim_delta.map(Vec2::normalize)
    } else {
        automation.aim_axis
    };
    let move_axis = if automation.aim_at_dummy && automation.move_axis != Vec2::ZERO {
        controlled_position
            .zip(dummy_position)
            .map(|(position, target)| target - position)
            .filter(|delta| delta.is_finite() && delta.length_squared() > f32::EPSILON)
            .map_or(automation.move_axis, Vec2::normalize)
    } else {
        automation.move_axis
    };
    if move_axis != Vec2::ZERO || aim_axis.is_some() || automation.aim_at_dummy || automation.fire {
        if automation.elapsed_ticks == 0 {
            info!(
                move_axis = ?move_axis,
                aim_axis = ?aim_axis,
                aim_at_dummy = automation.aim_at_dummy,
                fire = automation.fire,
                "headless movement automation enabled"
            );
        }
        pending.move_axis = move_axis;
        pending.aim_axis = aim_axis;
        pending.aim_distance = if automation.aim_at_dummy {
            aim_delta.map(Vec2::length)
        } else {
            None
        };
        let fire_held = automation.fire && automation.elapsed_ticks < HEADLESS_FIRE_DURATION_TICKS;
        pending.held_buttons = if fire_held {
            FighterInput::PRIMARY_FIRE
        } else {
            0
        };
        pending.active_device = ActiveInputDevice::KeyboardMouse;
    }
}

pub(super) fn advance_headless_automation(
    mut automation: ResMut<HeadlessAutomation>,
    statuses: Query<&ClientJoinStatus, With<Client>>,
) {
    if automation.simulation_ticks.is_some()
        && statuses
            .iter()
            .any(|status| matches!(status.phase, ClientJoinPhase::Active { .. }))
    {
        automation.elapsed_ticks = automation.elapsed_ticks.saturating_add(1);
    }
}

fn controlled_lob_range(
    fighters: &Query<(&Position, Option<&ResolvedWeapon>), (With<Fighter>, With<Controlled>)>,
) -> Option<f32> {
    fighters
        .iter()
        .next()?
        .1
        .and_then(|resolved| match resolved.recipe.delivery {
            crate::combat::DeliveryMethod::Lobbed { distance, .. } => Some(distance),
            _ => None,
        })
}

pub(super) fn mouse_aim(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    fighters: &Query<(&Position, Option<&ResolvedWeapon>), (With<Fighter>, With<Controlled>)>,
) -> Option<(Vec2, f32)> {
    let cursor = windows.iter().next()?.cursor_position()?;
    let (camera, camera_transform) = cameras.iter().next()?;
    let world = camera.viewport_to_world_2d(camera_transform, cursor).ok()?;
    let fighter = fighters.iter().next()?.0.0;
    let delta = world - fighter;
    (delta.is_finite() && delta.length_squared() > f32::EPSILON)
        .then(|| (delta.normalize(), delta.length()))
}

pub(super) fn write_client_input(
    mut pending: ResMut<PendingLocalActions>,
    trace: Option<ResMut<LiveInputTrace>>,
    context: Res<ClientInputContext>,
    playable: Res<ClientPlayableGate>,
    selecting: Query<(), (With<Fighter>, With<Controlled>, With<SelectingWeapon>)>,
    mut query: Query<&mut ActionState<FighterInput>, With<InputMarker<FighterInput>>>,
) {
    let mut trace = trace.filter(|trace| trace.enabled);
    let target_count = query.iter().count();
    let Ok(mut action) = query.single_mut() else {
        if let Some(trace) = trace.as_mut()
            && trace.last_write != Some((Vec2::ZERO, 0, target_count))
        {
            info!(
                target_count,
                "live client input has no unique Lightyear target"
            );
            trace.last_write = Some((Vec2::ZERO, 0, target_count));
        }
        return;
    };
    let input = if !playable.0
        || matches!(*context, ClientInputContext::Paused)
        || selecting.iter().next().is_some()
    {
        pending.latched_buttons = 0;
        FighterInput::default()
    } else {
        let buttons = pending.held_buttons | pending.latched_buttons;
        let input = FighterInput::from_axes_with_aim_distance(
            pending.move_axis,
            pending.aim_axis,
            pending.aim_distance,
            buttons,
        );
        pending.latched_buttons = 0;
        input
    };
    action.0 = input;
    let write_state = (
        input.move_axis.to_vec2(),
        input.gameplay_buttons,
        target_count,
    );
    if let Some(trace) = trace.as_mut()
        && trace.last_write != Some(write_state)
    {
        info!(
            target_count,
            move_axis = ?input.move_axis.to_vec2(),
            gameplay_buttons = input.gameplay_buttons,
            "live client Lightyear input changed"
        );
        trace.last_write = Some(write_state);
    }
}

pub(super) fn trace_client_interpolation_sync(
    trace: Option<ResMut<LiveInputTrace>>,
    timeline: Res<InterpolationTimeline>,
    clients: Query<&PingManager, (With<Client>, With<Connected>)>,
) {
    let Some(mut trace) = trace.filter(|trace| trace.enabled) else {
        return;
    };
    let samples = clients
        .iter()
        .next()
        .map_or(0, PingManager::latency_samples_recv);
    let state = timeline.is_synced();
    if trace.last_sync != Some(state) {
        info!(
            interpolation_synced = state,
            latency_samples = samples,
            interpolation_tick = timeline.now().tick().0,
            "live client interpolation sync changed"
        );
        trace.last_sync = Some(state);
    }
}

pub(super) fn trace_client_interpolation_history(
    trace: Option<ResMut<LiveInputTrace>>,
    fighters: Query<
        (
            Entity,
            Has<Interpolated>,
            Has<LinearVelocity>,
            Has<AngularVelocity>,
            Option<&ConfirmedHistory<Position>>,
            Option<&ConfirmedHistory<Rotation>>,
            Option<&ConfirmedHistory<LinearVelocity>>,
            Option<&ConfirmedHistory<AngularVelocity>>,
        ),
        (With<Fighter>, With<Remote>),
    >,
) {
    let Some(mut trace) = trace.filter(|trace| trace.enabled) else {
        return;
    };
    for (
        entity,
        interpolated,
        linear_velocity,
        angular_velocity,
        positions,
        rotations,
        linear_velocities,
        angular_velocities,
    ) in &fighters
    {
        let newest = positions
            .and_then(ConfirmedHistory::newest_present)
            .map(|(_, position)| position.0);
        let previous = trace
            .last_history
            .iter()
            .find(|(candidate, _)| *candidate == entity)
            .map(|(_, position)| *position);
        if newest
            .is_some_and(|newest| previous.is_none_or(|previous| previous.distance(newest) >= 32.0))
        {
            info!(
                ?entity,
                interpolated,
                linear_velocity,
                angular_velocity,
                position_history_len = positions.map_or(0, ConfirmedHistory::len),
                rotation_history_len = rotations.map_or(0, ConfirmedHistory::len),
                linear_velocity_history = linear_velocities.is_some(),
                angular_velocity_history = angular_velocities.is_some(),
                linear_velocity_history_len = linear_velocities.map_or(0, ConfirmedHistory::len),
                angular_velocity_history_len = angular_velocities.map_or(0, ConfirmedHistory::len),
                newest_position = ?newest,
                "live client interpolation history advanced"
            );
            trace
                .last_history
                .retain(|(candidate, _)| *candidate != entity);
            trace
                .last_history
                .push((entity, newest.expect("history position was checked")));
        }
    }
}

pub(super) fn add_controlled_input_marker(
    trigger: On<Add, Controlled>,
    mut commands: Commands,
    controlled: Query<(), Without<InputMarker<FighterInput>>>,
) {
    // Replicated components do not have to arrive in the same order as the server
    // spawn tuple.  In particular, Controlled can be added before Fighter.  The
    // ownership marker is the authoritative signal that this entity needs a local
    // input buffer; waiting for Fighter here can leave the client with no input
    // target at all.
    if controlled.get(trigger.entity).is_ok() {
        commands.entity(trigger.entity).insert((
            ActionState::<FighterInput>::default(),
            InputMarker::<FighterInput>::default(),
        ));
    }
}
