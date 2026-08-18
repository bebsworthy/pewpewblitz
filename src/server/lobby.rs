//! Bounded M01 lobby-worker session and allocation state.
//!
//! This module owns only the authenticated lobby roster and its supervisor allocation transaction.
//! It deliberately has no map, physics, combat, match, or terrain authority.  The pure state
//! machine is kept separate from the Bevy adapters so deterministic codec and idempotency tests do
//! not need a running network endpoint.

use super::{LobbyControlInbox, LobbyControlOutbox, RoutedPeer, ServerRoleResource};
use crate::{
    VERSION,
    builds::{BuildCatalog, BuildPresetId, resolve_build_recipe},
    combat::{FighterDefinitions, STANDARD_FIGHTER_DEFINITION, WeaponCatalog},
    config::GameMode,
    content::GameplayContentFingerprint,
    protocol::{
        ClientHello, JoinOutcome, JoinRejection, MatchRouteGrantV1, RouteCapability,
        SUPPORTED_PROTOCOL_VERSION, SessionChannel,
    },
};
use bevy::prelude::*;
use brawler_routing::{
    AllocateParticipant, AllocateRequestBody, AllocationGrantedBody, AllocationRejectedBody,
    Capability, CodecError, ControlBody, ControlFrame, LobbyAuthenticatedBody, LobbyManifestV1,
    LobbyNetcodeAuthenticatedBody, LobbySessionId, NetcodeClientId, PeerId, PlayerId, RequestId,
    RouteId,
};
#[cfg(test)]
use brawler_routing::{ProcessId, WorkerId};
use lightyear::prelude::{Connected, Disconnected, MessageReceiver, MessageSender, RemoteId};
use std::collections::{BTreeMap, BTreeSet};

/// M01's hard upper bound for authenticated lobby sessions.
pub const MAX_AUTHENTICATED_LOBBY_SESSIONS: usize = 32;
/// M01 allocates one match only when exactly two authenticated sessions are present.
pub const M01_PARTICIPANT_COUNT: usize = 2;
/// Bounded process-lifetime memory for identities that already participated in an allocation.
/// M01 does not own Queue Again/requeue, so a returned client may authenticate a fresh lobby
/// session without immediately forming another automatic match.
pub const MAX_ALLOCATED_LOBBY_CLIENT_IDS: usize = MAX_AUTHENTICATED_LOBBY_SESSIONS * 8;

/// Build identity carried in the supervisor allocation request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LobbyBuildIdentity {
    pub source_build_preset: Option<u16>,
    pub recipe_fingerprint: u64,
    pub build_revision: u16,
}

/// Resolve the embedded default build without installing any gameplay authority.
pub fn default_build_identity() -> Result<LobbyBuildIdentity, String> {
    let builds = BuildCatalog::embedded()?;
    let weapons = WeaponCatalog::embedded()?;
    let fighters = FighterDefinitions::default();
    let fighter = fighters
        .get(STANDARD_FIGHTER_DEFINITION)
        .ok_or_else(|| "standard fighter definition is missing".to_string())?;
    let preset = builds
        .preset(BuildPresetId(1))
        .ok_or_else(|| "default build preset is missing".to_string())?;
    let resolved = resolve_build_recipe(&builds, &weapons, fighter, preset.recipe, Some(preset.id))
        .map_err(|error| format!("default build resolution failed: {error:?}"))?;
    Ok(LobbyBuildIdentity {
        source_build_preset: resolved.identity.source_build_preset_id.map(|id| id.0),
        recipe_fingerprint: resolved.identity.recipe_fingerprint.0,
        build_revision: resolved.identity.revision.0,
    })
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
    IdentifierExhausted,
}

impl LobbySessionError {
    const fn rejection(self) -> JoinRejection {
        match self {
            Self::ProtocolVersionMismatch => JoinRejection::ProtocolVersionMismatch,
            Self::BuildVersionMismatch => JoinRejection::BuildVersionMismatch,
            Self::RegistryMismatch => JoinRejection::RegistryMismatch,
            Self::ContentMismatch => JoinRejection::ContentMismatch,
            Self::InvalidClientId
            | Self::NotRouted
            | Self::ServerFull
            | Self::IdentifierExhausted => JoinRejection::ServerFull,
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
pub struct LobbyState {
    manifest: LobbyManifestV1,
    mode: GameMode,
    build: LobbyBuildIdentity,
    next_player_id: u64,
    next_network_entity_id: u64,
    next_request_id: u64,
    session_ids: Box<dyn LobbySessionIdSource>,
    sessions: BTreeMap<NetcodeClientId, LobbySession>,
    /// These tombstones intentionally survive lobby-session teardown. Route, peer, and lobby
    /// session IDs are all fresh on handoff; the authenticated Netcode ID is the stable identity
    /// that prevents M01 from approximating M06 requeue.
    allocated_clients: BTreeSet<NetcodeClientId>,
    pending: Option<PendingAllocation>,
    allocation_completed: bool,
    grants: BTreeMap<NetcodeClientId, MatchRouteGrantV1>,
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
    pub fn new(manifest: LobbyManifestV1, mode: GameMode, build: LobbyBuildIdentity) -> Self {
        Self::with_id_source(manifest, mode, build, OsLobbySessionIdSource)
    }

    #[must_use]
    pub fn with_id_source<S>(
        manifest: LobbyManifestV1,
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
            allocated_clients: BTreeSet::new(),
            pending: None,
            allocation_completed: false,
            grants: BTreeMap::new(),
        }
    }

    #[must_use]
    pub fn manifest(&self) -> &LobbyManifestV1 {
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

    fn validate_hello(&self, hello: &ClientHello) -> Result<(), LobbySessionError> {
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
        Ok(())
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
        hello: &ClientHello,
    ) -> Result<LobbySession, LobbySessionError> {
        self.validate_hello(hello)?;
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
        self.sessions.insert(client_id, session);
        Ok(session)
    }

    /// Remove a disconnected client and revoke any local grant. BRCT v1 has no cancellation body,
    /// so a pending allocation is cancelled locally and a late supervisor response is ignored.
    pub fn remove_client(&mut self, client_id: u64) -> Option<LobbySession> {
        let id = NetcodeClientId::new(client_id)?;
        let session = self.sessions.remove(&id)?;
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
            .map(|session| AllocateParticipant {
                lobby_session_id: session.lobby_session_id,
                player_id: session.player_id,
                netcode_client_id: session.netcode_client_id,
                team: session.team,
                source_build_preset: session.build.source_build_preset,
                recipe_fingerprint: session.build.recipe_fingerprint,
                build_revision: session.build.build_revision,
            })
            .collect();
        let body = AllocateRequestBody {
            request_id,
            lobby_session_id: sessions[0].lobby_session_id,
            mode: match self.mode {
                GameMode::Wipeout => brawler_routing::GameMode::Wipeout,
                GameMode::HotZone => brawler_routing::GameMode::HotZone,
            },
            participants,
        };
        self.pending = Some(PendingAllocation { body, sent: false });
        Ok(())
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
        if body.grants.len() != M01_PARTICIPANT_COUNT {
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
            let route_grant = MatchRouteGrantV1 {
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
        if converted
            .iter()
            .any(|(client_id, _)| self.allocated_clients.contains(client_id))
            || self.allocated_clients.len().saturating_add(converted.len())
                > MAX_ALLOCATED_LOBBY_CLIENT_IDS
        {
            return Err(LobbyAllocationError::AllocationIdentityMemoryFull);
        }
        for (client_id, grant) in converted {
            self.allocated_clients.insert(client_id);
            self.grants.insert(client_id, grant);
        }
        self.pending = None;
        self.allocation_completed = true;
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
        Ok(())
    }

    /// Apply one decoded worker control body.  The worker control adapter can feed this message
    /// without giving the lobby direct ownership of the shared IPC reader.
    pub fn apply_control_frame(&mut self, frame: ControlFrame) -> Result<(), LobbyAllocationError> {
        match frame.body {
            ControlBody::AllocationGranted(body) => self.apply_allocation_granted(body),
            ControlBody::AllocationRejected(body) => self.apply_allocation_rejected(body),
            _ => Err(LobbyAllocationError::RequestMismatch),
        }
    }

    /// Take one grant for the authenticated client.  A grant is never logged and is delivered at
    /// most once.
    pub fn take_route_grant(&mut self, client_id: u64) -> Option<MatchRouteGrantV1> {
        NetcodeClientId::new(client_id).and_then(|id| self.grants.remove(&id))
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

impl core::fmt::Debug for LobbyClient {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("LobbyClient")
            .field("identity", &"[REDACTED]")
            .finish()
    }
}

/// Installs only lobby session, allocation, and grant-delivery systems.
pub struct LobbyPlugin;

impl Plugin for LobbyPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<LobbyControlInbox>()
            .init_resource::<LobbyControlOutbox>()
            .add_systems(Startup, initialize_lobby_state)
            .add_systems(
                Update,
                (
                    lobby_cleanup_disconnected,
                    lobby_receive_hellos,
                    lobby_apply_control_frames,
                    lobby_enqueue_allocation,
                    lobby_deliver_grants,
                )
                    .chain(),
            )
            .add_observer(lobby_client_removed)
            .add_observer(lobby_netcode_authenticated);
    }
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
    mut state: ResMut<LobbyState>,
) {
    if let Ok(client) = clients.get(trigger.entity) {
        state.remove_client(client.client_id.get());
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
}

fn lobby_cleanup_disconnected(
    mut state: ResMut<LobbyState>,
    query: Query<&LobbyClient, With<Disconnected>>,
) {
    for client in &query {
        state.remove_client(client.client_id.get());
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
) {
    let Some(manifest) = role.lobby_manifest().cloned() else {
        return;
    };
    let Ok(build) = default_build_identity() else {
        return;
    };
    commands.insert_resource(LobbyState::new(manifest, config.game_mode, build));
}

#[allow(
    clippy::type_complexity,
    reason = "the query is the one bounded lobby authentication transaction"
)]
fn lobby_receive_hellos(
    mut commands: Commands,
    mut state: ResMut<LobbyState>,
    mut outbox: ResMut<LobbyControlOutbox>,
    mut receivers: Query<(
        Entity,
        &RemoteId,
        &mut MessageReceiver<ClientHello>,
        &mut MessageSender<JoinOutcome>,
        Option<&RoutedPeer>,
        Option<&LobbyClient>,
        Has<Connected>,
        Has<Disconnected>,
    )>,
) {
    for (
        entity,
        remote_id,
        mut receiver,
        mut sender,
        routed_peer,
        lobby_client,
        connected,
        disconnected,
    ) in &mut receivers
    {
        if !connected || disconnected {
            continue;
        }
        let messages: Vec<_> = receiver.receive().collect();
        for hello in messages {
            let netcode_client_id =
                authenticated_netcode_id(remote_id).and_then(NetcodeClientId::new);
            let new_session = netcode_client_id
                .is_some_and(|client_id| state.session_for_client(client_id.get()).is_none());
            let outcome = match (authenticated_netcode_id(remote_id), routed_peer) {
                (Some(client_id), Some(peer)) => state
                    .accept_client(client_id, peer.route_id, peer.peer_id, &hello)
                    .map_or_else(
                        |error| JoinOutcome::Rejected {
                            reason: error.rejection(),
                        },
                        |session| {
                            if lobby_client.is_none() && new_session {
                                commands.entity(entity).insert(LobbyClient {
                                    client_id: session.netcode_client_id,
                                    lobby_session_id: session.lobby_session_id,
                                });
                                let _ = outbox.push_authenticated(LobbyAuthenticatedBody {
                                    route_id: session.route_id,
                                    peer_id: session.peer_id,
                                    lobby_session_id: session.lobby_session_id,
                                    netcode_client_id: session.netcode_client_id,
                                });
                            }
                            JoinOutcome::Accepted {
                                player_id: crate::protocol::PlayerId(session.player_id.get()),
                                network_entity_id: session.network_entity_id,
                            }
                        },
                    ),
                (Some(_), None) => JoinOutcome::Rejected {
                    reason: LobbySessionError::NotRouted.rejection(),
                },
                (None, _) => JoinOutcome::Rejected {
                    reason: LobbySessionError::InvalidClientId.rejection(),
                },
            };
            sender.send::<SessionChannel>(outcome);
        }
    }
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

fn lobby_deliver_grants(
    mut state: ResMut<LobbyState>,
    mut clients: Query<(&LobbyClient, &mut MessageSender<MatchRouteGrantV1>)>,
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

    fn manifest() -> LobbyManifestV1 {
        LobbyManifestV1 {
            common: ManifestCommon {
                manifest_version: 1,
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
                control_version: brawler_routing::CONTROL_VERSION_V1,
                flags: 0,
            },
            mode: brawler_routing::GameMode::Wipeout,
            default_route_id: RouteId::new(9).unwrap(),
            max_authenticated_sessions: 32,
            outstanding_allocations: 2,
            active_matches: 2,
            heartbeat_ms: 100,
            nonce: 11,
            digest: [0; 32],
        }
        .with_digest()
        .unwrap()
    }

    fn build() -> LobbyBuildIdentity {
        LobbyBuildIdentity {
            source_build_preset: Some(1),
            recipe_fingerprint: 123,
            build_revision: 4,
        }
    }

    fn hello() -> ClientHello {
        ClientHello {
            protocol_version: SUPPORTED_PROTOCOL_VERSION,
            build_version: VERSION.to_string(),
            registry_fingerprint: 7,
            content_fingerprint: GameplayContentFingerprint(8),
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
