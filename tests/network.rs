use avian2d::prelude::{Collider, CollisionLayers, Position, Rotation};
use bevy::{
    app::App,
    platform::time::Instant,
    prelude::{
        Entity, IntoScheduleConfigs, Messages, MinimalPlugins, Or, PreUpdate, Query, Res, ResMut,
        Resource, Vec2, With, Without,
    },
    state::app::StatesPlugin,
    time::TimeUpdateStrategy,
};
use brawler::{
    client::{
        ClientJoinPhase, ClientJoinStatus, ClientNetworkPlugin, PendingLocalActions,
        spawn_crossbeam_client,
    },
    combat::{
        ActiveAttackTrackers, ActiveEffects, AmmoRecovery, AttackDelivery, AttackId, AttackSource,
        CaptureCombatCues, CombatCue, CombatEventId, CombatLogRecord, CombatOutbox,
        CombatOutcomeFact, CombatOutcomeFacts, CombatOutcomeKind, CombatSourceKind,
        CombatTelemetry, ComposedProjectileRuntime, CurrentHealth, DUMMY_NETWORK_ENTITY, Defeated,
        FighterDefinitions, HealthRecoveryState, MeleeAttack, PendingDelivery, PendingPayload,
        Projectile, ProjectileDeadline, ReplicatedAttackSource, SpawnState, TeamId, TestDummy,
        TestDummyFixture, TestDummyResetDeadline, WeaponPhase, WeaponPresetId,
        WeaponRecipeFingerprint, WeaponState, WeaponTelemetry, WorldPoint,
    },
    config::{ClientNetworkConfig, NetworkTransport, ServerNetworkConfig},
    gameplay::GameplayPlugin,
    map::{
        AuthoritativeMapPlugin, ClientWorldObjectReadiness, DamageableTargetIdentity,
        DamageableWorldObject, MapCatalogResource, MapDynamicState, MapInstanceId,
        MapInstanceMember, MapPlacementId, MapPlacementParameters, MapPresetId as ArenaPresetId,
        MapRoot, PendingWorldTargetDamage, PendingWorldTargetDamages, PlayableBounds, ResolvedMap,
        ResolvedMapSnapshot, RestorationPickup, RestorationPickupIdentity, SpawnAssignment,
        SpawnPointCatalog, install_resolved_map,
    },
    matchplay::{
        ActiveCombatant, MatchMember, MatchParticipant, MatchPhase, MatchRoot as MatchRootMarker,
        MatchState,
    },
    movement::{
        ArenaWall, AuthoritativeMovementPlugin, AvianNetworkPlugin, InputTuning,
        InputValidationState, MovementTuning,
    },
    protocol::{
        Fighter, FighterInput, MatchCommand, MatchCommandDecision, MatchCommandRequest,
        NetworkEntityId, PlaceholderPlayer, PlayerId, ProtocolPlugin, SessionChannel,
        TestNativeInputMessage, send_forged_native_input_for_test,
    },
    server::{
        ServerNetworkPlugin, ServerSession, ServerSessionPhase, spawn_crossbeam_link,
        spawn_crossbeam_server,
    },
    timing::{SIMULATION_TICK, SimulationTick},
};
use lightyear::prelude::client::{Client, Connected, Disconnect, Disconnected, Remote};
use lightyear::prelude::server::{NetcodeServer, ServerPlugins, Stopped};
use lightyear::prelude::{
    AppMessageExt, ConfirmedHistory, Controlled, Interpolated, InterpolationTimeline, Link,
    LinkSystems, LocalAddr, MessageReceiver, MessageSender, MessageSystems, NetworkDirection,
    NetworkTimeline, Predicted,
};
use lightyear::transport::plugin::TransportSystems;
use serde::{Deserialize, Serialize};

#[path = "network/harness.rs"]
mod harness;
use harness::*;

#[path = "network/heist.rs"]
mod heist;
#[path = "network/hot_zone.rs"]
mod hot_zone;

#[path = "network/builds.rs"]
mod builds;
#[path = "network/combat_composed.rs"]
mod combat_composed;
#[path = "network/combat_projectiles.rs"]
mod combat_projectiles;
#[path = "network/combat_pulse.rs"]
mod combat_pulse;
#[path = "network/combat_recovery.rs"]
mod combat_recovery;
#[path = "network/concealment.rs"]
mod concealment;
#[path = "network/lifecycle.rs"]
mod lifecycle;
#[path = "network/lifecycle_roster.rs"]
mod lifecycle_roster;
#[path = "network/loadouts.rs"]
mod loadouts;
#[path = "network/map.rs"]
mod map;
#[path = "network/match.rs"]
mod matchplay;
#[path = "network/movement.rs"]
mod movement;
#[path = "network/movement_input.rs"]
mod movement_input;
#[cfg(feature = "owner-prediction")]
#[path = "network/prediction.rs"]
mod prediction;
#[path = "network/queue.rs"]
mod queue;
#[path = "network/soaks.rs"]
mod soaks;
