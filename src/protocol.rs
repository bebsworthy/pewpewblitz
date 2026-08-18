//! Stable protocol registration shared by the client and dedicated server.

use avian2d::prelude::{Position, Rotation};
use bevy::ecs::entity::{EntityMapper, MapEntities};
use bevy::prelude::*;
use bevy::reflect::Reflect;
use lightyear::input::config::InputConfig;
#[cfg(feature = "network-test")]
use lightyear::input::input_buffer::InputBuffer;
#[cfg(feature = "network-test")]
use lightyear::input::input_message::{
    ActionStateSequence, InputMessage, InputTarget, PerTargetData,
};
#[cfg(feature = "network-test")]
use lightyear::prelude::Tick;
#[cfg(feature = "network-test")]
use lightyear::prelude::input::InputChannel;
#[cfg(feature = "network-test")]
use lightyear::prelude::input::native::{ActionState, NativeStateSequence};
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
    CombatEvidenceCheckpoint, CurrentHealth, Defeated, FighterDefinitionId, KnockbackFeedback,
    LobbedFlight, Projectile, ProjectileDeadline, ProjectileSource, ReplicatedAttackSource,
    SelectingBuild, StraightFlight, TeamId, WeaponDefinitionId, WeaponState,
};
use crate::content::GameplayContentFingerprint;
use crate::map::{
    MapInstanceId, MapRoot, ResolvedMapIdentity, ResolvedMapSnapshot, SpawnAssignment,
};
use crate::matchplay::{
    HotZoneState, MatchClock, MatchParticipant, MatchRoot as MatchRootMarker, MatchState,
    RespawnState, SpawnProtection, WipeoutState,
};
use crate::timing::SIMULATION_TICK;

/// Netcode protocol ID. Bump this for incompatible wire-level changes.
pub const NETWORK_PROTOCOL_ID: u64 = 0x4252_4157_4c45_5240;

/// Brawler-level compatibility version exchanged after Netcode connects.
pub const SUPPORTED_PROTOCOL_VERSION: u16 = 13;

/// Development-only key for local loopback sessions. This is not authentication.
pub const DEVELOPMENT_PRIVATE_KEY: [u8; 32] = [0x42; 32];

/// Ordered reliable channel for the compatibility handshake and join outcome.
pub struct SessionChannel;

/// Ordered reliable server-to-client stream for presentation-only combat facts.
pub struct CombatChannel;

/// Ordered reliable bidirectional channel for terrain events and bounded recovery. Kept
/// distinct so a fragmented recovery snapshot never blocks joins or combat presentation.
pub struct TerrainChannel;

#[cfg(feature = "network-test")]
pub type TestNativeInputMessage = InputMessage<NativeStateSequence<FighterInput>>;

#[cfg(feature = "network-test")]
#[must_use]
pub fn forged_native_input_message_for_test(
    target: InputTarget,
    end_tick: u32,
    input: FighterInput,
) -> TestNativeInputMessage {
    let mut buffer = InputBuffer::default();
    buffer.set(Tick(end_tick), ActionState(input));
    let states = NativeStateSequence::build_from_input_buffer(&buffer, 1, Tick(end_tick))
        .expect("forged test input sequence should contain one state");
    let mut message = InputMessage::new(Tick(end_tick));
    message.inputs.push(PerTargetData { target, states });
    message
}

#[cfg(feature = "network-test")]
pub fn send_forged_native_input_for_test(
    sender: &mut lightyear::prelude::MessageSender<TestNativeInputMessage>,
    target: InputTarget,
    end_tick: u32,
    input: FighterInput,
) {
    sender.send::<InputChannel>(forged_native_input_message_for_test(
        target, end_tick, input,
    ));
}

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
pub struct ClientHello {
    pub protocol_version: u16,
    pub build_version: String,
    pub registry_fingerprint: u64,
    pub content_fingerprint: GameplayContentFingerprint,
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
pub struct MatchRouteGrantV1 {
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

impl MatchRouteGrantV1 {
    fn validate(&self) -> Result<(), &'static str> {
        if self.activation_expiry_unix_ms > self.route_expiry_unix_ms {
            return Err("activation expiry must not exceed route expiry");
        }
        Ok(())
    }
}

impl core::fmt::Debug for MatchRouteGrantV1 {
    fn fmt(&self, formatter: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        formatter
            .debug_struct("MatchRouteGrantV1")
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
struct MatchRouteGrantV1Wire {
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

impl Serialize for MatchRouteGrantV1 {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        self.validate().map_err(serde::ser::Error::custom)?;
        MatchRouteGrantV1Wire {
            request_id: self.request_id.get(),
            allocation_id: self.allocation_id.get(),
            match_id: self.match_id.get(),
            route_id: self.route_id.get(),
            peer_id: self.peer_id.get(),
            game_mode: match self.game_mode {
                crate::config::GameMode::Wipeout => 1,
                crate::config::GameMode::HotZone => 2,
            },
            capability: self.capability.bytes(),
            activation_expiry_unix_ms: self.activation_expiry_unix_ms,
            route_expiry_unix_ms: self.route_expiry_unix_ms,
        }
        .serialize(serializer)
    }
}

impl<'de> Deserialize<'de> for MatchRouteGrantV1 {
    fn deserialize<D>(deserializer: D) -> Result<Self, D::Error>
    where
        D: serde::Deserializer<'de>,
    {
        let wire = MatchRouteGrantV1Wire::deserialize(deserializer)?;
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
pub struct BuildSelectionRequest {
    pub request_id: u64,
    pub match_id: crate::matchplay::MatchId,
    pub selection: BuildSelection,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildSelection {
    Preset(crate::builds::BuildPresetId),
    Custom(crate::builds::BrawlerBuildRecipe),
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum BuildSelectionDecision {
    Accepted,
    Stale,
    WrongMatch,
    WrongPhase,
    ReadyLocked,
    UnknownId,
    /// Reserved for forward-compatible variable-length recipes; M08's `[PassiveDefinitionId; 2]`
    /// wire type makes this decision unreachable for current clients.
    InvalidSlots,
    InvalidCombination,
    OverBudget,
    ResolutionFailed,
    CandidateTooLarge,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildSelectionOutcome {
    pub request_id: u64,
    pub match_id: crate::matchplay::MatchId,
    pub decision: BuildSelectionDecision,
    pub accepted_identity: Option<crate::builds::SelectedBuild>,
    pub accepted_total_points: Option<u8>,
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

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum JoinOutcome {
    Accepted {
        player_id: PlayerId,
        network_entity_id: NetworkEntityId,
    },
    Rejected {
        reason: JoinRejection,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum JoinRejection {
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

/// Register the terrain wire: one ordered reliable bidirectional channel kept distinct
/// from session and combat traffic so fragmented recovery never blocks either.
fn register_terrain_protocol(app: &mut App) {
    app.register_message::<crate::terrain::TerrainDestructionEvent>()
        .add_direction(NetworkDirection::ServerToClient);
    app.register_message::<crate::terrain::TerrainResetEvent>()
        .add_direction(NetworkDirection::ServerToClient);
    app.register_message::<crate::terrain::TerrainRecoveryRequest>()
        .add_direction(NetworkDirection::ClientToServer);
    app.register_message::<crate::terrain::TerrainRecoverySnapshot>()
        .add_direction(NetworkDirection::ServerToClient);
    app.add_channel::<TerrainChannel>(ChannelSettings {
        mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
        ..default()
    })
    .add_direction(NetworkDirection::Bidirectional);
}

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::combat::WeaponCatalogResource>()
            .add_plugins(crate::builds::BuildContentPlugin)
            .add_plugins(crate::map::MapContentPlugin)
            .add_systems(Startup, initialize_content_fingerprint);
        app.register_message::<ClientHello>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<JoinOutcome>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<MatchRouteGrantV1>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<BuildSelectionRequest>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<BuildSelectionOutcome>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<MatchCommandRequest>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<MatchCommandOutcome>()
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

        app.register_message::<CombatCue>()
            .add_direction(NetworkDirection::ServerToClient);
        app.register_message::<CombatEvidenceCheckpoint>()
            .add_direction(NetworkDirection::ServerToClient);
        app.add_channel::<CombatChannel>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::ServerToClient);

        register_terrain_protocol(app);

        app.component::<Fighter>().replicate_once();
        app.component::<MatchRootMarker>().replicate_once();
        app.component::<MatchState>().replicate();
        app.component::<MatchClock>().replicate();
        app.component::<WipeoutState>().replicate();
        app.component::<HotZoneState>().replicate();
        app.component::<MatchParticipant>().replicate();
        app.component::<RespawnState>().replicate();
        app.component::<SpawnProtection>().replicate();
        app.component::<MapRoot>().replicate_once();
        app.component::<MapInstanceId>().replicate_once();
        app.component::<ResolvedMapIdentity>().replicate_once();
        app.component::<ResolvedMapSnapshot>().replicate_once();
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
        app.component::<SelectingBuild>().replicate();
        app.component::<ActiveEffects>().replicate();
        app.component::<KnockbackFeedback>().replicate();
        app.component::<AttackDelivery>().replicate_once();
        app.component::<LobbedFlight>().replicate_once();
        app.component::<ProjectileDeadline>().replicate_once();
        app.component::<StraightFlight>().replicate_once();
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
        app.add_systems(Startup, initialize_protocol_fingerprint);
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn initialize_content_fingerprint(
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

        assert!(app.is_message_registered::<ClientHello>());
        assert!(app.is_message_registered::<JoinOutcome>());
        assert!(app.is_message_registered::<MatchRouteGrantV1>());
        assert!(app.is_message_registered::<MatchCommandRequest>());
        assert!(app.is_message_registered::<MatchCommandOutcome>());
        assert!(app.is_message_registered::<CombatCue>());
        assert!(app.is_message_registered::<crate::terrain::TerrainDestructionEvent>());
        assert!(app.is_message_registered::<crate::terrain::TerrainResetEvent>());
        assert!(app.is_message_registered::<crate::terrain::TerrainRecoveryRequest>());
        assert!(app.is_message_registered::<crate::terrain::TerrainRecoverySnapshot>());
        assert!(app.world().contains_resource::<MessageRegistry>());
        assert!(app.world().contains_resource::<ChannelRegistry>());
        let channels = app.world().resource::<ChannelRegistry>();
        assert!((0..32).any(|id| {
            channels.get_name_from_net_id(id) == core::any::type_name::<SessionChannel>()
        }));
        assert!((0..32).any(|id| {
            channels.get_name_from_net_id(id) == core::any::type_name::<TerrainChannel>()
        }));
        let components = app.world().resource::<ComponentRegistry>();
        assert!(components.is_registered::<Fighter>());
        assert!(components.is_registered::<MatchRootMarker>());
        assert!(components.is_registered::<MatchState>());
        assert!(components.is_registered::<MatchClock>());
        assert!(components.is_registered::<WipeoutState>());
        assert!(components.is_registered::<HotZoneState>());
        assert!(components.is_registered::<MatchParticipant>());
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
        let message = JoinOutcome::Accepted {
            player_id: PlayerId(4),
            network_entity_id: NetworkEntityId(9),
        };
        let bytes = postcard::to_allocvec(&message).expect("message serializes");
        let decoded: JoinOutcome = postcard::from_bytes(&bytes).expect("message deserializes");
        assert_eq!(decoded, message);
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
        let message = MatchRouteGrantV1 {
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
        let decoded: MatchRouteGrantV1 = postcard::from_bytes(&bytes).expect("grant deserializes");
        assert_eq!(decoded, message);

        let mut zero_request = bytes;
        zero_request[0] = 0;
        assert!(postcard::from_bytes::<MatchRouteGrantV1>(&zero_request).is_err());
    }

    #[test]
    fn match_route_grant_rejects_activation_after_route_expiry() {
        let message = MatchRouteGrantV1 {
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

        let wire = MatchRouteGrantV1Wire {
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
        assert!(postcard::from_bytes::<MatchRouteGrantV1>(&bytes).is_err());
    }

    #[test]
    fn match_route_grant_debug_redacts_capability_bytes() {
        let message = MatchRouteGrantV1 {
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
    fn terrain_messages_install_senders_only_in_their_exact_directions() {
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
                    .get::<MessageSender<crate::terrain::TerrainRecoveryRequest>>(client_link)
                    .is_some()
            );
            assert!(
                world
                    .get::<MessageSender<crate::terrain::TerrainDestructionEvent>>(client_link)
                    .is_none()
            );
            assert!(
                world
                    .get::<MessageSender<crate::terrain::TerrainRecoverySnapshot>>(client_link)
                    .is_none()
            );
            assert!(
                world
                    .get::<MessageSender<MatchRouteGrantV1>>(client_link)
                    .is_none()
            );
            assert!(
                world
                    .get::<MessageSender<crate::terrain::TerrainResetEvent>>(client_link)
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
                    .get::<MessageSender<crate::terrain::TerrainDestructionEvent>>(server_link)
                    .is_some()
            );
            assert!(
                world
                    .get::<MessageSender<crate::terrain::TerrainRecoverySnapshot>>(server_link)
                    .is_some()
            );
            assert!(
                world
                    .get::<MessageSender<crate::terrain::TerrainResetEvent>>(server_link)
                    .is_some()
            );
            assert!(
                world
                    .get::<MessageSender<crate::terrain::TerrainRecoveryRequest>>(server_link)
                    .is_none()
            );
            assert!(
                world
                    .get::<MessageSender<MatchRouteGrantV1>>(server_link)
                    .is_some()
            );
        }
    }

    #[test]
    fn terrain_wire_shapes_round_trip_with_serde() {
        use crate::terrain::{
            TerrainBits, TerrainBrush, TerrainChunkId, TerrainChunkSnapshot,
            TerrainDestructionEvent, TerrainGeneration, TerrainRecoverySnapshot, TerrainResetEvent,
        };
        let generation = TerrainGeneration {
            map_instance_id: MapInstanceId(3),
            match_id: crate::matchplay::MatchId(7),
            terrain_fingerprint: 0x1234_5678_9abc_def0,
        };
        let event = TerrainDestructionEvent {
            generation,
            revision: 42,
            source_attack_id: crate::combat::AttackId(11),
            source_delivery_index: 0,
            brush: TerrainBrush {
                center_half_cells_x: -3,
                center_half_cells_y: 5,
                radius_half_cells: 12,
            },
            affected_chunks: vec![
                TerrainChunkId { x: -1, y: 0 },
                TerrainChunkId { x: 0, y: 0 },
            ],
            erased_cells: 77,
        };
        let bytes = postcard::to_allocvec(&event).expect("event serializes");
        let decoded: TerrainDestructionEvent =
            postcard::from_bytes(&bytes).expect("event deserializes");
        assert_eq!(decoded, event);

        let snapshot = TerrainRecoverySnapshot {
            generation,
            revision: 42,
            chunks: vec![TerrainChunkSnapshot {
                chunk_id: TerrainChunkId { x: 0, y: 0 },
                occupancy: TerrainBits([u64::MAX; 16]),
            }],
        };
        let bytes = postcard::to_allocvec(&snapshot).expect("snapshot serializes");
        let decoded: TerrainRecoverySnapshot =
            postcard::from_bytes(&bytes).expect("snapshot deserializes");
        assert_eq!(decoded, snapshot);
        let reset = TerrainResetEvent {
            previous_generation: generation,
            next_generation: TerrainGeneration {
                match_id: crate::matchplay::MatchId(8),
                ..generation
            },
        };
        let bytes = postcard::to_allocvec(&reset).expect("reset serializes");
        let decoded: TerrainResetEvent = postcard::from_bytes(&bytes).expect("reset deserializes");
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
