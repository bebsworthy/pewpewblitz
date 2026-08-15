//! Stable protocol registration shared by the client and dedicated server.
#![allow(clippy::needless_pass_by_value)]

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

use crate::builds::{AbilityState, PassiveRuntimeState, ResolvedMatchLoadout};
use crate::combat::{
    ActiveEffects, AttackDelivery, AuthoritativePose, AuthoritativeTick, CombatCue,
    CombatEvidenceCheckpoint, CurrentHealth, Defeated, FighterDefinitionId, KnockbackFeedback,
    LobbedFlight, Projectile, ProjectileDeadline, ProjectileSource, ReplicatedAttackSource,
    ResolvedWeapon, SelectedBuild, SelectingBuild, StraightFlight, TeamId, WeaponDefinitionId,
    WeaponState,
};
use crate::content::GameplayContentFingerprint;
use crate::map::{
    MapInstanceId, MapRoot, ResolvedMapIdentity, ResolvedMapSnapshot, SpawnAssignment,
};
use crate::matchplay::{
    MatchParticipant, MatchRoot as MatchRootMarker, MatchState, RespawnState, SpawnProtection,
};
use crate::timing::SIMULATION_TICK;

/// Netcode protocol ID. Bump this for incompatible wire-level changes.
pub const NETWORK_PROTOCOL_ID: u64 = 0x4252_4157_4c45_5240;

/// Brawler-level compatibility version exchanged after Netcode connects.
pub const SUPPORTED_PROTOCOL_VERSION: u16 = 9;

/// Development-only key for local loopback sessions. This is not authentication.
pub const DEVELOPMENT_PRIVATE_KEY: [u8; 32] = [0x42; 32];

/// Ordered reliable channel for the compatibility handshake and join outcome.
pub struct SessionChannel;

/// Ordered reliable server-to-client stream for presentation-only combat facts.
pub struct CombatChannel;

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

        app.component::<Fighter>().replicate_once();
        app.component::<MatchRootMarker>().replicate_once();
        app.component::<MatchState>().replicate();
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
        app.component::<SelectedBuild>().replicate();
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
        app.component::<ResolvedWeapon>().replicate();
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
        assert!(app.is_message_registered::<MatchCommandRequest>());
        assert!(app.is_message_registered::<MatchCommandOutcome>());
        assert!(app.is_message_registered::<CombatCue>());
        assert!(app.world().contains_resource::<MessageRegistry>());
        assert!(app.world().contains_resource::<ChannelRegistry>());
        let channels = app.world().resource::<ChannelRegistry>();
        assert!((0..32).any(|id| {
            channels.get_name_from_net_id(id) == core::any::type_name::<SessionChannel>()
        }));
        let components = app.world().resource::<ComponentRegistry>();
        assert!(components.is_registered::<Fighter>());
        assert!(components.is_registered::<MatchRootMarker>());
        assert!(components.is_registered::<MatchState>());
        assert!(components.is_registered::<MatchParticipant>());
        assert!(components.is_registered::<RespawnState>());
        assert!(components.is_registered::<SpawnProtection>());
        assert!(components.is_registered::<PlayerId>());
        assert!(components.is_registered::<NetworkEntityId>());
        assert!(components.is_registered::<PlaceholderState>());
        assert!(components.is_registered::<FighterDefinitionId>());
        assert!(components.is_registered::<WeaponDefinitionId>());
        assert!(components.is_registered::<SelectedBuild>());
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
