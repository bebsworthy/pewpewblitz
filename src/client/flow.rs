//! M03 product flow, bounded action arbitration, and recoverable lobby presentation.

use super::{
    ClientJoinPhase, ClientJoinStatus, ClientLobbyFailure, ClientLobbyMembership,
    ClientNetworkConfig, ClientSettingsUiSet, RoutedClientLifecycle, RoutedClientPhase,
    RoutedClientSession, RuntimeLobbyTarget,
    connection_persistence::{ClientConnectionsPath, save_connections},
    server_select::{LogicalServerAddress, MAX_RESOLVED_CANDIDATES, ServerAddressHost},
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
    tasks::{block_on, poll_once},
    ui::{InteractionDisabled, ScrollPosition, UiScale, UiSystems},
    window::PrimaryWindow,
};
use lightyear::prelude::client::Client;
use std::time::Duration;

mod actions;
mod connection;
mod input;
mod model;
mod persistence;
mod reducer;
mod screens;

#[cfg(test)]
use super::connection_persistence::ConnectionsFileV1;
use actions::{FlowCommit, FlowUiAction, OverlayCommit, PendingFlowActions, SessionObservation};
#[cfg(test)]
use connection::{
    AttemptDeadlineExpiry, bound_resolved_candidates, candidate_time_share, netcode_timeout_ceiling,
};
use connection::{
    ConnectionGeneration, ConnectionStage, PendingConnection, ResolverState, accepted_observation,
    attempt_deadline_expiry, begin_connection_target, connection_presentation, has_next_candidate,
    observation_for_expiry, spawn_current_candidate, validate_target,
};
use input::{
    dashboard_focus_neighbor, edited_value, edited_value_mut, insert_brawler_name_text,
    insert_editor_text, next_caret, overlay_allows_button, previous_caret, queue_ui_action,
    repair_dashboard_focus,
};
pub use model::{
    CancelMatchStartConfirmation, ClientFlow, ClientOverlay, FlowError, FlowErrorAction,
    FlowErrorKind, SelectedGameType, SessionPurpose,
};
use persistence::load_connection_state;
#[cfg(test)]
use persistence::startup_server_address;
pub(super) use persistence::{ClientLocalLoadFailures, ConnectionPersistence, local_load_error};
use reducer::{commit_flow, favorite_focus_after_removal, teardown_session};
#[cfg(test)]
use screens::results::MatchCompletionRoot;
use screens::{
    game_select::{keep_game_type_focus_visible, scroll_game_type_select, spawn_game_type_select},
    results::{clear_results, present_match_completion, spawn_results},
    server_select::{refresh_server_select, spawn_server_select},
};

const ERROR_BUTTON_BASE: usize = 1_000;
const GAME_TYPE_CONFIRM_INDEX: usize = 1_000;
const GAME_TYPE_BACK_INDEX: usize = 1_001;
const DASHBOARD_PLAY_INDEX: usize = 0;
const DASHBOARD_PRACTICE_INDEX: usize = 1;
const DASHBOARD_GAME_INDEX: usize = 2;
const DASHBOARD_BUILD_INDEX: usize = 3;
const DASHBOARD_SETTINGS_INDEX: usize = 4;
const DASHBOARD_MENU_INDEX: usize = 5;
const DASHBOARD_COMPACT_WIDTH: f32 = 1_000.0;
const DASHBOARD_COMPACT_HEIGHT: f32 = 640.0;

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

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
struct BrawlerCreationDraft {
    fighter_profile_id: crate::profiles::FighterProfileId,
    weapon_base_id: crate::profiles::WeaponBaseId,
    ultimate: crate::builds::UltimateDefinitionId,
    inline_error: Option<String>,
}

impl Default for BrawlerCreationDraft {
    fn default() -> Self {
        Self {
            fighter_profile_id: crate::profiles::FighterProfileId(1),
            weapon_base_id: crate::profiles::WeaponBaseId(1),
            ultimate: crate::builds::UltimateDefinitionId(1),
            inline_error: None,
        }
    }
}

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
struct BrawlerEditDraft {
    brawler_id: Option<crate::profiles::SavedBrawlerId>,
    name: String,
    fighter_profile_id: crate::profiles::FighterProfileId,
    weapon_base_id: crate::profiles::WeaponBaseId,
    ultimate_id: crate::builds::UltimateDefinitionId,
    passive_ids: [crate::builds::PassiveDefinitionId; 2],
    name_caret: usize,
    editing_name: bool,
    inline_error: Option<String>,
}

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
struct WeaponEquipmentDraft {
    brawler_id: Option<crate::profiles::SavedBrawlerId>,
    equipped_part_ids: [Option<crate::weapon_parts::WeaponPartInstanceId>;
        crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
    selected_slot: usize,
    inline_error: Option<String>,
}

impl Default for WeaponEquipmentDraft {
    fn default() -> Self {
        Self {
            brawler_id: None,
            equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
            selected_slot: 0,
            inline_error: None,
        }
    }
}

impl Default for BrawlerEditDraft {
    fn default() -> Self {
        Self {
            brawler_id: None,
            name: String::new(),
            fighter_profile_id: crate::profiles::FighterProfileId(1),
            weapon_base_id: crate::profiles::WeaponBaseId(1),
            ultimate_id: crate::builds::UltimateDefinitionId(1),
            passive_ids: [
                crate::builds::PassiveDefinitionId(3),
                crate::builds::PassiveDefinitionId(4),
            ],
            name_caret: 0,
            editing_name: false,
            inline_error: None,
        }
    }
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

#[derive(Resource, Default, Clone, Debug, PartialEq, Eq)]
struct GameTypeSelectionDraft {
    selected_index: Option<usize>,
    unavailable_previous: bool,
}

#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
struct DashboardReturnFocus(Option<usize>);

#[derive(Resource, Default, Clone, Debug, PartialEq, Eq)]
struct DashboardNotice(Option<String>);

#[derive(Resource, Default)]
struct PendingCreatedBrawler(Option<u64>);

#[derive(Resource, Default)]
struct PendingEditedBrawler(Option<crate::profiles::SavedBrawlerId>);

#[derive(Resource, Default)]
struct FlowNavigation {
    selected: usize,
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
enum DashboardLayoutClass {
    #[default]
    Wide,
    Compact,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum DashboardNavigationDirection {
    Up,
    Down,
    Left,
    Right,
}

#[derive(Resource, Default)]
struct MatchFailureNotice(bool);

#[derive(Component)]
struct FlowRoot;

#[derive(Component)]
pub(super) struct DashboardRoot;

#[derive(Component, Clone, Copy)]
enum DashboardLayoutRole {
    Root,
    Header,
    Wordmark,
    Identity,
    HeaderSpacer,
    Center,
    Preview,
    Build,
    ActionRow,
    Mode,
    UtilityButton { wide_width: f32 },
    UtilityLabel { has_icon: bool },
}

#[derive(Component, Clone, Debug)]
struct FlowButton {
    index: usize,
    action: FlowUiAction,
    error_action: bool,
}

#[derive(Component)]
struct FlowErrorRoot(FlowError);

#[derive(Component)]
struct RateLimitTryAgain;

#[derive(Component)]
struct RateLimitTryAgainLabel;

#[derive(Component)]
struct GamePopulationLabel(usize);

#[derive(Component)]
struct GameTypeSelectRoot;

#[derive(Component)]
struct DashboardGameSummaryLabel;

#[derive(Component)]
struct DashboardPlayLabel;

#[derive(Component)]
struct DashboardPracticeLabel;

#[derive(Component)]
struct DashboardBrawlerNameLabel;

#[derive(Component)]
struct DashboardBrawlerSummaryLabel;

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
struct ChangeServerConfirmationRoot;

#[derive(Component)]
struct DeleteBrawlerConfirmationRoot;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
struct BrawlerListRoot {
    profile_revision: crate::profiles::ProfileRevision,
    pending: bool,
}

#[derive(Component)]
struct BrawlerListScrollArea;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
struct BrawlerDetailsRoot {
    brawler_id: crate::profiles::SavedBrawlerId,
    profile_revision: crate::profiles::ProfileRevision,
    pending: bool,
    contextual_confirmation: bool,
    layout: BrawlerDetailsLayout,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum BrawlerDetailsLayout {
    Compact,
    Wide,
}

#[derive(Component)]
struct BrawlerDetailsScrollArea;

#[derive(Component)]
pub(super) struct BrawlerDetailsPreviewHost;

#[derive(Component, Clone, Debug, PartialEq, Eq)]
struct BrawlerCreationRoot {
    draft: BrawlerCreationDraft,
    layout: BrawlerDetailsLayout,
}

#[derive(Component, Clone, Debug, PartialEq, Eq)]
struct BrawlerEditorRoot {
    draft: BrawlerEditDraft,
    layout: BrawlerDetailsLayout,
}

#[derive(Component, Clone, Debug, PartialEq, Eq)]
struct WeaponEquipmentRoot {
    draft: WeaponEquipmentDraft,
    layout: BrawlerDetailsLayout,
}

#[derive(Component)]
struct WeaponEquipmentScrollArea;

#[derive(Component)]
struct DashboardMenuRoot;

#[derive(Component)]
struct DashboardBuildCard;

#[derive(Component)]
struct DashboardModeCard;

#[derive(Component, Clone, Copy, Debug)]
enum DashboardButtonStyle {
    Preview,
    Header,
    Build,
    Mode,
    Practice,
    Play,
}

#[derive(Component, Clone, Copy)]
enum FieldLabel {
    Address,
    Name,
}

#[derive(Component)]
struct ConnectingLabel;

#[derive(Component)]
pub(super) struct DashboardPreviewHost;

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
            .init_resource::<ClientLocalLoadFailures>()
            .init_resource::<super::ClientQueueModel>()
            .init_resource::<super::ClientPracticeModel>()
            .init_resource::<super::ClientMatchLoadingModel>()
            .init_resource::<crate::builds::BuildCatalogResource>()
            .init_resource::<crate::combat::WeaponCatalogResource>()
            .init_resource::<crate::weapon_parts::WeaponPartCatalogResource>()
            .init_resource::<SelectedGameType>()
            .init_resource::<GameTypeSelectionDraft>()
            .init_resource::<DashboardReturnFocus>()
            .init_resource::<DashboardNotice>()
            .init_resource::<PendingCreatedBrawler>()
            .init_resource::<PendingEditedBrawler>()
            .init_resource::<BrawlerCreationDraft>()
            .init_resource::<BrawlerEditDraft>()
            .init_resource::<WeaponEquipmentDraft>()
            .init_resource::<super::ClientProfileModel>()
            .init_resource::<SessionPurpose>()
            .init_resource::<super::ClientMatchResultState>()
            .init_resource::<RoutedClientLifecycle>()
            .init_resource::<MatchFailureNotice>()
            .init_resource::<FlowNavigation>()
            .add_systems(
                Startup,
                (
                    load_connection_state,
                    ApplyDeferred,
                    start_initial_connection,
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
                    present_change_server_confirmation
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
                    update_queue_cancel_button
                        .in_set(ClientFlowSet::PresentFlow)
                        .before(present_flow),
                    present_flow.in_set(ClientFlowSet::PresentFlow),
                ),
            )
            .add_systems(
                Update,
                (
                    apply_dashboard_layout,
                    scroll_dashboard.after(apply_dashboard_layout),
                    update_dashboard_live_facts,
                    present_dashboard_menu,
                    scroll_brawler_list.before(present_brawler_list),
                    present_brawler_list,
                    keep_brawler_list_focus_visible.after(present_brawler_list),
                    scroll_brawler_details.before(present_brawler_details),
                    present_brawler_details,
                    keep_brawler_details_focus_visible.after(present_brawler_details),
                    present_brawler_creation,
                    present_brawler_editor,
                    scroll_weapon_equipment.before(present_weapon_equipment),
                    present_weapon_equipment,
                    present_delete_brawler_confirmation,
                    scroll_game_type_select,
                )
                    .in_set(ClientFlowSet::PresentFlow)
                    .before(present_flow),
            )
            .add_systems(OnEnter(ClientFlow::ServerSelect), spawn_server_select)
            .add_systems(OnEnter(ClientFlow::Connecting), spawn_connecting)
            .add_systems(
                OnEnter(ClientFlow::Dashboard),
                (spawn_dashboard, open_empty_profile_creation).chain(),
            )
            .add_systems(OnEnter(ClientFlow::GameTypeSelect), spawn_game_type_select)
            .add_systems(OnEnter(ClientFlow::Match), enter_match_input)
            .add_systems(OnEnter(ClientFlow::Results), spawn_results)
            .add_systems(OnExit(ClientFlow::Results), clear_results)
            .add_systems(OnExit(ClientFlow::Match), exit_match_input);
        app.add_systems(
            PostUpdate,
            (
                keep_dashboard_focus_visible,
                keep_weapon_equipment_focus_visible,
            )
                .after(UiSystems::Layout)
                .run_if(in_state(ClientFlow::Dashboard)),
        );
        app.add_systems(
            PostUpdate,
            keep_game_type_focus_visible
                .after(UiSystems::Layout)
                .run_if(in_state(ClientFlow::GameTypeSelect)),
        );
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
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "startup creates the one bounded product connection from runtime-owned resources"
)]
fn start_initial_connection(
    mut commands: Commands,
    time: Res<Time<Real>>,
    config: Res<ClientNetworkConfig>,
    model: Res<ServerSelectModel>,
    mut generation: ResMut<ConnectionGeneration>,
    mut resolver: ResMut<ResolverState>,
    mut routed: ResMut<RoutedClientLifecycle>,
    mut next_flow: ResMut<NextState<ClientFlow>>,
    mut overlay: ResMut<ClientOverlay>,
) {
    let target = match validate_target(&model.address, &model.name) {
        Ok(target) => target,
        Err(error) => {
            *overlay = ClientOverlay::Error(FlowError {
                kind: FlowErrorKind::Connection,
                message: error,
                return_flow: ClientFlow::ServerSelect,
                actions: [Some(FlowErrorAction::Back), None],
            });
            next_flow.set(ClientFlow::ServerSelect);
            return;
        }
    };
    if let Err(error) = begin_connection_target(
        &mut commands,
        &config,
        time.elapsed(),
        &mut generation,
        &mut resolver,
        &mut routed,
        target,
    ) {
        *overlay = ClientOverlay::Error(FlowError {
            kind: FlowErrorKind::Connection,
            message: error,
            return_flow: ClientFlow::ServerSelect,
            actions: [
                Some(FlowErrorAction::RetryConnection),
                Some(FlowErrorAction::Back),
            ],
        });
        next_flow.set(ClientFlow::ServerSelect);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
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
    statuses: Query<(&ClientJoinStatus, &RoutedClientSession), With<Client>>,
    match_states: Query<&crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>,
    mut actions: ResMut<PendingFlowActions>,
    mut queue: ResMut<super::ClientQueueModel>,
    mut loading: ResMut<super::ClientMatchLoadingModel>,
    mut practice: ResMut<super::ClientPracticeModel>,
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
    if let Some(reason) = practice.take_rejection() {
        actions.session = Some(SessionObservation::PracticeRejected(reason));
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
        && statuses.iter().any(|(status, session)| {
            session.kind == super::RoutedClientSessionKind::Lobby
                && session.generation == routed.generation
                && matches!(status.phase, ClientJoinPhase::LobbyActive { .. })
        })
    {
        actions.session = Some(SessionObservation::FreshLobbyReturn);
        return;
    }
    if *flow.get() == ClientFlow::Match
        && routed.phase == RoutedClientPhase::Match
        && result_state.context.is_none()
        && statuses.iter().any(|(status, session)| {
            session.kind == super::RoutedClientSessionKind::Match
                && session.generation == routed.generation
                && matches!(status.phase, ClientJoinPhase::Disconnected)
        })
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
    if matches!(
        *flow.get(),
        ClientFlow::Dashboard
            | ClientFlow::GameTypeSelect
            | ClientFlow::Queue
            | ClientFlow::Results
    ) {
        if statuses.iter().any(|(status, session)| {
            session.kind == super::RoutedClientSessionKind::Lobby
                && session.generation == routed.generation
                && matches!(status.phase, ClientJoinPhase::Disconnected)
        }) {
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
        .any(|(status, _)| matches!(status.phase, ClientJoinPhase::Disconnected));
    if memberships.iter().next().is_some()
        && statuses
            .iter()
            .any(|(status, _)| matches!(status.phase, ClientJoinPhase::LobbyActive { .. }))
    {
        actions.session = Some(accepted_observation(time.elapsed(), &pending, disconnected));
        return;
    }
    let now = time.elapsed();
    if let Some(expiry) = attempt_deadline_expiry(now, &pending) {
        actions.session = Some(observation_for_expiry(expiry));
        return;
    }
    if let Some((status, _)) = statuses.iter().next() {
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
    dashboard_layout: Query<&DashboardLayoutClass, With<DashboardRoot>>,
    mut actions: ResMut<PendingFlowActions>,
    mut brawler_edit: ResMut<BrawlerEditDraft>,
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
    if matches!(overlay.as_ref(), ClientOverlay::BrawlerEditor) && brawler_edit.editing_name {
        if keyboard.just_pressed(KeyCode::Home) {
            brawler_edit.name_caret = 0;
        } else if keyboard.just_pressed(KeyCode::End) {
            brawler_edit.name_caret = brawler_edit.name.len();
        } else if keyboard.just_pressed(KeyCode::ArrowLeft) {
            brawler_edit.name_caret = previous_caret(
                &brawler_edit.name,
                brawler_edit.name_caret,
                EditingField::Name,
            );
        } else if keyboard.just_pressed(KeyCode::ArrowRight) {
            brawler_edit.name_caret = next_caret(
                &brawler_edit.name,
                brawler_edit.name_caret,
                EditingField::Name,
            );
        }
        for event in keyboard_events.read() {
            if event.state != ButtonState::Pressed {
                continue;
            }
            if event.key_code == KeyCode::Backspace {
                let previous = previous_caret(
                    &brawler_edit.name,
                    brawler_edit.name_caret,
                    EditingField::Name,
                );
                let caret = brawler_edit.name_caret;
                brawler_edit.name.replace_range(previous..caret, "");
                brawler_edit.name_caret = previous;
            } else if event.key_code == KeyCode::Delete {
                let next = next_caret(
                    &brawler_edit.name,
                    brawler_edit.name_caret,
                    EditingField::Name,
                );
                let caret = brawler_edit.name_caret;
                brawler_edit.name.replace_range(caret..next, "");
            } else if let Some(text) = event.text.as_deref() {
                insert_brawler_name_text(&mut brawler_edit, text);
            }
        }
        if keyboard.just_pressed(KeyCode::Enter) || pad_pressed(GamepadButton::South) {
            match crate::lobby::normalize_proposed_display_name(&brawler_edit.name) {
                Ok(name) => {
                    brawler_edit.name = name;
                    brawler_edit.editing_name = false;
                    brawler_edit.inline_error = None;
                }
                Err(error) => brawler_edit.inline_error = Some(format!("Invalid name: {error}")),
            }
        } else if keyboard.just_pressed(KeyCode::Escape) || pad_pressed(GamepadButton::East) {
            brawler_edit.editing_name = false;
        }
        return;
    }
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
    available.dedup();
    if !available.is_empty() {
        if *flow.get() == ClientFlow::Dashboard && matches!(*overlay, ClientOverlay::None) {
            navigation.selected = repair_dashboard_focus(navigation.selected, &available);
            let direction = if keyboard.any_just_pressed([KeyCode::ArrowDown, KeyCode::KeyS])
                || pad_pressed(GamepadButton::DPadDown)
            {
                Some(DashboardNavigationDirection::Down)
            } else if keyboard.any_just_pressed([KeyCode::ArrowUp, KeyCode::KeyW])
                || pad_pressed(GamepadButton::DPadUp)
            {
                Some(DashboardNavigationDirection::Up)
            } else if keyboard.any_just_pressed([KeyCode::ArrowLeft, KeyCode::KeyA])
                || pad_pressed(GamepadButton::DPadLeft)
            {
                Some(DashboardNavigationDirection::Left)
            } else if keyboard.any_just_pressed([KeyCode::ArrowRight, KeyCode::KeyD])
                || pad_pressed(GamepadButton::DPadRight)
            {
                Some(DashboardNavigationDirection::Right)
            } else {
                None
            };
            if let Some(direction) = direction {
                let class = dashboard_layout.iter().next().copied().unwrap_or_default();
                navigation.selected =
                    dashboard_focus_neighbor(class, navigation.selected, direction, &available);
            }
        } else {
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
        let action = if matches!(overlay.as_ref(), ClientOverlay::ChangeServerConfirmation) {
            FlowUiAction::KeepServer
        } else if matches!(overlay.as_ref(), ClientOverlay::DashboardMenu) {
            FlowUiAction::CloseDashboardMenu
        } else if matches!(overlay.as_ref(), ClientOverlay::BrawlerList) {
            FlowUiAction::CloseBrawlerList
        } else if matches!(overlay.as_ref(), ClientOverlay::BrawlerDetails(_)) {
            FlowUiAction::BackToBrawlerList
        } else if matches!(overlay.as_ref(), ClientOverlay::BrawlerCreation) {
            FlowUiAction::CancelCreateBrawler
        } else if matches!(overlay.as_ref(), ClientOverlay::BrawlerEditor) {
            FlowUiAction::CancelBrawlerEdit
        } else if matches!(overlay.as_ref(), ClientOverlay::WeaponEquipment) {
            FlowUiAction::CancelWeaponEquipment
        } else if matches!(
            overlay.as_ref(),
            ClientOverlay::DeleteBrawlerConfirmation(_)
        ) {
            FlowUiAction::CancelDeleteBrawler
        } else if matches!(overlay.as_ref(), ClientOverlay::Confirmation(_)) {
            FlowUiAction::KeepLoading
        } else {
            match *flow.get() {
                ClientFlow::Connecting => FlowUiAction::Cancel,
                ClientFlow::GameTypeSelect => FlowUiAction::CancelGameType,
                ClientFlow::Queue => FlowUiAction::CancelQueue,
                ClientFlow::MatchLoading => FlowUiAction::RequestCancelMatchStart,
                ClientFlow::Results => FlowUiAction::ReturnToDashboard,
                ClientFlow::Match => {
                    if matches!(overlay.as_ref(), ClientOverlay::LeaveConfirmation) {
                        FlowUiAction::KeepPlaying
                    } else {
                        return;
                    }
                }
                ClientFlow::ServerSelect => FlowUiAction::Back,
                ClientFlow::Dashboard => return,
            }
        };
        queue_ui_action(&mut actions, action);
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
    dashboard: (
        ResMut<SelectedGameType>,
        ResMut<GameTypeSelectionDraft>,
        ResMut<DashboardReturnFocus>,
        ResMut<DashboardNotice>,
        ResMut<PendingCreatedBrawler>,
        ResMut<PendingEditedBrawler>,
        ResMut<BrawlerCreationDraft>,
        ResMut<BrawlerEditDraft>,
        ResMut<WeaponEquipmentDraft>,
    ),
    models: (
        ResMut<super::ClientQueueModel>,
        ResMut<super::ClientPracticeModel>,
        ResMut<super::ClientMatchLoadingModel>,
        ResMut<super::ClientMatchResultState>,
        ResMut<super::ClientProfileModel>,
        ResMut<RoutedClientLifecycle>,
        ResMut<MatchFailureNotice>,
        ResMut<SessionPurpose>,
        MessageWriter<AppExit>,
        Res<ClientLocalLoadFailures>,
    ),
) {
    let (
        mut selection,
        mut game_draft,
        mut dashboard_focus,
        mut dashboard_notice,
        mut pending_created_brawler,
        mut pending_edited_brawler,
        mut creation_draft,
        mut brawler_edit,
        mut equipment_draft,
    ) = dashboard;
    let (
        mut queue,
        mut practice,
        mut loading,
        mut result_state,
        mut profile,
        mut routed,
        mut match_failure,
        mut purpose,
        mut exit,
        local_failures,
    ) = models;
    if let Some(decision) = profile.take_decision() {
        let accepted = matches!(decision, crate::profiles::ProfileDecision::Accepted);
        dashboard_notice.0 = Some(match decision {
            crate::profiles::ProfileDecision::Accepted => "Profile saved.".to_string(),
            crate::profiles::ProfileDecision::InvalidRequest => {
                "That brawler change is not valid.".to_string()
            }
            crate::profiles::ProfileDecision::StaleRevision => {
                "The profile changed; review it and try again.".to_string()
            }
            crate::profiles::ProfileDecision::MissingBrawler => {
                "That brawler no longer exists.".to_string()
            }
            crate::profiles::ProfileDecision::CapacityReached => {
                "Brawler limit reached (16).".to_string()
            }
            crate::profiles::ProfileDecision::QueueLocked => {
                "Leave the queue before changing a brawler.".to_string()
            }
            crate::profiles::ProfileDecision::TemporarilyUnavailable => {
                "Profile storage is temporarily unavailable; try again.".to_string()
            }
            crate::profiles::ProfileDecision::StorageFault => {
                "The profile could not be saved safely; owned data was preserved.".to_string()
            }
            crate::profiles::ProfileDecision::MissingPart => {
                "That weapon part is no longer in this inventory.".to_string()
            }
            crate::profiles::ProfileDecision::PartAlreadyEquipped => {
                "That physical part is already equipped on a brawler.".to_string()
            }
            crate::profiles::ProfileDecision::IncompatibleWeapon => {
                "Those parts do not form a valid weapon configuration.".to_string()
            }
        });
        if accepted
            && let Some(ordinal) = pending_created_brawler.0.take()
            && let Some(created) = profile.snapshot().and_then(|snapshot| {
                snapshot
                    .brawlers
                    .iter()
                    .find(|brawler| brawler.creation_ordinal == ordinal)
            })
        {
            dashboard_notice.0 = Some(format!("Created {}.", created.name));
            commit.overlay = Some(OverlayCommit::BrawlerDetails(created.id));
            commit.focus_index = Some(0);
        } else if accepted && let Some(brawler_id) = pending_edited_brawler.0.take() {
            commit.overlay = Some(OverlayCommit::BrawlerDetails(brawler_id));
            commit.focus_index = Some(1);
        } else if !accepted {
            if pending_created_brawler.0.take().is_some() {
                creation_draft.inline_error = dashboard_notice.0.clone();
                commit.overlay = Some(OverlayCommit::BrawlerCreation);
            }
            if pending_edited_brawler.0.take().is_some() {
                brawler_edit.inline_error = dashboard_notice.0.clone();
                commit.overlay = Some(OverlayCommit::BrawlerEditor);
            }
        }
    }
    if let Some(explicit) = actions.explicit.take() {
        match explicit {
            FlowUiAction::Cancel | FlowUiAction::Disconnect | FlowUiAction::ConfirmChangeServer => {
                commit.teardown = true;
                commit.overlay = Some(OverlayCommit::Clear);
                commit.next_flow = Some(if *purpose == SessionPurpose::Practice {
                    *purpose = SessionPurpose::Multiplayer;
                    ClientFlow::ServerSelect
                } else {
                    ClientFlow::ServerSelect
                });
                *selection = SelectedGameType::default();
                *game_draft = GameTypeSelectionDraft::default();
                dashboard_notice.0 = None;
                result_state.context = None;
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
                            return_flow: ClientFlow::Dashboard,
                            actions: [
                                Some(FlowErrorAction::RetrySave),
                                Some(FlowErrorAction::ContinueWithoutSaving),
                            ],
                        });
                    }
                    if commit.error.is_none()
                        && let Some(mut error) = local_load_error(*local_failures)
                    {
                        error.return_flow = ClientFlow::Dashboard;
                        commit.error = Some(error);
                    }
                    *selection = SelectedGameType::default();
                    dashboard_focus.0 = Some(DASHBOARD_PLAY_INDEX);
                    commit.next_flow = Some(ClientFlow::Dashboard);
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
                *game_draft = GameTypeSelectionDraft::default();
                dashboard_notice.0 = None;
                result_state.context = None;
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
                let destination = if result_state.context.is_some() {
                    ClientFlow::Results
                } else {
                    *purpose = SessionPurpose::Multiplayer;
                    dashboard_focus.0 = Some(DASHBOARD_PLAY_INDEX);
                    ClientFlow::Dashboard
                };
                commit.next_flow = Some(destination);
                if core::mem::take(&mut match_failure.0) {
                    commit.error = Some(FlowError {
                        kind: FlowErrorKind::Connection,
                        message: "The match server stopped unexpectedly".to_string(),
                        return_flow: ClientFlow::Dashboard,
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
            SessionObservation::PracticeRejected(reason) => {
                if matches!(
                    reason,
                    crate::lobby::PracticeStartRejection::StaleCatalog
                        | crate::lobby::PracticeStartRejection::StaleGameConfiguration
                        | crate::lobby::PracticeStartRejection::UnknownGameType
                ) {
                    commit.teardown = true;
                    fail_to_server_select_with_kind(
                        &mut commit,
                        FlowErrorKind::Content,
                        "The lobby content changed incompatibly; reconnect to obtain a fresh game list"
                            .to_string(),
                        true,
                    );
                } else {
                    commit.error = Some(FlowError {
                        kind: FlowErrorKind::Practice,
                        message: practice_rejection_copy(reason).to_string(),
                        return_flow: *flow.get(),
                        actions: [Some(FlowErrorAction::Back), None],
                    });
                }
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
                    commit.next_flow = Some(ClientFlow::Queue);
                    commit.overlay = Some(OverlayCommit::Clear);
                }
                crate::lobby::QueueDecision::Cancelled { .. } => {
                    *purpose = SessionPurpose::Multiplayer;
                    dashboard_focus.0 = Some(DASHBOARD_PLAY_INDEX);
                    commit.next_flow = Some(ClientFlow::Dashboard);
                    commit.overlay = Some(OverlayCommit::Clear);
                }
                crate::lobby::QueueDecision::Rejected(reason) => match reason {
                    crate::lobby::QueueRejection::IncompatiblePassives => {
                        commit.error = Some(FlowError {
                            kind: FlowErrorKind::Queue,
                            message: "The selected passives are incompatible".to_string(),
                            return_flow: *flow.get(),
                            actions: [Some(FlowErrorAction::Back), None],
                        });
                    }
                    crate::lobby::QueueRejection::OverBudget { used, budget } => {
                        commit.error = Some(FlowError {
                            kind: FlowErrorKind::Queue,
                            message: format!("The selected build uses {used} of {budget} points"),
                            return_flow: *flow.get(),
                            actions: [Some(FlowErrorAction::Back), None],
                        });
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
        FlowUiAction::Back => commit.overlay = Some(OverlayCommit::Clear),
        FlowUiAction::Quit => {
            exit.write(AppExit::Success);
        }
        FlowUiAction::OpenSettings => commit.overlay = Some(OverlayCommit::Settings),
        FlowUiAction::OpenCredits => commit.overlay = Some(OverlayCommit::Credits),
        FlowUiAction::OpenDashboardMenu => {
            commit.overlay = Some(OverlayCommit::DashboardMenu);
            commit.focus_index = Some(0);
        }
        FlowUiAction::OpenBrawlerList | FlowUiAction::BackToBrawlerList => {
            commit.overlay = Some(OverlayCommit::BrawlerList);
            commit.focus_index = Some(0);
        }
        FlowUiAction::CloseBrawlerList => {
            commit.overlay = Some(OverlayCommit::Clear);
            commit.focus_index = Some(DASHBOARD_BUILD_INDEX);
        }
        FlowUiAction::OpenBrawlerDetails(brawler_id) => {
            commit.overlay = Some(OverlayCommit::BrawlerDetails(brawler_id));
            commit.focus_index = Some(0);
        }
        FlowUiAction::CloseDashboardMenu | FlowUiAction::KeepServer => {
            commit.overlay = Some(OverlayCommit::Clear);
            commit.focus_index = Some(5);
        }
        FlowUiAction::RequestChangeServer => {
            commit.overlay = Some(OverlayCommit::ChangeServerConfirmation);
            commit.focus_index = Some(0);
        }
        FlowUiAction::Retry => match validate_target(&model.address, &model.name) {
            Ok(target) => {
                commit.start_target = Some(target);
                commit.next_flow = Some(ClientFlow::Connecting);
            }
            Err(error) => model.inline_error = Some(error),
        },
        FlowUiAction::RetrySave => {
            let result = save_connections(&path.0, &persistence.state);
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
        FlowUiAction::SelectGameTypeDraft(index) => {
            if membership
                .iter()
                .next()
                .is_some_and(|(membership, _, _)| index < membership.game_types.len())
            {
                game_draft.selected_index = Some(index);
            }
        }
        FlowUiAction::ConfirmGameType => {
            if let Some((membership, _, _)) = membership.iter().next()
                && accept_game_type_draft(&game_draft, membership, &mut selection)
            {
                dashboard_focus.0 = Some(DASHBOARD_GAME_INDEX);
                commit.next_flow = Some(ClientFlow::Dashboard);
            }
        }
        FlowUiAction::CancelGameType => {
            *game_draft = GameTypeSelectionDraft::default();
            dashboard_focus.0 = Some(DASHBOARD_GAME_INDEX);
            commit.next_flow = Some(ClientFlow::Dashboard);
        }
        FlowUiAction::ToggleFavoriteServer => {
            if let Some((membership, Some(target), _)) = membership.iter().next() {
                let removed = persistence
                    .state
                    .favorites
                    .iter()
                    .any(|favorite| favorite.address == target.logical_address)
                    && persistence.state.remove_favorite(&target.logical_address);
                if !removed
                    && let Err(error) = persistence
                        .state
                        .add_favorite(&membership.server_name, &target.logical_address)
                {
                    persistence.dirty_error = Some(error);
                    return;
                }
                if let Err(error) = save_connections(&path.0, &persistence.state) {
                    persistence.dirty_error = Some(error);
                    return;
                }
                commit.overlay = Some(OverlayCommit::Clear);
            }
        }
        FlowUiAction::CreateBrawler => {
            if queue.membership().is_some()
                || queue.pending().is_some()
                || practice.pending()
                || profile.pending()
            {
                return;
            }
            let Some(snapshot) = profile.snapshot() else {
                return;
            };
            let Some(catalog) = profile.catalog() else {
                return;
            };
            if snapshot.brawlers.len() >= usize::from(catalog.limits.maximum_saved_brawlers) {
                dashboard_notice.0 = Some(format!(
                    "Brawler limit reached ({}).",
                    catalog.limits.maximum_saved_brawlers
                ));
                commit.overlay = Some(OverlayCommit::Clear);
                return;
            }
            let (Some(fighter), Some(weapon), Some(ultimate)) = (
                catalog.fighter_profiles.first(),
                catalog.weapon_bases.first(),
                catalog.ultimates.first(),
            ) else {
                return;
            };
            *creation_draft = BrawlerCreationDraft {
                fighter_profile_id: fighter.id,
                weapon_base_id: weapon.id,
                ultimate: ultimate.id,
                inline_error: None,
            };
            commit.overlay = Some(OverlayCommit::BrawlerCreation);
        }
        FlowUiAction::CycleCreationProfile => {
            creation_draft.inline_error = None;
            let Some(catalog) = profile.catalog() else {
                return;
            };
            let index = catalog
                .fighter_profiles
                .iter()
                .position(|entry| entry.id == creation_draft.fighter_profile_id)
                .unwrap_or(0);
            creation_draft.fighter_profile_id =
                catalog.fighter_profiles[(index + 1) % catalog.fighter_profiles.len()].id;
        }
        FlowUiAction::CycleCreationWeapon => {
            creation_draft.inline_error = None;
            let Some(catalog) = profile.catalog() else {
                return;
            };
            let index = catalog
                .weapon_bases
                .iter()
                .position(|entry| entry.id == creation_draft.weapon_base_id)
                .unwrap_or(0);
            creation_draft.weapon_base_id =
                catalog.weapon_bases[(index + 1) % catalog.weapon_bases.len()].id;
        }
        FlowUiAction::CycleCreationUltimate => {
            creation_draft.inline_error = None;
            let Some(catalog) = profile.catalog() else {
                return;
            };
            let current = catalog
                .ultimates
                .iter()
                .position(|definition| definition.id == creation_draft.ultimate)
                .unwrap_or(0);
            creation_draft.ultimate = catalog.ultimates[(current + 1) % catalog.ultimates.len()].id;
        }
        FlowUiAction::CancelCreateBrawler => {
            commit.overlay = Some(OverlayCommit::BrawlerList);
        }
        FlowUiAction::CancelBrawlerEdit => {
            commit.overlay = brawler_edit
                .brawler_id
                .map_or(Some(OverlayCommit::BrawlerList), |id| {
                    Some(OverlayCommit::BrawlerDetails(id))
                });
        }
        FlowUiAction::CancelDeleteBrawler => {
            let details = match overlay.as_ref() {
                ClientOverlay::DeleteBrawlerConfirmation(id) => OverlayCommit::BrawlerDetails(*id),
                _ => OverlayCommit::BrawlerList,
            };
            commit.overlay = Some(details);
        }
        FlowUiAction::ConfirmCreateBrawler => {
            if !matches!(overlay.as_ref(), ClientOverlay::BrawlerCreation)
                || queue.membership().is_some()
                || queue.pending().is_some()
                || practice.pending()
                || profile.pending()
            {
                return;
            }
            let Some(snapshot) = profile.snapshot() else {
                return;
            };
            let ordinal = snapshot.next_brawler_ordinal;
            creation_draft.inline_error = None;
            let Some((passive_one, passive_two)) = profile.catalog().and_then(|catalog| {
                let mut passives = catalog.selectable_passives().map(|entry| entry.id);
                Some((passives.next()?, passives.next()?))
            }) else {
                return;
            };
            if profile.create(crate::profiles::BrawlerDraft {
                name: format!("Brawler {ordinal}"),
                fighter_profile_id: creation_draft.fighter_profile_id,
                weapon_base_id: creation_draft.weapon_base_id,
                ultimate_id: creation_draft.ultimate,
                passive_ids: [passive_one, passive_two],
            }) {
                pending_created_brawler.0 = Some(ordinal);
            }
        }
        FlowUiAction::SelectBrawler(brawler_id) => {
            if queue.membership().is_some()
                || queue.pending().is_some()
                || practice.pending()
                || profile.pending()
            {
                return;
            }
            let already_selected = profile
                .snapshot()
                .is_some_and(|snapshot| snapshot.selected_brawler_id == Some(brawler_id));
            if already_selected || profile.select(brawler_id) {
                commit.overlay = Some(OverlayCommit::Clear);
                commit.focus_index = Some(DASHBOARD_BUILD_INDEX);
            }
        }
        FlowUiAction::OpenBrawlerEditor(brawler_id) => {
            if queue.membership().is_some()
                || queue.pending().is_some()
                || practice.pending()
                || profile.pending()
            {
                return;
            }
            let selected = profile
                .snapshot()
                .and_then(|snapshot| {
                    snapshot
                        .brawlers
                        .iter()
                        .find(|brawler| brawler.id == brawler_id)
                })
                .cloned();
            if let Some(brawler) = selected {
                *brawler_edit = BrawlerEditDraft {
                    brawler_id: Some(brawler.id),
                    name_caret: brawler.name.len(),
                    name: brawler.name,
                    fighter_profile_id: brawler.fighter_profile_id,
                    weapon_base_id: brawler.weapon_base_id,
                    ultimate_id: brawler.ultimate_id,
                    passive_ids: brawler.passive_ids,
                    editing_name: false,
                    inline_error: None,
                };
                commit.overlay = Some(OverlayCommit::BrawlerEditor);
            }
        }
        FlowUiAction::OpenWeaponEquipment(brawler_id) => {
            if queue.membership().is_some()
                || queue.pending().is_some()
                || practice.pending()
                || profile.pending()
            {
                return;
            }
            let selected = profile.snapshot().and_then(|snapshot| {
                snapshot
                    .brawlers
                    .iter()
                    .find(|brawler| brawler.id == brawler_id)
            });
            if let Some(brawler) = selected {
                *equipment_draft = WeaponEquipmentDraft {
                    brawler_id: Some(brawler.id),
                    equipped_part_ids: brawler.equipped_part_ids,
                    selected_slot: 0,
                    inline_error: None,
                };
                commit.overlay = Some(OverlayCommit::WeaponEquipment);
            }
        }
        FlowUiAction::BeginBrawlerNameEdit => {
            brawler_edit.editing_name = true;
            brawler_edit.name_caret = brawler_edit.name.len();
            brawler_edit.inline_error = None;
        }
        FlowUiAction::CycleBrawlerUltimate => {
            let Some(catalog) = profile.catalog() else {
                return;
            };
            let index = catalog
                .ultimates
                .iter()
                .position(|definition| definition.id == brawler_edit.ultimate_id)
                .unwrap_or(0);
            brawler_edit.ultimate_id = catalog.ultimates[(index + 1) % catalog.ultimates.len()].id;
        }
        FlowUiAction::CycleBrawlerPassiveOne => {
            let Some(catalog) = profile.catalog() else {
                return;
            };
            let options: Vec<_> = catalog
                .selectable_passives()
                .map(|entry| entry.id)
                .collect();
            let index = options
                .iter()
                .position(|id| *id == brawler_edit.passive_ids[0])
                .unwrap_or(0);
            brawler_edit.passive_ids[0] = options[(index + 1) % options.len()];
            if brawler_edit.passive_ids[0] == brawler_edit.passive_ids[1] {
                brawler_edit.passive_ids[1] = options[(index + 2) % options.len()];
            }
        }
        FlowUiAction::CycleBrawlerPassiveTwo => {
            let Some(catalog) = profile.catalog() else {
                return;
            };
            let options: Vec<_> = catalog
                .selectable_passives()
                .map(|entry| entry.id)
                .collect();
            let index = options
                .iter()
                .position(|id| *id == brawler_edit.passive_ids[1])
                .unwrap_or(0);
            brawler_edit.passive_ids[1] = options[(index + 1) % options.len()];
            if brawler_edit.passive_ids[0] == brawler_edit.passive_ids[1] {
                brawler_edit.passive_ids[0] = options[(index + 2) % options.len()];
            }
        }
        FlowUiAction::ConfirmBrawlerEdit => {
            let Ok(name) = crate::lobby::normalize_proposed_display_name(&brawler_edit.name) else {
                brawler_edit.inline_error = Some("Enter a valid brawler name.".to_string());
                return;
            };
            let Some(brawler_id) = brawler_edit.brawler_id else {
                return;
            };
            if profile.edit(
                brawler_id,
                crate::profiles::BrawlerEdit {
                    name,
                    ultimate_id: brawler_edit.ultimate_id,
                    passive_ids: brawler_edit.passive_ids,
                },
            ) {
                pending_edited_brawler.0 = Some(brawler_id);
                brawler_edit.inline_error = None;
            }
        }
        FlowUiAction::SelectEquipmentSlot(slot) => {
            if slot < crate::weapon_parts::WEAPON_PART_SLOT_COUNT {
                equipment_draft.selected_slot = slot;
                equipment_draft.inline_error = None;
            }
        }
        FlowUiAction::EquipWeaponPart(part_id) => {
            let Some(snapshot) = profile.snapshot() else {
                return;
            };
            let Some(brawler_id) = equipment_draft.brawler_id else {
                return;
            };
            if snapshot.brawlers.iter().any(|brawler| {
                brawler.id != brawler_id && brawler.equipped_part_ids.contains(&Some(part_id))
            }) {
                equipment_draft.inline_error =
                    Some("That physical part is equipped on another brawler.".into());
                return;
            }
            for slot in &mut equipment_draft.equipped_part_ids {
                if *slot == Some(part_id) {
                    *slot = None;
                }
            }
            let selected_slot = equipment_draft.selected_slot;
            equipment_draft.equipped_part_ids[selected_slot] = Some(part_id);
            equipment_draft.inline_error = None;
        }
        FlowUiAction::UnequipWeaponPart => {
            let selected_slot = equipment_draft.selected_slot;
            equipment_draft.equipped_part_ids[selected_slot] = None;
            equipment_draft.inline_error = None;
        }
        FlowUiAction::ConfirmWeaponEquipment => {
            let Some(brawler_id) = equipment_draft.brawler_id else {
                return;
            };
            if profile.equip_weapon_parts(brawler_id, equipment_draft.equipped_part_ids) {
                commit.overlay = Some(OverlayCommit::BrawlerDetails(brawler_id));
            }
        }
        FlowUiAction::CancelWeaponEquipment => {
            commit.overlay = equipment_draft
                .brawler_id
                .map_or(Some(OverlayCommit::BrawlerList), |id| {
                    Some(OverlayCommit::BrawlerDetails(id))
                });
        }
        FlowUiAction::DeleteBrawler(brawler_id) => {
            if queue.membership().is_some()
                || queue.pending().is_some()
                || practice.pending()
                || profile.pending()
            {
                return;
            }
            commit.overlay = Some(OverlayCommit::DeleteBrawlerConfirmation(brawler_id));
        }
        FlowUiAction::ConfirmDeleteBrawler => {
            let ClientOverlay::DeleteBrawlerConfirmation(brawler_id) = overlay.as_ref() else {
                return;
            };
            let _ = profile.delete(*brawler_id);
            commit.overlay = Some(OverlayCommit::BrawlerList);
        }
        FlowUiAction::JoinQueue => {
            if *flow.get() != ClientFlow::Dashboard || practice.pending() {
                return;
            }
            *purpose = SessionPurpose::Multiplayer;
            let selected = membership.iter().next().and_then(|(membership, _, _)| {
                membership.profile.selected_brawler_id.and_then(|id| {
                    membership
                        .profile
                        .brawlers
                        .iter()
                        .find(|brawler| brawler.id == id)
                })
            });
            if let Some(brawler) = selected {
                let _ = queue.start_join(&selection, brawler.id, brawler.revision, time.elapsed());
            }
        }
        FlowUiAction::StartPractice => {
            if *flow.get() != ClientFlow::Dashboard || queue.pending().is_some() {
                return;
            }
            *purpose = SessionPurpose::Practice;
            let selected = membership.iter().next().and_then(|(membership, _, _)| {
                membership.profile.selected_brawler_id.and_then(|id| {
                    membership
                        .profile
                        .brawlers
                        .iter()
                        .find(|brawler| brawler.id == id)
                })
            });
            if let Some(brawler) = selected {
                let _ = practice.start(&selection, brawler.id, brawler.revision);
            }
        }
        FlowUiAction::QueueAgain => {
            let exact_game_type_id = result_state
                .context
                .as_ref()
                .and_then(|context| context.game_type_id.clone());
            if *purpose == SessionPurpose::Practice {
                let Some((membership, _, _)) = membership.iter().find(|(_, _, session)| {
                    session.kind == super::RoutedClientSessionKind::Lobby
                        && session.generation == routed.generation
                }) else {
                    return;
                };
                let Some(game_type_id) = exact_game_type_id else {
                    return;
                };
                let Some(game) = membership
                    .game_types
                    .iter()
                    .find(|game| game.id == game_type_id)
                else {
                    return;
                };
                selection.catalog_revision = Some(membership.catalog_revision);
                selection.game_type_id = Some(game.id.clone());
                selection.configuration_revision = Some(game.configuration_revision);
                let selected_brawler = membership.profile.selected_brawler_id.and_then(|id| {
                    membership
                        .profile
                        .brawlers
                        .iter()
                        .find(|brawler| brawler.id == id)
                });
                if selected_brawler
                    .is_none_or(|brawler| !practice.start(&selection, brawler.id, brawler.revision))
                {
                    commit.error = Some(FlowError {
                        kind: FlowErrorKind::Practice,
                        message: "The practice connection is unavailable.".to_string(),
                        return_flow: ClientFlow::Results,
                        actions: [Some(FlowErrorAction::Back), None],
                    });
                }
                return;
            }
            let current_lobby = membership.iter().find(|(_, _, session)| {
                session.kind == super::RoutedClientSessionKind::Lobby
                    && session.generation == routed.generation
            });
            if let Some((_, _, session)) = current_lobby {
                queue.bind_lobby_generation(session.generation);
            }
            let started = current_lobby.zip(exact_game_type_id).is_some_and(
                |((membership, _, session), game_type_id)| {
                    let started = queue.start_requeue_join(
                        session.generation,
                        membership,
                        &game_type_id,
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
        FlowUiAction::OpenGameTypeSelect => {
            if queue.pending().is_some() || practice.pending() {
                return;
            }
            if let Some((membership, _, _)) = membership.iter().next() {
                let selected_index = selection.game_type_id.as_ref().and_then(|selected| {
                    membership
                        .game_types
                        .iter()
                        .position(|game| game.id == *selected)
                });
                game_draft.selected_index =
                    selected_index.or_else(|| (!membership.game_types.is_empty()).then_some(0));
                game_draft.unavailable_previous =
                    selection.game_type_id.is_some() && selected_index.is_none();
                commit.next_flow = Some(ClientFlow::GameTypeSelect);
            }
        }
        FlowUiAction::ReturnToDashboard => {
            result_state.context = None;
            *purpose = SessionPurpose::Multiplayer;
            dashboard_focus.0 = Some(DASHBOARD_PLAY_INDEX);
            commit.next_flow = Some(ClientFlow::Dashboard);
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
                commit.overlay = Some(OverlayCommit::Clear);
            }
        }
        FlowUiAction::TryAgainQueue => {
            if queue.try_again_after_rate_limit(time.elapsed()) {
                commit.overlay = Some(OverlayCommit::Clear);
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
        FlowUiAction::Cancel | FlowUiAction::Disconnect | FlowUiAction::ConfirmChangeServer => {}
    }
    let _ = flow;
}

fn accept_game_type_draft(
    draft: &GameTypeSelectionDraft,
    membership: &ClientLobbyMembership,
    selection: &mut SelectedGameType,
) -> bool {
    let Some(game_type) = draft
        .selected_index
        .and_then(|index| membership.game_types.get(index))
    else {
        return false;
    };
    selection.catalog_revision = Some(membership.catalog_revision);
    selection.game_type_id = Some(game_type.id.clone());
    selection.configuration_revision = Some(game_type.configuration_revision);
    true
}

const fn practice_rejection_copy(reason: crate::lobby::PracticeStartRejection) -> &'static str {
    match reason {
        crate::lobby::PracticeStartRejection::StaleCatalog
        | crate::lobby::PracticeStartRejection::StaleGameConfiguration => {
            "The server's game catalog changed. Choose the game again."
        }
        crate::lobby::PracticeStartRejection::UnknownGameType => {
            "That practice game is no longer available."
        }
        crate::lobby::PracticeStartRejection::InvalidBuild => {
            "The selected build was rejected by the server."
        }
        crate::lobby::PracticeStartRejection::Busy => "Another match start is already in progress.",
        crate::lobby::PracticeStartRejection::CapacityUnavailable => {
            "The server has no free match capacity right now."
        }
        crate::lobby::PracticeStartRejection::Internal => "The server could not start practice.",
    }
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
    assets: Option<Res<super::ClientAssetHandles>>,
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
            if let Some(assets) = assets.as_deref() {
                root.spawn((
                    ImageNode::new(assets.loading_logo.clone()),
                    Node {
                        width: percent(62),
                        max_width: px(560),
                        height: auto(),
                        margin: UiRect::bottom(px(18)),
                        ..default()
                    },
                ));
            } else {
                spawn_heading(root, "PEWPEW BLITZ");
            }
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
                spawn_flow_button(panel, 1, FlowUiAction::OpenSettings, "SETTINGS", None);
                spawn_flow_button(panel, 2, FlowUiAction::Quit, "QUIT", None);
                panel.spawn((
                    Text::new("ESC / PAD EAST  -  CANCEL"),
                    TextFont::from_font_size(14.0),
                    TextColor(Color::srgb(0.58, 0.66, 0.74)),
                ));
            });
        });
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "the dashboard entry renders one bounded authenticated product snapshot"
)]
fn spawn_dashboard(
    mut commands: Commands,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    mut navigation: ResMut<FlowNavigation>,
    mut selection: ResMut<SelectedGameType>,
    profile: Res<super::ClientProfileModel>,
    queue: Res<super::ClientQueueModel>,
    practice: Res<super::ClientPracticeModel>,
    mut purpose: ResMut<SessionPurpose>,
    mut return_focus: ResMut<DashboardReturnFocus>,
    mut notice: ResMut<DashboardNotice>,
    assets: Option<Res<super::ClientAssetHandles>>,
) {
    let Some(membership) = memberships.iter().next() else {
        return;
    };
    *purpose = SessionPurpose::Multiplayer;
    let previous_game_type = selection.game_type_id.clone();
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
    let Some(game) = membership.game_types.get(selected_index) else {
        return;
    };
    selection.catalog_revision = Some(membership.catalog_revision);
    selection.game_type_id = Some(game.id.clone());
    selection.configuration_revision = Some(game.configuration_revision);
    if previous_game_type.is_some() && previous_game_type.as_ref() != Some(&game.id) {
        notice.0 = Some(format!(
            "The previous game is unavailable. {} is now selected.",
            game.display_name
        ));
    }
    navigation.selected = return_focus.0.take().unwrap_or(DASHBOARD_PLAY_INDEX);
    let dashboard_notice = notice.0.take();
    let admission_pending = queue.pending().is_some() || practice.pending() || profile.pending();
    let selected_brawler = membership.profile.selected_brawler_id.and_then(|id| {
        membership
            .profile
            .brawlers
            .iter()
            .find(|brawler| brawler.id == id)
    });
    let build_name =
        selected_brawler.map_or("CREATE YOUR FIRST BRAWLER", |brawler| brawler.name.as_str());
    let build_summary = selected_brawler.map_or_else(
        || "Choose a permanent fighter profile and weapon base".to_string(),
        |brawler| brawler_loadout_summary(brawler, &membership.brawler_catalog),
    );
    let game_summary = dashboard_game_summary(game);
    let population = if queue.required_snapshot_is_fresh() {
        queue_population(&queue, game)
    } else {
        "Population updating".to_string()
    };
    let capacity_occupied = queue.snapshot().is_some_and(|snapshot| {
        snapshot.formation_availability == crate::lobby::FormationAvailability::ProductMatchOccupied
    });
    let build_accessible = format!("View brawlers: {build_name}, {build_summary}");
    let game_accessible = format!(
        "Change game type: {}, {game_summary}, {population}",
        game.display_name
    );

    commands
        .spawn((
            FlowRoot,
            DashboardRoot,
            DashboardLayoutRole::Root,
            DashboardLayoutClass::Wide,
            DespawnOnExit(ClientFlow::Dashboard),
            dashboard_root_node(),
            ScrollPosition::default(),
            BackgroundColor(Color::NONE),
            GlobalZIndex(410),
        ))
        .with_children(|root| {
            root.spawn((
                DashboardLayoutRole::Header,
                Node {
                    width: percent(100),
                    display: Display::Flex,
                    align_items: AlignItems::Center,
                    column_gap: px(10),
                    padding: UiRect::axes(px(18), px(6)),
                    ..default()
                },
            ))
            .with_children(|header| {
                if let Some(assets) = assets.as_deref() {
                    header.spawn((
                        DashboardLayoutRole::Wordmark,
                        ImageNode::new(assets.wordmark.clone()),
                        Node {
                            width: px(220),
                            height: auto(),
                            ..default()
                        },
                    ));
                } else {
                    header.spawn((
                        DashboardLayoutRole::Wordmark,
                        Text::new("PEWPEW BLITZ"),
                        dashboard_font(assets.as_deref(), 32.0),
                        TextColor(Color::srgb(0.28, 0.92, 1.0)),
                    ));
                }
                header
                    .spawn((
                        DashboardLayoutRole::Identity,
                        AccessibleLabel::new(format!(
                            "Player {}, server {}, online",
                            membership.accepted_display_name, membership.server_name
                        )),
                        Node {
                            min_width: px(240),
                            flex_direction: FlexDirection::Column,
                            padding: UiRect::axes(px(14), px(7)),
                            border: UiRect::all(px(1)),
                            border_radius: BorderRadius::all(px(11)),
                            ..default()
                        },
                        BackgroundColor(Color::srgba(0.035, 0.12, 0.24, 0.92)),
                        BorderColor::all(Color::srgba(0.15, 0.5, 0.8, 0.45)),
                    ))
                    .with_children(|identity| {
                        identity.spawn((
                            Text::new(&membership.accepted_display_name),
                            dashboard_font(assets.as_deref(), 22.0),
                            TextColor(Color::WHITE),
                        ));
                        identity.spawn((
                            Text::new(format!("SERVER: {}  ONLINE", membership.server_name)),
                            TextFont::from_font_size(12.0),
                            TextColor(Color::srgb(0.2, 0.9, 0.72)),
                        ));
                    });
                header.spawn((
                    DashboardLayoutRole::HeaderSpacer,
                    Node {
                        flex_grow: 1.0,
                        ..default()
                    },
                ));
                spawn_dashboard_button(
                    header,
                    DASHBOARD_SETTINGS_INDEX,
                    FlowUiAction::OpenSettings,
                    DashboardButtonPresentation {
                        label: "SETTINGS",
                        width: px(112),
                        primary: false,
                        disabled: false,
                        assets: assets.as_deref(),
                        icon: assets.as_deref().map(|assets| assets.settings_icon.clone()),
                    },
                );
                spawn_dashboard_button(
                    header,
                    DASHBOARD_MENU_INDEX,
                    FlowUiAction::OpenDashboardMenu,
                    DashboardButtonPresentation {
                        label: "MENU",
                        width: px(92),
                        primary: false,
                        disabled: false,
                        assets: assets.as_deref(),
                        icon: assets.as_deref().map(|assets| assets.menu_icon.clone()),
                    },
                );
            });
            if let Some(message) = dashboard_notice {
                root.spawn((
                    Text::new(message),
                    TextFont::from_font_size(13.0),
                    TextColor(Color::srgb(1.0, 0.86, 0.48)),
                ));
            }
            root.spawn((
                DashboardLayoutRole::Center,
                Node {
                    width: percent(100),
                    flex_grow: 1.0,
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    row_gap: px(5),
                    ..default()
                },
            ))
            .with_children(|center| {
                let mut preview_button = center.spawn((
                    DashboardPreviewHost,
                    DashboardLayoutRole::Preview,
                    DashboardButtonStyle::Preview,
                    AccessibleLabel::new(build_accessible.clone()),
                    Button,
                    FlowButton {
                        index: DASHBOARD_BUILD_INDEX,
                        action: if selected_brawler.is_some() {
                            FlowUiAction::OpenBrawlerList
                        } else {
                            FlowUiAction::CreateBrawler
                        },
                        error_action: false,
                    },
                    Node {
                        width: percent(54),
                        max_width: px(650),
                        min_height: px(280),
                        max_height: px(470),
                        flex_grow: 1.0,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::End,
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(24)),
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::NONE),
                    BorderColor::all(Color::NONE),
                ));
                if admission_pending {
                    preview_button.insert(InteractionDisabled);
                }
                let mut build_button = center.spawn((
                    Button,
                    DashboardBuildCard,
                    DashboardLayoutRole::Build,
                    DashboardButtonStyle::Build,
                    AccessibleLabel::new(build_accessible.clone()),
                    FlowButton {
                        index: DASHBOARD_BUILD_INDEX,
                        action: if selected_brawler.is_some() {
                            FlowUiAction::OpenBrawlerList
                        } else {
                            FlowUiAction::CreateBrawler
                        },
                        error_action: false,
                    },
                    Node {
                        width: percent(30),
                        max_width: px(365),
                        min_height: px(104),
                        column_gap: px(12),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        padding: UiRect::axes(px(12), px(8)),
                        border: UiRect::all(px(2)),
                        border_radius: BorderRadius::all(px(14)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.91, 0.95, 1.0)),
                    BorderColor::all(Color::srgb(0.55, 0.7, 0.9)),
                    BoxShadow::new(
                        Color::srgba(0.0, 0.02, 0.08, 0.65),
                        px(0),
                        px(5),
                        px(0),
                        px(3),
                    ),
                ));
                if admission_pending {
                    build_button.insert(InteractionDisabled);
                }
                build_button.with_children(|card| {
                    spawn_dashboard_icon_well(
                        card,
                        assets.as_deref().map(|assets| assets.build_icon.clone()),
                        52.0,
                        31.0,
                        Color::srgb(0.05, 0.34, 0.82),
                    );
                    card.spawn((Node {
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },))
                        .with_children(|details| {
                            details.spawn((
                                Text::new(build_name.to_uppercase()),
                                DashboardBrawlerNameLabel,
                                dashboard_font(assets.as_deref(), 24.0),
                                TextColor(Color::srgb(0.035, 0.12, 0.32)),
                            ));
                            details.spawn((
                                Text::new(build_summary),
                                DashboardBrawlerSummaryLabel,
                                TextFont::from_font_size(12.0),
                                TextColor(Color::srgb(0.08, 0.18, 0.35)),
                            ));
                            details.spawn((
                                Text::new(if selected_brawler.is_some() {
                                    "VIEW BRAWLERS"
                                } else {
                                    "CREATE BRAWLER"
                                }),
                                dashboard_font(assets.as_deref(), 15.0),
                                TextColor(Color::srgb(0.03, 0.36, 0.82)),
                            ));
                        });
                });
            });
            root.spawn((
                DashboardLayoutRole::ActionRow,
                Node {
                    width: percent(94),
                    max_width: px(1180),
                    display: Display::Flex,
                    flex_direction: FlexDirection::Row,
                    column_gap: px(12),
                    justify_content: JustifyContent::Center,
                    align_items: AlignItems::Stretch,
                    ..default()
                },
            ))
            .with_children(|actions| {
                let mut mode_button = actions.spawn((
                    Button,
                    DashboardModeCard,
                    DashboardLayoutRole::Mode,
                    DashboardButtonStyle::Mode,
                    AccessibleLabel::new(game_accessible),
                    FlowButton {
                        index: DASHBOARD_GAME_INDEX,
                        action: FlowUiAction::OpenGameTypeSelect,
                        error_action: false,
                    },
                    Node {
                        width: percent(44),
                        min_height: px(104),
                        column_gap: px(14),
                        align_items: AlignItems::Center,
                        justify_content: JustifyContent::Center,
                        padding: UiRect::axes(px(18), px(10)),
                        border: UiRect::all(px(3)),
                        border_radius: BorderRadius::all(px(16)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.92, 0.95, 1.0)),
                    BorderColor::all(Color::srgb(0.55, 0.7, 0.9)),
                    BoxShadow::new(
                        Color::srgba(0.0, 0.02, 0.08, 0.65),
                        px(0),
                        px(5),
                        px(0),
                        px(3),
                    ),
                ));
                if admission_pending {
                    mode_button.insert(InteractionDisabled);
                }
                mode_button.with_children(|card| {
                    spawn_dashboard_icon_well(
                        card,
                        assets.as_deref().map(|assets| assets.mode_icon.clone()),
                        68.0,
                        42.0,
                        Color::srgb(0.05, 0.34, 0.82),
                    );
                    card.spawn((Node {
                        flex_direction: FlexDirection::Column,
                        justify_content: JustifyContent::Center,
                        ..default()
                    },))
                        .with_children(|details| {
                            details.spawn((
                                Text::new(game.display_name.to_uppercase()),
                                dashboard_font(assets.as_deref(), 28.0),
                                TextColor(Color::srgb(0.035, 0.12, 0.32)),
                            ));
                            details.spawn((
                                Text::new(format!("{game_summary}\n{population}")),
                                DashboardGameSummaryLabel,
                                TextFont::from_font_size(12.0),
                                TextColor(Color::srgb(0.08, 0.18, 0.35)),
                                TextLayout::new(Justify::Left, LineBreak::WordBoundary),
                            ));
                        });
                });
                spawn_dashboard_button(
                    actions,
                    DASHBOARD_PRACTICE_INDEX,
                    FlowUiAction::StartPractice,
                    DashboardButtonPresentation {
                        label: if practice.pending() {
                            "STARTING..."
                        } else {
                            "PRACTICE"
                        },
                        width: percent(21),
                        primary: false,
                        disabled: admission_pending || selected_brawler.is_none(),
                        assets: assets.as_deref(),
                        icon: assets.as_deref().map(|assets| assets.practice_icon.clone()),
                    },
                );
                spawn_dashboard_button(
                    actions,
                    DASHBOARD_PLAY_INDEX,
                    FlowUiAction::JoinQueue,
                    DashboardButtonPresentation {
                        label: if queue.pending().is_some() {
                            "JOINING..."
                        } else if capacity_occupied {
                            "MATCH IN PROGRESS"
                        } else {
                            "PLAY"
                        },
                        width: percent(33),
                        primary: true,
                        disabled: capacity_occupied
                            || admission_pending
                            || selected_brawler.is_none(),
                        assets: assets.as_deref(),
                        icon: assets.as_deref().map(|assets| assets.play_icon.clone()),
                    },
                );
            });
        });
}

fn dashboard_game_summary(game: &crate::lobby::AdvertisedGameType) -> String {
    let maps = crate::map::MapContentCatalog::embedded().ok().map_or_else(
        || "Map pool unavailable".to_string(),
        |catalog| {
            game.map_preset_ids
                .iter()
                .map(|id| {
                    catalog
                        .presets
                        .iter()
                        .find(|preset| preset.id == *id)
                        .map_or_else(
                            || format!("Map {}", id.0),
                            |preset| preset.display_name.clone(),
                        )
                })
                .collect::<Vec<_>>()
                .join(", ")
        },
    );
    let rules = match game.rules_summary {
        crate::lobby::AdvertisedRulesSummary::Wipeout {
            target_score,
            active_limit_ticks,
        } => format!(
            "First to {target_score} - {}s limit",
            active_limit_ticks / 60
        ),
        crate::lobby::AdvertisedRulesSummary::HotZone {
            target_progress_ticks,
            active_limit_ticks,
        } => format!(
            "Hold {}s - {}s limit",
            target_progress_ticks / 60,
            active_limit_ticks / 60
        ),
        crate::lobby::AdvertisedRulesSummary::Heist {
            safe_maximum_health,
            active_limit_ticks,
        } => format!(
            "Destroy the {safe_maximum_health} HP enemy idol - {}s limit",
            active_limit_ticks / 60
        ),
    };
    format!("{rules}\nMap pool: {maps}")
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
        crate::lobby::MatchLoadingPhase::Synchronizing => "Synchronizing map",
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
fn present_change_server_confirmation(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    roots: Query<Entity, With<ChangeServerConfirmationRoot>>,
    mut navigation: ResMut<FlowNavigation>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::ChangeServerConfirmation) {
        for entity in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    if roots.iter().next().is_some() || *flow.get() != ClientFlow::Dashboard {
        return;
    }
    navigation.selected = 0;
    commands
        .spawn((
            ChangeServerConfirmationRoot,
            DespawnOnExit(ClientFlow::Dashboard),
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
                spawn_heading(panel, "CHANGE SERVER?");
                panel.spawn((
                    Text::new("This disconnects from the current lobby."),
                    TextColor(Color::srgb(0.75, 0.84, 0.9)),
                ));
                spawn_flow_error_button(panel, 0, FlowUiAction::KeepServer, "STAY CONNECTED");
                spawn_flow_error_button(
                    panel,
                    1,
                    FlowUiAction::ConfirmChangeServer,
                    "CHANGE SERVER",
                );
            });
        });
}

#[allow(clippy::needless_pass_by_value)]
fn open_empty_profile_creation(
    profile: Res<super::ClientProfileModel>,
    mut overlay: ResMut<ClientOverlay>,
    mut draft: ResMut<BrawlerCreationDraft>,
) {
    if matches!(overlay.as_ref(), ClientOverlay::None)
        && profile
            .snapshot()
            .is_some_and(|snapshot| snapshot.brawlers.is_empty())
    {
        let Some(catalog) = profile.catalog() else {
            return;
        };
        let (Some(fighter), Some(weapon), Some(ultimate)) = (
            catalog.fighter_profiles.first(),
            catalog.weapon_bases.first(),
            catalog.ultimates.first(),
        ) else {
            return;
        };
        *draft = BrawlerCreationDraft {
            fighter_profile_id: fighter.id,
            weapon_base_id: weapon.id,
            ultimate: ultimate.id,
            inline_error: None,
        };
        *overlay = ClientOverlay::BrawlerCreation;
    }
}

fn advertised_fighter_name(
    catalog: &crate::profiles::AdvertisedBrawlerCatalog,
    id: crate::profiles::FighterProfileId,
) -> &str {
    catalog
        .fighter(id)
        .map_or("Unknown", |definition| definition.display_name.as_str())
}

fn advertised_weapon_name(
    catalog: &crate::profiles::AdvertisedBrawlerCatalog,
    id: crate::profiles::WeaponBaseId,
) -> &str {
    catalog
        .weapon(id)
        .map_or("Unknown", |definition| definition.display_name.as_str())
}

fn ultimate_name(
    catalog: &crate::profiles::AdvertisedBrawlerCatalog,
    id: crate::builds::UltimateDefinitionId,
) -> &str {
    catalog
        .ultimate(id)
        .map_or("Unknown", |definition| definition.display_name.as_str())
}

fn passive_name(
    catalog: &crate::profiles::AdvertisedBrawlerCatalog,
    id: crate::builds::PassiveDefinitionId,
) -> &str {
    catalog
        .passive(id)
        .map_or("Unknown", |definition| definition.display_name.as_str())
}

fn brawler_loadout_summary(
    brawler: &crate::profiles::SavedBrawler,
    catalog: &crate::profiles::AdvertisedBrawlerCatalog,
) -> String {
    format!(
        "{} · {}\n{} · {} + {}",
        advertised_fighter_name(catalog, brawler.fighter_profile_id),
        advertised_weapon_name(catalog, brawler.weapon_base_id),
        ultimate_name(catalog, brawler.ultimate_id),
        passive_name(catalog, brawler.passive_ids[0]),
        passive_name(catalog, brawler.passive_ids[1]),
    )
}

fn fighter_profile_stats(
    catalog: &crate::profiles::AdvertisedBrawlerCatalog,
    id: crate::profiles::FighterProfileId,
) -> Option<crate::builds::ResolvedFighterStats> {
    catalog.fighter(id).map(|definition| definition.stats)
}

fn spawn_brawler_list_row(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    brawler: &crate::profiles::SavedBrawler,
    selected: bool,
    catalog: &crate::profiles::AdvertisedBrawlerCatalog,
    disabled: bool,
) {
    let summary = brawler_loadout_summary(brawler, catalog);
    let status = if selected {
        " · SELECTED FOR PLAY"
    } else {
        ""
    };
    let mut row = parent.spawn((
        Button,
        AccessibleLabel::new(format!("{}{}: {summary}", brawler.name, status)),
        FlowButton {
            index,
            action: FlowUiAction::OpenBrawlerDetails(brawler.id),
            error_action: true,
        },
        Node {
            width: percent(100),
            min_height: px(92),
            flex_direction: FlexDirection::Column,
            align_items: AlignItems::FlexStart,
            justify_content: JustifyContent::Center,
            row_gap: px(4),
            padding: UiRect::axes(px(18), px(11)),
            border: UiRect::all(px(3)),
            border_radius: BorderRadius::all(px(12)),
            ..default()
        },
        BackgroundColor(Color::srgb(0.09, 0.14, 0.2)),
        BorderColor::all(if selected {
            Color::srgb(0.2, 0.9, 0.72)
        } else {
            Color::NONE
        }),
    ));
    if disabled {
        row.insert(InteractionDisabled);
    }
    row.with_children(|row| {
        row.spawn((
            Text::new(format!(
                "{}{}",
                brawler.name.to_uppercase(),
                if selected {
                    "  ✓ SELECTED FOR PLAY"
                } else {
                    ""
                }
            )),
            TextFont::from_font_size(21.0),
            TextColor(if selected {
                Color::srgb(0.2, 0.95, 0.75)
            } else {
                Color::WHITE
            }),
        ));
        row.spawn((
            Text::new(summary),
            TextFont::from_font_size(15.0),
            TextColor(Color::srgb(0.72, 0.84, 0.94)),
        ));
    });
}

fn spawn_brawler_screen_button(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    label: &str,
    width: Val,
    disabled: bool,
    color: Color,
) {
    let mut button = parent.spawn((
        Button,
        FlowButton {
            index,
            action,
            error_action: true,
        },
        Node {
            width,
            min_height: px(52),
            padding: UiRect::axes(px(16), px(10)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(px(2)),
            border_radius: BorderRadius::all(px(9)),
            ..default()
        },
        BackgroundColor(color),
        BorderColor::all(Color::srgb(0.38, 0.78, 1.0)),
    ));
    if disabled {
        button.insert(InteractionDisabled);
    }
    button.with_child((
        Text::new(label),
        TextFont::from_font_size(18.0),
        TextColor(Color::WHITE),
    ));
}

#[allow(clippy::needless_pass_by_value)]
fn scroll_brawler_list(
    overlay: Res<ClientOverlay>,
    mut events: MessageReader<MouseWheel>,
    mut areas: Query<&mut ScrollPosition, With<BrawlerListScrollArea>>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::BrawlerList) {
        return;
    }
    let delta = events
        .read()
        .map(|event| match event.unit {
            MouseScrollUnit::Line => event.y * 36.0,
            MouseScrollUnit::Pixel => event.y,
        })
        .sum::<f32>();
    if delta.abs() <= f32::EPSILON {
        return;
    }
    for mut position in &mut areas {
        position.0.y = (position.0.y - delta).max(0.0);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one cohesive bounded list builder keeps touch layout, navigation indices, and snapshot reconciliation adjacent"
)]
fn present_brawler_list(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    profile: Res<super::ClientProfileModel>,
    roots: Query<(Entity, &BrawlerListRoot)>,
    scroll_areas: Query<&ScrollPosition, With<BrawlerListScrollArea>>,
    mut navigation: ResMut<FlowNavigation>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::BrawlerList)
        || *flow.get() != ClientFlow::Dashboard
    {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    let Some(snapshot) = profile.snapshot() else {
        return;
    };
    let Some(catalog) = profile.catalog() else {
        return;
    };
    let pending = profile.pending();
    let render_key = BrawlerListRoot {
        profile_revision: snapshot.revision,
        pending,
    };
    if roots.iter().any(|(_, root)| *root == render_key) {
        return;
    }
    let retained_scroll = scroll_areas.iter().next().cloned().unwrap_or_default();
    let first_render = roots.is_empty();
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    if first_render {
        navigation.selected = snapshot
            .selected_brawler_id
            .and_then(|selected| {
                snapshot
                    .brawlers
                    .iter()
                    .position(|brawler| brawler.id == selected)
            })
            .unwrap_or(0);
    }
    commands
        .spawn((
            render_key,
            DespawnOnExit(ClientFlow::Dashboard),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                ..default()
            },
            BackgroundColor(Color::srgb(0.018, 0.11, 0.24)),
            GlobalZIndex(510),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(100),
                    min_height: px(72),
                    align_items: AlignItems::Center,
                    column_gap: px(14),
                    padding: UiRect::axes(px(18), px(10)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.04, 0.085)),
            ))
            .with_children(|header| {
                spawn_brawler_screen_button(
                    header,
                    snapshot.brawlers.len() + 1,
                    FlowUiAction::CloseBrawlerList,
                    "‹ DASHBOARD",
                    px(170),
                    false,
                    Color::srgb(0.06, 0.22, 0.4),
                );
                header.spawn((
                    Text::new("BRAWLERS"),
                    TextFont::from_font_size(34.0),
                    TextColor(Color::WHITE),
                ));
                header.spawn((Node {
                    flex_grow: 1.0,
                    ..default()
                },));
                header.spawn((
                    Text::new(format!(
                        "{} / {} SAVED",
                        snapshot.brawlers.len(),
                        catalog.limits.maximum_saved_brawlers
                    )),
                    TextFont::from_font_size(16.0),
                    TextColor(Color::srgb(0.42, 0.88, 1.0)),
                ));
            });
            root.spawn((
                Node {
                    width: percent(100),
                    padding: UiRect::axes(px(24), px(14)),
                    align_items: AlignItems::Center,
                    column_gap: px(16),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.025, 0.2, 0.42)),
            ))
            .with_children(|intro| {
                intro.spawn((
                    Text::new("CHOOSE YOUR BRAWLER"),
                    TextFont::from_font_size(22.0),
                    TextColor(Color::WHITE),
                ));
                intro.spawn((
                    Text::new("Tap any saved brawler to inspect its build."),
                    TextFont::from_font_size(15.0),
                    TextColor(Color::srgb(0.72, 0.88, 0.98)),
                ));
            });
            root.spawn((Node {
                width: percent(100),
                min_height: px(0),
                flex_grow: 1.0,
                justify_content: JustifyContent::Center,
                padding: UiRect::axes(px(22), px(12)),
                ..default()
            },))
                .with_children(|body| {
                    body.spawn((
                        BrawlerListScrollArea,
                        retained_scroll,
                        Node {
                            width: percent(100),
                            max_width: px(1040),
                            min_height: px(0),
                            flex_grow: 1.0,
                            flex_shrink: 1.0,
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Stretch,
                            row_gap: px(11),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                    ))
                    .with_children(|list| {
                        if snapshot.brawlers.is_empty() {
                            list.spawn((
                                Text::new("NO BRAWLERS YET\nCreate one to enter Play or Practice."),
                                TextFont::from_font_size(21.0),
                                TextColor(Color::srgb(0.78, 0.86, 0.94)),
                            ));
                        }
                        for (index, brawler) in snapshot.brawlers.iter().enumerate() {
                            spawn_brawler_list_row(
                                list,
                                index,
                                brawler,
                                snapshot.selected_brawler_id == Some(brawler.id),
                                catalog,
                                pending,
                            );
                        }
                    });
                });
            root.spawn((
                Node {
                    width: percent(100),
                    justify_content: JustifyContent::Center,
                    padding: UiRect::axes(px(22), px(12)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.055, 0.11)),
            ))
            .with_children(|footer| {
                footer
                    .spawn((Node {
                        width: percent(100),
                        max_width: px(1040),
                        ..default()
                    },))
                    .with_children(|actions| {
                        let create_index = snapshot.brawlers.len();
                        spawn_brawler_screen_button(
                            actions,
                            create_index,
                            FlowUiAction::CreateBrawler,
                            "+ CREATE BRAWLER",
                            percent(100),
                            pending
                                || snapshot.brawlers.len()
                                    >= usize::from(catalog.limits.maximum_saved_brawlers),
                            Color::srgb(0.03, 0.5, 0.78),
                        );
                    });
            });
        });
}

#[allow(clippy::needless_pass_by_value)]
fn keep_brawler_list_focus_visible(
    overlay: Res<ClientOverlay>,
    navigation: Res<FlowNavigation>,
    buttons: Query<(&FlowButton, &ChildOf, &ComputedNode, &UiGlobalTransform)>,
    mut areas: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            &mut ScrollPosition,
        ),
        With<BrawlerListScrollArea>,
    >,
    mut prior: Local<Option<(Entity, usize)>>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::BrawlerList) {
        *prior = None;
        return;
    }
    let Some((area_entity, area_node, area_transform, mut scroll)) = areas.iter_mut().next() else {
        *prior = None;
        return;
    };
    let focus_key = (area_entity, navigation.selected);
    if prior.as_ref() == Some(&focus_key) {
        return;
    }
    *prior = Some(focus_key);
    let Some((_, _, button_node, button_transform)) =
        buttons.iter().find(|(button, child_of, _, _)| {
            child_of.parent() == area_entity && button.index == navigation.selected
        })
    else {
        return;
    };
    if area_node.is_empty() || button_node.is_empty() {
        return;
    }
    let (_, _, area_center) = area_transform.to_scale_angle_translation();
    let (_, _, button_center) = button_transform.to_scale_angle_translation();
    let visible_top = area_center.y - area_node.size().y * 0.5 + 8.0;
    let visible_bottom = area_center.y + area_node.size().y * 0.5 - 8.0;
    let button_top = button_center.y - button_node.size().y * 0.5;
    let button_bottom = button_center.y + button_node.size().y * 0.5;
    if button_top < visible_top {
        scroll.0.y = (scroll.0.y - (visible_top - button_top)).max(0.0);
    } else if button_bottom > visible_bottom {
        scroll.0.y += button_bottom - visible_bottom;
    }
}

#[allow(clippy::needless_pass_by_value)]
fn scroll_brawler_details(
    overlay: Res<ClientOverlay>,
    mut events: MessageReader<MouseWheel>,
    mut areas: Query<&mut ScrollPosition, With<BrawlerDetailsScrollArea>>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::BrawlerDetails(_)) {
        return;
    }
    let delta = events
        .read()
        .map(|event| match event.unit {
            MouseScrollUnit::Line => event.y * 36.0,
            MouseScrollUnit::Pixel => event.y,
        })
        .sum::<f32>();
    if delta.abs() <= f32::EPSILON {
        return;
    }
    for mut position in &mut areas {
        position.0.y = (position.0.y - delta).max(0.0);
    }
}

#[allow(clippy::needless_pass_by_value)]
fn keep_brawler_details_focus_visible(
    overlay: Res<ClientOverlay>,
    navigation: Res<FlowNavigation>,
    buttons: Query<(Entity, &FlowButton, &ComputedNode, &UiGlobalTransform)>,
    parents: Query<&ChildOf>,
    mut areas: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            &mut ScrollPosition,
        ),
        With<BrawlerDetailsScrollArea>,
    >,
    mut prior: Local<Option<(Entity, usize)>>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::BrawlerDetails(_)) {
        *prior = None;
        return;
    }
    let Some((area_entity, area_node, area_transform, mut scroll)) = areas.iter_mut().next() else {
        *prior = None;
        return;
    };
    let focus_key = (area_entity, navigation.selected);
    if prior.as_ref() == Some(&focus_key) {
        return;
    }
    *prior = Some(focus_key);
    let Some((_, _, button_node, button_transform)) =
        buttons.iter().find(|(entity, button, _, _)| {
            button.index == navigation.selected
                && parents
                    .iter_ancestors(*entity)
                    .any(|ancestor| ancestor == area_entity)
        })
    else {
        return;
    };
    if area_node.is_empty() || button_node.is_empty() {
        return;
    }
    let (_, _, area_center) = area_transform.to_scale_angle_translation();
    let (_, _, button_center) = button_transform.to_scale_angle_translation();
    let visible_top = area_center.y - area_node.size().y * 0.5 + 8.0;
    let visible_bottom = area_center.y + area_node.size().y * 0.5 - 8.0;
    let button_top = button_center.y - button_node.size().y * 0.5;
    let button_bottom = button_center.y + button_node.size().y * 0.5;
    if button_top < visible_top {
        scroll.0.y = (scroll.0.y - (visible_top - button_top)).max(0.0);
    } else if button_bottom > visible_bottom {
        scroll.0.y += button_bottom - visible_bottom;
    }
}

#[derive(bevy::ecs::system::SystemParam)]
struct BrawlerScreenUi<'w, 's> {
    windows: Query<'w, 's, &'static Window, With<PrimaryWindow>>,
    scale: Option<Res<'w, UiScale>>,
    navigation: ResMut<'w, FlowNavigation>,
}

fn brawler_screen_layout(screen: &BrawlerScreenUi<'_, '_>) -> BrawlerDetailsLayout {
    screen
        .windows
        .iter()
        .next()
        .map_or(BrawlerDetailsLayout::Wide, |window| {
            if dashboard_layout_class(
                window.resolution.width(),
                window.resolution.height(),
                screen.scale.as_deref().map_or(1.0, |scale| scale.0),
            ) == DashboardLayoutClass::Compact
            {
                BrawlerDetailsLayout::Compact
            } else {
                BrawlerDetailsLayout::Wide
            }
        })
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "one cohesive bounded detail builder keeps resolved stats, touch actions, and navigation indices adjacent"
)]
fn present_brawler_details(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    profile: Res<super::ClientProfileModel>,
    mut screen: BrawlerScreenUi,
    roots: Query<(Entity, &BrawlerDetailsRoot)>,
) {
    let (brawler_id, contextual_confirmation) = match overlay.as_ref() {
        ClientOverlay::BrawlerDetails(brawler_id) => (brawler_id, false),
        ClientOverlay::DeleteBrawlerConfirmation(brawler_id) => (brawler_id, true),
        _ => {
            for (entity, _) in &roots {
                commands.entity(entity).despawn();
            }
            return;
        }
    };
    if *flow.get() != ClientFlow::Dashboard {
        return;
    }
    let Some(snapshot) = profile.snapshot() else {
        return;
    };
    let Some(catalog) = profile.catalog() else {
        return;
    };
    let Some(brawler) = snapshot
        .brawlers
        .iter()
        .find(|brawler| brawler.id == *brawler_id)
    else {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    };
    let layout = brawler_screen_layout(&screen);
    let render_key = BrawlerDetailsRoot {
        brawler_id: *brawler_id,
        profile_revision: snapshot.revision,
        pending: profile.pending(),
        contextual_confirmation,
        layout,
    };
    if roots.iter().any(|(_, root)| *root == render_key) {
        return;
    }
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    screen.navigation.selected = 0;
    let compact = layout == BrawlerDetailsLayout::Compact;
    let selected = snapshot.selected_brawler_id == Some(brawler.id);
    let stats = fighter_profile_stats(catalog, brawler.fighter_profile_id);
    let fighter_copy = stats.map_or_else(
        || "Fighter statistics unavailable".to_string(),
        |stats| {
            format!(
                "Health {} · Speed {:.0} · Reveal distance {:.0}",
                stats.maximum_health, stats.movement_speed, stats.reveal_proximity_radius
            )
        },
    );
    let resolved_weapon = snapshot
        .weapon_modifiers(brawler)
        .ok()
        .and_then(|modifiers| {
            let fighters = crate::combat::FighterDefinitions::default();
            let weapon = catalog.weapon(brawler.weapon_base_id)?;
            crate::weapon_parts::resolve_advertised_weapon_parts(
                &weapon.configuration,
                &catalog.weapon_policy,
                &fighters.entries[0],
                crate::combat::WeaponPresetId(brawler.weapon_base_id.0),
                modifiers,
            )
            .ok()
        });
    let weapon_copy = resolved_weapon.as_ref().map_or_else(
        || "Weapon statistics unavailable".to_string(),
        weapon_preview_text,
    );
    let equipped = brawler.equipped_part_ids.iter().flatten().count();
    let pending = profile.pending();
    let controls_disabled = pending || contextual_confirmation;
    commands
        .spawn((
            render_key,
            DespawnOnExit(ClientFlow::Dashboard),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                ..default()
            },
            BackgroundColor(Color::srgb(0.018, 0.13, 0.29)),
            GlobalZIndex(510),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(100),
                    min_height: px(72),
                    align_items: AlignItems::Center,
                    column_gap: px(16),
                    padding: UiRect::axes(px(18), px(10)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.04, 0.085)),
            ))
            .with_children(|header| {
                spawn_brawler_screen_button(
                    header,
                    4,
                    FlowUiAction::BackToBrawlerList,
                    "‹ BRAWLERS",
                    px(160),
                    contextual_confirmation,
                    Color::srgb(0.06, 0.22, 0.4),
                );
                header.spawn((
                    Text::new(brawler.name.to_uppercase()),
                    TextFont::from_font_size(if compact { 27.0 } else { 34.0 }),
                    TextColor(Color::WHITE),
                ));
                header.spawn((
                    Node {
                        flex_grow: 1.0,
                        ..default()
                    },
                ));
                if selected {
                    header.spawn((
                        Text::new("✓ SELECTED FOR PLAY"),
                        TextFont::from_font_size(16.0),
                        TextColor(Color::srgb(0.2, 0.95, 0.75)),
                    ));
                }
            });
            root.spawn((
                BrawlerDetailsScrollArea,
                Node {
                    width: percent(100),
                    min_height: px(0),
                    flex_grow: 1.0,
                    flex_direction: if compact {
                        FlexDirection::Column
                    } else {
                        FlexDirection::Row
                    },
                    align_items: AlignItems::Stretch,
                    justify_content: JustifyContent::Center,
                    row_gap: px(16),
                    column_gap: px(20),
                    padding: UiRect::all(if compact { px(14) } else { px(24) }),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollPosition::default(),
            ))
            .with_children(|body| {
                body.spawn((
                    Node {
                        width: if compact { percent(100) } else { percent(27) },
                        max_width: if compact { Val::Auto } else { px(360) },
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Stretch,
                        row_gap: px(12),
                        padding: UiRect::all(px(18)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.03, 0.22, 0.46)),
                ))
                .with_children(|identity| {
                    identity.spawn((
                        Text::new("FIGHTER"),
                        TextFont::from_font_size(15.0),
                        TextColor(Color::srgb(0.38, 0.88, 1.0)),
                    ));
                    identity.spawn((
                        Text::new(advertised_fighter_name(catalog, brawler.fighter_profile_id)),
                        TextFont::from_font_size(28.0),
                        TextColor(Color::WHITE),
                    ));
                    identity.spawn((
                        Text::new("PERMANENT BASE"),
                        TextFont::from_font_size(13.0),
                        TextColor(Color::srgb(1.0, 0.78, 0.25)),
                    ));
                    identity.spawn((
                        Text::new(fighter_copy),
                        TextFont::from_font_size(17.0),
                        TextColor(Color::srgb(0.82, 0.91, 0.98)),
                    ));
                });
                body.spawn((
                    BrawlerDetailsPreviewHost,
                    AccessibleLabel::new(format!("3D preview of {}", brawler.name)),
                    Node {
                        width: if compact { percent(100) } else { percent(42) },
                        max_width: if compact { Val::Auto } else { px(680) },
                        height: if compact { px(360) } else { percent(100) },
                        min_height: px(340),
                        flex_grow: if compact { 0.0 } else { 1.0 },
                        flex_shrink: 0.0,
                        justify_content: JustifyContent::Center,
                        align_items: AlignItems::End,
                        overflow: Overflow::clip(),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.015, 0.3, 0.58)),
                ));
                body.spawn((
                    Node {
                        width: if compact { percent(100) } else { percent(31) },
                        max_width: if compact { Val::Auto } else { px(420) },
                        flex_shrink: 0.0,
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Stretch,
                        row_gap: px(10),
                        padding: UiRect::all(px(18)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.025, 0.075, 0.15)),
                ))
                .with_children(|loadout| {
                    loadout.spawn((
                        Text::new("LOADOUT"),
                        TextFont::from_font_size(24.0),
                        TextColor(Color::WHITE),
                    ));
                    loadout.spawn((
                        Text::new(format!(
                            "WEAPON\n{} · PERMANENT\n{}\n\nULTIMATE\n{}\n\nPASSIVES\n{}\n{}\n\nWEAPON PARTS\n{equipped}/{} EQUIPPED",
                            advertised_weapon_name(catalog, brawler.weapon_base_id),
                            weapon_copy,
                            ultimate_name(catalog, brawler.ultimate_id),
                            passive_name(catalog, brawler.passive_ids[0]),
                            passive_name(catalog, brawler.passive_ids[1]),
                            crate::weapon_parts::WEAPON_PART_SLOT_COUNT,
                        )),
                        TextFont::from_font_size(15.0),
                        TextColor(Color::srgb(0.78, 0.88, 0.96)),
                    ));
                    spawn_brawler_screen_button(
                        loadout,
                        0,
                        FlowUiAction::SelectBrawler(brawler.id),
                        if selected {
                            "SELECTED FOR PLAY"
                        } else if profile.selection_pending(brawler.id) {
                            "SELECTING..."
                        } else {
                            "SELECT FOR PLAY"
                        },
                        percent(100),
                        controls_disabled,
                        Color::srgb(0.04, 0.62, 0.38),
                    );
                    spawn_brawler_screen_button(
                        loadout,
                        1,
                        FlowUiAction::OpenBrawlerEditor(brawler.id),
                        "CUSTOMIZE ABILITIES",
                        percent(100),
                        controls_disabled,
                        Color::srgb(0.04, 0.42, 0.72),
                    );
                    spawn_brawler_screen_button(
                        loadout,
                        2,
                        FlowUiAction::OpenWeaponEquipment(brawler.id),
                        "CUSTOMIZE WEAPON",
                        percent(100),
                        controls_disabled,
                        Color::srgb(0.04, 0.42, 0.72),
                    );
                    spawn_brawler_screen_button(
                        loadout,
                        3,
                        FlowUiAction::DeleteBrawler(brawler.id),
                        "DELETE BRAWLER",
                        percent(100),
                        controls_disabled,
                        Color::srgb(0.48, 0.08, 0.12),
                    );
                });
            });
        });
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    reason = "one bounded creation-screen builder owns its touch controls and inline mutation state"
)]
fn present_brawler_creation(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    draft: Res<BrawlerCreationDraft>,
    profile: Res<super::ClientProfileModel>,
    mut screen: BrawlerScreenUi,
    roots: Query<(Entity, &BrawlerCreationRoot)>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::BrawlerCreation)
        || *flow.get() != ClientFlow::Dashboard
    {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    let Some(catalog) = profile.catalog() else {
        return;
    };
    let layout = brawler_screen_layout(&screen);
    let render_key = BrawlerCreationRoot {
        draft: draft.clone(),
        layout,
    };
    if roots.iter().any(|(_, root)| *root == render_key) {
        return;
    }
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    screen.navigation.selected = 0;
    let compact = layout == BrawlerDetailsLayout::Compact;
    commands
        .spawn((
            render_key,
            DespawnOnExit(ClientFlow::Dashboard),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                ..default()
            },
            BackgroundColor(Color::srgb(0.018, 0.11, 0.24)),
            GlobalZIndex(510),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(100),
                    min_height: px(72),
                    align_items: AlignItems::Center,
                    column_gap: px(16),
                    padding: UiRect::axes(px(18), px(10)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.04, 0.085)),
            ))
            .with_children(|header| {
                spawn_brawler_screen_button(
                    header,
                    4,
                    FlowUiAction::CancelCreateBrawler,
                    "‹ BRAWLERS",
                    px(170),
                    false,
                    Color::srgb(0.06, 0.22, 0.4),
                );
                header.spawn((
                    Text::new("CREATE BRAWLER"),
                    TextFont::from_font_size(if compact { 27.0 } else { 34.0 }),
                    TextColor(Color::WHITE),
                ));
            });
            root.spawn((Node {
                width: percent(100),
                min_height: px(0),
                flex_grow: 1.0,
                flex_direction: if compact {
                    FlexDirection::Column
                } else {
                    FlexDirection::Row
                },
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::Center,
                row_gap: px(16),
                column_gap: px(18),
                padding: UiRect::all(if compact { px(14) } else { px(28) }),
                overflow: Overflow::scroll_y(),
                ..default()
            },))
            .with_children(|body| {
                body.spawn((
                    Node {
                        width: if compact { percent(100) } else { percent(31) },
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Stretch,
                        row_gap: px(12),
                        padding: UiRect::all(px(20)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.03, 0.22, 0.46)),
                ))
                .with_children(|card| {
                    card.spawn((
                        Text::new("FIGHTER PROFILE"),
                        TextFont::from_font_size(18.0),
                        TextColor(Color::srgb(0.38, 0.88, 1.0)),
                    ));
                    card.spawn((
                        Text::new("Defines health, speed, and reveal distance. Permanent after creation."),
                        TextFont::from_font_size(14.0),
                        TextColor(Color::srgb(0.76, 0.86, 0.95)),
                    ));
                    spawn_brawler_screen_button(
                        card,
                        0,
                        FlowUiAction::CycleCreationProfile,
                        advertised_fighter_name(catalog, draft.fighter_profile_id),
                        percent(100),
                        false,
                        Color::srgb(0.04, 0.42, 0.72),
                    );
                });
                body.spawn((
                    Node {
                        width: if compact { percent(100) } else { percent(31) },
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Stretch,
                        row_gap: px(12),
                        padding: UiRect::all(px(20)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.025, 0.16, 0.34)),
                ))
                .with_children(|card| {
                    card.spawn((
                        Text::new("WEAPON BASE"),
                        TextFont::from_font_size(18.0),
                        TextColor(Color::srgb(1.0, 0.78, 0.25)),
                    ));
                    card.spawn((
                        Text::new("Defines the permanent weapon family. Parts remain customizable."),
                        TextFont::from_font_size(14.0),
                        TextColor(Color::srgb(0.76, 0.86, 0.95)),
                    ));
                    spawn_brawler_screen_button(
                        card,
                        1,
                        FlowUiAction::CycleCreationWeapon,
                        advertised_weapon_name(catalog, draft.weapon_base_id),
                        percent(100),
                        false,
                        Color::srgb(0.12, 0.35, 0.62),
                    );
                });
                body.spawn((
                    Node {
                        width: if compact { percent(100) } else { percent(31) },
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Stretch,
                        row_gap: px(12),
                        padding: UiRect::all(px(20)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.08, 0.12, 0.28)),
                ))
                .with_children(|card| {
                    card.spawn((
                        Text::new("STARTING ULTIMATE"),
                        TextFont::from_font_size(18.0),
                        TextColor(Color::srgb(0.82, 0.62, 1.0)),
                    ));
                    card.spawn((
                        Text::new("The ultimate and both passives can be changed later."),
                        TextFont::from_font_size(14.0),
                        TextColor(Color::srgb(0.76, 0.86, 0.95)),
                    ));
                    spawn_brawler_screen_button(
                        card,
                        2,
                        FlowUiAction::CycleCreationUltimate,
                        ultimate_name(catalog, draft.ultimate),
                        percent(100),
                        false,
                        Color::srgb(0.34, 0.18, 0.62),
                    );
                });
            });
            root.spawn((
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: px(16),
                    padding: UiRect::axes(px(22), px(12)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.055, 0.11)),
            ))
            .with_children(|footer| {
                if let Some(error) = &draft.inline_error {
                    footer.spawn((
                        Text::new(error),
                        TextFont::from_font_size(14.0),
                        TextColor(Color::srgb(1.0, 0.5, 0.45)),
                    ));
                }
                spawn_brawler_screen_button(
                    footer,
                    3,
                    FlowUiAction::ConfirmCreateBrawler,
                    "CREATE BRAWLER",
                    if compact { percent(100) } else { px(420) },
                    profile.pending(),
                    Color::srgb(0.03, 0.58, 0.4),
                );
            });
        });
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    reason = "one cohesive Bevy overlay builder keeps editor layout and navigation indices adjacent"
)]
fn present_brawler_editor(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    draft: Res<BrawlerEditDraft>,
    profile: Res<super::ClientProfileModel>,
    mut screen: BrawlerScreenUi,
    roots: Query<(Entity, &BrawlerEditorRoot)>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::BrawlerEditor)
        || *flow.get() != ClientFlow::Dashboard
    {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    let Some(catalog) = profile.catalog() else {
        return;
    };
    let layout = brawler_screen_layout(&screen);
    let render_key = BrawlerEditorRoot {
        draft: draft.clone(),
        layout,
    };
    if roots.iter().any(|(_, root)| *root == render_key) {
        return;
    }
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    screen.navigation.selected = 0;
    let compact = layout == BrawlerDetailsLayout::Compact;
    let name = if draft.editing_name {
        let caret = draft.name_caret.min(draft.name.len());
        format!("{}|{}", &draft.name[..caret], &draft.name[caret..])
    } else {
        draft.name.clone()
    };
    commands
        .spawn((
            render_key,
            DespawnOnExit(ClientFlow::Dashboard),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                ..default()
            },
            BackgroundColor(Color::srgb(0.018, 0.11, 0.24)),
            GlobalZIndex(510),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(100),
                    min_height: px(72),
                    align_items: AlignItems::Center,
                    column_gap: px(16),
                    padding: UiRect::axes(px(18), px(10)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.04, 0.085)),
            ))
            .with_children(|header| {
                spawn_brawler_screen_button(
                    header,
                    5,
                    FlowUiAction::CancelBrawlerEdit,
                    "‹ BRAWLER",
                    px(170),
                    false,
                    Color::srgb(0.06, 0.22, 0.4),
                );
                header.spawn((
                    Text::new("CUSTOMIZE ABILITIES"),
                    TextFont::from_font_size(if compact { 27.0 } else { 34.0 }),
                    TextColor(Color::WHITE),
                ));
            });
            root.spawn((Node {
                width: percent(100),
                min_height: px(0),
                flex_grow: 1.0,
                flex_direction: if compact {
                    FlexDirection::Column
                } else {
                    FlexDirection::Row
                },
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::Center,
                row_gap: px(16),
                column_gap: px(20),
                padding: UiRect::all(if compact { px(14) } else { px(28) }),
                overflow: Overflow::scroll_y(),
                ..default()
            },))
            .with_children(|body| {
                body.spawn((
                    Node {
                        width: if compact { percent(100) } else { percent(32) },
                        max_width: if compact { Val::Auto } else { px(420) },
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Stretch,
                        row_gap: px(12),
                        padding: UiRect::all(px(22)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.03, 0.22, 0.46)),
                ))
                .with_children(|identity| {
                    identity.spawn((
                        Text::new("IDENTITY"),
                        TextFont::from_font_size(18.0),
                        TextColor(Color::srgb(0.38, 0.88, 1.0)),
                    ));
                    identity.spawn((
                        Text::new(format!(
                            "{}\n{}\n\nFighter profile and weapon base are permanent.",
                            advertised_fighter_name(catalog, draft.fighter_profile_id),
                            advertised_weapon_name(catalog, draft.weapon_base_id)
                        )),
                        TextFont::from_font_size(16.0),
                        TextColor(Color::srgb(0.8, 0.9, 0.98)),
                    ));
                    spawn_brawler_screen_button(
                        identity,
                        0,
                        FlowUiAction::BeginBrawlerNameEdit,
                        &format!("NAME · {name}"),
                        percent(100),
                        false,
                        Color::srgb(0.06, 0.35, 0.62),
                    );
                });
                body.spawn((
                    Node {
                        width: if compact { percent(100) } else { percent(68) },
                        max_width: if compact { Val::Auto } else { px(760) },
                        flex_direction: FlexDirection::Column,
                        align_items: AlignItems::Stretch,
                        row_gap: px(14),
                        padding: UiRect::all(px(22)),
                        ..default()
                    },
                    BackgroundColor(Color::srgb(0.025, 0.075, 0.15)),
                ))
                .with_children(|abilities| {
                    abilities.spawn((
                        Text::new("ACTIVE LOADOUT"),
                        TextFont::from_font_size(18.0),
                        TextColor(Color::srgb(0.82, 0.62, 1.0)),
                    ));
                    abilities.spawn((
                        Text::new("Tap a slot to cycle through the choices advertised by this server."),
                        TextFont::from_font_size(14.0),
                        TextColor(Color::srgb(0.72, 0.84, 0.94)),
                    ));
                    spawn_brawler_screen_button(
                        abilities,
                        1,
                        FlowUiAction::CycleBrawlerUltimate,
                        &format!("ULTIMATE\n{}", ultimate_name(catalog, draft.ultimate_id)),
                        percent(100),
                        false,
                        Color::srgb(0.34, 0.18, 0.62),
                    );
                    spawn_brawler_screen_button(
                        abilities,
                        2,
                        FlowUiAction::CycleBrawlerPassiveOne,
                        &format!("PASSIVE 1\n{}", passive_name(catalog, draft.passive_ids[0])),
                        percent(100),
                        false,
                        Color::srgb(0.08, 0.34, 0.5),
                    );
                    spawn_brawler_screen_button(
                        abilities,
                        3,
                        FlowUiAction::CycleBrawlerPassiveTwo,
                        &format!("PASSIVE 2\n{}", passive_name(catalog, draft.passive_ids[1])),
                        percent(100),
                        false,
                        Color::srgb(0.08, 0.34, 0.5),
                    );
                });
            });
            root.spawn((
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: px(16),
                    padding: UiRect::axes(px(22), px(12)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.055, 0.11)),
            ))
            .with_children(|footer| {
                if let Some(error) = &draft.inline_error {
                    footer.spawn((
                        Text::new(error),
                        TextFont::from_font_size(14.0),
                        TextColor(Color::srgb(1.0, 0.5, 0.45)),
                    ));
                }
                spawn_brawler_screen_button(
                    footer,
                    4,
                    FlowUiAction::ConfirmBrawlerEdit,
                    "SAVE CHANGES",
                    if compact { percent(100) } else { px(420) },
                    profile.pending(),
                    Color::srgb(0.03, 0.58, 0.4),
                );
            });
        });
}

fn scroll_weapon_equipment(
    mut wheel: MessageReader<MouseWheel>,
    mut areas: Query<&mut ScrollPosition, With<WeaponEquipmentScrollArea>>,
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
    for mut position in &mut areas {
        position.0.y = (position.0.y - delta).max(0.0);
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "one cohesive Bevy overlay builder renders the four slots, bounded inventory, and live preview"
)]
fn present_weapon_equipment(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    draft: Res<WeaponEquipmentDraft>,
    roots: Query<(Entity, &WeaponEquipmentRoot)>,
    scroll_areas: Query<&ScrollPosition, With<WeaponEquipmentScrollArea>>,
    profile: Res<super::ClientProfileModel>,
    parts: Res<crate::weapon_parts::WeaponPartCatalogResource>,
    mut screen: BrawlerScreenUi,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::WeaponEquipment)
        || *flow.get() != ClientFlow::Dashboard
    {
        for (entity, _) in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    let layout = brawler_screen_layout(&screen);
    let render_key = WeaponEquipmentRoot {
        draft: draft.clone(),
        layout,
    };
    let existing = roots.iter().next();
    if existing.is_some_and(|(_, root)| *root == render_key) {
        return;
    }
    let retained_scroll = scroll_areas.iter().next().cloned().unwrap_or_default();
    let first_render = existing.is_none();
    for (entity, _) in &roots {
        commands.entity(entity).despawn();
    }
    if first_render {
        screen.navigation.selected = draft.selected_slot;
    }
    let Some(snapshot) = profile.snapshot() else {
        return;
    };
    let Some(catalog) = profile.catalog() else {
        return;
    };
    let Some(brawler_id) = draft.brawler_id else {
        return;
    };
    let Some(saved) = snapshot.brawlers.iter().find(|item| item.id == brawler_id) else {
        return;
    };
    let mut candidate = snapshot.clone();
    if let Some(candidate_brawler) = candidate
        .brawlers
        .iter_mut()
        .find(|item| item.id == brawler_id)
    {
        candidate_brawler.equipped_part_ids = draft.equipped_part_ids;
    } else {
        return;
    }
    let Some(candidate_brawler) = candidate.brawlers.iter().find(|item| item.id == brawler_id)
    else {
        return;
    };
    let resolved_preview =
        candidate
            .weapon_modifiers(candidate_brawler)
            .ok()
            .and_then(|modifiers| {
                let fighters = crate::combat::FighterDefinitions::default();
                let weapon = catalog.weapon(saved.weapon_base_id)?;
                crate::weapon_parts::resolve_advertised_weapon_parts(
                    &weapon.configuration,
                    &catalog.weapon_policy,
                    &fighters.entries[0],
                    crate::combat::WeaponPresetId(saved.weapon_base_id.0),
                    modifiers,
                )
                .ok()
            });
    let preview_valid = resolved_preview.is_some();
    let preview = resolved_preview.map_or_else(
        || "INVALID PART COMBINATION".into(),
        |weapon| weapon_preview_text(&weapon),
    );
    let compact = layout == BrawlerDetailsLayout::Compact;
    let save_index = 5 + snapshot.inventory.len();

    commands
        .spawn((
            render_key,
            DespawnOnExit(ClientFlow::Dashboard),
            Node {
                position_type: PositionType::Absolute,
                left: px(0),
                right: px(0),
                top: px(0),
                bottom: px(0),
                flex_direction: FlexDirection::Column,
                align_items: AlignItems::Stretch,
                ..default()
            },
            BackgroundColor(Color::srgb(0.018, 0.11, 0.24)),
            GlobalZIndex(520),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(100),
                    min_height: px(72),
                    align_items: AlignItems::Center,
                    column_gap: px(16),
                    padding: UiRect::axes(px(18), px(10)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.04, 0.085)),
            ))
            .with_children(|header| {
                spawn_brawler_screen_button(
                    header,
                    save_index + 1,
                    FlowUiAction::CancelWeaponEquipment,
                    "‹ BRAWLER",
                    px(170),
                    false,
                    Color::srgb(0.06, 0.22, 0.4),
                );
                header.spawn((
                    Text::new("CUSTOMIZE WEAPON"),
                    TextFont::from_font_size(if compact { 27.0 } else { 34.0 }),
                    TextColor(Color::WHITE),
                ));
            });
            root.spawn((
                Node {
                    width: percent(100),
                    padding: UiRect::axes(px(24), px(12)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.025, 0.2, 0.42)),
            ))
            .with_children(|summary| {
                summary.spawn((
                    Text::new(format!(
                        "{}  ·  {}",
                        advertised_weapon_name(catalog, saved.weapon_base_id),
                        preview
                    )),
                    TextFont::from_font_size(16.0),
                    TextColor(Color::srgb(0.72, 0.9, 1.0)),
                ));
            });
            root.spawn((Node {
                width: percent(100),
                min_height: px(0),
                flex_grow: 1.0,
                flex_direction: if compact {
                    FlexDirection::Column
                } else {
                    FlexDirection::Row
                },
                align_items: AlignItems::Stretch,
                justify_content: JustifyContent::Center,
                row_gap: px(14),
                column_gap: px(18),
                padding: UiRect::all(if compact { px(12) } else { px(22) }),
                ..default()
            },))
                .with_children(|body| {
                    body.spawn((
                        Node {
                            width: if compact { percent(100) } else { percent(36) },
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Stretch,
                            row_gap: px(9),
                            padding: UiRect::all(px(18)),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.03, 0.22, 0.46)),
                    ))
                    .with_children(|slots| {
                        slots.spawn((
                            Text::new("EQUIPPED SLOTS"),
                            TextFont::from_font_size(18.0),
                            TextColor(Color::srgb(0.38, 0.88, 1.0)),
                        ));
                        for slot in 0..crate::weapon_parts::WEAPON_PART_SLOT_COUNT {
                            let label = draft.equipped_part_ids[slot]
                                .and_then(|id| snapshot.inventory.iter().find(|part| part.id == id))
                                .map_or_else(
                                    || format!("SLOT {} · EMPTY", slot + 1),
                                    |part| format!("SLOT {} · {}", slot + 1, part.display_name),
                                );
                            spawn_brawler_screen_button(
                                slots,
                                slot,
                                FlowUiAction::SelectEquipmentSlot(slot),
                                &if slot == draft.selected_slot {
                                    format!("> {label}")
                                } else {
                                    label
                                },
                                percent(100),
                                false,
                                if slot == draft.selected_slot {
                                    Color::srgb(0.04, 0.5, 0.66)
                                } else {
                                    Color::srgb(0.06, 0.27, 0.48)
                                },
                            );
                        }
                        spawn_brawler_screen_button(
                            slots,
                            4,
                            FlowUiAction::UnequipWeaponPart,
                            "UNEQUIP SELECTED SLOT",
                            percent(100),
                            false,
                            Color::srgb(0.35, 0.18, 0.24),
                        );
                    });
                    body.spawn((
                        WeaponEquipmentScrollArea,
                        retained_scroll,
                        Node {
                            width: if compact { percent(100) } else { percent(64) },
                            min_height: px(0),
                            flex_grow: 1.0,
                            flex_shrink: 1.0,
                            flex_direction: FlexDirection::Column,
                            align_items: AlignItems::Stretch,
                            row_gap: px(9),
                            padding: UiRect::all(px(18)),
                            overflow: Overflow::scroll_y(),
                            ..default()
                        },
                        BackgroundColor(Color::srgb(0.025, 0.075, 0.15)),
                    ))
                    .with_children(|scroll| {
                        scroll.spawn((
                            Text::new("OWNED PARTS"),
                            TextFont::from_font_size(18.0),
                            TextColor(Color::srgb(1.0, 0.82, 0.38)),
                        ));
                        for (index, part) in snapshot.inventory.iter().enumerate() {
                            let presentation = parts
                                .0
                                .definition(part.definition_id)
                                .map_or("Part", |definition| definition.presentation_type.as_str());
                            let equipped_elsewhere = snapshot.brawlers.iter().find(|brawler| {
                                brawler.id != brawler_id
                                    && brawler.equipped_part_ids.contains(&Some(part.id))
                            });
                            let availability = equipped_elsewhere
                                .map(|brawler| format!(" · EQUIPPED BY {}", brawler.name))
                                .unwrap_or_default();
                            let effects = part
                                .effects
                                .iter()
                                .map(|effect| weapon_part_effect_text(*effect))
                                .collect::<Vec<_>>()
                                .join(" · ");
                            spawn_brawler_screen_button(
                                scroll,
                                5 + index,
                                FlowUiAction::EquipWeaponPart(part.id),
                                &format!(
                                    "{} [{}] — {}{}",
                                    part.display_name, presentation, effects, availability
                                ),
                                percent(100),
                                false,
                                Color::srgb(0.09, 0.18, 0.28),
                            );
                        }
                        if let Some(error) = &draft.inline_error {
                            scroll.spawn((
                                Text::new(error),
                                TextFont::from_font_size(14.0),
                                TextColor(Color::srgb(1.0, 0.5, 0.45)),
                            ));
                        }
                    });
                });
            root.spawn((
                Node {
                    width: percent(100),
                    align_items: AlignItems::Center,
                    justify_content: JustifyContent::Center,
                    column_gap: px(16),
                    padding: UiRect::axes(px(22), px(12)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.015, 0.055, 0.11)),
            ))
            .with_children(|footer| {
                spawn_brawler_screen_button(
                    footer,
                    save_index,
                    FlowUiAction::ConfirmWeaponEquipment,
                    "SAVE EQUIPMENT",
                    if compact { percent(100) } else { px(420) },
                    !preview_valid || profile.pending(),
                    Color::srgb(0.03, 0.58, 0.4),
                );
            });
        });
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "computed UI bounds are available only after Bevy's layout pass"
)]
fn keep_weapon_equipment_focus_visible(
    overlay: Res<ClientOverlay>,
    navigation: Res<FlowNavigation>,
    buttons: Query<(&FlowButton, &ChildOf, &ComputedNode, &UiGlobalTransform)>,
    mut areas: Query<
        (
            Entity,
            &ComputedNode,
            &UiGlobalTransform,
            &mut ScrollPosition,
        ),
        With<WeaponEquipmentScrollArea>,
    >,
    mut prior: Local<Option<(Entity, usize)>>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::WeaponEquipment) {
        *prior = None;
        return;
    }
    let Some((area_entity, area_node, area_transform, mut scroll)) = areas.iter_mut().next() else {
        *prior = None;
        return;
    };
    let focus_key = (area_entity, navigation.selected);
    if prior.as_ref() == Some(&focus_key) {
        return;
    }
    *prior = Some(focus_key);
    let Some((_, _, button_node, button_transform)) =
        buttons.iter().find(|(button, child_of, _, _)| {
            child_of.parent() == area_entity && button.index == navigation.selected
        })
    else {
        return;
    };
    if area_node.is_empty() || button_node.is_empty() {
        return;
    }
    let (_, _, area_center) = area_transform.to_scale_angle_translation();
    let (_, _, button_center) = button_transform.to_scale_angle_translation();
    let visible_top = area_center.y - area_node.size().y * 0.5 + 8.0;
    let visible_bottom = area_center.y + area_node.size().y * 0.5 - 8.0;
    let button_top = button_center.y - button_node.size().y * 0.5;
    let button_bottom = button_center.y + button_node.size().y * 0.5;
    if button_top < visible_top {
        scroll.0.y = (scroll.0.y - (visible_top - button_top)).max(0.0);
    } else if button_bottom > visible_bottom {
        scroll.0.y += button_bottom - visible_bottom;
    }
}

fn weapon_part_effect_text(effect: crate::weapon_parts::WeaponPartEffect) -> String {
    let percent = |value: i16| format!("{:+}%", f32::from(value) / 100.0);
    match effect {
        crate::weapon_parts::WeaponPartEffect::Capacity {
            flat,
            percent_basis_points,
        } => format!("capacity {flat:+} {}", percent(percent_basis_points)),
        crate::weapon_parts::WeaponPartEffect::Damage {
            flat,
            percent_basis_points,
        } => format!("damage {flat:+} {}", percent(percent_basis_points)),
        crate::weapon_parts::WeaponPartEffect::FireInterval {
            flat_ticks,
            percent_basis_points,
        } => format!(
            "fire interval {flat_ticks:+}t {}",
            percent(percent_basis_points)
        ),
        crate::weapon_parts::WeaponPartEffect::RefillInterval {
            flat_ticks,
            percent_basis_points,
        } => format!("refill {flat_ticks:+}t {}", percent(percent_basis_points)),
        crate::weapon_parts::WeaponPartEffect::Reach {
            flat_milliunits,
            percent_basis_points,
        } => format!(
            "reach {:+.1} {}",
            f64::from(flat_milliunits) / 1_000.0,
            percent(percent_basis_points)
        ),
        crate::weapon_parts::WeaponPartEffect::Slow {
            penalty_basis_points,
            duration_ticks,
        } => format!(
            "Slow {:.0}%/{duration_ticks}t",
            f32::from(penalty_basis_points) / 100.0
        ),
    }
}

fn weapon_preview_text(weapon: &crate::combat::ResolvedWeapon) -> String {
    let damage = weapon
        .recipe
        .payload_bundles
        .iter()
        .flat_map(|bundle| &bundle.effects)
        .find_map(|effect| match effect {
            crate::combat::PayloadEffectDefinition::Damage { amount, .. } => Some(*amount),
            _ => None,
        })
        .unwrap_or_default();
    let slow = weapon.recipe.payload_bundles.iter().any(|bundle| {
        bundle
            .effects
            .iter()
            .any(|effect| matches!(effect, crate::combat::PayloadEffectDefinition::Slow { .. }))
    });
    let reach = match weapon.recipe.delivery {
        crate::combat::DeliveryMethod::Straight { range, .. } => range,
        crate::combat::DeliveryMethod::Lobbed { distance, .. } => distance,
        crate::combat::DeliveryMethod::MeleeArc { reach, .. } => reach,
    };
    format!(
        "Capacity {} · Damage {} · Fire {}t · Refill {}t · Reach {:.0}{}",
        weapon.recipe.economy.capacity(),
        damage,
        weapon.recipe.fire_cooldown_ticks,
        weapon.recipe.economy.refill_ticks(),
        reach,
        if slow { " · Slow" } else { "" }
    )
}

#[allow(clippy::needless_pass_by_value)]
fn present_delete_brawler_confirmation(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    roots: Query<Entity, With<DeleteBrawlerConfirmationRoot>>,
    profile: Res<super::ClientProfileModel>,
    mut navigation: ResMut<FlowNavigation>,
) {
    let ClientOverlay::DeleteBrawlerConfirmation(brawler_id) = overlay.as_ref() else {
        for entity in &roots {
            commands.entity(entity).despawn();
        }
        return;
    };
    if !roots.is_empty() || *flow.get() != ClientFlow::Dashboard {
        return;
    }
    let name = profile
        .snapshot()
        .and_then(|snapshot| {
            snapshot
                .brawlers
                .iter()
                .find(|brawler| brawler.id == *brawler_id)
        })
        .map_or("this brawler", |brawler| brawler.name.as_str());
    navigation.selected = 0;
    commands
        .spawn((
            DeleteBrawlerConfirmationRoot,
            DespawnOnExit(ClientFlow::Dashboard),
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
            BackgroundColor(Color::srgba(0.0, 0.0, 0.0, 0.78)),
            GlobalZIndex(610),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(84),
                    max_width: px(520),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(12),
                    padding: UiRect::all(px(24)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.055, 0.08, 0.12)),
            ))
            .with_children(|panel| {
                spawn_heading(panel, "DELETE BRAWLER?");
                panel.spawn((
                    Text::new(format!("Delete {name}? This cannot be undone.")),
                    TextColor(Color::srgb(0.82, 0.88, 0.94)),
                ));
                spawn_flow_error_button(
                    panel,
                    0,
                    FlowUiAction::CancelDeleteBrawler,
                    "KEEP BRAWLER",
                );
                spawn_flow_error_button(panel, 1, FlowUiAction::ConfirmDeleteBrawler, "DELETE");
            });
        });
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the bounded menu presenter declares its complete connected Dashboard view"
)]
fn present_dashboard_menu(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    roots: Query<Entity, With<DashboardMenuRoot>>,
    mut navigation: ResMut<FlowNavigation>,
    memberships: Query<Option<&RuntimeLobbyTarget>, With<Client>>,
    persistence: Res<ConnectionPersistence>,
) {
    if !matches!(overlay.as_ref(), ClientOverlay::DashboardMenu) {
        for entity in &roots {
            commands.entity(entity).despawn();
        }
        return;
    }
    if roots.iter().next().is_some() || *flow.get() != ClientFlow::Dashboard {
        return;
    }
    navigation.selected = 0;
    let favorite_label = memberships.iter().flatten().next().map(|target| {
        if persistence
            .state
            .favorites
            .iter()
            .any(|favorite| favorite.address == target.logical_address)
        {
            "REMOVE FAVORITE"
        } else {
            "FAVORITE SERVER"
        }
    });
    commands
        .spawn((
            DashboardMenuRoot,
            DespawnOnExit(ClientFlow::Dashboard),
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
            BackgroundColor(Color::srgba(0.0, 0.015, 0.04, 0.78)),
            GlobalZIndex(500),
        ))
        .with_children(|root| {
            root.spawn((
                Node {
                    width: percent(82),
                    max_width: px(430),
                    flex_direction: FlexDirection::Column,
                    align_items: AlignItems::Center,
                    row_gap: px(10),
                    padding: UiRect::all(px(22)),
                    border: UiRect::all(px(2)),
                    border_radius: BorderRadius::all(px(14)),
                    ..default()
                },
                BackgroundColor(Color::srgb(0.035, 0.075, 0.14)),
                BorderColor::all(Color::srgb(0.12, 0.42, 0.7)),
            ))
            .with_children(|panel| {
                spawn_heading(panel, "MENU");
                let mut index = 0;
                spawn_flow_error_button(panel, index, FlowUiAction::OpenCredits, "CREDITS");
                index += 1;
                if let Some(favorite_label) = favorite_label {
                    spawn_flow_error_button(
                        panel,
                        index,
                        FlowUiAction::ToggleFavoriteServer,
                        favorite_label,
                    );
                    index += 1;
                }
                spawn_flow_error_button(
                    panel,
                    index,
                    FlowUiAction::RequestChangeServer,
                    "CHANGE SERVER",
                );
                index += 1;
                spawn_flow_error_button(panel, index, FlowUiAction::Quit, "QUIT");
                spawn_flow_error_button(panel, index + 1, FlowUiAction::CloseDashboardMenu, "BACK");
            });
        });
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
                    "{} waiting - {} players per match",
                    row.queued, row.formation_size
                )
            },
        )
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    reason = "Dashboard presentation reads authenticated lobby and bounded queue resources"
)]
fn update_dashboard_live_facts(
    mut commands: Commands,
    flow: Res<State<ClientFlow>>,
    selection: Res<SelectedGameType>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    queue: Res<super::ClientQueueModel>,
    practice: Res<super::ClientPracticeModel>,
    profile: Res<super::ClientProfileModel>,
    mut texts: Query<(
        &mut Text,
        Has<DashboardGameSummaryLabel>,
        Has<DashboardPlayLabel>,
        Has<DashboardPracticeLabel>,
        Has<DashboardBrawlerNameLabel>,
        Has<DashboardBrawlerSummaryLabel>,
    )>,
    mut action_buttons: Query<(
        Entity,
        &DashboardButtonStyle,
        &mut FlowButton,
        Option<&AccessibleLabel>,
    )>,
) {
    if *flow.get() != ClientFlow::Dashboard {
        return;
    }
    let Some(membership) = memberships.iter().next() else {
        return;
    };
    let Some(game) = selection.game_type_id.as_ref().and_then(|selected| {
        membership
            .game_types
            .iter()
            .find(|game| game.id == *selected)
    }) else {
        return;
    };
    let population = if queue.required_snapshot_is_fresh() {
        queue_population(&queue, game)
    } else {
        "Population updating".to_string()
    };
    let copy = format!("{}\n{population}", dashboard_game_summary(game));
    let selected_brawler = membership.profile.selected_brawler_id.and_then(|id| {
        membership
            .profile
            .brawlers
            .iter()
            .find(|brawler| brawler.id == id)
    });
    let brawler_accessible = selected_brawler.map_or_else(
        || "Create your first brawler".to_string(),
        |brawler| {
            format!(
                "View brawlers: {}, {}",
                brawler.name,
                brawler_loadout_summary(brawler, &membership.brawler_catalog)
            )
        },
    );
    for (mut text, is_summary, _, _, is_brawler_name, is_brawler_summary) in &mut texts {
        if is_summary {
            text.0.clone_from(&copy);
        } else if is_brawler_name {
            text.0 = selected_brawler.map_or_else(
                || "CREATE YOUR FIRST BRAWLER".to_string(),
                |brawler| brawler.name.to_uppercase(),
            );
        } else if is_brawler_summary {
            text.0 = selected_brawler.map_or_else(
                || "Choose a permanent fighter profile and weapon base".to_string(),
                |brawler| brawler_loadout_summary(brawler, &membership.brawler_catalog),
            );
        }
    }
    let capacity_occupied = queue.snapshot().is_some_and(|snapshot| {
        snapshot.formation_availability == crate::lobby::FormationAvailability::ProductMatchOccupied
    });
    let admission_pending = queue.pending().is_some() || practice.pending() || profile.pending();
    let profile_empty = selected_brawler.is_none();
    let play_copy = if queue.pending().is_some() {
        "Joining match; Play unavailable"
    } else if capacity_occupied {
        "Match in progress; Play unavailable"
    } else {
        "Play"
    };
    let practice_copy = if practice.pending() {
        "Starting practice; Practice unavailable"
    } else {
        "Practice"
    };
    let busy_suffix = "; unavailable while admission is pending";
    for (entity, style, mut button, current_label) in &mut action_buttons {
        let disabled = match style {
            DashboardButtonStyle::Preview
            | DashboardButtonStyle::Build
            | DashboardButtonStyle::Mode => admission_pending,
            DashboardButtonStyle::Practice => admission_pending || profile_empty,
            DashboardButtonStyle::Play => capacity_occupied || admission_pending || profile_empty,
            DashboardButtonStyle::Header => false,
        };
        if disabled {
            commands.entity(entity).insert(InteractionDisabled);
        } else {
            commands.entity(entity).remove::<InteractionDisabled>();
        }
        let next_label = match style {
            DashboardButtonStyle::Mode => format!(
                "Change game type: {}, {}, {population}{}",
                game.display_name,
                dashboard_game_summary(game),
                if admission_pending { busy_suffix } else { "" }
            ),
            DashboardButtonStyle::Preview | DashboardButtonStyle::Build => {
                button.action = if profile_empty {
                    FlowUiAction::CreateBrawler
                } else {
                    FlowUiAction::OpenBrawlerList
                };
                format!(
                    "{brawler_accessible}{}",
                    if admission_pending { busy_suffix } else { "" }
                )
            }
            DashboardButtonStyle::Practice => practice_copy.to_string(),
            DashboardButtonStyle::Play => play_copy.to_string(),
            DashboardButtonStyle::Header => continue,
        };
        if current_label.is_none_or(|current| current.0 != next_label) {
            commands
                .entity(entity)
                .insert(AccessibleLabel::new(next_label));
        }
    }
    for (mut text, _, is_play_label, is_practice_label, _, _) in &mut texts {
        if is_play_label {
            text.0 = if queue.pending().is_some() {
                "JOINING...".to_string()
            } else if capacity_occupied {
                "MATCH IN PROGRESS".to_string()
            } else {
                "PLAY".to_string()
            };
        } else if is_practice_label {
            text.0 = if practice.pending() {
                "STARTING...".to_string()
            } else {
                "PRACTICE".to_string()
            };
        }
    }
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
        "{game_name}\n{population}\nSaved brawler · {} points\n{ultimate} · {} / {}",
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

fn dashboard_layout_class(
    logical_width: f32,
    logical_height: f32,
    ui_scale: f32,
) -> DashboardLayoutClass {
    let scale = ui_scale.max(0.01);
    let effective_width = logical_width / scale;
    let effective_height = logical_height / scale;
    if effective_width < DASHBOARD_COMPACT_WIDTH || effective_height < DASHBOARD_COMPACT_HEIGHT {
        DashboardLayoutClass::Compact
    } else {
        DashboardLayoutClass::Wide
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "one change-driven pass applies the closed Wide/Compact dashboard node contract"
)]
fn apply_dashboard_layout(
    windows: Query<&Window, With<PrimaryWindow>>,
    scale: Option<Res<UiScale>>,
    mut roots: Query<(&mut DashboardLayoutClass, &mut ScrollPosition), With<DashboardRoot>>,
    mut nodes: Query<(
        &mut Node,
        &DashboardLayoutRole,
        Option<&DashboardButtonStyle>,
    )>,
) {
    let Some(window) = windows.iter().next() else {
        return;
    };
    let next = dashboard_layout_class(
        window.resolution.width(),
        window.resolution.height(),
        scale.as_deref().map_or(1.0, |scale| scale.0),
    );
    let Some((mut current, mut scroll)) = roots.iter_mut().next() else {
        return;
    };
    if *current == next {
        return;
    }
    *current = next;
    if next == DashboardLayoutClass::Wide {
        scroll.0 = Vec2::ZERO;
    }
    for (mut node, role, style) in &mut nodes {
        apply_dashboard_layout_node(&mut node, *role, style.copied(), next);
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "the closed role match keeps the complete Wide/Compact layout contract reviewable"
)]
fn apply_dashboard_layout_node(
    node: &mut Node,
    role: DashboardLayoutRole,
    style: Option<DashboardButtonStyle>,
    class: DashboardLayoutClass,
) {
    let compact = class == DashboardLayoutClass::Compact;
    match role {
        DashboardLayoutRole::Root => {
            node.row_gap = px(if compact { 8 } else { 5 });
            node.padding = if compact {
                UiRect::axes(px(8), px(6))
            } else {
                UiRect::axes(px(16), px(8))
            };
            node.overflow = if compact {
                Overflow::scroll_y()
            } else {
                Overflow::clip()
            };
        }
        DashboardLayoutRole::Header => {
            node.column_gap = px(if compact { 6 } else { 10 });
            node.padding = if compact {
                UiRect::axes(px(4), px(4))
            } else {
                UiRect::axes(px(18), px(6))
            };
        }
        DashboardLayoutRole::Wordmark => {
            node.width = px(if compact { 105 } else { 220 });
        }
        DashboardLayoutRole::Identity => {
            node.min_width = if compact { auto() } else { px(240) };
            node.flex_grow = if compact { 1.0 } else { 0.0 };
            node.flex_shrink = if compact { 1.0 } else { 0.0 };
            node.padding = if compact {
                UiRect::axes(px(8), px(5))
            } else {
                UiRect::axes(px(14), px(7))
            };
        }
        DashboardLayoutRole::HeaderSpacer => {
            node.display = if compact {
                Display::None
            } else {
                Display::Flex
            };
        }
        DashboardLayoutRole::Center => {
            node.flex_grow = if compact { 0.0 } else { 1.0 };
            node.justify_content = if compact {
                JustifyContent::FlexStart
            } else {
                JustifyContent::Center
            };
            node.row_gap = px(if compact { 8 } else { 5 });
        }
        DashboardLayoutRole::Preview => {
            node.width = percent(if compact { 90 } else { 54 });
            node.max_width = px(if compact { 520 } else { 650 });
            node.min_height = px(if compact { 180 } else { 280 });
            node.max_height = px(if compact { 220 } else { 470 });
            node.flex_grow = if compact { 0.0 } else { 1.0 };
        }
        DashboardLayoutRole::Build => {
            node.width = percent(if compact { 94 } else { 30 });
            node.max_width = px(if compact { 700 } else { 365 });
            node.min_height = px(if compact { 88 } else { 104 });
        }
        DashboardLayoutRole::ActionRow => {
            node.flex_direction = if compact {
                FlexDirection::Column
            } else {
                FlexDirection::Row
            };
            node.column_gap = px(if compact { 0 } else { 12 });
            node.row_gap = px(if compact { 8 } else { 0 });
        }
        DashboardLayoutRole::Mode => {
            node.width = percent(if compact { 100 } else { 44 });
            node.min_height = px(if compact { 94 } else { 104 });
        }
        DashboardLayoutRole::UtilityButton { wide_width } => {
            node.width = px(if compact { 48.0 } else { wide_width });
            node.min_height = px(42);
            node.padding = UiRect::axes(px(if compact { 6 } else { 12 }), px(7));
        }
        DashboardLayoutRole::UtilityLabel { has_icon } => {
            node.display = if compact && has_icon {
                Display::None
            } else {
                Display::Flex
            };
        }
    }
    match style {
        Some(DashboardButtonStyle::Practice) => {
            node.width = percent(if compact { 100 } else { 21 });
            node.min_height = px(if compact { 80 } else { 104 });
        }
        Some(DashboardButtonStyle::Play) => {
            node.width = percent(if compact { 100 } else { 33 });
            node.min_height = px(if compact { 88 } else { 104 });
        }
        _ => {}
    }
}

fn scroll_dashboard(
    mut wheel: MessageReader<MouseWheel>,
    mut roots: Query<(&DashboardLayoutClass, &mut ScrollPosition), With<DashboardRoot>>,
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
    for (class, mut position) in &mut roots {
        if *class == DashboardLayoutClass::Compact {
            position.0.y = (position.0.y - delta).max(0.0);
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "computed UI bounds are available only after Bevy's layout pass"
)]
fn keep_dashboard_focus_visible(
    navigation: Res<FlowNavigation>,
    buttons: Query<(&FlowButton, &ComputedNode, &UiGlobalTransform)>,
    mut roots: Query<
        (
            Entity,
            &DashboardLayoutClass,
            &ComputedNode,
            &UiGlobalTransform,
            &mut ScrollPosition,
        ),
        With<DashboardRoot>,
    >,
    mut prior: Local<Option<(Entity, DashboardLayoutClass, usize)>>,
) {
    let Some((root_entity, class, _, _, _)) = roots.iter_mut().next() else {
        *prior = None;
        return;
    };
    let focus_key = (root_entity, *class, navigation.selected);
    if prior.as_ref() == Some(&focus_key) {
        return;
    }
    *prior = Some(focus_key);
    let mut focused_top = f32::MAX;
    let mut focused_bottom = f32::MIN;
    let mut found = false;
    for (button, node, transform) in &buttons {
        if button.index != navigation.selected || node.is_empty() {
            continue;
        }
        let (_, _, center) = transform.to_scale_angle_translation();
        found = true;
        focused_top = focused_top.min(center.y - node.size().y * 0.5);
        focused_bottom = focused_bottom.max(center.y + node.size().y * 0.5);
    }
    if !found {
        return;
    }
    for (_, class, root_node, root_transform, mut scroll) in &mut roots {
        if *class != DashboardLayoutClass::Compact || root_node.is_empty() {
            continue;
        }
        let (_, _, center) = root_transform.to_scale_angle_translation();
        let half_height = root_node.size().y * 0.5;
        let visible_top = center.y - half_height + 8.0;
        let visible_bottom = center.y + half_height - 8.0;
        if focused_top < visible_top {
            scroll.0.y = (scroll.0.y - (visible_top - focused_top)).max(0.0);
        } else if focused_bottom > visible_bottom {
            scroll.0.y += focused_bottom - visible_bottom;
        }
    }
}

fn dashboard_root_node() -> Node {
    Node {
        width: percent(100),
        height: percent(100),
        display: Display::Flex,
        flex_direction: FlexDirection::Column,
        align_items: AlignItems::Center,
        justify_content: JustifyContent::FlexStart,
        row_gap: px(5),
        padding: UiRect::axes(px(16), px(8)),
        ..default()
    }
}

fn dashboard_font(assets: Option<&super::ClientAssetHandles>, size: f32) -> TextFont {
    assets.map_or_else(
        || TextFont::from_font_size(size),
        |assets| TextFont {
            font: assets.dashboard_font.clone().into(),
            font_size: FontSize::Px(size),
            ..default()
        },
    )
}

fn spawn_dashboard_icon_well(
    parent: &mut ChildSpawnerCommands,
    icon: Option<Handle<Image>>,
    well_size: f32,
    icon_size: f32,
    color: Color,
) {
    parent
        .spawn((
            Node {
                width: px(well_size),
                height: px(well_size),
                flex_shrink: 0.0,
                justify_content: JustifyContent::Center,
                align_items: AlignItems::Center,
                border_radius: BorderRadius::all(px(well_size * 0.28)),
                ..default()
            },
            BackgroundColor(color),
        ))
        .with_children(|well| {
            if let Some(icon) = icon {
                well.spawn((
                    ImageNode::new(icon),
                    Node {
                        width: px(icon_size),
                        height: px(icon_size),
                        ..default()
                    },
                ));
            }
        });
}

struct DashboardButtonPresentation<'a> {
    label: &'a str,
    width: Val,
    primary: bool,
    disabled: bool,
    assets: Option<&'a super::ClientAssetHandles>,
    icon: Option<Handle<Image>>,
}

#[derive(Clone, Copy)]
enum DashboardButtonContentKind {
    Play,
    Practice,
    Utility { has_icon: bool },
    Other,
}

const fn dashboard_button_icon_size(is_play: bool, is_practice: bool) -> f32 {
    if is_play {
        42.0
    } else if is_practice {
        32.0
    } else {
        21.0
    }
}

fn spawn_dashboard_button(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    presentation: DashboardButtonPresentation<'_>,
) {
    let DashboardButtonPresentation {
        label,
        width,
        primary,
        disabled,
        assets,
        icon,
    } = presentation;
    let is_play = matches!(action, FlowUiAction::JoinQueue);
    let is_practice = matches!(action, FlowUiAction::StartPractice);
    let is_utility = matches!(
        action,
        FlowUiAction::OpenSettings | FlowUiAction::OpenDashboardMenu
    );
    let has_icon = icon.is_some();
    let utility_width = matches!(action, FlowUiAction::OpenSettings)
        .then_some(112.0)
        .or_else(|| matches!(action, FlowUiAction::OpenDashboardMenu).then_some(92.0));
    let icon_size = dashboard_button_icon_size(is_play, is_practice);
    let mut button = parent.spawn((
        Button,
        AccessibleLabel::new(label),
        FlowButton {
            index,
            action,
            error_action: false,
        },
        Node {
            width,
            min_height: px(if is_play || is_practice { 104 } else { 42 }),
            column_gap: px(if is_play || is_practice { 12 } else { 7 }),
            padding: UiRect::axes(px(12), px(7)),
            justify_content: JustifyContent::Center,
            align_items: AlignItems::Center,
            border: UiRect::all(px(if primary { 3 } else { 2 })),
            border_radius: BorderRadius::all(px(if primary { 16 } else { 11 })),
            ..default()
        },
        BackgroundColor(if primary {
            Color::srgb(0.95, 0.48, 0.08)
        } else {
            Color::srgb(0.09, 0.14, 0.2)
        }),
        BorderColor::all(Color::NONE),
        BoxShadow::new(
            Color::srgba(0.0, 0.02, 0.08, 0.65),
            px(0),
            px(if is_play { 7 } else { 4 }),
            px(0),
            px(3),
        ),
    ));
    if is_play {
        button.insert(DashboardButtonStyle::Play);
    } else if is_practice {
        button.insert(DashboardButtonStyle::Practice);
    } else {
        button.insert(DashboardButtonStyle::Header);
    }
    if let Some(wide_width) = utility_width {
        button.insert(DashboardLayoutRole::UtilityButton { wide_width });
    }
    if disabled {
        button.insert(InteractionDisabled);
    }
    let content_kind = if is_play {
        DashboardButtonContentKind::Play
    } else if is_practice {
        DashboardButtonContentKind::Practice
    } else if is_utility {
        DashboardButtonContentKind::Utility { has_icon }
    } else {
        DashboardButtonContentKind::Other
    };
    button.with_children(|button| {
        spawn_dashboard_button_contents(button, label, assets, icon, icon_size, content_kind);
    });
}

fn spawn_dashboard_button_contents(
    parent: &mut ChildSpawnerCommands,
    label: &str,
    assets: Option<&super::ClientAssetHandles>,
    icon: Option<Handle<Image>>,
    icon_size: f32,
    kind: DashboardButtonContentKind,
) {
    if let Some(icon) = icon {
        parent.spawn((
            ImageNode::new(icon),
            Node {
                width: px(icon_size),
                height: px(icon_size),
                ..default()
            },
        ));
    }
    let font_size = match kind {
        DashboardButtonContentKind::Play => 38.0,
        DashboardButtonContentKind::Practice => 24.0,
        DashboardButtonContentKind::Utility { .. } | DashboardButtonContentKind::Other => 15.0,
    };
    let color = match kind {
        DashboardButtonContentKind::Play => Color::WHITE,
        DashboardButtonContentKind::Practice => Color::srgb(0.04, 0.2, 0.55),
        DashboardButtonContentKind::Utility { .. } | DashboardButtonContentKind::Other => {
            Color::srgb(0.9, 0.95, 1.0)
        }
    };
    let mut text = parent.spawn((
        Text::new(label),
        dashboard_font(assets, font_size),
        TextColor(color),
    ));
    match kind {
        DashboardButtonContentKind::Play => {
            text.insert(DashboardPlayLabel);
        }
        DashboardButtonContentKind::Practice => {
            text.insert(DashboardPracticeLabel);
        }
        DashboardButtonContentKind::Utility { has_icon } => {
            text.insert(DashboardLayoutRole::UtilityLabel { has_icon });
        }
        DashboardButtonContentKind::Other => {}
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
    spawn_flow_button_disabled(parent, index, action, label, field, false);
}

fn spawn_flow_button_disabled(
    parent: &mut ChildSpawnerCommands,
    index: usize,
    action: FlowUiAction,
    label: &str,
    field: Option<FieldLabel>,
    disabled: bool,
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
    if disabled {
        entity.insert(InteractionDisabled);
    }
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
    spawn_flow_error_button_disabled(parent, index, action, label, false);
}

fn spawn_flow_error_button_disabled(
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
    ));
    if disabled {
        entity.insert(InteractionDisabled);
    }
    entity.with_child((Text::new(label), TextFont::from_font_size(18.0)));
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

fn flow_button_background(
    disabled: bool,
    interaction: Interaction,
    focused: bool,
    selected: bool,
    dashboard_style: Option<DashboardButtonStyle>,
) -> Color {
    if matches!(dashboard_style, Some(DashboardButtonStyle::Preview)) {
        // The 3D preview is embedded directly in the dashboard. Its interaction feedback must
        // never cover the model or procedural background with a UI rectangle.
        return Color::NONE;
    }
    if disabled {
        return Color::srgb(0.1, 0.1, 0.12);
    }
    match (interaction, dashboard_style) {
        (Interaction::Pressed, Some(DashboardButtonStyle::Play)) => Color::srgb(0.92, 0.48, 0.02),
        (
            Interaction::Pressed,
            Some(
                DashboardButtonStyle::Build
                | DashboardButtonStyle::Mode
                | DashboardButtonStyle::Practice,
            ),
        ) => Color::srgb(0.78, 0.87, 0.98),
        (Interaction::Pressed, Some(DashboardButtonStyle::Header)) => {
            Color::srgb(0.025, 0.22, 0.58)
        }
        (Interaction::Pressed, None) => Color::srgb(0.08, 0.48, 0.58),
        (Interaction::Hovered, Some(DashboardButtonStyle::Play)) => Color::srgb(1.0, 0.7, 0.08),
        (
            Interaction::Hovered,
            Some(
                DashboardButtonStyle::Build
                | DashboardButtonStyle::Mode
                | DashboardButtonStyle::Practice,
            ),
        ) => Color::srgb(0.86, 0.93, 1.0),
        (Interaction::Hovered, Some(DashboardButtonStyle::Header)) => Color::srgb(0.06, 0.4, 0.9),
        (Interaction::Hovered, None) => Color::srgb(0.12, 0.32, 0.42),
        (_, Some(DashboardButtonStyle::Play)) => Color::srgb(1.0, 0.62, 0.04),
        (
            _,
            Some(
                DashboardButtonStyle::Build
                | DashboardButtonStyle::Mode
                | DashboardButtonStyle::Practice,
            ),
        ) => Color::srgb(0.92, 0.95, 1.0),
        (_, Some(DashboardButtonStyle::Header)) => Color::srgb(0.035, 0.3, 0.76),
        (_, None) if focused => Color::srgb(0.12, 0.32, 0.42),
        (_, None) if selected => Color::srgb(0.12, 0.24, 0.34),
        (_, None) => Color::srgb(0.09, 0.14, 0.2),
        (_, Some(DashboardButtonStyle::Preview)) => unreachable!("handled above"),
    }
}

fn flow_button_border(
    disabled: bool,
    interaction: Interaction,
    focused: bool,
    selected: bool,
    dashboard_style: Option<DashboardButtonStyle>,
) -> Color {
    if disabled {
        Color::NONE
    } else if matches!(dashboard_style, Some(DashboardButtonStyle::Preview))
        && (interaction == Interaction::Hovered || focused)
    {
        Color::srgb(0.25, 0.9, 1.0)
    } else if focused {
        Color::WHITE
    } else if matches!(dashboard_style, Some(DashboardButtonStyle::Play)) {
        Color::srgb(1.0, 0.86, 0.35)
    } else if matches!(dashboard_style, Some(DashboardButtonStyle::Header)) {
        Color::srgb(0.18, 0.58, 1.0)
    } else if matches!(
        dashboard_style,
        Some(
            DashboardButtonStyle::Build
                | DashboardButtonStyle::Mode
                | DashboardButtonStyle::Practice
        )
    ) {
        Color::srgb(0.48, 0.66, 0.9)
    } else if selected {
        Color::srgb(0.25, 0.9, 1.0)
    } else {
        Color::NONE
    }
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
    game_draft: Res<GameTypeSelectionDraft>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    pending: Option<Res<PendingConnection>>,
    mut buttons: Query<(
        &FlowButton,
        &Interaction,
        Has<InteractionDisabled>,
        &mut BackgroundColor,
        &mut BorderColor,
        Option<&DashboardButtonStyle>,
    )>,
    mut fields: Query<(&FieldLabel, &mut Text)>,
    mut connecting: Query<&mut Text, (With<ConnectingLabel>, Without<FieldLabel>)>,
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
    let selected_brawler_id = memberships
        .iter()
        .next()
        .and_then(|membership| membership.profile.selected_brawler_id);
    for (button, interaction, disabled, mut background, mut border, dashboard_style) in &mut buttons
    {
        let focused = button.index == navigation.selected;
        let selected_game = match button.action {
            FlowUiAction::SelectGameTypeDraft(index) => game_draft.selected_index == Some(index),
            _ => false,
        };
        let selected_brawler = matches!(
            button.action,
            FlowUiAction::OpenBrawlerDetails(id) if Some(id) == selected_brawler_id
        );
        let selected = selected_game || selected_brawler;
        let dashboard_style = dashboard_style.copied();
        background.0 =
            flow_button_background(disabled, *interaction, focused, selected, dashboard_style);
        border.set_all(flow_button_border(
            disabled,
            *interaction,
            focused,
            selected,
            dashboard_style,
        ));
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

fn render_editor_value(model: &ServerSelectModel, field: EditingField) -> String {
    let value = edited_value(model, field);
    if model.editing != Some(field) {
        return value.to_string();
    }
    let caret = model.caret.min(value.len());
    format!("{}|{}", &value[..caret], &value[caret..])
}

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

    #[test]
    fn brawler_editor_uses_catalog_name_for_concealment_field() {
        let _app = flow_test_app();
        let catalog = crate::profiles::AdvertisedBrawlerCatalog::from_content(
            &crate::builds::BuildCatalog::embedded().unwrap(),
            &crate::combat::WeaponCatalog::embedded().unwrap(),
        )
        .unwrap();

        assert_eq!(
            ultimate_name(&catalog, crate::builds::UltimateDefinitionId(5)),
            "Concealment Field"
        );
    }

    #[test]
    fn dashboard_layout_class_uses_effective_ui_space() {
        assert_eq!(
            dashboard_layout_class(1280.0, 720.0, 1.0),
            DashboardLayoutClass::Wide
        );
        assert_eq!(
            dashboard_layout_class(1280.0, 720.0, 1.4),
            DashboardLayoutClass::Compact
        );
        assert_eq!(
            dashboard_layout_class(640.0, 360.0, 0.8),
            DashboardLayoutClass::Compact
        );
        assert_eq!(
            dashboard_layout_class(1000.0, 640.0, 1.0),
            DashboardLayoutClass::Wide
        );
        assert_eq!(
            dashboard_layout_class(999.0, 640.0, 1.0),
            DashboardLayoutClass::Compact
        );
    }

    #[test]
    fn dashboard_spatial_navigation_matches_wide_and_compact_layouts() {
        let all = [
            DASHBOARD_PLAY_INDEX,
            DASHBOARD_PRACTICE_INDEX,
            DASHBOARD_GAME_INDEX,
            DASHBOARD_BUILD_INDEX,
            DASHBOARD_SETTINGS_INDEX,
            DASHBOARD_MENU_INDEX,
        ];
        assert_eq!(
            dashboard_focus_neighbor(
                DashboardLayoutClass::Wide,
                DASHBOARD_PLAY_INDEX,
                DashboardNavigationDirection::Left,
                &all,
            ),
            DASHBOARD_PRACTICE_INDEX
        );
        assert_eq!(
            dashboard_focus_neighbor(
                DashboardLayoutClass::Wide,
                DASHBOARD_PRACTICE_INDEX,
                DashboardNavigationDirection::Up,
                &all,
            ),
            DASHBOARD_BUILD_INDEX
        );
        assert_eq!(
            dashboard_focus_neighbor(
                DashboardLayoutClass::Compact,
                DASHBOARD_GAME_INDEX,
                DashboardNavigationDirection::Down,
                &all,
            ),
            DASHBOARD_PRACTICE_INDEX
        );
        assert_eq!(
            dashboard_focus_neighbor(
                DashboardLayoutClass::Compact,
                DASHBOARD_PLAY_INDEX,
                DashboardNavigationDirection::Up,
                &all,
            ),
            DASHBOARD_PRACTICE_INDEX
        );
    }

    #[test]
    fn dashboard_navigation_skips_disabled_targets_and_repairs_focus() {
        let available = [
            DASHBOARD_GAME_INDEX,
            DASHBOARD_BUILD_INDEX,
            DASHBOARD_SETTINGS_INDEX,
            DASHBOARD_MENU_INDEX,
        ];
        assert_eq!(
            dashboard_focus_neighbor(
                DashboardLayoutClass::Wide,
                DASHBOARD_PLAY_INDEX,
                DashboardNavigationDirection::Left,
                &available,
            ),
            DASHBOARD_GAME_INDEX
        );
        assert_eq!(
            repair_dashboard_focus(DASHBOARD_PLAY_INDEX, &available),
            DASHBOARD_GAME_INDEX
        );
        assert_eq!(
            dashboard_focus_neighbor(
                DashboardLayoutClass::Wide,
                DASHBOARD_SETTINGS_INDEX,
                DashboardNavigationDirection::Left,
                &available,
            ),
            DASHBOARD_SETTINGS_INDEX
        );
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

    fn press_flow_button(app: &mut App, action: &FlowUiAction) {
        let entity = {
            let world = app.world_mut();
            let mut query = world.query::<(Entity, &FlowButton)>();
            query
                .iter(world)
                .find_map(|(entity, button)| (&button.action == action).then_some(entity))
                .unwrap_or_else(|| panic!("missing rendered flow button for {action:?}"))
        };
        app.world_mut()
            .entity_mut(entity)
            .insert(Interaction::Pressed);
        app.update();
    }

    fn lobby_membership() -> ClientLobbyMembership {
        let account_id = crate::profiles::AccountId::new(1).unwrap();
        ClientLobbyMembership {
            logical_server_id: 1,
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
            brawler_catalog: crate::profiles::AdvertisedBrawlerCatalog::from_content(
                &crate::builds::BuildCatalog::embedded().unwrap(),
                &crate::combat::WeaponCatalog::embedded().unwrap(),
            )
            .unwrap(),
            profile: crate::profiles::ProfileSnapshot::empty(account_id),
        }
    }

    fn lobby_membership_with_brawler() -> ClientLobbyMembership {
        let mut membership = lobby_membership();
        let brawler_id = crate::profiles::SavedBrawlerId::new(2).unwrap();
        membership
            .profile
            .brawlers
            .push(crate::profiles::SavedBrawler {
                id: brawler_id,
                creation_ordinal: 1,
                name: "Test Brawler".into(),
                fighter_profile_id: crate::profiles::FighterProfileId(1),
                weapon_base_id: crate::profiles::WeaponBaseId(1),
                ultimate_id: crate::builds::UltimateDefinitionId(1),
                passive_ids: [
                    crate::builds::PassiveDefinitionId(3),
                    crate::builds::PassiveDefinitionId(4),
                ],
                equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
                revision: crate::profiles::ProfileRevision::INITIAL,
            });
        membership.profile.selected_brawler_id = Some(brawler_id);
        membership.profile.next_brawler_ordinal = 2;
        membership
    }

    #[test]
    fn validated_target_freezes_canonical_address_and_normalized_name() {
        let target = validate_target("LOCALHOST", " Cafe\u{301} ").unwrap();
        assert_eq!(target.logical_address.canonical(), "localhost:5000");
        assert_eq!(target.proposed_display_name, "Café");
    }

    #[test]
    fn startup_server_precedence_is_explicit_then_recent_then_product_default() {
        let mut config = ClientNetworkConfig::new(7);
        let mut connections = ConnectionsFileV1::empty();
        assert_eq!(
            startup_server_address(&config, &connections),
            "127.0.0.1:5000"
        );

        connections
            .record_recent("Last Success", "recent.example:6000")
            .unwrap();
        assert_eq!(
            startup_server_address(&config, &connections),
            "recent.example:6000"
        );

        config.product_server_prefill = Some("explicit.example:7000".to_string());
        assert_eq!(
            startup_server_address(&config, &connections),
            "explicit.example:7000"
        );
    }

    #[test]
    fn rendered_server_select_connect_button_starts_connection() {
        let mut app = flow_test_app();
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::ServerSelect);
        app.update();

        press_flow_button(&mut app, &FlowUiAction::Connect);

        assert!(app.world().contains_resource::<PendingConnection>());
        app.update();
        assert_eq!(
            *app.world().resource::<State<ClientFlow>>().get(),
            ClientFlow::Connecting
        );
    }

    #[test]
    fn rendered_dashboard_menu_buttons_dispatch_their_actions() {
        let mut app = flow_test_app();
        app.world_mut().spawn((Client, lobby_membership()));
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::Dashboard);
        app.update();

        press_flow_button(&mut app, &FlowUiAction::OpenDashboardMenu);
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::DashboardMenu
        );
        app.update();

        press_flow_button(&mut app, &FlowUiAction::OpenCredits);
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::Credits
        );
    }

    #[test]
    fn empty_profile_creation_is_an_opaque_full_screen_destination() {
        let mut app = flow_test_app();
        let membership = lobby_membership();
        app.world_mut()
            .resource_mut::<super::super::ClientProfileModel>()
            .set_snapshot_for_test(membership.profile.clone());
        app.world_mut().spawn((Client, membership));
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::Dashboard);
        app.update();
        app.update();

        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::BrawlerCreation
        );
        let world = app.world_mut();
        let mut roots =
            world.query_filtered::<(&Node, &BackgroundColor), With<BrawlerCreationRoot>>();
        let (node, background) = roots.single(world).unwrap();
        assert_eq!(
            (node.left, node.right, node.top, node.bottom),
            (px(0), px(0), px(0), px(0))
        );
        assert!((background.0.to_srgba().alpha - 1.0).abs() <= f32::EPSILON);
    }

    #[test]
    #[allow(
        clippy::too_many_lines,
        reason = "one end-to-end UI regression follows the approved Dashboard-to-list-to-detail-to-customization path"
    )]
    fn selected_brawler_cards_open_list_details_and_reach_equipment() {
        let mut app = flow_test_app();
        let membership = lobby_membership_with_brawler();
        let brawler_id = membership.profile.selected_brawler_id.unwrap();
        app.world_mut()
            .resource_mut::<super::super::ClientProfileModel>()
            .set_snapshot_for_test(membership.profile.clone());
        app.world_mut().spawn((Client, membership));
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::Dashboard);
        app.update();
        let dashboard_copy = visible_text(&mut app);
        assert!(
            dashboard_copy
                .iter()
                .any(|text| text.contains("Default · Pulse Sidearm"))
        );
        assert!(
            dashboard_copy
                .iter()
                .any(|text| text.contains("Dash · Adrenal Response + Close Quarters"))
        );
        assert!(
            !dashboard_copy
                .iter()
                .any(|text| text.contains("Weapon base 1"))
        );
        assert!(dashboard_copy.iter().any(|text| text == "VIEW BRAWLERS"));
        assert!(
            !dashboard_copy
                .iter()
                .any(|text| text.contains("SELECTED FOR PLAY"))
        );

        let preview = {
            let world = app.world_mut();
            let mut query = world.query::<(Entity, &DashboardButtonStyle, &FlowButton)>();
            let mut selected = None;
            for (entity, style, button) in query.iter(world) {
                if matches!(
                    style,
                    DashboardButtonStyle::Preview | DashboardButtonStyle::Build
                ) {
                    assert_eq!(button.action, FlowUiAction::OpenBrawlerList);
                    if matches!(style, DashboardButtonStyle::Preview) {
                        selected = Some(entity);
                    }
                }
            }
            selected.expect("Dashboard renders its selected-brawler preview")
        };
        app.world_mut()
            .entity_mut(preview)
            .insert(Interaction::Pressed);
        app.update();
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::BrawlerList
        );
        app.update();
        {
            let world = app.world_mut();
            let mut roots =
                world.query_filtered::<(&Node, &BackgroundColor), With<BrawlerListRoot>>();
            let (node, background) = roots.single(world).unwrap();
            assert_eq!(node.left, px(0));
            assert_eq!(node.right, px(0));
            assert_eq!(node.top, px(0));
            assert_eq!(node.bottom, px(0));
            assert!((background.0.to_srgba().alpha - 1.0).abs() <= f32::EPSILON);
        }
        assert!(
            visible_text(&mut app)
                .iter()
                .any(|text| text.contains("SELECTED FOR PLAY"))
        );
        assert!(
            visible_text(&mut app)
                .iter()
                .any(|text| text.contains("Dash"))
        );

        press_flow_button(&mut app, &FlowUiAction::OpenBrawlerDetails(brawler_id));
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::BrawlerDetails(brawler_id)
        );
        app.update();
        {
            let world = app.world_mut();
            let mut roots =
                world.query_filtered::<(&Node, &BackgroundColor), With<BrawlerDetailsRoot>>();
            let (node, background) = roots.single(world).unwrap();
            assert_eq!(node.left, px(0));
            assert_eq!(node.right, px(0));
            assert_eq!(node.top, px(0));
            assert_eq!(node.bottom, px(0));
            assert!((background.0.to_srgba().alpha - 1.0).abs() <= f32::EPSILON);
            let mut previews = world.query_filtered::<Entity, With<BrawlerDetailsPreviewHost>>();
            assert_eq!(previews.iter(world).count(), 1);
        }
        assert!(
            visible_text(&mut app)
                .iter()
                .any(|text| text.contains("Pulse Sidearm"))
        );

        let select_disabled = {
            let world = app.world_mut();
            let mut buttons = world.query::<(&FlowButton, Has<InteractionDisabled>)>();
            buttons
                .iter(world)
                .find(|(button, _)| button.action == FlowUiAction::SelectBrawler(brawler_id))
                .map(|(_, disabled)| disabled)
                .expect("selected brawler retains its primary action")
        };
        assert!(!select_disabled);
        press_flow_button(&mut app, &FlowUiAction::SelectBrawler(brawler_id));
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::None
        );
        assert!(
            !app.world()
                .resource::<super::super::ClientProfileModel>()
                .pending(),
            "returning with the selected brawler must not send a profile mutation"
        );
        *app.world_mut().resource_mut::<ClientOverlay>() =
            ClientOverlay::BrawlerDetails(brawler_id);
        app.update();

        press_flow_button(&mut app, &FlowUiAction::DeleteBrawler(brawler_id));
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::DeleteBrawlerConfirmation(brawler_id)
        );
        app.update();
        {
            let world = app.world_mut();
            let mut details = world.query::<&BrawlerDetailsRoot>();
            assert!(details.single(world).unwrap().contextual_confirmation);
            let mut confirmations =
                world.query_filtered::<Entity, With<DeleteBrawlerConfirmationRoot>>();
            assert_eq!(confirmations.iter(world).count(), 1);
            let mut background_actions = world.query::<(&FlowButton, Has<InteractionDisabled>)>();
            assert!(
                background_actions
                    .iter(world)
                    .filter(|(button, _)| {
                        matches!(
                            button.action,
                            FlowUiAction::SelectBrawler(_)
                                | FlowUiAction::OpenBrawlerEditor(_)
                                | FlowUiAction::OpenWeaponEquipment(_)
                                | FlowUiAction::DeleteBrawler(_)
                        )
                    })
                    .all(|(_, disabled)| disabled)
            );
        }
        press_flow_button(&mut app, &FlowUiAction::CancelDeleteBrawler);
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::BrawlerDetails(brawler_id)
        );
        app.update();

        press_flow_button(&mut app, &FlowUiAction::OpenBrawlerEditor(brawler_id));
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::BrawlerEditor
        );
        app.update();
        {
            let world = app.world_mut();
            let mut roots =
                world.query_filtered::<(&Node, &BackgroundColor), With<BrawlerEditorRoot>>();
            let (node, background) = roots.single(world).unwrap();
            assert_eq!(
                (node.left, node.right, node.top, node.bottom),
                (px(0), px(0), px(0), px(0))
            );
            assert!((background.0.to_srgba().alpha - 1.0).abs() <= f32::EPSILON);
        }
        press_flow_button(&mut app, &FlowUiAction::CancelBrawlerEdit);
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::BrawlerDetails(brawler_id)
        );
        app.update();
        press_flow_button(&mut app, &FlowUiAction::OpenWeaponEquipment(brawler_id));
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::WeaponEquipment
        );
        app.update();
        let (scroll_area, save_parent, save_disabled) = {
            let world = app.world_mut();
            let mut roots = world
                .query_filtered::<(Entity, &Node, &BackgroundColor), With<WeaponEquipmentRoot>>();
            let (_, node, background) = roots.single(world).unwrap();
            assert_eq!(
                (node.left, node.right, node.top, node.bottom),
                (px(0), px(0), px(0), px(0))
            );
            assert!((background.0.to_srgba().alpha - 1.0).abs() <= f32::EPSILON);
            let mut areas = world.query_filtered::<Entity, With<WeaponEquipmentScrollArea>>();
            let area = areas.single(world).unwrap();
            let mut buttons = world.query::<(&FlowButton, &ChildOf, Has<InteractionDisabled>)>();
            let (parent, disabled) = buttons
                .iter(world)
                .find(|(button, _, _)| button.action == FlowUiAction::ConfirmWeaponEquipment)
                .map(|(_, child_of, disabled)| (child_of.parent(), disabled))
                .expect("equipment Save button is rendered");
            (area, parent, disabled)
        };
        assert_ne!(save_parent, scroll_area, "Save remains in the fixed footer");
        assert!(!save_disabled, "a valid equipment preview can be saved");

        app.world_mut().write_message(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: -2.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        app.update();
        assert!(
            (app.world().get::<ScrollPosition>(scroll_area).unwrap().0.y - 48.0).abs()
                <= f32::EPSILON
        );

        press_flow_button(&mut app, &FlowUiAction::ConfirmWeaponEquipment);
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::BrawlerDetails(brawler_id)
        );
        assert!(
            app.world()
                .resource::<super::super::ClientProfileModel>()
                .pending()
        );
    }

    #[test]
    fn brawler_details_refreshes_when_selection_request_finishes() {
        let mut app = flow_test_app();
        let mut membership = lobby_membership_with_brawler();
        let selected_id = membership.profile.selected_brawler_id.unwrap();
        let mut candidate = membership.profile.brawlers[0].clone();
        candidate.id = crate::profiles::SavedBrawlerId::new(3).unwrap();
        candidate.creation_ordinal = 2;
        candidate.name = "Candidate Brawler".into();
        membership.profile.brawlers.push(candidate.clone());
        membership.profile.next_brawler_ordinal = 3;
        app.world_mut()
            .resource_mut::<super::super::ClientProfileModel>()
            .set_snapshot_for_test(membership.profile.clone());
        app.world_mut().spawn((Client, membership.clone()));
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::Dashboard);
        app.update();

        *app.world_mut().resource_mut::<ClientOverlay>() =
            ClientOverlay::BrawlerDetails(candidate.id);
        app.update();
        assert!(
            app.world_mut()
                .resource_mut::<super::super::ClientProfileModel>()
                .select(candidate.id)
        );
        app.update();
        assert!(
            visible_text(&mut app)
                .iter()
                .any(|text| text == "SELECTING...")
        );

        membership.profile.selected_brawler_id = Some(selected_id);
        app.world_mut()
            .resource_mut::<super::super::ClientProfileModel>()
            .set_snapshot_for_test(membership.profile);
        app.update();

        let copy = visible_text(&mut app);
        assert!(copy.iter().any(|text| text == "SELECT FOR PLAY"));
        assert!(!copy.iter().any(|text| text == "SELECTING..."));
        let world = app.world_mut();
        let mut buttons = world.query::<(&FlowButton, Has<InteractionDisabled>)>();
        let (_, disabled) = buttons
            .iter(world)
            .find(|(button, _)| button.action == FlowUiAction::SelectBrawler(candidate.id))
            .expect("selection button remains rendered after the outcome");
        assert!(!disabled);
    }

    #[test]
    fn change_server_confirmation_clears_before_server_select_connect() {
        let mut app = flow_test_app();
        app.world_mut().spawn((
            Client,
            lobby_membership(),
            RoutedClientSession {
                generation: 1,
                kind: super::super::RoutedClientSessionKind::Lobby,
            },
            RuntimeLobbyTarget {
                logical_address: "127.0.0.1:5000".to_string(),
                proposed_display_name: "Player".to_string(),
            },
        ));
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::Dashboard);
        app.update();

        press_flow_button(&mut app, &FlowUiAction::OpenDashboardMenu);
        app.update();
        press_flow_button(&mut app, &FlowUiAction::RequestChangeServer);
        app.update();
        press_flow_button(&mut app, &FlowUiAction::ConfirmChangeServer);
        app.update();

        assert_eq!(
            *app.world().resource::<State<ClientFlow>>().get(),
            ClientFlow::ServerSelect
        );
        assert_eq!(
            *app.world().resource::<ClientOverlay>(),
            ClientOverlay::None
        );

        press_flow_button(&mut app, &FlowUiAction::Connect);
        app.update();
        assert_eq!(
            *app.world().resource::<State<ClientFlow>>().get(),
            ClientFlow::Connecting
        );
    }

    #[test]
    fn dashboard_menu_omits_favorite_without_a_real_server_target() {
        let mut app = flow_test_app();
        let clients = {
            let world = app.world_mut();
            let mut query = world.query_filtered::<Entity, With<Client>>();
            query.iter(world).collect::<Vec<_>>()
        };
        for entity in clients {
            app.world_mut().despawn(entity);
        }
        app.world_mut().spawn((Client, lobby_membership()));
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::Dashboard);
        app.update();

        press_flow_button(&mut app, &FlowUiAction::OpenDashboardMenu);
        app.update();

        let world = app.world_mut();
        let mut query = world.query::<&FlowButton>();
        assert!(
            !query
                .iter(world)
                .any(|button| button.action == FlowUiAction::ToggleFavoriteServer)
        );
    }

    #[test]
    fn dashboard_mode_card_separates_title_and_pool_without_claiming_a_selected_map() {
        let game = lobby_membership().game_types.remove(0);
        let summary = dashboard_game_summary(&game);
        assert!(game.display_name.contains("Wipeout"));
        assert!(summary.contains("First to"));
        assert!(summary.contains("Map pool:"));
        assert!(!summary.contains("Selected map"));
    }

    #[test]
    fn dashboard_actions_have_explicit_fact_based_accessible_labels() {
        let mut app = flow_test_app();
        app.world_mut().spawn((Client, lobby_membership()));
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::Dashboard);
        app.update();

        let world = app.world_mut();
        let mut query = world.query::<(&DashboardButtonStyle, &AccessibleLabel)>();
        let labels = query
            .iter(world)
            .map(|(style, label)| (*style, label.0.clone()))
            .collect::<Vec<_>>();
        assert_eq!(labels.len(), 7);
        assert!(labels.iter().all(|(_, label)| !label.trim().is_empty()));
        assert!(labels.iter().any(|(style, label)| {
            matches!(style, DashboardButtonStyle::Preview) && label == "Create your first brawler"
        }));
        assert!(labels.iter().any(|(style, label)| {
            matches!(style, DashboardButtonStyle::Mode)
                && label.contains("Map pool:")
                && !label.contains("Selected map")
        }));
        assert!(labels.iter().any(|(style, label)| {
            matches!(style, DashboardButtonStyle::Play) && label == "Play"
        }));
    }

    #[test]
    fn dashboard_fighter_preview_stays_transparent_during_every_interaction() {
        for interaction in [
            Interaction::None,
            Interaction::Hovered,
            Interaction::Pressed,
        ] {
            assert_eq!(
                flow_button_background(
                    false,
                    interaction,
                    false,
                    false,
                    Some(DashboardButtonStyle::Preview),
                ),
                Color::NONE
            );
        }
        assert_ne!(
            flow_button_border(
                false,
                Interaction::Hovered,
                false,
                false,
                Some(DashboardButtonStyle::Preview),
            ),
            Color::NONE
        );
    }

    #[test]
    fn dashboard_action_hover_colors_are_visibly_distinct_from_rest() {
        for style in [
            DashboardButtonStyle::Header,
            DashboardButtonStyle::Build,
            DashboardButtonStyle::Mode,
            DashboardButtonStyle::Practice,
            DashboardButtonStyle::Play,
        ] {
            assert_ne!(
                flow_button_background(false, Interaction::None, false, false, Some(style)),
                flow_button_background(false, Interaction::Hovered, false, false, Some(style)),
                "{style:?} must have a visible hover fill"
            );
        }
    }

    #[test]
    fn flow_has_the_v5_connected_state_set() {
        let states = [
            ClientFlow::Connecting,
            ClientFlow::ServerSelect,
            ClientFlow::Dashboard,
            ClientFlow::GameTypeSelect,
            ClientFlow::Queue,
            ClientFlow::MatchLoading,
            ClientFlow::Match,
            ClientFlow::Results,
        ];
        assert_eq!(states.len(), 8);
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
            .set(ClientFlow::GameTypeSelect);
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
        app.world_mut()
            .resource_mut::<RoutedClientLifecycle>()
            .generation = 3;
        app.update();
        app.update();

        assert_eq!(
            *app.world().resource::<State<ClientFlow>>().get(),
            ClientFlow::Dashboard
        );
        assert!(app.world().get_entity(completion_root).is_err());
        assert_eq!(count_flow_roots(&mut app), 1);
    }

    #[test]
    #[allow(clippy::too_many_lines)]
    fn results_replay_uses_the_exact_fresh_lobby_game() {
        let mut app = flow_test_app();
        app.world_mut()
            .insert_resource(super::super::ClientInputContext::Shell);
        let mut lobby = lobby_membership();
        let brawler_id = crate::profiles::SavedBrawlerId::new(2).unwrap();
        lobby.profile.brawlers.push(crate::profiles::SavedBrawler {
            id: brawler_id,
            creation_ordinal: 1,
            name: "Replay Brawler".into(),
            fighter_profile_id: crate::profiles::FighterProfileId(1),
            weapon_base_id: crate::profiles::WeaponBaseId(1),
            ultimate_id: crate::builds::UltimateDefinitionId(1),
            passive_ids: [
                crate::builds::PassiveDefinitionId(3),
                crate::builds::PassiveDefinitionId(4),
            ],
            equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
            revision: crate::profiles::ProfileRevision::INITIAL,
        });
        lobby.profile.selected_brawler_id = Some(brawler_id);
        lobby.profile.next_brawler_ordinal = 2;
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
            .generation = 3;
        {
            let mut result = app
                .world_mut()
                .resource_mut::<super::super::ClientMatchResultState>();
            result.last_accepted_game_type_id = Some(game_type_id.clone());
            result.context = Some(super::super::ClientMatchResultContext {
                result: crate::matchplay::MatchResult::Draw,
                local_team: None,
                game_type_id: Some(game_type_id.clone()),
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
            Some(&game_type_id)
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

        *app.world_mut().resource_mut::<GameTypeSelectionDraft>() = GameTypeSelectionDraft {
            selected_index: Some(1),
            unavailable_previous: false,
        };
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::GameTypeSelect);
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
    fn game_type_select_scrolls_long_catalog_and_keeps_confirm_available() {
        let mut app = flow_test_app();
        let mut lobby = lobby_membership();
        let prototype = lobby.game_types[0].clone();
        lobby.game_types = (0..crate::lobby::MAX_GAME_TYPES)
            .map(|index| {
                let mut game = prototype.clone();
                game.id = crate::lobby::GameTypeId::new(format!("test-game-{index}"))
                    .expect("bounded test game ID");
                game.display_name = format!("Test Game {index}");
                game
            })
            .collect();
        app.world_mut().spawn((Client, lobby));
        *app.world_mut().resource_mut::<GameTypeSelectionDraft>() = GameTypeSelectionDraft {
            selected_index: Some(crate::lobby::MAX_GAME_TYPES - 1),
            unavailable_previous: false,
        };
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::GameTypeSelect);
        app.update();

        let (root, confirm_disabled) = {
            let world = app.world_mut();
            let mut roots = world.query_filtered::<Entity, With<GameTypeSelectRoot>>();
            let root = roots.single(world).unwrap();
            let mut buttons = world.query::<(&FlowButton, Has<InteractionDisabled>)>();
            let confirm_disabled = buttons
                .iter(world)
                .find(|(button, _)| button.action == FlowUiAction::ConfirmGameType)
                .map(|(_, disabled)| disabled)
                .expect("Confirm button is rendered");
            (root, confirm_disabled)
        };
        assert!(!confirm_disabled);
        assert!(app.world().get::<ScrollPosition>(root).is_some());

        app.world_mut().write_message(MouseWheel {
            unit: MouseScrollUnit::Line,
            x: 0.0,
            y: -2.0,
            window: Entity::PLACEHOLDER,
            phase: bevy::input::touch::TouchPhase::Moved,
        });
        app.update();
        assert!(
            (app.world().get::<ScrollPosition>(root).unwrap().0.y - 48.0).abs() <= f32::EPSILON
        );
    }

    #[test]
    fn game_type_child_drafts_then_discards_or_confirms() {
        let mut lobby = lobby_membership();
        let first = lobby.game_types[0].clone();
        let mut second = first.clone();
        second.id = crate::lobby::GameTypeId::new("hot-zone-2v2").unwrap();
        second.configuration_revision = 2;
        second.display_name = "Hot Zone 2v2".to_string();
        second.mode_definition_id = crate::map::HOT_ZONE_MODE_DEFINITION;
        lobby.game_types.push(second.clone());
        let mut selection = SelectedGameType {
            catalog_revision: Some(lobby.catalog_revision),
            game_type_id: Some(first.id.clone()),
            configuration_revision: Some(first.configuration_revision),
        };
        let draft = GameTypeSelectionDraft {
            selected_index: Some(1),
            unavailable_previous: false,
        };

        // Merely editing or discarding the draft cannot mutate the accepted selection.
        assert_eq!(selection.game_type_id.as_ref(), Some(&first.id));
        let discarded = GameTypeSelectionDraft::default();
        assert_eq!(discarded.selected_index, None);
        assert_eq!(selection.game_type_id.as_ref(), Some(&first.id));

        assert!(accept_game_type_draft(&draft, &lobby, &mut selection));
        assert_eq!(selection.game_type_id.as_ref(), Some(&second.id));
        assert_eq!(
            selection.configuration_revision,
            Some(second.configuration_revision)
        );
        assert!(!accept_game_type_draft(
            &GameTypeSelectionDraft::default(),
            &lobby,
            &mut selection
        ));
    }

    #[test]
    fn results_disable_replay_when_the_exact_game_disappears() {
        let mut app = flow_test_app();
        app.world_mut().spawn((
            Client,
            lobby_membership(),
            RoutedClientSession {
                generation: 4,
                kind: super::super::RoutedClientSessionKind::Lobby,
            },
        ));
        app.world_mut()
            .resource_mut::<RoutedClientLifecycle>()
            .generation = 4;
        app.world_mut()
            .resource_mut::<super::super::ClientMatchResultState>()
            .context = Some(super::super::ClientMatchResultContext {
            result: crate::matchplay::MatchResult::Draw,
            local_team: None,
            game_type_id: Some(crate::lobby::GameTypeId::new("retired-mode").unwrap()),
            game_name: Some("Retired Mode".to_string()),
            final_score: None,
        });
        app.world_mut()
            .resource_mut::<NextState<ClientFlow>>()
            .set(ClientFlow::Results);
        app.update();

        let (replay_disabled, dashboard_disabled) = {
            let world = app.world_mut();
            let mut query = world.query::<(&FlowButton, Has<InteractionDisabled>)>();
            let replay = query
                .iter(world)
                .find(|(button, _)| button.action == FlowUiAction::QueueAgain)
                .map(|(_, disabled)| disabled)
                .unwrap();
            let dashboard = query
                .iter(world)
                .find(|(button, _)| button.action == FlowUiAction::ReturnToDashboard)
                .map(|(_, disabled)| disabled)
                .unwrap();
            (replay, dashboard)
        };
        assert!(replay_disabled);
        assert!(!dashboard_disabled);
        assert!(
            visible_text(&mut app)
                .iter()
                .any(|text| text.contains("previous game is not available"))
        );
    }

    #[test]
    fn queue_copy_uses_advertised_game_and_saved_brawler_recipe() {
        let builds = crate::builds::BuildCatalog::embedded().unwrap();
        let recipe = crate::builds::BrawlerBuildRecipe {
            weapon: crate::builds::WeaponChoice::Preset(crate::combat::WeaponPresetId(1)),
            ultimate: crate::builds::UltimateDefinitionId(1),
            passives: [
                crate::builds::PassiveDefinitionId(3),
                crate::builds::PassiveDefinitionId(4),
            ],
        };
        let membership = crate::lobby::QueueMembership {
            ticket_id: crate::lobby::QueueTicketId::new(1).unwrap(),
            catalog_revision: crate::lobby::CatalogRevision([1; 32]),
            game_type_id: crate::lobby::GameTypeId::new("wipeout-2v2").unwrap(),
            game_type_configuration_revision: 1,
            brawler_id: crate::profiles::SavedBrawlerId::new(1).unwrap(),
            brawler_revision: crate::profiles::ProfileRevision::INITIAL,
            accepted_build: crate::builds::AcceptedBuildSummary {
                canonical_recipe: recipe,
                identity: crate::builds::SelectedBuild {
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
        assert!(copy.contains("Saved brawler"));
        assert!(copy.contains("Updating queue"));
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
        assert_eq!(error.return_flow, ClientFlow::ServerSelect);
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
        };
        let error = FlowButton {
            index: 1_000,
            action: FlowUiAction::DismissError,
            error_action: true,
        };
        assert!(!overlay_allows_button(&overlay, &underlying));
        assert!(overlay_allows_button(&overlay, &error));
        assert!(!overlay_allows_button(
            &ClientOverlay::DashboardMenu,
            &underlying
        ));
        assert!(overlay_allows_button(&ClientOverlay::DashboardMenu, &error));
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
