//! Stable protocol registration shared by the client and dedicated server.

use avian2d::prelude::{Position, Rotation};
use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use bevy::reflect::Reflect;
use lightyear::input::config::InputConfig;
use lightyear::prelude::{
    AppChannelExt, AppComponentExt, AppInterpolationExt, AppMessageExt, ChannelMode,
    ChannelRegistry, ChannelSettings, ComponentRegistry, InterpolationFns,
    InterpolationRegistrationExt, MessageRegistry, NetworkDirection, ReliableSettings,
    input::native::InputPlugin,
};
use serde::{Deserialize, Serialize};

// These identifiers remain owned by the engine-independent routing package. Keep this import
// private so the protocol module does not accidentally become a second public ID namespace; the
// routed message types below remain directly constructible by using the routing package's IDs.
use brawler_routing::{AllocationId, MatchId, PeerId, RequestId, RouteId};

use crate::builds::{AbilityState, PassiveRuntimeState, ResolvedMatchLoadout};
use crate::combat::{
    ActiveEffects, AttackDelivery, AuthoritativePose, AuthoritativeTick, CombatCue,
    CombatEvidenceCheckpoint, ConeSpray, ConeSprayState, CurrentHealth, Defeated,
    FighterDefinitionId, KnockbackFeedback, LobbedFlight, PersistentSplash, PersistentSplashState,
    Projectile, ProjectileBody, ProjectileDeadline, ProjectileSource, ReplicatedAttackSource,
    StickyBlobState, StraightFlight, TeamId, WeaponDefinitionId, WeaponState,
};
use crate::content::GameplayContentFingerprint;
use crate::map::{
    EffectTileOccupancy, MapDynamicState, MapInstanceId, MapRoot, ResolvedMapIdentity,
    ResolvedMapSnapshot, SpawnAssignment,
};
use crate::matchplay::{
    FighterDisplayName, HotZoneState, MatchClock, MatchParticipant, MatchRoot as MatchRootMarker,
    MatchState, PublicParticipantState, RespawnState, SpawnProtection, WipeoutState,
};
use crate::timing::SIMULATION_TICK;

/// Netcode protocol ID. Bump this for incompatible wire-level changes.
pub const NETWORK_PROTOCOL_ID: u64 = 0x4252_4157_4c45_5241;

/// Brawler-level compatibility version exchanged after Netcode connects.
pub const SUPPORTED_PROTOCOL_VERSION: u16 = 38;

/// Development-only key for local loopback sessions. This is not authentication.
pub const DEVELOPMENT_PRIVATE_KEY: [u8; 32] = [0x42; 32];

/// Ordered reliable channel for the compatibility handshake and join outcome.
pub struct SessionChannel;

/// Ordered reliable saved-profile mutations and authoritative whole-snapshot outcomes.
pub struct ProfileChannel;

/// Sequenced-unreliable server-to-client stream for replaceable complete queue snapshots.
pub struct QueueSnapshotChannel;

/// Ordered reliable server-to-client stream for presentation-only combat facts.
pub struct CombatChannel;

/// Ordered reliable map-mutation and bounded recovery traffic.
pub struct MapDynamicChannel;

/// Hash of the Lightyear message, component, and channel registries for the local app.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolFingerprint(pub u64);

/// Stable server-assigned player identity.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerId(pub u64);

/// Stable server-assigned network entity identity.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct NetworkEntityId(pub u64);

/// Marker for an authoritative fighter replicated to every connected client.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Fighter;

/// Compatibility alias retained for the Milestone 02 roster tests and callers.
pub type PlaceholderPlayer = Fighter;

/// Small replicated state retaining the stable spawn slot for the greybox fighter.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaceholderState {
    pub spawn_slot: u64,
}

/// Signed 16-bit normalized axis value used on the wire.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
pub struct QuantizedAxis2 {
    pub x: i16,
    pub y: i16,
}

impl QuantizedAxis2 {
    pub const MAX: i16 = i16::MAX;

    #[must_use]
    pub fn from_vec2(axis: Vec2) -> Self {
        let axis = if axis.is_finite() {
            axis.clamp_length_max(1.0)
        } else {
            Vec2::ZERO
        };
        Self {
            x: quantize_axis_component(axis.x),
            y: quantize_axis_component(axis.y),
        }
    }

    #[must_use]
    pub fn to_vec2(self) -> Vec2 {
        Vec2::new(
            f32::from(self.x) / f32::from(Self::MAX),
            f32::from(self.y) / f32::from(Self::MAX),
        )
    }
}

#[allow(clippy::cast_possible_truncation)]
fn quantize_axis_component(value: f32) -> i16 {
    (value.clamp(-1.0, 1.0) * f32::from(QuantizedAxis2::MAX)).round() as i16
}

/// Unsigned whole-world-unit aim distance carried as player intent.
///
/// The authoritative combat rule clamps this value to the selected delivery's legal range.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
pub struct QuantizedAimDistance(pub u16);

impl QuantizedAimDistance {
    #[must_use]
    #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
    pub fn from_world_units(distance: f32) -> Self {
        let distance = if distance.is_finite() { distance } else { 0.0 };
        Self(distance.clamp(0.0, f32::from(u16::MAX)).round() as u16)
    }

    #[must_use]
    pub fn to_world_units(self) -> f32 {
        f32::from(self.0)
    }
}

/// The one fixed-tick intent payload accepted by the authoritative server.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Reflect)]
pub struct FighterInput {
    pub move_axis: QuantizedAxis2,
    pub aim_update: Option<QuantizedAxis2>,
    pub aim_distance: Option<QuantizedAimDistance>,
    pub gameplay_buttons: u8,
}

impl FighterInput {
    pub const PRIMARY_FIRE: u8 = 1 << 0;
    pub const ACTIVE_ITEM: u8 = 1 << 1;
    pub const ULTIMATE: u8 = 1 << 2;
    pub const INTERACT: u8 = 1 << 3;
    pub const ALLOWED_BUTTONS: u8 =
        Self::PRIMARY_FIRE | Self::ACTIVE_ITEM | Self::ULTIMATE | Self::INTERACT;

    #[must_use]
    pub fn from_axes(move_axis: Vec2, aim_update: Option<Vec2>, gameplay_buttons: u8) -> Self {
        Self::from_axes_with_aim_distance(move_axis, aim_update, None, gameplay_buttons)
    }

    #[must_use]
    pub fn from_axes_with_aim_distance(
        move_axis: Vec2,
        aim_update: Option<Vec2>,
        aim_distance: Option<f32>,
        gameplay_buttons: u8,
    ) -> Self {
        Self {
            move_axis: QuantizedAxis2::from_vec2(move_axis),
            aim_update: aim_update.map(QuantizedAxis2::from_vec2),
            aim_distance: aim_distance.map(QuantizedAimDistance::from_world_units),
            gameplay_buttons: gameplay_buttons & Self::ALLOWED_BUTTONS,
        }
    }

    #[must_use]
    pub fn is_valid(self) -> bool {
        self.gameplay_buttons & !Self::ALLOWED_BUTTONS == 0
            && self.move_axis.to_vec2().is_finite()
            && self
                .aim_update
                .is_none_or(|axis| axis.to_vec2().is_finite())
    }
}

impl MapEntities for FighterInput {
    fn map_entities<M: EntityMapper>(&mut self, _entity_mapper: &mut M) {}
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MatchHello {
    pub protocol_version: u16,
    pub build_version: String,
    pub registry_fingerprint: u64,
    pub content_fingerprint: GameplayContentFingerprint,
}

/// Public stable identity announced by a lobby before the client selects its local account key.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct LobbyServerIdentity {
    pub logical_server_id: u128,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct LobbyHello {
    pub protocol_version: u16,
    pub build_version: String,
    pub registry_fingerprint: u64,
    pub content_fingerprint: GameplayContentFingerprint,
    pub account_id: crate::profiles::AccountId,
    #[serde(deserialize_with = "crate::lobby::deserialize_player_name")]
    pub proposed_display_name: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum LobbyJoinOutcome {
    Accepted {
        logical_server_id: u128,
        player_id: PlayerId,
        #[serde(deserialize_with = "crate::lobby::deserialize_accepted_player_name")]
        accepted_display_name: String,
        #[serde(deserialize_with = "crate::lobby::deserialize_presentation_name")]
        server_name: String,
        catalog_revision: crate::lobby::CatalogRevision,
        #[serde(deserialize_with = "crate::lobby::deserialize_game_types")]
        game_types: Vec<crate::lobby::AdvertisedGameType>,
        brawler_catalog: Box<crate::profiles::AdvertisedBrawlerCatalog>,
        profile: Box<crate::profiles::ProfileSnapshot>,
    },
    Rejected {
        reason: LobbyJoinRejection,
    },
}

pub const MAX_LOBBY_WELCOME_BYTES: usize = 64 * 1024;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum LobbyJoinRejection {
    ProtocolVersionMismatch,
    BuildVersionMismatch,
    RegistryMismatch,
    ContentMismatch,
    ServerFull,
    InvalidName,
    IdentifierExhausted,
    InvalidAccount,
    AccountInUse,
    StorageUnavailable,
}

/// A route selector capability delivered over the authenticated lobby session.
///
/// Capabilities are bearer secrets. Keep the bytes private and redact both Debug and Display so a
/// grant can safely be included in bounded diagnostics or a test failure without leaking the
/// selector that routes public traffic to a match worker.
#[derive(Clone, Copy, PartialEq, Eq, Hash)]
pub struct RouteCapability([u8; 32]);

impl RouteCapability {
    pub const BYTES: usize = 32;

    #[must_use]
    pub const fn from_bytes(bytes: [u8; Self::BYTES]) -> Option<Self> {
        let mut index = 0;
        while index < bytes.len() {
            if bytes[index] != 0 {
                return Some(Self(bytes));
            }
            index += 1;
        }
        None
    }

    #[must_use]
    pub(crate) const fn bytes(self) -> [u8; Self::BYTES] {
        self.0
    }

    /// Convert the authenticated protocol capability into the engine-independent route selector.
    ///
    /// The bytes remain private and the routing type redacts them in diagnostics; this bridge is
    /// deliberately crate-visible so only the client transport can install a received grant.
    #[cfg(feature = "client")]
    pub(crate) fn to_routing_capability(self) -> brawler_routing::Capability {
        brawler_routing::Capability::from_bytes(self.0)
            .expect("validated route capability must be non-zero")
    }
}

impl core::fmt::Debug for RouteCapability {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("RouteCapability([REDACTED])")
    }
}

impl core::fmt::Display for RouteCapability {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter.write_str("[REDACTED]")
    }
}

/// Authenticated lobby-to-client grant for a fresh routed match session.
///
/// The grant is accepted at most once by the client for its current lobby request. It carries no
/// Netcode credential; the match worker still authenticates the fresh session and checks the
/// immutable participant manifest.
#[derive(Clone, Copy, PartialEq, Eq)]
pub struct MatchRouteGrant {
    pub request_id: RequestId,
    pub allocation_id: AllocationId,
    pub match_id: MatchId,
    pub route_id: RouteId,
    pub peer_id: PeerId,
    pub game_mode: crate::config::GameMode,
    pub capability: RouteCapability,
    pub activation_expiry_unix_ms: u64,
    pub route_expiry_unix_ms: u64,
}

impl MatchRouteGrant {
    fn validate(&self) -> Result<(), &'static str> {
        if self.activation_expiry_unix_ms > self.route_expiry_unix_ms {
            return Err("activation expiry must not exceed route expiry");
        }
        Ok(())
    }
}

impl core::fmt::Debug for MatchRouteGrant {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MatchRouteGrant")
            .field("request_id", &self.request_id)
            .field("allocation_id", &self.allocation_id)
            .field("match_id", &self.match_id)
            .field("route_id", &self.route_id)
            .field("peer_id", &self.peer_id)
            .field("game_mode", &self.game_mode)
            .field("capability", &self.capability)
            .field("activation_expiry_unix_ms", &self.activation_expiry_unix_ms)
            .field("route_expiry_unix_ms", &self.route_expiry_unix_ms)
            .finish()
    }
}

#[derive(Serialize, Deserialize)]
struct MatchRouteGrantWire {
    request_id: u64,
    allocation_id: u128,
    match_id: u128,
    route_id: u128,
    peer_id: u128,
    game_mode: u16,
    capability: [u8; RouteCapability::BYTES],
    activation_expiry_unix_ms: u64,
    route_expiry_unix_ms: u64,
}

impl Serialize for MatchRouteGrant {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.validate().map_err(serde::ser::Error::custom)?;
        MatchRouteGrantWire {
            request_id: self.request_id.get(),
            allocation_id: self.allocation_id.get(),
            match_id: self.match_id.get(),
            route_id: self.route_id.get(),
            peer_id: self.peer_id.get(),
            game_mode: match self.game_mode {
                crate::config::GameMode::Wipeout => 1,
                crate::config::GameMode::HotZone => 2,
                crate::config::GameMode::Heist => 3,
            },
            capability: self.capability.bytes(),
            activation_expiry_unix_ms: self.activation_expiry_unix_ms,
            route_expiry_unix_ms: self.route_expiry_unix_ms,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MatchRouteGrant {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = MatchRouteGrantWire::deserialize(deserializer)?;
        let request_id = RequestId::new(wire.request_id)
            .ok_or_else(|| serde::de::Error::custom("request ID must be nonzero"))?;
        let allocation_id = AllocationId::new(wire.allocation_id)
            .ok_or_else(|| serde::de::Error::custom("allocation ID must be nonzero"))?;
        let match_id = MatchId::new(wire.match_id)
            .ok_or_else(|| serde::de::Error::custom("match ID must be nonzero"))?;
        let route_id = RouteId::new(wire.route_id)
            .ok_or_else(|| serde::de::Error::custom("route ID must be nonzero"))?;
        let peer_id = PeerId::new(wire.peer_id)
            .ok_or_else(|| serde::de::Error::custom("peer ID must be nonzero"))?;
        let game_mode = match wire.game_mode {
            1 => crate::config::GameMode::Wipeout,
            2 => crate::config::GameMode::HotZone,
            3 => crate::config::GameMode::Heist,
            _ => return Err(serde::de::Error::custom("unsupported game mode")),
        };
        let capability = RouteCapability::from_bytes(wire.capability)
            .ok_or_else(|| serde::de::Error::custom("route capability must be nonzero"))?;
        let grant = Self {
            request_id,
            allocation_id,
            match_id,
            route_id,
            peer_id,
            game_mode,
            capability,
            activation_expiry_unix_ms: wire.activation_expiry_unix_ms,
            route_expiry_unix_ms: wire.route_expiry_unix_ms,
        };
        grant.validate().map_err(serde::de::Error::custom)?;
        Ok(grant)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchCommand {
    SetReady(bool),
    ReadyForRestart,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchCommandRequest {
    pub request_id: u64,
    pub match_id: crate::matchplay::MatchId,
    pub command: MatchCommand,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchCommandDecision {
    Accepted,
    Stale,
    WrongMatch,
    WrongPhase,
    NotParticipant,
    Locked,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchCommandOutcome {
    pub request_id: u64,
    pub match_id: crate::matchplay::MatchId,
    pub decision: MatchCommandDecision,
}

/// Ordered product-routed loading intent. Correlation fields bind every request to the immutable
/// allocation manifest rather than to a process-local entity.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchLoadingClientMessage {
    pub request_id: u64,
    pub allocation_id: u128,
    pub match_id: u128,
    pub action: MatchLoadingClientAction,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchLoadingClientAction {
    Ready,
    CancelMatchStart,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchLoadingServerMessage {
    pub request_id: u64,
    pub allocation_id: u128,
    pub match_id: u128,
    pub outcome: MatchLoadingServerOutcome,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MatchLoadingServerOutcome {
    CancellationAccepted,
    CancellationTooLate,
    TerminalFailure,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MatchLoadingStatus {
    pub generation: u32,
    pub revision: u32,
    pub request_id: u64,
    pub allocation_id: u128,
    pub match_id: u128,
    pub phase: crate::lobby::MatchLoadingPhase,
    pub expected: u8,
    pub connected: u8,
    pub checked_in: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum MatchJoinOutcome {
    Accepted {
        player_id: PlayerId,
        network_entity_id: NetworkEntityId,
    },
    Rejected {
        reason: MatchJoinRejection,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum MatchJoinRejection {
    ProtocolVersionMismatch,
    BuildVersionMismatch,
    RegistryMismatch,
    ContentMismatch,
    HandshakeTimeout,
    ServerFull,
    MatchFull,
    MatchInProgress,
    IdentifierExhausted,
}

/// Registers every message, channel, and replicated component used by the sandbox.
pub struct ProtocolPlugin;

/// Interpolate exactly the pose components declared by Brawler's wire protocol.
///
/// Lightyear's Avian integration also installs a four-component Hermite rule for pose and
/// velocity. Brawler intentionally keeps velocity server-local, so that rule can never obtain a
/// complete history sample. This pose-only rule has higher priority and prevents the incomplete
/// Avian bundle from suppressing interpolation of the replicated components.
fn interpolate_network_pose(
    start: (Position, Rotation),
    end: (Position, Rotation),
    t: f32,
) -> (Position, Rotation) {
    (
        Position(start.0.0.lerp(end.0.0, t)),
        start.1.slerp(end.1, t),
    )
}

fn register_map_dynamic_protocol(app: &mut App) {
    app.register_message::<crate::map::MapMutationEvent>()
        .add_direction(NetworkDirection::ServerToClient);
    app.register_message::<crate::map::MapDynamicResetEvent>()
        .add_direction(NetworkDirection::ServerToClient);
    app.register_message::<crate::map::MapDynamicRecoveryRequest>()
        .add_direction(NetworkDirection::ClientToServer);
    app.register_message::<crate::map::MapDynamicRecoverySnapshot>()
        .add_direction(NetworkDirection::ServerToClient);
    app.add_channel::<MapDynamicChannel>(ChannelSettings {
        mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
        ..default()
    })
    .add_direction(NetworkDirection::Bidirectional);
}

fn register_queue_protocol(app: &mut App) {
    app.register_message::<crate::lobby::QueueClientMessage>()
        .add_direction(NetworkDirection::ClientToServer);
    app.register_message::<crate::lobby::QueueCommandOutcome>()
        .add_direction(NetworkDirection::ServerToClient);
    app.register_message::<crate::lobby::QueuePoolSnapshot>()
        .add_direction(NetworkDirection::ServerToClient);
    app.register_message::<crate::lobby::MatchmakingClientMessage>()
        .add_direction(NetworkDirection::ClientToServer);
    app.register_message::<crate::lobby::MatchmakingServerMessage>()
        .add_direction(NetworkDirection::ServerToClient);
    app.register_message::<crate::lobby::PracticeStartRequest>()
        .add_direction(NetworkDirection::ClientToServer);
    app.add_channel::<QueueSnapshotChannel>(ChannelSettings {
        mode: ChannelMode::SequencedUnreliable,
        retry_unsent_messages: false,
        ..default()
    })
    .add_direction(NetworkDirection::ServerToClient);
}

fn register_replicated_components(app: &mut App) {
    app.component::<Fighter>().replicate_once();
    app.component::<crate::concealment::ConcealmentPresentationState>()
        .replicate();
    app.component::<crate::concealment::ConcealmentFieldState>()
        .replicate_once();
    app.component::<crate::concealment::ObjectiveCarrier>()
        .replicate();
    app.component::<MatchRootMarker>().replicate_once();
    app.component::<MatchState>().replicate();
    app.component::<MatchClock>().replicate();
    app.component::<WipeoutState>().replicate();
    app.component::<HotZoneState>().replicate();
    app.component::<crate::matchplay::HeistState>().replicate();
    app.component::<crate::matchplay::HeistSafe>().replicate();
    app.component::<MatchParticipant>().replicate();
    app.component::<FighterDisplayName>().replicate_once();
    app.component::<PublicParticipantState>().replicate();
    app.component::<RespawnState>().replicate();
    app.component::<SpawnProtection>().replicate();
    app.component::<MapRoot>().replicate_once();
    app.component::<MapInstanceId>().replicate_once();
    app.component::<ResolvedMapIdentity>().replicate_once();
    app.component::<ResolvedMapSnapshot>().replicate_once();
    app.component::<MapDynamicState>().replicate_once();
    app.component::<EffectTileOccupancy>().replicate();
    app.component::<crate::map::DamageableWorldObject>()
        .replicate_once();
    app.component::<crate::map::DamageableTargetIdentity>()
        .replicate();
    app.component::<crate::map::DamageableTargetClass>()
        .replicate_once();
    app.component::<crate::map::DamageableMaximumHealth>()
        .replicate_once();
    app.component::<crate::map::DamageableObjectProfile>()
        .replicate_once();
    app.component::<crate::map::DamageableObjectAsset>()
        .replicate_once();
    app.component::<crate::map::DamageableLifeState>()
        .replicate();
    app.component::<crate::map::RestorationPickup>()
        .replicate_once();
    app.component::<crate::map::RestorationPickupIdentity>()
        .replicate_once();
    app.component::<crate::map::RestorationPickupDefinitionId>()
        .replicate_once();
    app.component::<crate::map::PickupAvailableAtTick>()
        .replicate_once();
    app.component::<crate::map::PickupExpiresAtTick>()
        .replicate_once();
    app.component::<SpawnAssignment>().replicate();
    app.component::<PlayerId>().replicate_once();
    app.component::<NetworkEntityId>().replicate_once();
    app.component::<PlaceholderState>().replicate();
    app.component::<FighterDefinitionId>().replicate_once();
    app.component::<WeaponDefinitionId>().replicate_once();
    app.component::<crate::builds::SelectedBuild>().replicate();
    app.component::<ResolvedMatchLoadout>().replicate();
    app.component::<AbilityState>().replicate();
    app.component::<PassiveRuntimeState>().replicate();
    app.component::<crate::abilities::Sentry>().replicate_once();
    app.component::<crate::abilities::SentryIdentity>()
        .replicate_once();
    app.component::<crate::abilities::SentryDeadline>()
        .replicate_once();
    app.component::<ActiveEffects>().replicate();
    app.component::<crate::combat::ElementalFieldState>()
        .replicate();
    app.component::<KnockbackFeedback>().replicate();
    app.component::<AttackDelivery>().replicate_once();
    app.component::<ConeSpray>().replicate_once();
    app.component::<ConeSprayState>().replicate_once();
    app.component::<PersistentSplash>().replicate_once();
    app.component::<PersistentSplashState>().replicate();
    app.component::<LobbedFlight>().replicate_once();
    app.component::<ProjectileDeadline>().replicate_once();
    app.component::<StickyBlobState>().replicate();
    app.component::<StraightFlight>().replicate_once();
    app.component::<ProjectileBody>().replicate_once();
    app.component::<TeamId>().replicate();
    app.component::<CurrentHealth>().replicate();
    app.component::<WeaponState>().replicate();
    app.component::<AuthoritativeTick>().replicate();
    app.component::<AuthoritativePose>().replicate();
    app.component::<Defeated>().replicate();
    app.component::<Projectile>().replicate_once();
    app.component::<ProjectileSource>().replicate_once();
    app.component::<ReplicatedAttackSource>().replicate_once();
    app.component::<Position>()
        .replicate()
        .add_linear_interpolation();
    app.component::<Rotation>()
        .replicate()
        .add_linear_interpolation();
    app.interpolate_bundle_with_priority::<(Position, Rotation)>(
        5,
        InterpolationFns::interpolate(interpolate_network_pose),
    );
}

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::combat::WeaponCatalogResource>()
            .init_resource::<crate::combat::CombatConditionRulesResource>()
            .add_plugins(crate::builds::BuildContentPlugin)
            .add_plugins(crate::weapon_parts::WeaponPartContentPlugin)
            .add_plugins(crate::map::MapContentPlugin)
            .add_systems(Startup, initialize_content_fingerprint);
        app.register_message::<MatchHello>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<MatchJoinOutcome>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<LobbyHello>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<LobbyServerIdentity>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<LobbyJoinOutcome>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<crate::profiles::ProfileCommand>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<crate::profiles::ProfileOutcome>()
            .add_direction(NetworkDirection::ServerToClient);
        register_queue_protocol(app);
        app.register_message::<MatchRouteGrant>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<MatchCommandRequest>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<MatchCommandOutcome>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<MatchLoadingClientMessage>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<MatchLoadingServerMessage>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<MatchLoadingStatus>()
            .add_direction(NetworkDirection::ServerToClient);
        app.add_plugins(InputPlugin::<FighterInput> {
            config: InputConfig {
                send_interval: SIMULATION_TICK,
                packet_redundancy: 5,
                rebroadcast_inputs: false,
                ..default()
            },
        });
        app.add_channel::<SessionChannel>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::Bidirectional);
        app.add_channel::<ProfileChannel>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::Bidirectional);
        app.register_message::<CombatCue>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<crate::map::WorldObjectCue>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<crate::map::PickupCue>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<crate::matchplay::HeistObjectiveCue>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<CombatEvidenceCheckpoint>()
            .add_direction(NetworkDirection::ServerToClient);
        app.add_channel::<CombatChannel>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::ServerToClient);

        register_map_dynamic_protocol(app);

        register_replicated_components(app);
        app.add_systems(Startup, initialize_protocol_fingerprint);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(crate) fn initialize_content_fingerprint(
    weapons: Res<crate::combat::WeaponCatalogResource>,
    maps: Res<crate::map::MapCatalogResource>,
    builds: Res<crate::builds::BuildCatalogResource>,
    mut commands: Commands,
) {
    let fingerprint = crate::content::gameplay_content_fingerprint(&weapons.0, &maps.0, &builds.0)
        .expect("embedded gameplay catalogs must fingerprint");
    commands.insert_resource(fingerprint);
}

/// Compute the same application-owned fingerprint on both peers after all protocol plugins run.
pub fn protocol_fingerprint(world: &mut World) -> u64 {
    let messages = world
        .get_resource_mut::<MessageRegistry>()
        .map(|mut registry| registry.finish())
        .unwrap_or_default();
    let components = world
        .get_resource_mut::<ComponentRegistry>()
        .map(|mut registry| registry.finish())
        .unwrap_or_default();
    let channels = world
        .get_resource_mut::<ChannelRegistry>()
        .map(|mut registry| registry.finish())
        .unwrap_or_default();
    messages ^ components.rotate_left(21) ^ channels.rotate_left(42)
}

fn initialize_protocol_fingerprint(world: &mut World) {
    let fingerprint = protocol_fingerprint(world);
    world.insert_resource(ProtocolFingerprint(fingerprint));
}

#[cfg(test)]
mod tests {
    use super::*;
    #[cfg(feature = "client")]
    use crate::movement::AvianNetworkPlugin;
    use lightyear::prelude::{AppMessageExt, ComponentRegistry, MessageRegistry};
    #[cfg(feature = "client")]
    use lightyear::prelude::{
        ConfirmedHistory, Interpolated, InterpolationTimeline, NetworkTimeline, Tick,
    };

    #[test]
    fn protocol_plugin_registers_messages_channel_and_components() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin));
        #[cfg(feature = "client")]
        app.add_plugins(lightyear::prelude::client::ClientPlugins {
            tick_duration: SIMULATION_TICK,
        });
        #[cfg(all(not(feature = "client"), feature = "server"))]
        app.add_plugins(lightyear::prelude::server::ServerPlugins {
            tick_duration: SIMULATION_TICK,
        });
        app.add_plugins(ProtocolPlugin);

        assert!(app.is_message_registered::<MatchHello>());
        assert!(app.is_message_registered::<MatchJoinOutcome>());
        assert!(app.is_message_registered::<LobbyHello>());
        assert!(app.is_message_registered::<LobbyServerIdentity>());
        assert!(app.is_message_registered::<LobbyJoinOutcome>());
        assert!(app.is_message_registered::<crate::lobby::QueueClientMessage>());
        assert!(app.is_message_registered::<crate::lobby::QueueCommandOutcome>());
        assert!(app.is_message_registered::<crate::lobby::QueuePoolSnapshot>());
        assert!(app.is_message_registered::<crate::lobby::MatchmakingClientMessage>());
        assert!(app.is_message_registered::<crate::lobby::MatchmakingServerMessage>());
        assert!(app.is_message_registered::<crate::lobby::PracticeStartRequest>());
        assert!(app.is_message_registered::<MatchRouteGrant>());
        assert!(app.is_message_registered::<MatchCommandRequest>());
        assert!(app.is_message_registered::<MatchCommandOutcome>());
        assert!(app.is_message_registered::<MatchLoadingClientMessage>());
        assert!(app.is_message_registered::<MatchLoadingServerMessage>());
        assert!(app.is_message_registered::<MatchLoadingStatus>());
        assert!(app.is_message_registered::<CombatCue>());
        assert!(app.is_message_registered::<crate::map::MapMutationEvent>());
        assert!(app.is_message_registered::<crate::map::MapDynamicResetEvent>());
        assert!(app.is_message_registered::<crate::map::MapDynamicRecoveryRequest>());
        assert!(app.is_message_registered::<crate::map::MapDynamicRecoverySnapshot>());
        assert!(app.world().contains_resource::<MessageRegistry>());
        assert!(app.world().contains_resource::<ChannelRegistry>());
        let channels = app.world().resource::<ChannelRegistry>();
        assert!((0..32).any(|id| {
            channels.get_name_from_net_id(id) == core::any::type_name::<SessionChannel>()
        }));
        assert!((0..32).any(|id| {
            channels.get_name_from_net_id(id) == core::any::type_name::<QueueSnapshotChannel>()
        }));
        assert!((0..32).any(|id| {
            channels.get_name_from_net_id(id) == core::any::type_name::<MapDynamicChannel>()
        }));
        let components = app.world().resource::<ComponentRegistry>();
        assert!(components.is_registered::<Fighter>());
        assert!(components.is_registered::<crate::concealment::ConcealmentFieldState>());
        assert!(components.is_registered::<crate::concealment::ObjectiveCarrier>());
        assert!(components.is_registered::<MatchRootMarker>());
        assert!(components.is_registered::<MatchState>());
        assert!(components.is_registered::<MatchClock>());
        assert!(components.is_registered::<WipeoutState>());
        assert!(components.is_registered::<HotZoneState>());
        assert!(components.is_registered::<MatchParticipant>());
        assert!(components.is_registered::<FighterDisplayName>());
        assert!(components.is_registered::<RespawnState>());
        assert!(components.is_registered::<SpawnProtection>());
        assert!(components.is_registered::<PlayerId>());
        assert!(components.is_registered::<NetworkEntityId>());
        assert!(components.is_registered::<PlaceholderState>());
        assert!(components.is_registered::<FighterDefinitionId>());
        assert!(components.is_registered::<WeaponDefinitionId>());
        assert!(components.is_registered::<crate::builds::SelectedBuild>());
        assert!(components.is_registered::<ResolvedMatchLoadout>());
        assert!(!components.is_registered::<crate::combat::ResolvedWeapon>());
        assert!(components.is_registered::<TeamId>());
        assert!(components.is_registered::<CurrentHealth>());
        assert!(components.is_registered::<WeaponState>());
        assert!(components.is_registered::<AuthoritativeTick>());
        assert!(components.is_registered::<Defeated>());
        assert!(components.is_registered::<Projectile>());
        assert!(components.is_registered::<ProjectileSource>());
        assert!((0..32).any(|id| {
            channels.get_name_from_net_id(id) == core::any::type_name::<CombatChannel>()
        }));
    }

    #[cfg(feature = "client")]
    #[test]
    fn pose_only_rule_overrides_incomplete_avian_interpolation_bundle() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin));
        app.add_plugins(lightyear::prelude::client::ClientPlugins {
            tick_duration: SIMULATION_TICK,
        });
        app.add_plugins((ProtocolPlugin, AvianNetworkPlugin));
        app.finish();
        app.cleanup();

        let mut positions = ConfirmedHistory::default();
        positions.insert_present(Tick(10), Position::from_xy(0.0, 0.0));
        positions.insert_present(Tick(20), Position::from_xy(10.0, 0.0));
        let mut rotations = ConfirmedHistory::default();
        rotations.insert_present(Tick(10), Rotation::radians(0.0));
        rotations.insert_present(Tick(20), Rotation::radians(1.0));

        app.world_mut()
            .resource_mut::<InterpolationTimeline>()
            .apply_duration(SIMULATION_TICK * 15, SIMULATION_TICK);
        let entity = app
            .world_mut()
            .spawn((
                Interpolated,
                Position::from_xy(-1.0, -1.0),
                Rotation::radians(-1.0),
                positions,
                rotations,
            ))
            .id();

        app.update();

        let position = app
            .world()
            .get::<Position>(entity)
            .expect("interpolated position remains present");
        let rotation = app
            .world()
            .get::<Rotation>(entity)
            .expect("interpolated rotation remains present");
        assert!((position.0 - Vec2::new(5.0, 0.0)).length() < 0.001);
        assert!((rotation.as_radians() - 0.5).abs() < 0.001);
    }

    #[cfg(test)]
    use lightyear::prelude::MessageSender;

    #[test]
    fn session_messages_round_trip_with_serde() {
        let message = MatchJoinOutcome::Accepted {
            player_id: PlayerId(4),
            network_entity_id: NetworkEntityId(9),
        };
        let bytes = postcard::to_allocvec(&message).expect("message serializes");
        let decoded: MatchJoinOutcome = postcard::from_bytes(&bytes).expect("message deserializes");
        assert_eq!(decoded, message);
    }

    #[test]
    fn lobby_welcome_decoder_rejects_collection_and_normalization_overruns() {
        let game_type = crate::lobby::AdvertisedGameType {
            id: crate::lobby::GameTypeId::new("wipeout-2v2").unwrap(),
            configuration_revision: 1,
            display_name: "Wipeout 2v2".to_string(),
            mode_definition_id: crate::map::ModeDefinitionId(2),
            map_preset_ids: vec![crate::map::MapPresetId(1)],
            team_count: 2,
            players_per_team: 2,
            rules_summary: crate::lobby::AdvertisedRulesSummary::Wipeout {
                target_score: 10,
                active_limit_ticks: 10_800,
            },
        };
        let oversized = LobbyJoinOutcome::Accepted {
            logical_server_id: 1,
            player_id: PlayerId(1),
            accepted_display_name: "Player One".to_string(),
            server_name: "Local Brawler".to_string(),
            catalog_revision: crate::lobby::CatalogRevision([1; 32]),
            game_types: vec![game_type.clone(); crate::lobby::MAX_GAME_TYPES + 1],
            brawler_catalog: Box::new(
                crate::profiles::AdvertisedBrawlerCatalog::from_content(
                    &crate::builds::BuildCatalog::embedded().unwrap(),
                    &crate::combat::WeaponCatalog::embedded().unwrap(),
                )
                .unwrap(),
            ),
            profile: Box::new(crate::profiles::ProfileSnapshot::empty(
                crate::profiles::AccountId::new(1).unwrap(),
            )),
        };
        let bytes = postcard::to_allocvec(&oversized).unwrap();
        assert!(postcard::from_bytes::<LobbyJoinOutcome>(&bytes).is_err());

        let mut oversized_brawler_catalog =
            crate::profiles::AdvertisedBrawlerCatalog::from_content(
                &crate::builds::BuildCatalog::embedded().unwrap(),
                &crate::combat::WeaponCatalog::embedded().unwrap(),
            )
            .unwrap();
        oversized_brawler_catalog.fighter_profiles =
            vec![
                oversized_brawler_catalog.fighter_profiles[0].clone();
                crate::profiles::MAX_ADVERTISED_FIGHTER_PROFILES + 1
            ];
        let oversized_brawler = LobbyJoinOutcome::Accepted {
            logical_server_id: 1,
            player_id: PlayerId(1),
            accepted_display_name: "Player One".to_string(),
            server_name: "Local Brawler".to_string(),
            catalog_revision: crate::lobby::CatalogRevision([1; 32]),
            game_types: vec![game_type.clone()],
            brawler_catalog: Box::new(oversized_brawler_catalog),
            profile: Box::new(crate::profiles::ProfileSnapshot::empty(
                crate::profiles::AccountId::new(1).unwrap(),
            )),
        };
        let bytes = postcard::to_allocvec(&oversized_brawler).unwrap();
        assert!(postcard::from_bytes::<LobbyJoinOutcome>(&bytes).is_err());

        let invalid_name = LobbyJoinOutcome::Accepted {
            logical_server_id: 1,
            player_id: PlayerId(1),
            accepted_display_name: "Cafe\u{301}".to_string(),
            server_name: "Local Brawler".to_string(),
            catalog_revision: crate::lobby::CatalogRevision([1; 32]),
            game_types: vec![game_type],
            brawler_catalog: Box::new(
                crate::profiles::AdvertisedBrawlerCatalog::from_content(
                    &crate::builds::BuildCatalog::embedded().unwrap(),
                    &crate::combat::WeaponCatalog::embedded().unwrap(),
                )
                .unwrap(),
            ),
            profile: Box::new(crate::profiles::ProfileSnapshot::empty(
                crate::profiles::AccountId::new(1).unwrap(),
            )),
        };
        let bytes = postcard::to_allocvec(&invalid_name).unwrap();
        assert!(postcard::from_bytes::<LobbyJoinOutcome>(&bytes).is_err());
    }

    #[test]
    fn gameplay_match_messages_preserve_full_u128_match_id() {
        let message = MatchCommandRequest {
            request_id: 1,
            match_id: crate::matchplay::MatchId(u128::MAX),
            command: MatchCommand::ReadyForRestart,
        };
        let bytes = postcard::to_allocvec(&message).expect("match command serializes");
        let decoded: MatchCommandRequest =
            postcard::from_bytes(&bytes).expect("match command deserializes");
        assert_eq!(decoded, message);
    }

    #[test]
    fn match_route_grant_round_trips_and_rejects_zero_identity() {
        let message = MatchRouteGrant {
            request_id: RequestId::new(1).expect("nonzero request ID"),
            allocation_id: AllocationId::new(2).expect("nonzero allocation ID"),
            match_id: MatchId::new(3).expect("nonzero match ID"),
            route_id: RouteId::new(4).expect("nonzero route ID"),
            peer_id: PeerId::new(5).expect("nonzero peer ID"),
            game_mode: crate::config::GameMode::HotZone,
            capability: RouteCapability::from_bytes([0xa5; RouteCapability::BYTES])
                .expect("nonzero capability"),
            activation_expiry_unix_ms: 1_700_000_000_000,
            route_expiry_unix_ms: 1_700_000_600_000,
        };
        let bytes = postcard::to_allocvec(&message).expect("grant serializes");
        let decoded: MatchRouteGrant = postcard::from_bytes(&bytes).expect("grant deserializes");
        assert_eq!(decoded, message);

        let mut zero_request = bytes;
        zero_request[0] = 0;
        assert!(postcard::from_bytes::<MatchRouteGrant>(&zero_request).is_err());
    }

    #[test]
    fn match_route_grant_rejects_activation_after_route_expiry() {
        let message = MatchRouteGrant {
            request_id: RequestId::new(1).unwrap(),
            allocation_id: AllocationId::new(2).unwrap(),
            match_id: MatchId::new(3).unwrap(),
            route_id: RouteId::new(4).unwrap(),
            peer_id: PeerId::new(5).unwrap(),
            game_mode: crate::config::GameMode::Wipeout,
            capability: RouteCapability::from_bytes([0xa5; RouteCapability::BYTES]).unwrap(),
            activation_expiry_unix_ms: 3,
            route_expiry_unix_ms: 2,
        };
        assert!(postcard::to_allocvec(&message).is_err());

        let wire = MatchRouteGrantWire {
            request_id: 1,
            allocation_id: 2,
            match_id: 3,
            route_id: 4,
            peer_id: 5,
            game_mode: 1,
            capability: [0xa5; RouteCapability::BYTES],
            activation_expiry_unix_ms: 3,
            route_expiry_unix_ms: 2,
        };
        let bytes = postcard::to_allocvec(&wire).unwrap();
        assert!(postcard::from_bytes::<MatchRouteGrant>(&bytes).is_err());
    }

    #[test]
    fn match_route_grant_debug_redacts_capability_bytes() {
        let message = MatchRouteGrant {
            request_id: RequestId::new(11).expect("nonzero request ID"),
            allocation_id: AllocationId::new(12).expect("nonzero allocation ID"),
            match_id: MatchId::new(13).expect("nonzero match ID"),
            route_id: RouteId::new(14).expect("nonzero route ID"),
            peer_id: PeerId::new(15).expect("nonzero peer ID"),
            game_mode: crate::config::GameMode::Wipeout,
            capability: RouteCapability::from_bytes([0x7a; RouteCapability::BYTES])
                .expect("nonzero capability"),
            activation_expiry_unix_ms: 1,
            route_expiry_unix_ms: 2,
        };
        let debug = format!("{message:?}");
        assert!(debug.contains("REDACTED"));
        assert!(!debug.contains("7a"));
        assert_eq!(
            format!("{:?}", message.capability),
            "RouteCapability([REDACTED])"
        );
    }

    #[test]
    fn map_dynamic_messages_install_senders_only_in_their_exact_directions() {
        let mut client = App::new();
        client.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin));
        #[cfg(feature = "client")]
        client.add_plugins(lightyear::prelude::client::ClientPlugins {
            tick_duration: SIMULATION_TICK,
        });
        #[cfg(all(not(feature = "client"), feature = "server"))]
        client.add_plugins(lightyear::prelude::server::ServerPlugins {
            tick_duration: SIMULATION_TICK,
        });
        client.add_plugins(ProtocolPlugin);
        #[cfg(feature = "client")]
        {
            let client_link = client
                .world_mut()
                .spawn(lightyear::prelude::client::Client)
                .id();
            client.world_mut().flush();
            let world = client.world();
            assert!(
                world
                    .get::<MessageSender<crate::map::MapDynamicRecoveryRequest>>(client_link)
                    .is_some()
            );
            assert!(
                world
                    .get::<MessageSender<crate::map::MapMutationEvent>>(client_link)
                    .is_none()
            );
            assert!(
                world
                    .get::<MessageSender<crate::map::MapDynamicRecoverySnapshot>>(client_link)
                    .is_none()
            );
            assert!(
                world
                    .get::<MessageSender<MatchRouteGrant>>(client_link)
                    .is_none()
            );
            assert!(
                world
                    .get::<MessageSender<crate::map::MapDynamicResetEvent>>(client_link)
                    .is_none()
            );
        }

        let mut server = App::new();
        server.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin));
        #[cfg(feature = "client")]
        server.add_plugins(lightyear::prelude::client::ClientPlugins {
            tick_duration: SIMULATION_TICK,
        });
        #[cfg(all(not(feature = "client"), feature = "server"))]
        server.add_plugins(lightyear::prelude::server::ServerPlugins {
            tick_duration: SIMULATION_TICK,
        });
        server.add_plugins(ProtocolPlugin);
        #[cfg(feature = "server")]
        {
            let server_link = server
                .world_mut()
                .spawn(lightyear::prelude::server::ClientOf)
                .id();
            server.world_mut().flush();
            let world = server.world();
            assert!(
                world
                    .get::<MessageSender<crate::map::MapMutationEvent>>(server_link)
                    .is_some()
            );
            assert!(
                world
                    .get::<MessageSender<crate::map::MapDynamicRecoverySnapshot>>(server_link)
                    .is_some()
            );
            assert!(
                world
                    .get::<MessageSender<crate::map::MapDynamicResetEvent>>(server_link)
                    .is_some()
            );
            assert!(
                world
                    .get::<MessageSender<crate::map::MapDynamicRecoveryRequest>>(server_link)
                    .is_none()
            );
            assert!(
                world
                    .get::<MessageSender<MatchRouteGrant>>(server_link)
                    .is_some()
            );
        }
    }

    #[cfg(feature = "client")]
    #[test]
    fn queue_messages_install_only_the_client_command_sender() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::state::app::StatesPlugin,
            lightyear::prelude::client::ClientPlugins {
                tick_duration: SIMULATION_TICK,
            },
            ProtocolPlugin,
        ));
        let link = app
            .world_mut()
            .spawn(lightyear::prelude::client::Client)
            .id();
        app.world_mut().flush();
        let world = app.world();
        assert!(
            world
                .get::<MessageSender<crate::lobby::QueueClientMessage>>(link)
                .is_some()
        );
        assert!(
            world
                .get::<MessageSender<crate::lobby::QueueCommandOutcome>>(link)
                .is_none()
        );
        assert!(
            world
                .get::<MessageSender<crate::lobby::QueuePoolSnapshot>>(link)
                .is_none()
        );
    }

    #[cfg(feature = "server")]
    #[test]
    fn queue_messages_install_only_the_server_outcome_senders() {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            bevy::state::app::StatesPlugin,
            lightyear::prelude::server::ServerPlugins {
                tick_duration: SIMULATION_TICK,
            },
            ProtocolPlugin,
        ));
        let link = app
            .world_mut()
            .spawn(lightyear::prelude::server::ClientOf)
            .id();
        app.world_mut().flush();
        let world = app.world();
        assert!(
            world
                .get::<MessageSender<crate::lobby::QueueClientMessage>>(link)
                .is_none()
        );
        assert!(
            world
                .get::<MessageSender<crate::lobby::QueueCommandOutcome>>(link)
                .is_some()
        );
        assert!(
            world
                .get::<MessageSender<crate::lobby::QueuePoolSnapshot>>(link)
                .is_some()
        );
    }

    #[test]
    fn map_dynamic_wire_shapes_round_trip_with_serde() {
        use crate::map::{
            MapAssetId, MapDynamicGeneration, MapDynamicRecoverySnapshot, MapDynamicResetEvent,
            MapDynamicState, MapMutationEvent, MapPlacementId, MapPlacementOutcome,
            MapPlacementTransition,
        };
        let generation = MapDynamicGeneration {
            map_instance_id: MapInstanceId(3),
            generation: 7,
        };
        let transition = MapPlacementTransition {
            placement_id: MapPlacementId(11),
            outcome: MapPlacementOutcome::ReplacedWith(MapAssetId(4)),
        };
        let event = MapMutationEvent {
            generation,
            revision: 42,
            transitions: vec![transition],
        };
        let bytes = postcard::to_allocvec(&event).expect("event serializes");
        let decoded: MapMutationEvent = postcard::from_bytes(&bytes).expect("event deserializes");
        assert_eq!(decoded, event);

        let snapshot = MapDynamicRecoverySnapshot {
            state: MapDynamicState {
                map_instance_id: generation.map_instance_id,
                generation: generation.generation,
                revision: 42,
                terminal_states: vec![transition],
            },
        };
        let bytes = postcard::to_allocvec(&snapshot).expect("snapshot serializes");
        let decoded: MapDynamicRecoverySnapshot =
            postcard::from_bytes(&bytes).expect("snapshot deserializes");
        assert_eq!(decoded, snapshot);
        let reset = MapDynamicResetEvent {
            previous_generation: generation,
            next_generation: MapDynamicGeneration {
                generation: 8,
                ..generation
            },
        };
        let bytes = postcard::to_allocvec(&reset).expect("reset serializes");
        let decoded: MapDynamicResetEvent =
            postcard::from_bytes(&bytes).expect("reset deserializes");
        assert_eq!(decoded, reset);
    }

    #[test]
    fn fighter_input_round_trips_quantized_axes_and_rejects_unknown_buttons() {
        let input = FighterInput::from_axes(
            Vec2::new(0.5, 0.0),
            Some(Vec2::X),
            FighterInput::PRIMARY_FIRE,
        );
        let input = FighterInput {
            aim_distance: Some(QuantizedAimDistance::from_world_units(237.4)),
            ..input
        };
        let bytes = postcard::to_allocvec(&input).expect("input serializes");
        let decoded: FighterInput = postcard::from_bytes(&bytes).expect("input deserializes");
        assert_eq!(decoded, input);
        assert!((decoded.move_axis.to_vec2().x - 0.5).abs() < 1.0 / 32_000.0);
        assert_eq!(decoded.aim_distance, Some(QuantizedAimDistance(237)));
        assert!(
            !FighterInput {
                gameplay_buttons: 0x80,
                ..input
            }
            .is_valid()
        );
    }

    #[test]
    fn protocol_fingerprint_changes_when_a_registry_entry_changes() {
        let mut baseline_app = App::new();
        baseline_app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin));
        #[cfg(feature = "client")]
        baseline_app.add_plugins(lightyear::prelude::client::ClientPlugins {
            tick_duration: SIMULATION_TICK,
        });
        #[cfg(all(not(feature = "client"), feature = "server"))]
        baseline_app.add_plugins(lightyear::prelude::server::ServerPlugins {
            tick_duration: SIMULATION_TICK,
        });
        baseline_app.add_plugins(ProtocolPlugin);
        baseline_app.finish();
        baseline_app.cleanup();
        baseline_app.update();
        let baseline = baseline_app.world().resource::<ProtocolFingerprint>().0;

        let mut extra_app = App::new();
        extra_app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin));
        #[cfg(feature = "client")]
        extra_app.add_plugins(lightyear::prelude::client::ClientPlugins {
            tick_duration: SIMULATION_TICK,
        });
        #[cfg(all(not(feature = "client"), feature = "server"))]
        extra_app.add_plugins(lightyear::prelude::server::ServerPlugins {
            tick_duration: SIMULATION_TICK,
        });
        extra_app.register_message::<ProtocolFingerprintMessage>();
        extra_app.add_plugins(ProtocolPlugin);
        extra_app.finish();
        extra_app.cleanup();
        extra_app.update();
        let extra = extra_app.world().resource::<ProtocolFingerprint>().0;

        assert_ne!(extra, baseline);
    }

    #[derive(Serialize, Deserialize, Clone, Debug)]
    struct ProtocolFingerprintMessage;
}
