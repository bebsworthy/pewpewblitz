//! Match selection, matchmaking/loading, and Results reduction.

use super::{LobbyMembershipQuery, fail_to_server_select_with_kind};
use crate::{
    ClientLobbyMembership, RoutedClientLifecycle,
    flow::{
        actions::{FlowCommit, FlowUiAction, OverlayCommit, SessionObservation},
        model::{
            CancelMatchStartConfirmation, ClientFlow, FlowError, FlowErrorAction, FlowErrorKind,
            SelectedGameType, SessionPurpose,
        },
        screens::{
            dashboard::{
                DASHBOARD_GAME_INDEX, DASHBOARD_PLAY_INDEX, DASHBOARD_PRACTICE_INDEX,
                DashboardReturnFocus,
            },
            game_select::GameTypeSelectionDraft,
        },
    },
};
use bevy::prelude::Resource;

/// Dashboard explanation after the match session failed during Match Loading (BRL-hjmq).
pub(in crate::flow) const MATCH_CONNECT_FAILED: &str =
    "Could not connect to the match, so it did not start. You are back in the lobby.";

/// A match session was lost and the fresh lobby that follows has not been observed yet.
#[derive(Resource, Default)]
pub(in crate::flow) struct MatchFailureNotice(pub(in crate::flow) bool);

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
pub(super) fn resolve_observation(
    observation: SessionObservation,
    flow: ClientFlow,
    membership: &LobbyMembershipQuery,
    selection: &mut SelectedGameType,
    dashboard_focus: &mut DashboardReturnFocus,
    queue: &crate::ClientQueueModel,
    result_state: &mut crate::ClientMatchResultState,
    routed: &mut RoutedClientLifecycle,
    match_failure: &mut MatchFailureNotice,
    purpose: &mut SessionPurpose,
    commit: &mut FlowCommit,
) {
    match observation {
        SessionObservation::QueueProtocolFailure => {
            commit.teardown = true;
            *selection = SelectedGameType::default();
            fail_to_server_select_with_kind(
                commit,
                FlowErrorKind::Content,
                "The lobby queue state was incompatible with this client".to_string(),
                true,
            );
        }
        SessionObservation::QueueTimedOut => {
            let label = queue
                .pending()
                .map_or("queue command", |pending| match pending.command {
                    brawler::lobby::QueueCommand::Join(_) => "queue admission",
                    brawler::lobby::QueueCommand::Cancel(_) => "queue cancellation",
                });
            commit.error = Some(FlowError {
                kind: FlowErrorKind::Queue,
                message: format!("The {label} acknowledgement is taking longer than expected"),
                return_flow: flow,
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
                // BRL-hjmq: a match session lost while loading never reached the match.
                let message = if flow == ClientFlow::MatchLoading {
                    MATCH_CONNECT_FAILED
                } else {
                    "The connection to the match was lost, and the match could not be resumed."
                };
                commit.error = Some(FlowError {
                    kind: FlowErrorKind::Connection,
                    message: message.to_string(),
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
        SessionObservation::RejoinStarted => {
            // BRL-yyeg: a relaunched app's lobby announced a rejoin. Present the match while the
            // grant arrives; a failure falls back to the same Dashboard message as a lost match.
            match_failure.0 = true;
            commit.next_flow = Some(ClientFlow::Match);
            commit.overlay = Some(OverlayCommit::Clear);
        }
        SessionObservation::MatchRejoined => {
            routed.rejoined = false;
            match_failure.0 = false;
            commit.overlay = Some(OverlayCommit::Clear);
        }
        SessionObservation::PracticeRejected(reason) => {
            if matches!(
                reason,
                brawler::lobby::PracticeStartRejection::StaleCatalog
                    | brawler::lobby::PracticeStartRejection::StaleGameConfiguration
                    | brawler::lobby::PracticeStartRejection::UnknownGameType
            ) {
                commit.teardown = true;
                fail_to_server_select_with_kind(
                    commit,
                    FlowErrorKind::Content,
                    "The lobby content changed incompatibly; reconnect to obtain a fresh game list"
                        .to_string(),
                    true,
                );
            } else {
                commit.error = Some(FlowError {
                    kind: FlowErrorKind::Practice,
                    message: practice_rejection_copy(reason).to_string(),
                    return_flow: flow,
                    actions: [Some(FlowErrorAction::Back), None],
                });
            }
        }
        SessionObservation::CountdownObserved => {
            commit.next_flow = Some(ClientFlow::Match);
            commit.overlay = Some(OverlayCommit::Clear);
        }
        SessionObservation::QueueOutcome(outcome) => {
            resolve_queue_outcome(
                outcome,
                flow,
                selection,
                dashboard_focus,
                result_state,
                purpose,
                commit,
            );
        }
        _ => unreachable!("session observation was routed to the wrong reducer"),
    }
}

fn resolve_queue_outcome(
    outcome: brawler::lobby::QueueCommandOutcome,
    flow: ClientFlow,
    selection: &mut SelectedGameType,
    dashboard_focus: &mut DashboardReturnFocus,
    result_state: &mut crate::ClientMatchResultState,
    purpose: &mut SessionPurpose,
    commit: &mut FlowCommit,
) {
    match outcome.decision {
        brawler::lobby::QueueDecision::Joined(membership) => {
            result_state.context = None;
            result_state.last_accepted_game_type_id = Some(membership.game_type_id.clone());
            selection.catalog_revision = Some(membership.catalog_revision);
            selection.game_type_id = Some(membership.game_type_id.clone());
            selection.configuration_revision = Some(membership.game_type_configuration_revision);
            commit.next_flow = Some(ClientFlow::Queue);
            commit.overlay = Some(OverlayCommit::Clear);
        }
        brawler::lobby::QueueDecision::Cancelled { .. } => {
            *purpose = SessionPurpose::Multiplayer;
            dashboard_focus.0 = Some(DASHBOARD_PLAY_INDEX);
            commit.next_flow = Some(ClientFlow::Dashboard);
            commit.overlay = Some(OverlayCommit::Clear);
        }
        brawler::lobby::QueueDecision::Rejected(reason) => {
            resolve_queue_rejection(&reason, flow, commit);
        }
    }
}

fn resolve_queue_rejection(
    reason: &brawler::lobby::QueueRejection,
    flow: ClientFlow,
    commit: &mut FlowCommit,
) {
    match reason {
        brawler::lobby::QueueRejection::IncompatiblePassives => {
            commit.error = Some(FlowError {
                kind: FlowErrorKind::Queue,
                message: "The selected passives are incompatible".to_string(),
                return_flow: flow,
                actions: [Some(FlowErrorAction::Back), None],
            });
        }
        brawler::lobby::QueueRejection::OverBudget { used, budget } => {
            commit.error = Some(FlowError {
                kind: FlowErrorKind::Queue,
                message: format!("The selected build uses {used} of {budget} points"),
                return_flow: flow,
                actions: [Some(FlowErrorAction::Back), None],
            });
        }
        brawler::lobby::QueueRejection::StaleCatalog
        | brawler::lobby::QueueRejection::StaleGameConfiguration
        | brawler::lobby::QueueRejection::UnknownGameType
        | brawler::lobby::QueueRejection::ProtocolFailure => {
            commit.teardown = true;
            fail_to_server_select_with_kind(
                commit,
                FlowErrorKind::Content,
                "The lobby content changed incompatibly; reconnect to obtain a fresh game list"
                    .to_string(),
                true,
            );
        }
        brawler::lobby::QueueRejection::RateLimited { retry_after_millis } => {
            commit.error = Some(FlowError {
                kind: FlowErrorKind::Queue,
                message: format!(
                    "Queue commands are temporarily limited; try again in {retry_after_millis} ms",
                ),
                return_flow: flow,
                actions: [
                    Some(FlowErrorAction::TryAgainQueue),
                    Some(FlowErrorAction::Disconnect),
                ],
            });
        }
        brawler::lobby::QueueRejection::MustCancelFirst => {
            commit.error = Some(FlowError {
                kind: FlowErrorKind::Queue,
                message: "Cancel the current queue ticket before changing game or build"
                    .to_string(),
                return_flow: flow,
                actions: [Some(FlowErrorAction::Back), None],
            });
        }
        brawler::lobby::QueueRejection::TicketMismatch
        | brawler::lobby::QueueRejection::StaleRequest
        | brawler::lobby::QueueRejection::TemporarilyUnavailable
        | brawler::lobby::QueueRejection::InternalBuildResolution
        | brawler::lobby::QueueRejection::ServerMatchCapacityOccupied => {
            commit.error = Some(FlowError {
                kind: FlowErrorKind::Queue,
                message: "The queue request could not be completed".to_string(),
                return_flow: flow,
                actions: [Some(FlowErrorAction::Disconnect), None],
            });
        }
    }
}

#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_action(
    action: &FlowUiAction,
    now: std::time::Duration,
    flow: ClientFlow,
    membership: &LobbyMembershipQuery,
    selection: &mut SelectedGameType,
    game_draft: &mut GameTypeSelectionDraft,
    dashboard_focus: &mut DashboardReturnFocus,
    queue: &mut crate::ClientQueueModel,
    practice: &mut crate::ClientPracticeModel,
    loading: &mut crate::ClientMatchLoadingModel,
    result_state: &mut crate::ClientMatchResultState,
    routed: &mut RoutedClientLifecycle,
    purpose: &mut SessionPurpose,
    commit: &mut FlowCommit,
) {
    if matches!(action, FlowUiAction::CancelPracticeMaps)
        && flow == ClientFlow::PracticeMapSelect
        && !practice.pending()
    {
        dashboard_focus.0 = Some(DASHBOARD_PRACTICE_INDEX);
    }
    match action {
        FlowUiAction::SelectGameTypeDraft(_)
        | FlowUiAction::ConfirmGameType
        | FlowUiAction::CancelGameType
        | FlowUiAction::OpenGameTypeSelect => resolve_game_selection_action(
            action,
            membership,
            selection,
            game_draft,
            dashboard_focus,
            queue,
            practice,
            commit,
        ),
        FlowUiAction::JoinQueue
        | FlowUiAction::OpenPracticeMaps
        | FlowUiAction::ChoosePracticeMap(_)
        | FlowUiAction::CancelPracticeMaps
        | FlowUiAction::StartPractice
        | FlowUiAction::KeepLoading
        | FlowUiAction::CancelQueue
        | FlowUiAction::RetryQueue
        | FlowUiAction::TryAgainQueue
        | FlowUiAction::RequestCancelMatchStart
        | FlowUiAction::ConfirmCancelMatchStart => resolve_matchmaking_loading_action(
            action, now, flow, membership, selection, queue, practice, loading, purpose, commit,
        ),
        FlowUiAction::QueueAgain
        | FlowUiAction::ReturnToDashboard
        | FlowUiAction::KeepPlaying
        | FlowUiAction::ConfirmLeaveMatch => resolve_results_replay_action(
            action,
            now,
            membership,
            selection,
            dashboard_focus,
            queue,
            practice,
            result_state,
            routed,
            purpose,
            commit,
        ),
        _ => unreachable!("flow action was routed to the wrong reducer"),
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_game_selection_action(
    action: &FlowUiAction,
    membership: &LobbyMembershipQuery,
    selection: &mut SelectedGameType,
    game_draft: &mut GameTypeSelectionDraft,
    dashboard_focus: &mut DashboardReturnFocus,
    queue: &crate::ClientQueueModel,
    practice: &crate::ClientPracticeModel,
    commit: &mut FlowCommit,
) {
    match action {
        FlowUiAction::SelectGameTypeDraft(index) => {
            if membership
                .iter()
                .next()
                .is_some_and(|(membership, _, _)| *index < membership.game_types.len())
            {
                game_draft.selected_index = Some(*index);
            }
        }
        FlowUiAction::ConfirmGameType => {
            if let Some((membership, _, _)) = membership.iter().next()
                && accept_game_type_draft(game_draft, membership, selection)
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
        _ => unreachable!("game-selection action was routed to the wrong reducer"),
    }
}

#[allow(clippy::too_many_arguments)]
fn resolve_matchmaking_loading_action(
    action: &FlowUiAction,
    now: std::time::Duration,
    flow: ClientFlow,
    membership: &LobbyMembershipQuery,
    selection: &SelectedGameType,
    queue: &mut crate::ClientQueueModel,
    practice: &mut crate::ClientPracticeModel,
    loading: &mut crate::ClientMatchLoadingModel,
    purpose: &mut SessionPurpose,
    commit: &mut FlowCommit,
) {
    match action {
        FlowUiAction::JoinQueue => {
            if flow != ClientFlow::Dashboard || practice.pending() {
                return;
            }
            *purpose = SessionPurpose::Multiplayer;
            if let Some((brawler_id, brawler_revision)) = selected_brawler_identity(membership) {
                let _ = queue.start_join(selection, brawler_id, brawler_revision, now);
            }
        }
        FlowUiAction::OpenPracticeMaps => {
            if flow != ClientFlow::Dashboard || queue.pending().is_some() || practice.pending() {
                return;
            }
            if let Some((lobby, _, _)) = membership.iter().next()
                && let Some(game) = lobby
                    .game_types
                    .iter()
                    .find(|g| Some(&g.id) == selection.game_type_id.as_ref())
            {
                let previous = practice
                    .selected_map
                    .as_ref()
                    .filter(|(id, map)| {
                        *id == game.id && game.practice_map_preset_ids.contains(map)
                    })
                    .map(|(_, map)| *map);
                practice.selected_map = previous
                    .or_else(|| game.map_preset_ids.first().copied())
                    .map(|map| (game.id.clone(), map));
                commit.next_flow = Some(ClientFlow::PracticeMapSelect);
            }
        }
        FlowUiAction::ChoosePracticeMap(map) => {
            if flow != ClientFlow::PracticeMapSelect || practice.pending() {
                return;
            }
            if let Some((lobby, _, _)) = membership.iter().next()
                && let Some(game) = lobby
                    .game_types
                    .iter()
                    .find(|g| Some(&g.id) == selection.game_type_id.as_ref())
                && game.practice_map_preset_ids.contains(map)
            {
                practice.selected_map = Some((game.id.clone(), *map));
            }
        }
        FlowUiAction::CancelPracticeMaps => {
            if flow == ClientFlow::PracticeMapSelect && !practice.pending() {
                commit.next_flow = Some(ClientFlow::Dashboard);
            }
        }
        FlowUiAction::StartPractice => {
            if !matches!(flow, ClientFlow::Dashboard | ClientFlow::PracticeMapSelect)
                || queue.pending().is_some()
            {
                return;
            }
            *purpose = SessionPurpose::Practice;
            if let Some((brawler_id, brawler_revision)) = selected_brawler_identity(membership) {
                let _ = practice.start(selection, brawler_id, brawler_revision);
            }
        }
        FlowUiAction::KeepLoading => {
            commit.overlay = Some(OverlayCommit::Clear);
            commit.focus_index = Some(0);
        }
        FlowUiAction::CancelQueue => {
            let _ = queue.start_cancel(now);
        }
        FlowUiAction::RetryQueue => {
            if queue.retry_pending(now) {
                commit.overlay = Some(OverlayCommit::Clear);
            }
        }
        FlowUiAction::TryAgainQueue => {
            if queue.try_again_after_rate_limit(now) {
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
        _ => unreachable!("matchmaking/loading action was routed to the wrong reducer"),
    }
}

#[allow(clippy::too_many_arguments, clippy::too_many_lines)]
fn resolve_results_replay_action(
    action: &FlowUiAction,
    now: std::time::Duration,
    membership: &LobbyMembershipQuery,
    selection: &mut SelectedGameType,
    dashboard_focus: &mut DashboardReturnFocus,
    queue: &mut crate::ClientQueueModel,
    practice: &mut crate::ClientPracticeModel,
    result_state: &mut crate::ClientMatchResultState,
    routed: &mut RoutedClientLifecycle,
    purpose: &mut SessionPurpose,
    commit: &mut FlowCommit,
) {
    match action {
        FlowUiAction::QueueAgain => {
            let exact_game_type_id = result_state
                .context
                .as_ref()
                .and_then(|context| context.game_type_id.clone());
            if *purpose == SessionPurpose::Practice {
                let Some((membership, _, _)) = membership.iter().find(|(_, _, session)| {
                    session.kind == crate::RoutedClientSessionKind::Lobby
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
                    .is_none_or(|brawler| !practice.start(selection, brawler.id, brawler.revision))
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
                session.kind == crate::RoutedClientSessionKind::Lobby
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
                        now,
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
        FlowUiAction::ReturnToDashboard => {
            result_state.context = None;
            *purpose = SessionPurpose::Multiplayer;
            dashboard_focus.0 = Some(DASHBOARD_PLAY_INDEX);
            commit.next_flow = Some(ClientFlow::Dashboard);
        }
        FlowUiAction::KeepPlaying => {
            commit.overlay = Some(OverlayCommit::Clear);
            commit.focus_index = Some(0);
        }
        FlowUiAction::ConfirmLeaveMatch => {
            result_state.context = None;
            // BRL-8y1f: a deliberate leave releases the seat; the session tells the match
            // worker before it disconnects, so no automatic rejoin follows.
            let _ = routed.request_leave_match();
            commit.overlay = Some(OverlayCommit::Clear);
        }
        _ => unreachable!("Results/replay action was routed to the wrong reducer"),
    }
}

fn selected_brawler_identity(
    membership: &LobbyMembershipQuery,
) -> Option<(
    brawler::profiles::SavedBrawlerId,
    brawler::profiles::ProfileRevision,
)> {
    membership.iter().next().and_then(|(membership, _, _)| {
        membership.profile.selected_brawler_id.and_then(|id| {
            membership
                .profile
                .brawlers
                .iter()
                .find(|brawler| brawler.id == id)
                .map(|brawler| (brawler.id, brawler.revision))
        })
    })
}

pub(in crate::flow) fn accept_game_type_draft(
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

const fn practice_rejection_copy(reason: brawler::lobby::PracticeStartRejection) -> &'static str {
    match reason {
        brawler::lobby::PracticeStartRejection::StaleCatalog
        | brawler::lobby::PracticeStartRejection::StaleGameConfiguration => {
            "The server's game catalog changed. Choose the game again."
        }
        brawler::lobby::PracticeStartRejection::UnknownGameType => {
            "That practice game is no longer available."
        }
        brawler::lobby::PracticeStartRejection::InvalidBuild => {
            "The selected brawler no longer matches the server profile. Review and select it again."
        }
        brawler::lobby::PracticeStartRejection::IncompatibleBuild => {
            "This brawler has incompatible choices. Edit it and choose only one elemental resistance passive."
        }
        brawler::lobby::PracticeStartRejection::Busy => {
            "Another match start is already in progress."
        }
        brawler::lobby::PracticeStartRejection::CapacityUnavailable => {
            "The server has no free match capacity right now."
        }
        brawler::lobby::PracticeStartRejection::UnsupportedGameMode => {
            "That game mode is not available in Practice."
        }
        brawler::lobby::PracticeStartRejection::InvalidMap => {
            "This map is no longer available for Practice. Choose another map."
        }
        brawler::lobby::PracticeStartRejection::InvalidMapRecipe => {
            "The server rejected this map revision. Return to Map Maker and review the highlighted problems."
        }
        brawler::lobby::PracticeStartRejection::RosterMismatch => {
            "This map does not have enough safe spawns for the selected Practice roster."
        }
        brawler::lobby::PracticeStartRejection::Internal => "The server could not start practice.",
    }
}
