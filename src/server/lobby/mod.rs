//! Bounded M01 lobby-worker session and allocation state.
//!
//! This module owns only the authenticated lobby roster and its supervisor allocation transaction.
//! It deliberately has no map, physics, combat, match, or map authority.  The pure state
//! machine is kept separate from the Bevy adapters so deterministic codec and idempotency tests do
//! not need a running network endpoint.

mod catalog;
mod queue;

pub use queue::{
    QueueCommandResult, QueueState, QueueTelemetry, QueueTicket, QueueTicketIdSource,
    SnapshotPublication,
};

pub(crate) use catalog::resolve_operator_catalog;

use super::{LobbyControlInbox, LobbyControlOutbox, RoutedPeer, ServerRoleResource};
use crate::{
    VERSION,
    builds::BuildCatalog,
    combat::{FighterDefinitions, STANDARD_FIGHTER_DEFINITION, WeaponCatalog},
    config::GameMode,
    content::GameplayContentFingerprint,
    lobby::{duplicate_display_name, normalize_proposed_display_name},
    protocol::{
        LobbyHello, LobbyJoinOutcome, LobbyJoinRejection, LobbyServerIdentity, MatchRouteGrant,
        ProfileChannel, QueueSnapshotChannel, RouteCapability, SUPPORTED_PROTOCOL_VERSION,
        SessionChannel,
    },
};
use bevy::prelude::*;
use brawler_routing::{
    AllocateParticipant, AllocateRequestBody, AllocationGrantedBody, AllocationRejectedBody,
    Capability, CodecError, ControlBody, ControlFrame, LobbyAuthenticatedBody, LobbyManifest,
    LobbyNetcodeAuthenticatedBody, LobbySessionId, NetcodeClientId, PeerId, PlayerId, RequestId,
    RouteId,
};
#[cfg(test)]
use brawler_routing::{ProcessId, WorkerId};
use lightyear::prelude::{Connected, Disconnected, MessageReceiver, MessageSender, RemoteId};
use std::{
    collections::{BTreeMap, BTreeSet},
    time::Duration,
};

fn routing_mode_for_definition(
    mode: crate::map::ModeDefinitionId,
) -> Option<brawler_routing::GameMode> {
    match mode {
        crate::map::WIPEOUT_MODE_DEFINITION => Some(brawler_routing::GameMode::Wipeout),
        crate::map::HOT_ZONE_MODE_DEFINITION => Some(brawler_routing::GameMode::HotZone),
        crate::map::HEIST_MODE_DEFINITION => Some(brawler_routing::GameMode::Heist),
        _ => None,
    }
}

/// M01's hard upper bound for authenticated lobby sessions.
pub const MAX_AUTHENTICATED_LOBBY_SESSIONS: usize = crate::lobby::MAX_QUEUE_TICKETS as usize;
/// M01 allocates one match only when exactly two authenticated sessions are present.
pub const M01_PARTICIPANT_COUNT: usize = 2;
/// Bounded process-lifetime memory for identities that already participated in an allocation.
/// M01 does not own Queue Again/requeue, so a returned client may authenticate a fresh lobby
/// session without immediately forming another automatic match.
pub const MAX_ALLOCATED_LOBBY_CLIENT_IDS: usize = MAX_AUTHENTICATED_LOBBY_SESSIONS * 8;

/// Build identity carried in the supervisor allocation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LobbyBuildIdentity {
    pub recipe_fingerprint: u64,
    pub build_revision: u16,
    pub snapshot: brawler_routing::MatchBuildSnapshot,
}

/// Resolve the embedded default build without installing any gameplay authority.
pub fn default_build_identity() -> Result<LobbyBuildIdentity, String> {
    let builds = BuildCatalog::embedded()?;
    let weapons = WeaponCatalog::embedded()?;
    let fighters = FighterDefinitions::default();
    let fighter = fighters
        .get(STANDARD_FIGHTER_DEFINITION)
        .ok_or_else(|| "standard fighter definition is missing".to_string())?;
    let brawler = crate::profiles::SavedBrawler {
        id: crate::profiles::SavedBrawlerId::new(1)
            .map_err(|error| format!("invalid built-in brawler id: {error}"))?,
        creation_ordinal: 1,
        name: "Default Brawler".to_string(),
        fighter_profile_id: crate::profiles::FighterProfileId(1),
        weapon_base_id: crate::profiles::WeaponBaseId(1),
        ultimate_id: crate::builds::UltimateDefinitionId(1),
        passive_ids: [
            crate::builds::PassiveDefinitionId(3),
            crate::builds::PassiveDefinitionId(4),
        ],
        equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
        revision: crate::profiles::ProfileRevision::INITIAL,
    };
    let snapshot =
        crate::profiles::MatchBuildSnapshotV3::from_brawler(&brawler, &builds, &weapons, fighter)
            .map_err(|error| format!("built-in brawler resolution failed: {error:?}"))?;
    let accepted_identity = snapshot.accepted_identity;
    Ok(LobbyBuildIdentity {
        recipe_fingerprint: accepted_identity.recipe_fingerprint.0,
        build_revision: accepted_identity.revision.0,
        snapshot: snapshot.encode()?,
    })
}

/// Resolve the code-owned V11 Practice bot recipe through the ordinary saved-brawler path.
fn practice_bot_build_identity() -> Result<LobbyBuildIdentity, String> {
    let builds = BuildCatalog::embedded()?;
    let weapons = WeaponCatalog::embedded()?;
    let fighters = FighterDefinitions::default();
    let fighter = fighters
        .get(STANDARD_FIGHTER_DEFINITION)
        .ok_or_else(|| "standard fighter definition is missing".to_string())?;
    let brawler = crate::profiles::SavedBrawler {
        id: crate::profiles::SavedBrawlerId::new(u128::MAX)
            .map_err(|error| format!("invalid canonical bot brawler id: {error}"))?,
        creation_ordinal: u64::MAX,
        name: "Practice Bot".to_string(),
        fighter_profile_id: crate::profiles::FighterProfileId(1),
        weapon_base_id: crate::profiles::WeaponBaseId(1),
        ultimate_id: crate::builds::UltimateDefinitionId(1),
        passive_ids: [
            crate::builds::PassiveDefinitionId(3),
            crate::builds::PassiveDefinitionId(4),
        ],
        equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
        revision: crate::profiles::ProfileRevision::INITIAL,
    };
    let snapshot =
        crate::profiles::MatchBuildSnapshotV3::from_brawler(&brawler, &builds, &weapons, fighter)
            .map_err(|error| format!("canonical bot brawler resolution failed: {error:?}"))?;
    let accepted_identity = snapshot.accepted_identity;
    Ok(LobbyBuildIdentity {
        recipe_fingerprint: accepted_identity.recipe_fingerprint.0,
        build_revision: accepted_identity.revision.0,
        snapshot: snapshot.encode()?,
    })
}

fn practice_bot_rows(
    team_count: u8,
    players_per_team: u8,
) -> Result<Vec<brawler_routing::AllocateBot>, crate::lobby::PracticeStartRejection> {
    use crate::lobby::PracticeStartRejection as Rejection;
    let roster_size = usize::from(team_count) * usize::from(players_per_team);
    let mut bots = Vec::with_capacity(roster_size.saturating_sub(1));
    for ordinal in 0..roster_size.saturating_sub(1) {
        let roster_index = ordinal + 1;
        let team = u8::try_from(roster_index / usize::from(players_per_team))
            .map_err(|_| Rejection::Internal)?;
        let build = practice_bot_build_identity().map_err(|_| Rejection::Internal)?;
        bots.push(brawler_routing::AllocateBot {
            player_id: PlayerId::new(
                u64::MAX
                    .checked_sub(u64::try_from(ordinal).map_err(|_| Rejection::Internal)?)
                    .ok_or(Rejection::Internal)?,
            )
            .ok_or(Rejection::Internal)?,
            team,
            display_name: brawler_routing::MatchDisplayName::new(&format!("Bot {}", ordinal + 1))
                .map_err(|_| Rejection::Internal)?,
            recipe_fingerprint: build.recipe_fingerprint,
            build_revision: build.build_revision,
            build_snapshot: build.snapshot,
        });
    }
    Ok(bots)
}

/// Why an unauthenticated hello cannot become a lobby session.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbySessionError {
    InvalidClientId,
    NotRouted,
    ServerFull,
    ProtocolVersionMismatch,
    BuildVersionMismatch,
    RegistryMismatch,
    ContentMismatch,
    InvalidName,
    IdentifierExhausted,
}

impl LobbySessionError {
    const fn rejection(self) -> LobbyJoinRejection {
        match self {
            Self::ProtocolVersionMismatch => LobbyJoinRejection::ProtocolVersionMismatch,
            Self::BuildVersionMismatch => LobbyJoinRejection::BuildVersionMismatch,
            Self::RegistryMismatch => LobbyJoinRejection::RegistryMismatch,
            Self::ContentMismatch => LobbyJoinRejection::ContentMismatch,
            Self::InvalidName => LobbyJoinRejection::InvalidName,
            Self::IdentifierExhausted => LobbyJoinRejection::IdentifierExhausted,
            Self::InvalidClientId | Self::NotRouted | Self::ServerFull => {
                LobbyJoinRejection::ServerFull
            }
        }
    }
}

/// Why an allocation response was not accepted by the current lobby transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum LobbyAllocationError {
    NoPendingRequest,
    RequestMismatch,
    InvalidGrantCount,
    UnknownSession,
    DuplicateSession,
    InvalidCapability,
    InvalidExpiry,
    AllocationIdentityMemoryFull,
}

/// One authenticated lobby participant.  Secret-bearing route grants are stored separately and
/// this debug view intentionally avoids exposing stable identities or source-address-like peers.
#[derive(Clone, Copy, PartialEq, Eq)]
#[allow(
    clippy::struct_field_names,
    reason = "the explicit lobby_session_id distinguishes it from player, Netcode, route, and peer identities"
)]
pub struct LobbySession {
    pub lobby_session_id: LobbySessionId,
    pub player_id: PlayerId,
    pub network_entity_id: crate::protocol::NetworkEntityId,
    pub netcode_client_id: NetcodeClientId,
    pub route_id: RouteId,
    pub peer_id: PeerId,
    pub team: u8,
    pub build: LobbyBuildIdentity,
}

impl core::fmt::Debug for LobbySession {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LobbySession")
            .field("session", &"[REDACTED]")
            .field("team", &self.team)
            .field("build_revision", &self.build.build_revision)
            .finish_non_exhaustive()
    }
}

#[derive(Clone)]
struct PendingAllocation {
    body: AllocateRequestBody,
    sent: bool,
}

/// Source of fresh nonzero lobby-session identities. Production uses OS-backed capability
/// entropy; tests inject a deterministic source without weakening the production path.
pub trait LobbySessionIdSource: Send + Sync {
    fn next(&mut self) -> Option<LobbySessionId>;
}

struct OsLobbySessionIdSource;

impl LobbySessionIdSource for OsLobbySessionIdSource {
    fn next(&mut self) -> Option<LobbySessionId> {
        for _ in 0..4 {
            let bytes = Capability::generate().ok()?.into_bytes();
            let mut id_bytes = [0_u8; 16];
            id_bytes.copy_from_slice(&bytes[..16]);
            if let Some(id) = LobbySessionId::new(u128::from_le_bytes(id_bytes)) {
                return Some(id);
            }
        }
        None
    }
}

/// Pure, bounded lobby state.  All maps are ordered to make participant and request ordering
/// deterministic across runs and test harnesses.
#[derive(Resource)]
#[allow(clippy::struct_excessive_bools)]
pub struct LobbyState {
    manifest: LobbyManifest,
    mode: GameMode,
    build: LobbyBuildIdentity,
    next_player_id: u64,
    next_network_entity_id: u64,
    next_request_id: u64,
    session_ids: Box<dyn LobbySessionIdSource>,
    sessions: BTreeMap<NetcodeClientId, LobbySession>,
    accepted_names: BTreeMap<NetcodeClientId, String>,
    welcomed_clients: BTreeSet<NetcodeClientId>,
    /// These tombstones intentionally survive lobby-session teardown. Route, peer, and lobby
    /// session IDs are all fresh on handoff; the authenticated Netcode ID is the stable identity
    /// that prevents M01 from approximating M06 requeue.
    allocated_clients: BTreeSet<NetcodeClientId>,
    pending: Option<PendingAllocation>,
    allocation_completed: bool,
    active_allocation: Option<brawler_routing::ActivationBody>,
    allocation_rejected: bool,
    product_activated: bool,
    product_dissolved: bool,
    product_cancel_requested: bool,
    product_request: bool,
    free_match_slots: u8,
    grants: BTreeMap<NetcodeClientId, MatchRouteGrant>,
}

impl core::fmt::Debug for LobbyState {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LobbyState")
            .field("mode", &self.mode)
            .field("session_count", &self.sessions.len())
            .field("allocated_client_count", &self.allocated_clients.len())
            .field("allocation_pending", &self.pending.is_some())
            .field("grants_pending", &self.grants.len())
            .finish_non_exhaustive()
    }
}

impl LobbyState {
    #[must_use]
    pub fn new(manifest: LobbyManifest, mode: GameMode, build: LobbyBuildIdentity) -> Self {
        Self::with_id_source(manifest, mode, build, OsLobbySessionIdSource)
    }

    #[must_use]
    pub fn with_id_source<S>(
        manifest: LobbyManifest,
        mode: GameMode,
        build: LobbyBuildIdentity,
        session_ids: S,
    ) -> Self
    where
        S: LobbySessionIdSource + 'static,
    {
        Self {
            manifest,
            mode,
            build,
            next_player_id: 1,
            next_network_entity_id: 1,
            next_request_id: 1,
            session_ids: Box::new(session_ids),
            sessions: BTreeMap::new(),
            accepted_names: BTreeMap::new(),
            welcomed_clients: BTreeSet::new(),
            allocated_clients: BTreeSet::new(),
            pending: None,
            allocation_completed: false,
            active_allocation: None,
            allocation_rejected: false,
            product_activated: false,
            product_dissolved: false,
            product_cancel_requested: false,
            product_request: false,
            free_match_slots: 0,
            grants: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn manifest(&self) -> &LobbyManifest {
        &self.manifest
    }

    #[must_use]
    pub const fn mode(&self) -> GameMode {
        self.mode
    }

    #[must_use]
    pub fn session_count(&self) -> usize {
        self.sessions.len()
    }

    #[must_use]
    pub fn session_for_client(&self, client_id: u64) -> Option<&LobbySession> {
        NetcodeClientId::new(client_id).and_then(|id| self.sessions.get(&id))
    }

    #[must_use]
    fn authenticated_session_ids(&self) -> BTreeSet<LobbySessionId> {
        self.sessions
            .values()
            .map(|session| session.lobby_session_id)
            .collect()
    }

    #[must_use]
    fn session_by_lobby_id(&self, id: LobbySessionId) -> Option<LobbySession> {
        self.sessions
            .values()
            .find(|session| session.lobby_session_id == id)
            .copied()
    }

    fn validate_hello(&self, hello: &LobbyHello) -> Result<String, LobbySessionError> {
        if hello.protocol_version != SUPPORTED_PROTOCOL_VERSION {
            return Err(LobbySessionError::ProtocolVersionMismatch);
        }
        if hello.build_version != VERSION {
            return Err(LobbySessionError::BuildVersionMismatch);
        }
        if hello.registry_fingerprint != self.manifest.common.protocol_registry_fingerprint {
            return Err(LobbySessionError::RegistryMismatch);
        }
        if hello.content_fingerprint
            != GameplayContentFingerprint(self.manifest.common.content_fingerprint)
        {
            return Err(LobbySessionError::ContentMismatch);
        }
        normalize_proposed_display_name(&hello.proposed_display_name)
            .map_err(|_| LobbySessionError::InvalidName)
    }

    fn fresh_lobby_session_id(&mut self) -> Option<LobbySessionId> {
        for _ in 0..(MAX_AUTHENTICATED_LOBBY_SESSIONS * 2) {
            let id = self.session_ids.next()?;
            if !self
                .sessions
                .values()
                .any(|session| session.lobby_session_id == id)
            {
                return Some(id);
            }
        }
        None
    }

    fn fresh_player_id(&mut self) -> Option<PlayerId> {
        let id = PlayerId::new(self.next_player_id)?;
        self.next_player_id = self.next_player_id.checked_add(1)?;
        Some(id)
    }

    fn fresh_network_entity_id(&mut self) -> Option<crate::protocol::NetworkEntityId> {
        let id = crate::protocol::NetworkEntityId(self.next_network_entity_id);
        if id.0 == 0 {
            return None;
        }
        self.next_network_entity_id = self.next_network_entity_id.checked_add(1)?;
        Some(id)
    }

    /// Authenticate one routed Netcode client and allocate stable lobby identities exactly once.
    /// Repeated hellos from the same client are idempotent after compatibility validation.
    pub fn accept_client(
        &mut self,
        client_id: u64,
        route_id: RouteId,
        peer_id: PeerId,
        hello: &LobbyHello,
    ) -> Result<LobbySession, LobbySessionError> {
        let proposed_name = self.validate_hello(hello)?;
        let client_id =
            NetcodeClientId::new(client_id).ok_or(LobbySessionError::InvalidClientId)?;
        if let Some(session) = self.sessions.get(&client_id) {
            return Ok(*session);
        }
        let maximum = usize::from(self.manifest.max_authenticated_sessions)
            .min(MAX_AUTHENTICATED_LOBBY_SESSIONS);
        if self.sessions.len() >= maximum {
            return Err(LobbySessionError::ServerFull);
        }
        let session = LobbySession {
            lobby_session_id: self
                .fresh_lobby_session_id()
                .ok_or(LobbySessionError::IdentifierExhausted)?,
            player_id: self
                .fresh_player_id()
                .ok_or(LobbySessionError::IdentifierExhausted)?,
            network_entity_id: self
                .fresh_network_entity_id()
                .ok_or(LobbySessionError::IdentifierExhausted)?,
            netcode_client_id: client_id,
            route_id,
            peer_id,
            team: u8::try_from(self.sessions.len() % 2)
                .map_err(|_| LobbySessionError::IdentifierExhausted)?,
            build: self.build,
        };
        let accepted_name = self.fresh_accepted_name(&proposed_name)?;
        self.sessions.insert(client_id, session);
        self.accepted_names.insert(client_id, accepted_name);
        Ok(session)
    }

    fn fresh_accepted_name(&self, base: &str) -> Result<String, LobbySessionError> {
        for suffix in
            1..=u32::try_from(MAX_AUTHENTICATED_LOBBY_SESSIONS + 1).expect("session bound fits u32")
        {
            let candidate = if suffix == 1 {
                base.to_string()
            } else {
                duplicate_display_name(base, suffix).map_err(|_| LobbySessionError::InvalidName)?
            };
            if !self.accepted_names.values().any(|name| name == &candidate) {
                return Ok(candidate);
            }
        }
        Err(LobbySessionError::IdentifierExhausted)
    }

    #[must_use]
    pub fn accepted_name(&self, client_id: NetcodeClientId) -> Option<&str> {
        self.accepted_names.get(&client_id).map(String::as_str)
    }

    fn mark_welcome_sent(&mut self, client_id: NetcodeClientId) -> bool {
        self.welcomed_clients.insert(client_id)
    }

    /// Remove a disconnected client and revoke any local grant. BRCT v1 has no cancellation body,
    /// so a pending allocation is cancelled locally and a late supervisor response is ignored.
    pub fn remove_client(&mut self, client_id: u64) -> Option<LobbySession> {
        let id = NetcodeClientId::new(client_id)?;
        let session = self.sessions.remove(&id)?;
        self.accepted_names.remove(&id);
        self.welcomed_clients.remove(&id);
        self.grants.remove(&id);
        self.pending = None;
        self.allocation_completed = false;
        Some(session)
    }

    fn create_pending_request(&mut self) -> Result<(), CodecError> {
        if self.pending.is_some()
            || self.allocation_completed
            || self.sessions.len() != M01_PARTICIPANT_COUNT
        {
            return Ok(());
        }
        // A new lobby session is still accepted for a client that just returned from a match, but
        // M01 must not interpret that reconnect as an implicit Queue Again. The tombstones are
        // keyed by authenticated Netcode identity rather than route/peer/session IDs, all of
        // which are intentionally fresh on every handoff.
        if self
            .sessions
            .keys()
            .any(|client_id| self.allocated_clients.contains(client_id))
        {
            return Ok(());
        }
        // Keep the process-lifetime guard bounded. Once the budget is exhausted the lobby stays
        // available for authentication, but does not create another automatic M01 allocation;
        // an explicit lifecycle policy can replace this in M06.
        if self
            .allocated_clients
            .len()
            .saturating_add(M01_PARTICIPANT_COUNT)
            > MAX_ALLOCATED_LOBBY_CLIENT_IDS
        {
            return Ok(());
        }
        let request_id = RequestId::new(self.next_request_id).ok_or(CodecError::ZeroId)?;
        self.next_request_id = self
            .next_request_id
            .checked_add(1)
            .ok_or(CodecError::Oversize)?;
        let mut sessions: Vec<_> = self.sessions.values().copied().collect();
        sessions.sort_by_key(|session| session.lobby_session_id);
        let participants = sessions
            .iter()
            .map(|session| -> Result<AllocateParticipant, CodecError> {
                Ok(AllocateParticipant {
                    lobby_session_id: session.lobby_session_id,
                    player_id: session.player_id,
                    netcode_client_id: session.netcode_client_id,
                    team: session.team,
                    display_name: brawler_routing::MatchDisplayName::new(
                        self.accepted_name(session.netcode_client_id)
                            .ok_or(CodecError::InvalidValue)?,
                    )?,
                    recipe_fingerprint: session.build.recipe_fingerprint,
                    build_revision: session.build.build_revision,
                    build_snapshot: session.build.snapshot,
                })
            })
            .collect::<Result<Vec<_>, _>>()?;
        let map_preset = match self.mode {
            GameMode::Wipeout => crate::map::FEATURE_YARD_WIPEOUT_PRESET,
            GameMode::HotZone => crate::map::FEATURE_YARD_HOT_ZONE_PRESET,
            GameMode::Heist => crate::map::FEATURE_YARD_HEIST_PRESET,
        };
        let map_revision = crate::map::MapContentCatalog::embedded()
            .ok()
            .and_then(|catalog| {
                catalog
                    .preset(map_preset)
                    .map(|preset| preset.admission_revision)
            })
            .ok_or(CodecError::InvalidValue)?;
        let body = AllocateRequestBody {
            request_id,
            lobby_session_id: sessions[0].lobby_session_id,
            mode: match self.mode {
                GameMode::Wipeout => brawler_routing::GameMode::Wipeout,
                GameMode::HotZone => brawler_routing::GameMode::HotZone,
                GameMode::Heist => brawler_routing::GameMode::Heist,
            },
            map_preset: map_preset.0,
            map_revision,
            rules_profile: 1,
            objective_target: match self.mode {
                GameMode::Wipeout => 10,
                GameMode::HotZone => 1_800,
                GameMode::Heist => 2_000,
            },
            match_duration_ticks: 10_800,
            countdown_ticks: 180,
            respawn_ticks: 180,
            team_count: 2,
            players_per_team: 1,
            participants,
            bots: Vec::new(),
        };
        self.pending = Some(PendingAllocation { body, sent: false });
        self.product_request = false;
        Ok(())
    }

    fn create_product_request(
        &mut self,
        reservation: &queue::QueueReservation,
        queue: &QueueState,
        catalog: &catalog::ResolvedLobbyCatalog,
    ) -> Result<(), CodecError> {
        if self.pending.is_some() || self.allocation_completed {
            return Ok(());
        }
        let game = catalog
            .game_types
            .iter()
            .find(|game| game.id == reservation.game_type_id)
            .ok_or(CodecError::InvalidValue)?;
        let request_id = RequestId::new(self.next_request_id).ok_or(CodecError::ZeroId)?;
        self.next_request_id = self
            .next_request_id
            .checked_add(1)
            .ok_or(CodecError::Oversize)?;
        let mut participants = Vec::with_capacity(reservation.participants.len());
        for reserved in &reservation.participants {
            let ticket = queue
                .ticket(reserved.ticket_id)
                .ok_or(CodecError::InvalidValue)?;
            participants.push(AllocateParticipant {
                lobby_session_id: ticket.lobby_session_id,
                player_id: ticket.player_id,
                netcode_client_id: ticket.netcode_client_id,
                team: reserved.team,
                display_name: brawler_routing::MatchDisplayName::new(
                    self.accepted_name(ticket.netcode_client_id)
                        .ok_or(CodecError::InvalidValue)?,
                )?,
                recipe_fingerprint: ticket.accepted_build.identity.recipe_fingerprint.0,
                build_revision: ticket.accepted_build.identity.revision.0,
                build_snapshot: ticket
                    .build_snapshot
                    .encode()
                    .map_err(|_| CodecError::InvalidValue)?,
            });
        }
        let lobby_session_id = participants
            .first()
            .ok_or(CodecError::InvalidValue)?
            .lobby_session_id;
        let rules = catalog
            .rules(&reservation.game_type_id)
            .ok_or(CodecError::InvalidValue)?;
        self.pending = Some(PendingAllocation {
            body: AllocateRequestBody {
                request_id,
                lobby_session_id,
                mode: routing_mode_for_definition(game.mode_definition_id)
                    .ok_or(CodecError::InvalidValue)?,
                map_preset: reservation.map_preset_id.0,
                map_revision: catalog
                    .map_admission_revision(reservation.map_preset_id)
                    .ok_or(CodecError::InvalidValue)?,
                rules_profile: crate::config::MatchRulesProfile::Production.routing_id(),
                objective_target: rules.objective_target,
                match_duration_ticks: rules.match_duration_ticks,
                countdown_ticks: rules.countdown_ticks,
                respawn_ticks: rules.respawn_ticks,
                team_count: reservation.team_count,
                players_per_team: reservation.players_per_team,
                participants,
                bots: Vec::new(),
            },
            sent: false,
        });
        self.product_request = true;
        self.allocation_rejected = false;
        Ok(())
    }

    fn create_practice_request(
        &mut self,
        session: &LobbySession,
        command: &crate::lobby::PracticeStartRequest,
        catalog: &catalog::ResolvedLobbyCatalog,
        human_snapshot: crate::profiles::MatchBuildSnapshotV3,
        accepted_build: crate::builds::AcceptedBuildSummary,
    ) -> Result<crate::lobby::ReservationStarted, crate::lobby::PracticeStartRejection> {
        use crate::lobby::PracticeStartRejection as Rejection;
        if self.pending.is_some() || self.allocation_completed {
            return Err(Rejection::Busy);
        }
        if self.free_match_slots == 0 {
            return Err(Rejection::CapacityUnavailable);
        }
        if command.catalog_revision != catalog.revision {
            return Err(Rejection::StaleCatalog);
        }
        let game = catalog
            .game_types
            .iter()
            .find(|game| game.id == command.game_type_id)
            .ok_or(Rejection::UnknownGameType)?;
        if game.configuration_revision != command.game_type_configuration_revision {
            return Err(Rejection::StaleGameConfiguration);
        }
        if human_snapshot.brawler_id != command.brawler_id
            || human_snapshot.brawler_revision != command.brawler_revision
        {
            return Err(Rejection::InvalidBuild);
        }
        let accepted_identity = human_snapshot.accepted_identity;
        let human_snapshot = human_snapshot.encode().map_err(|_| Rejection::Internal)?;
        let request_id = RequestId::new(self.next_request_id).ok_or(Rejection::Internal)?;
        self.next_request_id = self
            .next_request_id
            .checked_add(1)
            .ok_or(Rejection::Internal)?;
        let reservation_entropy = Capability::generate().map_err(|_| Rejection::Internal)?;
        let reservation_id = crate::lobby::MatchReservationId::new(u128::from_le_bytes(
            reservation_entropy.into_bytes()[..16]
                .try_into()
                .expect("capability prefix width"),
        ))
        .ok_or(Rejection::Internal)?;
        let human = AllocateParticipant {
            lobby_session_id: session.lobby_session_id,
            player_id: session.player_id,
            netcode_client_id: session.netcode_client_id,
            team: 0,
            display_name: brawler_routing::MatchDisplayName::new(
                self.accepted_name(session.netcode_client_id)
                    .ok_or(Rejection::Internal)?,
            )
            .map_err(|_| Rejection::Internal)?,
            recipe_fingerprint: accepted_identity.recipe_fingerprint.0,
            build_revision: accepted_identity.revision.0,
            build_snapshot: human_snapshot,
        };
        let bots = practice_bot_rows(game.team_count, game.players_per_team)?;
        let rules = catalog.rules(&game.id).ok_or(Rejection::Internal)?;
        self.pending = Some(PendingAllocation {
            body: AllocateRequestBody {
                request_id,
                lobby_session_id: session.lobby_session_id,
                mode: routing_mode_for_definition(game.mode_definition_id)
                    .ok_or(Rejection::Internal)?,
                map_preset: game.map_preset_ids[0].0,
                map_revision: catalog
                    .map_admission_revision(game.map_preset_ids[0])
                    .ok_or(Rejection::Internal)?,
                rules_profile: crate::config::MatchRulesProfile::Production.routing_id(),
                objective_target: rules.objective_target,
                match_duration_ticks: rules.match_duration_ticks,
                countdown_ticks: rules.countdown_ticks,
                respawn_ticks: rules.respawn_ticks,
                team_count: game.team_count,
                players_per_team: game.players_per_team,
                participants: vec![human],
                bots,
            },
            sent: false,
        });
        self.product_request = true;
        self.allocation_rejected = false;
        Ok(crate::lobby::ReservationStarted {
            reservation_id,
            ticket_id: None,
            game_type_id: game.id.clone(),
            map_preset_id: game.map_preset_ids[0],
            team_count: game.team_count,
            players_per_team: game.players_per_team,
            accepted_build,
            loading_deadline_millis: 30_000,
        })
    }

    /// Return the one stable allocation request body for the current exact-two roster. The worker
    /// control owner adds the shared process/worker sequence and envelope when it dequeues it.
    pub fn pending_allocate_request(&mut self) -> Result<Option<AllocateRequestBody>, CodecError> {
        self.create_pending_request()?;
        Ok(self.pending.as_ref().map(|pending| pending.body.clone()))
    }

    /// Return the request body only while it still needs handing to the worker outbox.
    pub fn unsent_allocate_request(&mut self) -> Result<Option<AllocateRequestBody>, CodecError> {
        self.create_pending_request()?;
        Ok(self
            .pending
            .as_ref()
            .filter(|pending| !pending.sent)
            .map(|pending| pending.body.clone()))
    }

    fn unsent_existing_request(&self) -> Option<AllocateRequestBody> {
        self.pending
            .as_ref()
            .filter(|pending| !pending.sent)
            .map(|pending| pending.body.clone())
    }

    /// Mark the stable request as handed to the bounded worker outbox. A full outbox leaves it
    /// pending so the next update can retry without creating another request ID.
    pub fn mark_allocate_request_queued(&mut self) -> bool {
        let Some(pending) = self.pending.as_ref() else {
            return false;
        };
        if pending.sent {
            return false;
        }
        if let Some(pending) = self.pending.as_mut() {
            pending.sent = true;
        }
        true
    }

    fn pending_request_id(&self) -> Option<RequestId> {
        self.pending.as_ref().map(|pending| pending.body.request_id)
    }

    /// Validate and accept the supervisor's two-grant response.
    pub fn apply_allocation_granted(
        &mut self,
        body: AllocationGrantedBody,
    ) -> Result<(), LobbyAllocationError> {
        let Some(request_id) = self.pending_request_id() else {
            return Err(LobbyAllocationError::NoPendingRequest);
        };
        if body.request_id != request_id {
            return Err(LobbyAllocationError::RequestMismatch);
        }
        let expected_participants = self
            .pending
            .as_ref()
            .map_or(0, |pending| pending.body.participants.len());
        if body.grants.len() != expected_participants {
            return Err(LobbyAllocationError::InvalidGrantCount);
        }
        let mut seen = BTreeSet::new();
        let mut converted = Vec::with_capacity(body.grants.len());
        for grant in body.grants {
            if !seen.insert(grant.lobby_session_id) {
                return Err(LobbyAllocationError::DuplicateSession);
            }
            let Some(session) = self
                .sessions
                .values()
                .find(|session| session.lobby_session_id == grant.lobby_session_id)
            else {
                return Err(LobbyAllocationError::UnknownSession);
            };
            let Some(capability) = RouteCapability::from_bytes(grant.capability.into_bytes())
            else {
                return Err(LobbyAllocationError::InvalidCapability);
            };
            if grant.activation_expiry_unix_ms > grant.route_expiry_unix_ms {
                return Err(LobbyAllocationError::InvalidExpiry);
            }
            let route_grant = MatchRouteGrant {
                request_id,
                allocation_id: body.allocation_id,
                match_id: body.match_id,
                route_id: grant.route_id,
                peer_id: grant.peer_id,
                game_mode: self.mode,
                capability,
                activation_expiry_unix_ms: grant.activation_expiry_unix_ms,
                route_expiry_unix_ms: grant.route_expiry_unix_ms,
            };
            let client_id = session.netcode_client_id;
            converted.push((client_id, route_grant));
        }
        let new_identity_count = converted
            .iter()
            .filter(|(client_id, _)| !self.allocated_clients.contains(client_id))
            .count();
        if !self.product_request
            && self
                .allocated_clients
                .len()
                .saturating_add(new_identity_count)
                > MAX_ALLOCATED_LOBBY_CLIENT_IDS
        {
            return Err(LobbyAllocationError::AllocationIdentityMemoryFull);
        }
        for (client_id, grant) in converted {
            if !self.product_request {
                self.allocated_clients.insert(client_id);
            }
            self.grants.insert(client_id, grant);
        }
        self.active_allocation = Some(brawler_routing::ActivationBody {
            request_id,
            allocation_id: body.allocation_id,
            match_id: body.match_id,
        });
        self.pending = None;
        self.allocation_completed = true;
        self.product_request = false;
        Ok(())
    }

    /// Accept a matching rejection and clear the completed request, allowing a later retry with
    /// a fresh request identity.
    pub fn apply_allocation_rejected(
        &mut self,
        body: AllocationRejectedBody,
    ) -> Result<(), LobbyAllocationError> {
        let Some(request_id) = self.pending_request_id() else {
            return Err(LobbyAllocationError::NoPendingRequest);
        };
        if body.request_id != request_id {
            return Err(LobbyAllocationError::RequestMismatch);
        }
        self.pending = None;
        self.allocation_rejected = true;
        self.product_cancel_requested = false;
        self.product_request = false;
        Ok(())
    }

    /// Apply one decoded worker control body.  The worker control adapter can feed this message
    /// without giving the lobby direct ownership of the shared IPC reader.
    pub fn apply_control_frame(&mut self, frame: ControlFrame) -> Result<(), LobbyAllocationError> {
        match frame.body {
            ControlBody::AllocationGranted(body) => self.apply_allocation_granted(body),
            ControlBody::AllocationRejected(body) => self.apply_allocation_rejected(body),
            ControlBody::Activated(body) if self.active_allocation == Some(body) => {
                self.product_activated = true;
                Ok(())
            }
            ControlBody::ActivationDissolved(body) if self.active_allocation == Some(body) => {
                self.product_dissolved = true;
                self.active_allocation = None;
                self.allocation_completed = false;
                Ok(())
            }
            ControlBody::LobbyCapacity(body) => {
                self.free_match_slots = body.free_match_slots;
                Ok(())
            }
            _ => Err(LobbyAllocationError::RequestMismatch),
        }
    }

    /// Take one grant for the authenticated client.  A grant is never logged and is delivered at
    /// most once.
    pub fn take_route_grant(&mut self, client_id: u64) -> Option<MatchRouteGrant> {
        NetcodeClientId::new(client_id).and_then(|id| self.grants.remove(&id))
    }

    fn cancel_product_allocation(&mut self) -> Option<brawler_routing::ActivationBody> {
        if self.active_allocation.is_none()
            && self.pending.as_ref().is_some_and(|pending| pending.sent)
        {
            self.product_cancel_requested = true;
            self.grants.clear();
            return None;
        }
        self.pending = None;
        self.allocation_completed = false;
        self.grants.clear();
        self.active_allocation.take()
    }

    fn take_deferred_product_cancel(&mut self) -> Option<brawler_routing::ActivationBody> {
        if !self.product_cancel_requested {
            return None;
        }
        let fact = self.active_allocation.take()?;
        self.product_cancel_requested = false;
        self.allocation_completed = false;
        self.grants.clear();
        Some(fact)
    }

    fn detach_client(&mut self, client_id: u64) -> Option<LobbySession> {
        let id = NetcodeClientId::new(client_id)?;
        let session = self.sessions.remove(&id)?;
        self.accepted_names.remove(&id);
        self.welcomed_clients.remove(&id);
        Some(session)
    }

    fn take_allocation_rejected(&mut self) -> bool {
        core::mem::take(&mut self.allocation_rejected)
    }

    fn take_product_activated(&mut self) -> bool {
        core::mem::take(&mut self.product_activated)
    }

    fn complete_product_activation(&mut self) {
        self.pending = None;
        self.allocation_completed = false;
        self.active_allocation = None;
        self.grants.clear();
        self.product_cancel_requested = false;
        self.product_request = false;
    }

    fn take_product_dissolved(&mut self) -> bool {
        core::mem::take(&mut self.product_dissolved)
    }
}

/// Control-frame seam for the worker control owner to forward validated allocation responses.
#[derive(Message, Clone)]
pub struct LobbyControlFrame(pub ControlFrame);

/// Marker linking a Lightyear child link to one authenticated lobby session.
#[derive(Component, Clone, Copy, PartialEq, Eq)]
#[allow(
    clippy::struct_field_names,
    reason = "the explicit lobby_session_id preserves the cross-control-plane identity name"
)]
pub struct LobbyClient {
    pub client_id: NetcodeClientId,
    pub lobby_session_id: LobbySessionId,
}

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) enum LobbyScheduleSet {
    BeginLobbyFrame,
    AuthenticateLobbyHellos,
    ApplyProfileTransactions,
    CollectQueueClientMessages,
    CleanupDisconnectedSessions,
    ApplyQueueTransactions,
    FormReservations,
    PublishQueueOutcomesAndSnapshot,
}

#[derive(Resource, Default)]
struct ProductFormationState {
    active: Option<crate::lobby::MatchReservationId>,
    practice: Option<PracticeFormation>,
    next_sequence: BTreeMap<LobbySessionId, u64>,
    pending_grants: BTreeMap<LobbySessionId, MatchRouteGrant>,
    begin_sent: BTreeSet<LobbySessionId>,
    loading_deadline: Option<Duration>,
    grant_deadline: Option<Duration>,
}

#[derive(Clone)]
struct PracticeFormation {
    request_id: crate::lobby::PracticeRequestId,
    session_id: LobbySessionId,
    started: crate::lobby::ReservationStarted,
}

impl ProductFormationState {
    fn next_sequence(&mut self, session_id: LobbySessionId) -> u64 {
        let sequence = self.next_sequence.entry(session_id).or_insert(1);
        let current = *sequence;
        *sequence = sequence.saturating_add(1);
        current
    }

    fn clear_handoff(&mut self) {
        self.pending_grants.clear();
        self.begin_sent.clear();
        self.loading_deadline = None;
        self.grant_deadline = None;
    }

    fn clear_active(&mut self) {
        self.active = None;
        self.practice = None;
        self.clear_handoff();
    }
}

#[derive(Resource, Default)]
struct LobbyQueueFrame {
    eligible: BTreeSet<LobbySessionId>,
    collected: Vec<CollectedQueueMessages>,
    pending_deliveries: BTreeSet<LobbySessionId>,
    snapshot_changed: bool,
}

struct CollectedQueueMessages {
    entity: Entity,
    session_id: LobbySessionId,
    messages: Vec<crate::lobby::QueueClientMessage>,
}

#[derive(Resource, Default)]
struct LobbySessionLosses(BTreeMap<LobbySessionId, NetcodeClientId>);

#[derive(Resource, Default)]
struct PendingLobbyAdmissions(BTreeMap<u64, PendingLobbyAdmission>);

#[derive(Clone)]
struct PendingLobbyAdmission {
    entity: Entity,
    route_id: RouteId,
    peer_id: PeerId,
    hello: LobbyHello,
}

#[derive(Resource, Default)]
struct QueueSnapshotPublication {
    initial_pending: bool,
    mutation_pending: bool,
    last_refresh: Duration,
}

#[derive(Resource)]
struct QueueEvidenceSettings {
    report_aggregates: bool,
}

impl FromWorld for QueueEvidenceSettings {
    fn from_world(_: &mut World) -> Self {
        Self {
            report_aggregates: std::env::var("BRAWLER_QUEUE_EVIDENCE").as_deref() == Ok("1"),
        }
    }
}

impl core::fmt::Debug for LobbyClient {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LobbyClient")
            .field("identity", &"[REDACTED]")
            .finish()
    }
}

/// Installs product lobby session ownership. Product sessions remain idle after authentication.
pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LobbyControlInbox>()
            .init_resource::<LobbyControlOutbox>()
            .init_resource::<LobbyQueueFrame>()
            .init_resource::<LobbySessionLosses>()
            .init_resource::<PendingLobbyAdmissions>()
            .init_resource::<QueueSnapshotPublication>()
            .init_resource::<QueueEvidenceSettings>()
            .init_resource::<ProductFormationState>()
            .init_resource::<FighterDefinitions>()
            .add_systems(Startup, initialize_lobby_state)
            .configure_sets(
                Update,
                (
                    LobbyScheduleSet::BeginLobbyFrame,
                    LobbyScheduleSet::AuthenticateLobbyHellos,
                    LobbyScheduleSet::ApplyProfileTransactions,
                    LobbyScheduleSet::CollectQueueClientMessages,
                    LobbyScheduleSet::CleanupDisconnectedSessions,
                    LobbyScheduleSet::ApplyQueueTransactions,
                    LobbyScheduleSet::FormReservations,
                    LobbyScheduleSet::PublishQueueOutcomesAndSnapshot,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    begin_lobby_frame.in_set(LobbyScheduleSet::BeginLobbyFrame),
                    lobby_receive_hellos.in_set(LobbyScheduleSet::AuthenticateLobbyHellos),
                    (
                        process_profile_storage_results,
                        collect_profile_commands,
                        ApplyDeferred,
                    )
                        .chain()
                        .in_set(LobbyScheduleSet::ApplyProfileTransactions),
                    collect_queue_client_messages
                        .in_set(LobbyScheduleSet::CollectQueueClientMessages),
                    cleanup_disconnected_sessions
                        .in_set(LobbyScheduleSet::CleanupDisconnectedSessions),
                    apply_queue_transactions.in_set(LobbyScheduleSet::ApplyQueueTransactions),
                    (
                        apply_practice_start_requests,
                        apply_matchmaking_client_messages,
                        lobby_apply_control_frames,
                        flush_deferred_product_cancel,
                        apply_product_dissolution,
                        apply_product_activation,
                        expire_product_reservation,
                        lobby_deliver_product_grants,
                        form_product_reservation,
                        lobby_enqueue_product_allocation,
                    )
                        .chain()
                        .in_set(LobbyScheduleSet::FormReservations),
                    publish_queue_outcomes_and_snapshot
                        .in_set(LobbyScheduleSet::PublishQueueOutcomesAndSnapshot),
                    report_queue_telemetry_changes
                        .after(LobbyScheduleSet::PublishQueueOutcomesAndSnapshot),
                ),
            )
            .add_observer(lobby_client_removed)
            .add_observer(lobby_netcode_authenticated);
    }
}

/// Explicit M01 evidence composition. Production lobby workers never install this plugin.
pub(crate) struct LobbyTransitionDriverPlugin;

impl Plugin for LobbyTransitionDriverPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            Update,
            (
                lobby_apply_control_frames,
                lobby_enqueue_allocation,
                lobby_deliver_grants,
            )
                .chain()
                .after(LobbyScheduleSet::PublishQueueOutcomesAndSnapshot),
        );
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy evidence publication reads a runtime-owned resource parameter"
)]
fn report_queue_telemetry_changes(
    settings: Res<QueueEvidenceSettings>,
    queue: Res<QueueState>,
    mut previous: Local<Option<QueueTelemetry>>,
) {
    if !settings.report_aggregates {
        return;
    }
    let current = queue.telemetry();
    if previous.as_ref() == Some(current) {
        return;
    }
    let marker = format!("brawler-queue aggregate {current:?}\n");
    // Explicit evidence runs inherit the worker's stderr. Production lobby refreshes stay quiet.
    let _ = std::io::Write::write_all(&mut std::io::stderr().lock(), marker.as_bytes());
    *previous = Some(current.clone());
}

/// Netcode may despawn its connection entity in the same deferred boundary that adds
/// `Disconnected`, so the polling cleanup above is not sufficient on its own. `On<Remove>` still
/// exposes the component being removed and guarantees the minimum lobby cannot retain a stale
/// participant after the entity is gone.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy removal observers receive On as an owned system parameter"
)]
fn lobby_client_removed(
    trigger: On<Remove, LobbyClient>,
    clients: Query<&LobbyClient>,
    losses: Option<ResMut<LobbySessionLosses>>,
    state: Option<ResMut<LobbyState>>,
    authority: Option<ResMut<crate::profiles::ProfileAuthority>>,
) {
    if let Ok(client) = clients.get(trigger.entity) {
        if let Some(mut authority) = authority {
            authority.remove_client(client.client_id.get());
        }
        if let Some(mut losses) = losses {
            losses.0.insert(client.lobby_session_id, client.client_id);
        } else if let Some(mut state) = state {
            // Minimal queue-less compositions still need the pre-M04 session cleanup behavior.
            state.remove_client(client.client_id.get());
        }
    }
}

/// Promote a source as soon as the lobby worker's routed Netcode connection reaches `Connected`.
/// This is deliberately independent of Brawler hello/session admission: a valid Netcode identity
/// can still receive a Brawler compatibility rejection, and it must not remain trapped at the
/// eight-datagram pre-auth budget while the client drains that response. The observer emits once
/// per Lightyear connection and carries only route/peer/client identity, never a fabricated lobby
/// session.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy observers receive On as an owned system parameter"
)]
fn lobby_netcode_authenticated(
    trigger: On<Add, Connected>,
    peers: Query<(&RemoteId, &RoutedPeer)>,
    mut identity_senders: Query<&mut MessageSender<LobbyServerIdentity>>,
    role: Res<ServerRoleResource>,
    mut outbox: ResMut<LobbyControlOutbox>,
) {
    let Ok((remote_id, peer)) = peers.get(trigger.entity) else {
        return;
    };
    let Some(netcode_client_id) =
        authenticated_netcode_id(remote_id).and_then(NetcodeClientId::new)
    else {
        return;
    };
    let _ = outbox.push_netcode_authenticated(LobbyNetcodeAuthenticatedBody {
        route_id: peer.route_id,
        peer_id: peer.peer_id,
        netcode_client_id,
    });
    if let (Some(manifest), Ok(mut sender)) = (
        role.lobby_manifest(),
        identity_senders.get_mut(trigger.entity),
    ) {
        sender.send::<SessionChannel>(LobbyServerIdentity {
            logical_server_id: manifest.common.logical_server_id.get(),
        });
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy resources are schedule-owned system parameters"
)]
fn initialize_lobby_state(
    mut commands: Commands,
    role: Res<ServerRoleResource>,
    config: Res<crate::config::ServerNetworkConfig>,
    catalog: Res<catalog::ResolvedLobbyCatalog>,
) {
    let Some(manifest) = role.lobby_manifest().cloned() else {
        return;
    };
    let Ok(build) = default_build_identity() else {
        return;
    };
    let profile_database_path = std::path::PathBuf::from(&manifest.profile_database_path);
    let profile_authority = crate::profiles::ProfileAuthority::start_with_catalog(
        profile_database_path,
        catalog.brawler_catalog.clone(),
    )
    .unwrap_or_else(|error| panic!("profile storage failed to start: {error}"));
    commands.insert_resource(profile_authority);
    commands.insert_resource(LobbyState::new(manifest, config.game_mode, build));
    commands.insert_resource(QueueState::new(&catalog));
}

#[allow(
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "the query is the one bounded lobby authentication transaction"
)]
fn lobby_receive_hellos(
    state: Res<LobbyState>,
    mut authority: ResMut<crate::profiles::ProfileAuthority>,
    mut pending: ResMut<PendingLobbyAdmissions>,
    mut receivers: Query<(
        Entity,
        &RemoteId,
        &mut MessageReceiver<LobbyHello>,
        &mut MessageSender<LobbyJoinOutcome>,
        Option<&RoutedPeer>,
        Has<Connected>,
        Has<Disconnected>,
    )>,
) {
    for (entity, remote_id, mut receiver, mut sender, routed_peer, connected, disconnected) in
        &mut receivers
    {
        if !connected || disconnected {
            continue;
        }
        let messages: Vec<_> = receiver.receive().collect();
        for hello in messages {
            let Some(client_id) = authenticated_netcode_id(remote_id) else {
                sender.send::<SessionChannel>(LobbyJoinOutcome::Rejected {
                    reason: LobbySessionError::InvalidClientId.rejection(),
                });
                continue;
            };
            let Some(peer) = routed_peer else {
                sender.send::<SessionChannel>(LobbyJoinOutcome::Rejected {
                    reason: LobbySessionError::NotRouted.rejection(),
                });
                continue;
            };
            if let Err(error) = state.validate_hello(&hello) {
                sender.send::<SessionChannel>(LobbyJoinOutcome::Rejected {
                    reason: error.rejection(),
                });
                continue;
            }
            if state.session_for_client(client_id).is_some() {
                continue;
            }
            if let Some(existing) = pending.0.get(&client_id) {
                if existing.hello != hello {
                    sender.send::<SessionChannel>(LobbyJoinOutcome::Rejected {
                        reason: LobbyJoinRejection::InvalidAccount,
                    });
                }
                continue;
            }
            match authority.begin_load(client_id, hello.account_id) {
                Ok(()) => {
                    pending.0.insert(
                        client_id,
                        PendingLobbyAdmission {
                            entity,
                            route_id: peer.route_id,
                            peer_id: peer.peer_id,
                            hello,
                        },
                    );
                }
                Err(error) => sender.send::<SessionChannel>(LobbyJoinOutcome::Rejected {
                    reason: profile_authority_join_rejection(&error),
                }),
            }
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "one ordered authority result pump atomically promotes pending sessions and publishes outcomes"
)]
fn process_profile_storage_results(
    mut commands: Commands,
    mut state: ResMut<LobbyState>,
    mut authority: ResMut<crate::profiles::ProfileAuthority>,
    mut pending: ResMut<PendingLobbyAdmissions>,
    mut outbox: ResMut<LobbyControlOutbox>,
    catalog: Res<catalog::ResolvedLobbyCatalog>,
    mut publications: ResMut<QueueSnapshotPublication>,
    mut clients: Query<(
        &RemoteId,
        &mut MessageSender<LobbyJoinOutcome>,
        &mut MessageSender<crate::profiles::ProfileOutcome>,
        Has<Connected>,
        Has<Disconnected>,
    )>,
) {
    let (loads, mutations) = authority
        .poll_loads()
        .unwrap_or_else(|error| panic!("profile storage executor failed: {error:?}"));
    for completion in loads {
        let Some(admission) = pending.0.remove(&completion.client_key) else {
            authority.remove_client(completion.client_key);
            continue;
        };
        let Ok((remote_id, mut sender, _, connected, disconnected)) =
            clients.get_mut(admission.entity)
        else {
            authority.remove_client(completion.client_key);
            continue;
        };
        if !connected
            || disconnected
            || authenticated_netcode_id(remote_id) != Some(completion.client_key)
        {
            authority.remove_client(completion.client_key);
            continue;
        }
        let profile = match completion.result {
            Ok(profile) => profile,
            Err(decision) => {
                sender.send::<SessionChannel>(LobbyJoinOutcome::Rejected {
                    reason: match decision {
                        crate::profiles::ProfileDecision::StorageFault => {
                            LobbyJoinRejection::StorageUnavailable
                        }
                        _ => LobbyJoinRejection::InvalidAccount,
                    },
                });
                authority.remove_client(completion.client_key);
                continue;
            }
        };
        let session = match state.accept_client(
            completion.client_key,
            admission.route_id,
            admission.peer_id,
            &admission.hello,
        ) {
            Ok(session) => session,
            Err(error) => {
                sender.send::<SessionChannel>(LobbyJoinOutcome::Rejected {
                    reason: error.rejection(),
                });
                authority.remove_client(completion.client_key);
                continue;
            }
        };
        commands.entity(admission.entity).insert(LobbyClient {
            client_id: session.netcode_client_id,
            lobby_session_id: session.lobby_session_id,
        });
        let _ = outbox.push_authenticated(LobbyAuthenticatedBody {
            route_id: session.route_id,
            peer_id: session.peer_id,
            lobby_session_id: session.lobby_session_id,
            netcode_client_id: session.netcode_client_id,
        });
        if state.mark_welcome_sent(session.netcode_client_id) {
            sender.send::<SessionChannel>(LobbyJoinOutcome::Accepted {
                logical_server_id: state.manifest.common.logical_server_id.get(),
                player_id: crate::protocol::PlayerId(session.player_id.get()),
                accepted_display_name: state
                    .accepted_name(session.netcode_client_id)
                    .expect("accepted session owns a name")
                    .to_string(),
                server_name: catalog.server_name.clone(),
                catalog_revision: catalog.revision,
                game_types: catalog.game_types.clone(),
                brawler_catalog: Box::new(catalog.brawler_catalog.clone()),
                profile: Box::new(profile),
            });
            publications.initial_pending = true;
        }
    }
    for (client_key, outcome) in mutations {
        for (remote_id, _, mut sender, connected, disconnected) in &mut clients {
            if connected && !disconnected && authenticated_netcode_id(remote_id) == Some(client_key)
            {
                sender.send::<ProfileChannel>(outcome.clone());
                break;
            }
        }
    }
}

#[allow(
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "the bounded profile command receiver serializes mutations before queue admission"
)]
fn collect_profile_commands(
    mut authority: ResMut<crate::profiles::ProfileAuthority>,
    queue: Res<QueueState>,
    mut clients: Query<(
        &LobbyClient,
        &mut MessageReceiver<crate::profiles::ProfileCommand>,
        &mut MessageSender<crate::profiles::ProfileOutcome>,
        Has<Disconnected>,
    )>,
) {
    for (client, mut receiver, mut sender, disconnected) in &mut clients {
        if disconnected {
            continue;
        }
        let queue_locked = queue.ticket_for_client(client.client_id).is_some();
        for command in receiver.receive().take(4) {
            match authority.submit_command(client.client_id.get(), command.clone(), queue_locked) {
                Ok(crate::profiles::ProfileMutationSubmission::Pending) => {}
                Ok(crate::profiles::ProfileMutationSubmission::Immediate(outcome)) => {
                    sender.send::<ProfileChannel>(outcome);
                }
                Err(crate::profiles::ProfileAuthorityError::StorageStopped) => {
                    panic!("profile storage executor stopped")
                }
                Err(error) => sender.send::<ProfileChannel>(crate::profiles::ProfileOutcome {
                    request_id: profile_command_request_id(&command),
                    decision: match error {
                        crate::profiles::ProfileAuthorityError::QueueLocked => {
                            crate::profiles::ProfileDecision::QueueLocked
                        }
                        crate::profiles::ProfileAuthorityError::InvalidRequest
                        | crate::profiles::ProfileAuthorityError::UnknownSession => {
                            crate::profiles::ProfileDecision::InvalidRequest
                        }
                        _ => crate::profiles::ProfileDecision::TemporarilyUnavailable,
                    },
                    snapshot: None,
                }),
            }
        }
    }
}

fn profile_command_request_id(command: &crate::profiles::ProfileCommand) -> u64 {
    match command {
        crate::profiles::ProfileCommand::CreateBrawler { request_id, .. }
        | crate::profiles::ProfileCommand::EditBrawler { request_id, .. }
        | crate::profiles::ProfileCommand::SelectBrawler { request_id, .. }
        | crate::profiles::ProfileCommand::DeleteBrawler { request_id, .. }
        | crate::profiles::ProfileCommand::EquipWeaponParts { request_id, .. } => *request_id,
    }
}

fn profile_authority_join_rejection(
    error: &crate::profiles::ProfileAuthorityError,
) -> LobbyJoinRejection {
    match error {
        crate::profiles::ProfileAuthorityError::AccountInUse => LobbyJoinRejection::AccountInUse,
        crate::profiles::ProfileAuthorityError::StorageStopped => {
            LobbyJoinRejection::StorageUnavailable
        }
        _ => LobbyJoinRejection::InvalidAccount,
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn begin_lobby_frame(
    state: Res<LobbyState>,
    mut frame: ResMut<LobbyQueueFrame>,
    mut losses: ResMut<LobbySessionLosses>,
    disconnected: Query<&LobbyClient, With<Disconnected>>,
) {
    frame.eligible = state.authenticated_session_ids();
    frame.collected.clear();
    frame.snapshot_changed = false;
    for client in &disconnected {
        losses.0.insert(client.lobby_session_id, client.client_id);
    }
}

#[allow(
    clippy::type_complexity,
    reason = "one bounded queue receiver query owns the authentication and per-update envelope cap"
)]
fn collect_queue_client_messages(
    mut commands: Commands,
    mut frame: ResMut<LobbyQueueFrame>,
    mut losses: ResMut<LobbySessionLosses>,
    mut queue: ResMut<QueueState>,
    mut receivers: Query<(
        Entity,
        Option<&LobbyClient>,
        &mut MessageReceiver<crate::lobby::QueueClientMessage>,
        Has<Disconnected>,
    )>,
) {
    for (entity, client, mut receiver, disconnected) in &mut receivers {
        // Taking five is enough to distinguish the allowed four-envelope frame from abuse.
        // Dropping Lightyear's draining iterator discards the rest without interpreting them.
        let messages: Vec<_> = receiver.receive().take(5).collect();
        if messages.is_empty() {
            continue;
        }
        let Some(client) = client else {
            queue.record_protocol_abuse();
            commands.entity(entity).insert(Disconnected::default());
            continue;
        };
        if disconnected {
            losses.0.insert(client.lobby_session_id, client.client_id);
            commands.entity(entity).insert(Disconnected::default());
            continue;
        }
        if !frame.eligible.contains(&client.lobby_session_id) || messages.len() == 5 {
            queue.record_protocol_abuse();
            losses.0.insert(client.lobby_session_id, client.client_id);
            commands.entity(entity).insert(Disconnected::default());
            continue;
        }
        frame.collected.push(CollectedQueueMessages {
            entity,
            session_id: client.lobby_session_id,
            messages,
        });
    }
}

#[allow(clippy::needless_pass_by_value, clippy::too_many_arguments)]
fn cleanup_disconnected_sessions(
    mut state: ResMut<LobbyState>,
    mut authority: ResMut<crate::profiles::ProfileAuthority>,
    mut outbox: ResMut<LobbyControlOutbox>,
    mut queue: ResMut<QueueState>,
    mut formation: ResMut<ProductFormationState>,
    mut frame: ResMut<LobbyQueueFrame>,
    mut losses: ResMut<LobbySessionLosses>,
    mut exit: MessageWriter<AppExit>,
    mut clients: Query<(
        &LobbyClient,
        &mut MessageSender<crate::lobby::MatchmakingServerMessage>,
    )>,
) {
    let losses = core::mem::take(&mut losses.0);
    for (session_id, client_id) in losses {
        authority.remove_client(client_id.get());
        frame.eligible.remove(&session_id);
        if formation
            .practice
            .as_ref()
            .is_some_and(|practice| practice.session_id == session_id)
        {
            if let Some(fact) = state.cancel_product_allocation() {
                let _ = outbox.push_activation_cancel(fact);
            }
            formation.clear_active();
        }
        let reserved = queue.ticket_for_session(session_id).and_then(|ticket| {
            queue
                .reservation_for_ticket(ticket.ticket_id)
                .map(|reservation| reservation.reservation_id)
        });
        if let Some(reservation_id) = reserved {
            let removed = queue.complete_reservation(reservation_id);
            if let Some(fact) = state.cancel_product_allocation() {
                let _ = outbox.push_activation_cancel(fact);
            }
            formation.clear_active();
            frame.snapshot_changed = true;
            for (client, mut sender) in &mut clients {
                let Some(ticket) = removed
                    .iter()
                    .find(|ticket| ticket.lobby_session_id == client.lobby_session_id)
                else {
                    continue;
                };
                sender.send::<SessionChannel>(crate::lobby::MatchmakingServerMessage {
                    sequence: formation.next_sequence(client.lobby_session_id),
                    phase: crate::lobby::MatchmakingServerPhase::Removed {
                        reservation_id,
                        ticket_id: Some(ticket.ticket_id),
                        reason: crate::lobby::MatchStartFailure::ParticipantLost,
                    },
                });
            }
        }
        frame.snapshot_changed |= queue.remove_session(session_id);
        state.detach_client(client_id.get());
    }
    if queue.revision_exhausted() {
        error!("lobby queue pool revision exhausted during disconnect cleanup");
        exit.write(AppExit::error());
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "the Bevy system adapts one ordered bounded queue transaction across immutable catalogs and connection ownership"
)]
fn apply_queue_transactions(
    mut commands: Commands,
    mut state: ResMut<LobbyState>,
    mut queue: ResMut<QueueState>,
    builds: Res<crate::builds::BuildCatalogResource>,
    weapons: Res<crate::combat::WeaponCatalogResource>,
    fighters: Res<FighterDefinitions>,
    authority: Res<crate::profiles::ProfileAuthority>,
    mut frame: ResMut<LobbyQueueFrame>,
    mut exit: MessageWriter<AppExit>,
) {
    frame.collected.sort_by_key(|collected| {
        state
            .session_by_lobby_id(collected.session_id)
            .map_or((u64::MAX, u64::MAX), |session| {
                let request =
                    collected
                        .messages
                        .first()
                        .map_or(u64::MAX, |message| match message {
                            crate::lobby::QueueClientMessage::Command { request_id, .. }
                            | crate::lobby::QueueClientMessage::OutcomeAck { request_id } => {
                                request_id.get()
                            }
                        });
                (session.player_id.get(), request)
            })
    });
    let collected = core::mem::take(&mut frame.collected);
    for collected in collected {
        if !frame.eligible.contains(&collected.session_id) {
            continue;
        }
        let Some(session) = state.session_by_lobby_id(collected.session_id) else {
            continue;
        };
        let mut disconnect = false;
        for message in collected.messages {
            let result = match message {
                crate::lobby::QueueClientMessage::OutcomeAck { request_id } => {
                    queue.acknowledge(session.lobby_session_id, request_id)
                }
                crate::lobby::QueueClientMessage::Command {
                    request_id,
                    command,
                } => {
                    let admitted = match &command {
                        crate::lobby::QueueCommand::Join(join) => fighters
                            .get(STANDARD_FIGHTER_DEFINITION)
                            .and_then(|fighter| {
                                authority
                                    .admitted_snapshot(
                                        session.netcode_client_id.get(),
                                        join.brawler_id,
                                        join.brawler_revision,
                                        &builds.0,
                                        &weapons.0,
                                        fighter,
                                    )
                                    .ok()
                            }),
                        crate::lobby::QueueCommand::Cancel(_) => None,
                    };
                    queue.command(
                        &session,
                        request_id,
                        command,
                        monotonic_millis(),
                        admitted,
                        &builds.0,
                        &weapons.0,
                        &fighters,
                    )
                }
            };
            if result.outcome_ready() {
                frame.pending_deliveries.insert(session.lobby_session_id);
            }
            frame.snapshot_changed |= result.snapshot_changed();
            if result.disconnect() {
                disconnect = true;
                break;
            }
        }
        if disconnect {
            frame.pending_deliveries.remove(&session.lobby_session_id);
            frame.snapshot_changed |= queue.remove_session(session.lobby_session_id);
            state.remove_client(session.netcode_client_id.get());
            frame.eligible.remove(&session.lobby_session_id);
            commands
                .entity(collected.entity)
                .insert(Disconnected::default());
            if queue.revision_exhausted() {
                error!("lobby queue pool revision exhausted during protocol cleanup");
                exit.write(AppExit::error());
            }
        }
    }
}

#[allow(
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "one Bevy system publishes both queue wire types to the same bounded client set"
)]
fn publish_queue_outcomes_and_snapshot(
    time: Res<Time>,
    mut queue: ResMut<QueueState>,
    mut frame: ResMut<LobbyQueueFrame>,
    mut publication: ResMut<QueueSnapshotPublication>,
    mut clients: Query<
        (
            &LobbyClient,
            &mut MessageSender<crate::lobby::QueueCommandOutcome>,
            &mut MessageSender<crate::lobby::QueuePoolSnapshot>,
        ),
        Without<Disconnected>,
    >,
) {
    if queue.revision_exhausted() {
        return;
    }
    for (client, mut outcome_sender, _) in &mut clients {
        if frame.pending_deliveries.remove(&client.lobby_session_id)
            && let Some(outcome) = queue.pending_outcome(client.lobby_session_id)
        {
            outcome_sender.send::<SessionChannel>(outcome.clone());
        }
    }

    publication.mutation_pending |= frame.snapshot_changed;
    let now = time.elapsed();
    let refresh_due = !clients.is_empty()
        && now.saturating_sub(publication.last_refresh) >= Duration::from_secs(1);
    let kind = if publication.mutation_pending {
        Some(SnapshotPublication::Mutation)
    } else if publication.initial_pending {
        Some(SnapshotPublication::Initial)
    } else if refresh_due {
        Some(SnapshotPublication::Refresh)
    } else {
        None
    };
    if let Some(kind) = kind {
        let snapshot = queue.snapshot();
        for (_, _, mut snapshot_sender) in &mut clients {
            snapshot_sender.send::<QueueSnapshotChannel>(snapshot.clone());
        }
        queue.record_snapshot_publication(kind);
        publication.initial_pending = false;
        publication.mutation_pending = false;
        publication.last_refresh = now;
    }
}

fn monotonic_millis() -> u64 {
    use std::sync::OnceLock;
    static START: OnceLock<std::time::Instant> = OnceLock::new();
    u64::try_from(
        START
            .get_or_init(std::time::Instant::now)
            .elapsed()
            .as_millis(),
    )
    .unwrap_or(u64::MAX)
}

fn authenticated_netcode_id(remote_id: &RemoteId) -> Option<u64> {
    match remote_id.0 {
        lightyear::prelude::PeerId::Netcode(client_id) => Some(client_id),
        _ => None,
    }
}

fn lobby_apply_control_frames(mut state: ResMut<LobbyState>, mut inbox: ResMut<LobbyControlInbox>) {
    for frame in inbox.drain() {
        let _ = state.apply_control_frame(frame);
    }
}

fn lobby_enqueue_allocation(mut state: ResMut<LobbyState>, mut outbox: ResMut<LobbyControlOutbox>) {
    let Ok(Some(body)) = state.unsent_allocate_request() else {
        return;
    };
    if outbox.push(body) {
        state.mark_allocate_request_queued();
    }
}

fn lobby_enqueue_product_allocation(
    mut state: ResMut<LobbyState>,
    mut outbox: ResMut<LobbyControlOutbox>,
) {
    let Some(body) = state.unsent_existing_request() else {
        return;
    };
    if outbox.push(body) {
        state.mark_allocate_request_queued();
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "Practice admission coordinates one bounded authoritative reservation transaction"
)]
fn apply_practice_start_requests(
    time: Res<Time<Real>>,
    mut state: ResMut<LobbyState>,
    queue: Res<QueueState>,
    catalog: Res<catalog::ResolvedLobbyCatalog>,
    builds: Res<crate::builds::BuildCatalogResource>,
    weapons: Res<crate::combat::WeaponCatalogResource>,
    fighters: Res<FighterDefinitions>,
    authority: Res<crate::profiles::ProfileAuthority>,
    mut formation: ResMut<ProductFormationState>,
    mut clients: Query<(
        &LobbyClient,
        &mut MessageReceiver<crate::lobby::PracticeStartRequest>,
        &mut MessageSender<crate::lobby::MatchmakingServerMessage>,
        Has<Connected>,
        Has<Disconnected>,
    )>,
) {
    let mut ordered: Vec<_> = clients.iter_mut().collect();
    ordered.sort_by_key(|(client, _, _, _, _)| client.lobby_session_id);
    for (client, mut receiver, mut sender, connected, disconnected) in ordered {
        let requests: Vec<_> = receiver.receive().take(2).collect();
        if !connected || disconnected {
            continue;
        }
        for command in requests.into_iter().take(1) {
            if let Some(practice) = formation.practice.clone() {
                if practice.session_id == client.lobby_session_id
                    && practice.request_id == command.request_id
                {
                    sender.send::<SessionChannel>(crate::lobby::MatchmakingServerMessage {
                        sequence: formation.next_sequence(client.lobby_session_id),
                        phase: crate::lobby::MatchmakingServerPhase::ReservationStarted(
                            practice.started.clone(),
                        ),
                    });
                } else {
                    sender.send::<SessionChannel>(crate::lobby::MatchmakingServerMessage {
                        sequence: formation.next_sequence(client.lobby_session_id),
                        phase: crate::lobby::MatchmakingServerPhase::PracticeRejected {
                            request_id: command.request_id,
                            reason: crate::lobby::PracticeStartRejection::Busy,
                        },
                    });
                }
                continue;
            }
            let rejection = if formation.active.is_some() || queue.reservation_count() != 0 {
                Some(crate::lobby::PracticeStartRejection::Busy)
            } else {
                None
            };
            let session = state
                .session_by_lobby_id(client.lobby_session_id)
                .expect("authenticated lobby client owns session");
            let started = rejection.map_or_else(
                || {
                    let fighter = fighters
                        .get(STANDARD_FIGHTER_DEFINITION)
                        .ok_or(crate::lobby::PracticeStartRejection::Internal)?;
                    let snapshot = authority
                        .admitted_snapshot(
                            session.netcode_client_id.get(),
                            command.brawler_id,
                            command.brawler_revision,
                            &builds.0,
                            &weapons.0,
                            fighter,
                        )
                        .map_err(|error| {
                            let rejection = match error {
                                crate::profiles::ProfileAuthorityError::IncompatibleBuild => {
                                    crate::lobby::PracticeStartRejection::IncompatibleBuild
                                }
                                crate::profiles::ProfileAuthorityError::UnknownSession
                                | crate::profiles::ProfileAuthorityError::TemporarilyUnavailable
                                | crate::profiles::ProfileAuthorityError::StorageStopped => {
                                    crate::lobby::PracticeStartRejection::Internal
                                }
                                crate::profiles::ProfileAuthorityError::AccountInUse
                                | crate::profiles::ProfileAuthorityError::AlreadyPending
                                | crate::profiles::ProfileAuthorityError::QueueLocked
                                | crate::profiles::ProfileAuthorityError::InvalidRequest
                                | crate::profiles::ProfileAuthorityError::IdentifierExhausted => {
                                    crate::lobby::PracticeStartRejection::InvalidBuild
                                }
                            };
                            warn!(
                                client_id = session.netcode_client_id.get(),
                                request_id = command.request_id.get(),
                                brawler_id = ?command.brawler_id,
                                brawler_revision = command.brawler_revision.get(),
                                cause = ?error,
                                ?rejection,
                                "practice build admission rejected"
                            );
                            rejection
                        })?;
                    let resolved =
                        snapshot
                            .resolve(&builds.0, &weapons.0, fighter)
                            .map_err(|error| {
                                warn!(
                                    client_id = session.netcode_client_id.get(),
                                    request_id = command.request_id.get(),
                                    brawler_id = ?command.brawler_id,
                                    brawler_revision = command.brawler_revision.get(),
                                    cause = ?error,
                                    "admitted practice build failed repeat resolution"
                                );
                                crate::lobby::PracticeStartRejection::IncompatibleBuild
                            })?;
                    let accepted = crate::builds::AcceptedBuildSummary {
                        canonical_recipe: crate::builds::BrawlerBuildRecipe {
                            weapon: crate::builds::WeaponChoice::Preset(
                                crate::combat::WeaponPresetId(snapshot.weapon_base_id.0),
                            ),
                            ultimate: snapshot.ultimate_id,
                            passives: snapshot.passive_ids,
                        },
                        identity: resolved.identity,
                        total_points: resolved.total_points,
                    };
                    state.create_practice_request(&session, &command, &catalog, snapshot, accepted)
                },
                Err,
            );
            match started {
                Ok(started) => {
                    formation.active = Some(started.reservation_id);
                    formation.practice = Some(PracticeFormation {
                        request_id: command.request_id,
                        session_id: client.lobby_session_id,
                        started: started.clone(),
                    });
                    formation.loading_deadline =
                        Some(time.elapsed().saturating_add(Duration::from_secs(30)));
                    formation.grant_deadline =
                        Some(time.elapsed().saturating_add(Duration::from_secs(10)));
                    sender.send::<SessionChannel>(crate::lobby::MatchmakingServerMessage {
                        sequence: formation.next_sequence(client.lobby_session_id),
                        phase: crate::lobby::MatchmakingServerPhase::ReservationStarted(started),
                    });
                }
                Err(reason) => {
                    sender.send::<SessionChannel>(crate::lobby::MatchmakingServerMessage {
                        sequence: formation.next_sequence(client.lobby_session_id),
                        phase: crate::lobby::MatchmakingServerPhase::PracticeRejected {
                            request_id: command.request_id,
                            reason,
                        },
                    });
                }
            }
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_lines,
    clippy::type_complexity
)]
fn apply_matchmaking_client_messages(
    mut state: ResMut<LobbyState>,
    mut outbox: ResMut<LobbyControlOutbox>,
    mut queue: ResMut<QueueState>,
    mut formation: ResMut<ProductFormationState>,
    mut frame: ResMut<LobbyQueueFrame>,
    mut clients: Query<(
        &LobbyClient,
        Option<&mut MessageReceiver<crate::lobby::MatchmakingClientMessage>>,
        &mut MessageSender<crate::lobby::MatchmakingServerMessage>,
    )>,
) {
    let mut cancel = None;
    for (_, receiver, _) in &mut clients {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for message in receiver.receive() {
            let crate::lobby::MatchmakingClientAction::Cancel {
                reservation_id,
                generation: 1,
            } = message.action
            else {
                continue;
            };
            if formation.active == Some(reservation_id) {
                cancel = Some(reservation_id);
            }
        }
    }
    let Some(reservation_id) = cancel else {
        return;
    };
    if let Some(practice) = formation.practice.clone() {
        if let Some(fact) = state.cancel_product_allocation() {
            let _ = outbox.push_activation_cancel(fact);
        }
        formation.clear_active();
        for (client, _, mut sender) in &mut clients {
            if client.lobby_session_id == practice.session_id {
                sender.send::<SessionChannel>(crate::lobby::MatchmakingServerMessage {
                    sequence: formation.next_sequence(client.lobby_session_id),
                    phase: crate::lobby::MatchmakingServerPhase::Removed {
                        reservation_id,
                        ticket_id: None,
                        reason: crate::lobby::MatchStartFailure::Cancelled,
                    },
                });
            }
        }
        return;
    }
    let removed = queue.complete_reservation(reservation_id);
    if let Some(fact) = state.cancel_product_allocation() {
        let _ = outbox.push_activation_cancel(fact);
    }
    formation.clear_active();
    frame.snapshot_changed = true;
    for (client, _, mut sender) in &mut clients {
        let Some(ticket) = removed
            .iter()
            .find(|ticket| ticket.lobby_session_id == client.lobby_session_id)
        else {
            continue;
        };
        sender.send::<SessionChannel>(crate::lobby::MatchmakingServerMessage {
            sequence: formation.next_sequence(client.lobby_session_id),
            phase: crate::lobby::MatchmakingServerPhase::Removed {
                reservation_id,
                ticket_id: Some(ticket.ticket_id),
                reason: crate::lobby::MatchStartFailure::Cancelled,
            },
        });
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
fn form_product_reservation(
    time: Res<Time<Real>>,
    mut state: ResMut<LobbyState>,
    mut queue: ResMut<QueueState>,
    catalog: Res<catalog::ResolvedLobbyCatalog>,
    mut formation: ResMut<ProductFormationState>,
    mut frame: ResMut<LobbyQueueFrame>,
    mut clients: Query<(
        &LobbyClient,
        &mut MessageSender<crate::lobby::MatchmakingServerMessage>,
        Has<Connected>,
        Has<Disconnected>,
    )>,
) {
    if state.free_match_slots == 0 || formation.active.is_some() || queue.reservation_count() != 0 {
        return;
    }
    let Ok(capability) = Capability::generate() else {
        return;
    };
    let bytes = capability.into_bytes();
    let Some(reservation_id) = crate::lobby::MatchReservationId::new(u128::from_le_bytes(
        bytes[..16].try_into().expect("capability prefix width"),
    )) else {
        return;
    };
    let live_sessions = clients
        .iter_mut()
        .filter_map(|(client, _, connected, disconnected)| {
            (connected && !disconnected).then_some(client.lobby_session_id)
        })
        .collect();
    let Some(reservation) = queue.reserve_oldest_exact(&catalog, reservation_id, &live_sessions)
    else {
        return;
    };
    if state
        .create_product_request(&reservation, &queue, &catalog)
        .is_err()
    {
        queue.complete_reservation(reservation_id);
        return;
    }
    formation.active = Some(reservation_id);
    formation.clear_handoff();
    formation.loading_deadline = Some(time.elapsed().saturating_add(Duration::from_secs(30)));
    formation.grant_deadline = Some(time.elapsed().saturating_add(Duration::from_secs(10)));
    frame.snapshot_changed = true;
    for (client, mut sender, connected, disconnected) in &mut clients {
        if !connected || disconnected {
            continue;
        }
        let Some(ticket) = queue.ticket_for_session(client.lobby_session_id) else {
            continue;
        };
        if queue.reservation_for_ticket(ticket.ticket_id).is_none() {
            continue;
        }
        sender.send::<SessionChannel>(crate::lobby::MatchmakingServerMessage {
            sequence: formation.next_sequence(client.lobby_session_id),
            phase: crate::lobby::MatchmakingServerPhase::ReservationStarted(
                crate::lobby::ReservationStarted {
                    reservation_id,
                    ticket_id: Some(ticket.ticket_id),
                    game_type_id: reservation.game_type_id.clone(),
                    map_preset_id: reservation.map_preset_id,
                    team_count: reservation.team_count,
                    players_per_team: reservation.players_per_team,
                    accepted_build: ticket.accepted_build,
                    loading_deadline_millis: 30_000,
                },
            ),
        });
    }
}

fn flush_deferred_product_cancel(
    mut state: ResMut<LobbyState>,
    mut outbox: ResMut<LobbyControlOutbox>,
) {
    if let Some(fact) = state.take_deferred_product_cancel() {
        let _ = outbox.push_activation_cancel(fact);
    }
}

#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
fn apply_product_dissolution(
    mut state: ResMut<LobbyState>,
    mut queue: ResMut<QueueState>,
    mut formation: ResMut<ProductFormationState>,
    mut frame: ResMut<LobbyQueueFrame>,
    mut clients: Query<(
        &LobbyClient,
        &mut MessageSender<crate::lobby::MatchmakingServerMessage>,
    )>,
) {
    if !state.take_product_dissolved() {
        return;
    }
    let Some(reservation_id) = formation.active else {
        return;
    };
    if let Some(practice) = formation.practice.clone() {
        formation.clear_active();
        for (client, mut sender) in &mut clients {
            if client.lobby_session_id == practice.session_id {
                sender.send::<SessionChannel>(crate::lobby::MatchmakingServerMessage {
                    sequence: formation.next_sequence(client.lobby_session_id),
                    phase: crate::lobby::MatchmakingServerPhase::Removed {
                        reservation_id,
                        ticket_id: None,
                        reason: crate::lobby::MatchStartFailure::WorkerFailed,
                    },
                });
            }
        }
        return;
    }
    let removed = queue.complete_reservation(reservation_id);
    formation.clear_active();
    frame.snapshot_changed = true;
    for (client, mut sender) in &mut clients {
        let Some(ticket) = removed
            .iter()
            .find(|ticket| ticket.lobby_session_id == client.lobby_session_id)
        else {
            continue;
        };
        sender.send::<SessionChannel>(crate::lobby::MatchmakingServerMessage {
            sequence: formation.next_sequence(client.lobby_session_id),
            phase: crate::lobby::MatchmakingServerPhase::Removed {
                reservation_id,
                ticket_id: Some(ticket.ticket_id),
                reason: crate::lobby::MatchStartFailure::WorkerFailed,
            },
        });
    }
}

fn apply_product_activation(
    mut state: ResMut<LobbyState>,
    mut queue: ResMut<QueueState>,
    mut formation: ResMut<ProductFormationState>,
    mut frame: ResMut<LobbyQueueFrame>,
) {
    if !state.take_product_activated() {
        return;
    }
    let Some(reservation_id) = formation.active else {
        return;
    };
    if formation.practice.is_some() {
        formation.clear_active();
        state.complete_product_activation();
        return;
    }
    let _active_tickets = queue.complete_reservation(reservation_id);
    formation.clear_active();
    state.complete_product_activation();
    frame.snapshot_changed = true;
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity
)]
fn expire_product_reservation(
    time: Res<Time<Real>>,
    mut state: ResMut<LobbyState>,
    mut outbox: ResMut<LobbyControlOutbox>,
    mut queue: ResMut<QueueState>,
    mut formation: ResMut<ProductFormationState>,
    mut frame: ResMut<LobbyQueueFrame>,
    mut clients: Query<(
        &LobbyClient,
        &mut MessageSender<crate::lobby::MatchmakingServerMessage>,
    )>,
) {
    let Some(reservation_id) = formation.active else {
        return;
    };
    let now = time.elapsed();
    let expected = if formation.practice.is_some() {
        1
    } else {
        queue
            .reservation(reservation_id)
            .map_or(0, |reservation| reservation.participants.len())
    };
    let grant_expired = formation.pending_grants.len() != expected
        && formation
            .grant_deadline
            .is_some_and(|deadline| now >= deadline);
    let loading_expired = formation
        .loading_deadline
        .is_some_and(|deadline| now >= deadline);
    let allocation_rejected = state.take_allocation_rejected();
    if !allocation_rejected && !grant_expired && !loading_expired {
        return;
    }

    let reason = if allocation_rejected {
        crate::lobby::MatchStartFailure::WorkerFailed
    } else {
        crate::lobby::MatchStartFailure::TimedOut
    };
    let practice = formation.practice.clone();
    let removed = if practice.is_some() {
        Vec::new()
    } else {
        queue.complete_reservation(reservation_id)
    };
    if let Some(fact) = state.cancel_product_allocation() {
        let _ = outbox.push_activation_cancel(fact);
    }
    formation.clear_active();
    frame.snapshot_changed |= practice.is_none();
    for (client, mut sender) in &mut clients {
        let ticket_id = if practice
            .as_ref()
            .is_some_and(|practice| practice.session_id == client.lobby_session_id)
        {
            None
        } else if let Some(ticket) = removed
            .iter()
            .find(|ticket| ticket.lobby_session_id == client.lobby_session_id)
        {
            Some(ticket.ticket_id)
        } else {
            continue;
        };
        sender.send::<SessionChannel>(crate::lobby::MatchmakingServerMessage {
            sequence: formation.next_sequence(client.lobby_session_id),
            phase: crate::lobby::MatchmakingServerPhase::Removed {
                reservation_id,
                ticket_id,
                reason,
            },
        });
    }
}

#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
fn lobby_deliver_product_grants(
    mut state: ResMut<LobbyState>,
    mut queue: ResMut<QueueState>,
    mut formation: ResMut<ProductFormationState>,
    mut clients: Query<(
        &LobbyClient,
        &mut MessageSender<crate::lobby::MatchmakingServerMessage>,
        &mut MessageSender<MatchRouteGrant>,
    )>,
) {
    let Some(reservation_id) = formation.active else {
        return;
    };
    let participant_count = if formation.practice.is_some() {
        1
    } else {
        queue
            .reservation(reservation_id)
            .map_or(0, |reservation| reservation.participants.len())
    };
    if participant_count == 0 {
        return;
    }

    // Collect the complete roster's grants before beginning any client migration.
    for (client, _, _) in &mut clients {
        if let Some(practice) = formation.practice.as_ref() {
            if practice.session_id == client.lobby_session_id
                && !formation
                    .pending_grants
                    .contains_key(&client.lobby_session_id)
                && let Some(grant) = state.take_route_grant(client.client_id.get())
            {
                formation
                    .pending_grants
                    .insert(client.lobby_session_id, grant);
            }
            continue;
        }
        let Some(ticket) = queue.ticket_for_session(client.lobby_session_id) else {
            continue;
        };
        if queue.reservation_for_ticket(ticket.ticket_id).is_none()
            || formation
                .pending_grants
                .contains_key(&client.lobby_session_id)
        {
            continue;
        }
        if let Some(grant) = state.take_route_grant(client.client_id.get()) {
            formation
                .pending_grants
                .insert(client.lobby_session_id, grant);
        }
    }
    if formation.pending_grants.len() != participant_count {
        return;
    }

    if formation.practice.is_none() {
        queue.mark_reservation_handoff_ready(reservation_id);
    }

    for (client, mut phase_sender, mut compatibility_sender) in &mut clients {
        if formation.begin_sent.contains(&client.lobby_session_id) {
            continue;
        }
        let Some(grant) = formation
            .pending_grants
            .get(&client.lobby_session_id)
            .copied()
        else {
            continue;
        };
        phase_sender.send::<SessionChannel>(crate::lobby::MatchmakingServerMessage {
            sequence: formation.next_sequence(client.lobby_session_id),
            phase: crate::lobby::MatchmakingServerPhase::BeginMatchConnect(
                crate::lobby::BeginMatchConnect {
                    reservation_id,
                    generation: 1,
                    grant,
                },
            ),
        });
        compatibility_sender.send::<SessionChannel>(grant);
        formation.begin_sent.insert(client.lobby_session_id);
    }
}

fn lobby_deliver_grants(
    mut state: ResMut<LobbyState>,
    mut clients: Query<(&LobbyClient, &mut MessageSender<MatchRouteGrant>)>,
) {
    for (client, mut sender) in &mut clients {
        if let Some(grant) = state.take_route_grant(client.client_id.get()) {
            sender.send::<SessionChannel>(grant);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use brawler_routing::{AllocationId, LogicalServerId, ManifestCommon, MatchId, WorkerRole};

    fn manifest() -> LobbyManifest {
        LobbyManifest {
            common: ManifestCommon {
                manifest_version: 2,
                role: WorkerRole::Lobby,
                logical_server_id: LogicalServerId::new(1).unwrap(),
                process_id: ProcessId::new(2).unwrap(),
                worker_id: WorkerId::new(3).unwrap(),
                generation: brawler_routing::Generation::new(4).unwrap(),
                network_protocol: crate::protocol::NETWORK_PROTOCOL_ID,
                protocol_registry_fingerprint: 7,
                content_fingerprint: 8,
                route_version: brawler_routing::ROUTE_VERSION_V1,
                packet_version: brawler_routing::PACKET_VERSION_V1,
                control_version: brawler_routing::CONTROL_VERSION_CURRENT,
                flags: 0,
            },
            default_route_id: RouteId::new(9).unwrap(),
            max_authenticated_sessions: 32,
            outstanding_allocations: 2,
            active_matches: 2,
            heartbeat_ms: 100,
            profile_database_path: std::env::temp_dir()
                .join(format!(
                    "brawler-lobby-profiles-{}.sqlite3",
                    std::process::id()
                ))
                .to_string_lossy()
                .into_owned(),
            raw_catalog: include_bytes!("../../../config/server/game-types.ron").to_vec(),
            raw_catalog_fingerprint: brawler_routing::raw_catalog_fingerprint(include_bytes!(
                "../../../config/server/game-types.ron"
            )),
            nonce: 11,
            digest: [0; 32],
        }
        .with_digest()
        .unwrap()
    }

    fn build() -> LobbyBuildIdentity {
        LobbyBuildIdentity {
            recipe_fingerprint: 123,
            build_revision: 4,
            snapshot: default_build_identity().unwrap().snapshot,
        }
    }

    fn hello() -> LobbyHello {
        LobbyHello {
            protocol_version: SUPPORTED_PROTOCOL_VERSION,
            build_version: VERSION.to_string(),
            registry_fingerprint: 7,
            content_fingerprint: GameplayContentFingerprint(8),
            account_id: crate::profiles::AccountId::new(1).unwrap(),
            proposed_display_name: "Brawler-Test".to_string(),
        }
    }

    fn state() -> LobbyState {
        LobbyState::with_id_source(
            manifest(),
            GameMode::Wipeout,
            build(),
            DeterministicSessionIds { next: 1 },
        )
    }

    struct DeterministicSessionIds {
        next: u128,
    }

    impl LobbySessionIdSource for DeterministicSessionIds {
        fn next(&mut self) -> Option<LobbySessionId> {
            let id = LobbySessionId::new(self.next)?;
            self.next = self.next.checked_add(1)?;
            Some(id)
        }
    }

    fn admit_two(state: &mut LobbyState) {
        let hello = hello();
        state
            .accept_client(
                11,
                RouteId::new(20).unwrap(),
                PeerId::new(21).unwrap(),
                &hello,
            )
            .unwrap();
        state
            .accept_client(
                12,
                RouteId::new(22).unwrap(),
                PeerId::new(23).unwrap(),
                &hello,
            )
            .unwrap();
    }

    #[test]
    fn practice_uses_one_human_and_fills_the_selected_roster_with_named_bots() {
        let mut lobby = state();
        lobby.free_match_slots = 1;
        let session = lobby
            .accept_client(
                11,
                RouteId::new(20).unwrap(),
                PeerId::new(21).unwrap(),
                &hello(),
            )
            .unwrap();
        let catalog =
            resolve_operator_catalog(include_bytes!("../../../config/server/game-types.ron"))
                .unwrap();
        let game = catalog
            .game_types
            .iter()
            .find(|game| game.id.as_str() == "hot-zone-3v3")
            .unwrap();
        let builds = BuildCatalog::embedded().unwrap();
        let weapons = WeaponCatalog::embedded().unwrap();
        let command = crate::lobby::PracticeStartRequest {
            request_id: crate::lobby::PracticeRequestId::new(1).unwrap(),
            catalog_revision: catalog.revision,
            game_type_id: game.id.clone(),
            game_type_configuration_revision: game.configuration_revision,
            brawler_id: crate::profiles::SavedBrawlerId::new(1).unwrap(),
            brawler_revision: crate::profiles::ProfileRevision::INITIAL,
        };
        let fighter = FighterDefinitions::default();
        let brawler = crate::profiles::SavedBrawler {
            id: command.brawler_id,
            creation_ordinal: 1,
            name: "Practice".into(),
            fighter_profile_id: crate::profiles::FighterProfileId(1),
            weapon_base_id: crate::profiles::WeaponBaseId(1),
            ultimate_id: crate::builds::UltimateDefinitionId(1),
            passive_ids: [
                crate::builds::PassiveDefinitionId(3),
                crate::builds::PassiveDefinitionId(4),
            ],
            equipped_part_ids: [None; crate::weapon_parts::WEAPON_PART_SLOT_COUNT],
            revision: command.brawler_revision,
        };
        let snapshot = crate::profiles::MatchBuildSnapshotV3::from_brawler(
            &brawler,
            &builds,
            &weapons,
            fighter.get(STANDARD_FIGHTER_DEFINITION).unwrap(),
        )
        .unwrap();
        let resolved = snapshot
            .resolve(
                &builds,
                &weapons,
                fighter.get(STANDARD_FIGHTER_DEFINITION).unwrap(),
            )
            .unwrap();
        let accepted = crate::builds::AcceptedBuildSummary {
            canonical_recipe: crate::builds::BrawlerBuildRecipe {
                weapon: crate::builds::WeaponChoice::Preset(crate::combat::WeaponPresetId(1)),
                ultimate: brawler.ultimate_id,
                passives: brawler.passive_ids,
            },
            identity: resolved.identity,
            total_points: resolved.total_points,
        };

        let started = lobby
            .create_practice_request(&session, &command, &catalog, snapshot, accepted)
            .unwrap();
        let request = &lobby.pending.as_ref().unwrap().body;
        assert_eq!(started.ticket_id, None);
        assert_eq!(started.map_preset_id, crate::map::SWITCHBACK_BASIN_PRESET);
        assert_eq!(request.map_preset, crate::map::SWITCHBACK_BASIN_PRESET.0);
        assert_eq!(request.participants.len(), 1);
        assert_eq!(request.bots.len(), 5);
        assert_eq!(
            request
                .bots
                .iter()
                .map(|bot| bot.display_name.as_str())
                .collect::<Vec<_>>(),
            vec!["Bot 1", "Bot 2", "Bot 3", "Bot 4", "Bot 5"]
        );
        assert_eq!(request.bots.iter().filter(|bot| bot.team == 0).count(), 2);
        assert_eq!(request.bots.iter().filter(|bot| bot.team == 1).count(), 3);
        let canonical = practice_bot_build_identity().unwrap();
        assert!(request.bots.iter().all(|bot| {
            bot.recipe_fingerprint == canonical.recipe_fingerprint
                && bot.build_revision == canonical.build_revision
                && bot.build_snapshot == canonical.snapshot
        }));
    }

    #[test]
    fn despawned_lobby_client_cannot_leave_a_stale_allocation_participant() {
        let mut lobby = state();
        let session = lobby
            .accept_client(
                11,
                RouteId::new(20).unwrap(),
                PeerId::new(21).unwrap(),
                &hello(),
            )
            .unwrap();
        let mut app = App::new();
        app.insert_resource(lobby)
            .add_observer(lobby_client_removed);
        let entity = app
            .world_mut()
            .spawn(LobbyClient {
                client_id: session.netcode_client_id,
                lobby_session_id: session.lobby_session_id,
            })
            .id();

        app.world_mut().despawn(entity);

        assert!(app.world().resource::<LobbyState>().sessions.is_empty());
        assert!(app.world().resource::<LobbyState>().pending.is_none());
    }

    #[test]
    fn sessions_are_bounded_and_ids_are_fresh_nonzero() {
        let mut state = state();
        let hello = hello();
        let mut ids = BTreeSet::new();
        for client in 1..=32 {
            let session = state
                .accept_client(
                    client,
                    RouteId::new(u128::from(client)).unwrap(),
                    PeerId::new(u128::from(client)).unwrap(),
                    &hello,
                )
                .unwrap();
            assert!(ids.insert(session.lobby_session_id));
            assert_ne!(session.lobby_session_id.get(), 0);
        }
        assert_eq!(state.session_count(), MAX_AUTHENTICATED_LOBBY_SESSIONS);
        assert_eq!(
            state.accept_client(
                33,
                RouteId::new(33).unwrap(),
                PeerId::new(33).unwrap(),
                &hello
            ),
            Err(LobbySessionError::ServerFull)
        );
    }

    #[test]
    fn compatibility_and_non_netcode_identity_are_rejected() {
        let mut state = state();
        let mut bad = hello();
        bad.protocol_version += 1;
        assert_eq!(
            state.accept_client(1, RouteId::new(1).unwrap(), PeerId::new(1).unwrap(), &bad),
            Err(LobbySessionError::ProtocolVersionMismatch)
        );
        assert_eq!(
            state.accept_client(
                0,
                RouteId::new(1).unwrap(),
                PeerId::new(1).unwrap(),
                &hello()
            ),
            Err(LobbySessionError::InvalidClientId)
        );
    }

    #[test]
    fn exact_two_roster_emits_one_idempotent_request() {
        let mut state = state();
        admit_two(&mut state);
        let first = state.pending_allocate_request().unwrap().unwrap();
        let second = state.pending_allocate_request().unwrap().unwrap();
        assert_eq!(first, second);
        let body = first;
        assert_eq!(body.participants.len(), M01_PARTICIPANT_COUNT);
        assert_eq!(body.request_id.get(), 1);
        assert_eq!(body.participants[0].lobby_session_id.get(), 1);
        assert_eq!(body.participants[1].lobby_session_id.get(), 2);
    }

    #[test]
    fn accepted_names_suffix_deterministically_and_welcome_is_once_per_session() {
        let mut state = state();
        for client_id in [11, 12, 13] {
            state
                .accept_client(
                    client_id,
                    RouteId::new(u128::from(client_id) + 10).unwrap(),
                    PeerId::new(u128::from(client_id) + 20).unwrap(),
                    &hello(),
                )
                .unwrap();
        }
        assert_eq!(
            state.accepted_name(NetcodeClientId::new(11).unwrap()),
            Some("Brawler-Test")
        );
        assert_eq!(
            state.accepted_name(NetcodeClientId::new(12).unwrap()),
            Some("Brawler-Test #2")
        );
        let third = NetcodeClientId::new(13).unwrap();
        assert_eq!(state.accepted_name(third), Some("Brawler-Test #3"));
        assert!(state.mark_welcome_sent(third));
        assert!(!state.mark_welcome_sent(third));
        assert!(state.remove_client(13).is_some());
        assert!(state.mark_welcome_sent(third));
    }

    #[test]
    fn matching_grants_are_delivered_once_to_authenticated_clients() {
        let mut state = state();
        admit_two(&mut state);
        let request = state.pending_allocate_request().unwrap().unwrap();
        let request_id = request.request_id;
        let grants = [11_u64, 12_u64]
            .into_iter()
            .zip([1_u128, 2_u128])
            .map(|(client, session)| {
                let _ = client;
                brawler_routing::AllocationGrant {
                    lobby_session_id: LobbySessionId::new(session).unwrap(),
                    route_id: RouteId::new(30 + session).unwrap(),
                    peer_id: PeerId::new(40 + session).unwrap(),
                    capability: Capability::from_bytes([u8::try_from(session).unwrap(); 32])
                        .unwrap(),
                    activation_expiry_unix_ms: 100,
                    route_expiry_unix_ms: 200,
                }
            })
            .collect();
        state
            .apply_allocation_granted(AllocationGrantedBody {
                request_id,
                allocation_id: AllocationId::new(5).unwrap(),
                match_id: MatchId::new(6).unwrap(),
                worker_id: WorkerId::new(7).unwrap(),
                grants,
            })
            .unwrap();
        let first = state.take_route_grant(11).unwrap();
        assert_eq!(first.request_id, request_id);
        assert_eq!(first.route_id.get(), 31);
        assert!(state.take_route_grant(11).is_none());
        assert_eq!(state.take_route_grant(12).unwrap().route_id.get(), 32);
        assert!(state.pending_allocate_request().unwrap().is_none());
    }

    #[test]
    fn completed_clients_can_return_to_fresh_lobby_without_implicit_requeue() {
        let mut state = state();
        admit_two(&mut state);
        let request_id = state
            .pending_allocate_request()
            .unwrap()
            .unwrap()
            .request_id;
        let grants = [1_u128, 2_u128]
            .into_iter()
            .map(|session| brawler_routing::AllocationGrant {
                lobby_session_id: LobbySessionId::new(session).unwrap(),
                route_id: RouteId::new(30 + session).unwrap(),
                peer_id: PeerId::new(40 + session).unwrap(),
                capability: Capability::from_bytes([u8::try_from(session).unwrap(); 32]).unwrap(),
                activation_expiry_unix_ms: 100,
                route_expiry_unix_ms: 200,
            })
            .collect();
        state
            .apply_allocation_granted(AllocationGrantedBody {
                request_id,
                allocation_id: AllocationId::new(5).unwrap(),
                match_id: MatchId::new(6).unwrap(),
                worker_id: WorkerId::new(7).unwrap(),
                grants,
            })
            .unwrap();
        assert_eq!(state.allocated_clients.len(), M01_PARTICIPANT_COUNT);

        // A routed handoff creates a fresh lobby session and fresh route/peer identities for the
        // same Netcode IDs. M01 accepts that session for the return observation but does not
        // create another allocation request.
        assert!(state.remove_client(11).is_some());
        assert!(state.remove_client(12).is_some());
        state
            .accept_client(
                11,
                RouteId::new(50).unwrap(),
                PeerId::new(51).unwrap(),
                &hello(),
            )
            .unwrap();
        state
            .accept_client(
                12,
                RouteId::new(52).unwrap(),
                PeerId::new(53).unwrap(),
                &hello(),
            )
            .unwrap();
        assert!(state.pending_allocate_request().unwrap().is_none());

        // Distinct new authenticated identities remain eligible in the same long-lived lobby
        // process; this guard is identity-scoped rather than a global completed flag.
        assert!(state.remove_client(11).is_some());
        assert!(state.remove_client(12).is_some());
        state
            .accept_client(
                13,
                RouteId::new(60).unwrap(),
                PeerId::new(61).unwrap(),
                &hello(),
            )
            .unwrap();
        state
            .accept_client(
                14,
                RouteId::new(62).unwrap(),
                PeerId::new(63).unwrap(),
                &hello(),
            )
            .unwrap();
        let request = state.pending_allocate_request().unwrap().unwrap();
        assert_eq!(
            request
                .participants
                .iter()
                .map(|participant| participant.netcode_client_id.get())
                .collect::<Vec<_>>(),
            vec![13, 14]
        );
    }

    #[test]
    fn explicit_queue_can_allocate_returning_players_again() {
        let mut state = state();
        admit_two(&mut state);
        let mut request = state.pending_allocate_request().unwrap().unwrap();
        let first_grants = [1_u128, 2_u128]
            .into_iter()
            .map(|session| brawler_routing::AllocationGrant {
                lobby_session_id: LobbySessionId::new(session).unwrap(),
                route_id: RouteId::new(30 + session).unwrap(),
                peer_id: PeerId::new(40 + session).unwrap(),
                capability: Capability::from_bytes([u8::try_from(session).unwrap(); 32]).unwrap(),
                activation_expiry_unix_ms: 100,
                route_expiry_unix_ms: 200,
            })
            .collect();
        state
            .apply_allocation_granted(AllocationGrantedBody {
                request_id: request.request_id,
                allocation_id: AllocationId::new(5).unwrap(),
                match_id: MatchId::new(6).unwrap(),
                worker_id: WorkerId::new(7).unwrap(),
                grants: first_grants,
            })
            .unwrap();
        for (client, route, peer) in [(11, 50, 51), (12, 52, 53)] {
            assert!(state.remove_client(client).is_some());
            state
                .accept_client(
                    client,
                    RouteId::new(route).unwrap(),
                    PeerId::new(peer).unwrap(),
                    &hello(),
                )
                .unwrap();
        }

        let mut sessions: Vec<_> = state.sessions.values().copied().collect();
        sessions.sort_by_key(|session| session.lobby_session_id);
        request.request_id = RequestId::new(2).unwrap();
        request.lobby_session_id = sessions[0].lobby_session_id;
        for (participant, session) in request.participants.iter_mut().zip(&sessions) {
            participant.lobby_session_id = session.lobby_session_id;
            participant.player_id = session.player_id;
            participant.netcode_client_id = session.netcode_client_id;
        }
        state.pending = Some(PendingAllocation {
            body: request,
            sent: true,
        });
        let grants = sessions
            .iter()
            .enumerate()
            .map(|(index, session)| brawler_routing::AllocationGrant {
                lobby_session_id: session.lobby_session_id,
                route_id: RouteId::new(70 + index as u128).unwrap(),
                peer_id: PeerId::new(80 + index as u128).unwrap(),
                capability: Capability::from_bytes([u8::try_from(index + 3).unwrap(); 32]).unwrap(),
                activation_expiry_unix_ms: 100,
                route_expiry_unix_ms: 200,
            })
            .collect();
        state
            .apply_allocation_granted(AllocationGrantedBody {
                request_id: RequestId::new(2).unwrap(),
                allocation_id: AllocationId::new(15).unwrap(),
                match_id: MatchId::new(16).unwrap(),
                worker_id: WorkerId::new(17).unwrap(),
                grants,
            })
            .unwrap();
        assert_eq!(state.allocated_clients.len(), M01_PARTICIPANT_COUNT);
        assert!(state.take_route_grant(11).is_some());
        assert!(state.take_route_grant(12).is_some());
    }

    #[test]
    fn mismatched_allocation_response_does_not_consume_request() {
        let mut state = state();
        admit_two(&mut state);
        let request = state.pending_allocate_request().unwrap().unwrap();
        let request_id = request.request_id;
        let error = state
            .apply_allocation_rejected(AllocationRejectedBody {
                request_id: RequestId::new(request_id.get() + 1).unwrap(),
                reason: 1,
                retry_after_ms: 10,
            })
            .unwrap_err();
        assert_eq!(error, LobbyAllocationError::RequestMismatch);
        assert!(state.pending_allocate_request().unwrap().is_some());
    }

    #[test]
    fn disconnect_revokes_pending_transaction_once() {
        let mut state = state();
        admit_two(&mut state);
        assert!(state.pending_allocate_request().unwrap().is_some());
        assert!(state.remove_client(11).is_some());
        assert!(state.remove_client(11).is_none());
        assert!(state.pending_allocate_request().unwrap().is_none());
        assert_eq!(state.session_count(), 1);
    }
}
