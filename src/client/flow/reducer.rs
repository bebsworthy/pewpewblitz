//! Deferred flow commit and session teardown ownership.

use crate::client::{
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
            CancelMatchStartConfirmation, ClientFlow, ClientOverlay, FlowError, FlowErrorAction,
            FlowErrorKind, SelectedGameType, SessionPurpose,
        },
        persistence::{ClientLocalLoadFailures, ConnectionPersistence, local_load_error},
        screens::{
            brawlers::{BrawlerCreationDraft, BrawlerEditDraft, WeaponEquipmentDraft},
            dashboard::{
                DASHBOARD_BUILD_INDEX, DASHBOARD_GAME_INDEX, DASHBOARD_PLAY_INDEX, DashboardNotice,
                DashboardReturnFocus,
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

#[derive(Resource, Default)]
pub(in crate::client::flow) struct PendingCreatedBrawler(pub(in crate::client::flow) Option<u64>);

#[derive(Resource, Default)]
pub(in crate::client::flow) struct PendingEditedBrawler(
    pub(in crate::client::flow) Option<crate::profiles::SavedBrawlerId>,
);

#[derive(Resource, Default)]
pub(in crate::client::flow) struct MatchFailureNotice(pub(in crate::client::flow) bool);

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
        ResMut<crate::client::ClientQueueModel>,
        ResMut<crate::client::ClientPracticeModel>,
        ResMut<crate::client::ClientMatchLoadingModel>,
        ResMut<crate::client::ClientMatchResultState>,
        ResMut<crate::client::ClientProfileModel>,
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
            crate::profiles::ProfileDecision::IncompatibleBuild => {
                "Choose only one elemental resistance passive for this brawler.".to_string()
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
                    session.kind == crate::client::RoutedClientSessionKind::Lobby
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
                session.kind == crate::client::RoutedClientSessionKind::Lobby
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

pub(super) fn accept_game_type_draft(
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
            "The selected brawler no longer matches the server profile. Review and select it again."
        }
        crate::lobby::PracticeStartRejection::IncompatibleBuild => {
            "This brawler has incompatible choices. Edit it and choose only one elemental resistance passive."
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

pub(super) fn rejection_flow_error(reason: ClientLobbyFailure) -> FlowError {
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
        navigation.selected = index;
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
        next_flow.set(flow);
        if flow != ClientFlow::Connecting {
            commands.remove_resource::<PendingConnection>();
        }
    }
}
