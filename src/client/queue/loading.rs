//! Match reservation loading, cancellation, lobby return, and transport observation.

use bevy::prelude::*;
use lightyear::prelude::{MessageReceiver, MessageSender};
use std::collections::VecDeque;

use super::ClientPracticeModel;
use crate::client::{Client, RoutedClientSession};

#[derive(Resource, Clone, Debug, Default)]
pub struct ClientMatchLoadingModel {
    pub(super) lobby_generation: Option<u64>,
    pub(super) expected_sequence: u64,
    pub(super) active: Option<crate::lobby::ReservationStarted>,
    pub(super) started_observation: Option<crate::lobby::ReservationStarted>,
    pub(super) phase: Option<crate::lobby::MatchLoadingPhase>,
    pub(super) protocol_failure: bool,
    pub(super) next_client_sequence: u64,
    pub(super) outbound: VecDeque<crate::lobby::MatchmakingClientMessage>,
    pub(super) returned_observation: bool,
    pub(super) match_cancel_requested: bool,
    pub(super) last_status_revision: u32,
    pub(super) loading_counts: Option<(u8, u8, u8)>,
}

impl ClientMatchLoadingModel {
    pub(super) fn reset_for_lobby_generation(&mut self, generation: u64) {
        self.lobby_generation = Some(generation);
        self.expected_sequence = 0;
        self.active = None;
        self.started_observation = None;
        self.phase = None;
        self.protocol_failure = false;
        self.next_client_sequence = 0;
        self.outbound.clear();
        self.returned_observation = false;
        self.match_cancel_requested = false;
        self.last_status_revision = 0;
        self.loading_counts = None;
    }

    pub fn take_started(&mut self) -> Option<crate::lobby::ReservationStarted> {
        self.started_observation.take()
    }

    #[must_use]
    pub fn active(&self) -> Option<&crate::lobby::ReservationStarted> {
        self.active.as_ref()
    }

    #[must_use]
    pub const fn phase(&self) -> Option<crate::lobby::MatchLoadingPhase> {
        self.phase
    }

    pub(crate) fn observe_status(&mut self, status: crate::protocol::MatchLoadingStatus) {
        if status.generation == 1
            && status.revision > self.last_status_revision
            && status.connected <= status.expected
            && status.checked_in <= status.connected
        {
            self.last_status_revision = status.revision;
            self.phase = Some(status.phase);
            self.loading_counts = Some((status.expected, status.connected, status.checked_in));
        }
    }

    pub fn request_cancel(&mut self) -> bool {
        let Some(active) = self.active.as_ref() else {
            return false;
        };
        self.next_client_sequence = self.next_client_sequence.saturating_add(1).max(1);
        self.outbound
            .push_back(crate::lobby::MatchmakingClientMessage {
                sequence: self.next_client_sequence,
                action: crate::lobby::MatchmakingClientAction::Cancel {
                    reservation_id: active.reservation_id,
                    generation: 1,
                },
            });
        self.phase = Some(crate::lobby::MatchLoadingPhase::Cancelling);
        self.match_cancel_requested = true;
        true
    }

    pub fn take_returned(&mut self) -> bool {
        core::mem::take(&mut self.returned_observation)
    }

    pub(crate) fn take_match_cancel_requested(&mut self) -> bool {
        core::mem::take(&mut self.match_cancel_requested)
    }

    pub(crate) fn observe_match_cancellation(&mut self, accepted: bool) {
        self.match_cancel_requested = false;
        if accepted {
            self.active = None;
            self.phase = Some(crate::lobby::MatchLoadingPhase::ReturningToQueue);
            self.returned_observation = true;
        } else {
            self.phase = Some(crate::lobby::MatchLoadingPhase::WaitingForPlayers);
        }
    }
}

pub(super) fn send_matchmaking_messages(
    mut model: ResMut<ClientMatchLoadingModel>,
    mut senders: Query<&mut MessageSender<crate::lobby::MatchmakingClientMessage>, With<Client>>,
) {
    let Ok(mut sender) = senders.single_mut() else {
        return;
    };
    while let Some(message) = model.outbound.pop_front() {
        sender.send::<crate::protocol::SessionChannel>(message);
    }
}

#[allow(clippy::too_many_lines, clippy::type_complexity)]
pub(super) fn observe_matchmaking_messages(
    mut model: ResMut<ClientMatchLoadingModel>,
    mut practice: ResMut<ClientPracticeModel>,
    mut lifecycle: ResMut<crate::client::RoutedClientLifecycle>,
    mut clients: Query<
        (
            &RoutedClientSession,
            Option<&mut MessageReceiver<crate::lobby::MatchmakingServerMessage>>,
        ),
        With<Client>,
    >,
) {
    for (session, receiver) in &mut clients {
        if session.kind != crate::client::RoutedClientSessionKind::Lobby
            || session.generation != lifecycle.generation
        {
            continue;
        }
        if model.lobby_generation != Some(session.generation) {
            model.reset_for_lobby_generation(session.generation);
        }
        practice.bind_generation(session.generation);
        let Some(mut receiver) = receiver else {
            continue;
        };
        for message in receiver.receive() {
            if message.sequence == 0 || message.sequence <= model.expected_sequence {
                continue;
            }
            if message.sequence != model.expected_sequence.saturating_add(1) {
                model.protocol_failure = true;
                continue;
            }
            model.expected_sequence = message.sequence;
            observe_matchmaking_phase(&mut model, &mut practice, &mut lifecycle, message.phase);
        }
    }
}

fn observe_matchmaking_phase(
    model: &mut ClientMatchLoadingModel,
    practice: &mut ClientPracticeModel,
    lifecycle: &mut crate::client::RoutedClientLifecycle,
    phase: crate::lobby::MatchmakingServerPhase,
) {
    match phase {
        crate::lobby::MatchmakingServerPhase::ReservationStarted(started) => {
            if model
                .active
                .as_ref()
                .is_some_and(|active| active != &started)
            {
                model.protocol_failure = true;
                return;
            }
            model.phase = Some(crate::lobby::MatchLoadingPhase::Reserving);
            model.active = Some(started.clone());
            model.started_observation = Some(started);
            practice.accept_started();
        }
        crate::lobby::MatchmakingServerPhase::BeginMatchConnect(begin) => {
            if model
                .active
                .as_ref()
                .is_none_or(|active| active.reservation_id != begin.reservation_id)
                || !lifecycle.accept_grant(begin.grant)
            {
                model.protocol_failure = true;
            } else {
                model.phase = Some(crate::lobby::MatchLoadingPhase::Connecting);
            }
        }
        crate::lobby::MatchmakingServerPhase::Status { phase, .. } => {
            model.phase = Some(phase);
        }
        crate::lobby::MatchmakingServerPhase::Removed { .. } => {
            model.active = None;
            model.phase = Some(crate::lobby::MatchLoadingPhase::ReturningToQueue);
            model.returned_observation = true;
            model.match_cancel_requested = false;
        }
        crate::lobby::MatchmakingServerPhase::PracticeRejected { request_id, reason } => {
            practice.accept_rejection(request_id, reason);
        }
    }
}
