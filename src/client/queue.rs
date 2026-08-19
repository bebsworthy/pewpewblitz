//! Client-side queue command lifecycle and honest aggregate snapshot presentation state.

use super::{Client, ClientLobbyMembership, RoutedClientSession};
use bevy::prelude::*;
use lightyear::prelude::{Disconnected, MessageReceiver, MessageSender};
use std::{collections::VecDeque, time::Duration};

const SNAPSHOT_FRESHNESS: Duration = Duration::from_secs(3);
const COMMAND_TIMEOUT: Duration = Duration::from_secs(10);

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct PendingQueueCommand {
    pub request_id: crate::lobby::QueueRequestId,
    pub command: crate::lobby::QueueCommand,
    pub sent_at: Duration,
    pub timed_out: bool,
    timeout_presented: bool,
    pub rate_limited_until: Option<Duration>,
}

#[derive(Resource, Clone, Debug, Default)]
pub struct ClientQueueModel {
    generation: Option<u64>,
    next_request_id: u64,
    snapshot: Option<crate::lobby::QueuePoolSnapshot>,
    snapshot_received_at: Option<Duration>,
    deferred_snapshot: Option<crate::lobby::QueuePoolSnapshot>,
    snapshot_fresh: bool,
    membership: Option<crate::lobby::QueueMembership>,
    pending: Option<PendingQueueCommand>,
    latest_outcome: Option<crate::lobby::QueueCommandOutcome>,
    outbound: VecDeque<crate::lobby::QueueClientMessage>,
    protocol_failure: bool,
    required_snapshot_revision: Option<u64>,
    pub freshness_aged: u64,
    pub freshness_restored: u64,
}

impl ClientQueueModel {
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
        selected: &super::flow::SelectedGameType,
        build: crate::builds::BuildCandidate,
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
                build,
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
        *self = Self {
            generation,
            freshness_aged: aged,
            freshness_restored: restored,
            ..Self::default()
        };
    }

    fn accept_snapshot(
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

    fn accept_outcome(&mut self, outcome: crate::lobby::QueueCommandOutcome, now: Duration) {
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

    fn update_time(&mut self, now: Duration) {
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
                && membership.accepted_build.identity.revision == command.build.build_revision
                && match command.build.selection {
                    crate::builds::BuildSelection::Preset(id) => {
                        membership.accepted_build.identity.source_build_preset_id == Some(id)
                    }
                    crate::builds::BuildSelection::Custom(recipe) => {
                        membership
                            .accepted_build
                            .identity
                            .source_build_preset_id
                            .is_none()
                            && membership.accepted_build.canonical_recipe == recipe
                    }
                }
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

pub struct ClientQueuePlugin;

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
enum HeadlessQueueSmokeStage {
    #[default]
    AwaitingInitialSnapshot,
    Joining,
    AwaitingJoinedSnapshot,
    Cancelling,
    AwaitingCancelledSnapshot,
    Complete,
}

impl Plugin for ClientQueuePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientQueueModel>()
            .init_resource::<HeadlessQueueSmokeStage>()
            .add_systems(
                Update,
                (
                    observe_queue_messages,
                    update_queue_time,
                    drive_headless_queue_smoke,
                    send_queue_messages,
                )
                    .chain(),
            );
    }
}

#[allow(
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(super) fn observe_queue_messages(
    time: Res<Time<Real>>,
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
    let Ok((session, membership, outcome_receiver, snapshot_receiver, disconnected)) =
        clients.single_mut()
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
    if model.generation != Some(session.generation) {
        model.reset_for_generation(Some(session.generation));
    }
    let now = time.elapsed();
    if let Some(mut snapshots) = snapshot_receiver {
        for snapshot in snapshots.receive() {
            if let Some(membership) = membership {
                model.accept_snapshot(snapshot, membership, now);
            } else {
                model.deferred_snapshot = Some(snapshot);
            }
        }
    }
    if let Some(mut outcomes) = outcome_receiver {
        for outcome in outcomes.receive() {
            if membership.is_some() {
                model.accept_outcome(outcome, now);
            } else {
                model.protocol_failure = true;
            }
        }
    }
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
fn update_queue_time(time: Res<Time<Real>>, mut model: ResMut<ClientQueueModel>) {
    model.update_time(time.elapsed());
}

fn send_queue_messages(
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

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn drive_headless_queue_smoke(
    time: Res<Time<Real>>,
    config: Res<super::ClientNetworkConfig>,
    builds: Res<crate::builds::BuildCatalogResource>,
    memberships: Query<&ClientLobbyMembership, With<Client>>,
    mut model: ResMut<ClientQueueModel>,
    mut stage: ResMut<HeadlessQueueSmokeStage>,
    mut exit: MessageWriter<AppExit>,
) {
    if !config.product_queue_smoke || *stage == HeadlessQueueSmokeStage::Complete {
        return;
    }
    if model.protocol_failure() {
        error!("brawler product queue smoke observed incompatible queue state");
        exit.write(AppExit::error());
        *stage = HeadlessQueueSmokeStage::Complete;
        return;
    }
    if let Some(outcome) = model.take_outcome()
        && let crate::lobby::QueueDecision::Rejected(reason) = outcome.decision
    {
        error!(?reason, "brawler product queue smoke command was rejected");
        exit.write(AppExit::error());
        *stage = HeadlessQueueSmokeStage::Complete;
        return;
    }
    let Some(lobby) = memberships.iter().next() else {
        return;
    };
    match *stage {
        HeadlessQueueSmokeStage::AwaitingInitialSnapshot => {
            let Some(game) = lobby.game_types.first() else {
                return;
            };
            if model.snapshot().is_none() {
                return;
            }
            let selection = super::flow::SelectedGameType {
                catalog_revision: Some(lobby.catalog_revision),
                game_type_id: Some(game.id.clone()),
                configuration_revision: Some(game.configuration_revision),
            };
            let build_selection = match config.build_preset.unwrap_or(1) {
                5 => crate::builds::BuildSelection::Custom(
                    super::build_editor::default_custom_recipe(),
                ),
                id => crate::builds::BuildSelection::Preset(crate::builds::BuildPresetId(id)),
            };
            let candidate = crate::builds::BuildCandidate {
                build_revision: builds.0.balance_revision,
                selection: build_selection,
            };
            if model.start_join(&selection, candidate, time.elapsed()) {
                *stage = HeadlessQueueSmokeStage::Joining;
            }
        }
        HeadlessQueueSmokeStage::Joining => {
            if model.membership().is_some() {
                *stage = HeadlessQueueSmokeStage::AwaitingJoinedSnapshot;
            }
        }
        HeadlessQueueSmokeStage::AwaitingJoinedSnapshot => {
            if model.required_snapshot_is_fresh() && model.start_cancel(time.elapsed()) {
                *stage = HeadlessQueueSmokeStage::Cancelling;
            }
        }
        HeadlessQueueSmokeStage::Cancelling => {
            if model.membership().is_none() && model.pending().is_none() {
                *stage = HeadlessQueueSmokeStage::AwaitingCancelledSnapshot;
            }
        }
        HeadlessQueueSmokeStage::AwaitingCancelledSnapshot => {
            if model.required_snapshot_is_fresh() {
                let marker = format!(
                    "brawler-client queue-evidence admissions=1 cancellations=1 freshness_aged={} freshness_restored={}\n",
                    model.freshness_aged, model.freshness_restored
                );
                let _ = std::io::Write::write_all(&mut std::io::stderr().lock(), marker.as_bytes());
                exit.write(AppExit::Success);
                *stage = HeadlessQueueSmokeStage::Complete;
            }
        }
        HeadlessQueueSmokeStage::Complete => {}
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn game() -> crate::lobby::AdvertisedGameType {
        crate::lobby::AdvertisedGameType {
            id: crate::lobby::GameTypeId::new("wipeout-2v2").unwrap(),
            configuration_revision: 1,
            display_name: "Wipeout 2v2".to_string(),
            mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
            map_preset_ids: vec![crate::map::MapPresetId(1)],
            team_count: 2,
            players_per_team: 2,
            rules_summary: crate::lobby::AdvertisedRulesSummary::Wipeout {
                target_score: 10,
                active_limit_ticks: 1_000,
            },
        }
    }

    fn membership() -> ClientLobbyMembership {
        ClientLobbyMembership {
            player_id: crate::protocol::PlayerId(1),
            accepted_display_name: "Player".to_string(),
            server_name: "Server".to_string(),
            catalog_revision: crate::lobby::CatalogRevision([1; 32]),
            game_types: vec![game()],
        }
    }

    fn snapshot(revision: u64, queued: u16) -> crate::lobby::QueuePoolSnapshot {
        crate::lobby::QueuePoolSnapshot {
            catalog_revision: crate::lobby::CatalogRevision([1; 32]),
            state_revision: revision,
            pools: vec![crate::lobby::QueuePoolRow {
                game_type_id: crate::lobby::GameTypeId::new("wipeout-2v2").unwrap(),
                game_type_configuration_revision: 1,
                queued,
                formation_size: 4,
            }],
        }
    }

    fn joined_membership(game_type_id: &str, ticket_id: u128) -> crate::lobby::QueueMembership {
        let builds = crate::builds::BuildCatalog::embedded().unwrap();
        let weapons = crate::combat::WeaponCatalog::embedded().unwrap();
        let preset = builds.preset(crate::builds::BuildPresetId(1)).unwrap();
        let preview = super::super::build_editor::resolve_build_preview(
            crate::builds::BuildSelection::Preset(preset.id),
            &builds,
            &weapons,
        )
        .unwrap();
        crate::lobby::QueueMembership {
            ticket_id: crate::lobby::QueueTicketId::new(ticket_id).unwrap(),
            catalog_revision: crate::lobby::CatalogRevision([1; 32]),
            game_type_id: crate::lobby::GameTypeId::new(game_type_id).unwrap(),
            game_type_configuration_revision: 1,
            accepted_build: crate::builds::AcceptedBuildSummary {
                canonical_recipe: preset.recipe,
                identity: preview.identity,
                total_points: preview.total_points,
            },
            admitted_at_pool_state_revision: 2,
        }
    }

    #[test]
    fn equal_snapshot_refreshes_freshness_older_does_not_and_conflict_fails() {
        let membership = membership();
        let mut model = ClientQueueModel {
            generation: Some(1),
            ..default()
        };
        model.accept_snapshot(snapshot(2, 1), &membership, Duration::ZERO);
        model.update_time(Duration::from_secs(4));
        assert!(model.snapshot().is_none());
        assert_eq!(model.freshness_aged, 1);
        model.accept_snapshot(snapshot(1, 0), &membership, Duration::from_secs(4));
        assert!(model.snapshot().is_none());
        model.accept_snapshot(snapshot(2, 1), &membership, Duration::from_secs(4));
        assert!(model.snapshot().is_some());
        assert_eq!(model.freshness_restored, 2);
        model.accept_snapshot(snapshot(2, 2), &membership, Duration::from_secs(5));
        assert!(model.protocol_failure());
    }

    #[test]
    fn pending_timeout_retry_keeps_request_and_rate_limit_try_again_changes_it() {
        let mut model = ClientQueueModel {
            generation: Some(1),
            ..default()
        };
        let selected = super::super::flow::SelectedGameType {
            catalog_revision: Some(crate::lobby::CatalogRevision([1; 32])),
            game_type_id: Some(crate::lobby::GameTypeId::new("wipeout-2v2").unwrap()),
            configuration_revision: Some(1),
        };
        let candidate = crate::builds::BuildCandidate {
            build_revision: crate::builds::BuildRevision(1),
            selection: crate::builds::BuildSelection::Preset(crate::builds::BuildPresetId(1)),
        };
        assert!(model.start_join(&selected, candidate, Duration::ZERO));
        let first = model.pending().unwrap().request_id;
        model.update_time(Duration::from_secs(10));
        assert!(model.pending().unwrap().timed_out);
        assert!(model.take_timeout_notice());
        assert!(!model.take_timeout_notice());
        assert!(model.retry_pending(Duration::from_secs(10)));
        assert_eq!(model.pending().unwrap().request_id, first);
        model.accept_outcome(
            crate::lobby::QueueCommandOutcome {
                request_id: first,
                decision: crate::lobby::QueueDecision::Rejected(
                    crate::lobby::QueueRejection::RateLimited {
                        retry_after_millis: 500,
                    },
                ),
            },
            Duration::from_secs(11),
        );
        assert!(!model.try_again_after_rate_limit(Duration::from_millis(11_499)));
        assert!(model.try_again_after_rate_limit(Duration::from_millis(11_500)));
        assert!(model.pending().unwrap().request_id > first);
    }

    #[test]
    fn cancellation_revision_hides_an_older_fresh_snapshot_until_replacement_arrives() {
        let lobby = membership();
        let ticket_id = crate::lobby::QueueTicketId::new(9).unwrap();
        let mut model = ClientQueueModel {
            generation: Some(1),
            membership: Some(crate::lobby::QueueMembership {
                ticket_id,
                catalog_revision: lobby.catalog_revision,
                game_type_id: lobby.game_types[0].id.clone(),
                game_type_configuration_revision: 1,
                accepted_build: crate::builds::AcceptedBuildSummary {
                    canonical_recipe: super::super::build_editor::default_custom_recipe(),
                    identity: crate::builds::SelectedBuild {
                        source_build_preset_id: None,
                        recipe_fingerprint: crate::builds::BuildRecipeFingerprint(1),
                        revision: crate::builds::BuildRevision(1),
                    },
                    total_points: 10,
                },
                admitted_at_pool_state_revision: 2,
            }),
            ..default()
        };
        model.accept_snapshot(snapshot(2, 1), &lobby, Duration::ZERO);
        assert!(model.start_cancel(Duration::ZERO));
        let request_id = model.pending().unwrap().request_id;
        model.accept_outcome(
            crate::lobby::QueueCommandOutcome {
                request_id,
                decision: crate::lobby::QueueDecision::Cancelled {
                    ticket_id,
                    resulting_pool_state_revision: 3,
                },
            },
            Duration::ZERO,
        );

        assert!(model.snapshot().is_none());
        assert!(model.raw_snapshot().is_some());
        model.accept_snapshot(snapshot(3, 0), &lobby, Duration::from_millis(1));
        assert!(model.snapshot().is_some());
    }

    #[test]
    fn late_outcome_remains_authoritative_after_timeout_notice() {
        let mut model = ClientQueueModel {
            generation: Some(1),
            ..default()
        };
        let selected = super::super::flow::SelectedGameType {
            catalog_revision: Some(crate::lobby::CatalogRevision([1; 32])),
            game_type_id: Some(crate::lobby::GameTypeId::new("wipeout-2v2").unwrap()),
            configuration_revision: Some(1),
        };
        let candidate = crate::builds::BuildCandidate {
            build_revision: crate::builds::BuildRevision(1),
            selection: crate::builds::BuildSelection::Preset(crate::builds::BuildPresetId(1)),
        };
        assert!(model.start_join(&selected, candidate, Duration::ZERO));
        let request_id = model.pending().unwrap().request_id;
        model.update_time(Duration::from_secs(10));
        assert!(model.take_timeout_notice());
        model.accept_outcome(
            crate::lobby::QueueCommandOutcome {
                request_id,
                decision: crate::lobby::QueueDecision::Joined(joined_membership("wipeout-2v2", 9)),
            },
            Duration::from_secs(11),
        );
        assert!(model.pending().is_none());
        assert!(model.membership().is_some());
        assert!(matches!(
            model.take_outcome().unwrap().decision,
            crate::lobby::QueueDecision::Joined(_)
        ));
    }

    #[test]
    fn joined_outcome_must_match_the_frozen_join_target() {
        let mut model = ClientQueueModel {
            generation: Some(1),
            ..default()
        };
        let selected = super::super::flow::SelectedGameType {
            catalog_revision: Some(crate::lobby::CatalogRevision([1; 32])),
            game_type_id: Some(crate::lobby::GameTypeId::new("wipeout-2v2").unwrap()),
            configuration_revision: Some(1),
        };
        let candidate = crate::builds::BuildCandidate {
            build_revision: crate::builds::BuildRevision(1),
            selection: crate::builds::BuildSelection::Preset(crate::builds::BuildPresetId(1)),
        };
        assert!(model.start_join(&selected, candidate, Duration::ZERO));
        let request_id = model.pending().unwrap().request_id;

        model.accept_outcome(
            crate::lobby::QueueCommandOutcome {
                request_id,
                decision: crate::lobby::QueueDecision::Joined(joined_membership("hot-zone-2v2", 9)),
            },
            Duration::ZERO,
        );

        assert!(model.protocol_failure());
        assert!(model.membership().is_none());
        assert!(model.pending().is_some());
        assert_eq!(
            model.outbound.len(),
            1,
            "invalid outcome is not acknowledged"
        );
    }

    #[test]
    fn joined_outcome_cannot_replace_membership_while_cancel_is_pending() {
        let current = joined_membership("wipeout-2v2", 8);
        let mut model = ClientQueueModel {
            generation: Some(1),
            membership: Some(current.clone()),
            ..default()
        };
        assert!(model.start_cancel(Duration::ZERO));
        let request_id = model.pending().unwrap().request_id;

        model.accept_outcome(
            crate::lobby::QueueCommandOutcome {
                request_id,
                decision: crate::lobby::QueueDecision::Joined(joined_membership("wipeout-2v2", 9)),
            },
            Duration::ZERO,
        );

        assert!(model.protocol_failure());
        assert_eq!(model.membership(), Some(&current));
        assert!(model.pending().is_some());
    }
}
