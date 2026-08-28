//! M03 product-flow composition, bounded action arbitration, and recoverable lobby presentation.

use super::{
    ClientSettingsUiSet, RoutedClientLifecycle, connection_persistence::ClientConnectionsPath,
};
use actions::{FlowCommit, PendingFlowActions, begin_flow_frame};
use bevy::{ecs::schedule::ApplyDeferred, prelude::*, ui::UiSystems};
use connection::{ConnectionGeneration, ResolverState, start_initial_connection};
use input::collect_flow_input;
use observation::observe_session;
use persistence::load_connection_state;
use reducer::{
    MatchFailureNotice, PendingCreatedBrawler, PendingEditedBrawler, commit_flow,
    resolve_flow_action, teardown_session,
};
use screens::{
    brawlers::{
        BrawlerCreationDraft, BrawlerEditDraft, WeaponEquipmentDraft,
        keep_brawler_details_focus_visible, keep_brawler_list_focus_visible,
        keep_weapon_equipment_focus_visible, open_empty_profile_creation, present_brawler_creation,
        present_brawler_details, present_brawler_editor, present_brawler_list,
        present_delete_brawler_confirmation, present_weapon_equipment, scroll_brawler_details,
        scroll_brawler_list, scroll_weapon_equipment,
    },
    connecting::{spawn_connecting, update_connecting_copy},
    dashboard::{
        DashboardNotice, DashboardReturnFocus, apply_dashboard_layout,
        keep_dashboard_focus_visible, present_dashboard_menu, scroll_dashboard, spawn_dashboard,
        update_dashboard_live_facts,
    },
    game_select::{
        GameTypeSelectionDraft, keep_game_type_focus_visible, scroll_game_type_select,
        spawn_game_type_select, update_game_population,
    },
    match_loading::{spawn_match_loading, update_match_loading},
    overlays::{
        present_cancel_confirmation, present_change_server_confirmation,
        present_flow_error_overlay, present_leave_confirmation, update_rate_limit_try_again,
    },
    queue::{spawn_queue, update_queue_cancel_button, update_queue_status},
    results::{clear_results, present_match_completion, spawn_results},
    server_select::{refresh_server_select, spawn_server_select, update_server_select_copy},
    shared::{FlowNavigation, update_flow_button_chrome},
};

mod actions;
mod connection;
mod input;
mod model;
mod observation;
mod persistence;
mod reducer;
mod screens;

pub use model::{
    CancelMatchStartConfirmation, ClientFlow, ClientOverlay, FlowError, FlowErrorAction,
    FlowErrorKind, SelectedGameType, SessionPurpose,
};
pub(super) use persistence::{ClientLocalLoadFailures, ConnectionPersistence};
pub(super) use screens::{
    brawlers::BrawlerDetailsPreviewHost,
    dashboard::{DashboardPreviewHost, DashboardRoot},
};

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
                ),
            )
            .add_systems(
                Update,
                (
                    refresh_server_select,
                    update_server_select_copy,
                    present_flow_error_overlay,
                    present_cancel_confirmation,
                    present_leave_confirmation,
                    present_change_server_confirmation,
                    present_match_completion,
                    update_match_loading,
                    update_connecting_copy,
                    update_rate_limit_try_again.after(present_flow_error_overlay),
                    update_queue_cancel_button,
                    update_queue_status,
                    update_game_population,
                    update_flow_button_chrome,
                )
                    .in_set(ClientFlowSet::PresentFlow),
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
                    .before(update_flow_button_chrome),
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
            .add_systems(OnExit(ClientFlow::Match), exit_match_input)
            .add_systems(OnEnter(ClientFlow::Queue), spawn_queue)
            .add_systems(OnEnter(ClientFlow::MatchLoading), spawn_match_loading);
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
    }
}

fn enter_match_input(mut context: ResMut<super::ClientInputContext>) {
    *context = super::ClientInputContext::Gameplay;
}

fn exit_match_input(mut context: ResMut<super::ClientInputContext>) {
    *context = super::ClientInputContext::Shell;
}

#[cfg(test)]
mod tests;
