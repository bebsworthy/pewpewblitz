//! Client networking, compatibility status, and lightweight roster presentation.
#![allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines
)]

use crate::{
    VERSION,
    config::{ClientNetworkConfig, NetworkTransport},
    gameplay::GameplayPlugin,
    movement::{
        AvianNetworkPlugin, CAMERA_VERTICAL_SPAN, GreyboxArenaDefinition, InputTuning,
        trigger_pressed,
    },
    protocol::{
        ClientHello, Fighter, FighterInput, JoinOutcome, JoinRejection, NetworkEntityId, PlayerId,
        ProtocolFingerprint, ProtocolPlugin, SessionChannel,
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
    window::{PrimaryWindow, WindowCloseRequested},
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
    Authentication, InterpolationTimeline, LocalAddr, NetworkTimeline, PeerAddr,
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
struct HeadlessAutomation {
    move_axis: Vec2,
    aim_axis: Option<Vec2>,
    simulation_ticks: Option<u32>,
    elapsed_ticks: u32,
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

#[derive(Component)]
struct ArenaCamera;

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

    if keyboard_active
        || mouse_active
        || keyboard_scoreboard
        || mouse_buttons
            .is_some_and(|buttons| buttons.any_pressed([MouseButton::Left, MouseButton::Right]))
    {
        pending.active_device = ActiveInputDevice::KeyboardMouse;
    } else if let Some((entity, _, _, _)) = gamepad_sample {
        pending.active_device = ActiveInputDevice::Gamepad(entity);
    } else {
        pending.active_device = ActiveInputDevice::KeyboardMouse;
    }

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
    if gamepad_pause || pending.cancel_pressed {
        *context = match *context {
            ClientInputContext::Gameplay => ClientInputContext::Paused,
            ClientInputContext::Paused => ClientInputContext::Gameplay,
        };
        pending.latched_buttons = 0;
    }
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
    }
}

fn apply_headless_input(
    automation: Res<HeadlessAutomation>,
    mut pending: ResMut<PendingLocalActions>,
    statuses: Query<&ClientJoinStatus, With<Client>>,
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
    if automation.move_axis != Vec2::ZERO || automation.aim_axis.is_some() {
        if automation.elapsed_ticks == 0 {
            info!(
                move_axis = ?automation.move_axis,
                aim_axis = ?automation.aim_axis,
                "headless movement automation enabled"
            );
        }
        pending.move_axis = automation.move_axis;
        pending.aim_axis = automation.aim_axis;
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
    let input = if matches!(*context, ClientInputContext::Paused) {
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
    let border_color = Color::srgb(0.10, 0.13, 0.18);
    let cover_color = Color::srgb(0.22, 0.28, 0.36);
    let thickness = 48.0;
    let width = arena.max.x - arena.min.x;
    let height = arena.max.y - arena.min.y;
    let center = (arena.min + arena.max) / 2.0;
    for (position, size) in [
        (
            Vec2::new(arena.min.x - thickness / 2.0, center.y),
            Vec2::new(thickness, height + thickness * 2.0),
        ),
        (
            Vec2::new(arena.max.x + thickness / 2.0, center.y),
            Vec2::new(thickness, height + thickness * 2.0),
        ),
        (
            Vec2::new(center.x, arena.min.y - thickness / 2.0),
            Vec2::new(width, thickness),
        ),
        (
            Vec2::new(center.x, arena.max.y + thickness / 2.0),
            Vec2::new(width, thickness),
        ),
    ] {
        commands.spawn((
            ArenaVisual,
            Sprite::from_color(border_color, size),
            Transform::from_translation(position.extend(-10.0)),
        ));
    }
    for position in arena.cover_centers {
        commands.spawn((
            ArenaVisual,
            Sprite::from_color(cover_color, arena.cover_size),
            Transform::from_translation(position.extend(-5.0)),
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
        Node {
            position_type: PositionType::Absolute,
            left: px(16.0),
            bottom: px(16.0),
            ..default()
        },
    ));
    commands.spawn((
        InputStatusText,
        Text::new("Input: keyboard/mouse | gameplay"),
        TextFont::from_font_size(16.0),
        TextColor(Color::srgb(0.75, 0.9, 1.0)),
        Node {
            position_type: PositionType::Absolute,
            right: px(16.0),
            bottom: px(16.0),
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
    mut query: Query<(Entity, &PlayerId, Option<&mut Sprite>), (With<Fighter>, With<Remote>)>,
) {
    for (entity, player_id, sprite) in &mut query {
        if sprite.is_none() {
            let hue_index =
                u16::try_from(player_id.0.wrapping_mul(97) % 360).expect("hue index fits in u16");
            let hue = f32::from(hue_index) / 360.0;
            commands.entity(entity).insert((
                FighterVisual,
                Sprite::from_color(Color::hsl(hue, 0.78, 0.56), Vec2::new(48.0, 28.0)),
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
            .init_resource::<RosterLogState>()
            .init_resource::<ClientShutdown>()
            .init_resource::<PendingLocalActions>()
            .init_resource::<LiveInputTrace>()
            .init_resource::<HeadlessAutomation>()
            .init_resource::<InputDeviceActivity>()
            .init_resource::<ClientInputContext>()
            .init_resource::<GreyboxArenaDefinition>()
            .init_resource::<InputTuning>()
            .add_systems(Startup, spawn_client_connection)
            .add_systems(
                RunFixedMainLoop,
                (
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

fn send_client_hello(
    config: Res<ClientNetworkConfig>,
    fingerprint: Res<ProtocolFingerprint>,
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
            });
            status.phase = ClientJoinPhase::AwaitingOutcome;
            status.started_at = time.elapsed();
            info!("brawler client connected; awaiting compatibility outcome");
        }
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
    let mut app = App::new();
    app.insert_resource(config);
    if headless {
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
            crate::timing::SIMULATION_TICK,
        )))
        .add_plugins(StatesPlugin)
        .add_plugins(LogPlugin::default());
    } else {
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("Brawler Client {client_id}"),
                ..default()
            }),
            ..default()
        }));
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
