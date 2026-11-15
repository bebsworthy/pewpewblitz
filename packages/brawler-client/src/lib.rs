//! Client application composition across input, presentation, and network-session concerns.
//!
//! BRL-0170: this package is the client half of the former single `brawler` package. It builds on
//! the shared gameplay crate's `client` feature and never on `server`.

#[cfg(feature = "automation")]
pub use automation::{
    ClientAutomationConfig, RenderMeasurementConfig, ScreenshotSchedule, WindowedCombatDemo,
    WindowedControllerDemo,
};
#[cfg(feature = "automation")]
pub use brawler::config::HeadlessAutomation;
pub use brawler::map::ClientPlayableGate;

/// Repository root that owns `assets/` and `asset_src/`; this package lives two levels below it.
pub(crate) const REPOSITORY_ROOT: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../..");

use avian2d::prelude::{AngularVelocity, LinearVelocity, PhysicsSystems, Position, Rotation};
use bevy::{
    app::{RunFixedMainLoop, RunFixedMainLoopSystems, ScheduleRunnerPlugin},
    ecs::error::{FallbackErrorHandler, error},
    input::keyboard::Key,
    input::mouse::AccumulatedMouseMotion,
    prelude::*,
    state::app::StatesPlugin,
    window::{PresentMode, PrimaryWindow, WindowCloseRequested, WindowResolution},
    winit::{UpdateMode, WinitSettings},
};
use brawler::{
    VERSION,
    combat::{AuthoritativeTick, ClientCombatEvidenceStatus, ClientCombatPlugin},
    config::{ClientNetworkConfig, NetworkTransport, RenderProfile},
    content::GameplayContentPlugin,
    gameplay::GameplayPlugin,
    matchplay::{MatchParticipant, MatchPhase, MatchRoot, MatchState},
    movement::AvianNetworkPlugin,
    protocol::{
        Fighter, FighterInput, LobbyHello, LobbyJoinOutcome, LobbyServerIdentity, MatchCommand,
        MatchCommandOutcome, MatchCommandRequest, MatchHello, MatchJoinOutcome, MatchJoinRejection,
        MatchLoadingClientAction, MatchLoadingClientMessage, MatchLoadingServerMessage,
        MatchLoadingStatus, MatchRouteGrant, NetworkEntityId, PlayerId, ProtocolFingerprint,
        ProtocolPlugin, SessionChannel,
    },
};
use brawler_routing::{ROUTED_LINK_MTU, RequestId};
use core::time::Duration;
use lightyear::prelude::InterpolationSystems;
use lightyear::prelude::client::input::InputSystems;
use lightyear::prelude::client::{Client, Connected, Connecting, Disconnected, Remote};
use lightyear::prelude::client::{
    ClientPlugins, Connect, Disconnect, NetcodeClient, NetcodeConfig,
};
use lightyear::prelude::input::native::{ActionState, InputMarker};
use lightyear::prelude::{
    Authentication, InterpolationTimeline, Link, LinkMtu, LocalAddr, NetworkTimeline, PeerAddr,
    Unlink, UnlinkReason, Unlinked,
};
use lightyear::prelude::{ConfirmedHistory, Controlled, Interpolated};
use lightyear::prelude::{MessageReceiver, MessageSender, PingManager, ReplicationReceiver, UdpIo};
use std::env;

const DEFAULT_GAMEPLAY_WINDOW_WIDTH: u32 = 1_591;
const DEFAULT_GAMEPLAY_WINDOW_HEIGHT: u32 = 720;
const DEFAULT_GAMEPLAY_VIEWPORT: Vec2 = Vec2::new(1_591.0, 720.0);
const DEFAULT_GAMEPLAY_WINDOW_ASPECT: f32 = 1_591.0 / 720.0;

mod assets;
mod audio;
#[cfg(feature = "automation")]
mod automation;
mod back;
mod connection_persistence;
mod dashboard;
mod diagnostics_overlay;
mod evidence_capture;
mod flow;
mod hud;
mod input;
pub mod local_attack;
mod map_maker;
mod platform_dirs;
pub mod prediction;
mod presentation;
pub(crate) mod presentation_3d;
mod profile;
mod queue;
mod routed_udp;
mod server_select;
mod session;
mod settings;
mod shell;
mod targeting;
mod touch;
mod ui;
pub(crate) mod vfx;
#[cfg(feature = "automation")]
mod vfx_lab;
pub(crate) use assets::ClientAssetHandles;
pub use back::{BackInput, BackInputSystems};
pub use flow::{
    CancelMatchStartConfirmation, ClientFlow, ClientFlowPlugin, ClientOverlay, FlowError,
    FlowErrorAction, FlowErrorKind, SelectedGameType, SessionPurpose,
};
#[allow(clippy::wildcard_imports)]
use input::*;
pub use map_maker::{MapDraftDocument, MapDraftId, MapDraftLibrary, MapEditorSession};
pub use presentation::ClientPresentationPlugin;
#[cfg(test)]
use presentation::{clamp_camera_center, update_client_hud};
pub use profile::{ClientProfileModel, ClientProfilePlugin};
pub use queue::{
    ClientMatchLoadingModel, ClientPracticeModel, ClientQueueModel, ClientQueuePlugin,
    PendingQueueCommand,
};
pub use routed_udp::{RoutedUdpIo, RoutedUdpPlugin};
pub use server_select::{LogicalServerAddress, ServerAddressHost, parse_server_address};
pub use session::ClientNetworkPlugin;
#[cfg(test)]
#[allow(clippy::wildcard_imports)]
use session::*;
pub use settings::persistence::ClientShellSettings;
pub use settings::ui::{InputSettingsField, InputSettingsSelection, compose_input_settings_lines};
use settings::ui::{adjust_input_settings_from_pause_keys, update_input_settings_overlay};
pub use settings::{
    CalibrationField, ClientInputSettings, GamepadAction, GamepadBindings, KeyboardAction,
    KeyboardBindings, MAX_CALIBRATION, MIN_TRIGGER_HYSTERESIS,
};
pub use shell::ClientShellPlugin;
#[cfg(feature = "automation")]
pub use ui::gallery::build_ui_gallery_app;
#[cfg(feature = "automation")]
pub use vfx_lab::{VfxLabAccent, VfxLabConfig, VfxLabRelationship, build_vfx_lab_app};

/// Installs client-side gameplay state reconstructed from authoritative replication and messages.
/// Connection lifecycle and transport remain owned by [`ClientNetworkPlugin`].
pub struct ClientReplicatedGameplayPlugin;

impl Plugin for ClientReplicatedGameplayPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(ClientCombatPlugin);
        // Legacy direct-UDP combat evidence is automation-only (BRL-3p70).
        #[cfg(feature = "automation")]
        app.add_plugins(brawler::combat::ClientCombatEvidencePlugin);
        app.add_plugins((
            brawler::map::ClientMapPlugin,
            brawler::map::MapPresentationPlugin,
            ClientQueuePlugin,
            ClientProfilePlugin,
            prediction::OwnerPredictionPlugin,
            local_attack::LocalAttackPlugin,
        ));
    }
}

/// Explicit client-side state for the sequential routed transport lifecycle.
///
/// A routed client owns exactly one Lightyear `Client` entity at a time.  The lobby and match
/// sessions are intentionally separate Netcode sessions: a grant moves this state to
/// `AwaitingLobbyUnlink`, and only after the old entity has been deferred-unlinked and despawned
/// does the session system create a fresh match entity.  The same sequence is used in reverse for
/// an intentional return to the lobby.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RoutedClientPhase {
    #[default]
    Disabled,
    Lobby,
    AwaitingLobbyUnlink,
    AwaitingLobbyRetryUnlink,
    Match,
    AwaitingMatchUnlink,
}

#[derive(Resource, Debug, Default)]
pub struct RoutedClientLifecycle {
    pub phase: RoutedClientPhase,
    /// Request accepted on the current authenticated lobby session. M01 has no lobby session ID
    /// in the client-visible grant yet, so the request ID is the strongest available binding.
    pub current_request_id: Option<RequestId>,
    /// The one grant accepted for the current lobby request. Its capability is redacted by the
    /// protocol and routing types when this resource is logged or formatted for diagnostics.
    pub accepted_grant: Option<MatchRouteGrant>,
    /// Monotonic local generation. Every fresh lobby or match Netcode entity gets a new value.
    pub generation: u64,
    /// Automatic rejoin of a still-running match (BRL-yyeg), announced by the lobby before its
    /// welcome. Cleared when the rejoined match session is accepted, when the lobby reports the
    /// rejoin unavailable, or when a fresh lobby session starts.
    pub rejoin: RoutedRejoin,
    /// Set once when a rejoined match session is accepted; the product flow consumes it.
    pub rejoined: bool,
    /// The player confirmed Leave Match (BRL-8y1f). The session tells the match worker it is
    /// leaving and only then disconnects, so the worker releases the seat instead of holding it.
    pub leave_requested: bool,
}

/// Progress of one automatic match rejoin.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum RoutedRejoin {
    #[default]
    None,
    /// The lobby is obtaining a fresh grant for the held seat.
    AwaitingGrant,
    /// The grant was accepted; the match session is connecting.
    Connecting,
}

impl RoutedClientLifecycle {
    /// Start a fresh lobby request/session. This is also the recovery path after any transition
    /// failure; no match session is resumed.
    pub fn start_lobby(&mut self) {
        self.generation = self.generation.saturating_add(1).max(1);
        self.phase = RoutedClientPhase::Lobby;
        // The lobby owns RequestId. The client records it only after receiving the authenticated
        // grant; generating a local value here would not bind to the server's request namespace.
        self.current_request_id = None;
        self.accepted_grant = None;
        self.rejoin = RoutedRejoin::None;
        self.leave_requested = false;
    }

    /// Accept exactly one authenticated grant for the current lobby request.
    pub fn accept_grant(&mut self, grant: MatchRouteGrant) -> bool {
        if self.phase != RoutedClientPhase::Lobby || self.accepted_grant.is_some() {
            return false;
        }
        self.accepted_grant = Some(grant);
        self.current_request_id = Some(grant.request_id);
        self.phase = RoutedClientPhase::AwaitingLobbyUnlink;
        if self.rejoin == RoutedRejoin::AwaitingGrant {
            self.rejoin = RoutedRejoin::Connecting;
        }
        true
    }

    /// The current lobby session announced an automatic rejoin of a held match seat.
    pub fn begin_rejoin(&mut self) -> bool {
        if self.phase != RoutedClientPhase::Lobby || self.accepted_grant.is_some() {
            return false;
        }
        self.rejoin = RoutedRejoin::AwaitingGrant;
        true
    }

    /// The lobby reported that no rejoin grant will follow.
    pub fn abandon_rejoin(&mut self) {
        if self.rejoin == RoutedRejoin::AwaitingGrant {
            self.rejoin = RoutedRejoin::None;
        }
    }

    /// The rejoined match session was accepted by the match worker.
    pub fn complete_rejoin(&mut self) -> bool {
        if self.rejoin != RoutedRejoin::Connecting || self.phase != RoutedClientPhase::Match {
            return false;
        }
        self.rejoin = RoutedRejoin::None;
        self.rejoined = true;
        true
    }

    #[must_use]
    pub fn rejoin_in_progress(&self) -> bool {
        self.rejoin != RoutedRejoin::None
    }

    /// The player chose to leave the running match (BRL-8y1f). Unlike a lost connection this
    /// must release the seat, so the session first sends `MatchCommand::LeaveMatch` and returns to
    /// the lobby when the worker answers (or after a bounded wait).
    pub fn request_leave_match(&mut self) -> bool {
        if self.phase != RoutedClientPhase::Match {
            return false;
        }
        self.leave_requested = true;
        true
    }

    /// Request an intentional match-to-lobby transition. The caller still needs the session
    /// system to issue `Disconnect`; this separation makes the deferred unlink boundary explicit.
    pub fn request_return_to_lobby(&mut self) -> bool {
        if self.phase != RoutedClientPhase::Match {
            return false;
        }
        self.phase = RoutedClientPhase::AwaitingMatchUnlink;
        true
    }

    fn begin_match(&mut self) -> Option<MatchRouteGrant> {
        if self.phase != RoutedClientPhase::AwaitingLobbyUnlink {
            return None;
        }
        self.phase = RoutedClientPhase::Match;
        self.accepted_grant
    }

    fn begin_lobby_after_match(&mut self) {
        self.start_lobby();
    }
}

/// Identifies which side of the sequential routed lifecycle owns a fresh Lightyear entity.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RoutedClientSession {
    pub generation: u64,
    pub kind: RoutedClientSessionKind,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RoutedClientSessionKind {
    Lobby,
    Match,
}

/// Bounded client-local presentation copied from replicated match authority before unlink.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClientMatchResultContext {
    pub result: brawler::matchplay::MatchResult,
    pub local_team: Option<brawler::combat::TeamId>,
    pub game_type_id: Option<brawler::lobby::GameTypeId>,
    pub game_name: Option<String>,
    pub(crate) final_score: Option<hud::ModeScoreView>,
    pub(crate) wipeout: Option<brawler::matchplay::WipeoutState>,
    pub(crate) objective: Option<brawler::matchplay::ObjectiveResult>,
    pub(crate) local_player: Option<PlayerId>,
}

#[derive(Resource, Debug, Default, PartialEq, Eq)]
pub struct ClientMatchResultState {
    pub context: Option<ClientMatchResultContext>,
    pub(crate) last_accepted_game_type_id: Option<brawler::lobby::GameTypeId>,
}

/// User-visible client connection state. Lightyear lifecycle components remain the truth.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub enum ClientJoinPhase {
    Connecting,
    AwaitingOutcome,
    Active {
        player_id: PlayerId,
        network_entity_id: NetworkEntityId,
    },
    LobbyActive {
        player_id: PlayerId,
    },
    Rejected(MatchJoinRejection),
    Disconnected,
}

#[derive(Component, Clone, Debug, PartialEq)]
pub struct ClientLobbyMembership {
    pub logical_server_id: u128,
    pub player_id: PlayerId,
    pub accepted_display_name: String,
    pub server_name: String,
    pub catalog_revision: brawler::lobby::CatalogRevision,
    pub game_types: Vec<brawler::lobby::AdvertisedGameType>,
    pub brawler_catalog: brawler::profiles::AdvertisedBrawlerCatalog,
    pub profile: brawler::profiles::ProfileSnapshot,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
struct ClientLobbyIdentity {
    logical_server_id: u128,
    account_id: brawler::profiles::AccountId,
}

#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
struct ClientProfileIdentityState {
    logical_server_id: Option<u128>,
    account_id: Option<brawler::profiles::AccountId>,
}

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub enum ClientLobbyFailure {
    Rejected(brawler::protocol::LobbyJoinRejection),
    InvalidWelcome,
}

#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub struct RuntimeLobbyTarget {
    pub logical_address: String,
    pub proposed_display_name: String,
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

#[derive(Resource, Debug, Default)]
struct MatchCommandState {
    next_request_id: u64,
    sent_for_phase: Option<(brawler::matchplay::MatchId, MatchPhase)>,
    last_outcome: Option<MatchCommandOutcome>,
    /// The `LeaveMatch` request in flight and when it was sent (BRL-8y1f).
    leave_sent: Option<(u64, Duration)>,
}

/// Marker proving that the windowed presentation composition is installed.
#[derive(Default, Resource, Debug, PartialEq, Eq)]
pub struct ClientPresentation;

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq, Default)]
pub enum ClientInputContext {
    #[default]
    Gameplay,
    Menu,
    /// The offline product shell owns local input and always emits neutral gameplay intent.
    Shell,
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum ClientSettingsUiSet {
    Capture,
    Shell,
    Present,
}

#[derive(Resource, Default)]
pub(crate) struct InputCaptureConsumed(pub bool);

#[derive(Resource, Default)]
pub(crate) struct MatchSettingsRequest(pub bool);

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
    pub input_settings_revision: u32,
    pub(crate) targeted_ultimate: TargetedUltimateInput,
}

/// Local-only arming state for ultimates whose authoritative activation needs an aim-point
/// confirmation. Entering this mode never spends charge or sends gameplay intent.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TargetedUltimateInput {
    armed_for: Option<brawler::builds::UltimateDefinitionId>,
    previous_raw_buttons: u8,
    primary_suppressed_until_release: bool,
}

impl TargetedUltimateInput {
    pub(crate) fn is_targeting(self, id: brawler::builds::UltimateDefinitionId) -> bool {
        self.armed_for == Some(id)
    }
}

#[derive(Resource, Debug)]
struct LiveInputTrace {
    enabled: bool,
    last_sample: Option<(bool, [bool; 4], Vec2, ActiveInputDevice)>,
    last_aim: Option<Vec2>,
    last_write: Option<(Vec2, u8, usize)>,
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
            input_settings_revision: 0,
            targeted_ultimate: TargetedUltimateInput::default(),
        }
    }
}

#[cfg(test)]
const ACTION_PRIMARY_FIRE: u16 = 1 << 0;
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
    keyboard_mouse_action: bool,
    selected_gamepad: Option<Entity>,
    meaningful_gamepad: Option<Entity>,
    gamepad_action: Option<Entity>,
) -> ActiveInputDevice {
    if keyboard_mouse_action != gamepad_action.is_some() {
        return if keyboard_mouse_action {
            ActiveInputDevice::KeyboardMouse
        } else {
            ActiveInputDevice::Gamepad(gamepad_action.expect("checked gamepad action"))
        };
    }
    if keyboard_mouse_active && meaningful_gamepad.is_some() {
        return match current {
            ActiveInputDevice::KeyboardMouse => ActiveInputDevice::KeyboardMouse,
            ActiveInputDevice::Gamepad(current) if selected_gamepad == Some(current) => {
                ActiveInputDevice::Gamepad(current)
            }
            ActiveInputDevice::Gamepad(_) => selected_gamepad
                .map_or(ActiveInputDevice::KeyboardMouse, ActiveInputDevice::Gamepad),
        };
    }
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
            ClientInputContext::Gameplay => ClientInputContext::Menu,
            ClientInputContext::Menu => ClientInputContext::Gameplay,
            ClientInputContext::Shell => ClientInputContext::Shell,
        };
        pending.latched_buttons = 0;
    }
}

#[derive(Component)]
pub(crate) struct ArenaCamera;

#[derive(Component)]
struct PauseOverlay;

#[derive(Component)]
struct InputSettingsText;

#[derive(Component)]
struct ScoreboardOverlay;

/// Build the windowed or headless client application.
pub fn build_app_with_config(config: ClientNetworkConfig) -> App {
    compose_client_app(App::new(), config)
}

/// Build an automation client: the same composition plus the automation configuration that the
/// smoke, demo, capture, and render-report drivers read.
#[cfg(feature = "automation")]
pub fn build_automation_app(
    config: ClientNetworkConfig,
    automation: ClientAutomationConfig,
) -> App {
    let mut app = App::new();
    app.insert_resource(automation);
    compose_client_app(app, config)
}

fn compose_client_app(mut app: App, config: ClientNetworkConfig) -> App {
    let headless = config.headless;
    let render_profile = config.render_profile;
    let window_size = config.window_size;
    #[cfg(feature = "automation")]
    let screenshot_schedule = app
        .world()
        .get_resource::<ClientAutomationConfig>()
        .and_then(|automation| automation.screenshot_schedule.clone());
    app.insert_resource(config);
    if headless {
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
            brawler::timing::SIMULATION_TICK,
        )))
        .add_plugins(StatesPlugin);
        brawler::logging::add_log_plugin_once(&mut app);
    } else {
        let (present_mode, winit_settings) = render_profile_settings(render_profile);
        app.add_plugins(
            DefaultPlugins
                .set(AssetPlugin {
                    file_path: format!("{REPOSITORY_ROOT}/assets"),
                    ..default()
                })
                .set(ImagePlugin::default_nearest())
                .set(WindowPlugin {
                    primary_window: Some(Window {
                        title: "PewPew Blitz".to_string(),
                        present_mode,
                        resolution: gameplay_window_resolution(window_size),
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
        tick_duration: brawler::timing::SIMULATION_TICK,
    })
    .add_plugins((
        brawler::modes::ModeRegistryPlugin,
        brawler::modes::BuiltInModeRegistrationsPlugin,
        GameplayPlugin,
        GameplayContentPlugin,
        ProtocolPlugin,
        AvianNetworkPlugin,
        ClientReplicatedGameplayPlugin,
        ClientNetworkPlugin,
        diagnostics_overlay::ClientDiagnosticsOverlayPlugin,
    ));
    // Client process closeout and window sampling (`BRAWLER_DIAGNOSTICS_CLOSEOUT_FILE`,
    // `BRAWLER_DIAGNOSTICS_WINDOW_FILE`) serve only the automation scripts (BRL-3p70).
    #[cfg(feature = "automation")]
    app.add_plugins(brawler::diagnostics::ProcessDiagnosticsPlugin);
    if let Some(path) = brawler::diagnostics::failure_record_path() {
        // Parity with the dedicated server: a panic appends a bounded local failure record
        // before terminating, so client crashes keep the same category evidence.
        brawler::diagnostics::install_panic_failure_hook(path);
    }
    if !headless {
        app.add_plugins((
            ClientPresentationPlugin,
            evidence_capture::ClientEvidenceCapturePlugin,
        ));
        if app
            .world()
            .resource::<ClientNetworkConfig>()
            .presents_product_shell()
        {
            app.add_plugins((
                ClientFlowPlugin,
                ClientShellPlugin,
                dashboard::ClientDashboardPlugin,
            ));
        }
        #[cfg(feature = "automation")]
        if let Some(schedule) = screenshot_schedule {
            automation::install_scheduled_screenshots(&mut app, schedule);
        }
    }
    app
}

fn gameplay_window_resolution(window_size: Option<(u16, u16)>) -> WindowResolution {
    let (width, height) = window_size.map_or(
        (
            DEFAULT_GAMEPLAY_WINDOW_WIDTH,
            DEFAULT_GAMEPLAY_WINDOW_HEIGHT,
        ),
        |(width, height)| (u32::from(width), u32::from(height)),
    );
    WindowResolution::new(width, height)
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
#[allow(
    clippy::needless_pass_by_value,
    reason = "network-test callers pass an owned configuration alongside the owned Crossbeam IO"
)]
pub(crate) fn spawn_crossbeam_client(
    world: &mut World,
    config: ClientNetworkConfig,
    io: lightyear::crossbeam::CrossbeamIo,
) -> Entity {
    let auth = Authentication::Manual {
        server_addr: config.server_addr,
        client_id: config.client_id,
        private_key: brawler::protocol::DEVELOPMENT_PRIVATE_KEY,
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

/// Separate-App integration-test support for the combined client/server feature graph.
#[cfg(feature = "network-test")]
pub mod testing {
    use bevy::prelude::{Entity, World};
    use brawler::config::ClientNetworkConfig;

    /// Spawn one deterministic Crossbeam client endpoint for a separate-App test.
    #[allow(
        clippy::needless_pass_by_value,
        reason = "the test endpoint consumes its owned configuration and Crossbeam IO"
    )]
    pub fn spawn_crossbeam_client(
        world: &mut World,
        config: ClientNetworkConfig,
        io: lightyear::crossbeam::CrossbeamIo,
    ) -> Entity {
        crate::spawn_crossbeam_client(world, config, io)
    }
}
