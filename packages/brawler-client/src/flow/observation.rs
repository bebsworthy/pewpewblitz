//! Ordered runtime observation for the deferred client-flow reducer.

use crate::{
    ClientJoinPhase, ClientJoinStatus, ClientLobbyFailure, ClientLobbyMembership,
    ClientMatchLoadingModel, ClientMatchResultState, ClientPracticeModel, ClientQueueModel,
    RoutedClientLifecycle, RoutedClientPhase, RoutedClientSession, RoutedClientSessionKind,
    flow::{
        actions::{PendingFlowActions, SessionObservation},
        connection::{
            ConnectionStage, PendingConnection, ResolverState, accepted_observation,
            attempt_deadline_expiry, observation_for_expiry,
        },
        model::ClientFlow,
    },
};
use bevy::{
    prelude::*,
    tasks::{block_on, poll_once},
};
use lightyear::prelude::client::Client;

enum ObservationScope {
    Continue,
    Complete(Option<SessionObservation>),
}

fn poll_resolver_observation(
    resolver: &mut ResolverState,
    pending: Option<&PendingConnection>,
) -> Option<SessionObservation> {
    let task = resolver.task.as_mut()?;
    let result = block_on(poll_once(&mut task.task))?;
    let generation = task.generation;
    resolver.task = None;
    pending
        .is_some_and(|pending| pending.generation == generation)
        .then_some(SessionObservation::ResolverCompleted { generation, result })
}

fn take_global_observation(
    queue: &mut ClientQueueModel,
    practice: &mut ClientPracticeModel,
    loading: &mut ClientMatchLoadingModel,
) -> Option<SessionObservation> {
    if queue.protocol_failure() {
        Some(SessionObservation::QueueProtocolFailure)
    } else if let Some(reason) = practice.take_rejection() {
        Some(SessionObservation::PracticeRejected(reason))
    } else if loading.take_started().is_some() {
        Some(SessionObservation::ReservationStarted)
    } else if loading.take_returned() {
        Some(SessionObservation::MatchStartReturned)
    } else {
        None
    }
}

/// State of the fresh lobby session that follows a completed or lost match.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LobbyReturn {
    NotObserved,
    Accepted,
    Failed,
}

fn observe_match_scope(
    flow: ClientFlow,
    lobby_return: LobbyReturn,
    match_failed: bool,
    countdown_observed: bool,
    rejoin: RejoinView,
) -> ObservationScope {
    if flow == ClientFlow::Match && rejoin.rejoined {
        ObservationScope::Complete(Some(SessionObservation::MatchRejoined))
    } else if flow == ClientFlow::Match
        && lobby_return == LobbyReturn::Accepted
        && rejoin.in_progress
    {
        // BRL-yyeg: this lobby session is fetching a rejoin grant; keep the match presented.
        ObservationScope::Complete(None)
    } else if rejoin.in_progress && LOBBY_FLOWS.contains(&flow) {
        ObservationScope::Complete(Some(SessionObservation::RejoinStarted))
    } else if flow == ClientFlow::Match && lobby_return == LobbyReturn::Accepted {
        ObservationScope::Complete(Some(SessionObservation::FreshLobbyReturn))
    } else if flow == ClientFlow::Match && lobby_return == LobbyReturn::Failed {
        ObservationScope::Complete(Some(SessionObservation::LobbyReturnFailed))
    } else if flow == ClientFlow::Match && match_failed {
        ObservationScope::Complete(Some(SessionObservation::MatchFailed))
    } else if flow == ClientFlow::MatchLoading {
        ObservationScope::Complete(
            countdown_observed.then_some(SessionObservation::CountdownObserved),
        )
    } else {
        ObservationScope::Continue
    }
}

/// Screens presented over an authenticated lobby session.
const LOBBY_FLOWS: [ClientFlow; 7] = [
    ClientFlow::Dashboard,
    ClientFlow::GameTypeSelect,
    ClientFlow::PracticeMapSelect,
    ClientFlow::MapLibrary,
    ClientFlow::MapEditor,
    ClientFlow::Queue,
    ClientFlow::Results,
];

/// The routed lifecycle's automatic-rejoin facts the flow reacts to (BRL-yyeg).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
struct RejoinView {
    in_progress: bool,
    rejoined: bool,
}

fn observe_lobby_scope(
    flow: ClientFlow,
    disconnected: bool,
    queue: &mut ClientQueueModel,
) -> ObservationScope {
    if !LOBBY_FLOWS.contains(&flow) {
        return ObservationScope::Continue;
    }
    let observation = if disconnected {
        Some(SessionObservation::UnexpectedLoss)
    } else if queue.take_timeout_notice() {
        Some(SessionObservation::QueueTimedOut)
    } else {
        None
    };
    ObservationScope::Complete(observation)
}

fn observe_connection(
    now: std::time::Duration,
    pending: &mut PendingConnection,
    failure: Option<ClientLobbyFailure>,
    disconnected: bool,
    lobby_active: bool,
    first_phase: Option<&ClientJoinPhase>,
) -> Option<SessionObservation> {
    if let Some(failure) = failure {
        return Some(SessionObservation::Rejected(failure));
    }
    if lobby_active {
        return Some(accepted_observation(now, pending, disconnected));
    }
    if let Some(expiry) = attempt_deadline_expiry(now, pending) {
        return Some(observation_for_expiry(expiry));
    }
    if matches!(first_phase, Some(ClientJoinPhase::AwaitingOutcome)) {
        pending.stage = ConnectionStage::JoiningLobby;
    }
    (pending.current_entity.is_some() && disconnected)
        .then_some(SessionObservation::CandidateFailed)
}

fn commit_observation(
    actions: &mut PendingFlowActions,
    observation: Option<SessionObservation>,
    fallback: Option<SessionObservation>,
) {
    actions.session = observation.or(fallback);
}

#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "one ordered coordinator preserves observation priority across runtime-owned inputs"
)]
pub(super) fn observe_session(
    time: Res<Time<Real>>,
    flow: Res<State<ClientFlow>>,
    mut pending: Option<ResMut<PendingConnection>>,
    mut resolver: ResMut<ResolverState>,
    memberships: Query<(Entity, &ClientLobbyMembership), With<Client>>,
    failures: Query<&ClientLobbyFailure, With<Client>>,
    statuses: Query<(&ClientJoinStatus, &RoutedClientSession), With<Client>>,
    match_states: Query<&brawler::matchplay::MatchState, With<brawler::matchplay::MatchRoot>>,
    mut actions: ResMut<PendingFlowActions>,
    mut queue: ResMut<ClientQueueModel>,
    mut loading: ResMut<ClientMatchLoadingModel>,
    mut practice: ResMut<ClientPracticeModel>,
    result_state: Res<ClientMatchResultState>,
    routed: Res<RoutedClientLifecycle>,
) {
    let resolver_observation = poll_resolver_observation(&mut resolver, pending.as_deref());

    if let Some(observation) = take_global_observation(&mut queue, &mut practice, &mut loading) {
        commit_observation(&mut actions, Some(observation), resolver_observation);
        return;
    }

    let lobby_active_for_generation = statuses.iter().any(|(status, session)| {
        session.kind == RoutedClientSessionKind::Lobby
            && session.generation == routed.generation
            && matches!(status.phase, ClientJoinPhase::LobbyActive { .. })
    });
    let match_disconnected_for_generation = statuses.iter().any(|(status, session)| {
        session.kind == RoutedClientSessionKind::Match
            && session.generation == routed.generation
            && matches!(status.phase, ClientJoinPhase::Disconnected)
    });
    let countdown_observed = match_states.iter().any(|state| {
        matches!(
            state.phase,
            brawler::matchplay::MatchPhase::Countdown { .. }
                | brawler::matchplay::MatchPhase::Active { .. }
                | brawler::matchplay::MatchPhase::Completed { .. }
        )
    });
    let lobby_disconnected_for_generation = statuses.iter().any(|(status, session)| {
        session.kind == RoutedClientSessionKind::Lobby
            && session.generation == routed.generation
            && matches!(status.phase, ClientJoinPhase::Disconnected)
    });
    let lobby_return = if memberships.iter().next().is_some() && lobby_active_for_generation {
        LobbyReturn::Accepted
    } else if routed.phase == RoutedClientPhase::Lobby && lobby_disconnected_for_generation {
        LobbyReturn::Failed
    } else {
        LobbyReturn::NotObserved
    };
    match observe_match_scope(
        *flow.get(),
        lobby_return,
        routed.phase == RoutedClientPhase::Match
            && result_state.context.is_none()
            && match_disconnected_for_generation,
        countdown_observed,
        RejoinView {
            in_progress: routed.rejoin_in_progress(),
            rejoined: routed.rejoined,
        },
    ) {
        ObservationScope::Continue => {}
        ObservationScope::Complete(observation) => {
            commit_observation(&mut actions, observation, resolver_observation);
            return;
        }
    }

    if let Some(outcome) = queue.take_outcome() {
        commit_observation(
            &mut actions,
            Some(SessionObservation::QueueOutcome(outcome)),
            resolver_observation,
        );
        return;
    }

    match observe_lobby_scope(*flow.get(), lobby_disconnected_for_generation, &mut queue) {
        ObservationScope::Continue => {}
        ObservationScope::Complete(observation) => {
            commit_observation(&mut actions, observation, resolver_observation);
            return;
        }
    }

    if *flow.get() != ClientFlow::Connecting {
        commit_observation(&mut actions, None, resolver_observation);
        return;
    }
    let Some(pending) = pending.as_deref_mut() else {
        commit_observation(&mut actions, None, resolver_observation);
        return;
    };
    let disconnected = statuses
        .iter()
        .any(|(status, _)| matches!(status.phase, ClientJoinPhase::Disconnected));
    let lobby_active = memberships.iter().next().is_some()
        && statuses
            .iter()
            .any(|(status, _)| matches!(status.phase, ClientJoinPhase::LobbyActive { .. }));
    let first_phase = statuses.iter().next().map(|(status, _)| &status.phase);
    let observation = observe_connection(
        time.elapsed(),
        pending,
        failures.iter().next().cloned(),
        disconnected,
        lobby_active,
        first_phase,
    );
    commit_observation(&mut actions, observation, resolver_observation);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_observation_prefers_fresh_lobby_return_over_match_failure() {
        assert!(matches!(
            observe_match_scope(
                ClientFlow::Match,
                LobbyReturn::Accepted,
                true,
                false,
                RejoinView::default()
            ),
            ObservationScope::Complete(Some(SessionObservation::FreshLobbyReturn))
        ));
        assert!(matches!(
            observe_match_scope(
                ClientFlow::MatchLoading,
                LobbyReturn::NotObserved,
                false,
                false,
                RejoinView::default()
            ),
            ObservationScope::Complete(None)
        ));
    }

    #[test]
    fn failed_lobby_return_during_match_presentation_is_observed_before_match_failure() {
        assert!(matches!(
            observe_match_scope(
                ClientFlow::Match,
                LobbyReturn::Failed,
                true,
                false,
                RejoinView::default()
            ),
            ObservationScope::Complete(Some(SessionObservation::LobbyReturnFailed))
        ));
        assert!(matches!(
            observe_match_scope(
                ClientFlow::Dashboard,
                LobbyReturn::Failed,
                false,
                false,
                RejoinView::default()
            ),
            ObservationScope::Continue
        ));
    }

    #[test]
    fn a_rejoin_keeps_the_match_presented_and_lobby_screens_move_to_it() {
        let rejoining = RejoinView {
            in_progress: true,
            rejoined: false,
        };
        assert!(matches!(
            observe_match_scope(
                ClientFlow::Match,
                LobbyReturn::Accepted,
                false,
                false,
                rejoining
            ),
            ObservationScope::Complete(None)
        ));
        assert!(matches!(
            observe_match_scope(
                ClientFlow::Dashboard,
                LobbyReturn::Accepted,
                false,
                false,
                rejoining
            ),
            ObservationScope::Complete(Some(SessionObservation::RejoinStarted))
        ));
        // A lobby failure during the rejoin is still reported.
        assert!(matches!(
            observe_match_scope(
                ClientFlow::Match,
                LobbyReturn::Failed,
                false,
                false,
                rejoining
            ),
            ObservationScope::Complete(Some(SessionObservation::LobbyReturnFailed))
        ));
        assert!(matches!(
            observe_match_scope(
                ClientFlow::Match,
                LobbyReturn::NotObserved,
                false,
                true,
                RejoinView {
                    in_progress: false,
                    rejoined: true,
                }
            ),
            ObservationScope::Complete(Some(SessionObservation::MatchRejoined))
        ));
    }

    /// BRL-jxfy: a rejoin into a completed match returns to the lobby right after it is
    /// accepted. The accepted rejoin is still observed first, so the loss it resolves never
    /// reaches the fresh lobby return.
    #[test]
    fn an_accepted_rejoin_is_observed_before_the_fresh_lobby_return() {
        assert!(matches!(
            observe_match_scope(
                ClientFlow::Match,
                LobbyReturn::Accepted,
                false,
                true,
                RejoinView {
                    in_progress: false,
                    rejoined: true,
                }
            ),
            ObservationScope::Complete(Some(SessionObservation::MatchRejoined))
        ));
    }

    #[test]
    fn scoped_observation_replaces_resolver_fallback() {
        let mut actions = PendingFlowActions::default();
        commit_observation(
            &mut actions,
            Some(SessionObservation::MatchFailed),
            Some(SessionObservation::CandidateFailed),
        );
        assert!(matches!(
            actions.session,
            Some(SessionObservation::MatchFailed)
        ));
    }
}
