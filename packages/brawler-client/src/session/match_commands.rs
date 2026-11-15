use super::automation_hooks::AutomationHooks;
use super::{
    ACTION_INTERACT, AuthoritativeTick, Client, ClientMatchLoadingModel, ClientNetworkConfig,
    ClientPlayableGate, Controlled, Duration, Fighter, MatchCommand, MatchCommandOutcome,
    MatchCommandRequest, MatchCommandState, MatchLoadingClientAction, MatchLoadingClientMessage,
    MatchLoadingServerMessage, MatchLoadingStatus, MatchParticipant, MatchPhase, MatchRoot,
    MatchState, MessageReceiver, MessageSender, NetworkTransport, PendingLocalActions, Query, Real,
    Remote, Res, ResMut, Resource, RoutedClientLifecycle, RoutedClientPhase, SessionChannel, Time,
    With, info, warn,
};

/// How long a confirmed Leave Match waits for the worker's answer before disconnecting anyway.
/// Only a broken link reaches it; the seat is then held as for any lost session (BRL-8y1f).
pub(super) const MATCH_LEAVE_ANSWER_TIMEOUT: Duration = Duration::from_secs(2);

/// What the leave driver does this frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum LeaveStep {
    Idle,
    Send,
    Wait,
    Disconnect,
}

/// Decide the next step of a confirmed Leave Match from the facts this frame.
fn leave_step(
    requested: bool,
    sent: Option<(u64, Duration)>,
    answered_request: Option<u64>,
    can_send: bool,
    now: Duration,
) -> LeaveStep {
    if !requested {
        return LeaveStep::Idle;
    }
    match sent {
        None if can_send => LeaveStep::Send,
        None => LeaveStep::Disconnect,
        Some((request_id, _)) if answered_request == Some(request_id) => LeaveStep::Disconnect,
        Some((_, sent_at)) if now.saturating_sub(sent_at) >= MATCH_LEAVE_ANSWER_TIMEOUT => {
            LeaveStep::Disconnect
        }
        Some(_) => LeaveStep::Wait,
    }
}

/// Drive a confirmed Leave Match: tell the match worker, then disconnect once it answered or the
/// bounded wait passed (BRL-8y1f). Disconnecting without telling it would look like a lost
/// connection, and the lobby would put the player straight back into the match.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn drive_match_leave(
    time: Res<Time<Real>>,
    mut routed: ResMut<RoutedClientLifecycle>,
    mut state: ResMut<MatchCommandState>,
    roots: Query<&MatchState, With<MatchRoot>>,
    mut senders: Query<&mut MessageSender<MatchCommandRequest>, With<Client>>,
) {
    if routed.leave_requested && routed.phase != RoutedClientPhase::Match {
        routed.leave_requested = false;
    }
    let match_id = roots.single().ok().map(|root| root.match_id);
    let step = leave_step(
        routed.leave_requested,
        state.leave_sent,
        state.last_outcome.map(|outcome| outcome.request_id),
        match_id.is_some() && senders.single().is_ok(),
        time.elapsed(),
    );
    match step {
        LeaveStep::Idle => state.leave_sent = None,
        LeaveStep::Wait => {}
        LeaveStep::Send => {
            let (Some(match_id), Ok(mut sender)) = (match_id, senders.single_mut()) else {
                return;
            };
            state.next_request_id = state.next_request_id.saturating_add(1).max(1);
            sender.send::<SessionChannel>(MatchCommandRequest {
                request_id: state.next_request_id,
                match_id,
                command: MatchCommand::LeaveMatch,
            });
            state.leave_sent = Some((state.next_request_id, time.elapsed()));
            info!("brawler client told the match worker it is leaving");
        }
        LeaveStep::Disconnect => {
            if state.leave_sent.is_some_and(|(request_id, _)| {
                state.last_outcome.map(|o| o.request_id) != Some(request_id)
            }) {
                warn!("brawler client left the match without the worker's answer");
            }
            routed.leave_requested = false;
            state.leave_sent = None;
            let _ = routed.request_return_to_lobby();
        }
    }
}

pub(super) fn process_match_command_outcomes(
    mut state: ResMut<MatchCommandState>,
    mut receivers: Query<Option<&mut MessageReceiver<MatchCommandOutcome>>, With<Client>>,
) {
    for receiver in &mut receivers {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for outcome in receiver.receive() {
            state.last_outcome = Some(outcome);
        }
    }
}

#[derive(Resource, Default)]
pub(super) struct MatchLoadingCommandState {
    ready_sent_for: Option<(u64, u128, u128)>,
    ready_last_sent_at: Option<Duration>,
    cancel_sent_for: Option<(u64, u128, u128)>,
}

type MatchLoadingCorrelation = (u64, u128, u128);

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
pub(super) fn drive_match_loading_check_in(
    config: Res<ClientNetworkConfig>,
    automation: AutomationHooks,
    time: Res<Time<Real>>,
    mut routed: ResMut<RoutedClientLifecycle>,
    playable: Res<ClientPlayableGate>,
    mut loading: ResMut<ClientMatchLoadingModel>,
    mut state: ResMut<MatchLoadingCommandState>,
    roots: Query<&MatchState, With<MatchRoot>>,
    controlled: Query<&MatchParticipant, (With<Fighter>, With<Controlled>)>,
    mut clients: Query<
        (
            &mut MessageSender<MatchLoadingClientMessage>,
            Option<&mut MessageReceiver<MatchLoadingServerMessage>>,
            Option<&mut MessageReceiver<MatchLoadingStatus>>,
        ),
        With<Client>,
    >,
) {
    if config.transport != NetworkTransport::RoutedUdp || routed.phase != RoutedClientPhase::Match {
        return;
    }
    let Some(grant) = routed.accepted_grant else {
        return;
    };
    let correlation = (
        grant.request_id.get(),
        grant.allocation_id.get(),
        grant.match_id.get(),
    );
    for (mut sender, receiver, status_receiver) in &mut clients {
        if let Some(mut status_receiver) = status_receiver {
            consume_match_loading_statuses(&mut status_receiver, correlation, &mut loading);
        }
        if let Some(mut receiver) = receiver {
            consume_match_loading_outcomes(&mut receiver, correlation, &mut loading, &mut routed);
        }
        if loading.match_cancel_requested() && state.cancel_sent_for != Some(correlation) {
            send_match_loading_action(
                &mut sender,
                correlation,
                MatchLoadingClientAction::CancelMatchStart,
            );
            state.cancel_sent_for = Some(correlation);
            loading.mark_match_cancel_sent();
            continue;
        }
        if !match_loading_ready_to_send(
            playable.0,
            loading.phase(),
            ready_retry_due(&state, correlation, time.elapsed()),
            roots.single().is_ok_and(|root| {
                root.match_id.0 == correlation.2 && matches!(root.phase, MatchPhase::Waiting)
            }),
            controlled.single().is_ok(),
        ) {
            continue;
        }
        send_match_loading_action(&mut sender, correlation, MatchLoadingClientAction::Ready);
        if state.ready_sent_for != Some(correlation) {
            automation.report_match_ready(config.client_id, correlation.0);
        }
        state.ready_sent_for = Some(correlation);
        state.ready_last_sent_at = Some(time.elapsed());
    }
}

fn consume_match_loading_statuses(
    receiver: &mut MessageReceiver<MatchLoadingStatus>,
    correlation: MatchLoadingCorrelation,
    loading: &mut ClientMatchLoadingModel,
) {
    for status in receiver.receive() {
        if (status.request_id, status.allocation_id, status.match_id) == correlation {
            loading.observe_status(status);
        }
    }
}

fn consume_match_loading_outcomes(
    receiver: &mut MessageReceiver<MatchLoadingServerMessage>,
    correlation: MatchLoadingCorrelation,
    loading: &mut ClientMatchLoadingModel,
    routed: &mut RoutedClientLifecycle,
) {
    for outcome in receiver.receive() {
        if (outcome.request_id, outcome.allocation_id, outcome.match_id) != correlation {
            continue;
        }
        match outcome.outcome {
            brawler::protocol::MatchLoadingServerOutcome::CancellationAccepted
            | brawler::protocol::MatchLoadingServerOutcome::TerminalFailure => {
                loading.observe_match_cancellation(true);
                let _ = routed.request_return_to_lobby();
            }
            brawler::protocol::MatchLoadingServerOutcome::CancellationTooLate => {
                loading.observe_match_cancellation(false);
            }
        }
    }
}

fn send_match_loading_action(
    sender: &mut MessageSender<MatchLoadingClientMessage>,
    correlation: MatchLoadingCorrelation,
    action: MatchLoadingClientAction,
) {
    sender.send::<SessionChannel>(MatchLoadingClientMessage {
        request_id: correlation.0,
        allocation_id: correlation.1,
        match_id: correlation.2,
        action,
    });
}

fn ready_retry_due(
    state: &MatchLoadingCommandState,
    correlation: MatchLoadingCorrelation,
    now: Duration,
) -> bool {
    state.ready_sent_for != Some(correlation)
        || state
            .ready_last_sent_at
            .is_none_or(|sent_at| now.saturating_sub(sent_at) >= Duration::from_millis(500))
}

#[allow(
    clippy::fn_params_excessive_bools,
    reason = "the named readiness facts keep the conjunction independently testable"
)]
fn match_loading_ready_to_send(
    playable: bool,
    phase: Option<brawler::lobby::MatchLoadingPhase>,
    retry_due: bool,
    waiting_for_same_match: bool,
    has_controlled_fighter: bool,
) -> bool {
    playable
        && phase != Some(brawler::lobby::MatchLoadingPhase::Cancelling)
        && retry_due
        && waiting_for_same_match
        && has_controlled_fighter
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
#[allow(clippy::too_many_arguments)]
pub(super) fn send_match_command(
    config: Res<ClientNetworkConfig>,
    automation: AutomationHooks,
    routed: Res<RoutedClientLifecycle>,
    mut state: ResMut<MatchCommandState>,
    pending: Res<PendingLocalActions>,
    roots: Query<&MatchState, With<MatchRoot>>,
    controlled: Query<&MatchParticipant, (With<Fighter>, With<Controlled>)>,
    authoritative_ticks: Query<&AuthoritativeTick, (With<Fighter>, With<Controlled>)>,
    roster: Query<(), (With<Remote>, With<Fighter>)>,
    mut senders: Query<&mut MessageSender<MatchCommandRequest>, With<Client>>,
) {
    if config.transport == NetworkTransport::RoutedUdp && routed.phase != RoutedClientPhase::Match {
        return;
    }
    let Ok(match_state) = roots.single() else {
        return;
    };
    let Ok(participant) = controlled.single() else {
        return;
    };
    let pressed = pending.action_indicator & ACTION_INTERACT != 0;
    // Every routed allocation uses the correlated loading check-in, including diagnostics.
    let product_routed = config.transport == NetworkTransport::RoutedUdp;
    if product_routed {
        return;
    }
    let automatic = automation.automatic_match_commands(roster.iter().count());
    if should_rearm_headless_match_command(
        automation.rearms_match_commands(),
        state.sent_for_phase,
        match_state.match_id,
        match_state.phase,
    ) {
        // A countdown departure returns the same match ID to Waiting. Re-arm automation while the
        // countdown is still observable so that the unchanged Waiting key can be sent again.
        state.sent_for_phase = None;
    }
    let command = match match_state.phase {
        MatchPhase::Waiting
            if !participant.ready && ((!product_routed && pressed) || automatic) =>
        {
            Some(MatchCommand::SetReady(true))
        }
        MatchPhase::Completed {
            restart_unlocked_at_tick,
            ..
        } if !product_routed
            && !participant.restart_ready
            && authoritative_ticks
                .iter()
                .next()
                .is_some_and(|tick| tick.0 >= restart_unlocked_at_tick)
            && (pressed || automatic) =>
        {
            Some(MatchCommand::ReadyForRestart)
        }
        _ => None,
    };
    let Some(command) = command else {
        return;
    };
    if automation.rearms_match_commands()
        && state.sent_for_phase == Some((match_state.match_id, match_state.phase))
    {
        return;
    }
    state.next_request_id = state.next_request_id.saturating_add(1).max(1);
    for mut sender in &mut senders {
        sender.send::<SessionChannel>(MatchCommandRequest {
            request_id: state.next_request_id,
            match_id: match_state.match_id,
            command,
        });
    }
    state.sent_for_phase = Some((match_state.match_id, match_state.phase));
}

pub(crate) fn should_rearm_headless_match_command(
    automation_enabled: bool,
    sent_for_phase: Option<(brawler::matchplay::MatchId, MatchPhase)>,
    match_id: brawler::matchplay::MatchId,
    phase: MatchPhase,
) -> bool {
    automation_enabled
        && matches!(phase, MatchPhase::Countdown { .. })
        && sent_for_phase == Some((match_id, MatchPhase::Waiting))
}

#[cfg(test)]
mod tests {
    use super::{
        LeaveStep, MATCH_LEAVE_ANSWER_TIMEOUT, MatchLoadingCommandState, leave_step,
        match_loading_ready_to_send, ready_retry_due,
    };
    use core::time::Duration;

    #[test]
    fn a_confirmed_leave_tells_the_worker_and_waits_a_bounded_time_for_its_answer() {
        let sent_at = Duration::from_secs(10);
        assert_eq!(
            leave_step(false, None, None, true, sent_at),
            LeaveStep::Idle
        );
        assert_eq!(leave_step(true, None, None, true, sent_at), LeaveStep::Send);
        // Without a match session to tell, the client leaves at once.
        assert_eq!(
            leave_step(true, None, None, false, sent_at),
            LeaveStep::Disconnect
        );
        let sent = Some((4, sent_at));
        assert_eq!(leave_step(true, sent, None, true, sent_at), LeaveStep::Wait);
        assert_eq!(
            leave_step(true, sent, Some(3), true, sent_at),
            LeaveStep::Wait,
            "an earlier command's answer is not the leave's"
        );
        assert_eq!(
            leave_step(true, sent, Some(4), true, sent_at),
            LeaveStep::Disconnect
        );
        assert_eq!(
            leave_step(
                true,
                sent,
                None,
                true,
                sent_at + MATCH_LEAVE_ANSWER_TIMEOUT.saturating_sub(Duration::from_millis(1))
            ),
            LeaveStep::Wait
        );
        assert_eq!(
            leave_step(true, sent, None, true, sent_at + MATCH_LEAVE_ANSWER_TIMEOUT),
            LeaveStep::Disconnect
        );
    }

    #[test]
    fn match_loading_readiness_requires_every_owned_fact() {
        assert!(match_loading_ready_to_send(true, None, true, true, true));
        assert!(!match_loading_ready_to_send(false, None, true, true, true));
        assert!(!match_loading_ready_to_send(
            true,
            Some(brawler::lobby::MatchLoadingPhase::Cancelling),
            true,
            true,
            true,
        ));
        assert!(!match_loading_ready_to_send(true, None, false, true, true));
        assert!(!match_loading_ready_to_send(true, None, true, false, true));
        assert!(!match_loading_ready_to_send(true, None, true, true, false));
    }

    #[test]
    fn match_loading_ready_retry_is_bound_to_correlation_and_interval() {
        let correlation = (1, 2, 3);
        let mut state = MatchLoadingCommandState::default();
        assert!(ready_retry_due(&state, correlation, Duration::ZERO));

        state.ready_sent_for = Some(correlation);
        state.ready_last_sent_at = Some(Duration::from_secs(1));
        assert!(!ready_retry_due(
            &state,
            correlation,
            Duration::from_millis(1_499),
        ));
        assert!(ready_retry_due(
            &state,
            correlation,
            Duration::from_millis(1_500),
        ));
        assert!(ready_retry_due(&state, (4, 5, 6), Duration::from_secs(1)));
    }
}
