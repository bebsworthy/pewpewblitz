use avian2d::prelude::{Collider, CollisionLayers, Position, Rotation};
use bevy::{
    app::App,
    platform::time::Instant,
    prelude::{
        Entity, IntoScheduleConfigs, MinimalPlugins, PreUpdate, Query, ResMut, Resource, Vec2,
        With, Without,
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
        ActiveEffects, AttackDelivery, AttackId, AttackSource, CaptureCombatCues, CombatCue,
        CombatEventId, CombatLogRecord, CombatTelemetry, ComposedProjectileRuntime, CurrentHealth,
        DUMMY_NETWORK_ENTITY, Defeated, FighterDefinitions, Projectile, ProjectileDeadline,
        ReplicatedAttackSource, ResolvedWeapon, SelectedBuild, SelectingWeapon, SpawnState, TeamId,
        TestDummy, WeaponPhase, WeaponPresetId, WeaponRecipeFingerprint, WeaponState,
        WeaponTelemetry, WorldPoint,
    },
    config::{
        ClientNetworkConfig, NetworkImpairmentProfile, NetworkTransport, ServerNetworkConfig,
    },
    gameplay::GameplayPlugin,
    map::{
        AuthoritativeMapPlugin, CollisionProfileId, EntityDefinitionId, GeometryPlacement,
        MapCatalogResource, MapEntityPlacement, MapInstanceId, MapInstanceMember,
        MapLayoutRequirements, MapPlacementId, MapPresentationProfileId,
        MapPresetId as ArenaPresetId, MapRegionPlacement, MapRoot, MapShape, PlayableBounds,
        RegionId, RegionProfileId, ResolvedMap, ResolvedMapSnapshot, SpawnPointId, TeamSpawnPoint,
        VisualPlacementKind, install_resolved_map,
    },
    movement::{
        ArenaWall, AuthoritativeMovementPlugin, AvianNetworkPlugin, InputTuning,
        InputValidationState, MovementTuning,
    },
    protocol::{
        Fighter, FighterInput, NetworkEntityId, PlaceholderPlayer, PlayerId, ProtocolPlugin,
        SessionChannel, TestNativeInputMessage, WeaponSelectionDecision, WeaponSelectionRequest,
        send_forged_native_input_for_test,
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
    LinkSystems, LocalAddr, MessageSender, MessageSystems, NetworkDirection, NetworkTimeline,
    Predicted,
};
use lightyear::transport::plugin::TransportSystems;
use serde::{Deserialize, Serialize};

#[path = "network/harness.rs"]
mod harness;
use harness::*;

#[path = "network/combat_composed.rs"]
mod combat_composed;
#[path = "network/combat_projectiles.rs"]
mod combat_projectiles;
#[path = "network/combat_pulse.rs"]
mod combat_pulse;
#[path = "network/combat_recovery.rs"]
mod combat_recovery;
#[path = "network/lifecycle.rs"]
mod lifecycle;
#[path = "network/lifecycle_roster.rs"]
mod lifecycle_roster;
#[path = "network/map.rs"]
mod map;
#[path = "network/movement.rs"]
mod movement;
#[path = "network/movement_input.rs"]
mod movement_input;
#[path = "network/selection.rs"]
mod selection;
