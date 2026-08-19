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
    input::{ButtonState, keyboard::KeyboardInput},
    prelude::*,
    tasks::{IoTaskPool, Task, block_on, poll_once},
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

#[derive(States, Clone, Copy, Debug, Default, Eq, Hash, PartialEq)]
pub enum ClientFlow {
    #[default]
    Title,
    ServerSelect,
    Connecting,
    GameSelect,
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub enum ClientOverlay {
    #[default]
    None,
    Settings,
    Credits,
    Error(FlowError),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FlowError {
    pub message: String,
    pub return_flow: ClientFlow,
    pub actions: [Option<FlowErrorAction>; 2],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FlowErrorAction {
    RetryConnection,
    EditName,
    Back,
    RetrySave,
    ContinueWithoutSaving,
    ContinueWithDefaults,
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
    clear_overlay: bool,
    refresh_server_select: Option<usize>,
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
}

pub(super) fn local_load_error(failures: ClientLocalLoadFailures) -> Option<FlowError> {
    let message = match (failures.settings_failed, failures.connections_failed) {
        (true, true) => {
            "Settings and connection data could not be loaded; safe defaults are active"
        }
        (true, false) => "Settings could not be loaded; safe defaults are active",
        (false, true) => "Connection data could not be loaded; safe defaults are active",
        (false, false) => return None,
    };
    Some(FlowError {
        message: message.to_string(),
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

#[derive(Component)]
struct FlowRoot;

#[derive(Component, Clone, Debug)]
struct FlowButton {
    index: usize,
    action: FlowUiAction,
    error_action: bool,
}

#[derive(Component)]
struct FlowErrorRoot;

#[derive(Component, Clone, Copy)]
enum FieldLabel {
    Address,
    Name,
}

#[derive(Component)]
struct ConnectingLabel;

pub struct ClientFlowPlugin;

impl Plugin for ClientFlowPlugin {
    fn build(&self, app: &mut App) {
        app.init_state::<ClientFlow>()
            .init_resource::<ClientOverlay>()
            .init_resource::<PendingFlowActions>()
            .init_resource::<FlowCommit>()
            .init_resource::<ConnectionGeneration>()
            .init_resource::<ResolverState>()
            .init_resource::<ClientConnectionsPath>()
            .init_resource::<ClientLocalLoadFailures>()
            .init_resource::<SelectedGameType>()
            .init_resource::<FlowNavigation>()
            .add_systems(Startup, load_connection_state)
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
                    observe_session.in_set(ClientFlowSet::ObserveSession),
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
                    present_flow.in_set(ClientFlowSet::PresentFlow),
                ),
            )
            .add_systems(OnEnter(ClientFlow::ServerSelect), spawn_server_select)
            .add_systems(OnEnter(ClientFlow::Connecting), spawn_connecting)
            .add_systems(OnEnter(ClientFlow::GameSelect), spawn_game_select);
    }
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
    mut overlay: ResMut<ClientOverlay>,
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
    mut actions: ResMut<PendingFlowActions>,
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
    if *flow.get() == ClientFlow::GameSelect {
        if statuses
            .iter()
            .any(|status| matches!(status.phase, ClientJoinPhase::Disconnected))
        {
            actions.session = Some(SessionObservation::UnexpectedLoss);
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
    buttons: Query<(&FlowButton, &Interaction)>,
    mut actions: ResMut<PendingFlowActions>,
) {
    for (button, interaction) in &buttons {
        if *interaction == Interaction::Pressed && overlay_allows_button(&overlay, button) {
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
        .filter(|(button, _)| overlay_allows_button(&overlay, button))
        .map(|(button, _)| button.index)
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
        && let Some((button, _)) = buttons
            .iter()
            .filter(|(button, _)| {
                button.index == navigation.selected && overlay_allows_button(&overlay, button)
            })
            .min_by_key(|(button, _)| button.index)
    {
        queue_ui_action(&mut actions, button.action.clone());
    }
    if keyboard.just_pressed(KeyCode::Escape) || pad_pressed(GamepadButton::East) {
        let action = match *flow.get() {
            ClientFlow::Connecting => FlowUiAction::Cancel,
            ClientFlow::GameSelect => FlowUiAction::Disconnect,
            ClientFlow::ServerSelect => FlowUiAction::Back,
            ClientFlow::Title => return,
        };
        queue_ui_action(&mut actions, action);
    }
}

fn overlay_allows_button(overlay: &ClientOverlay, button: &FlowButton) -> bool {
    !matches!(overlay, ClientOverlay::Error(_)) || button.error_action
}

fn queue_ui_action(actions: &mut PendingFlowActions, action: FlowUiAction) {
    if matches!(action, FlowUiAction::Cancel | FlowUiAction::Disconnect) {
        actions.explicit = Some(action);
    } else {
        actions.ordinary = Some(action);
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "one bounded coordinator makes flow-action precedence and commits explicit"
)]
fn resolve_flow_action(
    flow: Res<State<ClientFlow>>,
    mut actions: ResMut<PendingFlowActions>,
    mut commit: ResMut<FlowCommit>,
    mut model: ResMut<ServerSelectModel>,
    mut persistence: ResMut<ConnectionPersistence>,
    mut pending: Option<ResMut<PendingConnection>>,
    membership: Query<(&ClientLobbyMembership, &RuntimeLobbyTarget), With<Client>>,
    path: Res<ClientConnectionsPath>,
    overlay: Res<ClientOverlay>,
    mut selection: ResMut<SelectedGameType>,
) {
    if let Some(explicit) = actions.explicit.take() {
        match explicit {
            FlowUiAction::Cancel | FlowUiAction::Disconnect => {
                commit.teardown = true;
                commit.next_flow = Some(ClientFlow::ServerSelect);
                *selection = SelectedGameType::default();
            }
            _ => {}
        }
        return;
    }
    if let Some(observation) = actions.session.take() {
        match observation {
            SessionObservation::Accepted => {
                if let Some((membership, target)) = membership.iter().next() {
                    persistence.state.preferred_display_name = Some(model.committed_name.clone());
                    let _ = persistence
                        .state
                        .record_recent(&membership.server_name, &target.logical_address);
                    if let Err(error) = save_connections(&path.0, &persistence.state) {
                        persistence.dirty_error = Some(error.clone());
                        commit.error = Some(FlowError {
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
                commit.clear_overlay = true;
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
                commit.clear_overlay = true;
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
        FlowUiAction::RetrySave => match save_connections(&path.0, &persistence.state) {
            Ok(()) => {
                persistence.dirty_error = None;
                commit.clear_overlay = true;
            }
            Err(error) => persistence.dirty_error = Some(error),
        },
        FlowUiAction::ContinueWithoutSaving => {
            persistence.dirty_error = None;
            commit.clear_overlay = true;
        }
        FlowUiAction::DismissError => {
            commit.clear_overlay = true;
            commit.refresh_server_select = Some(2);
        }
        FlowUiAction::SelectGame(index) => {
            if let Some((membership, _)) = membership.iter().next()
                && let Some(game_type) = membership.game_types.get(index)
            {
                selection.catalog_revision = Some(membership.catalog_revision);
                selection.game_type_id = Some(game_type.id.clone());
                selection.configuration_revision = Some(game_type.configuration_revision);
            }
        }
        FlowUiAction::ToggleFavorite => {
            if let Some((membership, target)) = membership.iter().next() {
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
        FlowUiAction::Cancel | FlowUiAction::Disconnect => {}
    }
    let _ = flow;
}

fn fail_to_server_select(commit: &mut FlowCommit, message: String, retryable: bool) {
    commit.next_flow = Some(ClientFlow::ServerSelect);
    commit.error = Some(FlowError {
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
) {
    if let Some(error) = &commit.error {
        *overlay = ClientOverlay::Error(error.clone());
    } else if commit.clear_overlay {
        *overlay = ClientOverlay::None;
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
    roots: Query<Entity, With<FlowErrorRoot>>,
    mut navigation: ResMut<FlowNavigation>,
) {
    let ClientOverlay::Error(error) = overlay.as_ref() else {
        for entity in &roots {
            commands.entity(entity).despawn();
        }
        return;
    };
    if *flow.get() == ClientFlow::Title {
        return;
    }
    if error.return_flow != *flow.get() || !roots.is_empty() {
        return;
    }
    navigation.selected = ERROR_BUTTON_BASE;
    commands
        .spawn((
            FlowErrorRoot,
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
                spawn_heading(panel, "CONNECTION ERROR");
                panel.spawn((
                    Text::new(error.message.clone()),
                    TextColor(Color::srgb(1.0, 0.72, 0.65)),
                ));
                for (offset, action) in error.actions.into_iter().flatten().enumerate() {
                    let (ui_action, label) = flow_error_action_button(action);
                    spawn_flow_error_button(panel, ERROR_BUTTON_BASE + offset, ui_action, label);
                }
            });
        });
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
) {
    let Some(membership) = memberships.iter().next() else {
        return;
    };
    navigation.selected = 0;
    if let Some(first) = membership.game_types.first() {
        selection.catalog_revision = Some(membership.catalog_revision);
        selection.game_type_id = Some(first.id.clone());
        selection.configuration_revision = Some(first.configuration_revision);
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
                        "{} | {mode_name} | 2v2 | {map_names} | {rules}",
                        game_type.display_name
                    ),
                    None,
                );
            }
            let offset = membership.game_types.len();
            spawn_flow_button(
                root,
                offset,
                FlowUiAction::ToggleFavorite,
                "FAVORITE / UNFAVORITE CURRENT SERVER",
                None,
            );
            spawn_flow_button(
                root,
                offset + 1,
                FlowUiAction::Disconnect,
                "DISCONNECT",
                None,
            );
        });
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

#[allow(
    clippy::too_many_arguments,
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
        &mut BackgroundColor,
        &mut BorderColor,
    )>,
    mut fields: Query<(&FieldLabel, &mut Text)>,
    mut connecting: Query<&mut Text, (With<ConnectingLabel>, Without<FieldLabel>)>,
) {
    for (button, interaction, mut background, mut border) in &mut buttons {
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
        background.0 = if *interaction == Interaction::Pressed {
            Color::srgb(0.08, 0.48, 0.58)
        } else if focused || *interaction == Interaction::Hovered {
            Color::srgb(0.12, 0.32, 0.42)
        } else if selected_game {
            Color::srgb(0.12, 0.24, 0.34)
        } else {
            Color::srgb(0.09, 0.14, 0.2)
        };
        border.set_all(if focused {
            Color::WHITE
        } else if selected_game {
            Color::srgb(0.25, 0.9, 1.0)
        } else {
            Color::NONE
        });
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

    #[test]
    fn validated_target_freezes_canonical_address_and_normalized_name() {
        let target = validate_target("LOCALHOST", " Cafe\u{301} ").unwrap();
        assert_eq!(target.logical_address.canonical(), "localhost:5000");
        assert_eq!(target.proposed_display_name, "Café");
    }

    #[test]
    fn flow_has_only_the_four_m03_states() {
        let states = [
            ClientFlow::Title,
            ClientFlow::ServerSelect,
            ClientFlow::Connecting,
            ClientFlow::GameSelect,
        ];
        assert_eq!(states.len(), 4);
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
    fn combined_local_load_failure_has_one_fixed_error_shape() {
        let error = local_load_error(ClientLocalLoadFailures {
            settings_failed: true,
            connections_failed: true,
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
            message: "blocked".to_string(),
            return_flow: ClientFlow::ServerSelect,
            actions: [Some(FlowErrorAction::Back), None],
        });
        let underlying = FlowButton {
            index: 1,
            action: FlowUiAction::Connect,
            error_action: false,
        };
        let error = FlowButton {
            index: 1_000,
            action: FlowUiAction::DismissError,
            error_action: true,
        };
        assert!(!overlay_allows_button(&overlay, &underlying));
        assert!(overlay_allows_button(&overlay, &error));
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
