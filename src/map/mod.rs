//! Typed map content, authoritative runtime state, and client reconstruction boundaries.

mod catalog;
#[cfg(feature = "client")]
mod client;
mod effect_tiles;
mod model;
mod objects;
mod pickups;
#[cfg(feature = "server")]
mod runtime;
#[cfg(feature = "server")]
mod server;

#[cfg(feature = "client")]
pub(crate) use catalog::effective_projectile_collider;
pub use catalog::{
    ASHEN_COURT_ADMISSION_REVISION, ASHEN_COURT_PRESET, ASHEN_PINE_ASSET, ASHEN_ROCK_ASSET,
    ASHEN_WALL_ASSET, BARREL_WOOD_DEBRIS_ASSET, BREAKABLE_BARRIER_ASSET, CACTUS_ASSET,
    COFFIN_DECORATION_ASSET, DAMAGE_TILE_ASSET, DESTRUCTIBLE_COVER_ASSET,
    EnvironmentExplosionProfile, EnvironmentExplosionProfileId,
    FEATURE_YARD_HEIST_ADMISSION_REVISION, FEATURE_YARD_HEIST_PRESET,
    FEATURE_YARD_HOT_ZONE_ADMISSION_REVISION, FEATURE_YARD_HOT_ZONE_PRESET,
    FEATURE_YARD_WIPEOUT_ADMISSION_REVISION, FEATURE_YARD_WIPEOUT_PRESET, GARDEN_WALL_ASSET,
    GRAVESTONE_DECORATION_ASSET, GREEN_STRIPED_COVER_ASSET, GREEN_SYMBOL_WALL_ASSET, GROUND_ASSET,
    HEIST_MODE_DEFINITION, HEIST_SAFE_VISUAL_PROFILE, HOT_ZONE_MODE_DEFINITION,
    LANTERN_DECORATION_ASSET, MAP_CELL_SIZE_WORLD, MAP_RECIPE_SCHEMA_VERSION,
    MAX_MAP_DIMENSION_CELLS, METAL_WALL_ASSET, MapAssetDefinition, MapAssetId, MapAssetPlacement,
    MapAssetSlot, MapCatalogResource, MapCell, MapColliderShape, MapConcealmentBehavior,
    MapContentCatalog, MapContentPlugin, MapDamageProfile, MapDamageProfileId,
    MapDestructionBehavior, MapDimensionLimits, MapDimensions, MapDurabilityBehavior,
    MapDynamicGeneration, MapDynamicRecoveryRequest, MapDynamicRecoverySnapshot,
    MapDynamicResetEvent, MapDynamicState, MapFilledRect, MapFootprint, MapGameplayProfile,
    MapGameplayProfileId, MapHalfCellPoint, MapInteractionBehavior, MapModeAnchorKind,
    MapModeAnchorPlacement, MapMutationEvent, MapObjectTerminalBehavior, MapPlacementOutcome,
    MapPlacementParameterKind, MapPlacementParameters, MapPlacementTransition, MapPreset,
    MapRecipe, MapSurfaceTagId, MapVisualProfileId, OIL_BARREL_ASSET, PLAYER_SPAWN_ASSET,
    POWDERLINE_VAULT_ADMISSION_REVISION, POWDERLINE_VAULT_PRESET, PlayerCollision,
    ProjectileCollision, RED_BRICK_WALL_ASSET, RUBBLE_ASSET, ResolvedHeistSafeAnchor, ResolvedMap,
    ResolvedMapSnapshot, RestorationPickupDefinition, RestorationPickupDefinitionId,
    SAND_FLOOR_ASSET, SLOW_TILE_ASSET, SPEED_TILE_ASSET, SWITCHBACK_BASIN_ADMISSION_REVISION,
    SWITCHBACK_BASIN_PRESET, TALL_GRASS_ASSET, TREASURE_CHEST_ASSET,
    VERDANT_CROSSFIRE_ADMISSION_REVISION, VERDANT_CROSSFIRE_PRESET, WALL_ARENA_ASSET,
    WALL_DUNGEON_ASSET, WATER_ASSET, WIPEOUT_MODE_DEFINITION, WOOD_WALL_ASSET,
    YELLOW_STRIPED_COVER_ASSET, cardinal_adjacency_mask, circle_overlaps_blocking_map,
    placement_cells, placement_world_center, resolve_circle_against_blocking_map,
};
#[cfg(feature = "client")]
pub use client::{
    ClientMapPlugin, ClientMapReadiness, ClientWorldObjectReadiness, MapPresentationMember,
    MapPresentationPlugin, MapPresentationSet, PresentedMap, perimeter_visual_shapes,
};
pub use effect_tiles::{
    DAMAGE_TILE_DAMAGE, DAMAGE_TILE_INTERVAL_TICKS, EffectTileKind, EffectTileOccupancy,
    MAX_EFFECT_TILE_PLACEMENTS, MapEffectTileBehavior, ResolvedEffectTile,
    SLOW_TILE_MULTIPLIER_MILLI, SPEED_TILE_MULTIPLIER_MILLI,
};
pub use model::{
    AxisAlignedMapRect, MapInstanceId, MapInstanceMember, MapPlacementId, MapPresentationThemeId,
    MapPresetId, MapRecipeFingerprint, MapRecipeId, MapRoot, MapShape, ModeAnchorId,
    ModeDefinitionId, NormalizedArea, PlayableBounds, PracticeDummySpawn, ResolvedMapIdentity,
    SpawnAssignment, SpawnPointCatalog, SpawnPointId, TeamSpawnPoint,
};
#[cfg(feature = "client")]
pub use objects::ReceivedWorldObjectCues;
pub use objects::{
    DamageableLifeState, DamageableMaximumHealth, DamageableObjectAsset, DamageableObjectProfile,
    DamageableTargetClass, DamageableTargetIdentity, DamageableWorldObject,
    MAX_DAMAGEABLE_MAP_OBJECTS, MAX_SECONDARY_DAMAGE_APPLICATIONS, MAX_TERMINAL_REACTIONS_PER_TICK,
    MAX_WORLD_OBJECT_CUES, MAX_WORLD_TARGET_FACTS, WorldObjectCue, WorldObjectExplosionFact,
    WorldObjectExplosionFacts, WorldTargetDamageFact, WorldTargetDamageFacts,
    WorldTargetTerminalFact, object_is_live,
};
#[cfg(feature = "server")]
pub use objects::{
    PendingWorldTargetDamage, PendingWorldTargetDamages, WorldObjectOutbox, WorldObjectTelemetry,
};
#[cfg(feature = "client")]
pub use pickups::ReceivedPickupCues;
pub use pickups::{
    MAX_LIVE_RESTORATION_PICKUPS, MAX_PICKUP_CUES, MAX_PICKUP_FACTS, PickupAvailableAtTick,
    PickupCue, PickupExpiresAtTick, PickupLifecycleFact, PickupLifecycleFacts, PickupLifecycleKind,
    RestorationPickup, RestorationPickupIdentity,
};
#[cfg(feature = "server")]
pub use pickups::{PickupOutbox, PickupTelemetry};
#[cfg(feature = "server")]
pub use runtime::{
    DestructibleMapCollider, MapDynamicTelemetry, MapRuntimeSet, install_resolved_map,
    register_map_runtime,
};

#[cfg(feature = "server")]
pub use server::{
    AuthoritativeMapPlugin, MapStartupSet, NextMapInstanceId, ServerMapSelection,
    perimeter_wall_shapes, teardown_authoritative_map,
};
