//! Matchmaking queue state, correlation, freshness, and queue transport.

use bevy::prelude::*;
use lightyear::prelude::{Disconnected, MessageReceiver, MessageSender};
use std::{collections::VecDeque, time::Duration};

use crate::client::{Client, ClientLobbyMembership, RoutedClientSession};

const SNAPSHOT_FRESHNESS: Duration = Duration::from_secs(3);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingQueueCommand {
    pub request_id: crate::lobby::QueueRequestId,
    pub command: crate::lobby::QueueCommand,
    pub sent_at: Duration,
    pub timed_out: bool,
    pub(super) timeout_presented: bool,
    pub rate_limited_until: Option<Duration>,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct ClientQueueModel {
    pub(super) generation: Option<u64>,
    pub(super) next_request_id: u64,
    pub(super) snapshot: Option<crate::lobby::QueuePoolSnapshot>,
    pub(super) snapshot_received_at: Option<Duration>,
    pub(super) deferred_snapshot: Option<crate::lobby::QueuePoolSnapshot>,
    pub(super) snapshot_fresh: bool,
    pub(super) membership: Option<crate::lobby::QueueMembership>,
    pub(super) last_accepted_game_type_id: Option<crate::lobby::GameTypeId>,
    pub(super) pending: Option<PendingQueueCommand>,
    pub(super) latest_outcome: Option<crate::lobby::QueueCommandOutcome>,
    pub(super) outbound: VecDeque<crate::lobby::QueueClientMessage>,
    pub(super) protocol_failure: bool,
    pub(super) required_snapshot_revision: Option<u64>,
    pub freshness_aged: u64,
    pub freshness_restored: u64,
}

impl ClientQueueModel {
    pub(in crate::client) fn bind_lobby_generation(&mut self, generation: u64) {
        if self.generation != Some(generation) {
            self.reset_for_generation(Some(generation));
        }
    }

    pub(in crate::client) fn start_requeue_join(
        &mut self,
        generation: u64,
        lobby: &ClientLobbyMembership,
        game_type_id: &crate::lobby::GameTypeId,
        now: Duration,
    ) -> bool {
        let Some(game) = lobby
            .game_types
            .iter()
            .find(|game| &game.id == game_type_id)
        else {
            return false;
        };
        let Some(brawler) = lobby.profile.selected_brawler_id.and_then(|id| {
            lobby
                .profile
                .brawlers
                .iter()
                .find(|brawler| brawler.id == id)
        }) else {
            return false;
        };
        self.bind_lobby_generation(generation);
        self.start_join(
            &crate::client::flow::SelectedGameType {
                catalog_revision: Some(lobby.catalog_revision),
                game_type_id: Some(game.id.clone()),
                configuration_revision: Some(game.configuration_revision),
            },
            brawler.id,
            brawler.revision,
            now,
        )
    }

    #[must_use]
    pub fn snapshot(&self) -> Option<&crate::lobby::QueuePoolSnapshot> {
        self.snapshot
            .as_ref()
            .filter(|_| self.required_snapshot_is_fresh())
    }

    #[must_use]
    pub fn raw_snapshot(&self) -> Option<&crate::lobby::QueuePoolSnapshot> {
        self.snapshot.as_ref()
    }

    #[must_use]
    pub fn membership(&self) -> Option<&crate::lobby::QueueMembership> {
        self.membership.as_ref()
    }

    #[must_use]
    pub(super) fn last_accepted_game_type_id(&self) -> Option<&crate::lobby::GameTypeId> {
        self.last_accepted_game_type_id.as_ref()
    }

    #[must_use]
    pub fn pending(&self) -> Option<&PendingQueueCommand> {
        self.pending.as_ref()
    }

    #[must_use]
    pub const fn protocol_failure(&self) -> bool {
        self.protocol_failure
    }

    #[must_use]
    pub fn required_snapshot_is_fresh(&self) -> bool {
        self.snapshot_fresh
            && self.required_snapshot_revision.is_none_or(|required| {
                self.snapshot
                    .as_ref()
                    .is_some_and(|snapshot| snapshot.state_revision >= required)
            })
    }

    pub fn take_outcome(&mut self) -> Option<crate::lobby::QueueCommandOutcome> {
        self.latest_outcome.take()
    }

    pub fn start_join(
        &mut self,
        selected: &crate::client::flow::SelectedGameType,
        brawler_id: crate::profiles::SavedBrawlerId,
        brawler_revision: crate::profiles::ProfileRevision,
        now: Duration,
    ) -> bool {
        let (Some(catalog_revision), Some(game_type_id), Some(configuration_revision)) = (
            selected.catalog_revision,
            selected.game_type_id.clone(),
            selected.configuration_revision,
        ) else {
            return false;
        };
        self.start_command(
            crate::lobby::QueueCommand::Join(crate::lobby::QueueJoinCommand {
                catalog_revision,
                game_type_id,
                game_type_configuration_revision: configuration_revision,
                brawler_id,
                brawler_revision,
            }),
            now,
        )
    }

    pub fn start_cancel(&mut self, now: Duration) -> bool {
        let Some(ticket_id) = self
            .membership
            .as_ref()
            .map(|membership| membership.ticket_id)
        else {
            return false;
        };
        self.start_command(
            crate::lobby::QueueCommand::Cancel(crate::lobby::QueueCancelCommand { ticket_id }),
            now,
        )
    }

    fn start_command(&mut self, command: crate::lobby::QueueCommand, now: Duration) -> bool {
        if self.pending.is_some() || self.generation.is_none() {
            return false;
        }
        let Some(next) = self
            .next_request_id
            .checked_add(1)
            .and_then(crate::lobby::QueueRequestId::new)
        else {
            self.protocol_failure = true;
            return false;
        };
        self.next_request_id = next.get();
        self.outbound
            .push_back(crate::lobby::QueueClientMessage::Command {
                request_id: next,
                command: command.clone(),
            });
        self.pending = Some(PendingQueueCommand {
            request_id: next,
            command,
            sent_at: now,
            timed_out: false,
            timeout_presented: false,
            rate_limited_until: None,
        });
        true
    }

    pub fn retry_pending(&mut self, now: Duration) -> bool {
        let Some(pending) = self.pending.as_mut() else {
            return false;
        };
        if !pending.timed_out || pending.rate_limited_until.is_some() {
            return false;
        }
        self.outbound
            .push_back(crate::lobby::QueueClientMessage::Command {
                request_id: pending.request_id,
                command: pending.command.clone(),
            });
        pending.sent_at = now;
        pending.timed_out = false;
        pending.timeout_presented = false;
        true
    }

    /// Return one presentation fact for each timeout transition. The pending command remains
    /// timed out so an explicit Retry can resend the exact frozen request.
    pub fn take_timeout_notice(&mut self) -> bool {
        let Some(pending) = self.pending.as_mut() else {
            return false;
        };
        if !pending.timed_out || pending.timeout_presented {
            return false;
        }
        pending.timeout_presented = true;
        true
    }

    pub fn try_again_after_rate_limit(&mut self, now: Duration) -> bool {
        let Some(pending) = self.pending.take() else {
            return false;
        };
        if pending
            .rate_limited_until
            .is_none_or(|deadline| now < deadline)
        {
            self.pending = Some(pending);
            return false;
        }
        self.start_command(pending.command, now)
    }

    fn reset_for_generation(&mut self, generation: Option<u64>) {
        let aged = self.freshness_aged;
        let restored = self.freshness_restored;
        let last_accepted_game_type_id = self.last_accepted_game_type_id.clone();
        *self = Self {
            generation,
            last_accepted_game_type_id,
            freshness_aged: aged,
            freshness_restored: restored,
            ..Self::default()
        };
    }

    pub(super) fn accept_snapshot(
        &mut self,
        snapshot: crate::lobby::QueuePoolSnapshot,
        membership: &ClientLobbyMembership,
        now: Duration,
    ) {
        if snapshot.catalog_revision != membership.catalog_revision
            || snapshot.pools.len() != membership.game_types.len()
            || !snapshot
                .pools
                .iter()
                .zip(&membership.game_types)
                .all(|(row, game)| {
                    row.game_type_id == game.id
                        && row.game_type_configuration_revision == game.configuration_revision
                        && row.formation_size
                            == game
                                .team_count
                                .checked_mul(game.players_per_team)
                                .unwrap_or(0)
                })
        {
            self.protocol_failure = true;
            return;
        }
        match self.snapshot.as_ref() {
            Some(current) if snapshot.state_revision < current.state_revision => return,
            Some(current) if snapshot.state_revision == current.state_revision => {
                if snapshot != *current {
                    self.protocol_failure = true;
                    return;
                }
                self.snapshot_received_at = Some(now);
            }
            _ => {
                self.snapshot = Some(snapshot);
                self.snapshot_received_at = Some(now);
            }
        }
        if !self.snapshot_fresh {
            self.snapshot_fresh = true;
            self.freshness_restored = self.freshness_restored.saturating_add(1);
        }
    }

    pub(super) fn accept_outcome(
        &mut self,
        outcome: crate::lobby::QueueCommandOutcome,
        now: Duration,
    ) {
        let Some(pending) = self.pending.as_ref() else {
            return;
        };
        if outcome.request_id != pending.request_id {
            return;
        }
        if !outcome_matches_pending(pending, &outcome.decision, self.membership.as_ref()) {
            self.protocol_failure = true;
            return;
        }
        self.outbound
            .push_back(crate::lobby::QueueClientMessage::OutcomeAck {
                request_id: outcome.request_id,
            });
        match &outcome.decision {
            crate::lobby::QueueDecision::Joined(membership) => {
                self.last_accepted_game_type_id = Some(membership.game_type_id.clone());
                self.required_snapshot_revision = Some(membership.admitted_at_pool_state_revision);
                self.membership = Some(membership.clone());
                self.pending = None;
            }
            crate::lobby::QueueDecision::Cancelled {
                ticket_id,
                resulting_pool_state_revision,
            } => {
                if self
                    .membership
                    .as_ref()
                    .is_some_and(|membership| membership.ticket_id == *ticket_id)
                {
                    self.membership = None;
                    self.required_snapshot_revision = Some(*resulting_pool_state_revision);
                    self.pending = None;
                } else {
                    self.protocol_failure = true;
                }
            }
            crate::lobby::QueueDecision::Rejected(crate::lobby::QueueRejection::RateLimited {
                retry_after_millis,
            }) => {
                if let Some(pending) = self.pending.as_mut() {
                    pending.rate_limited_until = Some(
                        now.saturating_add(Duration::from_millis(u64::from(*retry_after_millis))),
                    );
                    pending.timed_out = false;
                }
            }
            crate::lobby::QueueDecision::Rejected(_) => self.pending = None,
        }
        self.latest_outcome = Some(outcome);
    }

    pub(super) fn update_time(&mut self, now: Duration) {
        if self.snapshot_fresh
            && self
                .snapshot_received_at
                .is_some_and(|received| now.saturating_sub(received) > SNAPSHOT_FRESHNESS)
        {
            self.snapshot_fresh = false;
            self.freshness_aged = self.freshness_aged.saturating_add(1);
        }
        if let Some(pending) = self.pending.as_mut()
            && pending.rate_limited_until.is_none()
            && now.saturating_sub(pending.sent_at) >= COMMAND_TIMEOUT
            && !pending.timed_out
        {
            pending.timed_out = true;
            pending.timeout_presented = false;
        }
    }
}

fn outcome_matches_pending(
    pending: &PendingQueueCommand,
    decision: &crate::lobby::QueueDecision,
    current_membership: Option<&crate::lobby::QueueMembership>,
) -> bool {
    match (&pending.command, decision) {
        (
            crate::lobby::QueueCommand::Join(command),
            crate::lobby::QueueDecision::Joined(membership),
        ) => {
            membership.catalog_revision == command.catalog_revision
                && membership.game_type_id == command.game_type_id
                && membership.game_type_configuration_revision
                    == command.game_type_configuration_revision
                && membership.brawler_id == command.brawler_id
                && membership.brawler_revision == command.brawler_revision
                && membership.accepted_build.identity.recipe_fingerprint.0 != 0
        }
        (
            crate::lobby::QueueCommand::Cancel(command),
            crate::lobby::QueueDecision::Cancelled { ticket_id, .. },
        ) => {
            command.ticket_id == *ticket_id
                && current_membership
                    .is_some_and(|membership| membership.ticket_id == command.ticket_id)
        }
        (_, crate::lobby::QueueDecision::Rejected(_)) => true,
        _ => false,
    }
}

#[allow(
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(in crate::client) fn observe_queue_messages(
    time: Res<Time<Real>>,
    lifecycle: Res<crate::client::RoutedClientLifecycle>,
    mut model: ResMut<ClientQueueModel>,
    mut clients: Query<
        (
            &RoutedClientSession,
            Option<&ClientLobbyMembership>,
            Option<&mut MessageReceiver<crate::lobby::QueueCommandOutcome>>,
            Option<&mut MessageReceiver<crate::lobby::QueuePoolSnapshot>>,
            Has<Disconnected>,
        ),
        With<Client>,
    >,
) {
    let Some((session, membership, outcome_receiver, snapshot_receiver, disconnected)) =
        clients.iter_mut().find(|(session, _, _, _, _)| {
            session.kind == crate::client::RoutedClientSessionKind::Lobby
                && session.generation == lifecycle.generation
        })
    else {
        if model.generation.is_some() {
            model.reset_for_generation(None);
        }
        return;
    };
    if disconnected {
        if model.generation.is_some() {
            model.reset_for_generation(None);
        }
        return;
    }
    model.bind_lobby_generation(session.generation);
    let now = time.elapsed();
    if let Some(mut snapshots) = snapshot_receiver {
        consume_queue_snapshots(&mut model, &mut snapshots, membership, now);
    }
    if let Some(mut outcomes) = outcome_receiver {
        consume_queue_outcomes(&mut model, &mut outcomes, membership, now);
    }
    apply_deferred_snapshot(&mut model, membership, now);
}

fn consume_queue_snapshots(
    model: &mut ClientQueueModel,
    receiver: &mut MessageReceiver<crate::lobby::QueuePoolSnapshot>,
    membership: Option<&ClientLobbyMembership>,
    now: Duration,
) {
    for snapshot in receiver.receive() {
        if let Some(membership) = membership {
            model.accept_snapshot(snapshot, membership, now);
        } else {
            model.deferred_snapshot = Some(snapshot);
        }
    }
}

fn consume_queue_outcomes(
    model: &mut ClientQueueModel,
    receiver: &mut MessageReceiver<crate::lobby::QueueCommandOutcome>,
    membership: Option<&ClientLobbyMembership>,
    now: Duration,
) {
    for outcome in receiver.receive() {
        if membership.is_some() {
            model.accept_outcome(outcome, now);
        } else {
            model.protocol_failure = true;
        }
    }
}

fn apply_deferred_snapshot(
    model: &mut ClientQueueModel,
    membership: Option<&ClientLobbyMembership>,
    now: Duration,
) {
    if let Some(membership) = membership
        && let Some(snapshot) = model.deferred_snapshot.take()
    {
        model.accept_snapshot(snapshot, membership, now);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(super) fn update_queue_time(time: Res<Time<Real>>, mut model: ResMut<ClientQueueModel>) {
    model.update_time(time.elapsed());
}

pub(super) fn send_queue_messages(
    mut model: ResMut<ClientQueueModel>,
    mut senders: Query<&mut MessageSender<crate::lobby::QueueClientMessage>, With<Client>>,
) {
    let Ok(mut sender) = senders.single_mut() else {
        return;
    };
    while let Some(message) = model.outbound.pop_front() {
        sender.send::<crate::protocol::SessionChannel>(message);
    }
}
