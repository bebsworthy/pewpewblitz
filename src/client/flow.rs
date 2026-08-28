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
    input::{ButtonState, keyboard::KeyboardInput, mouse::MouseWheel},
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
use input::{collect_flow_input, edited_value};
#[cfg(test)]
use input::{
    dashboard_focus_neighbor, edited_value_mut, insert_editor_text, overlay_allows_button,
    previous_caret, queue_ui_action, repair_dashboard_focus,
};
pub use model::{
    CancelMatchStartConfirmation, ClientFlow, ClientOverlay, FlowError, FlowErrorAction,
    FlowErrorKind, SelectedGameType, SessionPurpose,
};
use persistence::load_connection_state;
#[cfg(test)]
use persistence::startup_server_address;
pub(super) use persistence::{ClientLocalLoadFailures, ConnectionPersistence, local_load_error};
#[cfg(test)]
use reducer::{accept_game_type_draft, favorite_focus_after_removal, rejection_flow_error};
use reducer::{commit_flow, resolve_flow_action, teardown_session};
#[cfg(test)]
use screens::brawlers::ultimate_name;
#[cfg(test)]
use screens::dashboard::dashboard_game_summary;
#[cfg(test)]
use screens::results::MatchCompletionRoot;
use screens::{
    brawlers::{
        brawler_loadout_summary, keep_brawler_details_focus_visible,
        keep_brawler_list_focus_visible, keep_weapon_equipment_focus_visible,
        open_empty_profile_creation, present_brawler_creation, present_brawler_details,
        present_brawler_editor, present_brawler_list, present_delete_brawler_confirmation,
        present_weapon_equipment, scroll_brawler_details, scroll_brawler_list,
        scroll_weapon_equipment,
    },
    dashboard::{
        apply_dashboard_layout, dashboard_layout_class, keep_dashboard_focus_visible,
        present_dashboard_menu, scroll_dashboard, spawn_dashboard, update_dashboard_live_facts,
    },
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
    use bevy::input::mouse::MouseScrollUnit;

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
