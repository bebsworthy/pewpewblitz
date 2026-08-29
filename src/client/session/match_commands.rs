use super::{
    ACTION_INTERACT, AppExit, AuthoritativeTick, Client, ClientMatchLoadingModel,
    ClientNetworkConfig, ClientPlayableGate, Controlled, Duration, Fighter, MatchCommand,
    MatchCommandOutcome, MatchCommandRequest, MatchCommandState, MatchLoadingClientAction,
    MatchLoadingClientMessage, MatchLoadingServerMessage, MatchLoadingStatus, MatchParticipant,
    MatchPhase, MatchRoot, MatchState, MessageReceiver, MessageSender, MessageWriter,
    NetworkTransport, PendingLocalActions, Query, Real, Remote, Res, ResMut, Resource,
    RoutedClientLifecycle, RoutedClientPhase, SessionChannel, Time, With,
};

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
    if config.transport != NetworkTransport::RoutedUdp
        || (!config.presents_product_shell() && !config.product_match_smoke)
        || routed.phase != RoutedClientPhase::Match
    {
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
        if config.render_measurement.is_some() && state.ready_sent_for != Some(correlation) {
            eprintln!(
                "brawler-client timing match-ready client_id={} request_id={} ts_ms={}",
                config.client_id,
                correlation.0,
                crate::diagnostics::unix_micros_now() / 1_000
            );
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
            crate::protocol::MatchLoadingServerOutcome::CancellationAccepted
            | crate::protocol::MatchLoadingServerOutcome::TerminalFailure => {
                loading.observe_match_cancellation(true);
                let _ = routed.request_return_to_lobby();
            }
            crate::protocol::MatchLoadingServerOutcome::CancellationTooLate => {
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
    phase: Option<crate::lobby::MatchLoadingPhase>,
    retry_due: bool,
    waiting_for_same_match: bool,
    has_controlled_fighter: bool,
) -> bool {
    playable
        && phase != Some(crate::lobby::MatchLoadingPhase::Cancelling)
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
    routed: Res<RoutedClientLifecycle>,
    playable: Res<ClientPlayableGate>,
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
    let product_routed =
        config.transport == NetworkTransport::RoutedUdp && config.presents_product_shell();
    if product_routed {
        return;
    }
    let automatic = if product_routed {
        playable.0
    } else {
        automatic_match_command_enabled(&config, roster.iter().count())
    };
    if should_rearm_headless_match_command(
        config.headless_simulation_ticks.is_some(),
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
    if config.headless_simulation_ticks.is_some()
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

#[allow(clippy::needless_pass_by_value)]
pub(super) fn finish_product_match_smoke(
    config: Res<ClientNetworkConfig>,
    roots: Query<&MatchState, With<MatchRoot>>,
    mut exit: MessageWriter<AppExit>,
) {
    if config.product_match_smoke
        && config.render_measurement.is_none()
        && !config.product_requeue_smoke
        && roots
            .iter()
            .any(|state| matches!(state.phase, MatchPhase::Active { .. }))
    {
        exit.write(AppExit::Success);
    }
}

pub(in crate::client) fn should_rearm_headless_match_command(
    automation_enabled: bool,
    sent_for_phase: Option<(crate::matchplay::MatchId, MatchPhase)>,
    match_id: crate::matchplay::MatchId,
    phase: MatchPhase,
) -> bool {
    automation_enabled
        && matches!(phase, MatchPhase::Countdown { .. })
        && sent_for_phase == Some((match_id, MatchPhase::Waiting))
}

pub(in crate::client) fn automatic_match_command_enabled(
    config: &ClientNetworkConfig,
    roster_count: usize,
) -> bool {
    (config.headless_simulation_ticks.is_some()
        || config.windowed_combat_demo.is_some()
        || config.windowed_controller_demo.is_some()
        || config.render_measurement.is_some())
        && config
            .exit_after_roster
            .is_none_or(|target| roster_count >= target)
}

#[cfg(test)]
mod tests {
    use super::{MatchLoadingCommandState, match_loading_ready_to_send, ready_retry_due};
    use core::time::Duration;

    #[test]
    fn match_loading_readiness_requires_every_owned_fact() {
        assert!(match_loading_ready_to_send(true, None, true, true, true));
        assert!(!match_loading_ready_to_send(false, None, true, true, true));
        assert!(!match_loading_ready_to_send(
            true,
            Some(crate::lobby::MatchLoadingPhase::Cancelling),
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
