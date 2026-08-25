//! Typed map content, authoritative runtime state, and client reconstruction boundaries.

mod catalog;
#[cfg(feature = "client")]
mod client;
mod model;
mod objects;
mod pickups;
#[cfg(feature = "server")]
mod runtime;
#[cfg(feature = "server")]
mod server;

pub use catalog::{
    ASHEN_COURT_ADMISSION_REVISION, ASHEN_COURT_PRESET, ASHEN_PINE_ASSET, ASHEN_ROCK_ASSET,
    ASHEN_WALL_ASSET, BARREL_WOOD_DEBRIS_ASSET, BREAKABLE_BARRIER_ASSET, COFFIN_DECORATION_ASSET,
    DESTRUCTIBLE_COVER_ASSET, EnvironmentExplosionProfile, EnvironmentExplosionProfileId,
    FEATURE_YARD_HEIST_ADMISSION_REVISION, FEATURE_YARD_HEIST_PRESET,
    FEATURE_YARD_HOT_ZONE_ADMISSION_REVISION, FEATURE_YARD_HOT_ZONE_PRESET,
    FEATURE_YARD_WIPEOUT_ADMISSION_REVISION, FEATURE_YARD_WIPEOUT_PRESET, GARDEN_WALL_ASSET,
    GRAVESTONE_DECORATION_ASSET, GROUND_ASSET, HEIST_MODE_DEFINITION, HEIST_SAFE_VISUAL_PROFILE,
    HOT_ZONE_MODE_DEFINITION, LANTERN_DECORATION_ASSET, MAP_CELL_SIZE_WORLD,
    MAP_RECIPE_SCHEMA_VERSION, MapAssetDefinition, MapAssetId, MapAssetPlacement, MapAssetSlot,
    MapCatalogResource, MapCell, MapColliderShape, MapConcealmentBehavior, MapContentCatalog,
    MapContentPlugin, MapDamageProfile, MapDamageProfileId, MapDestructionBehavior, MapDimensions,
    MapDurabilityBehavior, MapDynamicGeneration, MapDynamicRecoveryRequest,
    MapDynamicRecoverySnapshot, MapDynamicResetEvent, MapDynamicState, MapFilledRect, MapFootprint,
    MapGameplayProfile, MapGameplayProfileId, MapGridVertex, MapInteractionBehavior,
    MapModeAnchorKind, MapModeAnchorPlacement, MapMutationEvent, MapObjectTerminalBehavior,
    MapPlacementOutcome, MapPlacementParameterKind, MapPlacementParameters, MapPlacementTransition,
    MapPreset, MapRecipe, MapSurfaceTagId, MapVisualProfileId, OIL_BARREL_ASSET,
    PLAYER_SPAWN_ASSET, PlayerCollision, ProjectileCollision, RUBBLE_ASSET,
    ResolvedHeistSafeAnchor, ResolvedMap, ResolvedMapSnapshot, RestorationPickupDefinition,
    RestorationPickupDefinitionId, SAND_FLOOR_ASSET, TALL_GRASS_ASSET, TREASURE_CHEST_ASSET,
    WALL_ARENA_ASSET, WALL_DUNGEON_ASSET, WATER_ASSET, WIPEOUT_MODE_DEFINITION,
    cardinal_adjacency_mask, circle_overlaps_blocking_map, placement_cells, placement_world_center,
    resolve_circle_against_blocking_map,
};
#[cfg(feature = "client")]
pub use client::{
    ClientMapPlugin, ClientMapReadiness, ClientWorldObjectReadiness, MapPresentationMember,
    MapPresentationPlugin, MapPresentationSet, PresentedMap, perimeter_visual_shapes,
};
pub use model::*;
pub use objects::*;
pub use pickups::*;
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
