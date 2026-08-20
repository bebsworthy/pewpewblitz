//! M03 product flow, bounded action arbitration, and recoverable lobby presentation.

use super::{
    ClientJoinPhase, ClientJoinStatus, ClientLobbyFailure, ClientLobbyMembership,
    ClientNetworkConfig, ClientSettingsUiSet, RoutedClientLifecycle, RoutedClientPhase,
    RoutedClientSession, RuntimeLobbyTarget,
    connection_persistence::{
        ClientConnectionsPath, ConnectionsFileV1, load_connections, save_connections,
    },
    server_select::{
        LogicalServerAddress, MAX_RESOLVED_CANDIDATES, ServerAddressHost, parse_server_address,
    },
    session::{ProductLobbyAttempt, spawn_product_lobby_connection},
};
use bevy::{
    ecs::schedule::ApplyDeferred,
    input::{
        ButtonState,
        keyboard::KeyboardInput,
        mouse::{MouseScrollUnit, MouseWheel},
    },
    prelude::*,
    tasks::{IoTaskPool, Task, block_on, poll_once},
    ui::{InteractionDisabled, ScrollPosition},
};
use lightyear::prelude::client::{Client, Disconnect};
use lightyear::prelude::{Unlink, UnlinkReason};
use std::{
    collections::BTreeSet,
    net::{SocketAddr, ToSocketAddrs as _},
    time::Duration,
};
use unicode_segmentation::UnicodeSegmentation as _;

const DNS_DEADLINE: Duration = Duration::from_secs(5);
const ATTEMPT_DEADLINE: Duration = Duration::from_secs(10);
const ERROR_BUTTON_BASE: usize = 1_000;
const BUILD_EDITOR_CHOICE_BASE: usize = 2_000;
const BUILD_EDITOR_FIELD_BASE: usize = 2_010;
const BUILD_EDITOR_OPTION_BASE: usize = 2_030;
const BUILD_EDITOR_JOIN_INDEX: usize = 2_100;
const BUILD_EDITOR_BACK_INDEX: usize = 2_101;
const BUILD_EDITOR_DISCONNECT_INDEX: usize = 2_102;

#[derive(States, Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ClientFlow {
    #[default]
    Title,
    ServerSelect,
    Connecting,
    GameSelect,
    Queue,
    MatchLoading,
    Match,
    Results,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct CancelMatchStartConfirmation {
    pub reservation_id: crate::lobby::MatchReservationId,
    pub generation: u32,
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub enum ClientOverlay {
    #[default]
    None,
    Settings,
    Credits,
    BuildEditor,
    Confirmation(CancelMatchStartConfirmation),
    LeaveConfirmation,
    Error(FlowError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowError {
    pub kind: FlowErrorKind,
    pub message: String,
    pub return_flow: ClientFlow,
    pub actions: [Option<FlowErrorAction>; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowErrorKind {
    Connection,
    Queue,
    Persistence,
    Content,
}

impl FlowErrorKind {
    const fn title(self) -> &'static str {
        match self {
            Self::Connection => "CONNECTION ERROR",
            Self::Queue => "QUEUE ERROR",
            Self::Persistence => "SAVE ERROR",
            Self::Content => "CONTENT ERROR",
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowErrorAction {
    RetryConnection,
    EditName,
    Back,
    RetrySave,
    ContinueWithoutSaving,
    ContinueWithDefaults,
    RetryQueue,
    TryAgainQueue,
    Disconnect,
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum ClientFlowSet {
    BeginFlowFrame,
    ObserveSession,
    CollectFlowInput,
    ResolveFlowAction,
    TeardownSession,
    CommitFlow,
    PresentFlow,
}

#[derive(Clone, Debug, PartialEq, Eq)]
enum FlowUiAction {
    EditAddress,
    EditName,
    Connect,
    Back,
    Cancel,
    Retry,
    RetrySave,
    ContinueWithoutSaving,
    DismissError,
    JoinSaved(String),
    RemoveFavorite(String),
    SelectGame(usize),
    ToggleFavorite,
    Disconnect,
    OpenBuildEditor,
    ChooseBuild(usize),
    FocusBuildField(usize),
    ChooseBuildFieldValue {
        field_index: usize,
        value_index: usize,
    },
    CancelBuildEditor,
    JoinQueue,
    CancelQueue,
    RetryQueue,
    TryAgainQueue,
    RequestCancelMatchStart,
    KeepLoading,
    ConfirmCancelMatchStart,
    QueueAgain,
    ChangeGame,
    KeepPlaying,
    ConfirmLeaveMatch,
}

#[derive(Clone, Debug)]
enum SessionObservation {
    Accepted,
    Rejected(ClientLobbyFailure),
    ResolverCompleted {
        generation: u64,
        result: Result<Vec<SocketAddr>, String>,
    },
    CandidateFailed,
    CandidateTimedOut,
    DnsTimedOut,
    UnexpectedLoss,
    TimedOut,
    QueueOutcome(crate::lobby::QueueCommandOutcome),
    QueueProtocolFailure,
    QueueTimedOut,
    ReservationStarted,
    MatchStartReturned,
    CountdownObserved,
    FreshLobbyReturn,
    MatchFailed,
}

#[derive(Resource, Default)]
struct PendingFlowActions {
    session: Option<SessionObservation>,
    explicit: Option<FlowUiAction>,
    ordinary: Option<FlowUiAction>,
}

#[derive(Resource, Default)]
struct FlowCommit {
    next_flow: Option<ClientFlow>,
    start_target: Option<ValidatedConnectionTarget>,
    teardown: bool,
    advance_candidate: bool,
    error: Option<FlowError>,
    overlay: Option<OverlayCommit>,
    refresh_server_select: Option<usize>,
    focus_index: Option<usize>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum OverlayCommit {
    Clear,
    BuildEditor,
    Confirmation(CancelMatchStartConfirmation),
}

#[derive(Clone, Debug, PartialEq, Eq)]
struct ValidatedConnectionTarget {
    logical_address: LogicalServerAddress,
    proposed_display_name: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConnectionStage {
    ResolvingAddress,
    ContactingServer { current: usize, total: usize },
    JoiningLobby,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum AttemptDeadlineExpiry {
    Dns,
    Overall,
    Candidate,
}

#[derive(Resource, Clone, Debug)]
struct PendingConnection {
    generation: u64,
    target: ValidatedConnectionTarget,
    candidates: Vec<SocketAddr>,
    current_candidate: usize,
    overall_deadline: Duration,
    dns_deadline: Option<Duration>,
    candidate_deadline: Option<Duration>,
    current_entity: Option<Entity>,
    stage: ConnectionStage,
}

#[derive(Resource, Default)]
struct ConnectionGeneration(u64);

struct ResolverTask {
    generation: u64,
    task: Task<Result<Vec<SocketAddr>, String>>,
}

#[derive(Resource, Default)]
struct ResolverState {
    task: Option<ResolverTask>,
}

#[derive(Resource, Clone, Debug)]
struct ServerSelectModel {
    address: String,
    committed_name: String,
    name: String,
    editing: Option<EditingField>,
    caret: usize,
    inline_error: Option<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EditingField {
    Address,
    Name,
}

#[derive(Resource, Clone, Debug)]
struct ConnectionPersistence {
    state: ConnectionsFileV1,
    dirty_error: Option<String>,
}

#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ClientLocalLoadFailures {
    pub settings_failed: bool,
    pub connections_failed: bool,
    pub build_failed: bool,
}

pub(super) fn local_load_error(failures: ClientLocalLoadFailures) -> Option<FlowError> {
    let mut sources = Vec::new();
    if failures.settings_failed {
        sources.push("Settings");
    }
    if failures.connections_failed {
        sources.push("connection data");
    }
    if failures.build_failed {
        sources.push("saved build");
    }
    if sources.is_empty() {
        return None;
    }
    let message = format!(
        "{} could not be loaded; safe defaults are active",
        match sources.as_slice() {
            [one] => (*one).to_string(),
            [first, second] => format!("{first} and {second}"),
            [first, second, third] => format!("{first}, {second}, and {third}"),
            _ => unreachable!("three closed persistence sources"),
        }
    );
    Some(FlowError {
        kind: FlowErrorKind::Persistence,
        message,
        return_flow: ClientFlow::Title,
        actions: [Some(FlowErrorAction::ContinueWithDefaults), None],
    })
}

#[derive(Resource, Default, Clone, Debug, PartialEq, Eq)]
pub struct SelectedGameType {
    pub catalog_revision: Option<crate::lobby::CatalogRevision>,
    pub game_type_id: Option<crate::lobby::GameTypeId>,
    pub configuration_revision: Option<u32>,
}

#[derive(Resource, Default)]
struct FlowNavigation {
    selected: usize,
}

#[derive(Resource, Default)]
struct MatchFailureNotice(bool);

#[derive(Component)]
struct FlowRoot;

#[derive(Component, Clone, Debug)]
struct FlowButton {
    index: usize,
    action: FlowUiAction,
    error_action: bool,
    build_editor_action: bool,
}

#[derive(Component)]
struct FlowErrorRoot(FlowError);

#[derive(Component)]
struct RateLimitTryAgain;

#[derive(Component)]
struct RateLimitTryAgainLabel;

#[derive(Component)]
struct BuildEditorRoot;

#[derive(Clone, Debug, PartialEq, Eq)]
struct BuildEditorRenderKey {
    selected_choice: usize,
    custom_recipe: crate::builds::BrawlerBuildRecipe,
    focused_field: super::BuildEditorField,
    inline_error: Option<String>,
    game_type_id: Option<crate::lobby::GameTypeId>,
    game_name: String,
    joining: bool,
    capacity_occupied: bool,
}

#[derive(Component)]
struct GamePopulationLabel(usize);

#[derive(Component)]
struct QueueStatusLabel;

#[derive(Component)]
struct QueueCancelButton;

#[derive(Component)]
struct QueueCancelLabel;

#[derive(Component)]
struct MatchLoadingStatusLabel;

#[derive(Component)]
struct CancelConfirmationRoot;

#[derive(Component)]
struct LeaveConfirmationRoot;

#[derive(Component)]
struct MatchCompletionRoot;

#[derive(Component, Clone, Copy)]
enum FieldLabel {
    Address,
    Name,
}

#[derive(Component)]
struct ConnectingLabel;

pub struct ClientFlowPlugin;

impl Plugin for ClientFlowPlugin {
    #[allow(
        clippy::too_many_lines,
        reason = "the composition point keeps the ordered product-flow schedule and state hooks visible"
    )]
    fn build(&self, app: &mut App) {
        app.init_state::<ClientFlow>()
            .init_resource::<ClientOverlay>()
            .init_resource::<PendingFlowActions>()
            .init_resource::<FlowCommit>()
            .init_resource::<ConnectionGeneration>()
            .init_resource::<ResolverState>()
            .init_resource::<ClientConnectionsPath>()
            .init_resource::<super::ClientBuildPath>()
            .init_resource::<ClientLocalLoadFailures>()
            .init_resource::<super::BuildEditorState>()
            .init_resource::<super::ClientQueueModel>()
            .init_resource::<super::ClientMatchLoadingModel>()
            .init_resource::<crate::builds::BuildCatalogResource>()
            .init_resource::<crate::combat::WeaponCatalogResource>()
            .init_resource::<SelectedGameType>()
            .init_resource::<super::ClientMatchResultState>()
            .init_resource::<RoutedClientLifecycle>()
            .init_resource::<MatchFailureNotice>()
            .init_resource::<FlowNavigation>()
            .add_systems(
                Startup,
                (
                    load_connection_state,
                    load_build_state,
                    show_local_load_error,
                )
                    .chain(),
            )
            .configure_sets(
                Update,
                (
                    ClientFlowSet::BeginFlowFrame,
                    ClientFlowSet::ObserveSession,
                    ClientFlowSet::CollectFlowInput,
                    ClientFlowSet::ResolveFlowAction,
                    ClientFlowSet::TeardownSession,
                    ClientFlowSet::CommitFlow,
                    ClientFlowSet::PresentFlow,
                )
                    .chain()
                    .in_set(ClientSettingsUiSet::Shell),
            )
            .add_systems(
                Update,
                (
                    begin_flow_frame.in_set(ClientFlowSet::BeginFlowFrame),
                    observe_session
                        .in_set(ClientFlowSet::ObserveSession)
                        .after(super::queue::observe_queue_messages),
                    collect_flow_input.in_set(ClientFlowSet::CollectFlowInput),
                    resolve_flow_action.in_set(ClientFlowSet::ResolveFlowAction),
                    teardown_session.in_set(ClientFlowSet::TeardownSession),
                    ApplyDeferred
                        .after(ClientFlowSet::TeardownSession)
                        .before(ClientFlowSet::CommitFlow),
                    commit_flow.in_set(ClientFlowSet::CommitFlow),
                    refresh_server_select
                        .in_set(ClientFlowSet::PresentFlow)
                        .before(present_flow),
                    present_flow_error_overlay
                        .in_set(ClientFlowSet::PresentFlow)
                        .before(present_flow),
                    present_cancel_confirmation
                        .in_set(ClientFlowSet::PresentFlow)
                        .before(present_flow),
                    present_leave_confirmation
                        .in_set(ClientFlowSet::PresentFlow)
                        .before(present_flow),
                    present_match_completion
                        .in_set(ClientFlowSet::PresentFlow)
                        .before(present_flow),
                    update_match_loading
                        .in_set(ClientFlowSet::PresentFlow)
                        .before(present_flow),
                    update_rate_limit_try_again
                        .in_set(ClientFlowSet::PresentFlow)
                        .after(present_flow_error_overlay)
                        .before(present_flow),
                    present_build_editor_overlay
                        .in_set(ClientFlowSet::PresentFlow)
                        .before(present_flow),
                    scroll_build_editor
                        .in_set(ClientFlowSet::PresentFlow)
                        .after(present_build_editor_overlay)
                        .before(present_flow),
                    keep_build_editor_focus_visible
                        .in_set(ClientFlowSet::PresentFlow)
                        .after(scroll_build_editor)
                        .before(present_flow),
                    update_queue_cancel_button
                        .in_set(ClientFlowSet::PresentFlow)
                        .before(present_flow),
                    present_flow.in_set(ClientFlowSet::PresentFlow),
                ),
            )
            .add_systems(OnEnter(ClientFlow::ServerSelect), spawn_server_select)
            .add_systems(OnEnter(ClientFlow::Connecting), spawn_connecting)
            .add_systems(OnEnter(ClientFlow::GameSelect), spawn_game_select)
            .add_systems(OnEnter(ClientFlow::Match), enter_match_input)
            .add_systems(OnEnter(ClientFlow::Results), spawn_results)
            .add_systems(OnExit(ClientFlow::Results), clear_results)
            .add_systems(OnExit(ClientFlow::Match), exit_match_input);
        app.add_systems(OnEnter(ClientFlow::Queue), spawn_queue);
        app.add_systems(OnEnter(ClientFlow::MatchLoading), spawn_match_loading);
    }
}

fn enter_match_input(mut context: ResMut<super::ClientInputContext>) {
    *context = super::ClientInputContext::Gameplay;
}

fn exit_match_input(mut context: ResMut<super::ClientInputContext>) {
    *context = super::ClientInputContext::Shell;
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn load_connection_state(
    mut commands: Commands,
    path: Res<ClientConnectionsPath>,
    config: Res<ClientNetworkConfig>,
    mut failures: ResMut<ClientLocalLoadFailures>,
) {
    let state = match load_connections(&path.0) {
        Ok(Some(state)) => state,
        Ok(None) => ConnectionsFileV1::empty(),
        Err(_) => {
            failures.connections_failed = true;
            ConnectionsFileV1::empty()
        }
    };
    let name = state
        .preferred_display_name
        .clone()
        .unwrap_or_else(|| crate::lobby::generated_display_name(config.client_id));
    let address = config
        .product_server_prefill
        .clone()
        .unwrap_or_else(|| "127.0.0.1:5000".to_string());
    commands.insert_resource(ServerSelectModel {
        address,
        committed_name: name.clone(),
        name,
        editing: None,
        caret: 0,
        inline_error: None,
    });
    commands.insert_resource(ConnectionPersistence {
        state,
        dirty_error: None,
    });
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn load_build_state(
    build_path: Res<super::ClientBuildPath>,
    builds: Res<crate::builds::BuildCatalogResource>,
    weapons: Res<crate::combat::WeaponCatalogResource>,
    mut editor: ResMut<super::BuildEditorState>,
    mut failures: ResMut<ClientLocalLoadFailures>,
) {
    match super::load_build(&build_path.0, &builds.0, &weapons.0) {
        Ok(Some(file)) => editor.loaded_selection = file.selection,
        Ok(None) => {}
        Err(_) => failures.build_failed = true,
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn show_local_load_error(
    failures: Res<ClientLocalLoadFailures>,
    mut overlay: ResMut<ClientOverlay>,
) {
    if let Some(error) = local_load_error(*failures) {
        *overlay = ClientOverlay::Error(error);
    }
}

fn begin_flow_frame(mut actions: ResMut<PendingFlowActions>, mut commit: ResMut<FlowCommit>) {
    *actions = PendingFlowActions::default();
    *commit = FlowCommit::default();
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "this schedule phase observes distinct runtime-owned Bevy inputs"
)]
fn observe_session(
    time: Res<Time<Real>>,
    flow: Res<State<ClientFlow>>,
    pending: Option<ResMut<PendingConnection>>,
    mut resolver: ResMut<ResolverState>,
    memberships: Query<(Entity, &ClientLobbyMembership), With<Client>>,
    failures: Query<&ClientLobbyFailure, With<Client>>,
    statuses: Query<&ClientJoinStatus, (With<Client>, With<RoutedClientSession>)>,
    match_states: Query<&crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>,
    mut actions: ResMut<PendingFlowActions>,
    mut queue: ResMut<super::ClientQueueModel>,
    mut loading: ResMut<super::ClientMatchLoadingModel>,
    result_state: Res<super::ClientMatchResultState>,
    routed: Res<RoutedClientLifecycle>,
) {
    if let Some(task) = resolver.task.as_mut()
        && let Some(result) = block_on(poll_once(&mut task.task))
    {
        let generation = task.generation;
        resolver.task = None;
        if pending
            .as_ref()
            .is_some_and(|pending| pending.generation == generation)
        {
            actions.session = Some(SessionObservation::ResolverCompleted { generation, result });
        }
    }
    if queue.protocol_failure() {
        actions.session = Some(SessionObservation::QueueProtocolFailure);
        return;
    }
    if let Some(started) = loading.take_started() {
        let _ = started;
        actions.session = Some(SessionObservation::ReservationStarted);
        return;
    }
    if loading.take_returned() {
        actions.session = Some(SessionObservation::MatchStartReturned);
        return;
    }
    if *flow.get() == ClientFlow::Match
        && memberships.iter().next().is_some()
        && statuses
            .iter()
            .any(|status| matches!(status.phase, ClientJoinPhase::LobbyActive { .. }))
    {
        actions.session = Some(SessionObservation::FreshLobbyReturn);
        return;
    }
    if *flow.get() == ClientFlow::Match
        && routed.phase == RoutedClientPhase::Match
        && result_state.context.is_none()
        && statuses
            .iter()
            .any(|status| matches!(status.phase, ClientJoinPhase::Disconnected))
    {
        actions.session = Some(SessionObservation::MatchFailed);
        return;
    }
    if *flow.get() == ClientFlow::MatchLoading {
        if match_states.iter().any(|state| {
            matches!(
                state.phase,
                crate::matchplay::MatchPhase::Countdown { .. }
                    | crate::matchplay::MatchPhase::Active { .. }
                    | crate::matchplay::MatchPhase::Completed { .. }
            )
        }) {
            actions.session = Some(SessionObservation::CountdownObserved);
        }
        return;
    }
    if let Some(outcome) = queue.take_outcome() {
        actions.session = Some(SessionObservation::QueueOutcome(outcome));
        return;
    }
    if matches!(*flow.get(), ClientFlow::GameSelect | ClientFlow::Queue) {
        if statuses
            .iter()
            .any(|status| matches!(status.phase, ClientJoinPhase::Disconnected))
        {
            actions.session = Some(SessionObservation::UnexpectedLoss);
        } else if queue.take_timeout_notice() {
            actions.session = Some(SessionObservation::QueueTimedOut);
        }
        return;
    }
    if *flow.get() != ClientFlow::Connecting {
        return;
    }
    let Some(mut pending) = pending else {
        return;
    };
    if let Some(failure) = failures.iter().next() {
        actions.session = Some(SessionObservation::Rejected(failure.clone()));
        return;
    }
    let disconnected = statuses
        .iter()
        .any(|status| matches!(status.phase, ClientJoinPhase::Disconnected));
    if memberships.iter().next().is_some()
        && statuses
            .iter()
            .any(|status| matches!(status.phase, ClientJoinPhase::LobbyActive { .. }))
    {
        actions.session = Some(accepted_observation(time.elapsed(), &pending, disconnected));
        return;
    }
    let now = time.elapsed();
    if let Some(expiry) = attempt_deadline_expiry(now, &pending) {
        actions.session = Some(observation_for_expiry(expiry));
        return;
    }
    if let Some(status) = statuses.iter().next() {
        if matches!(status.phase, ClientJoinPhase::AwaitingOutcome) {
            pending.stage = ConnectionStage::JoiningLobby;
        }
        if pending.current_entity.is_some() && disconnected {
            actions.session = Some(SessionObservation::CandidateFailed);
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "the bounded flow input phase keeps field and navigation precedence visible"
)]
fn collect_flow_input(
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    keyboard: Res<ButtonInput<KeyCode>>,
    gamepads: Query<&Gamepad>,
    mut keyboard_events: MessageReader<KeyboardInput>,
    mut model: ResMut<ServerSelectModel>,
    mut persistence: ResMut<ConnectionPersistence>,
    path: Res<ClientConnectionsPath>,
    mut navigation: ResMut<FlowNavigation>,
    buttons: Query<(&FlowButton, &Interaction, Has<InteractionDisabled>)>,
    mut actions: ResMut<PendingFlowActions>,
    queue: Res<super::ClientQueueModel>,
) {
    for (button, interaction, disabled) in &buttons {
        if !disabled
            && *interaction == Interaction::Pressed
            && overlay_allows_button(&overlay, button)
        {
            navigation.selected = button.index;
            queue_ui_action(&mut actions, button.action.clone());
        }
    }
    let pad_pressed = |button| gamepads.iter().any(|pad| pad.just_pressed(button));
    if let Some(field) = model.editing {
        if keyboard.just_pressed(KeyCode::Home) {
            model.caret = 0;
        } else if keyboard.just_pressed(KeyCode::End) {
            model.caret = edited_value(&model, field).len();
        } else if keyboard.just_pressed(KeyCode::ArrowLeft) {
            model.caret = previous_caret(edited_value(&model, field), model.caret, field);
        } else if keyboard.just_pressed(KeyCode::ArrowRight) {
            model.caret = next_caret(edited_value(&model, field), model.caret, field);
        }
        if keyboard.just_pressed(KeyCode::KeyV)
            && keyboard.any_pressed([
                KeyCode::ControlLeft,
                KeyCode::ControlRight,
                KeyCode::SuperLeft,
                KeyCode::SuperRight,
            ])
        {
            match arboard::Clipboard::new().and_then(|mut clipboard| clipboard.get_text()) {
                Ok(text) => insert_editor_text(&mut model, field, &text),
                Err(error) => {
                    model.inline_error = Some(format!("Clipboard text is unavailable: {error}"));
                }
            }
        }
        for event in keyboard_events.read() {
            if event.state != ButtonState::Pressed {
                continue;
            }
            if event.key_code == KeyCode::Backspace {
                let previous = previous_caret(edited_value(&model, field), model.caret, field);
                let caret = model.caret;
                edited_value_mut(&mut model, field).replace_range(previous..caret, "");
                model.caret = previous;
            } else if event.key_code == KeyCode::Delete {
                let next = next_caret(edited_value(&model, field), model.caret, field);
                let caret = model.caret;
                edited_value_mut(&mut model, field).replace_range(caret..next, "");
            } else if let Some(text) = event.text.as_deref() {
                insert_editor_text(&mut model, field, text);
            }
        }
        if keyboard.just_pressed(KeyCode::Enter) || pad_pressed(GamepadButton::South) {
            if field == EditingField::Name {
                match crate::lobby::normalize_proposed_display_name(&model.name) {
                    Ok(name) => {
                        model.name.clone_from(&name);
                        model.committed_name.clone_from(&name);
                        persistence.state.preferred_display_name = Some(name);
                        if let Err(error) = save_connections(&path.0, &persistence.state) {
                            persistence.dirty_error = Some(error);
                        }
                        model.inline_error = None;
                    }
                    Err(error) => model.inline_error = Some(format!("Invalid name: {error}")),
                }
            }
            model.editing = None;
        } else if keyboard.just_pressed(KeyCode::Escape) || pad_pressed(GamepadButton::East) {
            if field == EditingField::Name {
                model.name = model.committed_name.clone();
            }
            model.editing = None;
        }
        return;
    }

    let mut available = buttons
        .iter()
        .filter(|(button, _, disabled)| !*disabled && overlay_allows_button(&overlay, button))
        .map(|(button, _, _)| button.index)
        .collect::<Vec<_>>();
    available.sort_unstable();
    if !available.is_empty() {
        let position = available
            .iter()
            .position(|index| *index == navigation.selected)
            .unwrap_or(0);
        if keyboard.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS])
            || pad_pressed(GamepadButton::DPadDown)
        {
            navigation.selected = available[(position + 1).min(available.len() - 1)];
        }
        if keyboard.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW])
            || pad_pressed(GamepadButton::DPadUp)
        {
            navigation.selected = available[position.saturating_sub(1)];
        }
    }
    if (keyboard.any_just_pressed([KeyCode::Enter, KeyCode::Space])
        || pad_pressed(GamepadButton::South))
        && let Some((button, _, _)) = buttons
            .iter()
            .filter(|(button, _, disabled)| {
                !*disabled
                    && button.index == navigation.selected
                    && overlay_allows_button(&overlay, button)
            })
            .min_by_key(|(button, _, _)| button.index)
    {
        queue_ui_action(&mut actions, button.action.clone());
    }
    if keyboard.just_pressed(KeyCode::Escape) || pad_pressed(GamepadButton::East) {
        let action = if matches!(overlay.as_ref(), ClientOverlay::BuildEditor) {
            if queue.pending().is_some_and(|pending| {
                matches!(pending.command, crate::lobby::QueueCommand::Join(_))
            }) {
                return;
            }
            FlowUiAction::CancelBuildEditor
        } else {
            match *flow.get() {
                ClientFlow::Connecting => FlowUiAction::Cancel,
                ClientFlow::GameSelect
                | ClientFlow::Queue
                | ClientFlow::MatchLoading
                | ClientFlow::Results => FlowUiAction::Disconnect,
                ClientFlow::Match => {
                    if matches!(overlay.as_ref(), ClientOverlay::LeaveConfirmation) {
                        FlowUiAction::KeepPlaying
                    } else {
                        return;
                    }
                }
                ClientFlow::ServerSelect => FlowUiAction::Back,
                ClientFlow::Title => return,
            }
        };
        queue_ui_action(&mut actions, action);
    }
}

fn overlay_allows_button(overlay: &ClientOverlay, button: &FlowButton) -> bool {
    match overlay {
        ClientOverlay::Error(_)
        | ClientOverlay::Confirmation(_)
        | ClientOverlay::LeaveConfirmation => button.error_action,
        ClientOverlay::BuildEditor => button.build_editor_action,
        _ => !button.error_action && !button.build_editor_action,
    }
}

fn queue_ui_action(actions: &mut PendingFlowActions, action: FlowUiAction) {
    if matches!(
        action,
        FlowUiAction::Cancel | FlowUiAction::Disconnect | FlowUiAction::CancelBuildEditor
    ) {
        actions.explicit = Some(action);
    } else {
        actions.ordinary = Some(action);
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "one bounded coordinator makes flow-action precedence and commits explicit"
)]
fn resolve_flow_action(
    time: Res<Time<Real>>,
    flow: Res<State<ClientFlow>>,
    mut actions: ResMut<PendingFlowActions>,
    mut commit: ResMut<FlowCommit>,
    mut model: ResMut<ServerSelectModel>,
    mut persistence: ResMut<ConnectionPersistence>,
    mut pending: Option<ResMut<PendingConnection>>,
    membership: Query<
        (
            &ClientLobbyMembership,
            Option<&RuntimeLobbyTarget>,
            &RoutedClientSession,
        ),
        With<Client>,
    >,
    path: Res<ClientConnectionsPath>,
    overlay: Res<ClientOverlay>,
    mut selection: ResMut<SelectedGameType>,
    mut models: (
        ResMut<super::ClientQueueModel>,
        ResMut<super::ClientMatchLoadingModel>,
        ResMut<super::ClientMatchResultState>,
        ResMut<RoutedClientLifecycle>,
        ResMut<MatchFailureNotice>,
    ),
    mut editor: ResMut<super::BuildEditorState>,
    builds: Res<crate::builds::BuildCatalogResource>,
    weapons: Res<crate::combat::WeaponCatalogResource>,
    build_path: Res<super::ClientBuildPath>,
) {
    let (
        ref mut queue,
        ref mut loading,
        ref mut result_state,
        ref mut routed,
        ref mut match_failure,
    ) = models;
    if let Some(explicit) = actions.explicit.take() {
        match explicit {
            FlowUiAction::Cancel | FlowUiAction::Disconnect => {
                commit.teardown = true;
                commit.next_flow = Some(ClientFlow::ServerSelect);
                *selection = SelectedGameType::default();
                result_state.context = None;
                editor.close_without_acceptance();
            }
            FlowUiAction::CancelBuildEditor => {
                editor.close_without_acceptance();
                commit.overlay = Some(OverlayCommit::Clear);
                commit.focus_index = membership
                    .iter()
                    .next()
                    .map(|(membership, _, _)| membership.game_types.len());
            }
            _ => {}
        }
        return;
    }
    if let Some(observation) = actions.session.take() {
        match observation {
            SessionObservation::Accepted => {
                if let Some((membership, target, _)) = membership.iter().next() {
                    persistence.state.preferred_display_name = Some(model.committed_name.clone());
                    if let Some(target) = target {
                        let _ = persistence
                            .state
                            .record_recent(&membership.server_name, &target.logical_address);
                    }
                    if let Err(error) = save_connections(&path.0, &persistence.state) {
                        persistence.dirty_error = Some(error.clone());
                        commit.error = Some(FlowError {
                            kind: FlowErrorKind::Persistence,
                            message: format!("Could not save connection data: {error}"),
                            return_flow: ClientFlow::GameSelect,
                            actions: [
                                Some(FlowErrorAction::RetrySave),
                                Some(FlowErrorAction::ContinueWithoutSaving),
                            ],
                        });
                    }
                    *selection = SelectedGameType::default();
                    commit.next_flow = Some(ClientFlow::GameSelect);
                }
            }
            SessionObservation::ResolverCompleted { generation, result } => {
                if let Some(pending) = pending.as_deref_mut()
                    && pending.generation == generation
                {
                    match result {
                        Ok(candidates) if !candidates.is_empty() => {
                            pending.candidates = candidates;
                            pending.dns_deadline = None;
                            pending.current_candidate = 0;
                            pending.stage = ConnectionStage::ContactingServer {
                                current: 1,
                                total: pending.candidates.len(),
                            };
                            commit.advance_candidate = true;
                        }
                        Ok(_) => fail_to_server_select(
                            &mut commit,
                            "Address resolution returned no usable addresses".to_string(),
                            true,
                        ),
                        Err(error) => fail_to_server_select(&mut commit, error, true),
                    }
                }
            }
            SessionObservation::CandidateFailed => {
                commit.teardown = true;
                if pending
                    .as_ref()
                    .is_some_and(|pending| has_next_candidate(pending))
                {
                    commit.advance_candidate = true;
                } else {
                    fail_to_server_select(
                        &mut commit,
                        "Could not contact the server".to_string(),
                        true,
                    );
                }
            }
            SessionObservation::CandidateTimedOut => {
                commit.teardown = true;
                if pending
                    .as_ref()
                    .is_some_and(|pending| has_next_candidate(pending))
                {
                    commit.advance_candidate = true;
                } else {
                    fail_to_server_select(
                        &mut commit,
                        "The lobby handshake timed out".to_string(),
                        true,
                    );
                }
            }
            SessionObservation::DnsTimedOut => {
                commit.teardown = true;
                fail_to_server_select(
                    &mut commit,
                    "Address resolution timed out".to_string(),
                    true,
                );
            }
            SessionObservation::UnexpectedLoss => {
                commit.teardown = true;
                *selection = SelectedGameType::default();
                fail_to_server_select(
                    &mut commit,
                    "The lobby connection was lost unexpectedly".to_string(),
                    true,
                );
            }
            SessionObservation::TimedOut => {
                commit.teardown = true;
                fail_to_server_select(
                    &mut commit,
                    "The connection attempt timed out".to_string(),
                    true,
                );
            }
            SessionObservation::Rejected(reason) => {
                commit.teardown = true;
                commit.next_flow = Some(ClientFlow::ServerSelect);
                commit.error = Some(rejection_flow_error(reason));
            }
            SessionObservation::QueueProtocolFailure => {
                commit.teardown = true;
                editor.close_without_acceptance();
                *selection = SelectedGameType::default();
                fail_to_server_select_with_kind(
                    &mut commit,
                    FlowErrorKind::Content,
                    "The lobby queue state was incompatible with this client".to_string(),
                    true,
                );
            }
            SessionObservation::QueueTimedOut => {
                let label =
                    queue
                        .pending()
                        .map_or("queue command", |pending| match pending.command {
                            crate::lobby::QueueCommand::Join(_) => "queue admission",
                            crate::lobby::QueueCommand::Cancel(_) => "queue cancellation",
                        });
                commit.error = Some(FlowError {
                    kind: FlowErrorKind::Queue,
                    message: format!("The {label} acknowledgement is taking longer than expected"),
                    return_flow: *flow.get(),
                    actions: [
                        Some(FlowErrorAction::RetryQueue),
                        Some(FlowErrorAction::Disconnect),
                    ],
                });
            }
            SessionObservation::ReservationStarted => {
                commit.next_flow = Some(ClientFlow::MatchLoading);
                commit.overlay = Some(OverlayCommit::Clear);
            }
            SessionObservation::MatchStartReturned | SessionObservation::FreshLobbyReturn => {
                if let Some(context) = result_state.context.as_mut()
                    && let Some(game_type_id) = context.game_type_id.as_ref()
                    && let Some((membership, _, _)) = membership.iter().next()
                    && let Some(game) = membership
                        .game_types
                        .iter()
                        .find(|game| &game.id == game_type_id)
                {
                    context.game_name = Some(game.display_name.clone());
                    selection.catalog_revision = Some(membership.catalog_revision);
                    selection.game_type_id = Some(game.id.clone());
                    selection.configuration_revision = Some(game.configuration_revision);
                }
                commit.next_flow = Some(if result_state.context.is_some() {
                    ClientFlow::Results
                } else {
                    ClientFlow::GameSelect
                });
                if core::mem::take(&mut match_failure.0) {
                    commit.error = Some(FlowError {
                        kind: FlowErrorKind::Connection,
                        message: "The match server stopped unexpectedly".to_string(),
                        return_flow: ClientFlow::GameSelect,
                        actions: [Some(FlowErrorAction::Back), None],
                    });
                }
                commit.overlay = Some(OverlayCommit::Clear);
            }
            SessionObservation::MatchFailed => {
                result_state.context = None;
                match_failure.0 = true;
                let _ = routed.request_return_to_lobby();
            }
            SessionObservation::CountdownObserved => {
                commit.next_flow = Some(ClientFlow::Match);
                commit.overlay = Some(OverlayCommit::Clear);
            }
            SessionObservation::QueueOutcome(outcome) => match outcome.decision {
                crate::lobby::QueueDecision::Joined(membership) => {
                    result_state.context = None;
                    result_state.last_accepted_game_type_id = Some(membership.game_type_id.clone());
                    selection.catalog_revision = Some(membership.catalog_revision);
                    selection.game_type_id = Some(membership.game_type_id.clone());
                    selection.configuration_revision =
                        Some(membership.game_type_configuration_revision);
                    let selection_to_save = editor.submitted_selection.unwrap_or_else(|| {
                        membership
                            .accepted_build
                            .identity
                            .source_build_preset_id
                            .map_or(
                                crate::builds::BuildSelection::Custom(
                                    membership.accepted_build.canonical_recipe,
                                ),
                                crate::builds::BuildSelection::Preset,
                            )
                    });
                    let canonical_recipe = match selection_to_save {
                        crate::builds::BuildSelection::Preset(id) => {
                            builds.0.preset(id).map(|preset| preset.recipe)
                        }
                        crate::builds::BuildSelection::Custom(recipe) => Some(recipe),
                    };
                    let local =
                        super::resolve_build_preview(selection_to_save, &builds.0, &weapons.0);
                    if canonical_recipe != Some(membership.accepted_build.canonical_recipe)
                        || local.as_ref().map_or(true, |preview| {
                            preview.identity != membership.accepted_build.identity
                                || preview.total_points != membership.accepted_build.total_points
                        })
                    {
                        commit.teardown = true;
                        fail_to_server_select_with_kind(
                            &mut commit,
                            FlowErrorKind::Content,
                            "The accepted build disagreed with local authenticated content"
                                .to_string(),
                            true,
                        );
                        return;
                    }
                    editor.accept(selection_to_save);
                    commit.next_flow = Some(ClientFlow::Queue);
                    commit.overlay = Some(OverlayCommit::Clear);
                    let file = super::BuildFileV1::new(
                        membership.accepted_build.identity.revision,
                        selection_to_save,
                    );
                    if let Err(error) =
                        super::save_build(&build_path.0, file, &builds.0, &weapons.0)
                    {
                        commit.error = Some(FlowError {
                            kind: FlowErrorKind::Persistence,
                            message: format!(
                                "Queued successfully, but the build could not be saved: {error}"
                            ),
                            return_flow: ClientFlow::Queue,
                            actions: [
                                Some(FlowErrorAction::RetrySave),
                                Some(FlowErrorAction::ContinueWithoutSaving),
                            ],
                        });
                    }
                }
                crate::lobby::QueueDecision::Cancelled { .. } => {
                    commit.next_flow = Some(ClientFlow::GameSelect);
                    commit.overlay = Some(OverlayCommit::Clear);
                }
                crate::lobby::QueueDecision::Rejected(reason) => match reason {
                    crate::lobby::QueueRejection::IncompatiblePassives => {
                        editor.inline_error = Some(
                            "Lightweight Frame and Reinforced Frame cannot be combined".to_string(),
                        );
                        editor.focused_field = super::BuildEditorField::PassiveTwo;
                        commit.focus_index = Some(build_editor_field_focus_index(
                            super::BuildEditorField::PassiveTwo,
                        ));
                        if !editor.is_open {
                            editor.is_open = true;
                        }
                        commit.overlay = Some(OverlayCommit::BuildEditor);
                    }
                    crate::lobby::QueueRejection::OverBudget { used, budget } => {
                        editor.inline_error = Some(format!("Build uses {used} of {budget} points"));
                        editor.focused_field = editor
                            .last_edited_field
                            .unwrap_or(super::BuildEditorField::Power);
                        commit.focus_index =
                            Some(build_editor_field_focus_index(editor.focused_field));
                        editor.is_open = true;
                        commit.overlay = Some(OverlayCommit::BuildEditor);
                    }
                    crate::lobby::QueueRejection::StaleCatalog
                    | crate::lobby::QueueRejection::StaleGameConfiguration
                    | crate::lobby::QueueRejection::UnknownGameType
                    | crate::lobby::QueueRejection::ProtocolFailure => {
                        commit.teardown = true;
                        fail_to_server_select_with_kind(
                            &mut commit,
                            FlowErrorKind::Content,
                            "The lobby content changed incompatibly; reconnect to obtain a fresh game list"
                                .to_string(),
                            true,
                        );
                    }
                    crate::lobby::QueueRejection::RateLimited { retry_after_millis } => {
                        commit.error = Some(FlowError {
                            kind: FlowErrorKind::Queue,
                            message: format!(
                                "Queue commands are temporarily limited; try again in {retry_after_millis} ms",
                            ),
                            return_flow: *flow.get(),
                            actions: [
                                Some(FlowErrorAction::TryAgainQueue),
                                Some(FlowErrorAction::Disconnect),
                            ],
                        });
                    }
                    crate::lobby::QueueRejection::MustCancelFirst => {
                        commit.error = Some(FlowError {
                            kind: FlowErrorKind::Queue,
                            message:
                                "Cancel the current queue ticket before changing game or build"
                                    .to_string(),
                            return_flow: *flow.get(),
                            actions: [Some(FlowErrorAction::Back), None],
                        });
                    }
                    crate::lobby::QueueRejection::TicketMismatch
                    | crate::lobby::QueueRejection::StaleRequest
                    | crate::lobby::QueueRejection::TemporarilyUnavailable
                    | crate::lobby::QueueRejection::InternalBuildResolution
                    | crate::lobby::QueueRejection::ServerMatchCapacityOccupied => {
                        commit.error = Some(FlowError {
                            kind: FlowErrorKind::Queue,
                            message: "The queue request could not be completed".to_string(),
                            return_flow: *flow.get(),
                            actions: [Some(FlowErrorAction::Disconnect), None],
                        });
                    }
                },
            },
        }
        return;
    }
    let Some(action) = actions.ordinary.take() else {
        return;
    };
    match action {
        FlowUiAction::EditAddress => {
            model.editing = Some(EditingField::Address);
            model.caret = model.address.len();
        }
        FlowUiAction::EditName => {
            model.editing = Some(EditingField::Name);
            model.caret = model.name.len();
            if matches!(overlay.as_ref(), ClientOverlay::Error(_)) {
                commit.overlay = Some(OverlayCommit::Clear);
                commit.refresh_server_select = Some(1);
            }
        }
        FlowUiAction::Connect => match validate_target(&model.address, &model.name) {
            Ok(target) => {
                model.address = target.logical_address.canonical().to_string();
                model.name.clone_from(&target.proposed_display_name);
                model
                    .committed_name
                    .clone_from(&target.proposed_display_name);
                model.inline_error = None;
                commit.start_target = Some(target);
                commit.next_flow = Some(ClientFlow::Connecting);
                commit.overlay = Some(OverlayCommit::Clear);
            }
            Err(error) => model.inline_error = Some(error),
        },
        FlowUiAction::JoinSaved(address) => match validate_target(&address, &model.name) {
            Ok(target) => {
                model.address = target.logical_address.canonical().to_string();
                commit.start_target = Some(target);
                commit.next_flow = Some(ClientFlow::Connecting);
            }
            Err(error) => model.inline_error = Some(error),
        },
        FlowUiAction::RemoveFavorite(address) => {
            let removed_index = persistence
                .state
                .favorites
                .iter()
                .position(|favorite| favorite.address == address);
            if persistence.state.remove_favorite(&address) {
                if let Err(error) = save_connections(&path.0, &persistence.state) {
                    persistence.dirty_error = Some(error);
                }
                commit.refresh_server_select = Some(favorite_focus_after_removal(
                    removed_index,
                    persistence.state.favorites.len(),
                ));
            }
        }
        FlowUiAction::Back => commit.next_flow = Some(ClientFlow::Title),
        FlowUiAction::Retry => match validate_target(&model.address, &model.name) {
            Ok(target) => {
                commit.start_target = Some(target);
                commit.next_flow = Some(ClientFlow::Connecting);
            }
            Err(error) => model.inline_error = Some(error),
        },
        FlowUiAction::RetrySave => {
            let result = if *flow.get() == ClientFlow::Queue {
                super::save_build(
                    &build_path.0,
                    super::BuildFileV1::new(builds.0.balance_revision, editor.loaded_selection),
                    &builds.0,
                    &weapons.0,
                )
            } else {
                save_connections(&path.0, &persistence.state)
            };
            match result {
                Ok(()) => {
                    persistence.dirty_error = None;
                    commit.overlay = Some(OverlayCommit::Clear);
                }
                Err(error) => persistence.dirty_error = Some(error),
            }
        }
        FlowUiAction::ContinueWithoutSaving => {
            persistence.dirty_error = None;
            commit.overlay = Some(OverlayCommit::Clear);
        }
        FlowUiAction::DismissError => {
            commit.overlay = Some(OverlayCommit::Clear);
            if *flow.get() == ClientFlow::ServerSelect {
                commit.refresh_server_select = Some(2);
            }
        }
        FlowUiAction::SelectGame(index) => {
            if let Some((membership, _, _)) = membership.iter().next()
                && let Some(game_type) = membership.game_types.get(index)
            {
                selection.catalog_revision = Some(membership.catalog_revision);
                selection.game_type_id = Some(game_type.id.clone());
                selection.configuration_revision = Some(game_type.configuration_revision);
            }
        }
        FlowUiAction::ToggleFavorite => {
            if let Some((membership, target, _)) = membership.iter().next() {
                let Some(target) = target else {
                    return;
                };
                if persistence
                    .state
                    .favorites
                    .iter()
                    .any(|favorite| favorite.address == target.logical_address)
                {
                    persistence.state.remove_favorite(&target.logical_address);
                } else if let Err(error) = persistence
                    .state
                    .add_favorite(&membership.server_name, &target.logical_address)
                {
                    persistence.dirty_error = Some(error);
                    return;
                }
                if let Err(error) = save_connections(&path.0, &persistence.state) {
                    persistence.dirty_error = Some(error);
                }
            }
        }
        FlowUiAction::OpenBuildEditor => {
            if queue.snapshot().is_some_and(|snapshot| {
                snapshot.formation_availability
                    == crate::lobby::FormationAvailability::ProductMatchOccupied
            }) {
                return;
            }
            editor.open();
            commit.overlay = Some(OverlayCommit::BuildEditor);
        }
        FlowUiAction::ChooseBuild(index) => {
            editor.selected_choice = index.min(4);
            editor.inline_error = None;
        }
        FlowUiAction::FocusBuildField(index) => {
            editor.selected_choice = 4;
            editor.focused_field = super::BuildEditorField::from_index(index);
            editor.inline_error = None;
        }
        FlowUiAction::ChooseBuildFieldValue {
            field_index,
            value_index,
        } => {
            editor.selected_choice = 4;
            editor.set_field_value(
                super::BuildEditorField::from_index(field_index),
                value_index,
            );
        }
        FlowUiAction::JoinQueue => {
            let draft = editor.selection(&builds.0);
            match super::resolve_build_preview(draft, &builds.0, &weapons.0) {
                Ok(_) => {
                    let candidate = crate::builds::BuildCandidate {
                        build_revision: builds.0.balance_revision,
                        selection: draft,
                    };
                    if queue.start_join(&selection, candidate, time.elapsed()) {
                        editor.submitted_selection = Some(draft);
                        editor.inline_error = None;
                    }
                }
                Err(error) => {
                    editor.inline_error = Some(super::build_editor::build_error_copy(&error));
                }
            }
        }
        FlowUiAction::QueueAgain => {
            let current_lobby = membership
                .iter()
                .find(|(_, _, session)| session.kind == super::RoutedClientSessionKind::Lobby);
            if let Some((_, _, session)) = current_lobby {
                queue.bind_lobby_generation(session.generation);
            }
            let game_type_id = result_state
                .context
                .as_ref()
                .and_then(|context| context.game_type_id.clone())
                .or_else(|| result_state.last_accepted_game_type_id.clone())
                .or_else(|| queue.last_accepted_game_type_id().cloned())
                .or_else(|| selection.game_type_id.clone());
            let started = current_lobby.zip(game_type_id).is_some_and(
                |((membership, _, session), game_type_id)| {
                    let started = queue.start_requeue_join(
                        session.generation,
                        membership,
                        &game_type_id,
                        editor.loaded_selection,
                        builds.0.balance_revision,
                        time.elapsed(),
                    );
                    if started {
                        selection.catalog_revision = Some(membership.catalog_revision);
                        selection.game_type_id = Some(game_type_id);
                        selection.configuration_revision = membership
                            .game_types
                            .iter()
                            .find(|game| selection.game_type_id.as_ref() == Some(&game.id))
                            .map(|game| game.configuration_revision);
                    }
                    started
                },
            );
            if !started && queue.pending().is_none() {
                commit.error = Some(FlowError {
                    kind: FlowErrorKind::Queue,
                    message: "The queue connection is unavailable.".to_string(),
                    return_flow: ClientFlow::Results,
                    actions: [Some(FlowErrorAction::Back), None],
                });
            }
        }
        FlowUiAction::ChangeGame => {
            result_state.context = None;
            commit.next_flow = Some(ClientFlow::GameSelect);
        }
        FlowUiAction::KeepPlaying | FlowUiAction::KeepLoading => {
            commit.overlay = Some(OverlayCommit::Clear);
            commit.focus_index = Some(0);
        }
        FlowUiAction::ConfirmLeaveMatch => {
            result_state.context = None;
            let _ = routed.request_return_to_lobby();
            commit.overlay = Some(OverlayCommit::Clear);
        }
        FlowUiAction::CancelQueue => {
            let _ = queue.start_cancel(time.elapsed());
        }
        FlowUiAction::RetryQueue => {
            if queue.retry_pending(time.elapsed()) {
                commit.overlay = queue
                    .pending()
                    .map(|pending| queue_recovery_overlay(&pending.command));
            }
        }
        FlowUiAction::TryAgainQueue => {
            if queue.try_again_after_rate_limit(time.elapsed()) {
                commit.overlay = queue
                    .pending()
                    .map(|pending| queue_recovery_overlay(&pending.command));
            }
        }
        FlowUiAction::RequestCancelMatchStart => {
            if let Some(active) = loading.active() {
                commit.overlay = Some(OverlayCommit::Confirmation(CancelMatchStartConfirmation {
                    reservation_id: active.reservation_id,
                    generation: 1,
                }));
                commit.focus_index = Some(0);
            }
        }
        FlowUiAction::ConfirmCancelMatchStart => {
            if loading.request_cancel() {
                commit.overlay = Some(OverlayCommit::Clear);
            }
        }
        FlowUiAction::CancelBuildEditor | FlowUiAction::Cancel | FlowUiAction::Disconnect => {}
    }
    let _ = flow;
}

fn queue_recovery_overlay(command: &crate::lobby::QueueCommand) -> OverlayCommit {
    match command {
        crate::lobby::QueueCommand::Join(_) => OverlayCommit::BuildEditor,
        crate::lobby::QueueCommand::Cancel(_) => OverlayCommit::Clear,
    }
}

const fn build_editor_field_focus_index(field: super::BuildEditorField) -> usize {
    BUILD_EDITOR_FIELD_BASE + field.index()
}

fn fail_to_server_select(commit: &mut FlowCommit, message: String, retryable: bool) {
    fail_to_server_select_with_kind(commit, FlowErrorKind::Connection, message, retryable);
}

fn fail_to_server_select_with_kind(
    commit: &mut FlowCommit,
    kind: FlowErrorKind,
    message: String,
    retryable: bool,
) {
    commit.next_flow = Some(ClientFlow::ServerSelect);
    commit.error = Some(FlowError {
        kind,
        message,
        return_flow: ClientFlow::ServerSelect,
        actions: if retryable {
            [
                Some(FlowErrorAction::RetryConnection),
                Some(FlowErrorAction::Back),
            ]
        } else {
            [Some(FlowErrorAction::Back), None]
        },
    });
}

fn rejection_flow_error(reason: ClientLobbyFailure) -> FlowError {
    let (message, actions) = match reason {
        ClientLobbyFailure::Rejected(crate::protocol::LobbyJoinRejection::InvalidName) => (
            "The server rejected the proposed display name".to_string(),
            [Some(FlowErrorAction::EditName), Some(FlowErrorAction::Back)],
        ),
        ClientLobbyFailure::Rejected(crate::protocol::LobbyJoinRejection::ServerFull) => (
            "The server lobby is full".to_string(),
            [
                Some(FlowErrorAction::RetryConnection),
                Some(FlowErrorAction::Back),
            ],
        ),
        ClientLobbyFailure::Rejected(rejection) => (
            format!("The server rejected compatibility: {rejection:?}"),
            [Some(FlowErrorAction::Back), None],
        ),
        ClientLobbyFailure::InvalidWelcome => (
            "The server sent an invalid or conflicting lobby welcome".to_string(),
            [Some(FlowErrorAction::Back), None],
        ),
    };
    FlowError {
        kind: FlowErrorKind::Connection,
        message,
        return_flow: ClientFlow::ServerSelect,
        actions,
    }
}

fn favorite_focus_after_removal(removed_index: Option<usize>, remaining: usize) -> usize {
    removed_index.map_or(0, |index| {
        if index < remaining {
            3 + index * 2
        } else if index > 0 {
            3 + (index - 1) * 2
        } else {
            0
        }
    })
}

fn edited_value(model: &ServerSelectModel, field: EditingField) -> &str {
    match field {
        EditingField::Address => &model.address,
        EditingField::Name => &model.name,
    }
}

fn edited_value_mut(model: &mut ServerSelectModel, field: EditingField) -> &mut String {
    match field {
        EditingField::Address => &mut model.address,
        EditingField::Name => &mut model.name,
    }
}

fn previous_caret(value: &str, caret: usize, field: EditingField) -> usize {
    if caret == 0 {
        return 0;
    }
    match field {
        EditingField::Address => caret.saturating_sub(1),
        EditingField::Name => value[..caret]
            .grapheme_indices(true)
            .next_back()
            .map_or(0, |(index, _)| index),
    }
}

fn next_caret(value: &str, caret: usize, field: EditingField) -> usize {
    if caret >= value.len() {
        return value.len();
    }
    match field {
        EditingField::Address => caret + 1,
        EditingField::Name => value[caret..]
            .grapheme_indices(true)
            .nth(1)
            .map_or(value.len(), |(index, _)| caret + index),
    }
}

fn insert_editor_text(model: &mut ServerSelectModel, field: EditingField, text: &str) {
    let allowed = match field {
        EditingField::Address => text.is_ascii() && !text.chars().any(char::is_control),
        EditingField::Name => !text.chars().any(|character| {
            character.is_control() || matches!(character, '\u{2028}' | '\u{2029}')
        }),
    };
    let maximum = match field {
        EditingField::Address => 255,
        EditingField::Name => 64,
    };
    if !allowed || edited_value(model, field).len().saturating_add(text.len()) > maximum {
        model.inline_error = Some("Text exceeds this field's bounds".to_string());
        return;
    }
    let caret = model.caret;
    edited_value_mut(model, field).insert_str(caret, text);
    model.caret = caret + text.len();
    model.inline_error = None;
}

fn validate_target(address: &str, name: &str) -> Result<ValidatedConnectionTarget, String> {
    Ok(ValidatedConnectionTarget {
        logical_address: parse_server_address(address)
            .map_err(|error| format!("Invalid server address: {error:?}"))?,
        proposed_display_name: crate::lobby::normalize_proposed_display_name(name)
            .map_err(|error| format!("Invalid display name: {error}"))?,
    })
}

fn attempt_deadline_expiry(
    now: Duration,
    pending: &PendingConnection,
) -> Option<AttemptDeadlineExpiry> {
    if pending.dns_deadline.is_some_and(|deadline| now > deadline) {
        Some(AttemptDeadlineExpiry::Dns)
    } else if now > pending.overall_deadline {
        Some(AttemptDeadlineExpiry::Overall)
    } else if pending
        .candidate_deadline
        .is_some_and(|deadline| now > deadline)
    {
        Some(AttemptDeadlineExpiry::Candidate)
    } else {
        None
    }
}

fn observation_for_expiry(expiry: AttemptDeadlineExpiry) -> SessionObservation {
    match expiry {
        AttemptDeadlineExpiry::Dns => SessionObservation::DnsTimedOut,
        AttemptDeadlineExpiry::Overall => SessionObservation::TimedOut,
        AttemptDeadlineExpiry::Candidate => SessionObservation::CandidateTimedOut,
    }
}

fn accepted_observation(
    now: Duration,
    pending: &PendingConnection,
    disconnected: bool,
) -> SessionObservation {
    if disconnected {
        SessionObservation::UnexpectedLoss
    } else if let Some(expiry) = attempt_deadline_expiry(now, pending) {
        observation_for_expiry(expiry)
    } else {
        SessionObservation::Accepted
    }
}

fn has_next_candidate(pending: &PendingConnection) -> bool {
    pending.current_candidate.saturating_add(1) < pending.candidates.len()
}

fn candidate_time_share(remaining: Duration, remaining_candidates: u32) -> Duration {
    debug_assert!(remaining_candidates > 0);
    remaining.div_f64(f64::from(remaining_candidates.max(1)))
}

fn netcode_timeout_ceiling(remaining: Duration) -> Duration {
    Duration::from_secs(
        remaining
            .as_secs()
            .saturating_add(u64::from(remaining.subsec_nanos() != 0))
            .max(1),
    )
}

fn bound_resolved_candidates(candidates: impl IntoIterator<Item = SocketAddr>) -> Vec<SocketAddr> {
    let mut unique = BTreeSet::new();
    candidates
        .into_iter()
        .filter(|address| unique.insert(*address))
        .take(MAX_RESOLVED_CANDIDATES)
        .collect()
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn teardown_session(
    mut commands: Commands,
    commit: Res<FlowCommit>,
    clients: Query<Entity, With<RoutedClientSession>>,
    mut routed: ResMut<RoutedClientLifecycle>,
) {
    if !commit.teardown {
        return;
    }
    for entity in &clients {
        commands.trigger(Disconnect { entity });
        commands.trigger(Unlink {
            entity,
            reason: UnlinkReason::UserRequested(None),
        });
        commands.entity(entity).despawn();
    }
    routed.phase = RoutedClientPhase::Disabled;
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "the flow commit phase coordinates runtime-owned Bevy resources"
)]
fn commit_flow(
    mut commands: Commands,
    time: Res<Time<Real>>,
    config: Res<ClientNetworkConfig>,
    mut generation: ResMut<ConnectionGeneration>,
    mut resolver: ResMut<ResolverState>,
    mut routed: ResMut<RoutedClientLifecycle>,
    pending: Option<ResMut<PendingConnection>>,
    commit: Res<FlowCommit>,
    mut next_flow: ResMut<NextState<ClientFlow>>,
    mut overlay: ResMut<ClientOverlay>,
    mut navigation: ResMut<FlowNavigation>,
) {
    if let Some(error) = &commit.error {
        *overlay = ClientOverlay::Error(error.clone());
    } else if let Some(overlay_commit) = commit.overlay {
        *overlay = match overlay_commit {
            OverlayCommit::Clear => ClientOverlay::None,
            OverlayCommit::BuildEditor => ClientOverlay::BuildEditor,
            OverlayCommit::Confirmation(value) => ClientOverlay::Confirmation(value),
        };
    }
    if let Some(index) = commit.focus_index {
        navigation.selected = index;
    }
    if let Some(target) = commit.start_target.clone() {
        generation.0 = generation.0.saturating_add(1).max(1);
        let now = time.elapsed();
        let mut connection = PendingConnection {
            generation: generation.0,
            target,
            candidates: Vec::new(),
            current_candidate: 0,
            overall_deadline: now.saturating_add(ATTEMPT_DEADLINE),
            dns_deadline: None,
            candidate_deadline: None,
            current_entity: None,
            stage: ConnectionStage::ResolvingAddress,
        };
        if let Some(socket) = connection.target.logical_address.numeric_socket() {
            connection.candidates.push(socket);
            connection.stage = ConnectionStage::ContactingServer {
                current: 1,
                total: 1,
            };
            spawn_current_candidate(&mut commands, &config, now, &mut routed, &mut connection);
        } else if resolver.task.is_some() {
            *overlay = ClientOverlay::Error(FlowError {
                kind: FlowErrorKind::Connection,
                message: "A previous operating-system address lookup is still busy".to_string(),
                return_flow: ClientFlow::ServerSelect,
                actions: [
                    Some(FlowErrorAction::RetryConnection),
                    Some(FlowErrorAction::Back),
                ],
            });
            next_flow.set(ClientFlow::ServerSelect);
            return;
        } else if let ServerAddressHost::Dns(host) = &connection.target.logical_address.host {
            let generation = connection.generation;
            let query = format!("{}:{}", host, connection.target.logical_address.port);
            resolver.task = Some(ResolverTask {
                generation,
                task: IoTaskPool::get().spawn(async move {
                    bound_resolved_candidates(
                        query
                            .to_socket_addrs()
                            .map_err(|error| format!("Address resolution failed: {error}"))?,
                    )
                    .pipe(Ok)
                }),
            });
            connection.dns_deadline = Some(now.saturating_add(DNS_DEADLINE));
        }
        commands.insert_resource(connection);
    } else if commit.advance_candidate
        && let Some(mut pending) = pending
    {
        if pending.current_entity.is_some() {
            pending.current_candidate = pending.current_candidate.saturating_add(1);
        }
        pending.current_entity = None;
        spawn_current_candidate(
            &mut commands,
            &config,
            time.elapsed(),
            &mut routed,
            &mut pending,
        );
    }
    if let Some(flow) = commit.next_flow {
        next_flow.set(flow);
        if flow != ClientFlow::Connecting && flow != ClientFlow::GameSelect {
            commands.remove_resource::<PendingConnection>();
        }
    }
}

fn spawn_current_candidate(
    commands: &mut Commands,
    config: &ClientNetworkConfig,
    now: Duration,
    routed: &mut RoutedClientLifecycle,
    pending: &mut PendingConnection,
) {
    let Some(server_addr) = pending.candidates.get(pending.current_candidate).copied() else {
        return;
    };
    let remaining_candidates = pending
        .candidates
        .len()
        .saturating_sub(pending.current_candidate)
        .max(1);
    let remaining = pending.overall_deadline.saturating_sub(now);
    let divisor = u32::try_from(remaining_candidates).expect("candidate bound fits u32");
    let share = candidate_time_share(remaining, divisor);
    pending.candidate_deadline = Some(now.saturating_add(share));
    pending.stage = ConnectionStage::ContactingServer {
        current: pending.current_candidate + 1,
        total: pending.candidates.len(),
    };
    pending.current_entity = spawn_product_lobby_connection(
        commands,
        config,
        routed,
        ProductLobbyAttempt {
            started_at: now,
            server_addr,
            logical_address: pending.target.logical_address.canonical().to_string(),
            proposed_display_name: pending.target.proposed_display_name.clone(),
            netcode_timeout: netcode_timeout_ceiling(remaining),
        },
    )
    .ok();
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn spawn_server_select(
    mut commands: Commands,
    model: Res<ServerSelectModel>,
    persistence: Res<ConnectionPersistence>,
    mut navigation: ResMut<FlowNavigation>,
) {
    spawn_server_select_root(&mut commands, &model, &persistence, &mut navigation, None);
}

fn spawn_server_select_root(
    commands: &mut Commands,
    model: &ServerSelectModel,
    persistence: &ConnectionPersistence,
    navigation: &mut FlowNavigation,
    requested_selection: Option<usize>,
) {
    navigation.selected = requested_selection.unwrap_or(2);
    commands
        .spawn((
            FlowRoot,
            DespawnOnExit(ClientFlow::ServerSelect),
            flow_root_node(),
            BackgroundColor(Color::srgb(0.025, 0.04, 0.07)),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            spawn_heading(root, "SERVER SELECT");
            spawn_flow_button(
                root,
                0,
                FlowUiAction::EditAddress,
                "",
                Some(FieldLabel::Address),
            );
            spawn_flow_button(root, 1, FlowUiAction::EditName, "", Some(FieldLabel::Name));
            spawn_flow_button(root, 2, FlowUiAction::Connect, "CONNECT", None);
            let mut index = 3;
            for favorite in &persistence.state.favorites {
                spawn_flow_button(
                    root,
                    index,
                    FlowUiAction::JoinSaved(favorite.address.clone()),
                    &format!("JOIN {} - {}", favorite.name, favorite.address),
                    None,
                );
                index += 1;
                spawn_flow_button(
                    root,
                    index,
                    FlowUiAction::RemoveFavorite(favorite.address.clone()),
                    &format!("REMOVE {}", favorite.name),
                    None,
                );
                index += 1;
            }
            for recent in &persistence.state.recents {
                spawn_flow_button(
                    root,
                    index,
                    FlowUiAction::JoinSaved(recent.address.clone()),
                    &format!("RECENT {} - {}", recent.server_name, recent.address),
                    None,
                );
                index += 1;
            }
            spawn_flow_button(root, index, FlowUiAction::Back, "BACK", None);
            if let Some(error) = model.inline_error.as_ref() {
                root.spawn((
                    Text::new(error.clone()),
                    TextColor(Color::srgb(1.0, 0.55, 0.45)),
                ));
            }
        });
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "the bounded rebuild phase owns one complete server-select root replacement"
)]
fn refresh_server_select(
    mut commands: Commands,
    commit: Res<FlowCommit>,
    flow: Res<State<ClientFlow>>,
    roots: Query<Entity, With<FlowRoot>>,
    model: Res<ServerSelectModel>,
    persistence: Res<ConnectionPersistence>,
    mut navigation: ResMut<FlowNavigation>,
) {
    let Some(selection) = commit.refresh_server_select else {
        return;
    };
    if *flow.get() != ClientFlow::ServerSelect {
        return;
    }
    for entity in &roots {
        commands.entity(entity).despawn();
    }
    spawn_server_select_root(
        &mut commands,
        &model,
        &persistence,
        &mut navigation,
        Some(selection),
    );
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn present_flow_error_overlay(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    roots: Query<(Entity, &FlowErrorRoot)>,
    mut navigation: ResMut<FlowNavigation>,
) {
    let ClientOverlay::Error(error) = overlay.as_ref() else {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    };
    if *flow.get() == ClientFlow::Title {
        return;
    }
    if error.return_flow != *flow.get() {
        return;
    }
    let matches_current = roots.iter().any(|(_, rendered)| rendered.0 == *error);
    if matches_current && roots.iter().count() == 1 {
        return;
    }
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    navigation.selected = ERROR_BUTTON_BASE;
    commands
        .spawn((
            FlowErrorRoot(error.clone()),
            DespawnOnExit(error.return_flow),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                display: Display::Flex,
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                padding: UiRect::all(px(24)),
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(500),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(88),
                    max_width: px(720),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(12),
                    padding: UiRect::all(px(24)),
                    border: UiRect::all(px(2)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.055, 0.08, 0.12)),
                BorderColor::all(Color::srgb(0.85, 0.3, 0.25)),
            ))
            .with_children(|panel| {
                spawn_heading(panel, error.kind.title());
                panel.spawn((
                    Text::new(error.message.clone()),
                    TextColor(Color::srgb(1.0, 0.72, 0.65)),
                ));
                for (offset, action) in error.actions.into_iter().flatten().enumerate() {
                    let (ui_action, label) = flow_error_action_button(action);
                    if action == FlowErrorAction::TryAgainQueue {
                        spawn_rate_limit_try_again_button(
                            panel,
                            ERROR_BUTTON_BASE + offset,
                            ui_action,
                        );
                    } else {
                        spawn_flow_error_button(
                            panel,
                            ERROR_BUTTON_BASE + offset,
                            ui_action,
                            label,
                        );
                    }
                }
            });
        });
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy presentation reads runtime-owned time and queue state"
)]
fn update_rate_limit_try_again(
    mut commands: Commands,
    time: Res<Time<Real>>,
    queue: Res<super::ClientQueueModel>,
    buttons: Query<Entity, With<RateLimitTryAgain>>,
    mut labels: Query<&mut Text, With<RateLimitTryAgainLabel>>,
) {
    let remaining = queue
        .pending()
        .and_then(|pending| pending.rate_limited_until)
        .map_or(Duration::ZERO, |deadline| {
            deadline.saturating_sub(time.elapsed())
        });
    let enabled = remaining.is_zero();
    for entity in &buttons {
        if enabled {
            commands.entity(entity).remove::<InteractionDisabled>();
        } else {
            commands.entity(entity).insert(InteractionDisabled);
        }
    }
    for mut label in &mut labels {
        label.0 = if enabled {
            "TRY AGAIN".to_string()
        } else {
            format!("TRY AGAIN IN {:.1}s", remaining.as_secs_f32())
        };
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "the product editor overlay renders one bounded draft from authored catalogs"
)]
fn present_build_editor_overlay(
    mut commands: Commands,
    overlay: Res<ClientOverlay>,
    flow: Res<State<ClientFlow>>,
    editor: Res<super::BuildEditorState>,
    builds: Res<crate::builds::BuildCatalogResource>,
    weapons: Res<crate::combat::WeaponCatalogResource>,
    selection: Res<SelectedGameType>,
    queue: Res<super::ClientQueueModel>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    roots: Query<(Entity, &ScrollPosition), With<BuildEditorRoot>>,
    mut navigation: ResMut<FlowNavigation>,
    mut rendered: Local<Option<BuildEditorRenderKey>>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::BuildEditor)
        || *flow.get() != ClientFlow::GameSelect
    {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        *rendered = None;
        return;
    }
    let joining = queue
        .pending()
        .is_some_and(|pending| matches!(pending.command, crate::lobby::QueueCommand::Join(_)));
    let capacity_occupied = queue.snapshot().is_some_and(|snapshot| {
        snapshot.formation_availability == crate::lobby::FormationAvailability::ProductMatchOccupied
    });
    let game_name = selected_game_name(&selection, memberships.iter().next()).to_string();
    let render_key = BuildEditorRenderKey {
        selected_choice: editor.selected_choice,
        custom_recipe: editor.custom_recipe,
        focused_field: editor.focused_field,
        inline_error: editor.inline_error.clone(),
        game_type_id: selection.game_type_id.clone(),
        game_name: game_name.clone(),
        joining,
        capacity_occupied,
    };
    if !roots.is_empty() && rendered.as_ref() == Some(&render_key) {
        return;
    }
    let first_spawn = roots.is_empty();
    let scroll = roots
        .iter()
        .next()
        .map_or_else(ScrollPosition::default, |(_, scroll)| scroll.clone());
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    if first_spawn && !is_build_editor_focus_index(navigation.selected) {
        navigation.selected = BUILD_EDITOR_CHOICE_BASE + editor.selected_choice;
    }
    let draft = editor.selection(&builds.0);
    let preview = super::resolve_build_preview(draft, &builds.0, &weapons.0);
    let budget_summary = super::build_editor::build_budget_summary(draft, &builds.0).ok();
    commands
        .spawn((
            BuildEditorRoot,
            scroll,
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                display: Display::Flex,
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Center,
                row_gap: px(8),
                padding: UiRect::all(px(20)),
                overflow: Overflow::scroll_y(),
                ..default()
            },
            BackgroundColor(Color::srgba(0.02, 0.035, 0.06, 0.98)),
            GlobalZIndex(480),
        ))
        .with_children(|root| {
            spawn_heading(root, "BUILD EDITOR");
            root.spawn((
                Text::new(format!("Game: {game_name}")),
                TextColor(Color::srgb(0.75, 0.85, 0.92)),
            ));
            for (index, preset) in builds.0.presets.iter().enumerate() {
                let points = super::resolve_build_preview(
                    crate::builds::BuildSelection::Preset(preset.id),
                    &builds.0,
                    &weapons.0,
                )
                .map_or(0, |preview| preview.total_points);
                spawn_build_editor_button(
                    root,
                    BUILD_EDITOR_CHOICE_BASE + index,
                    FlowUiAction::ChooseBuild(index),
                    &format!("{} — {points} points", preset.display_name),
                    false,
                );
            }
            spawn_build_editor_button(
                root,
                BUILD_EDITOR_CHOICE_BASE + 4,
                FlowUiAction::ChooseBuild(4),
                "CUSTOM — edit all six fields below",
                false,
            );
            if editor.selected_choice == 4 {
                for index in 0..6 {
                    spawn_build_editor_button(
                        root,
                        BUILD_EDITOR_FIELD_BASE + index,
                        FlowUiAction::FocusBuildField(index),
                        &custom_field_label(
                            super::BuildEditorField::from_index(index),
                            editor.custom_recipe,
                            &builds.0,
                        ),
                        false,
                    );
                }
                let field = editor.focused_field;
                root.spawn((
                    Text::new(format!("OPTIONS — {}", custom_field_name(field))),
                    TextColor(Color::srgb(0.25, 0.9, 1.0)),
                ));
                for value_index in 0..super::build_editor::custom_field_option_count(field) {
                    let Some(option) = super::build_editor::custom_field_option_label(
                        field,
                        value_index,
                        &builds.0,
                    ) else {
                        continue;
                    };
                    let detail = editor
                        .selection_with_field_value(field, value_index)
                        .map_or_else(
                            || "Unavailable".to_string(),
                            |alternative| {
                                super::compare_build_alternative(
                                    draft,
                                    alternative,
                                    &builds.0,
                                    &weapons.0,
                                )
                                .map_or_else(|error| error, |lines| lines.join(" · "))
                            },
                        );
                    spawn_build_editor_button(
                        root,
                        BUILD_EDITOR_OPTION_BASE + value_index,
                        FlowUiAction::ChooseBuildFieldValue {
                            field_index: field.index(),
                            value_index,
                        },
                        &format!("{option} — {detail}"),
                        false,
                    );
                }
            }
            match &preview {
                Ok(preview) => {
                    root.spawn((
                        Text::new(format!(
                            "{}\n\n{}",
                            budget_summary.as_deref().unwrap_or("Budget unavailable"),
                            preview.lines.join("\n"),
                        )),
                        TextFont::from_font_size(16.0),
                        TextColor(Color::srgb(0.86, 0.94, 1.0)),
                        TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                        Node {
                            width: percent(88),
                            max_width: px(820),
                            ..default()
                        },
                    ));
                }
                Err(error) => {
                    let copy = budget_summary.as_ref().map_or_else(
                        || super::build_editor::build_error_copy(error),
                        |budget| {
                            format!(
                                "{budget}\n\n{}",
                                super::build_editor::build_error_copy(error)
                            )
                        },
                    );
                    root.spawn((Text::new(copy), TextColor(Color::srgb(1.0, 0.55, 0.45))));
                }
            }
            if let Some(error) = &editor.inline_error {
                root.spawn((
                    Text::new(error.clone()),
                    TextColor(Color::srgb(1.0, 0.55, 0.45)),
                ));
            }
            spawn_build_editor_button(
                root,
                BUILD_EDITOR_JOIN_INDEX,
                FlowUiAction::JoinQueue,
                if joining { "JOINING..." } else { "JOIN QUEUE" },
                joining || preview.is_err() || capacity_occupied,
            );
            spawn_build_editor_button(
                root,
                BUILD_EDITOR_BACK_INDEX,
                FlowUiAction::CancelBuildEditor,
                "BACK",
                joining,
            );
            spawn_build_editor_button(
                root,
                BUILD_EDITOR_DISCONNECT_INDEX,
                FlowUiAction::Disconnect,
                "DISCONNECT",
                false,
            );
        });
    *rendered = Some(render_key);
}

fn selected_game_name<'a>(
    selection: &SelectedGameType,
    membership: Option<&'a ClientLobbyMembership>,
) -> &'a str {
    let Some(selected) = selection.game_type_id.as_ref() else {
        return "No game selected";
    };
    membership
        .and_then(|membership| {
            membership
                .game_types
                .iter()
                .find(|game| game.id == *selected)
        })
        .map_or("Selected game unavailable", |game| {
            game.display_name.as_str()
        })
}

fn scroll_build_editor(
    mut wheel: MessageReader<MouseWheel>,
    mut roots: Query<&mut ScrollPosition, With<BuildEditorRoot>>,
) {
    let delta = wheel
        .read()
        .map(|event| match event.unit {
            MouseScrollUnit::Line => event.y * 24.0,
            MouseScrollUnit::Pixel => event.y,
        })
        .sum::<f32>();
    if delta.abs() <= f32::EPSILON {
        return;
    }
    for mut position in &mut roots {
        position.0.y = (position.0.y - delta).max(0.0);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn keep_build_editor_focus_visible(
    navigation: Res<FlowNavigation>,
    buttons: Query<(&FlowButton, &ComputedNode, &UiGlobalTransform)>,
    mut roots: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            &mut ScrollPosition,
        ),
        With<BuildEditorRoot>,
    >,
    mut prior_focus: Local<Option<(usize, Entity)>>,
) {
    let Some(root_entity) = roots.iter_mut().next().map(|(entity, _, _, _)| entity) else {
        *prior_focus = None;
        return;
    };
    let focus_key = (navigation.selected, root_entity);
    if prior_focus.as_ref() == Some(&focus_key) {
        return;
    }
    *prior_focus = Some(focus_key);
    let Some((_, button_node, button_transform)) = buttons
        .iter()
        .find(|(button, _, _)| button.build_editor_action && button.index == navigation.selected)
    else {
        return;
    };
    if button_node.is_empty() {
        return;
    }
    let (_, _, button_center) = button_transform.to_scale_angle_translation();
    let button_half_height = button_node.size().y * 0.5;
    for (_, root_node, root_transform, mut scroll) in &mut roots {
        if root_node.is_empty() {
            continue;
        }
        let (_, _, root_center) = root_transform.to_scale_angle_translation();
        let root_half_height = root_node.size().y * 0.5;
        let top = root_center.y - root_half_height;
        let bottom = root_center.y + root_half_height;
        let button_top = button_center.y - button_half_height;
        let button_bottom = button_center.y + button_half_height;
        if button_top < top {
            scroll.0.y = (scroll.0.y - (top - button_top)).max(0.0);
        } else if button_bottom > bottom {
            scroll.0.y += button_bottom - bottom;
        }
    }
}

const fn custom_field_name(field: super::BuildEditorField) -> &'static str {
    match field {
        super::BuildEditorField::Power => "POWER",
        super::BuildEditorField::Reach => "REACH",
        super::BuildEditorField::Magazine => "MAGAZINE",
        super::BuildEditorField::Ultimate => "ULTIMATE",
        super::BuildEditorField::PassiveOne => "PASSIVE 1",
        super::BuildEditorField::PassiveTwo => "PASSIVE 2",
    }
}

const fn is_build_editor_focus_index(index: usize) -> bool {
    index >= BUILD_EDITOR_CHOICE_BASE && index <= BUILD_EDITOR_DISCONNECT_INDEX
}

fn spawn_build_editor_button(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    label: &str,
    disabled: bool,
) {
    let mut entity = parent.spawn((
        Button,
        FlowButton {
            index,
            action,
            error_action: false,
            build_editor_action: true,
        },
        Node {
            width: percent(88),
            max_width: px(820),
            min_height: px(40),
            padding: UiRect::axes(px(12), px(7)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(px(2)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.09, 0.14, 0.2)),
        BorderColor::all(Color::NONE),
    ));
    if disabled {
        entity.insert(InteractionDisabled);
    }
    entity.with_child((Text::new(label), TextFont::from_font_size(16.0)));
}

fn custom_field_label(
    field: super::BuildEditorField,
    recipe: crate::builds::BrawlerBuildRecipe,
    builds: &crate::builds::BuildCatalog,
) -> String {
    use crate::builds::{PulseMagazine, PulsePower, PulseReach, WeaponChoice};
    let WeaponChoice::CustomPulse {
        power,
        reach,
        magazine,
    } = recipe.weapon
    else {
        return "Custom weapon unavailable".to_string();
    };
    match field {
        super::BuildEditorField::Power => format!(
            "POWER: {} | Light +0 · Balanced +0 · Heavy +1",
            match power {
                PulsePower::Light => "Light",
                PulsePower::Balanced => "Balanced",
                PulsePower::Heavy => "Heavy",
            }
        ),
        super::BuildEditorField::Reach => format!(
            "REACH: {} | Compact +0 · Standard +0 · Long +1",
            match reach {
                PulseReach::Compact => "Compact",
                PulseReach::Standard => "Standard",
                PulseReach::Long => "Long",
            }
        ),
        super::BuildEditorField::Magazine => format!(
            "MAGAZINE: {} | Quick +0 · Standard +0 · Expanded +1",
            match magazine {
                PulseMagazine::Quick => "Quick",
                PulseMagazine::Standard => "Standard",
                PulseMagazine::Expanded => "Expanded",
            }
        ),
        super::BuildEditorField::Ultimate => {
            let selected = builds
                .ultimates
                .iter()
                .find(|definition| definition.id == recipe.ultimate)
                .map_or("Unknown", |definition| definition.display_name.as_str());
            format!(
                "ULTIMATE: {selected} | {}",
                builds
                    .ultimates
                    .iter()
                    .map(|definition| format!(
                        "{} {}pt",
                        definition.display_name, definition.point_cost
                    ))
                    .collect::<Vec<_>>()
                    .join(" · ")
            )
        }
        super::BuildEditorField::PassiveOne | super::BuildEditorField::PassiveTwo => {
            let slot = usize::from(matches!(field, super::BuildEditorField::PassiveTwo));
            let selected = builds
                .passives
                .iter()
                .find(|definition| definition.id == recipe.passives[slot])
                .map_or("Unknown", |definition| definition.display_name.as_str());
            format!(
                "PASSIVE {}: {selected} | {}",
                slot + 1,
                builds
                    .passives
                    .iter()
                    .map(|definition| format!(
                        "{} {}pt",
                        definition.display_name, definition.point_cost
                    ))
                    .collect::<Vec<_>>()
                    .join(" · ")
            )
        }
    }
}

fn flow_error_action_button(action: FlowErrorAction) -> (FlowUiAction, &'static str) {
    match action {
        FlowErrorAction::RetryConnection => (FlowUiAction::Retry, "RETRY"),
        FlowErrorAction::EditName => (FlowUiAction::EditName, "EDIT NAME"),
        FlowErrorAction::Back => (FlowUiAction::DismissError, "BACK"),
        FlowErrorAction::RetrySave => (FlowUiAction::RetrySave, "RETRY SAVE"),
        FlowErrorAction::ContinueWithoutSaving => (
            FlowUiAction::ContinueWithoutSaving,
            "CONTINUE WITHOUT SAVING",
        ),
        FlowErrorAction::ContinueWithDefaults => {
            (FlowUiAction::DismissError, "CONTINUE WITH DEFAULTS")
        }
        FlowErrorAction::RetryQueue => (FlowUiAction::RetryQueue, "RETRY"),
        FlowErrorAction::TryAgainQueue => (FlowUiAction::TryAgainQueue, "TRY AGAIN"),
        FlowErrorAction::Disconnect => (FlowUiAction::Disconnect, "DISCONNECT"),
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn spawn_connecting(
    mut commands: Commands,
    pending: Option<Res<PendingConnection>>,
    mut navigation: ResMut<FlowNavigation>,
) {
    navigation.selected = 0;
    let address = pending.as_ref().map_or("server", |pending| {
        pending.target.logical_address.canonical()
    });
    commands
        .spawn((
            FlowRoot,
            DespawnOnExit(ClientFlow::Connecting),
            flow_root_node(),
            BackgroundColor(Color::srgb(0.025, 0.04, 0.07)),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            spawn_heading(root, "CONNECTING");
            root.spawn((
                Node {
                    width: percent(88),
                    max_width: px(720),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(18),
                    padding: UiRect::all(px(28)),
                    border: UiRect::all(px(2)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.055, 0.08, 0.12)),
                BorderColor::all(Color::srgb(0.12, 0.32, 0.42)),
            ))
            .with_children(|panel| {
                panel.spawn((
                    ConnectingLabel,
                    Text::new(format!("PREPARING CONNECTION\n{address}")),
                    TextFont::from_font_size(22.0),
                    TextColor(Color::srgb(0.9, 0.95, 1.0)),
                    TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                ));
                spawn_flow_button(panel, 0, FlowUiAction::Cancel, "CANCEL", None);
                panel.spawn((
                    Text::new("ESC / PAD EAST  -  CANCEL"),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::srgb(0.58, 0.66, 0.74)),
                ));
            });
        });
}

#[allow(
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "the bounded game-select root renders the complete catalog card contract"
)]
fn spawn_game_select(
    mut commands: Commands,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    mut navigation: ResMut<FlowNavigation>,
    mut selection: ResMut<SelectedGameType>,
    queue: Res<super::ClientQueueModel>,
) {
    let Some(membership) = memberships.iter().next() else {
        return;
    };
    let selected_index = selection
        .game_type_id
        .as_ref()
        .and_then(|selected| {
            membership
                .game_types
                .iter()
                .position(|game| game.id == *selected)
        })
        .unwrap_or(0);
    navigation.selected = selected_index;
    if let Some(selected) = membership.game_types.get(selected_index) {
        selection.catalog_revision = Some(membership.catalog_revision);
        selection.game_type_id = Some(selected.id.clone());
        selection.configuration_revision = Some(selected.configuration_revision);
    }
    let map_catalog = crate::map::MapContentCatalog::embedded().ok();
    commands
        .spawn((
            FlowRoot,
            DespawnOnExit(ClientFlow::GameSelect),
            flow_root_node(),
            BackgroundColor(Color::srgb(0.025, 0.04, 0.07)),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            spawn_heading(root, &membership.server_name);
            root.spawn(Text::new(format!(
                "SIGNED IN AS {}",
                membership.accepted_display_name
            )));
            for (index, game_type) in membership.game_types.iter().enumerate() {
                let mode_name =
                    if game_type.mode_definition_id == crate::map::WIPEOUT_MODE_DEFINITION {
                        "Wipeout"
                    } else if game_type.mode_definition_id == crate::map::HOT_ZONE_MODE_DEFINITION {
                        "Hot Zone"
                    } else {
                        "Unknown mode"
                    };
                let map_names = game_type
                    .map_preset_ids
                    .iter()
                    .map(|id| {
                        map_catalog
                            .as_ref()
                            .and_then(|catalog| {
                                catalog.presets.iter().find(|preset| preset.id == *id)
                            })
                            .map_or_else(
                                || format!("Map {}", id.0),
                                |preset| preset.display_name.clone(),
                            )
                    })
                    .collect::<Vec<_>>()
                    .join(", ");
                let rules = match game_type.rules_summary {
                    crate::lobby::AdvertisedRulesSummary::Wipeout {
                        target_score,
                        active_limit_ticks,
                    } => format!(
                        "first to {target_score}; {}s limit",
                        active_limit_ticks / 60
                    ),
                    crate::lobby::AdvertisedRulesSummary::HotZone {
                        target_progress_ticks,
                        active_limit_ticks,
                    } => format!(
                        "hold {}s; {}s limit",
                        target_progress_ticks / 60,
                        active_limit_ticks / 60
                    ),
                };
                spawn_flow_button(
                    root,
                    index,
                    FlowUiAction::SelectGame(index),
                    &format!(
                        "{} | {mode_name} | {}v{} | {map_names} | {rules}",
                        game_type.display_name,
                        game_type.players_per_team,
                        game_type.players_per_team,
                    ),
                    None,
                );
                root.spawn((
                    GamePopulationLabel(index),
                    Text::new(queue_population(&queue, game_type)),
                    TextFont::from_font_size(15.0),
                    TextColor(Color::srgb(0.68, 0.78, 0.86)),
                ));
            }
            let offset = membership.game_types.len();
            let capacity_occupied = queue.snapshot().is_some_and(|snapshot| {
                snapshot.formation_availability
                    == crate::lobby::FormationAvailability::ProductMatchOccupied
            });
            spawn_flow_button(
                root,
                offset,
                FlowUiAction::OpenBuildEditor,
                if capacity_occupied {
                    "MATCH IN PROGRESS"
                } else {
                    "BUILD & JOIN"
                },
                None,
            );
            spawn_flow_button(
                root,
                offset + 1,
                FlowUiAction::ToggleFavorite,
                "FAVORITE / UNFAVORITE CURRENT SERVER",
                None,
            );
            spawn_flow_button(
                root,
                offset + 2,
                FlowUiAction::Disconnect,
                "DISCONNECT",
                None,
            );
        });
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn spawn_queue(
    mut commands: Commands,
    queue: Res<super::ClientQueueModel>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    builds: Res<crate::builds::BuildCatalogResource>,
    mut navigation: ResMut<FlowNavigation>,
) {
    let Some(membership) = queue.membership() else {
        return;
    };
    navigation.selected = 0;
    let lobby = memberships.iter().next();
    commands
        .spawn((
            FlowRoot,
            DespawnOnExit(ClientFlow::Queue),
            flow_root_node(),
            BackgroundColor(Color::srgb(0.025, 0.04, 0.07)),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            spawn_heading(root, "QUEUE");
            root.spawn((
                QueueStatusLabel,
                Text::new(queue_membership_text(&queue, membership, lobby, &builds.0)),
                TextFont::from_font_size(20.0),
                TextColor(Color::srgb(0.86, 0.94, 1.0)),
                TextLayout::new(Justify::Center, LineBreak::WordBoundary),
            ));
            spawn_queue_cancel_button(root);
            spawn_flow_button(root, 1, FlowUiAction::Disconnect, "DISCONNECT", None);
        });
}

#[allow(clippy::needless_pass_by_value)]
fn spawn_match_loading(
    mut commands: Commands,
    loading: Res<super::ClientMatchLoadingModel>,
    mut navigation: ResMut<FlowNavigation>,
) {
    let Some(active) = loading.active() else {
        return;
    };
    navigation.selected = 0;
    commands
        .spawn((
            FlowRoot,
            DespawnOnExit(ClientFlow::MatchLoading),
            flow_root_node(),
            BackgroundColor(Color::srgb(0.025, 0.04, 0.07)),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            spawn_heading(root, "MATCH LOADING");
            root.spawn((
                MatchLoadingStatusLabel,
                Text::new(match_loading_text(active, loading.phase())),
                TextFont::from_font_size(20.0),
                TextColor(Color::srgb(0.86, 0.94, 1.0)),
                TextLayout::new(Justify::Center, LineBreak::WordBoundary),
            ));
            spawn_flow_button(
                root,
                0,
                FlowUiAction::RequestCancelMatchStart,
                "CANCEL MATCH START",
                None,
            );
            spawn_flow_button(root, 1, FlowUiAction::Disconnect, "DISCONNECT", None);
        });
}

fn match_loading_text(
    active: &crate::lobby::ReservationStarted,
    phase: Option<crate::lobby::MatchLoadingPhase>,
) -> String {
    let phase = match phase.unwrap_or(crate::lobby::MatchLoadingPhase::Reserving) {
        crate::lobby::MatchLoadingPhase::Reserving => "Reserving roster",
        crate::lobby::MatchLoadingPhase::StartingServer => "Starting server",
        crate::lobby::MatchLoadingPhase::Connecting => "Connecting",
        crate::lobby::MatchLoadingPhase::Synchronizing => "Synchronizing map and terrain",
        crate::lobby::MatchLoadingPhase::WaitingForPlayers => "Waiting for players",
        crate::lobby::MatchLoadingPhase::Cancelling => "Cancelling",
        crate::lobby::MatchLoadingPhase::ReturningToQueue => "Returning to queue",
    };
    format!(
        "{phase}\n{}v{} · Map {}\nYour accepted build: {}/12 points",
        active.players_per_team,
        active.players_per_team,
        active.map_preset_id.0,
        active.accepted_build.total_points
    )
}

#[allow(clippy::needless_pass_by_value)]
fn update_match_loading(
    loading: Res<super::ClientMatchLoadingModel>,
    mut labels: Query<&mut Text, With<MatchLoadingStatusLabel>>,
) {
    let Some(active) = loading.active() else {
        return;
    };
    for mut label in &mut labels {
        label.0 = match_loading_text(active, loading.phase());
    }
}

#[allow(clippy::needless_pass_by_value)]
fn present_cancel_confirmation(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    roots: Query<Entity, With<CancelConfirmationRoot>>,
    mut navigation: ResMut<FlowNavigation>,
) {
    let ClientOverlay::Confirmation(_) = overlay.as_ref() else {
        for entity in &roots {
            commands.entity(entity).despawn();
        }
        return;
    };
    if roots.iter().next().is_some() || *flow.get() != ClientFlow::MatchLoading {
        return;
    }
    navigation.selected = 0;
    commands
        .spawn((
            CancelConfirmationRoot,
            DespawnOnExit(ClientFlow::MatchLoading),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(500),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(88),
                    max_width: px(640),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(12),
                    padding: UiRect::all(px(24)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.055, 0.08, 0.12)),
            ))
            .with_children(|panel| {
                spawn_heading(panel, "CANCEL MATCH START?");
                spawn_flow_error_button(panel, 0, FlowUiAction::KeepLoading, "KEEP LOADING");
                spawn_flow_error_button(
                    panel,
                    1,
                    FlowUiAction::ConfirmCancelMatchStart,
                    "CANCEL MATCH START",
                );
            });
        });
}

#[allow(clippy::needless_pass_by_value)]
fn present_leave_confirmation(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    roots: Query<Entity, With<LeaveConfirmationRoot>>,
    mut navigation: ResMut<FlowNavigation>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::LeaveConfirmation) {
        for entity in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    if roots.iter().next().is_some() || *flow.get() != ClientFlow::Match {
        return;
    }
    navigation.selected = 0;
    commands
        .spawn((
            LeaveConfirmationRoot,
            DespawnOnExit(ClientFlow::Match),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                align_items: AlignItems::Center,
                justify_content: JustifyContent::Center,
                ..default()
            },
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.72)),
            GlobalZIndex(500),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(88),
                    max_width: px(640),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(12),
                    padding: UiRect::all(px(24)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.055, 0.08, 0.12)),
            ))
            .with_children(|panel| {
                spawn_heading(panel, "LEAVE MATCH?");
                spawn_flow_error_button(panel, 0, FlowUiAction::KeepPlaying, "KEEP PLAYING");
                spawn_flow_error_button(panel, 1, FlowUiAction::ConfirmLeaveMatch, "LEAVE MATCH");
            });
        });
}

#[allow(clippy::needless_pass_by_value)]
fn present_match_completion(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    matches: Query<&crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>,
    roots: Query<Entity, With<MatchCompletionRoot>>,
) {
    if *flow.get() != ClientFlow::Match || roots.iter().next().is_some() {
        return;
    }
    let Some(result) = matches.iter().find_map(|state| match state.phase {
        crate::matchplay::MatchPhase::Completed { result, .. } => Some(result),
        _ => None,
    }) else {
        return;
    };
    let result = match result {
        crate::matchplay::MatchResult::TeamVictory { team } => {
            format!("TEAM {} WINS", team.0 + 1)
        }
        crate::matchplay::MatchResult::Draw => "DRAW".to_string(),
        crate::matchplay::MatchResult::Forfeit { winner, .. } => {
            format!("TEAM {} WINS BY FORFEIT", winner.0 + 1)
        }
    };
    commands
        .spawn((
            MatchCompletionRoot,
            DespawnOnExit(ClientFlow::Match),
            flow_root_node(),
            BackgroundColor(Color::srgba(0.025, 0.04, 0.07, 0.96)),
            GlobalZIndex(450),
        ))
        .with_children(|root| {
            spawn_heading(root, "MATCH COMPLETE");
            root.spawn((
                Text::new(result),
                TextFont::from_font_size(28.0),
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
            ));
            root.spawn((
                Text::new("RETURNING TO LOBBY…"),
                TextFont::from_font_size(18.0),
                TextColor(Color::srgb(0.58, 0.66, 0.74)),
            ));
        });
}

#[allow(clippy::needless_pass_by_value)]
fn spawn_results(
    mut commands: Commands,
    result_state: Res<super::ClientMatchResultState>,
    mut navigation: ResMut<FlowNavigation>,
) {
    let Some(context) = result_state.context.as_ref() else {
        return;
    };
    navigation.selected = 0;
    let outcome = match context.result {
        crate::matchplay::MatchResult::TeamVictory { team } => {
            if context.local_team == Some(team) {
                "VICTORY".to_string()
            } else if context.local_team.is_some() {
                "DEFEAT".to_string()
            } else {
                format!("TEAM {} WINS", team.0 + 1)
            }
        }
        crate::matchplay::MatchResult::Draw => "DRAW".to_string(),
        crate::matchplay::MatchResult::Forfeit { winner, .. } => {
            if context.local_team == Some(winner) {
                "VICTORY BY FORFEIT".to_string()
            } else if context.local_team.is_some() {
                "DEFEAT BY FORFEIT".to_string()
            } else {
                format!("TEAM {} WINS BY FORFEIT", winner.0 + 1)
            }
        }
    };
    commands
        .spawn((
            FlowRoot,
            DespawnOnExit(ClientFlow::Results),
            flow_root_node(),
            BackgroundColor(Color::srgb(0.025, 0.04, 0.07)),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            spawn_heading(root, "RESULTS");
            root.spawn((
                Text::new(outcome),
                TextFont::from_font_size(30.0),
                TextColor(Color::srgb(0.9, 0.95, 1.0)),
            ));
            if let Some(name) = context.game_name.as_deref() {
                root.spawn((
                    Text::new(name.to_string()),
                    TextColor(Color::srgb(0.68, 0.78, 0.86)),
                ));
            }
            if let Some(team) = context.local_team {
                root.spawn((
                    Text::new(format!("YOU — T{}", team.0 + 1)),
                    TextColor(Color::srgb(0.85, 0.9, 0.96)),
                ));
            }
            if let Some(score) = context.final_score {
                root.spawn((
                    Text::new(super::hud::mode_score_text(score)),
                    TextFont::from_font_size(22.0),
                    TextColor(Color::WHITE),
                    TextLayout::new(Justify::Center, LineBreak::WordBoundary),
                ));
            }
            spawn_flow_button(root, 0, FlowUiAction::QueueAgain, "QUEUE AGAIN", None);
            spawn_flow_button(root, 1, FlowUiAction::ChangeGame, "CHANGE GAME", None);
            spawn_flow_button(root, 2, FlowUiAction::Disconnect, "DISCONNECT", None);
        });
}

fn clear_results(mut result_state: ResMut<super::ClientMatchResultState>) {
    result_state.context = None;
}

fn queue_population(
    queue: &super::ClientQueueModel,
    game: &crate::lobby::AdvertisedGameType,
) -> String {
    queue
        .snapshot()
        .and_then(|snapshot| {
            snapshot
                .pools
                .iter()
                .find(|row| row.game_type_id == game.id)
        })
        .map_or_else(
            || "Updating queue".to_string(),
            |row| {
                format!(
                    "{} waiting · {} players per match",
                    row.queued, row.formation_size
                )
            },
        )
}

fn queue_membership_text(
    queue: &super::ClientQueueModel,
    membership: &crate::lobby::QueueMembership,
    lobby: Option<&ClientLobbyMembership>,
    builds: &crate::builds::BuildCatalog,
) -> String {
    let population = if queue.required_snapshot_is_fresh() {
        queue
            .raw_snapshot()
            .and_then(|snapshot| {
                snapshot
                    .pools
                    .iter()
                    .find(|row| row.game_type_id == membership.game_type_id)
            })
            .map_or_else(
                || "Updating queue".to_string(),
                |row| {
                    format!(
                        "{} waiting · {} players per match",
                        row.queued, row.formation_size
                    )
                },
            )
    } else {
        "Updating queue".to_string()
    };
    let game_name = lobby
        .and_then(|lobby| {
            lobby
                .game_types
                .iter()
                .find(|game| game.id == membership.game_type_id)
        })
        .map_or(membership.game_type_id.as_str(), |game| {
            game.display_name.as_str()
        });
    let build_name = membership
        .accepted_build
        .identity
        .source_build_preset_id
        .and_then(|id| builds.preset(id))
        .map_or("Custom", |preset| preset.display_name.as_str());
    let recipe = membership.accepted_build.canonical_recipe;
    let ultimate = builds
        .ultimates
        .iter()
        .find(|definition| definition.id == recipe.ultimate)
        .map_or("Unknown ultimate", |definition| {
            definition.display_name.as_str()
        });
    let passives = recipe.passives.map(|id| {
        builds
            .passives
            .iter()
            .find(|definition| definition.id == id)
            .map_or("Unknown passive", |definition| {
                definition.display_name.as_str()
            })
    });
    format!(
        "{game_name}\n{population}\nBuild: {build_name} · {} points\n{ultimate} · {} / {}",
        membership.accepted_build.total_points, passives[0], passives[1],
    )
}

fn spawn_queue_cancel_button(parent: &mut ChildSpawnerCommands) {
    parent
        .spawn((
            Button,
            QueueCancelButton,
            FlowButton {
                index: 0,
                action: FlowUiAction::CancelQueue,
                error_action: false,
                build_editor_action: false,
            },
            Node {
                width: percent(88),
                max_width: px(820),
                min_height: px(42),
                padding: UiRect::axes(px(12), px(8)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(2)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.09, 0.14, 0.2)),
            BorderColor::all(Color::NONE),
        ))
        .with_child((
            QueueCancelLabel,
            Text::new("CANCEL QUEUE"),
            TextFont::from_font_size(18.0),
        ));
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn update_queue_cancel_button(
    mut commands: Commands,
    queue: Res<super::ClientQueueModel>,
    buttons: Query<Entity, With<QueueCancelButton>>,
    mut labels: Query<&mut Text, With<QueueCancelLabel>>,
) {
    let (label, cancelling) =
        queue_cancel_presentation(queue.pending().map(|pending| &pending.command));
    for entity in &buttons {
        if cancelling {
            commands.entity(entity).insert(InteractionDisabled);
        } else {
            commands.entity(entity).remove::<InteractionDisabled>();
        }
    }
    for mut text in &mut labels {
        text.0 = label.to_string();
    }
}

fn queue_cancel_presentation(pending: Option<&crate::lobby::QueueCommand>) -> (&'static str, bool) {
    if pending.is_some_and(|command| matches!(command, crate::lobby::QueueCommand::Cancel(_))) {
        ("CANCELLING…", true)
    } else {
        ("CANCEL QUEUE", false)
    }
}

fn flow_root_node() -> Node {
    Node {
        width: percent(100),
        height: percent(100),
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::Center,
        row_gap: px(10),
        padding: UiRect::all(px(20)),
        overflow: Overflow::scroll_y(),
        ..default()
    }
}

fn spawn_heading(parent: &mut ChildSpawnerCommands, text: &str) {
    parent.spawn((
        Text::new(text),
        TextFont::from_font_size(38.0),
        TextColor(Color::srgb(0.25, 0.9, 1.0)),
    ));
}

fn spawn_flow_button(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    label: &str,
    field: Option<FieldLabel>,
) {
    let mut entity = parent.spawn((
        Button,
        FlowButton {
            index,
            action,
            error_action: false,
            build_editor_action: false,
        },
        Node {
            width: percent(88),
            max_width: px(820),
            min_height: px(42),
            padding: UiRect::axes(px(12), px(8)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(px(2)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.09, 0.14, 0.2)),
        BorderColor::all(Color::NONE),
    ));
    entity.with_children(|button| {
        let mut text = button.spawn((Text::new(label), TextFont::from_font_size(18.0)));
        if let Some(field) = field {
            text.insert(field);
        }
    });
}

fn spawn_flow_error_button(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    label: &str,
) {
    parent
        .spawn((
            Button,
            FlowButton {
                index,
                action,
                error_action: true,
                build_editor_action: false,
            },
            Node {
                width: percent(92),
                min_height: px(44),
                padding: UiRect::axes(px(12), px(8)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(2)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.16, 0.12, 0.15)),
            BorderColor::all(Color::NONE),
        ))
        .with_child((Text::new(label), TextFont::from_font_size(18.0)));
}

fn spawn_rate_limit_try_again_button(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
) {
    parent
        .spawn((
            Button,
            InteractionDisabled,
            RateLimitTryAgain,
            FlowButton {
                index,
                action,
                error_action: true,
                build_editor_action: false,
            },
            Node {
                width: percent(92),
                min_height: px(44),
                padding: UiRect::axes(px(12), px(8)),
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border: UiRect::all(px(2)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.1, 0.1, 0.12)),
            BorderColor::all(Color::NONE),
        ))
        .with_child((
            RateLimitTryAgainLabel,
            Text::new("TRY AGAIN"),
            TextFont::from_font_size(18.0),
        ));
}

#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "this presentation phase reads the complete bounded flow model"
)]
fn present_flow(
    time: Res<Time<Real>>,
    flow: Res<State<ClientFlow>>,
    model: Res<ServerSelectModel>,
    navigation: Res<FlowNavigation>,
    selection: Res<SelectedGameType>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    pending: Option<Res<PendingConnection>>,
    mut buttons: Query<(
        &FlowButton,
        &Interaction,
        Has<InteractionDisabled>,
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
    mut fields: Query<(&FieldLabel, &mut Text)>,
    mut connecting: Query<&mut Text, (With<ConnectingLabel>, Without<FieldLabel>)>,
    editor: Res<super::BuildEditorState>,
    queue: Res<super::ClientQueueModel>,
    builds: Res<crate::builds::BuildCatalogResource>,
    mut population_labels: Query<
        (&GamePopulationLabel, &mut Text),
        (
            Without<FieldLabel>,
            Without<ConnectingLabel>,
            Without<QueueStatusLabel>,
        ),
    >,
    mut queue_labels: Query<
        &mut Text,
        (
            With<QueueStatusLabel>,
            Without<FieldLabel>,
            Without<ConnectingLabel>,
            Without<GamePopulationLabel>,
        ),
    >,
) {
    for (button, interaction, disabled, mut background, mut border) in &mut buttons {
        let focused = button.index == navigation.selected;
        let selected_game = match button.action {
            FlowUiAction::SelectGame(index) => {
                memberships.iter().next().is_some_and(|membership| {
                    membership.game_types.get(index).is_some_and(|game_type| {
                        selection.game_type_id.as_ref() == Some(&game_type.id)
                    })
                })
            }
            _ => false,
        };
        let selected_build = matches!(
            button.action,
            FlowUiAction::ChooseBuild(index) if index == editor.selected_choice
        );
        background.0 = if disabled {
            Color::srgb(0.1, 0.1, 0.12)
        } else if *interaction == Interaction::Pressed {
            Color::srgb(0.08, 0.48, 0.58)
        } else if focused || *interaction == Interaction::Hovered {
            Color::srgb(0.12, 0.32, 0.42)
        } else if selected_game || selected_build {
            Color::srgb(0.12, 0.24, 0.34)
        } else {
            Color::srgb(0.09, 0.14, 0.2)
        };
        border.set_all(if disabled {
            Color::NONE
        } else if focused {
            Color::WHITE
        } else if selected_game || selected_build {
            Color::srgb(0.25, 0.9, 1.0)
        } else {
            Color::NONE
        });
    }
    if let Some(membership) = memberships.iter().next() {
        for (label, mut text) in &mut population_labels {
            if let Some(game) = membership.game_types.get(label.0) {
                text.0 = queue_population(&queue, game);
            }
        }
    }
    if let Some(membership) = queue.membership() {
        let lobby = memberships.iter().next();
        for mut text in &mut queue_labels {
            text.0 = queue_membership_text(&queue, membership, lobby, &builds.0);
        }
    }
    for (field, mut text) in &mut fields {
        text.0 = match field {
            FieldLabel::Address => format!(
                "ADDRESS: {}",
                render_editor_value(&model, EditingField::Address)
            ),
            FieldLabel::Name => {
                format!("NAME: {}", render_editor_value(&model, EditingField::Name))
            }
        };
    }
    if *flow.get() == ClientFlow::Connecting
        && let Some(pending) = pending
        && let Ok(mut text) = connecting.single_mut()
    {
        text.0 = connection_presentation(&pending, time.elapsed());
    }
}

fn connection_presentation(pending: &PendingConnection, now: Duration) -> String {
    let dots = "."
        .repeat(usize::try_from((now.as_millis() / 350) % 3 + 1).expect("pulse width is bounded"));
    let remaining = pending.overall_deadline.saturating_sub(now);
    let remaining_seconds = remaining
        .as_secs()
        .saturating_add(u64::from(remaining.subsec_nanos() != 0));
    let address = pending.target.logical_address.canonical();
    match pending.stage {
        ConnectionStage::ResolvingAddress => format!(
            "STEP 1 OF 3\nResolving server address{dots}\n{address}\nUp to {remaining_seconds}s remaining"
        ),
        ConnectionStage::ContactingServer { current, total } => format!(
            "STEP 2 OF 3\nOpening routed connection{dots}\n{address}\nCandidate {current} of {total}  -  up to {remaining_seconds}s remaining"
        ),
        ConnectionStage::JoiningLobby => format!(
            "STEP 3 OF 3\nChecking compatibility and game list{dots}\n{address}\nUp to {remaining_seconds}s remaining"
        ),
    }
}

fn render_editor_value(model: &ServerSelectModel, field: EditingField) -> String {
    let value = edited_value(model, field);
    if model.editing != Some(field) {
        return value.to_string();
    }
    let caret = model.caret.min(value.len());
    format!("{}|{}", &value[..caret], &value[caret..])
}

trait Pipe: Sized {
    fn pipe<T>(self, function: impl FnOnce(Self) -> T) -> T {
        function(self)
    }
}

impl<T> Pipe for T {}

#[cfg(test)]
mod tests {
    use super::*;

    fn flow_test_app() -> App {
        let mut app = App::new();
        let mut config = ClientNetworkConfig::new(0x1234);
        config.transport = crate::config::NetworkTransport::RoutedUdp;
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin))
            .add_message::<KeyboardInput>()
            .add_message::<MouseWheel>()
            .insert_resource(ButtonInput::<KeyCode>::default())
            .insert_resource(config)
            .insert_resource(RoutedClientLifecycle::default())
            .insert_resource(ClientConnectionsPath(std::env::temp_dir().join(format!(
                "brawler-m03-flow-test-{}-connections.ron",
                std::process::id()
            ))))
            .add_plugins(ClientFlowPlugin);
        app.update();
        app
    }

    fn count_flow_roots(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<FlowRoot>>();
        query.iter(world).count()
    }

    fn count_error_roots(app: &mut App) -> usize {
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<FlowErrorRoot>>();
        query.iter(world).count()
    }

    fn visible_text(app: &mut App) -> Vec<String> {
        let world = app.world_mut();
        let mut query = world.query::<&Text>();
        query.iter(world).map(|text| text.0.clone()).collect()
    }

    fn lobby_membership() -> ClientLobbyMembership {
        ClientLobbyMembership {
            player_id: crate::protocol::PlayerId(1),
            accepted_display_name: "Player".to_string(),
            server_name: "Test Lobby".to_string(),
            catalog_revision: crate::lobby::CatalogRevision([1; 32]),
            game_types: vec![crate::lobby::AdvertisedGameType {
                id: crate::lobby::GameTypeId::new("wipeout-2v2").unwrap(),
                configuration_revision: 1,
                display_name: "Wipeout 2v2".to_string(),
                mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
                map_preset_ids: vec![crate::map::MapPresetId(1)],
                team_count: 2,
                players_per_team: 2,
                rules_summary: crate::lobby::AdvertisedRulesSummary::Wipeout {
                    target_score: 10,
                    active_limit_ticks: 3_600,
                },
            }],
        }
    }

    #[test]
    fn validated_target_freezes_canonical_address_and_normalized_name() {
        let target = validate_target("LOCALHOST", " Cafe\u{301} ").unwrap();
        assert_eq!(target.logical_address.canonical(), "localhost:5000");
        assert_eq!(target.proposed_display_name, "Café");
    }

    #[test]
    fn flow_has_the_m04_queue_state() {
        let states = [
            ClientFlow::Title,
            ClientFlow::ServerSelect,
            ClientFlow::Connecting,
            ClientFlow::GameSelect,
            ClientFlow::Queue,
        ];
        assert_eq!(states.len(), 5);
    }

    #[test]
    fn match_flow_hands_input_to_gameplay_and_returns_it_to_the_shell() {
        let mut app = flow_test_app();
        app.world_mut()
            .insert_resource(super::super::ClientInputContext::Shell);

        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::Match);
        app.update();
        assert_eq!(
            *app.world().resource::<super::super::ClientInputContext>(),
            super::super::ClientInputContext::Gameplay
        );

        *app.world_mut()
            .resource_mut::<super::super::ClientInputContext>() =
            super::super::ClientInputContext::Menu;
        app.update();
        assert_eq!(
            *app.world().resource::<super::super::ClientInputContext>(),
            super::super::ClientInputContext::Menu
        );

        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::GameSelect);
        app.update();
        assert_eq!(
            *app.world().resource::<super::super::ClientInputContext>(),
            super::super::ClientInputContext::Shell
        );
    }

    #[test]
    fn completed_match_stays_covered_until_the_fresh_lobby_is_ready() {
        let mut app = flow_test_app();
        app.world_mut()
            .insert_resource(super::super::ClientInputContext::Shell);
        let match_root = app
            .world_mut()
            .spawn((
                crate::matchplay::MatchRoot,
                crate::matchplay::MatchState {
                    match_id: crate::matchplay::MatchId(9),
                    mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
                    phase: crate::matchplay::MatchPhase::Completed {
                        completed_at_tick: 12,
                        restart_unlocked_at_tick: 72,
                        result: crate::matchplay::MatchResult::Draw,
                    },
                    rules_revision: 1,
                },
            ))
            .id();
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::Match);
        app.update();

        let completion_root = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<MatchCompletionRoot>>();
            query.single(world).unwrap()
        };
        assert!(visible_text(&mut app).iter().any(|text| text == "DRAW"));

        app.world_mut().entity_mut(match_root).despawn();
        app.update();
        assert!(app.world().get_entity(completion_root).is_ok());

        app.world_mut().spawn((
            Client,
            lobby_membership(),
            RoutedClientSession {
                generation: 3,
                kind: super::super::RoutedClientSessionKind::Lobby,
            },
            ClientJoinStatus {
                phase: ClientJoinPhase::LobbyActive {
                    player_id: crate::protocol::PlayerId(1),
                },
                started_at: Duration::ZERO,
                disconnect_requested: false,
            },
        ));
        app.update();
        app.update();

        assert_eq!(
            *app.world().resource::<State<ClientFlow>>().get(),
            ClientFlow::GameSelect
        );
        assert!(app.world().get_entity(completion_root).is_err());
        assert_eq!(count_flow_roots(&mut app), 1);
    }

    #[test]
    fn results_queue_again_uses_the_fresh_lobby_catalog_when_selection_was_cleared() {
        let mut app = flow_test_app();
        app.world_mut()
            .insert_resource(super::super::ClientInputContext::Shell);
        let lobby = lobby_membership();
        let game_type_id = lobby.game_types[0].id.clone();
        app.world_mut().spawn((
            Client,
            lobby,
            RoutedClientSession {
                generation: 3,
                kind: super::super::RoutedClientSessionKind::Lobby,
            },
            ClientJoinStatus {
                phase: ClientJoinPhase::LobbyActive {
                    player_id: crate::protocol::PlayerId(1),
                },
                started_at: Duration::ZERO,
                disconnect_requested: false,
            },
        ));
        app.world_mut().spawn((
            Client,
            RoutedClientSession {
                generation: 2,
                kind: super::super::RoutedClientSessionKind::Match,
            },
            ClientJoinStatus {
                phase: ClientJoinPhase::Disconnected,
                started_at: Duration::ZERO,
                disconnect_requested: true,
            },
        ));
        app.world_mut()
            .resource_mut::<RoutedClientLifecycle>()
            .generation = 4;
        {
            let mut result = app
                .world_mut()
                .resource_mut::<super::super::ClientMatchResultState>();
            result.last_accepted_game_type_id = Some(game_type_id.clone());
            result.context = Some(super::super::ClientMatchResultContext {
                result: crate::matchplay::MatchResult::Draw,
                local_team: None,
                game_type_id: None,
                game_name: None,
                final_score: None,
            });
        }
        *app.world_mut().resource_mut::<SelectedGameType>() = SelectedGameType::default();
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::Match);
        app.update();
        app.update();

        assert_eq!(
            *app.world().resource::<State<ClientFlow>>().get(),
            ClientFlow::Results
        );
        assert_eq!(
            app.world()
                .resource::<SelectedGameType>()
                .game_type_id
                .as_ref(),
            None
        );

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Enter);
        app.update();

        let pending = app
            .world()
            .resource::<super::super::ClientQueueModel>()
            .pending()
            .expect("Queue Again should create a fresh Join");
        assert!(matches!(
            &pending.command,
            crate::lobby::QueueCommand::Join(command) if command.game_type_id == game_type_id
        ));
        assert_eq!(
            app.world()
                .resource::<SelectedGameType>()
                .game_type_id
                .as_ref(),
            Some(&game_type_id)
        );
    }

    #[test]
    fn build_editor_retains_root_and_scroll_until_render_state_changes() {
        let mut app = flow_test_app();
        app.world_mut().spawn((Client, lobby_membership()));
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::GameSelect);
        app.update();
        app.world_mut()
            .resource_mut::<super::super::BuildEditorState>()
            .open();
        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::BuildEditor;
        app.update();

        let first = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<BuildEditorRoot>>();
            query.single(world).unwrap()
        };
        app.world_mut()
            .entity_mut(first)
            .get_mut::<ScrollPosition>()
            .unwrap()
            .0
            .y = 120.0;
        app.update();
        let unchanged = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<BuildEditorRoot>>();
            query.single(world).unwrap()
        };
        assert_eq!(unchanged, first);
        assert!(
            (app.world().get::<ScrollPosition>(unchanged).unwrap().0.y - 120.0).abs()
                <= f32::EPSILON
        );

        app.world_mut()
            .resource_mut::<super::super::BuildEditorState>()
            .selected_choice = 4;
        app.update();
        let rebuilt = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<BuildEditorRoot>>();
            query.single(world).unwrap()
        };
        assert_ne!(rebuilt, first);
        assert!(
            (app.world().get::<ScrollPosition>(rebuilt).unwrap().0.y - 120.0).abs() <= f32::EPSILON
        );
    }

    #[test]
    fn cancelling_build_editor_restores_visible_build_and_join_focus() {
        let mut app = flow_test_app();
        app.world_mut().spawn((
            Client,
            lobby_membership(),
            RoutedClientSession {
                generation: 1,
                kind: super::super::RoutedClientSessionKind::Lobby,
            },
            RuntimeLobbyTarget {
                logical_address: "localhost:5000".to_string(),
                proposed_display_name: "Player".to_string(),
            },
        ));
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::GameSelect);
        app.update();
        app.world_mut()
            .resource_mut::<super::super::BuildEditorState>()
            .open();
        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::BuildEditor;
        app.update();
        assert!(app.world().resource::<FlowNavigation>().selected >= BUILD_EDITOR_CHOICE_BASE);

        app.world_mut()
            .resource_mut::<ButtonInput<KeyCode>>()
            .press(KeyCode::Escape);
        app.update();

        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::None
        );
        assert_eq!(app.world().resource::<FlowNavigation>().selected, 1);
        let focused_action = {
            let world = app.world_mut();
            let mut query = world.query::<(&FlowButton, Has<InteractionDisabled>)>();
            query
                .iter(world)
                .find(|(button, _)| button.index == 1)
                .map(|(button, disabled)| (button.action.clone(), disabled))
        };
        assert_eq!(focused_action, Some((FlowUiAction::OpenBuildEditor, false)));
    }

    #[test]
    fn returning_to_game_select_preserves_a_still_advertised_game() {
        let mut app = flow_test_app();
        let mut lobby = lobby_membership();
        let mut second = lobby.game_types[0].clone();
        second.id = crate::lobby::GameTypeId::new("hot-zone-2v2").unwrap();
        second.configuration_revision = 2;
        second.display_name = "Hot Zone 2v2".to_string();
        second.mode_definition_id = crate::map::HOT_ZONE_MODE_DEFINITION;
        lobby.game_types.push(second.clone());
        app.world_mut().spawn((Client, lobby.clone()));
        *app.world_mut().resource_mut::<SelectedGameType>() = SelectedGameType {
            catalog_revision: Some(lobby.catalog_revision),
            game_type_id: Some(second.id.clone()),
            configuration_revision: Some(second.configuration_revision),
        };

        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::GameSelect);
        app.update();

        assert_eq!(app.world().resource::<FlowNavigation>().selected, 1);
        assert_eq!(
            app.world()
                .resource::<SelectedGameType>()
                .game_type_id
                .as_ref(),
            Some(&second.id)
        );
    }

    #[test]
    fn correctable_rejection_focus_indices_name_the_actual_field_controls() {
        assert_eq!(
            build_editor_field_focus_index(super::super::BuildEditorField::Power),
            BUILD_EDITOR_FIELD_BASE
        );
        assert_eq!(
            build_editor_field_focus_index(super::super::BuildEditorField::PassiveTwo),
            BUILD_EDITOR_FIELD_BASE + 5
        );
    }

    #[test]
    fn reopening_editor_preserves_authoritative_corrective_focus() {
        let mut app = flow_test_app();
        app.world_mut().spawn((Client, lobby_membership()));
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::GameSelect);
        app.update();

        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Error(FlowError {
            kind: FlowErrorKind::Queue,
            message: "Queue admission is taking longer than expected".to_string(),
            return_flow: ClientFlow::GameSelect,
            actions: [
                Some(FlowErrorAction::RetryQueue),
                Some(FlowErrorAction::Disconnect),
            ],
        });
        app.update();
        assert_eq!(count_error_roots(&mut app), 1);

        let corrective_index =
            build_editor_field_focus_index(super::super::BuildEditorField::PassiveTwo);
        {
            let mut editor = app
                .world_mut()
                .resource_mut::<super::super::BuildEditorState>();
            editor.selected_choice = 4;
            editor.focused_field = super::super::BuildEditorField::PassiveTwo;
            editor.is_open = true;
        }
        app.world_mut().resource_mut::<FlowNavigation>().selected = corrective_index;
        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::BuildEditor;
        app.update();

        assert_eq!(
            app.world().resource::<FlowNavigation>().selected,
            corrective_index
        );
        let world = app.world_mut();
        let mut query = world.query::<&FlowButton>();
        assert!(query.iter(world).any(|button| {
            button.index == corrective_index
                && matches!(button.action, FlowUiAction::FocusBuildField(5))
        }));
    }

    #[test]
    fn queue_copy_uses_advertised_game_and_accepted_build_names() {
        let builds = crate::builds::BuildCatalog::embedded().unwrap();
        let preset = builds.preset(crate::builds::BuildPresetId(1)).unwrap();
        let membership = crate::lobby::QueueMembership {
            ticket_id: crate::lobby::QueueTicketId::new(1).unwrap(),
            catalog_revision: crate::lobby::CatalogRevision([1; 32]),
            game_type_id: crate::lobby::GameTypeId::new("wipeout-2v2").unwrap(),
            game_type_configuration_revision: 1,
            accepted_build: crate::builds::AcceptedBuildSummary {
                canonical_recipe: preset.recipe,
                identity: crate::builds::SelectedBuild {
                    source_build_preset_id: Some(preset.id),
                    recipe_fingerprint: crate::builds::BuildRecipeFingerprint(1),
                    revision: builds.balance_revision,
                },
                total_points: 10,
            },
            admitted_at_pool_state_revision: 2,
        };
        let copy = queue_membership_text(
            &super::super::ClientQueueModel::default(),
            &membership,
            Some(&lobby_membership()),
            &builds,
        );
        assert!(copy.contains("Wipeout 2v2"));
        assert!(copy.contains(&preset.display_name));
        assert!(copy.contains("Updating queue"));
    }

    #[test]
    fn queue_recovery_restores_the_command_owner_and_editor_uses_advertised_name() {
        let lobby = lobby_membership();
        let selection = SelectedGameType {
            catalog_revision: Some(lobby.catalog_revision),
            game_type_id: Some(lobby.game_types[0].id.clone()),
            configuration_revision: Some(1),
        };
        assert_eq!(selected_game_name(&selection, Some(&lobby)), "Wipeout 2v2");
        let join = crate::lobby::QueueCommand::Join(crate::lobby::QueueJoinCommand {
            catalog_revision: lobby.catalog_revision,
            game_type_id: lobby.game_types[0].id.clone(),
            game_type_configuration_revision: 1,
            build: crate::builds::BuildCandidate {
                build_revision: crate::builds::BuildRevision(1),
                selection: crate::builds::BuildSelection::Preset(crate::builds::BuildPresetId(1)),
            },
        });
        let cancel = crate::lobby::QueueCommand::Cancel(crate::lobby::QueueCancelCommand {
            ticket_id: crate::lobby::QueueTicketId::new(1).unwrap(),
        });
        assert_eq!(queue_recovery_overlay(&join), OverlayCommit::BuildEditor);
        assert_eq!(queue_recovery_overlay(&cancel), OverlayCommit::Clear);
    }

    #[test]
    fn cancel_pending_copy_is_explicit_and_disables_only_cancel() {
        let cancel = crate::lobby::QueueCommand::Cancel(crate::lobby::QueueCancelCommand {
            ticket_id: crate::lobby::QueueTicketId::new(1).unwrap(),
        });
        assert_eq!(
            queue_cancel_presentation(Some(&cancel)),
            ("CANCELLING…", true)
        );
        assert_eq!(queue_cancel_presentation(None), ("CANCEL QUEUE", false));
    }

    #[test]
    fn state_scoped_flow_roots_replace_exactly_and_error_waits_for_destination() {
        let mut app = flow_test_app();
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::ServerSelect);
        app.update();
        assert_eq!(count_flow_roots(&mut app), 1);

        app.world_mut().resource_mut::<FlowNavigation>().selected = 7;
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::Connecting);
        app.update();
        assert_eq!(count_flow_roots(&mut app), 1);
        assert_eq!(app.world().resource::<FlowNavigation>().selected, 0);

        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Error(FlowError {
            kind: FlowErrorKind::Connection,
            message: "recoverable".to_string(),
            return_flow: ClientFlow::ServerSelect,
            actions: [Some(FlowErrorAction::Back), None],
        });
        app.update();
        assert_eq!(count_error_roots(&mut app), 0);

        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::ServerSelect);
        app.update();
        assert_eq!(count_flow_roots(&mut app), 1);
        assert_eq!(count_error_roots(&mut app), 1);
        assert_eq!(app.world().resource::<FlowNavigation>().selected, 1_000);

        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::None;
        app.update();
        assert_eq!(count_error_roots(&mut app), 0);
    }

    #[test]
    fn replacing_error_in_place_rebuilds_message_and_actions() {
        let mut app = flow_test_app();
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::ServerSelect);
        app.update();
        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Error(FlowError {
            kind: FlowErrorKind::Queue,
            message: "The queue acknowledgement is taking longer than expected".to_string(),
            return_flow: ClientFlow::ServerSelect,
            actions: [
                Some(FlowErrorAction::RetryQueue),
                Some(FlowErrorAction::Disconnect),
            ],
        });
        app.update();
        assert_eq!(count_error_roots(&mut app), 1);

        *app.world_mut().resource_mut::<ClientOverlay>() = ClientOverlay::Error(FlowError {
            kind: FlowErrorKind::Queue,
            message: "Queue commands are temporarily limited".to_string(),
            return_flow: ClientFlow::ServerSelect,
            actions: [
                Some(FlowErrorAction::TryAgainQueue),
                Some(FlowErrorAction::Disconnect),
            ],
        });
        app.update();

        assert_eq!(count_error_roots(&mut app), 1);
        let text = visible_text(&mut app);
        assert!(text.iter().any(|line| line.contains("temporarily limited")));
        assert!(!text.iter().any(|line| line.contains("taking longer")));
        let world = app.world_mut();
        let mut query = world.query_filtered::<Entity, With<RateLimitTryAgain>>();
        assert_eq!(query.iter(world).count(), 1);
    }

    #[test]
    fn combined_local_load_failure_has_one_fixed_error_shape() {
        let error = local_load_error(ClientLocalLoadFailures {
            settings_failed: true,
            connections_failed: true,
            build_failed: false,
        })
        .unwrap();
        assert!(error.message.contains("Settings and connection data"));
        assert_eq!(error.return_flow, ClientFlow::Title);
        assert_eq!(
            error.actions,
            [Some(FlowErrorAction::ContinueWithDefaults), None]
        );
    }

    fn deadline_fixture() -> PendingConnection {
        PendingConnection {
            generation: 1,
            target: validate_target("127.0.0.1:5000", "Player One").unwrap(),
            candidates: vec![
                "127.0.0.1:5000".parse().unwrap(),
                "127.0.0.2:5000".parse().unwrap(),
            ],
            current_candidate: 0,
            overall_deadline: Duration::from_secs(10),
            dns_deadline: Some(Duration::from_secs(5)),
            candidate_deadline: Some(Duration::from_secs(7)),
            current_entity: None,
            stage: ConnectionStage::ResolvingAddress,
        }
    }

    #[test]
    fn deadline_boundaries_accept_exact_and_expire_only_after() {
        let pending = deadline_fixture();
        assert_eq!(
            attempt_deadline_expiry(Duration::from_secs(5), &pending),
            None
        );
        assert_eq!(
            attempt_deadline_expiry(Duration::from_secs(5) + Duration::from_nanos(1), &pending),
            Some(AttemptDeadlineExpiry::Dns)
        );

        let mut pending = deadline_fixture();
        pending.dns_deadline = None;
        assert_eq!(
            attempt_deadline_expiry(Duration::from_secs(7), &pending),
            None
        );
        assert_eq!(
            attempt_deadline_expiry(Duration::from_secs(7) + Duration::from_nanos(1), &pending),
            Some(AttemptDeadlineExpiry::Candidate)
        );
        pending.candidate_deadline = None;
        assert_eq!(
            attempt_deadline_expiry(Duration::from_secs(10), &pending),
            None
        );
        assert_eq!(
            attempt_deadline_expiry(Duration::from_secs(10) + Duration::from_nanos(1), &pending),
            Some(AttemptDeadlineExpiry::Overall)
        );

        assert!(matches!(
            accepted_observation(Duration::from_secs(10), &pending, false),
            SessionObservation::Accepted
        ));
        assert!(matches!(
            accepted_observation(
                Duration::from_secs(10) + Duration::from_nanos(1),
                &pending,
                false
            ),
            SessionObservation::TimedOut
        ));
        assert!(matches!(
            accepted_observation(Duration::from_secs(1), &pending, true),
            SessionObservation::UnexpectedLoss
        ));
    }

    #[test]
    fn connecting_copy_reports_stage_candidate_and_bounded_time() {
        let mut pending = deadline_fixture();
        pending.dns_deadline = None;
        pending.stage = ConnectionStage::ContactingServer {
            current: 1,
            total: 2,
        };

        let copy = connection_presentation(&pending, Duration::from_millis(2_100));

        assert!(copy.contains("STEP 2 OF 3"));
        assert!(copy.contains("Opening routed connection."));
        assert!(copy.contains("127.0.0.1:5000"));
        assert!(copy.contains("Candidate 1 of 2"));
        assert!(copy.contains("up to 8s remaining"));
    }

    #[test]
    fn candidate_shares_rounding_and_ordered_dedup_are_exact() {
        assert_eq!(
            candidate_time_share(Duration::from_secs(10), 4),
            Duration::from_millis(2_500)
        );
        assert_eq!(
            netcode_timeout_ceiling(Duration::from_millis(2_500)),
            Duration::from_secs(3)
        );
        assert_eq!(
            netcode_timeout_ceiling(Duration::ZERO),
            Duration::from_secs(1)
        );

        let input = [
            "127.0.0.3:5000".parse().unwrap(),
            "127.0.0.1:5000".parse().unwrap(),
            "127.0.0.3:5000".parse().unwrap(),
            "127.0.0.2:5000".parse().unwrap(),
            "127.0.0.4:5000".parse().unwrap(),
            "127.0.0.5:5000".parse().unwrap(),
        ];
        let bounded = bound_resolved_candidates(input);
        assert_eq!(bounded.len(), MAX_RESOLVED_CANDIDATES);
        assert_eq!(bounded[0], input[0]);
        assert_eq!(bounded[1], input[1]);
        assert_eq!(bounded[2], input[3]);
        assert_eq!(bounded[3], input[4]);

        let mut pending = deadline_fixture();
        assert!(has_next_candidate(&pending));
        pending.current_candidate = 1;
        assert!(!has_next_candidate(&pending));
    }

    #[test]
    fn name_editor_moves_and_deletes_on_grapheme_boundaries() {
        let mut model = ServerSelectModel {
            address: String::new(),
            committed_name: String::new(),
            name: "A👨‍👩‍👧B".to_string(),
            editing: Some(EditingField::Name),
            caret: "A👨‍👩‍👧".len(),
            inline_error: None,
        };
        let previous = previous_caret(&model.name, model.caret, EditingField::Name);
        assert_eq!(previous, 1);
        let caret = model.caret;
        edited_value_mut(&mut model, EditingField::Name).replace_range(previous..caret, "");
        model.caret = previous;
        assert_eq!(model.name, "AB");
        insert_editor_text(&mut model, EditingField::Name, "é");
        assert_eq!(model.name, "AéB");
    }

    #[test]
    fn address_editor_rejects_non_ascii_and_respects_mid_string_caret() {
        let mut model = ServerSelectModel {
            address: "localhost:5000".to_string(),
            committed_name: String::new(),
            name: String::new(),
            editing: Some(EditingField::Address),
            caret: 9,
            inline_error: None,
        };
        insert_editor_text(&mut model, EditingField::Address, "-dev");
        assert_eq!(model.address, "localhost-dev:5000");
        insert_editor_text(&mut model, EditingField::Address, "é");
        assert!(model.inline_error.is_some());
    }

    #[test]
    fn explicit_cancel_has_its_own_slot_and_overlay_blocks_underlying_controls() {
        let mut actions = PendingFlowActions::default();
        queue_ui_action(&mut actions, FlowUiAction::Connect);
        queue_ui_action(&mut actions, FlowUiAction::Cancel);
        assert!(matches!(actions.explicit, Some(FlowUiAction::Cancel)));
        assert!(matches!(actions.ordinary, Some(FlowUiAction::Connect)));

        let overlay = ClientOverlay::Error(FlowError {
            kind: FlowErrorKind::Connection,
            message: "blocked".to_string(),
            return_flow: ClientFlow::ServerSelect,
            actions: [Some(FlowErrorAction::Back), None],
        });
        let underlying = FlowButton {
            index: 1,
            action: FlowUiAction::Connect,
            error_action: false,
            build_editor_action: false,
        };
        let error = FlowButton {
            index: 1_000,
            action: FlowUiAction::DismissError,
            error_action: true,
            build_editor_action: false,
        };
        assert!(!overlay_allows_button(&overlay, &underlying));
        assert!(overlay_allows_button(&overlay, &error));
    }

    #[test]
    fn error_kinds_have_specific_user_facing_titles() {
        assert_eq!(FlowErrorKind::Connection.title(), "CONNECTION ERROR");
        assert_eq!(FlowErrorKind::Queue.title(), "QUEUE ERROR");
        assert_eq!(FlowErrorKind::Persistence.title(), "SAVE ERROR");
        assert_eq!(FlowErrorKind::Content.title(), "CONTENT ERROR");
    }

    #[test]
    fn rejection_actions_and_favorite_focus_are_deterministic() {
        let invalid_name = rejection_flow_error(ClientLobbyFailure::Rejected(
            crate::protocol::LobbyJoinRejection::InvalidName,
        ));
        assert_eq!(
            invalid_name.actions,
            [Some(FlowErrorAction::EditName), Some(FlowErrorAction::Back)]
        );
        assert_eq!(favorite_focus_after_removal(Some(1), 2), 5);
        assert_eq!(favorite_focus_after_removal(Some(2), 2), 5);
        assert_eq!(favorite_focus_after_removal(Some(0), 0), 0);
        assert_eq!(favorite_focus_after_removal(None, 2), 0);
    }

    #[test]
    fn controller_can_enter_and_leave_text_editing_without_becoming_trapped() {
        let mut app = flow_test_app();
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::ServerSelect);
        app.update();
        assert_eq!(app.world().resource::<FlowNavigation>().selected, 2);

        let mut gamepad = Gamepad::default();
        gamepad.digital_mut().press(GamepadButton::DPadUp);
        let gamepad_entity = app.world_mut().spawn(gamepad).id();
        app.update();
        assert_eq!(app.world().resource::<FlowNavigation>().selected, 1);

        {
            let mut gamepad = app.world_mut().entity_mut(gamepad_entity);
            let mut gamepad = gamepad.get_mut::<Gamepad>().unwrap();
            gamepad.digital_mut().reset_all();
            gamepad.digital_mut().press(GamepadButton::South);
        }
        app.update();
        assert_eq!(
            app.world().resource::<ServerSelectModel>().editing,
            Some(EditingField::Name)
        );

        {
            let mut gamepad = app.world_mut().entity_mut(gamepad_entity);
            let mut gamepad = gamepad.get_mut::<Gamepad>().unwrap();
            gamepad.digital_mut().reset_all();
            gamepad.digital_mut().press(GamepadButton::East);
        }
        app.update();
        assert_eq!(app.world().resource::<ServerSelectModel>().editing, None);
    }
}
