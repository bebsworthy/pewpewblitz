//! Deferred flow commit and session teardown ownership.

use crate::{
    ClientLobbyFailure, ClientLobbyMembership, ClientNetworkConfig, RoutedClientLifecycle,
    RoutedClientPhase, RoutedClientSession, RuntimeLobbyTarget,
    connection_persistence::{ClientConnectionsPath, save_connections},
    flow::{
        actions::{
            FlowCommit, FlowUiAction, OverlayCommit, PendingFlowActions, SessionObservation,
        },
        connection::{
            ConnectionGeneration, ConnectionStage, PendingConnection, ResolverState,
            begin_connection_target, has_next_candidate, spawn_current_candidate, validate_target,
        },
        model::{
            ClientFlow, ClientOverlay, FlowError, FlowErrorAction, FlowErrorKind, SelectedGameType,
            SessionPurpose,
        },
        persistence::{ClientLocalLoadFailures, ConnectionPersistence, local_load_error},
        screens::{
            brawlers::{BrawlerCreationDraft, BrawlerEditDraft, WeaponEquipmentDraft},
            dashboard::{
                DASHBOARD_MENU_INDEX, DASHBOARD_PLAY_INDEX, DashboardNotice, DashboardReturnFocus,
            },
            game_select::GameTypeSelectionDraft,
            server_select::{EditingField, ServerSelectModel},
            shared::FlowNavigation,
        },
    },
};
use bevy::prelude::*;
use lightyear::prelude::client::Client;
use lightyear::prelude::{Unlink, UnlinkReason, client::Disconnect};

mod equipment;
mod match_flow;
mod profile;

#[cfg(test)]
pub(in crate::flow) use match_flow::MATCH_CONNECT_FAILED;
pub(in crate::flow) use match_flow::MatchFailureNotice;
#[allow(
    unused_imports,
    reason = "the reducer facade preserves the focused test and composition path"
)]
pub(super) use match_flow::accept_game_type_draft;
pub(in crate::flow) use profile::{
    PendingCreatedBrawler, PendingDeletedBrawler, PendingEditedBrawler,
};
use profile::{resolve_profile_action, resolve_profile_decision};

type LobbyMembershipQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static ClientLobbyMembership,
        Option<&'static RuntimeLobbyTarget>,
        &'static RoutedClientSession,
    ),
    With<Client>,
>;

fn resolve_explicit_action(
    action: &FlowUiAction,
    commit: &mut FlowCommit,
    purpose: &mut SessionPurpose,
    selection: &mut SelectedGameType,
    game_draft: &mut GameTypeSelectionDraft,
    dashboard_notice: &mut DashboardNotice,
    result_state: &mut crate::ClientMatchResultState,
) {
    if matches!(
        action,
        FlowUiAction::Cancel | FlowUiAction::Disconnect | FlowUiAction::ConfirmChangeServer
    ) {
        commit.teardown = true;
        commit.overlay = Some(OverlayCommit::Clear);
        commit.next_flow = Some(ClientFlow::ServerSelect);
        if *purpose == SessionPurpose::Practice {
            *purpose = SessionPurpose::Multiplayer;
        }
        *selection = SelectedGameType::default();
        *game_draft = GameTypeSelectionDraft::default();
        dashboard_notice.0 = None;
        result_state.context = None;
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_session_observation(
    observation: SessionObservation,
    flow: ClientFlow,
    membership: &LobbyMembershipQuery,
    pending: &mut Option<ResMut<PendingConnection>>,
    path: &ClientConnectionsPath,
    model: &mut ServerSelectModel,
    persistence: &mut ConnectionPersistence,
    selection: &mut SelectedGameType,
    game_draft: &mut GameTypeSelectionDraft,
    dashboard_focus: &mut DashboardReturnFocus,
    dashboard_notice: &mut DashboardNotice,
    queue: &mut crate::ClientQueueModel,
    result_state: &mut crate::ClientMatchResultState,
    routed: &mut RoutedClientLifecycle,
    match_failure: &mut MatchFailureNotice,
    purpose: &mut SessionPurpose,
    local_failures: ClientLocalLoadFailures,
    commit: &mut FlowCommit,
) {
    if matches!(observation, SessionObservation::LobbyReturnFailed) {
        // BRL-0199: the fresh lobby after a match failed while the match was still presented.
        // Leave the dead match screen for the recoverable connection error (Retry reconnects
        // to the same server).
        let message = if core::mem::take(&mut match_failure.0) {
            MATCH_LOST_LOBBY_UNREACHABLE
        } else {
            "Could not reconnect to the lobby after the match. Check your network, then retry."
        };
        resolve_connection_observation(
            SessionObservation::UnexpectedLoss,
            membership,
            pending,
            path,
            model,
            persistence,
            selection,
            game_draft,
            dashboard_focus,
            dashboard_notice,
            result_state,
            local_failures,
            commit,
        );
        if let Some(error) = commit.error.as_mut() {
            error.message = message.to_string();
        }
        return;
    }
    match observation {
        observation @ (SessionObservation::Accepted
        | SessionObservation::ResolverCompleted { .. }
        | SessionObservation::CandidateFailed
        | SessionObservation::CandidateTimedOut
        | SessionObservation::DnsTimedOut
        | SessionObservation::UnexpectedLoss
        | SessionObservation::TimedOut
        | SessionObservation::Rejected(_)) => resolve_connection_observation(
            observation,
            membership,
            pending,
            path,
            model,
            persistence,
            selection,
            game_draft,
            dashboard_focus,
            dashboard_notice,
            result_state,
            local_failures,
            commit,
        ),
        observation => match_flow::resolve_observation(
            observation,
            flow,
            membership,
            selection,
            dashboard_focus,
            queue,
            result_state,
            routed,
            match_failure,
            purpose,
            commit,
        ),
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_connection_observation(
    observation: SessionObservation,
    membership: &LobbyMembershipQuery,
    pending: &mut Option<ResMut<PendingConnection>>,
    path: &ClientConnectionsPath,
    model: &ServerSelectModel,
    persistence: &mut ConnectionPersistence,
    selection: &mut SelectedGameType,
    game_draft: &mut GameTypeSelectionDraft,
    dashboard_focus: &mut DashboardReturnFocus,
    dashboard_notice: &mut DashboardNotice,
    result_state: &mut crate::ClientMatchResultState,
    local_failures: ClientLocalLoadFailures,
    commit: &mut FlowCommit,
) {
    match observation {
        SessionObservation::Accepted => {
            // A new accepted session retires prior connection errors. Any new
            // persistence/content error below still takes precedence in commit_flow.
            commit.overlay = Some(OverlayCommit::Clear);
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
                    && let Some(mut error) = local_load_error(local_failures)
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
                        commit,
                        "Address resolution returned no usable addresses".to_string(),
                        true,
                    ),
                    Err(error) => fail_to_server_select(commit, error, true),
                }
            }
        }
        SessionObservation::CandidateFailed | SessionObservation::CandidateTimedOut => {
            commit.teardown = true;
            if pending
                .as_ref()
                .is_some_and(|pending| has_next_candidate(pending))
            {
                commit.advance_candidate = true;
            } else {
                fail_to_server_select(commit, no_reply_message(&observation), true);
            }
        }
        SessionObservation::DnsTimedOut => {
            commit.teardown = true;
            fail_to_server_select(commit, "Address resolution timed out".to_string(), true);
        }
        SessionObservation::UnexpectedLoss => {
            commit.teardown = true;
            *selection = SelectedGameType::default();
            *game_draft = GameTypeSelectionDraft::default();
            dashboard_notice.0 = None;
            result_state.context = None;
            fail_to_server_select(
                commit,
                "The lobby connection was lost unexpectedly".to_string(),
                true,
            );
        }
        SessionObservation::TimedOut => {
            commit.teardown = true;
            fail_to_server_select(commit, no_reply_message(&observation), true);
        }
        SessionObservation::Rejected(reason) => {
            commit.teardown = true;
            commit.next_flow = Some(ClientFlow::ServerSelect);
            commit.error = Some(rejection_flow_error(reason));
        }
        _ => unreachable!("session observation was routed to the wrong reducer"),
    }
}

fn remove_favorite(
    address: &str,
    path: &ClientConnectionsPath,
    persistence: &mut ConnectionPersistence,
    commit: &mut FlowCommit,
) {
    let removed_index = persistence
        .state
        .favorites
        .iter()
        .position(|favorite| favorite.address == address);
    if persistence.state.remove_favorite(address) {
        if let Err(error) = save_connections(&path.0, &persistence.state) {
            persistence.dirty_error = Some(error);
        }
        commit.refresh_server_select = Some(favorite_focus_after_removal(
            removed_index,
            persistence.state.favorites.len(),
        ));
    }
}

fn forget_server(
    address: &str,
    path: &ClientConnectionsPath,
    persistence: &mut ConnectionPersistence,
    commit: &mut FlowCommit,
) {
    if persistence.state.forget_server(address) {
        if let Err(error) = save_connections(&path.0, &persistence.state) {
            persistence.dirty_error = Some(error);
        }
        // Focus returns to Connect; the forgotten row and its neighbours are rebuilt.
        commit.refresh_server_select = Some(2);
    }
}

fn retry_connection_persistence(
    path: &ClientConnectionsPath,
    persistence: &mut ConnectionPersistence,
    commit: &mut FlowCommit,
) {
    match save_connections(&path.0, &persistence.state) {
        Ok(()) => {
            persistence.dirty_error = None;
            commit.overlay = Some(OverlayCommit::Clear);
        }
        Err(error) => persistence.dirty_error = Some(error),
    }
}

fn toggle_favorite_server(
    membership: &LobbyMembershipQuery,
    path: &ClientConnectionsPath,
    persistence: &mut ConnectionPersistence,
    commit: &mut FlowCommit,
) {
    let Some((membership, Some(target), _)) = membership.iter().next() else {
        return;
    };
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

fn begin_server_field_edit(
    model: &mut ServerSelectModel,
    field: EditingField,
    overlay: &ClientOverlay,
    commit: &mut FlowCommit,
) {
    model.editing = Some(field);
    model.caret = match field {
        EditingField::Address => model.address.len(),
        EditingField::Name => model.name.len(),
    };
    if field == EditingField::Name && matches!(overlay, ClientOverlay::Error(_)) {
        commit.overlay = Some(OverlayCommit::Clear);
        commit.refresh_server_select = Some(1);
    }
}

fn start_entered_connection(model: &mut ServerSelectModel, commit: &mut FlowCommit) {
    match validate_target(&model.address, &model.name) {
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
    }
}

fn start_saved_connection(address: &str, model: &mut ServerSelectModel, commit: &mut FlowCommit) {
    match validate_target(address, &model.name) {
        Ok(target) => {
            model.address = target.logical_address.canonical().to_string();
            commit.start_target = Some(target);
            commit.next_flow = Some(ClientFlow::Connecting);
            commit.overlay = Some(OverlayCommit::Clear);
        }
        Err(error) => model.inline_error = Some(error),
    }
}

fn retry_connection(model: &mut ServerSelectModel, commit: &mut FlowCommit) {
    match validate_target(&model.address, &model.name) {
        Ok(target) => {
            commit.start_target = Some(target);
            commit.next_flow = Some(ClientFlow::Connecting);
            commit.overlay = Some(OverlayCommit::Clear);
        }
        Err(error) => model.inline_error = Some(error),
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "one bounded coordinator makes flow-action precedence and commits explicit"
)]
pub(super) fn resolve_flow_action(
    time: Res<Time<Real>>,
    flow: Res<State<ClientFlow>>,
    mut actions: ResMut<PendingFlowActions>,
    mut commit: ResMut<FlowCommit>,
    mut model: ResMut<ServerSelectModel>,
    mut persistence: ResMut<ConnectionPersistence>,
    mut pending: Option<ResMut<PendingConnection>>,
    membership: LobbyMembershipQuery,
    path: Res<ClientConnectionsPath>,
    overlay: Res<ClientOverlay>,
    dashboard: (
        ResMut<SelectedGameType>,
        ResMut<GameTypeSelectionDraft>,
        ResMut<DashboardReturnFocus>,
        ResMut<DashboardNotice>,
        ResMut<PendingCreatedBrawler>,
        ResMut<PendingEditedBrawler>,
        ResMut<PendingDeletedBrawler>,
        ResMut<BrawlerCreationDraft>,
        ResMut<BrawlerEditDraft>,
        ResMut<WeaponEquipmentDraft>,
    ),
    models: (
        ResMut<crate::ClientQueueModel>,
        ResMut<crate::ClientPracticeModel>,
        ResMut<crate::ClientMatchLoadingModel>,
        ResMut<crate::ClientMatchResultState>,
        ResMut<crate::ClientProfileModel>,
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
        mut pending_deleted_brawler,
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
    resolve_profile_decision(
        &mut profile,
        &mut dashboard_notice,
        &mut pending_created_brawler,
        &mut pending_edited_brawler,
        &mut pending_deleted_brawler,
        &mut creation_draft,
        &mut brawler_edit,
        &mut equipment_draft,
        &mut commit,
    );
    if let Some(explicit) = actions.explicit.take() {
        resolve_explicit_action(
            &explicit,
            &mut commit,
            &mut purpose,
            &mut selection,
            &mut game_draft,
            &mut dashboard_notice,
            &mut result_state,
        );
        return;
    }
    if let Some(observation) = actions.session.take() {
        // Session facts are rare (one per lifecycle step), so each is logged with the screen it
        // was reduced on; with the flow-change record in `commit_flow` this explains every
        // navigation on a device without a debugger (BRL-k0tr).
        info!(flow = ?flow.get(), ?observation, "brawler client flow observed a session fact");
        if matches!(
            observation,
            SessionObservation::UnexpectedLoss
                | SessionObservation::LobbyReturnFailed
                | SessionObservation::TimedOut
                | SessionObservation::Rejected(_)
        ) {
            *equipment_draft = WeaponEquipmentDraft::default();
        }
        resolve_session_observation(
            observation,
            *flow.get(),
            &membership,
            &mut pending,
            &path,
            &mut model,
            &mut persistence,
            &mut selection,
            &mut game_draft,
            &mut dashboard_focus,
            &mut dashboard_notice,
            &mut queue,
            &mut result_state,
            &mut routed,
            &mut match_failure,
            &mut purpose,
            *local_failures,
            &mut commit,
        );
        return;
    }
    let Some(action) = actions.ordinary.take() else {
        return;
    };
    match action {
        FlowUiAction::EditAddress => {
            begin_server_field_edit(
                &mut model,
                EditingField::Address,
                overlay.as_ref(),
                &mut commit,
            );
        }
        FlowUiAction::EditName => {
            begin_server_field_edit(
                &mut model,
                EditingField::Name,
                overlay.as_ref(),
                &mut commit,
            );
        }
        FlowUiAction::Connect => start_entered_connection(&mut model, &mut commit),
        FlowUiAction::JoinSaved(address) => {
            start_saved_connection(&address, &mut model, &mut commit);
        }
        FlowUiAction::RemoveFavorite(address) => {
            remove_favorite(&address, &path, &mut persistence, &mut commit);
        }
        FlowUiAction::ForgetServer(address) => {
            forget_server(&address, &path, &mut persistence, &mut commit);
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
        FlowUiAction::CloseDashboardMenu | FlowUiAction::KeepServer => {
            commit.overlay = Some(OverlayCommit::Clear);
            commit.focus_index = Some(DASHBOARD_MENU_INDEX);
        }
        FlowUiAction::RequestChangeServer => {
            commit.overlay = Some(OverlayCommit::ChangeServerConfirmation);
            commit.focus_index = Some(0);
        }
        FlowUiAction::Retry => retry_connection(&mut model, &mut commit),
        FlowUiAction::RetrySave => {
            retry_connection_persistence(&path, &mut persistence, &mut commit);
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
        FlowUiAction::ToggleFavoriteServer => {
            toggle_favorite_server(&membership, &path, &mut persistence, &mut commit);
        }
        action @ (FlowUiAction::OpenBrawlerList
        | FlowUiAction::BackToBrawlerList
        | FlowUiAction::CloseBrawlerList
        | FlowUiAction::OpenBrawlerDetails(_)
        | FlowUiAction::CreateBrawler
        | FlowUiAction::OpenCreationFighter
        | FlowUiAction::OpenCreationWeapon
        | FlowUiAction::ChooseCreationFighter(_)
        | FlowUiAction::ChooseCreationWeapon(_)
        | FlowUiAction::ChooseCreationUltimate(_)
        | FlowUiAction::EditCreationName
        | FlowUiAction::OpenCreationUltimate
        | FlowUiAction::CancelCreateBrawler
        | FlowUiAction::ConfirmCreateBrawler
        | FlowUiAction::CancelBrawlerEdit
        | FlowUiAction::CancelDeleteBrawler
        | FlowUiAction::SelectBrawler(_)
        | FlowUiAction::OpenBrawlerEditor(_)
        | FlowUiAction::BeginBrawlerNameEdit
        | FlowUiAction::OpenBrawlerUltimate
        | FlowUiAction::OpenBrawlerPassive(_)
        | FlowUiAction::ChooseBrawlerUltimate(_)
        | FlowUiAction::ChooseBrawlerPassive(_)
        | FlowUiAction::ConfirmBrawlerEdit
        | FlowUiAction::DeleteBrawler(_)
        | FlowUiAction::ConfirmDeleteBrawler) => resolve_profile_action(
            &action,
            overlay.as_ref(),
            &queue,
            &practice,
            &mut profile,
            &mut creation_draft,
            &mut brawler_edit,
            &mut pending_created_brawler,
            &mut pending_edited_brawler,
            &mut pending_deleted_brawler,
            &mut dashboard_notice,
            &mut commit,
        ),
        action @ (FlowUiAction::OpenWeaponEquipment(_)
        | FlowUiAction::SelectEquipmentSlot(_)
        | FlowUiAction::EquipWeaponPart(_)
        | FlowUiAction::UnequipWeaponPart
        | FlowUiAction::CancelWeaponEquipment) => {
            equipment::resolve_equipment_action(
                &action,
                &queue,
                &practice,
                &mut profile,
                &mut equipment_draft,
                &mut commit,
            );
        }
        action @ (FlowUiAction::SelectGameTypeDraft(_)
        | FlowUiAction::ConfirmGameType
        | FlowUiAction::CancelGameType
        | FlowUiAction::JoinQueue
        | FlowUiAction::OpenPracticeMaps
        | FlowUiAction::ChoosePracticeMap(_)
        | FlowUiAction::CancelPracticeMaps
        | FlowUiAction::StartPractice
        | FlowUiAction::QueueAgain
        | FlowUiAction::OpenGameTypeSelect
        | FlowUiAction::ReturnToDashboard
        | FlowUiAction::KeepPlaying
        | FlowUiAction::KeepLoading
        | FlowUiAction::ConfirmLeaveMatch
        | FlowUiAction::CancelQueue
        | FlowUiAction::RetryQueue
        | FlowUiAction::TryAgainQueue
        | FlowUiAction::RequestCancelMatchStart
        | FlowUiAction::ConfirmCancelMatchStart) => match_flow::resolve_action(
            &action,
            time.elapsed(),
            *flow.get(),
            &membership,
            &mut selection,
            &mut game_draft,
            &mut dashboard_focus,
            &mut queue,
            &mut practice,
            &mut loading,
            &mut result_state,
            &mut routed,
            &mut purpose,
            &mut commit,
        ),
        FlowUiAction::Cancel
        | FlowUiAction::Disconnect
        | FlowUiAction::ConfirmChangeServer
        | FlowUiAction::OpenMapMaker
        | FlowUiAction::CreateMap
        | FlowUiAction::ConfirmCreateMap
        | FlowUiAction::CancelCreateMap
        | FlowUiAction::OpenMapDraft(_)
        | FlowUiAction::SelectMapDraft(_)
        | FlowUiAction::CancelDeleteMapDraft
        | FlowUiAction::MapMakerShowPaletteItem(_)
        | FlowUiAction::MapMakerToggleCategoryMenu
        | FlowUiAction::BeginRenameMapDraft(_)
        | FlowUiAction::ConfirmRenameMapDraft
        | FlowUiAction::CancelRenameMapDraft
        | FlowUiAction::DuplicateMapDraft(_)
        | FlowUiAction::DeleteMapDraft(_)
        | FlowUiAction::PublishMapDraft(_)
        | FlowUiAction::CloseMapMaker
        | FlowUiAction::BackToMapLibrary
        | FlowUiAction::SaveMapDraft
        | FlowUiAction::TestAuthoredMap
        | FlowUiAction::MapMakerUndo
        | FlowUiAction::MapMakerRedo
        | FlowUiAction::MapMakerClear
        | FlowUiAction::MapMakerCycleAsset(_)
        | FlowUiAction::MapMakerSelectAsset(_)
        | FlowUiAction::MapMakerCycleSymmetry
        | FlowUiAction::MapMakerRotate
        | FlowUiAction::MapMakerPlace
        | FlowUiAction::MapMakerPaintStroke(_)
        | FlowUiAction::MapMakerErase
        | FlowUiAction::MapMakerEraseStroke(_)
        | FlowUiAction::MapMakerUsePlaceTool
        | FlowUiAction::MapMakerUseEraseTool
        | FlowUiAction::MapMakerUseMoveTool
        | FlowUiAction::MapMakerSelectRect(_, _, _)
        | FlowUiAction::MapMakerMoveSelection(_, _)
        | FlowUiAction::MapMakerDuplicateSelection
        | FlowUiAction::MapMakerDeleteSelection
        | FlowUiAction::MapMakerClearSelection
        | FlowUiAction::MapMakerSelectCategory(_)
        | FlowUiAction::MapMakerToggleSearch
        | FlowUiAction::MapMakerClearSearch
        | FlowUiAction::MapMakerToggleAssets
        | FlowUiAction::MapMakerToggleValidation
        | FlowUiAction::MapMakerToggleGrid
        | FlowUiAction::MapMakerFitView
        | FlowUiAction::MapMakerShowRuleIssue
        | FlowUiAction::MapMakerToggleChromeFocus => {}
    }
    let _ = flow;
}

/// Shown when a match session was lost (network gap beyond the routed session bound, app
/// suspension, or a worker failure) and the fresh lobby session after it also failed.
const MATCH_LOST_LOBBY_UNREACHABLE: &str = "The connection to the match was lost and the lobby could not be reached. Check your network, then retry.";

/// Copy for a failed connection attempt that received no reply. A network that blocks UDP looks
/// exactly like this, so the player gets the actionable hint without a diagnosis we cannot make.
const NO_REPLY_HINT: &str = "Check the address and that the server is running. Some networks block the UDP traffic the game uses; if so, try another network.";

fn no_reply_message(observation: &SessionObservation) -> String {
    let headline = match observation {
        SessionObservation::CandidateFailed => "Could not contact the server",
        SessionObservation::CandidateTimedOut => "The server did not answer in time",
        _ => "The connection attempt timed out",
    };
    format!("{headline}. {NO_REPLY_HINT}")
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

pub(super) fn rejection_flow_error(reason: ClientLobbyFailure) -> FlowError {
    let (message, actions) = match reason {
        ClientLobbyFailure::Rejected(brawler::protocol::LobbyJoinRejection::InvalidName) => (
            "The server rejected the proposed display name".to_string(),
            [Some(FlowErrorAction::EditName), Some(FlowErrorAction::Back)],
        ),
        ClientLobbyFailure::Rejected(brawler::protocol::LobbyJoinRejection::ServerFull) => (
            "The server lobby is full".to_string(),
            [
                Some(FlowErrorAction::RetryConnection),
                Some(FlowErrorAction::Back),
            ],
        ),
        ClientLobbyFailure::Rejected(brawler::protocol::LobbyJoinRejection::AccountInUse) => (
            "This account is already connected from another running copy of the game. Close it, \
             then retry."
                .to_string(),
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

pub(super) fn favorite_focus_after_removal(
    removed_index: Option<usize>,
    remaining: usize,
) -> usize {
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

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(super) fn teardown_session(
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
pub(super) fn commit_flow(
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
    burst: Option<ResMut<super::screens::shared::FlowBurst>>,
    current: Res<State<ClientFlow>>,
) {
    if let Some(error) = &commit.error {
        *overlay = ClientOverlay::Error(error.clone());
    } else if let Some(overlay_commit) = commit.overlay {
        *overlay = match overlay_commit {
            OverlayCommit::Clear => ClientOverlay::None,
            OverlayCommit::Settings => ClientOverlay::Settings,
            OverlayCommit::Credits => ClientOverlay::Credits,
            OverlayCommit::DashboardMenu => ClientOverlay::DashboardMenu,
            OverlayCommit::BrawlerList => ClientOverlay::BrawlerList,
            OverlayCommit::BrawlerDetails(value) => ClientOverlay::BrawlerDetails(value),
            OverlayCommit::BrawlerCreation => ClientOverlay::BrawlerCreation,
            OverlayCommit::BrawlerEditor => ClientOverlay::BrawlerEditor,
            OverlayCommit::WeaponEquipment => ClientOverlay::WeaponEquipment,
            OverlayCommit::DeleteBrawlerConfirmation(value) => {
                ClientOverlay::DeleteBrawlerConfirmation(value)
            }
            OverlayCommit::Confirmation(value) => ClientOverlay::Confirmation(value),
            OverlayCommit::ChangeServerConfirmation => ClientOverlay::ChangeServerConfirmation,
        };
    }
    if let Some(index) = commit.focus_index {
        navigation.focused = index;
        if commit.celebrate
            && commit.error.is_none()
            && let Some(mut burst) = burst
        {
            burst.request(index);
        }
    }
    if let Some(target) = commit.start_target.clone()
        && let Err(error) = begin_connection_target(
            &mut commands,
            &config,
            time.elapsed(),
            &mut generation,
            &mut resolver,
            &mut routed,
            target,
        )
    {
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
        return;
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
        if flow != *current.get() {
            info!(from = ?current.get(), to = ?flow, "brawler client flow changed");
        }
        next_flow.set(flow);
        if flow != ClientFlow::Connecting {
            commands.remove_resource::<PendingConnection>();
        }
    }
}
