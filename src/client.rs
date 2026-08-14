//! Client networking, compatibility status, and lightweight roster presentation.
#![allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use crate::{
    VERSION,
    combat::{
        ClientCombatPlugin, CombatHudText, SelectingWeapon, WeaponCatalogResource,
        WeaponSelectionText, fighter_color,
    },
    config::{ClientNetworkConfig, NetworkTransport, RenderProfile},
    gameplay::GameplayPlugin,
    movement::{
        AvianNetworkPlugin, CAMERA_VERTICAL_SPAN, GreyboxArenaDefinition, InputTuning,
        trigger_pressed,
    },
    protocol::{
        ClientHello, Fighter, FighterInput, JoinOutcome, JoinRejection, NetworkEntityId, PlayerId,
        ProtocolFingerprint, ProtocolPlugin, SessionChannel, WeaponSelectionDecision,
        WeaponSelectionOutcome, WeaponSelectionRequest,
    },
};
use avian2d::prelude::{AngularVelocity, LinearVelocity, PhysicsSystems, Position, Rotation};
use bevy::camera::ScalingMode;
use bevy::{
    app::{RunFixedMainLoop, RunFixedMainLoopSystems, ScheduleRunnerPlugin},
    ecs::error::{FallbackErrorHandler, error},
    input::keyboard::Key,
    input::mouse::AccumulatedMouseMotion,
    log::LogPlugin,
    prelude::*,
    state::app::StatesPlugin,
    window::{PresentMode, PrimaryWindow, WindowCloseRequested},
    winit::{UpdateMode, WinitSettings},
};
use core::time::Duration;
use lightyear::prelude::InterpolationSystems;
use lightyear::prelude::client::input::InputSystems;
use lightyear::prelude::client::{Client, Connected, Connecting, Disconnected, Remote};
use lightyear::prelude::client::{
    ClientPlugins, Connect, Disconnect, NetcodeClient, NetcodeConfig,
};
use lightyear::prelude::input::native::{ActionState, InputMarker};
use lightyear::prelude::{
    Authentication, InterpolationTimeline, Link, LocalAddr, NetworkTimeline, PeerAddr,
};
use lightyear::prelude::{ConfirmedHistory, Controlled, Interpolated};
use lightyear::prelude::{MessageReceiver, MessageSender, PingManager, ReplicationReceiver, UdpIo};
use std::env;

/// User-visible client connection state. Lightyear lifecycle components remain the truth.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub enum ClientJoinPhase {
    Connecting,
    AwaitingOutcome,
    Active {
        player_id: PlayerId,
        network_entity_id: NetworkEntityId,
    },
    Rejected(JoinRejection),
    Disconnected,
}

#[derive(Component, Debug)]
pub struct ClientJoinStatus {
    pub phase: ClientJoinPhase,
    pub started_at: Duration,
    pub disconnect_requested: bool,
}

#[derive(Resource, Default, Debug)]
struct ClientShutdown {
    requested_exit: Option<AppExit>,
}

#[derive(Resource, Default, Debug, PartialEq, Eq)]
struct RosterLogState(Vec<(PlayerId, NetworkEntityId)>);

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(crate) struct HeadlessAutomation {
    move_axis: Vec2,
    aim_axis: Option<Vec2>,
    aim_at_dummy: bool,
    fire: bool,
    pub(crate) simulation_ticks: Option<u32>,
    pub(crate) elapsed_ticks: u32,
}

#[derive(Resource, Debug)]
struct WeaponSelectionState {
    next_request_id: u64,
    current_index: usize,
    last_sent: Option<u64>,
    last_outcome: Option<WeaponSelectionOutcome>,
    analog_ready: bool,
}

impl Default for WeaponSelectionState {
    fn default() -> Self {
        Self {
            next_request_id: 0,
            current_index: 0,
            last_sent: None,
            last_outcome: None,
            analog_ready: true,
        }
    }
}

impl FromWorld for HeadlessAutomation {
    fn from_world(world: &mut World) -> Self {
        let config = world.resource::<ClientNetworkConfig>();
        Self {
            move_axis: config
                .headless_move
                .map_or(Vec2::ZERO, |(x, y)| Vec2::new(f32::from(x), f32::from(y))),
            aim_axis: config
                .headless_aim
                .map(|(x, y)| Vec2::new(f32::from(x), f32::from(y))),
            aim_at_dummy: config.headless_aim_at_dummy,
            fire: config.headless_fire,
            simulation_ticks: config.headless_simulation_ticks,
            elapsed_ticks: 0,
        }
    }
}

/// Marker proving that the windowed presentation composition is installed.
#[derive(Default, Resource, Debug, PartialEq, Eq)]
pub struct ClientPresentation;

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ClientInputContext {
    #[default]
    Gameplay,
    Paused,
}

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ActiveInputDevice {
    #[default]
    KeyboardMouse,
    Gamepad(Entity),
}

/// Connected gamepads ordered by most recent meaningful activity.
#[derive(Resource, Default, Debug, PartialEq)]
pub struct InputDeviceActivity {
    recent_gamepads: Vec<Entity>,
    last_samples: Vec<(Entity, Vec2, Vec2, f32)>,
}

/// Render-frame state that is converted to exactly one native input value per fixed tick.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct PendingLocalActions {
    pub move_axis: Vec2,
    pub aim_axis: Option<Vec2>,
    pub held_buttons: u8,
    pub latched_buttons: u8,
    pub active_device: ActiveInputDevice,
    pub cancel_pressed: bool,
    pub scoreboard_held: bool,
    pub action_indicator: u16,
}

#[derive(Resource, Debug)]
struct LiveInputTrace {
    enabled: bool,
    last_sample: Option<(bool, [bool; 4], Vec2, ActiveInputDevice)>,
    last_aim: Option<Vec2>,
    last_write: Option<(Vec2, u8, usize)>,
    last_presented: Vec<(Entity, Vec2)>,
    last_history: Vec<(Entity, Vec2)>,
    last_sync: Option<bool>,
}

impl FromWorld for LiveInputTrace {
    fn from_world(_world: &mut World) -> Self {
        Self {
            enabled: env::var("BRAWLER_INPUT_TRACE").as_deref() == Ok("1"),
            last_sample: None,
            last_aim: None,
            last_write: None,
            last_presented: Vec::new(),
            last_history: Vec::new(),
            last_sync: None,
        }
    }
}

impl Default for PendingLocalActions {
    fn default() -> Self {
        Self {
            move_axis: Vec2::ZERO,
            aim_axis: None,
            held_buttons: 0,
            latched_buttons: 0,
            active_device: ActiveInputDevice::KeyboardMouse,
            cancel_pressed: false,
            scoreboard_held: false,
            action_indicator: 0,
        }
    }
}

const ACTION_PRIMARY_FIRE: u16 = 1 << 0;
const HEADLESS_FIRE_DURATION_TICKS: u32 = 480;
const ACTION_ACTIVE_ITEM: u16 = 1 << 1;
const ACTION_ULTIMATE: u16 = 1 << 2;
const ACTION_INTERACT: u16 = 1 << 3;
const ACTION_CANCEL: u16 = 1 << 8;
const ACTION_PAUSE: u16 = 1 << 9;
const ACTION_SCOREBOARD: u16 = 1 << 10;

fn select_active_gamepad(
    current: ActiveInputDevice,
    recent_gamepads: &[Entity],
    meaningful_gamepads: &[Entity],
    connected_gamepads: &[Entity],
) -> Option<Entity> {
    let current_connected = match current {
        ActiveInputDevice::Gamepad(id) => connected_gamepads.contains(&id).then_some(id),
        ActiveInputDevice::KeyboardMouse => None,
    };
    if meaningful_gamepads.is_empty() {
        current_connected.or_else(|| {
            recent_gamepads
                .iter()
                .rev()
                .find(|id| connected_gamepads.contains(id))
                .copied()
        })
    } else {
        recent_gamepads
            .iter()
            .rev()
            .find(|id| connected_gamepads.contains(id))
            .copied()
            .or_else(|| {
                meaningful_gamepads
                    .iter()
                    .rev()
                    .find(|id| connected_gamepads.contains(id))
                    .copied()
            })
    }
}

fn select_active_input_device(
    current: ActiveInputDevice,
    keyboard_mouse_active: bool,
    selected_gamepad: Option<Entity>,
    meaningful_gamepad: Option<Entity>,
) -> ActiveInputDevice {
    match current {
        ActiveInputDevice::KeyboardMouse => {
            meaningful_gamepad.map_or(ActiveInputDevice::KeyboardMouse, ActiveInputDevice::Gamepad)
        }
        ActiveInputDevice::Gamepad(_) if keyboard_mouse_active => ActiveInputDevice::KeyboardMouse,
        ActiveInputDevice::Gamepad(_) => {
            selected_gamepad.map_or(ActiveInputDevice::KeyboardMouse, ActiveInputDevice::Gamepad)
        }
    }
}

fn apply_pause_request(
    context: &mut ClientInputContext,
    pending: &mut PendingLocalActions,
    pause_pressed: bool,
) {
    if pause_pressed {
        *context = match *context {
            ClientInputContext::Gameplay => ClientInputContext::Paused,
            ClientInputContext::Paused => ClientInputContext::Gameplay,
        };
        pending.latched_buttons = 0;
    }
}

#[derive(Component)]
struct ArenaCamera;

/// Marker for the reproducible, windowed controller smoke path.
#[derive(Component)]
struct ControllerDemoGamepad;

#[derive(Component)]
struct PauseOverlay;

#[derive(Component)]
struct ControlsText;

#[derive(Component)]
struct InputStatusText;

#[derive(Component)]
struct ScoreboardOverlay;

#[derive(Component)]
struct FighterVisual;

#[derive(Component)]
struct ArenaVisual;

fn logical_key_pressed(keyboard: Option<&ButtonInput<Key>>, expected: &str) -> bool {
    keyboard.is_some_and(|keyboard| {
        keyboard.get_pressed().any(|key| {
            matches!(key, Key::Character(character) if character
                .as_str()
                .eq_ignore_ascii_case(expected))
        })
    })
}

/// Converts controller, keyboard, and mouse state to the shared action representation.
fn sample_local_input(
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
    fighters: Query<&Position, (With<Fighter>, With<Controlled>)>,
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

    let (move_axis, aim_axis, gamepad_buttons, gamepad_pause, gamepad_interact) =
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
                (
                    keyboard_move,
                    mouse_aim_direction(&windows, &cameras, &fighters),
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

fn apply_headless_input(
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
    let aim_axis = if automation.aim_at_dummy {
        controlled_position
            .zip(dummy_position)
            .map(|(position, target)| target - position)
            .filter(|delta| delta.is_finite() && delta.length_squared() > f32::EPSILON)
            .map(Vec2::normalize)
    } else {
        automation.aim_axis
    };
    let move_axis = if automation.aim_at_dummy && automation.move_axis != Vec2::ZERO {
        controlled_position
            .zip(dummy_position)
            .map(|(position, target)| target - position)
            .filter(|delta| delta.is_finite() && delta.length_squared() > f32::EPSILON)
            .map(Vec2::normalize)
            .unwrap_or(automation.move_axis)
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
        let fire_held = automation.fire && automation.elapsed_ticks < HEADLESS_FIRE_DURATION_TICKS;
        pending.held_buttons = if fire_held {
            FighterInput::PRIMARY_FIRE
        } else {
            0
        };
        pending.active_device = ActiveInputDevice::KeyboardMouse;
    }
}

fn advance_headless_automation(
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

fn mouse_aim_direction(
    windows: &Query<&Window, With<PrimaryWindow>>,
    cameras: &Query<(&Camera, &GlobalTransform), With<Camera2d>>,
    fighters: &Query<&Position, (With<Fighter>, With<Controlled>)>,
) -> Option<Vec2> {
    let cursor = windows.iter().next()?.cursor_position()?;
    let (camera, camera_transform) = cameras.iter().next()?;
    let world = camera.viewport_to_world_2d(camera_transform, cursor).ok()?;
    let fighter = fighters.iter().next()?.0;
    let delta = world - fighter;
    (delta.is_finite() && delta.length_squared() > f32::EPSILON).then(|| delta.normalize())
}

fn write_client_input(
    mut pending: ResMut<PendingLocalActions>,
    trace: Option<ResMut<LiveInputTrace>>,
    context: Res<ClientInputContext>,
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
    let input =
        if matches!(*context, ClientInputContext::Paused) || selecting.iter().next().is_some() {
            pending.latched_buttons = 0;
            FighterInput::default()
        } else {
            let buttons = pending.held_buttons | pending.latched_buttons;
            let input = FighterInput::from_axes(pending.move_axis, pending.aim_axis, buttons);
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

fn trace_client_interpolation_sync(
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

fn trace_client_interpolation_history(
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

fn add_controlled_input_marker(
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

/// Client-only greybox visuals, camera follow, and pause feedback.
pub struct MovementPresentationPlugin;

impl Plugin for MovementPresentationPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Startup,
            (
                spawn_client_arena,
                spawn_client_camera,
                spawn_pause_overlay,
                spawn_client_hud,
            )
                .chain(),
        )
        .add_systems(
            Update,
            (
                ensure_fighter_visuals,
                update_pause_overlay,
                update_client_hud,
            )
                .chain(),
        )
        .add_systems(
            PostUpdate,
            (
                write_interpolated_fighter_pose_to_transform,
                follow_controlled_camera,
            )
                .chain()
                .after(InterpolationSystems::Interpolate)
                .after(PhysicsSystems::Writeback)
                .before(TransformSystems::Propagate),
        );
    }
}

fn spawn_client_arena(mut commands: Commands, arena: Res<GreyboxArenaDefinition>) {
    let border_color = Color::srgb(0.08, 0.34, 0.58);
    let boundary_color = Color::srgb(0.40, 0.86, 1.0);
    let cover_color = Color::srgb(0.08, 0.34, 0.68);
    let cover_edge_color = Color::srgb(0.68, 0.92, 1.0);
    for (position, size) in arena.perimeter_visual_shapes() {
        commands.spawn((
            ArenaVisual,
            Sprite::from_color(border_color, size),
            Transform::from_translation(position.extend(-2.0)),
        ));
    }
    // The collision bodies remain outside the playable bounds. This in-bounds layer is
    // deliberately thick enough to survive a compact window and a camera at any arena edge;
    // only its inner edge is bright so the HUD remains readable when the camera reaches a wall.
    for (position, size) in arena.perimeter_visual_edge_shapes() {
        commands.spawn((
            ArenaVisual,
            Sprite::from_color(boundary_color, size),
            Transform::from_translation(position.extend(1.0)),
        ));
    }
    for (position, size) in arena.cover_shapes() {
        commands.spawn((
            ArenaVisual,
            Sprite::from_color(cover_color, size),
            // Keep blocker bodies above the arena markers/background so the complete cover,
            // rather than only its edge strip, remains visible in the window.
            Transform::from_translation(position.extend(2.0)),
        ));
        commands.spawn((
            ArenaVisual,
            Sprite::from_color(cover_edge_color, Vec2::new(size.x, 10.0)),
            Transform::from_translation(
                (position + Vec2::new(0.0, size.y / 2.0 - 5.0)).extend(3.0),
            ),
        ));
    }
}

fn spawn_client_camera(mut commands: Commands) {
    commands.spawn((
        Camera2d,
        ArenaCamera,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: CAMERA_VERTICAL_SPAN,
            },
            ..OrthographicProjection::default_2d()
        }),
        Transform::from_xyz(0.0, 0.0, 1000.0),
    ));
}

fn spawn_pause_overlay(mut commands: Commands) {
    commands
        .spawn((
            PauseOverlay,
            Node {
                position_type: PositionType::Absolute,
                left: percent(25.0),
                right: percent(25.0),
                top: percent(40.0),
                bottom: percent(40.0),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.03, 0.05, 0.88)),
            Visibility::Hidden,
        ))
        .with_children(|parent| {
            parent.spawn((
                Text::new("PAUSED\nEscape / Menu to resume"),
                TextFont::from_font_size(28.0),
                TextColor(Color::WHITE),
            ));
        });
}

fn spawn_client_hud(mut commands: Commands) {
    commands.spawn((
        ControlsText,
        Text::new("WASD / left stick: move   Mouse / right stick: aim\nQ: active item   E: ultimate   Space: interact   Tab: scoreboard   Esc: pause/cancel"),
        TextFont::from_font_size(16.0),
        TextColor(Color::WHITE),
        TextLayout::linebreak(LineBreak::WordBoundary),
        Node {
            position_type: PositionType::Absolute,
            left: px(16.0),
            bottom: px(16.0),
            width: percent(52.0),
            ..default()
        },
    ));
    commands.spawn((
        InputStatusText,
        Text::new("Input: keyboard/mouse | gameplay"),
        TextFont::from_font_size(16.0),
        TextColor(Color::srgb(0.75, 0.9, 1.0)),
        TextLayout::new(Justify::Right, LineBreak::WordBoundary),
        Node {
            position_type: PositionType::Absolute,
            right: px(16.0),
            bottom: px(16.0),
            width: percent(42.0),
            ..default()
        },
    ));
    commands.spawn((
        CombatHudText,
        Text::new("Health ---   Pulse --/--   READY"),
        TextFont::from_font_size(20.0),
        TextColor(Color::srgb(1.0, 0.85, 0.35)),
        Node {
            position_type: PositionType::Absolute,
            left: px(16.0),
            top: px(16.0),
            ..default()
        },
    ));
    commands.spawn((
        WeaponSelectionText,
        Text::new("Select weapon: A/D or arrows • Space / South to confirm\nPulse Sidearm"),
        TextFont::from_font_size(22.0),
        TextColor(Color::srgb(0.85, 0.95, 1.0)),
        Visibility::Inherited,
        Node {
            position_type: PositionType::Absolute,
            left: percent(25.0),
            right: percent(25.0),
            top: percent(18.0),
            ..default()
        },
    ));
    commands.spawn((
        ScoreboardOverlay,
        Text::new("SCOREBOARD\nLocal fighter roster is authoritative"),
        TextFont::from_font_size(22.0),
        TextColor(Color::WHITE),
        Node {
            position_type: PositionType::Absolute,
            right: px(24.0),
            top: px(24.0),
            ..default()
        },
        Visibility::Hidden,
    ));
}

fn ensure_fighter_visuals(
    mut commands: Commands,
    mut query: Query<
        (Entity, &PlayerId, &NetworkEntityId, Option<&mut Sprite>),
        (With<Fighter>, With<Remote>),
    >,
) {
    for (entity, player_id, network_id, sprite) in &mut query {
        if sprite.is_none() {
            if network_id.0 == 0 {
                commands.entity(entity).insert((
                    FighterVisual,
                    Sprite::from_color(Color::srgb(0.95, 0.25, 0.1), Vec2::new(52.0, 32.0)),
                ));
                continue;
            }
            commands.entity(entity).insert((
                FighterVisual,
                Sprite::from_color(fighter_color(*player_id), Vec2::new(48.0, 28.0)),
            ));
        }
    }
}

/// Keep render-only replicated fighters visually aligned with Lightyear's interpolated pose.
///
/// The client intentionally does not replicate a server `RigidBody`, so Avian's normal
/// `RigidBody -> Transform` writeback is not sufficient for every interpolated fighter.  The
/// replicated Position/Rotation pair is the canonical presentation pose in Position mode.
fn write_interpolated_fighter_pose_to_transform(
    trace: Option<ResMut<LiveInputTrace>>,
    fighters: Query<(Entity, &Position, &Rotation, &mut Transform), (With<Fighter>, With<Remote>)>,
) {
    let mut trace = trace.filter(|trace| trace.enabled);
    for (entity, position, rotation, mut transform) in fighters {
        transform.translation.x = position.0.x;
        transform.translation.y = position.0.y;
        transform.rotation = Quat::from_rotation_z(rotation.as_radians());
        if let Some(trace) = trace.as_mut() {
            let last_position = trace
                .last_presented
                .iter()
                .find(|(candidate, _)| *candidate == entity)
                .map(|(_, position)| *position);
            if last_position.is_none_or(|last| last.distance(position.0) >= 32.0) {
                info!(
                    ?entity,
                    replicated_position = ?position.0,
                    visible_translation = ?transform.translation.truncate(),
                    "live client presented fighter pose"
                );
                trace
                    .last_presented
                    .retain(|(candidate, _)| *candidate != entity);
                trace.last_presented.push((entity, position.0));
            }
        }
    }
}

fn follow_controlled_camera(
    arena: Res<GreyboxArenaDefinition>,
    fighters: Query<&Position, (With<Fighter>, With<Controlled>, Without<ArenaCamera>)>,
    mut cameras: Query<(&Camera, &mut Transform), With<ArenaCamera>>,
) {
    let Some(position) = fighters.iter().next().map(|position| position.0) else {
        return;
    };
    for (camera, mut transform) in &mut cameras {
        let viewport = camera
            .logical_viewport_size()
            .filter(|size| size.x > 0.0 && size.y > 0.0)
            .unwrap_or(Vec2::new(16.0, 9.0));
        let center = clamp_camera_center(position, *arena, viewport);
        transform.translation.x = center.x;
        transform.translation.y = center.y;
    }
}

fn clamp_camera_center(position: Vec2, arena: GreyboxArenaDefinition, viewport: Vec2) -> Vec2 {
    let aspect = if viewport.y > 0.0 {
        viewport.x / viewport.y
    } else {
        16.0 / 9.0
    };
    let half_height = CAMERA_VERTICAL_SPAN / 2.0;
    let half_width = half_height * aspect.max(0.0);
    let min = arena.min + Vec2::new(half_width, half_height);
    let max = arena.max - Vec2::new(half_width, half_height);
    Vec2::new(
        if min.x > max.x {
            f32::midpoint(arena.min.x, arena.max.x)
        } else {
            position.x.clamp(min.x, max.x)
        },
        if min.y > max.y {
            f32::midpoint(arena.min.y, arena.max.y)
        } else {
            position.y.clamp(min.y, max.y)
        },
    )
}

fn update_pause_overlay(
    context: Res<ClientInputContext>,
    mut overlays: Query<&mut Visibility, With<PauseOverlay>>,
) {
    for mut visibility in &mut overlays {
        *visibility = if matches!(*context, ClientInputContext::Paused) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

fn update_client_hud(
    pending: Res<PendingLocalActions>,
    context: Res<ClientInputContext>,
    connection: Query<&ClientJoinStatus, With<Client>>,
    mut status: Query<&mut Text, With<InputStatusText>>,
    mut scoreboard: Query<&mut Visibility, With<ScoreboardOverlay>>,
) {
    if pending.is_changed() || context.is_changed() {
        let device = match pending.active_device {
            ActiveInputDevice::KeyboardMouse => "keyboard/mouse",
            ActiveInputDevice::Gamepad(_) => "gamepad",
        };
        let mode = if matches!(*context, ClientInputContext::Paused) {
            "paused"
        } else {
            "gameplay"
        };
        let connection = connection
            .iter()
            .next()
            .map_or("offline", |status| match status.phase {
                ClientJoinPhase::Connecting => "connecting",
                ClientJoinPhase::AwaitingOutcome => "handshaking",
                ClientJoinPhase::Active { .. } => "connected",
                ClientJoinPhase::Rejected(_) => "rejected",
                ClientJoinPhase::Disconnected => "disconnected",
            });
        let mut actions = String::new();
        for (bit, label) in [
            (ACTION_PRIMARY_FIRE, "fire"),
            (ACTION_ACTIVE_ITEM, "item"),
            (ACTION_ULTIMATE, "ultimate"),
            (ACTION_INTERACT, "interact"),
            (ACTION_CANCEL, "cancel"),
            (ACTION_PAUSE, "pause"),
            (ACTION_SCOREBOARD, "scoreboard"),
        ] {
            if pending.action_indicator & bit != 0 {
                if !actions.is_empty() {
                    actions.push(',');
                }
                actions.push_str(label);
            }
        }
        if actions.is_empty() {
            actions.push_str("none");
        }
        for mut text in &mut status {
            **text =
                format!("Connection: {connection}\nInput: {device} | {mode}\nActions: {actions}");
        }
    }
    for mut visibility in &mut scoreboard {
        *visibility = if pending.scoreboard_held {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
    }
}

/// Adds client-only window behavior and startup diagnostics.
pub struct ClientPresentationPlugin;

impl Plugin for ClientPresentationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientPresentation>()
            .add_systems(Update, exit_on_close_requested)
            .add_plugins(MovementPresentationPlugin);
    }
}

fn exit_on_close_requested(
    mut close_requests: MessageReader<WindowCloseRequested>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if close_requests.read().next().is_some() {
        app_exit.write(AppExit::Success);
    }
}

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

fn spawn_client_connection(
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

fn spawn_controller_demo_gamepad(mut commands: Commands, config: Res<ClientNetworkConfig>) {
    if config.windowed_controller_demo.is_some() {
        commands.spawn((Gamepad::default(), ControllerDemoGamepad));
        info!("windowed synthetic controller demo enabled");
    }
}

/// Keep the synthetic controller aimed at the server-owned neutral dummy while preserving the
/// normal gamepad sampling path. This is only a visual/input smoke aid; it is not gameplay logic.
fn update_controller_demo_gamepad(
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

fn send_client_hello(
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

fn process_weapon_selection_outcomes(
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

fn send_weapon_selection_request(
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

fn crossbeam_transport(config: &ClientNetworkConfig) -> bool {
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

fn update_weapon_selection_overlay(
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
        crate::combat::DeliveryMethod::Lobbed { distance, .. } => {
            format!("fixed landing {distance:.0}")
        }
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

fn process_join_outcome(
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

fn disconnect_rejected_client(
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

fn observe_client_lifecycle(
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

fn log_replicated_roster(
    config: Res<ClientNetworkConfig>,
    automation: Res<HeadlessAutomation>,
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
    {
        app_exit.write(AppExit::Success);
    }
}

fn enforce_client_timeout(
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

fn forward_app_exit_to_client_disconnect(
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

fn finish_client_shutdown(
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

/// Build the windowed or headless client application.
pub fn build_app_with_config(config: ClientNetworkConfig) -> App {
    let headless = config.headless;
    let client_id = config.client_id;
    let render_profile = config.render_profile;
    let mut app = App::new();
    app.insert_resource(config);
    if headless {
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
            crate::timing::SIMULATION_TICK,
        )))
        .add_plugins(StatesPlugin)
        .add_plugins(LogPlugin::default());
    } else {
        let (present_mode, winit_settings) = render_profile_settings(render_profile);
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("Brawler Client {client_id}"),
                present_mode,
                ..default()
            }),
            ..default()
        }))
        .insert_resource(winit_settings);
        info!(
            profile = render_profile.name(),
            "windowed render profile selected"
        );
    }
    app.add_plugins(ClientPlugins {
        tick_duration: crate::timing::SIMULATION_TICK,
    })
    .add_plugins((
        GameplayPlugin,
        ProtocolPlugin,
        AvianNetworkPlugin,
        ClientNetworkPlugin,
    ));
    if !headless {
        app.add_plugins(ClientPresentationPlugin);
    }
    app
}

const RENDER_30_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 30);
const RENDER_60_INTERVAL: Duration = Duration::from_nanos(1_000_000_000 / 60);

fn render_profile_settings(profile: RenderProfile) -> (PresentMode, WinitSettings) {
    match profile {
        RenderProfile::Native => (PresentMode::Fifo, WinitSettings::game()),
        RenderProfile::ThirtyFps => (
            PresentMode::Fifo,
            WinitSettings {
                focused_mode: UpdateMode::reactive(RENDER_30_INTERVAL),
                unfocused_mode: UpdateMode::reactive_low_power(RENDER_30_INTERVAL),
            },
        ),
        RenderProfile::SixtyFps => (
            PresentMode::Fifo,
            WinitSettings {
                focused_mode: UpdateMode::reactive(RENDER_60_INTERVAL),
                unfocused_mode: UpdateMode::reactive_low_power(RENDER_60_INTERVAL),
            },
        ),
        RenderProfile::HighRefresh => (PresentMode::AutoNoVsync, WinitSettings::continuous()),
    }
}

/// Build the default client application.
pub fn build_app() -> App {
    build_app_with_config(ClientNetworkConfig::new(1))
}

#[cfg(feature = "network-test")]
pub fn spawn_crossbeam_client(
    world: &mut World,
    config: ClientNetworkConfig,
    io: lightyear::crossbeam::CrossbeamIo,
) -> Entity {
    let auth = Authentication::Manual {
        server_addr: config.server_addr,
        client_id: config.client_id,
        private_key: crate::protocol::DEVELOPMENT_PRIVATE_KEY,
        protocol_id: config.network_protocol_id,
    };
    let entity = world
        .spawn((
            ClientJoinStatus {
                phase: ClientJoinPhase::Connecting,
                started_at: Duration::ZERO,
                disconnect_requested: false,
            },
            PingManager::default(),
            ReplicationReceiver,
            Link::default().with_conditioner(config.impairment_profile.receive_conditioner()),
            NetcodeClient::new(auth, NetcodeConfig::default()).expect("test netcode client"),
            io,
        ))
        .id();
    world.flush();
    world.trigger(Connect { entity });
    entity
}

#[cfg(test)]
mod tests {
    use super::*;

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
            .init_resource::<ClientInputContext>()
            .init_resource::<InputDeviceActivity>()
            .init_resource::<InputTuning>()
            .add_systems(Update, sample_local_input);
        let gamepad_entity = app.world_mut().spawn(gamepad).id();

        app.update();

        let pending = app.world().resource::<PendingLocalActions>();
        assert_eq!(
            pending.active_device,
            ActiveInputDevice::Gamepad(gamepad_entity)
        );
        assert_eq!(pending.move_axis, Vec2::new(0.75, 0.0));
        assert_eq!(pending.aim_axis, Some(Vec2::new(0.0, -0.8)));
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
            (transform.rotation.to_euler(EulerRot::ZYX).0 - core::f32::consts::FRAC_PI_2).abs()
                < 0.001
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
            select_active_input_device(
                ActiveInputDevice::KeyboardMouse,
                false,
                Some(gamepad),
                None,
            ),
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
        let arena = GreyboxArenaDefinition::default();
        let landscape = clamp_camera_center(Vec2::new(900.0, 0.0), arena, Vec2::new(16.0, 9.0));
        assert!((landscape.x - 160.0).abs() < 0.001);

        let portrait = clamp_camera_center(Vec2::new(900.0, 0.0), arena, Vec2::new(9.0, 16.0));
        assert!(portrait.x > landscape.x);
        assert!(portrait.x <= arena.max.x);

        let oversized =
            clamp_camera_center(Vec2::new(900.0, 400.0), arena, Vec2::new(4000.0, 100.0));
        assert!(oversized.x.abs() < 0.001);
        assert!((oversized.y - 140.0).abs() < 0.001);
    }

    #[test]
    fn client_arena_spawns_visible_geometry_for_every_blocker() {
        let arena = GreyboxArenaDefinition::default();
        let expected_visuals =
            arena.perimeter_visual_shapes().len() * 2 + arena.cover_shapes().len() * 2;
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .insert_resource(arena)
            .add_systems(Startup, spawn_client_arena);

        app.update();

        let mut visuals = app
            .world_mut()
            .query_filtered::<Entity, With<ArenaVisual>>();
        assert_eq!(visuals.iter(app.world()).count(), expected_visuals);
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
}
