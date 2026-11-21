#[expect(
    unused_imports,
    reason = "the imports exist only to prove the established public map paths still resolve"
)]
use brawler::map::{
    ASHEN_COURT_ADMISSION_REVISION, ASHEN_COURT_PRESET, ASHEN_PINE_ASSET, ASHEN_ROCK_ASSET,
    ASHEN_WALL_ASSET, AxisAlignedMapRect, BARREL_WOOD_DEBRIS_ASSET, BREAKABLE_BARRIER_ASSET,
    CACTUS_ASSET, COFFIN_DECORATION_ASSET, DAMAGE_TILE_ASSET, DESTRUCTIBLE_COVER_ASSET,
    DamageableLifeState, DamageableMaximumHealth, DamageableObjectAsset, DamageableObjectProfile,
    DamageableTargetClass, DamageableTargetIdentity, DamageableWorldObject, EffectTileCapabilities,
    EffectTileDefinition, EffectTileDefinitionId, EffectTileEffectDefinition,
    EffectTileEffectDefinitionId, EffectTileEffectRef, EffectTileFamilyId, EffectTileOccupancy,
    EffectTileParameter, EffectTileSpawnClearance, EnvironmentExplosionProfile,
    EnvironmentExplosionProfileId, FEATURE_YARD_TAKEDOWN_ADMISSION_REVISION,
    FEATURE_YARD_TAKEDOWN_PRESET, FEATURE_YARD_TURF_HOLD_ADMISSION_REVISION,
    FEATURE_YARD_TURF_HOLD_PRESET, FEATURE_YARD_VAULT_BREAK_ADMISSION_REVISION,
    FEATURE_YARD_VAULT_BREAK_PRESET, GARDEN_WALL_ASSET, GRAVESTONE_DECORATION_ASSET,
    GREEN_STRIPED_COVER_ASSET, GREEN_SYMBOL_WALL_ASSET, GROUND_ASSET, LANTERN_DECORATION_ASSET,
    MAP_CELL_SIZE_WORLD, MAP_RECIPE_SCHEMA_VERSION, MAX_DAMAGEABLE_MAP_OBJECTS,
    MAX_EFFECT_TILE_PLACEMENTS, MAX_LIVE_RESTORATION_PICKUPS, MAX_MAP_DIMENSION_CELLS,
    MAX_PICKUP_CUES, MAX_PICKUP_FACTS, MAX_SECONDARY_DAMAGE_APPLICATIONS,
    MAX_TERMINAL_REACTIONS_PER_TICK, MAX_WORLD_OBJECT_CUES, MAX_WORLD_TARGET_FACTS,
    METAL_WALL_ASSET, MapAssetDefinition, MapAssetId, MapAssetPlacement, MapAssetSlot,
    MapCatalogResource, MapCell, MapColliderShape, MapConcealmentBehavior, MapContentCatalog,
    MapContentPlugin, MapDamageProfile, MapDamageProfileId, MapDestructionBehavior,
    MapDimensionLimits, MapDimensions, MapDurabilityBehavior, MapDynamicGeneration,
    MapDynamicRecoveryRequest, MapDynamicRecoverySnapshot, MapDynamicResetEvent, MapDynamicState,
    MapFilledRect, MapFootprint, MapGameplayProfile, MapGameplayProfileId, MapHalfCellPoint,
    MapInstanceId, MapInstanceMember, MapInteractionBehavior, MapModeAnchorKind,
    MapModeAnchorPlacement, MapMutationEvent, MapObjectTerminalBehavior, MapPlacementId,
    MapPlacementOutcome, MapPlacementParameterKind, MapPlacementParameters, MapPlacementTransition,
    MapPresentationThemeId, MapPreset, MapPresetId, MapRecipe, MapRecipeFingerprint, MapRecipeId,
    MapRoot, MapShape, MapSurfaceTagId, MapVisualProfileId, ModeAnchorId, ModeDefinitionId,
    NormalizedArea, OIL_BARREL_ASSET, PLAYER_SPAWN_ASSET, POWDERLINE_VAULT_ADMISSION_REVISION,
    POWDERLINE_VAULT_PRESET, PickupAvailableAtTick, PickupCue, PickupExpiresAtTick,
    PickupLifecycleFact, PickupLifecycleFacts, PickupLifecycleKind, PlayableBounds,
    PlayerCollision, PracticeDummySpawn, ProjectileCollision, RED_BRICK_WALL_ASSET, RUBBLE_ASSET,
    ResolvedEffectTile, ResolvedMap, ResolvedMapIdentity, ResolvedMapSnapshot,
    ResolvedVaultBreakSafeAnchor, RestorationPickup, RestorationPickupDefinition,
    RestorationPickupDefinitionId, RestorationPickupIdentity, SAND_FLOOR_ASSET, SLOW_TILE_ASSET,
    SPEED_TILE_ASSET, SWITCHBACK_BASIN_ADMISSION_REVISION, SWITCHBACK_BASIN_PRESET,
    SpawnAssignment, SpawnPointCatalog, SpawnPointId, TAKEDOWN_MODE_DEFINITION, TALL_GRASS_ASSET,
    TREASURE_CHEST_ASSET, TURF_HOLD_MODE_DEFINITION, TeamSpawnPoint, VAULT_BREAK_MODE_DEFINITION,
    VAULT_BREAK_SAFE_VISUAL_PROFILE, VERDANT_CROSSFIRE_ADMISSION_REVISION,
    VERDANT_CROSSFIRE_PRESET, WALL_ARENA_ASSET, WALL_DUNGEON_ASSET, WATER_ASSET, WOOD_WALL_ASSET,
    WorldObjectCue, WorldObjectExplosionFact, WorldObjectExplosionFacts, WorldTargetDamageFact,
    WorldTargetDamageFacts, WorldTargetTerminalFact, YELLOW_STRIPED_COVER_ASSET,
    cardinal_adjacency_mask, circle_overlaps_blocking_map, object_is_live, placement_cells,
    placement_world_center, resolve_circle_against_blocking_map,
};

#[cfg(feature = "client")]
#[expect(
    unused_imports,
    reason = "the imports exist only to prove the established public map paths still resolve"
)]
use brawler::map::{
    ClientMapPlugin, ClientMapReadiness, ClientWorldObjectReadiness, MapPresentationMember,
    MapPresentationPlugin, MapPresentationSet, PresentedMap, ReceivedPickupCues,
    ReceivedWorldObjectCues, perimeter_visual_shapes,
};

#[cfg(feature = "server")]
#[expect(
    unused_imports,
    reason = "the imports exist only to prove the established public map paths still resolve"
)]
use brawler::map::{
    AuthoritativeMapPlugin, DestructibleMapCollider, MapDynamicTelemetry, MapRuntimeSet,
    MapStartupSet, NextMapInstanceId, PendingWorldTargetDamage, PendingWorldTargetDamages,
    PickupOutbox, PickupTelemetry, ServerMapSelection, WorldObjectOutbox, WorldObjectTelemetry,
    install_resolved_map, perimeter_wall_shapes, register_map_runtime, teardown_authoritative_map,
};

#[test]
fn established_map_api_remains_importable() {}
