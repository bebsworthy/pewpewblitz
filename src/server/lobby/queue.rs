//! Pure bounded lobby queue authority and privacy-safe aggregate telemetry.

use super::{LobbySession, MAX_AUTHENTICATED_LOBBY_SESSIONS, catalog::ResolvedLobbyCatalog};
use crate::{
    builds::{AcceptedBuildSummary, BuildCatalog, ResolvedMatchLoadout},
    combat::{FighterDefinitions, STANDARD_FIGHTER_DEFINITION, WeaponCatalog},
    lobby::{
        GameTypeId, QueueCancelCommand, QueueCommand, QueueCommandOutcome, QueueDecision,
        QueueJoinCommand, QueueMembership, QueuePoolRow, QueuePoolSnapshot, QueueRejection,
        QueueRequestId, QueueTicketId,
    },
};
use bevy::prelude::Resource;
use brawler_routing::{Capability, LobbySessionId, NetcodeClientId, PlayerId};
use std::collections::{BTreeMap, BTreeSet, VecDeque};

const COMMAND_TOKEN_CAPACITY: u8 = 4;
const COMMAND_REFILL_MILLIS: u64 = 1_000;
const IDENTICAL_RETRY_MILLIS: u64 = 10_000;
const EARLY_IDENTICAL_RETRY_LIMIT: u8 = 4;

/// Source of unpredictable nonzero ticket identities. Tests inject deterministic identities.
pub trait QueueTicketIdSource: Send + Sync {
    fn next(&mut self) -> Option<QueueTicketId>;
}

struct OsQueueTicketIdSource;

impl QueueTicketIdSource for OsQueueTicketIdSource {
    fn next(&mut self) -> Option<QueueTicketId> {
        for _ in 0..4 {
            let value = u128::from_le_bytes(
                Capability::generate().ok()?.into_bytes()[..16]
                    .try_into()
                    .ok()?,
            );
            if let Some(id) = QueueTicketId::new(value) {
                return Some(id);
            }
        }
        None
    }
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueueTicket {
    pub ticket_id: QueueTicketId,
    pub lobby_session_id: LobbySessionId,
    pub player_id: PlayerId,
    pub netcode_client_id: NetcodeClientId,
    pub catalog_revision: crate::lobby::CatalogRevision,
    pub game_type_id: GameTypeId,
    pub game_type_configuration_revision: u32,
    pub accepted_build: AcceptedBuildSummary,
    pub resolved_loadout: ResolvedMatchLoadout,
    pub build_snapshot: crate::profiles::MatchBuildSnapshotV3,
    pub admission_order: u64,
    pub admitted_at_pool_state_revision: u64,
    pub formation_eligible: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ReservedParticipant {
    pub ticket_id: QueueTicketId,
    pub team: u8,
}

#[derive(Clone, Debug, PartialEq)]
pub struct QueueReservation {
    pub reservation_id: crate::lobby::MatchReservationId,
    pub game_type_id: GameTypeId,
    pub map_preset_id: crate::map::MapPresetId,
    pub team_count: u8,
    pub players_per_team: u8,
    pub participants: Vec<ReservedParticipant>,
    pub handoff_ready: bool,
}

#[derive(Clone, Debug)]
struct LastCommand {
    request_id: QueueRequestId,
    command: QueueCommand,
    outcome: QueueCommandOutcome,
}

#[derive(Clone, Debug)]
struct QueueCommandMemory {
    last: Option<LastCommand>,
    pending_outcome: Option<LastCommand>,
    last_acknowledged_outcome_request_id: Option<QueueRequestId>,
    command_tokens: u8,
    command_token_refill_at: u64,
    rate_limit_notice_until: Option<u64>,
    identical_retry_not_before: Option<u64>,
    early_identical_retry_count: u8,
    active_ticket_id: Option<QueueTicketId>,
    last_cancelled: Option<(QueueTicketId, QueueCommandOutcome)>,
}

impl QueueCommandMemory {
    const fn new(now_millis: u64) -> Self {
        Self {
            last: None,
            pending_outcome: None,
            last_acknowledged_outcome_request_id: None,
            command_tokens: COMMAND_TOKEN_CAPACITY,
            command_token_refill_at: now_millis,
            rate_limit_notice_until: None,
            identical_retry_not_before: None,
            early_identical_retry_count: 0,
            active_ticket_id: None,
            last_cancelled: None,
        }
    }

    fn refill(&mut self, now_millis: u64) {
        if now_millis < self.command_token_refill_at {
            return;
        }
        let elapsed = now_millis.saturating_sub(self.command_token_refill_at);
        let tokens = elapsed / COMMAND_REFILL_MILLIS;
        if tokens > 0 {
            self.command_tokens = self
                .command_tokens
                .saturating_add(u8::try_from(tokens).unwrap_or(u8::MAX))
                .min(COMMAND_TOKEN_CAPACITY);
            self.command_token_refill_at = self
                .command_token_refill_at
                .saturating_add(tokens.saturating_mul(COMMAND_REFILL_MILLIS));
        }
        if self
            .rate_limit_notice_until
            .is_some_and(|deadline| now_millis >= deadline)
        {
            self.rate_limit_notice_until = None;
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
enum QueueCommandDisposition {
    #[default]
    Idle,
    OutcomeReady,
    Disconnect,
    SuppressedIdenticalRetry,
    EarlyIdenticalRetry,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct QueueCommandResult {
    disposition: QueueCommandDisposition,
    snapshot_changed: bool,
}

impl QueueCommandResult {
    #[must_use]
    pub const fn outcome_ready(self) -> bool {
        matches!(self.disposition, QueueCommandDisposition::OutcomeReady)
    }

    #[must_use]
    pub const fn disconnect(self) -> bool {
        matches!(self.disposition, QueueCommandDisposition::Disconnect)
    }

    #[must_use]
    pub const fn snapshot_changed(self) -> bool {
        self.snapshot_changed
    }

    #[cfg(test)]
    const fn suppressed_identical_retry(self) -> bool {
        matches!(
            self.disposition,
            QueueCommandDisposition::SuppressedIdenticalRetry
        )
    }

    #[cfg(test)]
    const fn early_identical_retry(self) -> bool {
        matches!(
            self.disposition,
            QueueCommandDisposition::EarlyIdenticalRetry
        )
    }
}

#[derive(Clone, Copy)]
struct OutcomeStorage {
    snapshot_changed: bool,
    remember_as_last: bool,
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct QueueTelemetry {
    pub current_tickets: u16,
    pub high_water_tickets: u16,
    pub current_pending_outcomes: u16,
    pub high_water_pending_outcomes: u16,
    pub admissions: u64,
    pub cancellations: u64,
    pub disconnect_removals: u64,
    pub build_rejections: u64,
    pub game_rejections: u64,
    pub membership_rejections: u64,
    pub unavailable_rejections: u64,
    pub rate_limit_notices: u64,
    pub protocol_abuse_disconnects: u64,
    pub suppressed_identical_retries: u64,
    pub early_identical_retries: u64,
    pub initial_snapshot_publications: u64,
    pub mutation_snapshot_publications: u64,
    pub refresh_snapshot_publications: u64,
}

impl QueueTelemetry {
    fn observe_counts(&mut self, tickets: usize, pending: usize) {
        self.current_tickets = u16::try_from(tickets).unwrap_or(u16::MAX);
        self.current_pending_outcomes = u16::try_from(pending).unwrap_or(u16::MAX);
        self.high_water_tickets = self.high_water_tickets.max(self.current_tickets);
        self.high_water_pending_outcomes = self
            .high_water_pending_outcomes
            .max(self.current_pending_outcomes);
    }

    fn increment(value: &mut u64) {
        *value = value.saturating_add(1);
    }
}

/// Bounded queue authority. Public aggregate revision starts at one and changes only with counts.
#[derive(Resource)]
pub struct QueueState {
    catalog_revision: crate::lobby::CatalogRevision,
    pool_rows: Vec<QueuePoolRow>,
    pools: BTreeMap<GameTypeId, VecDeque<QueueTicketId>>,
    tickets: BTreeMap<QueueTicketId, QueueTicket>,
    tickets_by_session: BTreeMap<LobbySessionId, QueueTicketId>,
    tickets_by_client: BTreeMap<NetcodeClientId, QueueTicketId>,
    reservations: BTreeMap<crate::lobby::MatchReservationId, QueueReservation>,
    reservation_by_ticket: BTreeMap<QueueTicketId, crate::lobby::MatchReservationId>,
    map_ordinals: BTreeMap<GameTypeId, u64>,
    memories: BTreeMap<LobbySessionId, QueueCommandMemory>,
    next_admission_order: u64,
    state_revision: u64,
    revision_exhausted: bool,
    ticket_namespace: Option<u64>,
    ticket_ids: Box<dyn QueueTicketIdSource>,
    telemetry: QueueTelemetry,
}

impl core::fmt::Debug for QueueState {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("QueueState")
            .field("pool_count", &self.pools.len())
            .field("ticket_count", &self.tickets.len())
            .field("state_revision", &self.state_revision)
            .finish_non_exhaustive()
    }
}

impl QueueState {
    #[must_use]
    pub(crate) fn new(catalog: &ResolvedLobbyCatalog) -> Self {
        Self::with_id_source(catalog, OsQueueTicketIdSource)
    }

    #[must_use]
    pub(crate) fn with_id_source<S>(catalog: &ResolvedLobbyCatalog, ticket_ids: S) -> Self
    where
        S: QueueTicketIdSource + 'static,
    {
        let pool_rows = catalog
            .game_types
            .iter()
            .map(|game| QueuePoolRow {
                game_type_id: game.id.clone(),
                game_type_configuration_revision: game.configuration_revision,
                queued: 0,
                formation_size: game
                    .team_count
                    .checked_mul(game.players_per_team)
                    .expect("validated lobby formation size fits u8"),
            })
            .collect::<Vec<_>>();
        let pools = pool_rows
            .iter()
            .map(|row| (row.game_type_id.clone(), VecDeque::new()))
            .collect();
        let map_ordinals = pool_rows
            .iter()
            .map(|row| (row.game_type_id.clone(), 0))
            .collect();
        Self {
            catalog_revision: catalog.revision,
            pool_rows,
            pools,
            tickets: BTreeMap::new(),
            tickets_by_session: BTreeMap::new(),
            tickets_by_client: BTreeMap::new(),
            reservations: BTreeMap::new(),
            reservation_by_ticket: BTreeMap::new(),
            map_ordinals,
            memories: BTreeMap::new(),
            next_admission_order: 1,
            state_revision: 1,
            revision_exhausted: false,
            ticket_namespace: None,
            ticket_ids: Box::new(ticket_ids),
            telemetry: QueueTelemetry::default(),
        }
    }

    #[must_use]
    pub fn state_revision(&self) -> u64 {
        self.state_revision
    }

    #[must_use]
    pub const fn revision_exhausted(&self) -> bool {
        self.revision_exhausted
    }

    #[must_use]
    pub fn ticket_count(&self) -> usize {
        self.tickets.len()
    }

    #[must_use]
    pub fn pending_outcome_count(&self) -> usize {
        self.memories
            .values()
            .filter(|memory| memory.pending_outcome.is_some())
            .count()
    }

    #[must_use]
    pub fn telemetry(&self) -> &QueueTelemetry {
        &self.telemetry
    }

    #[must_use]
    pub fn ticket(&self, id: QueueTicketId) -> Option<&QueueTicket> {
        self.tickets.get(&id)
    }

    #[must_use]
    pub fn ticket_for_session(&self, id: LobbySessionId) -> Option<&QueueTicket> {
        self.tickets_by_session
            .get(&id)
            .and_then(|ticket| self.tickets.get(ticket))
    }

    #[must_use]
    pub fn ticket_for_client(&self, id: NetcodeClientId) -> Option<&QueueTicket> {
        self.tickets_by_client
            .get(&id)
            .and_then(|ticket| self.tickets.get(ticket))
    }

    #[must_use]
    pub fn reservation(&self, id: crate::lobby::MatchReservationId) -> Option<&QueueReservation> {
        self.reservations.get(&id)
    }

    #[must_use]
    pub fn reservation_count(&self) -> usize {
        self.reservations.len()
    }

    pub fn complete_reservation(
        &mut self,
        reservation_id: crate::lobby::MatchReservationId,
    ) -> Vec<QueueTicket> {
        let Some(reservation) = self.reservations.get(&reservation_id).cloned() else {
            return Vec::new();
        };
        let tickets = reservation
            .participants
            .iter()
            .filter_map(|participant| self.tickets.get(&participant.ticket_id).cloned())
            .collect::<Vec<_>>();
        for ticket in &tickets {
            self.remove_ticket(ticket.ticket_id);
        }
        self.refresh_pool_rows();
        self.observe_counts();
        tickets
    }

    #[must_use]
    pub fn reservation_for_ticket(&self, ticket_id: QueueTicketId) -> Option<&QueueReservation> {
        self.reservation_by_ticket
            .get(&ticket_id)
            .and_then(|id| self.reservations.get(id))
    }

    #[must_use]
    pub fn pending_outcome(&self, id: LobbySessionId) -> Option<&QueueCommandOutcome> {
        self.memories
            .get(&id)?
            .pending_outcome
            .as_ref()
            .map(|pending| &pending.outcome)
    }

    #[must_use]
    pub fn snapshot(&self) -> QueuePoolSnapshot {
        QueuePoolSnapshot {
            catalog_revision: self.catalog_revision,
            state_revision: self.state_revision,
            formation_availability: crate::lobby::FormationAvailability::Available,
            pools: self.pool_rows.clone(),
        }
    }

    pub fn record_snapshot_publication(&mut self, kind: SnapshotPublication) {
        match kind {
            SnapshotPublication::Initial => {
                QueueTelemetry::increment(&mut self.telemetry.initial_snapshot_publications);
            }
            SnapshotPublication::Mutation => {
                QueueTelemetry::increment(&mut self.telemetry.mutation_snapshot_publications);
            }
            SnapshotPublication::Refresh => {
                QueueTelemetry::increment(&mut self.telemetry.refresh_snapshot_publications);
            }
        }
    }

    pub fn record_protocol_abuse(&mut self) {
        QueueTelemetry::increment(&mut self.telemetry.protocol_abuse_disconnects);
    }

    pub fn acknowledge(
        &mut self,
        session_id: LobbySessionId,
        request_id: QueueRequestId,
    ) -> QueueCommandResult {
        let Some(memory) = self.memories.get_mut(&session_id) else {
            return self.protocol_disconnect();
        };
        if memory
            .pending_outcome
            .as_ref()
            .is_some_and(|pending| pending.request_id == request_id)
        {
            let joined_ticket = memory
                .pending_outcome
                .as_ref()
                .and_then(|pending| match &pending.outcome.decision {
                    QueueDecision::Joined(membership) => Some(membership.ticket_id),
                    _ => None,
                });
            memory.pending_outcome = None;
            memory.last_acknowledged_outcome_request_id = Some(request_id);
            memory.identical_retry_not_before = None;
            memory.early_identical_retry_count = 0;
            if let Some(ticket_id) = joined_ticket
                && let Some(ticket) = self.tickets.get_mut(&ticket_id)
            {
                ticket.formation_eligible = true;
            }
            self.observe_counts();
            QueueCommandResult::default()
        } else if memory.last_acknowledged_outcome_request_id == Some(request_id) {
            QueueCommandResult::default()
        } else {
            self.protocol_disconnect()
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub fn command(
        &mut self,
        session: &LobbySession,
        request_id: QueueRequestId,
        command: QueueCommand,
        now_millis: u64,
        admitted_build: Option<crate::profiles::MatchBuildSnapshotV3>,
        builds: &BuildCatalog,
        weapons: &WeaponCatalog,
        fighters: &FighterDefinitions,
    ) -> QueueCommandResult {
        let session_id = session.lobby_session_id;
        self.memories
            .entry(session_id)
            .or_insert_with(|| QueueCommandMemory::new(now_millis));

        if let Some(result) = self.handle_duplicate(session_id, request_id, &command, now_millis) {
            return result;
        }

        {
            let memory = self.memories.get_mut(&session_id).expect("memory inserted");
            memory.refill(now_millis);
            if memory.pending_outcome.is_some() || memory.rate_limit_notice_until.is_some() {
                return self.protocol_disconnect();
            }
            if memory.command_tokens == 0 {
                let remainder = now_millis.saturating_sub(memory.command_token_refill_at);
                let delay = COMMAND_REFILL_MILLIS.saturating_sub(remainder.min(999));
                let retry_after_millis = u16::try_from(delay.clamp(1, 1_000)).unwrap_or(1_000);
                memory.rate_limit_notice_until = Some(now_millis.saturating_add(delay));
                QueueTelemetry::increment(&mut self.telemetry.rate_limit_notices);
                return self.store_outcome(
                    session_id,
                    request_id,
                    command,
                    QueueDecision::Rejected(QueueRejection::RateLimited { retry_after_millis }),
                    now_millis,
                    OutcomeStorage {
                        snapshot_changed: false,
                        remember_as_last: true,
                    },
                );
            }
            memory.command_tokens -= 1;
        }
        if self
            .memories
            .get(&session_id)
            .and_then(|memory| memory.last.as_ref())
            .is_some_and(|last| request_id < last.request_id)
        {
            return self.store_outcome(
                session_id,
                request_id,
                command,
                QueueDecision::Rejected(QueueRejection::StaleRequest),
                now_millis,
                OutcomeStorage {
                    snapshot_changed: false,
                    remember_as_last: false,
                },
            );
        }

        let (decision, changed) = match &command {
            QueueCommand::Join(join) => {
                self.join(session, join, admitted_build, builds, weapons, fighters)
            }
            QueueCommand::Cancel(cancel) => self.cancel(session, *cancel),
        };
        self.store_outcome(
            session_id,
            request_id,
            command,
            decision,
            now_millis,
            OutcomeStorage {
                snapshot_changed: changed,
                remember_as_last: true,
            },
        )
    }

    fn handle_duplicate(
        &mut self,
        session_id: LobbySessionId,
        request_id: QueueRequestId,
        command: &QueueCommand,
        now_millis: u64,
    ) -> Option<QueueCommandResult> {
        let memory = self.memories.get_mut(&session_id)?;
        if let Some(pending) = memory.pending_outcome.as_ref() {
            if request_id != pending.request_id {
                return None;
            }
            if command != &pending.command {
                return Some(self.protocol_disconnect());
            }
            let deadline = memory
                .identical_retry_not_before
                .unwrap_or(now_millis.saturating_add(IDENTICAL_RETRY_MILLIS));
            if now_millis >= deadline {
                memory.identical_retry_not_before =
                    Some(now_millis.saturating_add(IDENTICAL_RETRY_MILLIS));
                memory.early_identical_retry_count = 0;
                QueueTelemetry::increment(&mut self.telemetry.suppressed_identical_retries);
                return Some(QueueCommandResult {
                    disposition: QueueCommandDisposition::SuppressedIdenticalRetry,
                    ..QueueCommandResult::default()
                });
            }
            memory.early_identical_retry_count =
                memory.early_identical_retry_count.saturating_add(1);
            QueueTelemetry::increment(&mut self.telemetry.early_identical_retries);
            if memory.early_identical_retry_count >= EARLY_IDENTICAL_RETRY_LIMIT {
                return Some(self.protocol_disconnect());
            }
            return Some(QueueCommandResult {
                disposition: QueueCommandDisposition::EarlyIdenticalRetry,
                ..QueueCommandResult::default()
            });
        }
        let last = memory.last.as_ref()?;
        if request_id != last.request_id {
            return None;
        }
        if command != &last.command {
            return Some(self.protocol_disconnect());
        }
        memory.pending_outcome = Some(last.clone());
        memory.identical_retry_not_before = Some(now_millis.saturating_add(IDENTICAL_RETRY_MILLIS));
        self.observe_counts();
        Some(QueueCommandResult {
            disposition: QueueCommandDisposition::OutcomeReady,
            ..QueueCommandResult::default()
        })
    }

    #[allow(clippy::too_many_lines)]
    fn join(
        &mut self,
        session: &LobbySession,
        command: &QueueJoinCommand,
        admitted_build: Option<crate::profiles::MatchBuildSnapshotV3>,
        builds: &BuildCatalog,
        weapons: &WeaponCatalog,
        fighters: &FighterDefinitions,
    ) -> (QueueDecision, bool) {
        if let Err(rejection) = self.validate_join_target(command) {
            QueueTelemetry::increment(&mut self.telemetry.game_rejections);
            return (QueueDecision::Rejected(rejection), false);
        }

        if let Some(ticket) = self.ticket_for_session(session.lobby_session_id) {
            if ticket.catalog_revision == command.catalog_revision
                && ticket.game_type_id == command.game_type_id
                && ticket.game_type_configuration_revision
                    == command.game_type_configuration_revision
                && ticket.build_snapshot.brawler_id == command.brawler_id
                && ticket.build_snapshot.brawler_revision == command.brawler_revision
            {
                return (QueueDecision::Joined(membership_from_ticket(ticket)), false);
            }
            QueueTelemetry::increment(&mut self.telemetry.membership_rejections);
            return (
                QueueDecision::Rejected(QueueRejection::MustCancelFirst),
                false,
            );
        }
        if self.tickets.len() >= MAX_AUTHENTICATED_LOBBY_SESSIONS {
            QueueTelemetry::increment(&mut self.telemetry.unavailable_rejections);
            return (
                QueueDecision::Rejected(QueueRejection::TemporarilyUnavailable),
                false,
            );
        }
        let Some(build_snapshot) = admitted_build.filter(|snapshot| {
            snapshot.brawler_id == command.brawler_id
                && snapshot.brawler_revision == command.brawler_revision
        }) else {
            QueueTelemetry::increment(&mut self.telemetry.build_rejections);
            return (
                QueueDecision::Rejected(QueueRejection::InternalBuildResolution),
                false,
            );
        };
        let Some(fighter) = fighters.get(STANDARD_FIGHTER_DEFINITION) else {
            return (
                QueueDecision::Rejected(QueueRejection::TemporarilyUnavailable),
                false,
            );
        };
        let Ok(resolved) = build_snapshot.resolve(builds, weapons, fighter) else {
            QueueTelemetry::increment(&mut self.telemetry.build_rejections);
            return (
                QueueDecision::Rejected(QueueRejection::InternalBuildResolution),
                false,
            );
        };
        let admission_order = self.next_admission_order;
        let Some(next_admission_order) = admission_order.checked_add(1) else {
            QueueTelemetry::increment(&mut self.telemetry.unavailable_rejections);
            return (
                QueueDecision::Rejected(QueueRejection::TemporarilyUnavailable),
                false,
            );
        };
        let Some(ticket_id) = self.fresh_ticket_id(admission_order) else {
            QueueTelemetry::increment(&mut self.telemetry.unavailable_rejections);
            return (
                QueueDecision::Rejected(QueueRejection::TemporarilyUnavailable),
                false,
            );
        };
        let Some(revision) = self.state_revision.checked_add(1) else {
            QueueTelemetry::increment(&mut self.telemetry.unavailable_rejections);
            return (
                QueueDecision::Rejected(QueueRejection::TemporarilyUnavailable),
                false,
            );
        };
        let accepted_build = AcceptedBuildSummary {
            canonical_recipe: crate::builds::BrawlerBuildRecipe {
                weapon: crate::builds::WeaponChoice::Preset(crate::combat::WeaponPresetId(
                    build_snapshot.weapon_base_id.0,
                )),
                ultimate: build_snapshot.ultimate_id,
                passives: build_snapshot.passive_ids,
            },
            identity: resolved.identity,
            total_points: resolved.total_points,
        };
        let ticket = QueueTicket {
            ticket_id,
            lobby_session_id: session.lobby_session_id,
            player_id: session.player_id,
            netcode_client_id: session.netcode_client_id,
            catalog_revision: command.catalog_revision,
            game_type_id: command.game_type_id.clone(),
            game_type_configuration_revision: command.game_type_configuration_revision,
            accepted_build,
            resolved_loadout: resolved,
            build_snapshot,
            admission_order,
            admitted_at_pool_state_revision: revision,
            formation_eligible: false,
        };
        self.next_admission_order = next_admission_order;
        self.state_revision = revision;
        self.pools
            .get_mut(&ticket.game_type_id)
            .expect("validated game owns pool")
            .push_back(ticket_id);
        self.tickets_by_session
            .insert(ticket.lobby_session_id, ticket_id);
        self.tickets_by_client
            .insert(ticket.netcode_client_id, ticket_id);
        self.memories
            .get_mut(&ticket.lobby_session_id)
            .expect("command memory exists")
            .active_ticket_id = Some(ticket_id);
        let membership = membership_from_ticket(&ticket);
        self.tickets.insert(ticket_id, ticket);
        self.refresh_pool_rows();
        QueueTelemetry::increment(&mut self.telemetry.admissions);
        (QueueDecision::Joined(membership), true)
    }

    fn validate_join_target(&self, command: &QueueJoinCommand) -> Result<(), QueueRejection> {
        if command.catalog_revision != self.catalog_revision {
            return Err(QueueRejection::StaleCatalog);
        }
        let row = self
            .pool_rows
            .iter()
            .find(|row| row.game_type_id == command.game_type_id)
            .ok_or(QueueRejection::UnknownGameType)?;
        if row.game_type_configuration_revision != command.game_type_configuration_revision {
            return Err(QueueRejection::StaleGameConfiguration);
        }
        Ok(())
    }

    fn cancel(
        &mut self,
        session: &LobbySession,
        command: QueueCancelCommand,
    ) -> (QueueDecision, bool) {
        let memory = self
            .memories
            .get(&session.lobby_session_id)
            .expect("command memory exists");
        if let Some((ticket_id, outcome)) = memory.last_cancelled.as_ref()
            && *ticket_id == command.ticket_id
        {
            return (outcome.decision.clone(), false);
        }
        let Some(owned) = self
            .tickets_by_session
            .get(&session.lobby_session_id)
            .copied()
        else {
            QueueTelemetry::increment(&mut self.telemetry.membership_rejections);
            return (
                QueueDecision::Rejected(QueueRejection::TicketMismatch),
                false,
            );
        };
        if owned != command.ticket_id {
            QueueTelemetry::increment(&mut self.telemetry.membership_rejections);
            return (
                QueueDecision::Rejected(QueueRejection::TicketMismatch),
                false,
            );
        }
        let Some(revision) = self.state_revision.checked_add(1) else {
            QueueTelemetry::increment(&mut self.telemetry.unavailable_rejections);
            return (
                QueueDecision::Rejected(QueueRejection::TemporarilyUnavailable),
                false,
            );
        };
        self.remove_ticket(owned);
        self.state_revision = revision;
        self.refresh_pool_rows();
        QueueTelemetry::increment(&mut self.telemetry.cancellations);
        (
            QueueDecision::Cancelled {
                ticket_id: owned,
                resulting_pool_state_revision: revision,
            },
            true,
        )
    }

    fn store_outcome(
        &mut self,
        session_id: LobbySessionId,
        request_id: QueueRequestId,
        command: QueueCommand,
        decision: QueueDecision,
        now_millis: u64,
        storage: OutcomeStorage,
    ) -> QueueCommandResult {
        let outcome = QueueCommandOutcome {
            request_id,
            decision,
        };
        if postcard::to_allocvec(&outcome).map_or(true, |bytes| {
            bytes.len() > crate::lobby::MAX_QUEUE_OUTCOME_BYTES
        }) {
            return self.protocol_disconnect();
        }
        let memory = self
            .memories
            .get_mut(&session_id)
            .expect("command memory exists");
        if let QueueDecision::Cancelled { ticket_id, .. } = &outcome.decision {
            memory.last_cancelled = Some((*ticket_id, outcome.clone()));
        }
        let retained = LastCommand {
            request_id,
            command,
            outcome,
        };
        if storage.remember_as_last {
            memory.last = Some(retained.clone());
        }
        memory.pending_outcome = Some(retained);
        memory.identical_retry_not_before = Some(now_millis.saturating_add(IDENTICAL_RETRY_MILLIS));
        memory.early_identical_retry_count = 0;
        self.observe_counts();
        QueueCommandResult {
            disposition: QueueCommandDisposition::OutcomeReady,
            snapshot_changed: storage.snapshot_changed,
        }
    }

    /// Remove queue state before the owning authenticated lobby session disappears.
    pub fn remove_session(&mut self, session_id: LobbySessionId) -> bool {
        let ticket = self.tickets_by_session.get(&session_id).copied();
        let next_revision = ticket.and_then(|_| self.state_revision.checked_add(1));
        let removed = ticket.is_some_and(|ticket| {
            self.remove_ticket(ticket);
            true
        });
        self.memories.remove(&session_id);
        if removed {
            self.refresh_pool_rows();
            QueueTelemetry::increment(&mut self.telemetry.disconnect_removals);
            if let Some(revision) = next_revision {
                self.state_revision = revision;
            } else {
                // Cleanup remains mandatory, but the worker can no longer publish a distinct
                // aggregate revision. The Bevy adapter terminates this authority generation
                // rather than publishing changed contents under an existing revision.
                self.revision_exhausted = true;
            }
        }
        self.observe_counts();
        removed
    }

    fn remove_ticket(&mut self, ticket_id: QueueTicketId) {
        let Some(ticket) = self.tickets.remove(&ticket_id) else {
            return;
        };
        self.tickets_by_session.remove(&ticket.lobby_session_id);
        self.tickets_by_client.remove(&ticket.netcode_client_id);
        if let Some(reservation_id) = self.reservation_by_ticket.remove(&ticket_id)
            && let Some(reservation) = self.reservations.get_mut(&reservation_id)
        {
            reservation
                .participants
                .retain(|participant| participant.ticket_id != ticket_id);
            if reservation.participants.is_empty() {
                self.reservations.remove(&reservation_id);
            }
        }
        if let Some(pool) = self.pools.get_mut(&ticket.game_type_id)
            && let Some(index) = pool.iter().position(|candidate| *candidate == ticket_id)
        {
            pool.remove(index);
        }
        if let Some(memory) = self.memories.get_mut(&ticket.lobby_session_id) {
            memory.active_ticket_id = None;
        }
    }

    fn fresh_ticket_id(&mut self, admission_order: u64) -> Option<QueueTicketId> {
        let namespace = if let Some(namespace) = self.ticket_namespace {
            namespace
        } else {
            let seed = self.ticket_ids.next()?.get();
            let high = u64::try_from(seed >> 64).ok()?;
            let low = u64::try_from(seed & u128::from(u64::MAX)).ok()?;
            let namespace = (high ^ low).max(1);
            self.ticket_namespace = Some(namespace);
            namespace
        };
        QueueTicketId::new((u128::from(namespace) << 64) | u128::from(admission_order))
    }

    fn refresh_pool_rows(&mut self) {
        for row in &mut self.pool_rows {
            row.queued = self
                .pools
                .get(&row.game_type_id)
                .map_or(0, |pool| u16::try_from(pool.len()).unwrap_or(u16::MAX));
        }
        self.observe_counts();
    }

    fn observe_counts(&mut self) {
        self.telemetry
            .observe_counts(self.tickets.len(), self.pending_outcome_count());
    }

    fn protocol_disconnect(&mut self) -> QueueCommandResult {
        QueueTelemetry::increment(&mut self.telemetry.protocol_abuse_disconnects);
        QueueCommandResult {
            disposition: QueueCommandDisposition::Disconnect,
            ..QueueCommandResult::default()
        }
    }

    #[must_use]
    pub fn indexes_are_valid(&self) -> bool {
        if self.tickets.len() != self.tickets_by_session.len()
            || self.tickets.len() != self.tickets_by_client.len()
            || self.tickets.len() > MAX_AUTHENTICATED_LOBBY_SESSIONS
        {
            return false;
        }
        let pooled: BTreeSet<_> = self.pools.values().flatten().copied().collect();
        let reserved: BTreeSet<_> = self.reservation_by_ticket.keys().copied().collect();
        pooled.is_disjoint(&reserved)
            && pooled.len().saturating_add(reserved.len()) == self.tickets.len()
            && self
                .tickets
                .keys()
                .all(|id| pooled.contains(id) || reserved.contains(id))
            && self.tickets.values().all(|ticket| {
                self.tickets_by_session.get(&ticket.lobby_session_id) == Some(&ticket.ticket_id)
                    && self.tickets_by_client.get(&ticket.netcode_client_id)
                        == Some(&ticket.ticket_id)
            })
    }

    /// Atomically reserve the oldest complete eligible roster across catalog pools. Queued
    /// overflow stays ordered and catalog order breaks an otherwise impossible identity tie.
    pub(crate) fn reserve_oldest_exact(
        &mut self,
        catalog: &ResolvedLobbyCatalog,
        reservation_id: crate::lobby::MatchReservationId,
        live_sessions: &BTreeSet<LobbySessionId>,
    ) -> Option<QueueReservation> {
        if !self.reservations.is_empty() || self.reservations.contains_key(&reservation_id) {
            return None;
        }
        let mut candidate = None;
        for (catalog_index, game) in catalog.game_types.iter().enumerate() {
            let formation_size = usize::from(game.team_count.checked_mul(game.players_per_team)?);
            let pool = self.pools.get(&game.id)?;
            let selected = pool
                .iter()
                .filter(|id| {
                    self.tickets.get(id).is_some_and(|ticket| {
                        ticket.formation_eligible
                            && live_sessions.contains(&ticket.lobby_session_id)
                    })
                })
                .take(formation_size)
                .copied()
                .collect::<Vec<_>>();
            if selected.len() != formation_size {
                continue;
            }
            let first = self.tickets.get(selected.first()?)?;
            let key = (first.admission_order, first.ticket_id, catalog_index);
            if candidate
                .as_ref()
                .is_none_or(|(current, _, _)| key < *current)
            {
                candidate = Some((key, game, selected));
            }
        }
        let (_, game, selected) = candidate?;
        let next_revision = self.state_revision.checked_add(1)?;
        let ordinal = *self.map_ordinals.get(&game.id).unwrap_or(&0);
        let map_index = usize::try_from(ordinal).unwrap_or(0) % game.map_preset_ids.len();
        let selected_set: BTreeSet<_> = selected.iter().copied().collect();
        self.pools
            .get_mut(&game.id)
            .expect("catalog pool exists")
            .retain(|id| !selected_set.contains(id));
        let participants = selected
            .iter()
            .enumerate()
            .map(|(index, ticket_id)| ReservedParticipant {
                ticket_id: *ticket_id,
                team: u8::try_from(index % usize::from(game.team_count)).expect("team bound"),
            })
            .collect::<Vec<_>>();
        let reservation = QueueReservation {
            reservation_id,
            game_type_id: game.id.clone(),
            map_preset_id: game.map_preset_ids[map_index],
            team_count: game.team_count,
            players_per_team: game.players_per_team,
            participants,
            handoff_ready: false,
        };
        for participant in &reservation.participants {
            self.reservation_by_ticket
                .insert(participant.ticket_id, reservation_id);
        }
        self.reservations
            .insert(reservation_id, reservation.clone());
        self.state_revision = next_revision;
        self.refresh_pool_rows();
        Some(reservation)
    }

    pub fn mark_reservation_handoff_ready(
        &mut self,
        reservation_id: crate::lobby::MatchReservationId,
    ) -> bool {
        let Some(reservation) = self.reservations.get_mut(&reservation_id) else {
            return false;
        };
        if reservation.handoff_ready {
            return false;
        }
        reservation.handoff_ready = true;
        if let Some(ordinal) = self.map_ordinals.get_mut(&reservation.game_type_id) {
            *ordinal = ordinal.saturating_add(1);
        }
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SnapshotPublication {
    Initial,
    Mutation,
    Refresh,
}

fn membership_from_ticket(ticket: &QueueTicket) -> QueueMembership {
    QueueMembership {
        ticket_id: ticket.ticket_id,
        catalog_revision: ticket.catalog_revision,
        game_type_id: ticket.game_type_id.clone(),
        game_type_configuration_revision: ticket.game_type_configuration_revision,
        brawler_id: ticket.build_snapshot.brawler_id,
        brawler_revision: ticket.build_snapshot.brawler_revision,
        accepted_build: ticket.accepted_build,
        admitted_at_pool_state_revision: ticket.admitted_at_pool_state_revision,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brawler_routing::{PeerId, RouteId};

    struct SequentialTicketIds(u128);

    impl QueueTicketIdSource for SequentialTicketIds {
        fn next(&mut self) -> Option<QueueTicketId> {
            let id = QueueTicketId::new(self.0)?;
            self.0 = self.0.checked_add(1)?;
            Some(id)
        }
    }

    struct FixedTicketSeed(u128);

    impl QueueTicketIdSource for FixedTicketSeed {
        fn next(&mut self) -> Option<QueueTicketId> {
            QueueTicketId::new(self.0)
        }
    }

    fn catalog() -> ResolvedLobbyCatalog {
        super::super::resolve_operator_catalog(include_bytes!(
            "../../../config/server/game-types.ron"
        ))
        .expect("development catalog resolves")
    }

    fn session(value: u64) -> LobbySession {
        LobbySession {
            lobby_session_id: LobbySessionId::new(u128::from(value)).unwrap(),
            player_id: PlayerId::new(value).unwrap(),
            network_entity_id: crate::protocol::NetworkEntityId(value),
            netcode_client_id: NetcodeClientId::new(value).unwrap(),
            route_id: RouteId::new(u128::from(value)).unwrap(),
            peer_id: PeerId::new(u128::from(value)).unwrap(),
            team: 0,
            build: super::super::LobbyBuildIdentity {
                source_build_preset: Some(1),
                recipe_fingerprint: 1,
                build_revision: 1,
                snapshot: super::super::default_build_identity().unwrap().snapshot,
            },
        }
    }

    fn join(catalog: &ResolvedLobbyCatalog, preset: u16) -> QueueCommand {
        join_pool(catalog, 0, preset)
    }

    fn join_pool(catalog: &ResolvedLobbyCatalog, pool: usize, preset: u16) -> QueueCommand {
        let game = &catalog.game_types[pool];
        QueueCommand::Join(QueueJoinCommand {
            catalog_revision: catalog.revision,
            game_type_id: game.id.clone(),
            game_type_configuration_revision: game.configuration_revision,
            brawler_id: crate::profiles::SavedBrawlerId::new(u128::from(preset)).unwrap(),
            brawler_revision: crate::profiles::ProfileRevision::INITIAL,
        })
    }

    fn content() -> (BuildCatalog, WeaponCatalog, FighterDefinitions) {
        (
            BuildCatalog::embedded().unwrap(),
            WeaponCatalog::embedded().unwrap(),
            FighterDefinitions::default(),
        )
    }

    #[allow(clippy::large_types_passed_by_value)]
    fn submit(
        queue: &mut QueueState,
        session: LobbySession,
        request: u64,
        command: QueueCommand,
        now: u64,
    ) -> QueueCommandResult {
        let (builds, weapons, fighters) = content();
        let admitted = match &command {
            QueueCommand::Join(join) => {
                let preset = u16::try_from(join.brawler_id.get()).unwrap_or(0);
                if !(1..=4).contains(&preset) {
                    return queue.command(
                        &session,
                        QueueRequestId::new(request).unwrap(),
                        command,
                        now,
                        None,
                        &builds,
                        &weapons,
                        &fighters,
                    );
                }
                let definition = builds.preset(crate::builds::BuildPresetId(preset)).unwrap();
                let weapon_base_id = match definition.recipe.weapon {
                    crate::builds::WeaponChoice::Preset(id) => crate::profiles::WeaponBaseId(id.0),
                    crate::builds::WeaponChoice::CustomPulse { .. } => {
                        crate::profiles::WeaponBaseId(1)
                    }
                };
                let brawler = crate::profiles::SavedBrawler {
                    id: join.brawler_id,
                    creation_ordinal: 1,
                    name: "Test Brawler".into(),
                    fighter_profile_id: crate::profiles::FighterProfileId(1),
                    weapon_base_id,
                    ultimate_id: definition.recipe.ultimate,
                    passive_ids: [
                        crate::builds::PassiveDefinitionId(3),
                        crate::builds::PassiveDefinitionId(4),
                    ],
                    equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
                    revision: join.brawler_revision,
                };
                crate::profiles::MatchBuildSnapshotV3::from_brawler(
                    &brawler,
                    &builds,
                    &weapons,
                    fighters.get(STANDARD_FIGHTER_DEFINITION).unwrap(),
                )
                .ok()
            }
            QueueCommand::Cancel(_) => None,
        };
        queue.command(
            &session,
            QueueRequestId::new(request).unwrap(),
            command,
            now,
            admitted,
            &builds,
            &weapons,
            &fighters,
        )
    }

    #[test]
    fn join_ack_cancel_is_atomic_revisioned_and_bijective() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(10));
        let player = session(1);
        let result = submit(&mut queue, player, 1, join(&catalog, 1), 0);
        assert_eq!(
            result,
            QueueCommandResult {
                disposition: QueueCommandDisposition::OutcomeReady,
                snapshot_changed: true,
            }
        );
        assert_eq!(queue.ticket_count(), 1);
        assert_eq!(queue.state_revision(), 2);
        assert!(queue.indexes_are_valid());
        let membership = match &queue
            .pending_outcome(player.lobby_session_id)
            .unwrap()
            .decision
        {
            QueueDecision::Joined(membership) => membership.clone(),
            decision => panic!("unexpected decision: {decision:?}"),
        };
        assert_eq!(membership.admitted_at_pool_state_revision, 2);
        assert_eq!(
            membership.accepted_build.identity.source_build_preset_id,
            None
        );
        assert_eq!(queue.snapshot().pools[0].queued, 1);

        assert_eq!(
            queue.acknowledge(player.lobby_session_id, QueueRequestId::new(1).unwrap()),
            QueueCommandResult::default()
        );
        let result = submit(
            &mut queue,
            player,
            2,
            QueueCommand::Cancel(QueueCancelCommand {
                ticket_id: membership.ticket_id,
            }),
            0,
        );
        assert!(result.outcome_ready() && result.snapshot_changed());
        assert_eq!(queue.ticket_count(), 0);
        assert_eq!(queue.state_revision(), 3);
        assert_eq!(queue.snapshot().pools[0].queued, 0);
        assert!(queue.indexes_are_valid());
    }

    #[test]
    fn equivalent_new_join_reuses_ticket_and_admission_revision() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(10));
        let player = session(1);
        submit(&mut queue, player, 1, join(&catalog, 2), 0);
        let first = queue.ticket_for_session(player.lobby_session_id).unwrap();
        let first_id = first.ticket_id;
        let first_revision = first.admitted_at_pool_state_revision;
        queue.acknowledge(player.lobby_session_id, QueueRequestId::new(1).unwrap());
        let result = submit(&mut queue, player, 2, join(&catalog, 2), 1_000);
        assert!(result.outcome_ready() && !result.snapshot_changed());
        let second = queue.ticket_for_session(player.lobby_session_id).unwrap();
        assert_eq!(second.ticket_id, first_id);
        assert_eq!(second.admitted_at_pool_state_revision, first_revision);
        assert_eq!(queue.state_revision(), first_revision);
    }

    #[test]
    fn retired_ticket_identity_is_not_reused_within_one_worker_generation() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, FixedTicketSeed(10));
        let player = session(1);
        submit(&mut queue, player, 1, join(&catalog, 1), 0);
        let first = queue
            .ticket_for_session(player.lobby_session_id)
            .unwrap()
            .ticket_id;
        queue.acknowledge(player.lobby_session_id, QueueRequestId::new(1).unwrap());
        submit(
            &mut queue,
            player,
            2,
            QueueCommand::Cancel(QueueCancelCommand { ticket_id: first }),
            1_000,
        );
        queue.acknowledge(player.lobby_session_id, QueueRequestId::new(2).unwrap());

        submit(&mut queue, player, 3, join(&catalog, 1), 2_000);
        let second = queue
            .ticket_for_session(player.lobby_session_id)
            .unwrap()
            .ticket_id;
        assert_ne!(second, first);
        queue.acknowledge(player.lobby_session_id, QueueRequestId::new(3).unwrap());
        let result = submit(
            &mut queue,
            player,
            4,
            QueueCommand::Cancel(QueueCancelCommand { ticket_id: second }),
            3_000,
        );
        assert!(result.outcome_ready() && result.snapshot_changed());
        assert_eq!(queue.ticket_count(), 0);
    }

    #[test]
    fn sustained_join_cancel_churn_keeps_unique_ids_and_bounded_state() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, FixedTicketSeed(10));
        let player = session(1);
        let mut issued = BTreeSet::new();
        let mut request = 1_u64;
        for cycle in 0..128_u64 {
            submit(
                &mut queue,
                player,
                request,
                join(&catalog, 1),
                cycle.saturating_mul(2_000),
            );
            let ticket = queue
                .ticket_for_session(player.lobby_session_id)
                .unwrap()
                .ticket_id;
            assert!(issued.insert(ticket));
            queue.acknowledge(
                player.lobby_session_id,
                QueueRequestId::new(request).unwrap(),
            );
            request += 1;
            submit(
                &mut queue,
                player,
                request,
                QueueCommand::Cancel(QueueCancelCommand { ticket_id: ticket }),
                cycle.saturating_mul(2_000).saturating_add(1_000),
            );
            queue.acknowledge(
                player.lobby_session_id,
                QueueRequestId::new(request).unwrap(),
            );
            request += 1;
            assert_eq!(queue.ticket_count(), 0);
            assert_eq!(queue.pending_outcome_count(), 0);
            assert!(queue.indexes_are_valid());
        }
        assert_eq!(issued.len(), 128);
    }

    #[test]
    fn pending_identical_retries_are_suppressed_and_early_abuse_disconnects() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(10));
        let player = session(1);
        let command = join(&catalog, 1);
        submit(&mut queue, player, 1, command.clone(), 0);
        for attempt in 1..4 {
            let result = submit(&mut queue, player, 1, command.clone(), attempt);
            assert!(result.early_identical_retry() && !result.disconnect());
        }
        let result = submit(&mut queue, player, 1, command, 4);
        assert!(result.disconnect());
        assert_eq!(
            queue.ticket_count(),
            1,
            "abuse handling does not mutate membership directly"
        );
        assert_eq!(queue.telemetry().early_identical_retries, 4);
    }

    #[test]
    fn ten_second_identical_retry_is_token_and_wire_copy_neutral() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(10));
        let player = session(1);
        let command = join(&catalog, 1);
        submit(&mut queue, player, 1, command.clone(), 0);
        let result = submit(&mut queue, player, 1, command, 10_000);
        assert!(result.suppressed_identical_retry());
        assert!(!result.outcome_ready());
        assert_eq!(queue.ticket_count(), 1);
        assert_eq!(queue.telemetry().suppressed_identical_retries, 1);
    }

    #[test]
    fn different_command_while_outcome_pending_is_protocol_abuse() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(10));
        let player = session(1);
        submit(&mut queue, player, 1, join(&catalog, 1), 0);
        let result = submit(&mut queue, player, 2, join(&catalog, 2), 0);
        assert!(result.disconnect());
        assert_eq!(queue.ticket_count(), 1);
        assert_eq!(queue.pending_outcome_count(), 1);
    }

    #[test]
    fn stale_outcome_does_not_forget_the_highest_request_or_its_exact_replay() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(10));
        let player = session(1);
        let command = join(&catalog, 1);
        submit(&mut queue, player, 5, command.clone(), 0);
        let joined = queue
            .pending_outcome(player.lobby_session_id)
            .unwrap()
            .clone();
        queue.acknowledge(player.lobby_session_id, QueueRequestId::new(5).unwrap());

        submit(&mut queue, player, 3, command.clone(), 0);
        assert!(matches!(
            queue
                .pending_outcome(player.lobby_session_id)
                .unwrap()
                .decision,
            QueueDecision::Rejected(QueueRejection::StaleRequest)
        ));
        queue.acknowledge(player.lobby_session_id, QueueRequestId::new(3).unwrap());

        let replay = submit(&mut queue, player, 5, command, 0);
        assert!(replay.outcome_ready() && !replay.snapshot_changed());
        assert_eq!(
            queue.pending_outcome(player.lobby_session_id),
            Some(&joined)
        );
        assert_eq!(queue.ticket_count(), 1);
        assert_eq!(queue.state_revision(), 2);
    }

    #[test]
    fn disconnect_removes_exact_ticket_and_updates_aggregate() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(10));
        let first = session(1);
        let second = session(2);
        submit(&mut queue, first, 1, join(&catalog, 1), 0);
        submit(&mut queue, second, 1, join(&catalog, 1), 0);
        assert_eq!(queue.snapshot().pools[0].queued, 2);
        assert!(queue.remove_session(first.lobby_session_id));
        assert_eq!(queue.snapshot().pools[0].queued, 1);
        assert!(queue.ticket_for_session(second.lobby_session_id).is_some());
        assert!(queue.indexes_are_valid());
        assert_eq!(queue.telemetry().disconnect_removals, 1);
    }

    #[test]
    fn middle_cancellation_preserves_fifo_and_cross_pool_snapshot_is_aggregate_only() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(10));
        let first = session(1);
        let middle = session(2);
        let last = session(3);
        let other_pool = session(4);
        for player in [first, middle, last] {
            submit(&mut queue, player, 1, join(&catalog, 1), 0);
        }
        submit(&mut queue, other_pool, 1, join_pool(&catalog, 1, 2), 0);
        let first_ticket = queue.ticket_for_session(first.lobby_session_id).unwrap();
        let first_id = first_ticket.ticket_id;
        let first_order = first_ticket.admission_order;
        let middle_id = queue
            .ticket_for_session(middle.lobby_session_id)
            .unwrap()
            .ticket_id;
        let last_ticket = queue.ticket_for_session(last.lobby_session_id).unwrap();
        let last_id = last_ticket.ticket_id;
        let last_order = last_ticket.admission_order;
        queue.acknowledge(middle.lobby_session_id, QueueRequestId::new(1).unwrap());
        submit(
            &mut queue,
            middle,
            2,
            QueueCommand::Cancel(QueueCancelCommand {
                ticket_id: middle_id,
            }),
            1_000,
        );

        assert_eq!(
            queue.pools.get(&catalog.game_types[0].id).unwrap(),
            &VecDeque::from([first_id, last_id])
        );
        assert!(first_order < last_order);
        assert_eq!(queue.snapshot().pools[0].queued, 2);
        assert_eq!(queue.snapshot().pools[1].queued, 1);
        assert!(queue.indexes_are_valid());
    }

    #[test]
    fn invalid_build_never_mutates_queue() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(10));
        let player = session(1);
        let result = submit(&mut queue, player, 1, join(&catalog, 999), 0);
        assert!(result.outcome_ready() && !result.snapshot_changed());
        assert_eq!(queue.ticket_count(), 0);
        assert_eq!(queue.state_revision(), 1);
        assert!(matches!(
            queue
                .pending_outcome(player.lobby_session_id)
                .unwrap()
                .decision,
            QueueDecision::Rejected(QueueRejection::InternalBuildResolution)
        ));
    }

    #[test]
    fn saved_brawler_admission_does_not_apply_the_legacy_point_budget() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(10));
        submit(&mut queue, session(1), 1, join(&catalog, 1), 0);
        assert!(matches!(
            queue
                .pending_outcome(session(1).lobby_session_id)
                .unwrap()
                .decision,
            QueueDecision::Joined(_)
        ));
        assert_eq!(queue.ticket_count(), 1);
    }

    #[test]
    fn outcome_and_snapshot_wire_shapes_stay_bounded_and_private() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(10));
        let player = session(1);
        submit(&mut queue, player, 1, join(&catalog, 1), 0);
        let outcome = queue.pending_outcome(player.lobby_session_id).unwrap();
        assert!(
            postcard::to_allocvec(outcome).unwrap().len() <= crate::lobby::MAX_QUEUE_OUTCOME_BYTES
        );
        assert_eq!(queue.snapshot().pools.len(), catalog.game_types.len());
        assert_eq!(queue.telemetry().current_tickets, 1);
        assert_eq!(queue.telemetry().high_water_tickets, 1);
    }

    #[test]
    fn first_semantic_rate_overflow_is_one_fail_soft_notice_then_abuse_disconnects() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(10));
        let player = session(1);
        let mut ticket = None;
        for request in 1..=4 {
            let command = if let Some(ticket_id) = ticket.take() {
                QueueCommand::Cancel(QueueCancelCommand { ticket_id })
            } else {
                join(&catalog, 1)
            };
            let result = submit(&mut queue, player, request, command, 0);
            assert!(result.outcome_ready() && !result.disconnect());
            if let Some(active) = queue.ticket_for_session(player.lobby_session_id) {
                ticket = Some(active.ticket_id);
            }
            queue.acknowledge(
                player.lobby_session_id,
                QueueRequestId::new(request).unwrap(),
            );
        }
        let result = submit(&mut queue, player, 5, join(&catalog, 1), 0);
        assert!(result.outcome_ready() && !result.disconnect());
        assert!(matches!(
            queue
                .pending_outcome(player.lobby_session_id)
                .unwrap()
                .decision,
            QueueDecision::Rejected(QueueRejection::RateLimited {
                retry_after_millis: 1_000
            })
        ));
        queue.acknowledge(player.lobby_session_id, QueueRequestId::new(5).unwrap());
        let abusive = submit(&mut queue, player, 6, join(&catalog, 1), 500);
        assert!(abusive.disconnect() && !abusive.outcome_ready());
        assert_eq!(queue.telemetry().rate_limit_notices, 1);
    }

    #[test]
    fn all_thirty_two_sessions_can_queue_in_one_fifo_pool() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(1));
        for value in 1..=u64::try_from(MAX_AUTHENTICATED_LOBBY_SESSIONS).unwrap() {
            let player = session(value);
            let result = submit(&mut queue, player, 1, join(&catalog, 1), 0);
            assert!(result.outcome_ready() && result.snapshot_changed());
        }
        assert_eq!(queue.ticket_count(), MAX_AUTHENTICATED_LOBBY_SESSIONS);
        assert_eq!(queue.snapshot().pools[0].queued, 32);
        assert_eq!(queue.pending_outcome_count(), 32);
        let retained_bytes = queue
            .memories
            .values()
            .filter_map(|memory| memory.pending_outcome.as_ref())
            .map(|pending| postcard::to_allocvec(&pending.outcome).unwrap().len())
            .sum::<usize>();
        assert!(retained_bytes <= 32 * crate::lobby::MAX_QUEUE_OUTCOME_BYTES);
        assert!(queue.indexes_are_valid());
        assert_eq!(queue.telemetry().high_water_tickets, 32);
    }

    #[test]
    fn exact_acknowledged_roster_reserves_oldest_and_leaves_overflow_queued() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(1));
        for value in 1..=5 {
            let player = session(value);
            submit(&mut queue, player, 1, join(&catalog, 1), 0);
            if value <= 4 {
                queue.acknowledge(player.lobby_session_id, QueueRequestId::new(1).unwrap());
            }
        }
        let reservation_id = crate::lobby::MatchReservationId::new(9).unwrap();
        let live_sessions = (1..=5)
            .map(|value| LobbySessionId::new(value).unwrap())
            .collect();
        let reservation = queue
            .reserve_oldest_exact(&catalog, reservation_id, &live_sessions)
            .expect("four acknowledged 2v2 tickets form");
        assert_eq!(reservation.participants.len(), 4);
        assert_eq!(
            reservation
                .participants
                .iter()
                .map(|participant| participant.team)
                .collect::<Vec<_>>(),
            vec![0, 1, 0, 1]
        );
        assert_eq!(queue.snapshot().pools[0].queued, 1);
        assert!(queue.indexes_are_valid());

        let removed = queue.complete_reservation(reservation_id);
        assert_eq!(removed.len(), 4);
        assert_eq!(queue.reservation_count(), 0);
        assert_eq!(queue.snapshot().pools[0].queued, 1);
        assert!(queue.indexes_are_valid());
    }

    #[test]
    fn oldest_complete_pool_wins_and_the_next_roster_can_start_after_completion() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(1));
        for value in 1..=4 {
            let player = session(value);
            submit(&mut queue, player, 1, join_pool(&catalog, 1, 1), 0);
            queue.acknowledge(player.lobby_session_id, QueueRequestId::new(1).unwrap());
        }
        for value in 5..=8 {
            let player = session(value);
            submit(&mut queue, player, 1, join_pool(&catalog, 0, 1), 0);
            queue.acknowledge(player.lobby_session_id, QueueRequestId::new(1).unwrap());
        }
        let live_sessions = (1..=8)
            .map(|value| LobbySessionId::new(value).unwrap())
            .collect();
        let first_id = crate::lobby::MatchReservationId::new(30).unwrap();
        let first = queue
            .reserve_oldest_exact(&catalog, first_id, &live_sessions)
            .unwrap();
        assert_eq!(first.game_type_id, catalog.game_types[1].id);
        assert_eq!(queue.complete_reservation(first_id).len(), 4);

        let second = queue
            .reserve_oldest_exact(
                &catalog,
                crate::lobby::MatchReservationId::new(31).unwrap(),
                &live_sessions,
            )
            .unwrap();
        assert_eq!(second.game_type_id, catalog.game_types[0].id);
        assert!(queue.indexes_are_valid());
    }

    #[test]
    fn stale_disconnected_ticket_cannot_complete_a_live_roster() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(1));
        for value in 1..=4 {
            let player = session(value);
            submit(&mut queue, player, 1, join(&catalog, 1), 0);
            queue.acknowledge(player.lobby_session_id, QueueRequestId::new(1).unwrap());
        }
        let live_sessions = [1, 3, 4]
            .map(|value| LobbySessionId::new(value).unwrap())
            .into_iter()
            .collect();

        assert!(
            queue
                .reserve_oldest_exact(
                    &catalog,
                    crate::lobby::MatchReservationId::new(9).unwrap(),
                    &live_sessions,
                )
                .is_none(),
            "three live players plus one stale ticket must not form a 2v2 match"
        );
        assert_eq!(queue.reservation_count(), 0);
        assert_eq!(queue.snapshot().pools[0].queued, 4);
        assert!(queue.indexes_are_valid());
    }

    #[test]
    fn exact_six_player_catalog_entry_forms_balanced_3v3() {
        let catalog = catalog();
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(1));
        for value in 1..=6 {
            let player = session(value);
            submit(&mut queue, player, 1, join_pool(&catalog, 2, 1), 0);
            queue.acknowledge(player.lobby_session_id, QueueRequestId::new(1).unwrap());
        }
        let reservation = queue
            .reserve_oldest_exact(
                &catalog,
                crate::lobby::MatchReservationId::new(20).unwrap(),
                &(1..=6)
                    .map(|value| LobbySessionId::new(value).unwrap())
                    .collect(),
            )
            .unwrap();
        assert_eq!(reservation.players_per_team, 3);
        assert_eq!(
            reservation
                .participants
                .iter()
                .filter(|participant| participant.team == 0)
                .count(),
            3
        );
        assert_eq!(
            reservation
                .participants
                .iter()
                .filter(|participant| participant.team == 1)
                .count(),
            3
        );
    }

    #[test]
    fn revision_and_admission_order_exhaustion_do_not_partially_mutate() {
        let catalog = catalog();
        let player = session(1);
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(1));
        queue.state_revision = u64::MAX;
        let result = submit(&mut queue, player, 1, join(&catalog, 1), 0);
        assert!(result.outcome_ready() && !result.snapshot_changed());
        assert_eq!(queue.ticket_count(), 0);
        assert!(queue.indexes_are_valid());

        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(1));
        queue.next_admission_order = u64::MAX;
        let result = submit(&mut queue, player, 1, join(&catalog, 1), 0);
        assert!(result.outcome_ready() && !result.snapshot_changed());
        assert_eq!(queue.ticket_count(), 0);
        assert_eq!(queue.state_revision(), 1);
        assert!(queue.indexes_are_valid());
    }

    #[test]
    fn disconnect_at_revision_exhaustion_cleans_internal_state_and_fails_generation() {
        let catalog = catalog();
        let player = session(1);
        let mut queue = QueueState::with_id_source(&catalog, SequentialTicketIds(1));
        submit(&mut queue, player, 1, join(&catalog, 1), 0);
        assert_eq!(queue.ticket_count(), 1);
        queue.state_revision = u64::MAX;

        assert!(queue.remove_session(player.lobby_session_id));

        assert_eq!(queue.ticket_count(), 0);
        assert_eq!(queue.snapshot().pools[0].queued, 0);
        assert_eq!(queue.state_revision(), u64::MAX);
        assert!(queue.revision_exhausted());
        assert!(queue.indexes_are_valid());
        assert_eq!(queue.telemetry().disconnect_removals, 1);
    }
}
