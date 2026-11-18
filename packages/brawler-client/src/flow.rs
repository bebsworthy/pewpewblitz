//! M03 product-flow composition, bounded action arbitration, and recoverable lobby presentation.

use super::{
    ClientSettingsUiSet, RoutedClientLifecycle, connection_persistence::ClientConnectionsPath,
};
use actions::{FlowCommit, PendingFlowActions, begin_flow_frame};
use bevy::{ecs::schedule::ApplyDeferred, prelude::*, ui::UiSystems};
use connection::{ConnectionGeneration, ResolverState, start_initial_connection};
use input::{collect_flow_input, publish_back_root};
use observation::observe_session;
use persistence::load_connection_state;
use reducer::{
    MatchFailureNotice, PendingCreatedBrawler, PendingDeletedBrawler, PendingEditedBrawler,
    commit_flow, resolve_flow_action, teardown_session,
};
use screens::{
    brawlers::{
        BrawlerCreationDraft, BrawlerEditDraft, WeaponEquipmentDraft,
        keep_brawler_details_focus_visible, keep_brawler_list_focus_visible,
        keep_weapon_equipment_focus_visible, open_empty_profile_creation, present_brawler_creation,
        present_brawler_details, present_brawler_editor, present_brawler_list,
        present_weapon_equipment, scroll_brawler_details, scroll_brawler_list,
        scroll_weapon_equipment,
    },
    confirmations::{present_confirmations, scroll_confirmations},
    connecting::{spawn_connecting, update_connecting_copy},
    dashboard::{
        DashboardNotice, DashboardReturnFocus, apply_dashboard_layout,
        keep_dashboard_focus_visible, present_dashboard_menu, scroll_dashboard, spawn_dashboard,
        update_dashboard_live_facts,
    },
    game_select::{
        GameTypeSelectionDraft, keep_game_type_focus_visible, scroll_game_type_select,
        spawn_game_type_select,
    },
    map_maker::{
        MapMakerRuntime, load_map_library, map_maker_input, position_map_editor_selection_toolbar,
        present_map_editor, present_map_library, resolve_map_maker_action,
        return_to_editor_after_practice, sync_map_editor_preview, update_map_editor_cursor_copy,
        update_map_editor_values,
    },
    match_loading::{spawn_match_loading, update_match_loading},
    overlays::{
        present_cancel_confirmation, present_flow_error_overlay, update_rate_limit_try_again,
    },
    practice_maps::{
        PracticeCarousel, animate_practice_maps, practice_map_input, present_practice_maps,
    },
    queue::{spawn_queue, update_queue_cancel_button, update_queue_status},
    results::{clear_results, present_match_completion, scroll_results, spawn_results},
    server_select::{refresh_server_select, spawn_server_select, update_server_select_copy},
    shared::{FlowNavigation, update_flow_button_chrome},
};

mod actions;
#[cfg(feature = "automation")]
mod automation;
mod connection;
mod input;
mod model;
mod observation;
mod persistence;
mod reducer;
mod screens;

#[cfg(feature = "automation")]
pub use automation::ShellCaptureRoute;
#[cfg(feature = "automation")]
pub(super) use automation::install_shell_capture;
pub use model::{
    CancelMatchStartConfirmation, ClientFlow, ClientOverlay, FlowError, FlowErrorAction,
    FlowErrorKind, SelectedGameType, SessionPurpose,
};
pub(super) use persistence::{ClientLocalLoadFailures, ConnectionPersistence};
/// Native render reports count Dashboard-owned entities (automation only).
#[cfg(feature = "automation")]
pub(super) use screens::dashboard::DashboardRoot;
pub(super) use screens::{
    brawlers::{BrawlerCardPreview, BrawlerDetailsPreviewHost},
    dashboard::DashboardPreviewHost,
    map_maker::{MapPalettePreviewHost, MapPaletteVisual},
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
        super::ui::press::add_release_activation(app);
        super::ui::touch_target::add_touch_targets(app);
        super::back::add_back_input(app);
        super::ui::feedback::add_ui_feedback(app);
        app.init_state::<ClientFlow>()
            .init_resource::<ClientOverlay>()
            .init_resource::<PendingFlowActions>()
            .init_resource::<FlowCommit>()
            .init_resource::<ConnectionGeneration>()
            .init_resource::<ResolverState>()
            .init_resource::<ClientConnectionsPath>()
            .init_resource::<super::map_maker::MapDraftStorage>()
            .init_resource::<super::MapDraftLibrary>()
            .init_resource::<MapMakerRuntime>()
            .init_resource::<ClientLocalLoadFailures>()
            .init_resource::<super::ClientQueueModel>()
            .init_resource::<super::ClientPracticeModel>()
            .init_resource::<super::ClientMatchLoadingModel>()
            .init_resource::<brawler::builds::BuildCatalogResource>()
            .init_resource::<brawler::combat::WeaponCatalogResource>()
            .init_resource::<brawler::weapon_parts::WeaponPartCatalogResource>()
            .init_resource::<SelectedGameType>()
            .init_resource::<GameTypeSelectionDraft>()
            .init_resource::<PracticeCarousel>()
            .init_resource::<DashboardReturnFocus>()
            .init_resource::<DashboardNotice>()
            .init_resource::<PendingCreatedBrawler>()
            .init_resource::<PendingEditedBrawler>()
            .init_resource::<PendingDeletedBrawler>()
            .init_resource::<BrawlerCreationDraft>()
            .init_resource::<BrawlerEditDraft>()
            .init_resource::<WeaponEquipmentDraft>()
            .init_resource::<super::ClientProfileModel>()
            .init_resource::<SessionPurpose>()
            .init_resource::<super::ClientMatchResultState>()
            .init_resource::<RoutedClientLifecycle>()
            .init_resource::<MatchFailureNotice>()
            .init_resource::<FlowNavigation>()
            .init_resource::<screens::shared::FlowBurst>()
            .add_systems(
                Startup,
                (
                    load_connection_state,
                    load_map_library,
                    ApplyDeferred,
                    start_initial_connection,
                )
                    .chain(),
            )
            // The root flag reflects this frame's committed screen for the platform Back adapter.
            .add_systems(Last, publish_back_root)
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
                    map_maker_input
                        .after(collect_flow_input)
                        .run_if(
                            in_state(ClientFlow::MapLibrary)
                                .or_else(in_state(ClientFlow::MapEditor)),
                        )
                        .in_set(ClientFlowSet::CollectFlowInput),
                    practice_map_input
                        .after(collect_flow_input)
                        .in_set(ClientFlowSet::CollectFlowInput),
                    resolve_map_maker_action
                        .before(resolve_flow_action)
                        .in_set(ClientFlowSet::ResolveFlowAction),
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
                    scroll_confirmations,
                    scroll_results,
                    present_match_completion,
                    spawn_results.run_if(in_state(ClientFlow::Results)),
                    spawn_match_loading.before(update_match_loading),
                    update_match_loading,
                    screens::rejoin_status::present_rejoin_status,
                    update_connecting_copy,
                    update_rate_limit_try_again.after(present_flow_error_overlay),
                    update_queue_cancel_button,
                    update_queue_status,
                    spawn_game_type_select.before(update_flow_button_chrome),
                    present_map_library.run_if(in_state(ClientFlow::MapLibrary)),
                    (
                        present_map_editor.run_if(in_state(ClientFlow::MapEditor)),
                        sync_map_editor_preview,
                        update_map_editor_cursor_copy.after(present_map_editor),
                        position_map_editor_selection_toolbar
                            .after(present_map_editor)
                            .run_if(in_state(ClientFlow::MapEditor)),
                        update_map_editor_values
                            .after(present_map_editor)
                            .after(update_map_editor_cursor_copy)
                            .after(position_map_editor_selection_toolbar)
                            .run_if(in_state(ClientFlow::MapEditor)),
                    ),
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
                    present_confirmations,
                    scroll_game_type_select,
                )
                    .in_set(ClientFlowSet::PresentFlow)
                    .before(update_flow_button_chrome),
            )
            .add_systems(OnEnter(ClientFlow::ServerSelect), spawn_server_select)
            .add_systems(OnEnter(ClientFlow::Connecting), spawn_connecting)
            .add_systems(
                OnEnter(ClientFlow::Dashboard),
                (
                    return_to_editor_after_practice,
                    spawn_dashboard,
                    open_empty_profile_creation,
                )
                    .chain(),
            )
            .add_systems(OnEnter(ClientFlow::GameTypeSelect), spawn_game_type_select)
            .add_systems(OnEnter(ClientFlow::Match), enter_match_input)
            .add_systems(OnEnter(ClientFlow::Results), spawn_results)
            .add_systems(OnExit(ClientFlow::Results), clear_results)
            .add_systems(OnExit(ClientFlow::Match), exit_match_input)
            .add_systems(OnEnter(ClientFlow::Queue), spawn_queue)
            .add_systems(OnEnter(ClientFlow::MatchLoading), spawn_match_loading);
        app.add_systems(
            Update,
            (present_practice_maps, animate_practice_maps)
                .chain()
                .in_set(ClientFlowSet::PresentFlow),
        );
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
        app.add_systems(Update, begin_surface_entrance);
        app.add_systems(
            Update,
            screens::shared::apply_flow_burst.after(ClientFlowSet::PresentFlow),
        );
    }
}

/// Opens the shared surface entrance when the visible destination changes. Rebuilds of an
/// unchanged screen keep their surfaces still; presentation never gates the navigation itself.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn begin_surface_entrance(
    flow: Res<State<ClientFlow>>,
    overlay: Res<ClientOverlay>,
    time: Res<Time>,
    entrance: Option<ResMut<super::ui::surface::SurfaceEntrance>>,
    mut shown: Local<Option<(ClientFlow, ClientOverlay)>>,
) {
    let current = (*flow.get(), overlay.clone());
    if shown.as_ref() == Some(&current) {
        return;
    }
    *shown = Some(current);
    if let Some(mut entrance) = entrance {
        entrance.begin(time.elapsed_secs());
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

pub(crate) use screens::equipment::WeaponPreviewHost;
