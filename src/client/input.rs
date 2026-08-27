//! Local device sampling, render-to-fixed input bridging, and input diagnostics.
#![allow(clippy::wildcard_imports)]

use super::settings::key_code_letter;
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

/// A device value counts as activity only when it strictly exceeds its threshold. A zero
/// threshold still requires a nonzero sample, so a connected-but-resting stick or trigger
/// cannot claim the active input device (the default move deadzone is 0.0).
pub(super) fn exceeds_activity_threshold(value: f32, threshold: f32) -> bool {
    if threshold > 0.0 {
        value > threshold
    } else {
        value > 0.0
    }
}

/// Converts controller, keyboard, and mouse state to the shared action representation.
///
/// All device shaping (deadzones, aim commit, trigger hysteresis, inversion, bindings) is
/// client-owned and applied before quantization; the server keeps validating the quantized
/// intent without seeing physical devices.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub(super) fn sample_local_input(
    mut pending: ResMut<PendingLocalActions>,
    trace: Option<ResMut<LiveInputTrace>>,
    mut context: ResMut<ClientInputContext>,
    mut activity: ResMut<InputDeviceActivity>,
    settings: Res<ClientInputSettings>,
    selection: Option<Res<InputSettingsSelection>>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    logical_keyboard: Option<Res<ButtonInput<Key>>>,
    mouse_buttons: Option<Res<ButtonInput<MouseButton>>>,
    mouse_motion: Option<Res<AccumulatedMouseMotion>>,
    gamepads: Query<(Entity, &Gamepad)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    cameras: Query<(&Camera, &GlobalTransform), With<ArenaCamera>>,
    // The replicated loadout is the wire shape; a standalone `ResolvedWeapon` never
    // arrives in network play, so controller lob ranging must read the loadout's weapon.
    fighters: Query<
        (&Position, Option<&crate::builds::ResolvedMatchLoadout>),
        (With<Fighter>, With<Controlled>),
    >,
) {
    let Some(keyboard) = keyboard else {
        return;
    };
    if windows.is_empty() && gamepads.is_empty() {
        // Headless clients can still drive PendingLocalActions from a test/script. Do not
        // replace that scripted state with a synthetic zero-device sample.
        return;
    }
    // A binding or calibration change is a hard boundary for held state: a rebind must not
    // synthesize a stuck action from the previous physical layout.
    if pending.input_settings_revision != settings.revision {
        pending.held_buttons = 0;
        pending.latched_buttons = 0;
        pending.input_settings_revision = settings.revision;
    }
    let mouse_buttons = mouse_buttons.as_deref();
    let logical_keyboard = logical_keyboard.as_deref();
    let binding_pressed = |code: KeyCode| -> bool {
        keyboard.pressed(code)
            || key_code_letter(code)
                .is_some_and(|letter| logical_key_pressed(logical_keyboard, &letter.to_string()))
    };
    let bindings = settings.keyboard;
    let mut keyboard_move = Vec2::ZERO;
    if binding_pressed(bindings.move_left) {
        keyboard_move.x -= 1.0;
    }
    if binding_pressed(bindings.move_right) {
        keyboard_move.x += 1.0;
    }
    if binding_pressed(bindings.move_down) {
        keyboard_move.y -= 1.0;
    }
    if binding_pressed(bindings.move_up) {
        keyboard_move.y += 1.0;
    }
    let action_keys = [
        bindings.active_item,
        bindings.ultimate,
        bindings.interact,
        bindings.pause,
    ];
    let keyboard_active = keyboard_move != Vec2::ZERO || keyboard.any_just_pressed(action_keys);
    let keyboard_scoreboard = binding_pressed(bindings.scoreboard);
    let mouse_active = mouse_motion.is_some_and(|motion| motion.delta.length_squared() > 0.0);

    let mut meaningful_gamepads = Vec::new();
    for (entity, gamepad) in &gamepads {
        let left = gamepad.left_stick();
        let right = gamepad.right_stick();
        let trigger = gamepad.get(settings.gamepad.primary).unwrap_or(0.0);
        let meaningful = exceeds_activity_threshold(left.length(), settings.move_deadzone)
            || exceeds_activity_threshold(right.length(), settings.aim_deadzone)
            || exceeds_activity_threshold(trigger, settings.trigger_release)
            || settings
                .gamepad
                .rows()
                .iter()
                .any(|(_, button)| gamepad.pressed(*button));
        let changed = activity
            .last_samples
            .iter()
            .find(|(id, _, _, _)| *id == entity)
            .is_none_or(|(_, previous_left, previous_right, previous_trigger)| {
                previous_left.distance_squared(left) > 0.0001
                    || previous_right.distance_squared(right) > 0.0001
                    || (trigger >= settings.trigger_release
                        && *previous_trigger < settings.trigger_release)
            });
        if meaningful
            && (changed
                || settings
                    .gamepad
                    .rows()
                    .iter()
                    .any(|(_, button)| gamepad.just_pressed(*button)))
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
        || mouse_buttons.is_some_and(|buttons| {
            buttons.any_pressed([settings.mouse_primary, MouseButton::Right])
        });
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
                let trigger = gamepad.get(settings.gamepad.primary).unwrap_or(0.0);
                if settings.trigger_is_pressed(
                    pending.held_buttons & FighterInput::PRIMARY_FIRE != 0,
                    trigger,
                ) {
                    buttons |= FighterInput::PRIMARY_FIRE;
                }
                if gamepad.pressed(settings.gamepad.active_item) {
                    buttons |= FighterInput::ACTIVE_ITEM;
                }
                if gamepad.pressed(settings.gamepad.ultimate) {
                    buttons |= FighterInput::ULTIMATE;
                }
                let shaped_aim = right.is_finite().then(|| settings.shape_aim(right));
                (
                    settings.shape_move(left),
                    shaped_aim.flatten(),
                    shaped_aim
                        .is_some()
                        .then(|| {
                            controlled_lob_range(&fighters)
                                .and_then(|range| settings.shape_aim_distance(right, range))
                        })
                        .flatten(),
                    buttons,
                    gamepad.just_pressed(settings.gamepad.pause),
                    gamepad.just_pressed(settings.gamepad.interact),
                )
            }
            _ => {
                let mut buttons = 0;
                if mouse_buttons.is_some_and(|buttons| buttons.pressed(settings.mouse_primary)) {
                    buttons |= FighterInput::PRIMARY_FIRE;
                }
                if binding_pressed(bindings.active_item) {
                    buttons |= FighterInput::ACTIVE_ITEM;
                }
                if binding_pressed(bindings.ultimate) {
                    buttons |= FighterInput::ULTIMATE;
                }
                let mouse_aim = mouse_aim(&windows, &cameras, &fighters);
                (
                    keyboard_move,
                    mouse_aim.map(|(direction, _)| direction),
                    mouse_aim.map(|(_, distance)| distance),
                    buttons,
                    keyboard.just_pressed(bindings.pause),
                    keyboard.just_pressed(bindings.interact),
                )
            }
        };

    // While the pause overlay is capturing the next binding press, local-only keys must not
    // also unpause, cancel, or latch actions in the middle of a rebind.
    let capturing_rebind = selection.is_some_and(|selection| {
        selection.listening
            && matches!(
                *context,
                ClientInputContext::Menu | ClientInputContext::Shell
            )
    });
    pending.cancel_pressed = !capturing_rebind
        && (gamepad_sample
            .is_some_and(|(_, _, _, gamepad)| gamepad.just_pressed(settings.gamepad.cancel))
            || keyboard.just_pressed(bindings.pause));
    pending.scoreboard_held = !capturing_rebind
        && (gamepad_sample
            .is_some_and(|(_, _, _, gamepad)| gamepad.pressed(settings.gamepad.scoreboard))
            || binding_pressed(bindings.scoreboard));
    pending.action_indicator = u16::from(gamepad_buttons);
    if !capturing_rebind && (gamepad_interact || keyboard.just_pressed(bindings.interact)) {
        pending.action_indicator |= ACTION_INTERACT;
    }
    if pending.cancel_pressed {
        pending.action_indicator |= ACTION_CANCEL;
    }
    if gamepad_pause && !capturing_rebind {
        pending.action_indicator |= ACTION_PAUSE;
    }
    if pending.scoreboard_held {
        pending.action_indicator |= ACTION_SCOREBOARD;
    }
    if !capturing_rebind {
        apply_pause_request(&mut context, &mut pending, gamepad_pause);
    }
    if !capturing_rebind && (gamepad_interact || keyboard.just_pressed(bindings.interact)) {
        pending.latched_buttons |= FighterInput::INTERACT;
    }
    pending.move_axis = move_axis;
    pending.aim_axis = aim_axis;
    pending.aim_distance = aim_distance;
    pending.held_buttons = gamepad_buttons;

    if let Some(mut trace) = trace.filter(|trace| trace.enabled) {
        let focused = windows.iter().next().is_some_and(|window| window.focused);
        let wasd = [
            keyboard.pressed(bindings.move_up),
            keyboard.pressed(bindings.move_left),
            keyboard.pressed(bindings.move_down),
            keyboard.pressed(bindings.move_right),
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

/// Converts a targeted ultimate's physical two-phase interaction into the existing one-tick
/// authoritative ultimate intent. Immediate ultimates pass through unchanged.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn resolve_targeted_ultimate_input(
    mut pending: ResMut<PendingLocalActions>,
    context: Res<ClientInputContext>,
    playable: Res<ClientPlayableGate>,
    config: Option<Res<ClientNetworkConfig>>,
    controlled: Query<
        (
            &crate::builds::ResolvedMatchLoadout,
            &crate::builds::AbilityState,
            Has<crate::combat::Defeated>,
        ),
        (With<Fighter>, With<Controlled>),
    >,
) {
    let raw_buttons = pending.held_buttons;
    let rising_buttons = raw_buttons & !pending.targeted_ultimate.previous_raw_buttons;
    pending.targeted_ultimate.previous_raw_buttons = raw_buttons;
    if raw_buttons & FighterInput::PRIMARY_FIRE == 0 {
        pending.targeted_ultimate.primary_suppressed_until_release = false;
    } else if pending.targeted_ultimate.primary_suppressed_until_release {
        pending.held_buttons &= !FighterInput::PRIMARY_FIRE;
    }

    // Headless automation writes a fully formed authoritative test intent in the following
    // system and deliberately bypasses product device interaction modes.
    if config.is_some_and(|config| config.headless) {
        pending.targeted_ultimate.armed_for = None;
        return;
    }

    let Ok((loadout, ability, defeated)) = controlled.single() else {
        pending.targeted_ultimate.armed_for = None;
        pending.latched_buttons &= !FighterInput::ULTIMATE;
        return;
    };
    if loadout.ultimate.kind.activation_style() != crate::builds::UltimateActivationStyle::Targeted
    {
        pending.targeted_ultimate.armed_for = None;
        return;
    }

    // A targeted ultimate never reaches authority directly from its physical ultimate button.
    // Only a primary-fire confirmation below emits the existing ultimate intent.
    pending.held_buttons &= !FighterInput::ULTIMATE;
    let eligible = playable.0
        && matches!(*context, ClientInputContext::Gameplay)
        && !defeated
        && matches!(ability.phase, crate::builds::AbilityPhase::Ready);
    if !eligible {
        pending.targeted_ultimate.armed_for = None;
        pending.latched_buttons &= !FighterInput::ULTIMATE;
        return;
    }

    let ultimate_pressed = rising_buttons & FighterInput::ULTIMATE != 0;
    let primary_pressed = rising_buttons & FighterInput::PRIMARY_FIRE != 0;
    if pending.targeted_ultimate.is_targeting(loadout.ultimate.id) {
        // Targeting owns primary fire so confirmation cannot also fire the weapon.
        pending.held_buttons &= !FighterInput::PRIMARY_FIRE;
        if pending.cancel_pressed || ultimate_pressed {
            pending.targeted_ultimate.armed_for = None;
        } else if primary_pressed {
            pending.latched_buttons |= FighterInput::ULTIMATE;
            pending.targeted_ultimate.armed_for = None;
            // The confirmation press remains physically held for at least one render frame.
            // Keep consuming it until release so network acknowledgement latency cannot turn the
            // same click or trigger pull into a primary shot on the following frame.
            pending.targeted_ultimate.primary_suppressed_until_release = true;
        }
    } else if ultimate_pressed {
        pending.targeted_ultimate.armed_for = Some(loadout.ultimate.id);
        // A simultaneous or already-held primary press cannot confirm or fire; release and press
        // again makes the two phases unambiguous on mouse and analog-trigger input.
        pending.held_buttons &= !FighterInput::PRIMARY_FIRE;
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn apply_headless_input(
    automation: Res<HeadlessAutomation>,
    mut pending: ResMut<PendingLocalActions>,
    statuses: Query<&ClientJoinStatus, With<Client>>,
    controlled: Query<
        (
            &Position,
            &crate::combat::TeamId,
            Option<&crate::builds::AbilityState>,
            Option<&crate::builds::ResolvedMatchLoadout>,
        ),
        (With<Fighter>, With<Controlled>),
    >,
    fighters: Query<
        (
            &NetworkEntityId,
            &Position,
            &crate::combat::TeamId,
            Option<&crate::combat::Defeated>,
        ),
        With<Fighter>,
    >,
    sentries: Query<&crate::abilities::SentryIdentity, With<crate::abilities::Sentry>>,
    sentry_owners: Query<
        (
            &crate::combat::TeamId,
            &crate::builds::AbilityState,
            &crate::builds::ResolvedMatchLoadout,
        ),
        With<Fighter>,
    >,
) {
    if !statuses
        .iter()
        .any(|status| matches!(status.phase, ClientJoinPhase::Active { .. }))
    {
        return;
    }
    if headless_automation_complete(&automation) {
        return;
    }
    let controlled_fighter = controlled.iter().next();
    let controlled_position = controlled_fighter.map(|(position, _, _, _)| position.0);
    let enemy_sentry_deployed = controlled_fighter
        .is_some_and(|(_, team, _, _)| sentries.iter().any(|identity| identity.team_id != *team));
    let enemy_sentry_imminent = controlled_fighter.is_some_and(|(_, team, _, _)| {
        sentry_owners.iter().any(|(owner_team, ability, loadout)| {
            *owner_team != *team
                && loadout.ultimate.kind == crate::builds::UltimateKind::Sentry
                && (ability.charge >= 800
                    || matches!(
                        ability.phase,
                        crate::builds::AbilityPhase::Ready
                            | crate::builds::AbilityPhase::Deployed { .. }
                    ))
        })
    });
    let target_position = fighters
        .iter()
        .find(|(network_id, _, _, _)| network_id.0 == 0)
        .map(|(_, target, _, _)| target.0)
        .or_else(|| {
            let (controlled_position, controlled_team, _, _) = controlled_fighter?;
            fighters
                .iter()
                .filter(|(_, _, team, defeated)| **team != *controlled_team && defeated.is_none())
                .min_by(|(_, left, _, _), (_, right, _, _)| {
                    left.0
                        .distance_squared(controlled_position.0)
                        .total_cmp(&right.0.distance_squared(controlled_position.0))
                })
                .map(|(_, target, _, _)| target.0)
        });
    let aim_delta = controlled_position
        .zip(target_position)
        .map(|(position, target)| target - position)
        .filter(|delta| delta.is_finite() && delta.length_squared() > f32::EPSILON);
    let movement_delta = controlled_position
        .zip(target_position)
        .and_then(|(position, target)| headless_navigation_delta(position, target));
    let aim_axis = if automation.aim_at_dummy {
        aim_delta.map(Vec2::normalize)
    } else {
        automation.aim_axis
    };
    let (ability, loadout) =
        controlled_fighter.map_or((None, None), |(_, _, ability, loadout)| (ability, loadout));
    let (ultimate_ready, sentry_ready, sentry_deployed) =
        headless_ultimate_state(automation.ultimate, ability, loadout);
    let move_axis = if automation.aim_at_dummy && automation.move_axis != Vec2::ZERO {
        headless_combat_move_axis(
            automation.move_axis,
            aim_delta,
            movement_delta,
            sentry_ready || sentry_deployed,
        )
    } else {
        automation.move_axis
    };
    if move_axis != Vec2::ZERO
        || aim_axis.is_some()
        || automation.aim_at_dummy
        || automation.fire
        || automation.ultimate
    {
        if automation.elapsed_ticks == 0 {
            info!(
                move_axis = ?move_axis,
                aim_axis = ?aim_axis,
                aim_at_dummy = automation.aim_at_dummy,
                fire = automation.fire,
                ultimate = automation.ultimate,
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
        let fire_held = automation.fire
            && automation.elapsed_ticks < HEADLESS_FIRE_DURATION_TICKS
            && !enemy_sentry_deployed
            && !enemy_sentry_imminent;
        let ultimate_held = ultimate_ready && automation.elapsed_ticks.is_multiple_of(12);
        pending.held_buttons = (u8::from(fire_held) * FighterInput::PRIMARY_FIRE)
            | (u8::from(ultimate_held) * FighterInput::ULTIMATE);
        pending.active_device = ActiveInputDevice::KeyboardMouse;
    }
}

fn headless_automation_complete(automation: &HeadlessAutomation) -> bool {
    automation
        .simulation_ticks
        .is_some_and(|limit| automation.elapsed_ticks >= limit)
}

pub(super) fn headless_navigation_delta(position: Vec2, target: Vec2) -> Option<Vec2> {
    let direct = target - position;
    let delta = if direct.x.abs() > 300.0 {
        if position.y < 170.0 {
            Vec2::Y
        } else if position.y > 190.0 {
            Vec2::NEG_Y
        } else {
            Vec2::new(direct.x.signum(), 0.0)
        }
    } else {
        direct
    };
    (delta.is_finite() && delta.length_squared() > f32::EPSILON).then_some(delta)
}

pub(super) fn headless_ultimate_state(
    enabled: bool,
    ability: Option<&crate::builds::AbilityState>,
    loadout: Option<&crate::builds::ResolvedMatchLoadout>,
) -> (bool, bool, bool) {
    let ready = enabled
        && ability
            .is_some_and(|ability| matches!(ability.phase, crate::builds::AbilityPhase::Ready));
    let sentry =
        loadout.is_some_and(|loadout| loadout.ultimate.kind == crate::builds::UltimateKind::Sentry);
    let deployed = enabled
        && sentry
        && ability.is_some_and(|ability| {
            matches!(ability.phase, crate::builds::AbilityPhase::Deployed { .. })
        });
    (ready, ready && sentry, deployed)
}

pub(super) fn headless_combat_move_axis(
    fallback: Vec2,
    target_delta: Option<Vec2>,
    navigation_delta: Option<Vec2>,
    sentry_standoff: bool,
) -> Vec2 {
    navigation_delta.map_or(fallback, |delta| {
        let target_distance = target_delta.map_or(f32::INFINITY, Vec2::length);
        if sentry_standoff && target_distance < 320.0 {
            -delta.normalize()
        } else if sentry_standoff && target_distance <= 400.0 {
            Vec2::ZERO
        } else if target_distance > 130.0 {
            delta.normalize()
        } else if target_distance < 90.0 {
            -delta.normalize()
        } else {
            Vec2::ZERO
        }
    })
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

#[allow(
    clippy::type_complexity,
    reason = "the query declares this system's complete world view inline at its schedule boundary"
)]
fn controlled_lob_range(
    fighters: &Query<
        (&Position, Option<&crate::builds::ResolvedMatchLoadout>),
        (With<Fighter>, With<Controlled>),
    >,
) -> Option<f32> {
    fighters
        .iter()
        .next()?
        .1
        .map(|loadout| match loadout.primary_weapon.recipe.delivery {
            crate::combat::DeliveryMethod::Lobbed { distance, .. } => Some(distance),
            _ => None,
        })?
}

#[allow(
    clippy::type_complexity,
    reason = "the query declares this system's complete world view inline at its schedule boundary"
)]
pub(super) fn mouse_aim(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &Query<(&Camera, &GlobalTransform), With<ArenaCamera>>,
    fighters: &Query<
        (&Position, Option<&crate::builds::ResolvedMatchLoadout>),
        (With<Fighter>, With<Controlled>),
    >,
) -> Option<(Vec2, f32)> {
    let cursor = windows.iter().next()?.cursor_position()?;
    let (camera, camera_transform) = cameras.iter().next()?;
    let world = presentation_3d::cursor_ground_point(camera, camera_transform, cursor)?;
    let fighter = fighters.iter().next()?.0.0;
    let delta = world - fighter;
    (delta.is_finite() && delta.length_squared() > f32::EPSILON)
        .then(|| (delta.normalize(), delta.length()))
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn write_client_input(
    mut pending: ResMut<PendingLocalActions>,
    trace: Option<ResMut<LiveInputTrace>>,
    context: Res<ClientInputContext>,
    playable: Res<ClientPlayableGate>,
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
    let input = if !playable.0 || !matches!(*context, ClientInputContext::Gameplay) {
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

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
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

#[allow(
    clippy::type_complexity,
    reason = "the query declares this system's complete world view inline at its schedule boundary"
)]
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

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
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
