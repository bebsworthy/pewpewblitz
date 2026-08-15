//! Client application composition across input, presentation, and network-session concerns.
#![allow(clippy::needless_pass_by_value, clippy::type_complexity)]

use crate::{
    VERSION,
    combat::{
        AuthoritativeTick, BuildSelectionText, ClientCombatEvidenceStatus, ClientCombatPlugin,
        CombatHudText, ResolvedWeapon, SelectingBuild,
    },
    config::{ClientNetworkConfig, NetworkTransport, RenderProfile},
    gameplay::GameplayPlugin,
    matchplay::{
        MatchParticipant, MatchPhase, MatchRoot, MatchState, RespawnState, SpawnProtection,
    },
    movement::{
        AvianNetworkPlugin, CAMERA_VERTICAL_SPAN, InputTuning, committed_aim, radial_deadzone,
        trigger_pressed,
    },
    protocol::{
        BuildSelection, BuildSelectionDecision, BuildSelectionOutcome, BuildSelectionRequest,
        ClientHello, Fighter, FighterInput, JoinOutcome, JoinRejection, MatchCommand,
        MatchCommandOutcome, MatchCommandRequest, NetworkEntityId, PlayerId, ProtocolFingerprint,
        ProtocolPlugin, SessionChannel,
    },
};
use avian2d::prelude::{AngularVelocity, LinearVelocity, PhysicsSystems, Position, Rotation};
use bevy::camera::{ScalingMode, visibility::RenderLayers};
use bevy::{
    app::{RunFixedMainLoop, RunFixedMainLoopSystems, ScheduleRunnerPlugin},
    ecs::error::{FallbackErrorHandler, error},
    input::keyboard::Key,
    input::mouse::AccumulatedMouseMotion,
    log::LogPlugin,
    prelude::*,
    state::app::StatesPlugin,
    window::{PresentMode, PrimaryWindow, WindowCloseRequested, WindowResolution},
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

mod assets;
mod audio;
mod hud;
mod input;
mod presentation;
mod session;
pub(crate) use assets::ClientAssetHandles;
#[allow(clippy::wildcard_imports)]
use input::*;
pub use presentation::{ClientPresentationPlugin, MovementPresentationPlugin};
#[cfg(test)]
use presentation::{
    clamp_camera_center, update_client_hud, write_interpolated_fighter_pose_to_transform,
};
pub use session::ClientNetworkPlugin;
#[cfg(test)]
#[allow(clippy::wildcard_imports)]
use session::*;

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
    ultimate: bool,
    pub(crate) simulation_ticks: Option<u32>,
    pub(crate) elapsed_ticks: u32,
}

#[derive(Resource, Debug)]
struct BuildSelectionState {
    next_request_id: u64,
    current_index: usize,
    last_sent: Option<u64>,
    last_outcome: Option<BuildSelectionOutcome>,
    last_match_id: Option<crate::matchplay::MatchId>,
    analog_x_ready: bool,
    analog_y_ready: bool,
    custom_field: usize,
    custom_recipe: crate::builds::BrawlerBuildRecipe,
}

#[derive(Resource, Debug, Default)]
struct MatchCommandState {
    next_request_id: u64,
    sent_for_phase: Option<(crate::matchplay::MatchId, MatchPhase)>,
    last_outcome: Option<MatchCommandOutcome>,
}

impl Default for BuildSelectionState {
    fn default() -> Self {
        Self {
            next_request_id: 0,
            current_index: 0,
            last_sent: None,
            last_outcome: None,
            last_match_id: None,
            analog_x_ready: true,
            analog_y_ready: true,
            custom_field: 0,
            custom_recipe: crate::builds::BrawlerBuildRecipe {
                weapon: crate::builds::WeaponChoice::CustomPulse {
                    power: crate::builds::PulsePower::Balanced,
                    reach: crate::builds::PulseReach::Standard,
                    magazine: crate::builds::PulseMagazine::Standard,
                },
                ultimate: crate::builds::UltimateDefinitionId(1),
                passives: [
                    crate::builds::PassiveDefinitionId(1),
                    crate::builds::PassiveDefinitionId(6),
                ],
            },
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
            ultimate: config.headless_ultimate,
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

/// Local presentation/readiness gate. Headless automation has no asset requirement.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ClientPlayableGate(pub bool);

impl FromWorld for ClientPlayableGate {
    fn from_world(world: &mut World) -> Self {
        Self(world.resource::<ClientNetworkConfig>().headless)
    }
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
    pub aim_distance: Option<f32>,
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
            aim_distance: None,
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
const HEADLESS_FIRE_DURATION_TICKS: u32 = 6_000;
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

/// Build the windowed or headless client application.
pub fn build_app_with_config(config: ClientNetworkConfig) -> App {
    let headless = config.headless;
    let client_id = config.client_id;
    let render_profile = config.render_profile;
    let window_size = config.window_size;
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
        app.add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: format!("{}/assets", env!("CARGO_MANIFEST_DIR")),
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: format!("Brawler Client {client_id}"),
                        present_mode,
                        resolution: window_size.map_or_else(Default::default, |(width, height)| {
                            WindowResolution::new(u32::from(width), u32::from(height))
                        }),
                        ..default()
                    }),
                    ..default()
                }),
        )
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
mod tests;
