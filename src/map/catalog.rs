//! Sparse-grid map assets, authored recipes, canonical resolution, and shared wire state.
//!
//! This module is intentionally independent from the V4 object-role and region catalogs. During
//! the canonical runtime it derives the neutral runtime facts still consumed by existing
//! match systems; it never converts a grid placement into an authored V4 recipe or region.

use super::{
    AxisAlignedMapRect, MapEffectTileBehavior, MapInstanceId, MapPlacementId,
    MapPresentationThemeId, MapPresetId, MapRecipeFingerprint, MapRecipeId, MapShape, ModeAnchorId,
    ModeDefinitionId, ResolvedEffectTile, ResolvedMapIdentity, SpawnPointId, TeamSpawnPoint,
};
use bevy::prelude::{App, Component, Plugin, Resource, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAP_CELL_SIZE_WORLD: f32 = 32.0;
pub const MAX_MAP_OBJECT_HEALTH: u16 = 1_000;
pub const MAX_RESOLVED_MAP_SNAPSHOT_BYTES: usize = 32 * 1024 * 1024;
pub const MAP_CATALOG_SCHEMA_VERSION: u16 = 7;
pub const MAP_RECIPE_SCHEMA_VERSION: u16 = 5;
pub const MAP_FINGERPRINT_FORMAT_VERSION: u16 = 9;
#[cfg(test)]
const CROSSROADS_PRESET: MapPresetId = MapPresetId(1);
pub const ASHEN_COURT_PRESET: MapPresetId = MapPresetId(3);
pub const ASHEN_COURT_ADMISSION_REVISION: u16 = 2;
#[cfg(test)]
const TIDAL_GARDEN_PRESET: MapPresetId = MapPresetId(4);
#[cfg(test)]
const BARREL_YARD_PRESET: MapPresetId = MapPresetId(5);
pub const FEATURE_YARD_WIPEOUT_PRESET: MapPresetId = MapPresetId(7);
pub const FEATURE_YARD_WIPEOUT_ADMISSION_REVISION: u16 = 5;
pub const FEATURE_YARD_HOT_ZONE_PRESET: MapPresetId = MapPresetId(8);
pub const FEATURE_YARD_HOT_ZONE_ADMISSION_REVISION: u16 = 2;
pub const FEATURE_YARD_HEIST_PRESET: MapPresetId = MapPresetId(9);
pub const FEATURE_YARD_HEIST_ADMISSION_REVISION: u16 = 2;
pub const VERDANT_CROSSFIRE_PRESET: MapPresetId = MapPresetId(10);
pub const VERDANT_CROSSFIRE_ADMISSION_REVISION: u16 = 2;
pub const SWITCHBACK_BASIN_PRESET: MapPresetId = MapPresetId(11);
pub const SWITCHBACK_BASIN_ADMISSION_REVISION: u16 = 2;
pub const POWDERLINE_VAULT_PRESET: MapPresetId = MapPresetId(12);
pub const POWDERLINE_VAULT_ADMISSION_REVISION: u16 = 3;
pub const WIPEOUT_MODE_DEFINITION: ModeDefinitionId = ModeDefinitionId(2);
pub const HOT_ZONE_MODE_DEFINITION: ModeDefinitionId = ModeDefinitionId(3);
pub const HEIST_MODE_DEFINITION: ModeDefinitionId = ModeDefinitionId(4);
pub const HEIST_SAFE_VISUAL_PROFILE: MapVisualProfileId = MapVisualProfileId(39);

pub const GROUND_ASSET: MapAssetId = MapAssetId(1);
pub const WALL_ARENA_ASSET: MapAssetId = MapAssetId(2);
pub const WALL_DUNGEON_ASSET: MapAssetId = MapAssetId(3);
pub const DESTRUCTIBLE_COVER_ASSET: MapAssetId = MapAssetId(4);
pub const SAND_FLOOR_ASSET: MapAssetId = MapAssetId(5);
pub const WATER_ASSET: MapAssetId = MapAssetId(6);
pub const GARDEN_WALL_ASSET: MapAssetId = MapAssetId(7);
pub const TALL_GRASS_ASSET: MapAssetId = MapAssetId(8);
pub const BREAKABLE_BARRIER_ASSET: MapAssetId = MapAssetId(9);
pub const RUBBLE_ASSET: MapAssetId = MapAssetId(14);
pub const ASHEN_WALL_ASSET: MapAssetId = MapAssetId(16);
pub const ASHEN_PINE_ASSET: MapAssetId = MapAssetId(17);
pub const ASHEN_ROCK_ASSET: MapAssetId = MapAssetId(18);
pub const PLAYER_SPAWN_ASSET: MapAssetId = MapAssetId(20);
pub const GRAVESTONE_DECORATION_ASSET: MapAssetId = MapAssetId(21);
pub const COFFIN_DECORATION_ASSET: MapAssetId = MapAssetId(22);
pub const LANTERN_DECORATION_ASSET: MapAssetId = MapAssetId(23);
pub const OIL_BARREL_ASSET: MapAssetId = MapAssetId(24);
pub const BARREL_WOOD_DEBRIS_ASSET: MapAssetId = MapAssetId(25);
pub const TREASURE_CHEST_ASSET: MapAssetId = MapAssetId(26);
pub const CACTUS_ASSET: MapAssetId = MapAssetId(28);
pub const GREEN_SYMBOL_WALL_ASSET: MapAssetId = MapAssetId(29);
pub const RED_BRICK_WALL_ASSET: MapAssetId = MapAssetId(30);
pub const METAL_WALL_ASSET: MapAssetId = MapAssetId(31);
pub const WOOD_WALL_ASSET: MapAssetId = MapAssetId(32);
pub const YELLOW_STRIPED_COVER_ASSET: MapAssetId = MapAssetId(33);
pub const GREEN_STRIPED_COVER_ASSET: MapAssetId = MapAssetId(34);
pub const SPEED_TILE_ASSET: MapAssetId = MapAssetId(35);
pub const SLOW_TILE_ASSET: MapAssetId = MapAssetId(36);
pub const DAMAGE_TILE_ASSET: MapAssetId = MapAssetId(37);

macro_rules! grid_id {
    ($name:ident) => {
        #[derive(
            Serialize,
            Deserialize,
            Clone,
            Copy,
            Debug,
            Default,
            PartialEq,
            Eq,
            Hash,
            Ord,
            PartialOrd,
        )]
        pub struct $name(pub u16);
    };
}

grid_id!(MapAssetId);
grid_id!(MapGameplayProfileId);
grid_id!(MapVisualProfileId);
grid_id!(MapSurfaceTagId);
grid_id!(MapDamageProfileId);
grid_id!(EnvironmentExplosionProfileId);

#[derive(
    Component,
    Serialize,
    Deserialize,
    Clone,
    Copy,
    Debug,
    Default,
    PartialEq,
    Eq,
    Hash,
    Ord,
    PartialOrd,
)]
pub struct RestorationPickupDefinitionId(pub u16);

#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Ord, PartialOrd,
)]
#[serde(deny_unknown_fields)]
pub struct MapCell {
    pub x: u16,
    pub y: u16,
}

impl MapCell {
    #[must_use]
    pub const fn new(x: u16, y: u16) -> Self {
        Self { x, y }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MapDimensions {
    pub width: u16,
    pub height: u16,
}

/// Hard engine ceiling for either authored map axis.
///
/// Server operator policy may choose a narrower envelope, but it may not widen this bound without
/// extending the measured map/runtime capacity contract.
pub const MAX_MAP_DIMENSION_CELLS: u16 = 512;
const MAP_ASSET_SLOT_COUNT: usize = 4;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MapDimensionLimits {
    pub minimum_width: u16,
    pub minimum_height: u16,
    pub maximum_width: u16,
    pub maximum_height: u16,
}

impl Default for MapDimensionLimits {
    fn default() -> Self {
        Self {
            minimum_width: 20,
            minimum_height: 20,
            maximum_width: MAX_MAP_DIMENSION_CELLS,
            maximum_height: MAX_MAP_DIMENSION_CELLS,
        }
    }
}

impl MapDimensionLimits {
    pub fn validate(self) -> Result<(), String> {
        if self.minimum_width == 0
            || self.minimum_height == 0
            || self.minimum_width > self.maximum_width
            || self.minimum_height > self.maximum_height
            || self.maximum_width > MAX_MAP_DIMENSION_CELLS
            || self.maximum_height > MAX_MAP_DIMENSION_CELLS
        {
            return Err(format!(
                "map dimension limits must be positive, ordered, and no greater than {MAX_MAP_DIMENSION_CELLS} cells per axis"
            ));
        }
        Ok(())
    }

    pub fn validate_dimensions(self, dimensions: MapDimensions) -> Result<(), String> {
        self.validate()?;
        dimensions.validate()?;
        if !(self.minimum_width..=self.maximum_width).contains(&dimensions.width)
            || !(self.minimum_height..=self.maximum_height).contains(&dimensions.height)
        {
            return Err(format!(
                "map dimensions {}x{} fall outside the server envelope {}..={} by {}..={} cells",
                dimensions.width,
                dimensions.height,
                self.minimum_width,
                self.maximum_width,
                self.minimum_height,
                self.maximum_height
            ));
        }
        Ok(())
    }
}

impl MapDimensions {
    pub fn validate(self) -> Result<(), String> {
        if !(1..=MAX_MAP_DIMENSION_CELLS).contains(&self.width)
            || !(1..=MAX_MAP_DIMENSION_CELLS).contains(&self.height)
        {
            return Err(format!(
                "grid map dimensions must be 1..={MAX_MAP_DIMENSION_CELLS} cells per axis"
            ));
        }
        Ok(())
    }

    #[must_use]
    pub const fn cell_count(self) -> usize {
        self.width as usize * self.height as usize
    }

    #[must_use]
    const fn placement_capacity(self) -> usize {
        self.cell_count() * MAP_ASSET_SLOT_COUNT
    }

    #[must_use]
    pub fn world_size(self) -> Vec2 {
        Vec2::new(
            f32::from(self.width) * MAP_CELL_SIZE_WORLD,
            f32::from(self.height) * MAP_CELL_SIZE_WORLD,
        )
    }

    #[must_use]
    pub fn bounds(self) -> AxisAlignedMapRect {
        let half = self.world_size() * 0.5;
        AxisAlignedMapRect {
            min: -half,
            max: half,
        }
    }

    #[must_use]
    pub fn contains(self, cell: MapCell) -> bool {
        cell.x < self.width && cell.y < self.height
    }

    #[must_use]
    pub fn cell_min(self, cell: MapCell) -> Vec2 {
        self.bounds().min + Vec2::new(f32::from(cell.x), f32::from(cell.y)) * MAP_CELL_SIZE_WORLD
    }

    #[must_use]
    pub fn cell_center(self, cell: MapCell) -> Vec2 {
        self.cell_min(cell) + Vec2::splat(MAP_CELL_SIZE_WORLD * 0.5)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub enum MapAssetSlot {
    Surface,
    Feature,
    Decoration,
    Marker,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlayerCollision {
    Pass,
    Block,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProjectileCollision {
    Pass,
    BlockAndConsume,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapColliderShape {
    None,
    FootprintRectangle,
    Circle { radius_world_units: u16 },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapDestructionBehavior {
    Indestructible,
    RemoveOnMapDestruction,
    ReplaceOnMapDestruction(MapAssetId),
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MapDurabilityBehavior {
    #[default]
    Indestructible,
    HitPoints(MapDamageProfileId),
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapObjectTerminalBehavior {
    Explode {
        explosion_profile_id: EnvironmentExplosionProfileId,
        outcome: MapPlacementOutcome,
    },
    DropPickup {
        pickup_definition_id: RestorationPickupDefinitionId,
        outcome: MapPlacementOutcome,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MapDamageProfile {
    pub id: MapDamageProfileId,
    pub maximum_health: u16,
    pub terminal: MapObjectTerminalBehavior,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct EnvironmentExplosionProfile {
    pub id: EnvironmentExplosionProfileId,
    pub damage: u16,
    pub radius_world_units: u16,
    pub maximum_targets: u8,
    pub maximum_chain_reactions: u8,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct RestorationPickupDefinition {
    pub id: RestorationPickupDefinitionId,
    pub restoration: u16,
    pub collection_radius_world_units: u16,
    pub lifetime_ticks: u16,
    pub visual_profile_id: MapVisualProfileId,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapInteractionBehavior {
    None,
    PlayerSpawn,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapConcealmentBehavior {
    None,
    HideOccupants,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapPlacementParameterKind {
    None,
    PlayerSpawn,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MapGameplayProfile {
    pub id: MapGameplayProfileId,
    pub player_collision: PlayerCollision,
    pub projectile_collision: ProjectileCollision,
    pub collider_shape: MapColliderShape,
    pub destruction: MapDestructionBehavior,
    pub durability: MapDurabilityBehavior,
    pub interaction: MapInteractionBehavior,
    pub concealment: MapConcealmentBehavior,
    pub effect_tile: MapEffectTileBehavior,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MapAssetDefinition {
    pub id: MapAssetId,
    pub key: String,
    pub display_name: String,
    pub slot: MapAssetSlot,
    pub gameplay_profile_id: MapGameplayProfileId,
    pub visual_profile_id: MapVisualProfileId,
    pub footprint_cells: MapFootprint,
    pub allowed_quarter_turns: u8,
    pub surface_tag: Option<MapSurfaceTagId>,
    pub allowed_surface_tags: Vec<MapSurfaceTagId>,
    pub parameter_kind: MapPlacementParameterKind,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MapFootprint {
    pub width: u8,
    pub height: u8,
}

impl MapFootprint {
    #[must_use]
    pub const fn rotated(self, quarter_turns: u8) -> Self {
        if quarter_turns & 1 == 0 {
            self
        } else {
            Self {
                width: self.height,
                height: self.width,
            }
        }
    }
}

#[must_use]
pub fn placement_cells(
    dimensions: MapDimensions,
    asset: &MapAssetDefinition,
    placement: &MapAssetPlacement,
) -> Option<Vec<MapCell>> {
    let footprint = asset.footprint_cells.rotated(placement.quarter_turns);
    let mut cells =
        Vec::with_capacity(usize::from(footprint.width) * usize::from(footprint.height));
    for y in 0..u16::from(footprint.height) {
        for x in 0..u16::from(footprint.width) {
            let cell = MapCell::new(
                placement.cell.x.checked_add(x)?,
                placement.cell.y.checked_add(y)?,
            );
            if !dimensions.contains(cell) {
                return None;
            }
            cells.push(cell);
        }
    }
    Some(cells)
}

#[must_use]
pub fn placement_world_center(
    dimensions: MapDimensions,
    asset: &MapAssetDefinition,
    placement: &MapAssetPlacement,
) -> Vec2 {
    let footprint = asset.footprint_cells.rotated(placement.quarter_turns);
    dimensions.cell_min(placement.cell)
        + Vec2::new(f32::from(footprint.width), f32::from(footprint.height))
            * (MAP_CELL_SIZE_WORLD * 0.5)
}

/// Four-bit north/east/south/west neighborhood mask used by bounded tile presentation.
#[must_use]
pub fn cardinal_adjacency_mask(cell: MapCell, occupied: &BTreeSet<MapCell>) -> u8 {
    let north = cell
        .y
        .checked_add(1)
        .is_some_and(|y| occupied.contains(&MapCell::new(cell.x, y)));
    let east = cell
        .x
        .checked_add(1)
        .is_some_and(|x| occupied.contains(&MapCell::new(x, cell.y)));
    let south = cell
        .y
        .checked_sub(1)
        .is_some_and(|y| occupied.contains(&MapCell::new(cell.x, y)));
    let west = cell
        .x
        .checked_sub(1)
        .is_some_and(|x| occupied.contains(&MapCell::new(x, cell.y)));
    u8::from(north) | (u8::from(east) << 1) | (u8::from(south) << 2) | (u8::from(west) << 3)
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapCellRect {
    pub min: MapCell,
    pub width: u16,
    pub height: u16,
}

fn merge_cells_to_rectangles(mut cells: BTreeSet<MapCell>) -> Vec<MapCellRect> {
    let mut rectangles = Vec::new();
    while let Some(start) = cells.first().copied() {
        let mut width = 1_u16;
        while cells.contains(&MapCell::new(start.x + width, start.y)) {
            width += 1;
        }
        let mut height = 1_u16;
        'rows: while let Some(y) = start.y.checked_add(height) {
            for x in start.x..start.x + width {
                if !cells.contains(&MapCell::new(x, y)) {
                    break 'rows;
                }
            }
            height += 1;
        }
        for y in start.y..start.y + height {
            for x in start.x..start.x + width {
                cells.remove(&MapCell::new(x, y));
            }
        }
        rectangles.push(MapCellRect {
            min: start,
            width,
            height,
        });
    }
    rectangles
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapPlacementParameters {
    None,
    PlayerSpawn {
        team_slot: u8,
        ordinal: u8,
        facing_quarter_turns: u8,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MapAssetPlacement {
    pub placement_id: MapPlacementId,
    pub cell: MapCell,
    pub asset_id: MapAssetId,
    pub quarter_turns: u8,
    pub parameters: MapPlacementParameters,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MapFilledRect {
    pub first_placement_id: MapPlacementId,
    pub min: MapCell,
    pub width: u16,
    pub height: u16,
    pub asset_id: MapAssetId,
    pub quarter_turns: u8,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Ord, PartialOrd)]
#[serde(deny_unknown_fields)]
pub struct MapHalfCellPoint {
    pub x: u16,
    pub y: u16,
}

impl MapDimensions {
    #[must_use]
    pub fn half_cell_world(self, point: MapHalfCellPoint) -> Option<Vec2> {
        (u32::from(point.x) <= u32::from(self.width) * 2
            && u32::from(point.y) <= u32::from(self.height) * 2)
            .then(|| {
                self.bounds().min
                    + Vec2::new(f32::from(point.x), f32::from(point.y))
                        * (MAP_CELL_SIZE_WORLD * 0.5)
            })
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub enum MapModeAnchorKind {
    HotZoneCircle {
        center_half_cell: MapHalfCellPoint,
        radius_half_cells: u16,
    },
    HeistSafe {
        team_slot: u8,
        origin_cell: MapCell,
        width_cells: u16,
        height_cells: u16,
        quarter_turns: u8,
        objective_visual_profile_id: MapVisualProfileId,
    },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
#[serde(deny_unknown_fields)]
pub struct MapModeAnchorPlacement {
    pub placement_id: MapPlacementId,
    pub anchor_id: ModeAnchorId,
    pub kind: MapModeAnchorKind,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MapRecipe {
    pub recipe_id: MapRecipeId,
    pub revision: u32,
    pub recipe_version: u16,
    pub mode_definition_id: ModeDefinitionId,
    pub presentation_theme_id: MapPresentationThemeId,
    pub dimensions: MapDimensions,
    pub default_surface_asset_id: MapAssetId,
    pub placements: Vec<MapAssetPlacement>,
    pub filled_rects: Vec<MapFilledRect>,
    pub mode_anchors: Vec<MapModeAnchorPlacement>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MapPreset {
    pub id: MapPresetId,
    pub key: String,
    pub display_name: String,
    pub admission_revision: u16,
    pub recipe: MapRecipe,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MapPresentationTheme {
    pub id: MapPresentationThemeId,
    pub key: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MapContentCatalog {
    pub schema_version: u16,
    pub gameplay_profiles: Vec<MapGameplayProfile>,
    pub damage_profiles: Vec<MapDamageProfile>,
    pub explosion_profiles: Vec<EnvironmentExplosionProfile>,
    pub restoration_pickups: Vec<RestorationPickupDefinition>,
    pub assets: Vec<MapAssetDefinition>,
    pub presentation_themes: Vec<MapPresentationTheme>,
    pub presets: Vec<MapPreset>,
}

#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct MapCatalogResource(pub MapContentCatalog);

impl bevy::prelude::FromWorld for MapCatalogResource {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self(MapContentCatalog::embedded().expect("embedded grid map catalog is valid"))
    }
}

/// Installs the one build-embedded, headless-safe map catalog.
pub struct MapContentPlugin;

impl Plugin for MapContentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapCatalogResource>();
    }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct GameplayProfileSource {
    schema_version: u16,
    gameplay_profiles: Vec<MapGameplayProfile>,
    damage_profiles: Vec<MapDamageProfile>,
    explosion_profiles: Vec<EnvironmentExplosionProfile>,
    restoration_pickups: Vec<RestorationPickupDefinition>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapAssetSource {
    schema_version: u16,
    assets: Vec<MapAssetDefinition>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapPresentationThemeSource {
    schema_version: u16,
    themes: Vec<MapPresentationTheme>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapIndexSource {
    schema_version: u16,
    maps: Vec<MapIndexEntry>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapIndexEntry {
    id: MapPresetId,
    key: String,
    display_name: String,
    admission_revision: u16,
    document: String,
}

include!(concat!(env!("OUT_DIR"), "/embedded_builtin_maps.rs"));

impl MapContentCatalog {
    pub fn embedded() -> Result<Self, String> {
        let profiles: GameplayProfileSource = ron::from_str(include_str!(
            "../../content/catalogs/map_gameplay_profiles.ron"
        ))
        .map_err(|error| format!("map gameplay profiles parse failed: {error}"))?;
        let assets: MapAssetSource =
            ron::from_str(include_str!("../../content/catalogs/map_assets.ron"))
                .map_err(|error| format!("map assets parse failed: {error}"))?;
        let themes: MapPresentationThemeSource = ron::from_str(include_str!(
            "../../content/catalogs/map_presentation_themes.ron"
        ))
        .map_err(|error| format!("map presentation themes parse failed: {error}"))?;
        let index: MapIndexSource = ron::from_str(include_str!("../../content/maps/index.ron"))
            .map_err(|error| format!("map index parse failed: {error}"))?;
        if profiles.schema_version != MAP_CATALOG_SCHEMA_VERSION
            || assets.schema_version != MAP_CATALOG_SCHEMA_VERSION
            || themes.schema_version != MAP_CATALOG_SCHEMA_VERSION
            || index.schema_version != MAP_CATALOG_SCHEMA_VERSION
        {
            return Err("unsupported grid map catalog schema".to_string());
        }
        let sources: BTreeMap<_, _> = EMBEDDED_BUILTIN_MAPS.iter().copied().collect();
        if sources.len() != index.maps.len() {
            return Err("map index and embedded source count differ".to_string());
        }
        let mut presets = Vec::with_capacity(index.maps.len());
        for entry in index.maps {
            if entry.document != format!("builtin/{}.ron", entry.key) {
                return Err("map document path must match its stable key".to_string());
            }
            let source = sources
                .get(entry.document.as_str())
                .ok_or_else(|| "indexed map document is missing".to_string())?;
            if source.len() > 96 * 1024 {
                return Err("map recipe exceeds the source byte ceiling".to_string());
            }
            let recipe = ron::from_str(source)
                .map_err(|error| format!("map recipe {} parse failed: {error}", entry.key))?;
            presets.push(MapPreset {
                id: entry.id,
                key: entry.key,
                display_name: entry.display_name,
                admission_revision: entry.admission_revision,
                recipe,
            });
        }
        presets.sort_by_key(|preset| preset.id);
        let catalog = Self {
            schema_version: MAP_CATALOG_SCHEMA_VERSION,
            gameplay_profiles: profiles.gameplay_profiles,
            damage_profiles: profiles.damage_profiles,
            explosion_profiles: profiles.explosion_profiles,
            restoration_pickups: profiles.restoration_pickups,
            assets: assets.assets,
            presentation_themes: themes.themes,
            presets,
        };
        catalog.validate()?;
        for preset in &catalog.presets {
            catalog.resolve_preset(preset.id, MapInstanceId(1))?;
        }
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != MAP_CATALOG_SCHEMA_VERSION
            || self.gameplay_profiles.is_empty()
            || self.damage_profiles.is_empty()
            || self.explosion_profiles.is_empty()
            || self.restoration_pickups.is_empty()
            || self.assets.is_empty()
            || self.presentation_themes.is_empty()
            || self.presets.is_empty()
        {
            return Err("invalid or empty grid map catalog".to_string());
        }
        validate_sorted_ids(
            self.gameplay_profiles.iter().map(|profile| profile.id.0),
            "gameplay profiles",
        )?;
        validate_damageable_profiles(self)?;
        validate_restoration_pickups(self)?;
        validate_sorted_ids(self.assets.iter().map(|asset| asset.id.0), "map assets")?;
        validate_sorted_ids(
            self.presentation_themes.iter().map(|theme| theme.id.0),
            "map presentation themes",
        )?;
        validate_sorted_ids(self.presets.iter().map(|preset| preset.id.0), "map presets")?;
        let profiles: BTreeMap<_, _> = self
            .gameplay_profiles
            .iter()
            .map(|profile| (profile.id, profile))
            .collect();
        let mut keys = BTreeSet::new();
        for asset in &self.assets {
            let profile = profiles
                .get(&asset.gameplay_profile_id)
                .ok_or_else(|| format!("asset {} references an unknown profile", asset.key))?;
            if !valid_key(&asset.key)
                || asset.display_name.trim().is_empty()
                || asset.display_name.len() > 64
                || asset.visual_profile_id.0 == 0
                || asset.allowed_quarter_turns == 0
                || asset.allowed_quarter_turns & !0b1111 != 0
                || !(1..=8).contains(&asset.footprint_cells.width)
                || !(1..=8).contains(&asset.footprint_cells.height)
                || !keys.insert(asset.key.as_str())
                || !asset
                    .allowed_surface_tags
                    .windows(2)
                    .all(|pair| pair[0] < pair[1])
            {
                return Err(format!("invalid map asset: {}", asset.key));
            }
            validate_asset_profile(asset, **profile)?;
        }
        validate_replacement_assets(self, &profiles)?;
        let theme_ids: BTreeSet<_> = self
            .presentation_themes
            .iter()
            .map(|theme| theme.id)
            .collect();
        let mut theme_keys = BTreeSet::new();
        for theme in &self.presentation_themes {
            if !valid_key(&theme.key) || !theme_keys.insert(theme.key.as_str()) {
                return Err("invalid map presentation theme metadata".to_string());
            }
        }
        let mut preset_keys = BTreeSet::new();
        for preset in &self.presets {
            if preset.admission_revision == 0
                || !valid_key(&preset.key)
                || preset.display_name.trim().is_empty()
                || !preset_keys.insert(preset.key.as_str())
                || !theme_ids.contains(&preset.recipe.presentation_theme_id)
            {
                return Err("invalid map preset metadata".to_string());
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn preset(&self, id: MapPresetId) -> Option<&MapPreset> {
        self.presets.iter().find(|preset| preset.id == id)
    }

    #[must_use]
    pub fn asset(&self, id: MapAssetId) -> Option<&MapAssetDefinition> {
        self.assets.iter().find(|asset| asset.id == id)
    }

    #[must_use]
    pub fn profile(&self, id: MapGameplayProfileId) -> Option<&MapGameplayProfile> {
        self.gameplay_profiles
            .iter()
            .find(|profile| profile.id == id)
    }

    #[must_use]
    pub fn damage_profile(&self, id: MapDamageProfileId) -> Option<&MapDamageProfile> {
        self.damage_profiles.iter().find(|profile| profile.id == id)
    }

    #[must_use]
    pub fn explosion_profile(
        &self,
        id: EnvironmentExplosionProfileId,
    ) -> Option<&EnvironmentExplosionProfile> {
        self.explosion_profiles
            .iter()
            .find(|profile| profile.id == id)
    }

    #[must_use]
    pub fn restoration_pickup(
        &self,
        id: RestorationPickupDefinitionId,
    ) -> Option<&RestorationPickupDefinition> {
        self.restoration_pickups
            .iter()
            .find(|definition| definition.id == id)
    }

    pub fn canonical_fingerprint_material(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut canonical = self.clone();
        canonical
            .gameplay_profiles
            .sort_by_key(|profile| profile.id);
        canonical.damage_profiles.sort_by_key(|profile| profile.id);
        canonical
            .explosion_profiles
            .sort_by_key(|profile| profile.id);
        canonical
            .restoration_pickups
            .sort_by_key(|definition| definition.id);
        canonical.assets.sort_by_key(|asset| asset.id);
        canonical.presentation_themes.sort_by_key(|theme| theme.id);
        canonical.presets.sort_by_key(|preset| preset.id);
        for preset in &mut canonical.presets {
            preset
                .recipe
                .placements
                .sort_by_key(|placement| placement.placement_id);
            preset
                .recipe
                .filled_rects
                .sort_by_key(|rect| rect.first_placement_id);
            preset
                .recipe
                .mode_anchors
                .sort_by_key(|anchor| (anchor.placement_id, anchor.anchor_id));
        }
        postcard::to_allocvec(&(MAP_FINGERPRINT_FORMAT_VERSION, canonical))
            .map_err(|error| format!("grid map catalog fingerprint failed: {error}"))
    }

    pub fn resolve_preset(
        &self,
        preset_id: MapPresetId,
        instance_id: MapInstanceId,
    ) -> Result<ResolvedMap, String> {
        let preset = self
            .preset(preset_id)
            .ok_or_else(|| "unknown grid map preset".to_string())?;
        resolve_grid_recipe(&preset.recipe, preset.id, instance_id, self)
    }
}

fn validate_replacement_assets(
    catalog: &MapContentCatalog,
    profiles: &BTreeMap<MapGameplayProfileId, &MapGameplayProfile>,
) -> Result<(), String> {
    let replacement_for = |profile: &MapGameplayProfile| match profile.durability {
        MapDurabilityBehavior::HitPoints(id) => {
            catalog
                .damage_profile(id)
                .and_then(|damage| match damage.terminal {
                    MapObjectTerminalBehavior::Explode {
                        outcome: MapPlacementOutcome::ReplacedWith(id),
                        ..
                    }
                    | MapObjectTerminalBehavior::DropPickup {
                        outcome: MapPlacementOutcome::ReplacedWith(id),
                        ..
                    } => Some(id),
                    MapObjectTerminalBehavior::Explode {
                        outcome: MapPlacementOutcome::Removed,
                        ..
                    }
                    | MapObjectTerminalBehavior::DropPickup {
                        outcome: MapPlacementOutcome::Removed,
                        ..
                    } => None,
                })
        }
        MapDurabilityBehavior::Indestructible => match profile.destruction {
            MapDestructionBehavior::ReplaceOnMapDestruction(id) => Some(id),
            MapDestructionBehavior::Indestructible
            | MapDestructionBehavior::RemoveOnMapDestruction => None,
        },
    };
    let terminal_assets: BTreeSet<_> = profiles
        .values()
        .filter_map(|profile| replacement_for(profile))
        .collect();
    for source in &catalog.assets {
        let profile = profiles
            .get(&source.gameplay_profile_id)
            .expect("asset profiles were validated before replacements");
        let Some(replacement_id) = replacement_for(profile) else {
            continue;
        };
        let replacement = catalog
            .asset(replacement_id)
            .ok_or_else(|| "replacement references an unknown asset".to_string())?;
        let replacement_profile = profiles
            .get(&replacement.gameplay_profile_id)
            .ok_or_else(|| "replacement profile disappeared".to_string())?;
        if source.slot != replacement.slot
            || source.footprint_cells != replacement.footprint_cells
            || replacement.parameter_kind != MapPlacementParameterKind::None
            || replacement_profile.destruction != MapDestructionBehavior::Indestructible
            || replacement_profile.durability != MapDurabilityBehavior::Indestructible
            || source.allowed_surface_tags != replacement.allowed_surface_tags
            || (profile.player_collision == PlayerCollision::Pass
                && replacement_profile.player_collision == PlayerCollision::Block)
            || (profile.projectile_collision == ProjectileCollision::Pass
                && replacement_profile.projectile_collision == ProjectileCollision::BlockAndConsume)
            || ((replacement_profile.player_collision == PlayerCollision::Block
                || replacement_profile.projectile_collision
                    == ProjectileCollision::BlockAndConsume)
                && profile.collider_shape != replacement_profile.collider_shape)
        {
            return Err("replacement is incompatible with its source asset".to_string());
        }
    }
    let terminal_is_authored = terminal_assets.iter().any(|id| {
        catalog.presets.iter().any(|preset| {
            preset
                .recipe
                .placements
                .iter()
                .any(|placement| placement.asset_id == *id)
                || preset
                    .recipe
                    .filled_rects
                    .iter()
                    .any(|rect| rect.asset_id == *id)
        })
    });
    if terminal_is_authored {
        Err("terminal replacement assets cannot be authored directly".to_string())
    } else {
        Ok(())
    }
}

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ResolvedMapSnapshot {
    pub identity: ResolvedMapIdentity,
    pub catalog_schema_version: u16,
    pub recipe_schema_version: u16,
    pub presentation_theme_id: MapPresentationThemeId,
    pub mode_definition_id: ModeDefinitionId,
    pub dimensions: MapDimensions,
    pub default_surface_asset_id: MapAssetId,
    pub placements: Vec<MapAssetPlacement>,
    pub mode_anchors: Vec<MapModeAnchorPlacement>,
}

#[derive(Component, Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MapDynamicState {
    pub map_instance_id: MapInstanceId,
    pub generation: u64,
    pub revision: u64,
    pub terminal_states: Vec<MapPlacementTransition>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapDynamicGeneration {
    pub map_instance_id: MapInstanceId,
    pub generation: u64,
}

impl MapDynamicState {
    #[must_use]
    pub const fn generation_id(&self) -> MapDynamicGeneration {
        MapDynamicGeneration {
            map_instance_id: self.map_instance_id,
            generation: self.generation,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MapMutationEvent {
    pub generation: MapDynamicGeneration,
    pub revision: u64,
    pub transitions: Vec<MapPlacementTransition>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub enum MapPlacementOutcome {
    Removed,
    ReplacedWith(MapAssetId),
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub struct MapPlacementTransition {
    pub placement_id: MapPlacementId,
    pub outcome: MapPlacementOutcome,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapDynamicResetEvent {
    pub previous_generation: MapDynamicGeneration,
    pub next_generation: MapDynamicGeneration,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct MapDynamicRecoveryRequest {
    pub generation: MapDynamicGeneration,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct MapDynamicRecoverySnapshot {
    pub state: MapDynamicState,
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct ResolvedMap {
    pub snapshot: ResolvedMapSnapshot,
    pub spawn_points_by_team: BTreeMap<u8, Vec<TeamSpawnPoint>>,
    pub static_colliders: Vec<ResolvedMapCollider>,
    pub dynamic_placements: Vec<MapAssetPlacement>,
    pub player_only_surface_rects: Vec<MapCellRect>,
    pub objective_zone: Option<ResolvedMapObjective>,
    pub heist_safes: Vec<ResolvedHeistSafeAnchor>,
    pub effect_tiles: Vec<ResolvedEffectTile>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedMapCollider {
    pub placement_id: MapPlacementId,
    pub position: Vec2,
    pub shape: MapShape,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedMapObjective {
    pub anchor_id: ModeAnchorId,
    pub area: super::NormalizedArea,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ResolvedHeistSafeAnchor {
    pub placement_id: MapPlacementId,
    pub anchor_id: ModeAnchorId,
    pub defending_team: crate::combat::TeamId,
    pub center: Vec2,
    pub half_extents: Vec2,
    pub quarter_turns: u8,
    pub objective_visual_profile_id: MapVisualProfileId,
}

#[allow(
    clippy::too_many_lines,
    reason = "canonical resolution keeps validation and fingerprint construction in one auditable transaction"
)]
fn resolve_grid_recipe(
    recipe: &MapRecipe,
    preset_id: MapPresetId,
    instance_id: MapInstanceId,
    catalog: &MapContentCatalog,
) -> Result<ResolvedMap, String> {
    if instance_id.0 == 0
        || recipe.recipe_id.0 == 0
        || recipe.revision == 0
        || recipe.recipe_version != MAP_RECIPE_SCHEMA_VERSION
        || !matches!(
            recipe.mode_definition_id,
            WIPEOUT_MODE_DEFINITION | HOT_ZONE_MODE_DEFINITION | HEIST_MODE_DEFINITION
        )
    {
        return Err("invalid grid map recipe identity or mode".to_string());
    }
    recipe.dimensions.validate()?;
    let default_surface = catalog
        .asset(recipe.default_surface_asset_id)
        .ok_or_else(|| "unknown default surface asset".to_string())?;
    if default_surface.slot != MapAssetSlot::Surface {
        return Err("default surface must use the surface slot".to_string());
    }
    let default_tag = default_surface
        .surface_tag
        .ok_or_else(|| "default surface must declare a surface tag".to_string())?;
    let mut placements = recipe.placements.clone();
    for rect in &recipe.filled_rects {
        if rect.width == 0 || rect.height == 0 {
            return Err("filled rectangle dimensions must be positive".to_string());
        }
        let count = u32::from(rect.width)
            .checked_mul(u32::from(rect.height))
            .ok_or_else(|| "filled rectangle count overflow".to_string())?;
        for offset in 0..count {
            let x = u32::from(rect.min.x) + offset % u32::from(rect.width);
            let y = u32::from(rect.min.y) + offset / u32::from(rect.width);
            placements.push(MapAssetPlacement {
                placement_id: MapPlacementId(
                    rect.first_placement_id
                        .0
                        .checked_add(offset)
                        .ok_or_else(|| "filled rectangle placement ID overflow".to_string())?,
                ),
                cell: MapCell::new(
                    u16::try_from(x).map_err(|_| "filled rectangle x overflow")?,
                    u16::try_from(y).map_err(|_| "filled rectangle y overflow")?,
                ),
                asset_id: rect.asset_id,
                quarter_turns: rect.quarter_turns,
                parameters: MapPlacementParameters::None,
            });
        }
    }
    let concealment_placement_count = placements
        .iter()
        .filter(|placement| {
            catalog
                .asset(placement.asset_id)
                .and_then(|asset| catalog.profile(asset.gameplay_profile_id))
                .is_some_and(|profile| profile.concealment == MapConcealmentBehavior::HideOccupants)
        })
        .count();
    let effect_tile_placement_count = placements
        .iter()
        .filter(|placement| {
            catalog
                .asset(placement.asset_id)
                .and_then(|asset| catalog.profile(asset.gameplay_profile_id))
                .is_some_and(|profile| profile.effect_tile != MapEffectTileBehavior::None)
        })
        .count();
    if effect_tile_placement_count > super::MAX_EFFECT_TILE_PLACEMENTS {
        return Err("map recipe exceeds the effect-tile ceiling".to_string());
    }
    validate_placement_capacity(
        recipe.dimensions,
        placements.len(),
        concealment_placement_count,
    )?;
    let mut ids = BTreeSet::new();
    let mut occupied: BTreeMap<(MapCell, MapAssetSlot), MapPlacementId> = BTreeMap::new();
    let mut spawn_ordinals = BTreeSet::new();
    for placement in &placements {
        let asset = catalog
            .asset(placement.asset_id)
            .ok_or_else(|| "placement references an unknown asset".to_string())?;
        if placement.placement_id.0 == 0
            || !ids.insert(placement.placement_id)
            || placement.quarter_turns > 3
            || asset.allowed_quarter_turns & (1 << placement.quarter_turns) == 0
        {
            return Err("invalid, duplicate, out-of-bounds, or conflicting placement".to_string());
        }
        let cells = placement_cells(recipe.dimensions, asset, placement).ok_or_else(|| {
            "invalid, duplicate, out-of-bounds, or conflicting placement".to_string()
        })?;
        for cell in cells {
            if occupied
                .insert((cell, asset.slot), placement.placement_id)
                .is_some()
            {
                return Err(
                    "invalid, duplicate, out-of-bounds, or conflicting placement".to_string(),
                );
            }
        }
        match (asset.parameter_kind, placement.parameters) {
            (MapPlacementParameterKind::None, MapPlacementParameters::None) => {}
            (
                MapPlacementParameterKind::PlayerSpawn,
                MapPlacementParameters::PlayerSpawn {
                    team_slot,
                    ordinal,
                    facing_quarter_turns,
                },
            ) if team_slot <= 1
                && ordinal > 0
                && facing_quarter_turns <= 3
                && spawn_ordinals.insert((team_slot, ordinal)) => {}
            _ => return Err("placement parameters do not match the asset contract".to_string()),
        }
    }
    let mut effective_surface_tags = BTreeMap::new();
    for placement in &placements {
        let asset = catalog
            .asset(placement.asset_id)
            .ok_or_else(|| "placement asset disappeared".to_string())?;
        if asset.slot == MapAssetSlot::Surface {
            let tag = asset
                .surface_tag
                .ok_or_else(|| "surface placement has no surface tag".to_string())?;
            effective_surface_tags.insert(placement.cell, tag);
        }
    }
    for placement in &placements {
        let asset = catalog
            .asset(placement.asset_id)
            .ok_or_else(|| "placement asset disappeared".to_string())?;
        if asset.slot == MapAssetSlot::Surface || asset.allowed_surface_tags.is_empty() {
            continue;
        }
        let cells = placement_cells(recipe.dimensions, asset, placement)
            .ok_or_else(|| "placement footprint disappeared".to_string())?;
        if cells.into_iter().any(|cell| {
            !asset
                .allowed_surface_tags
                .contains(effective_surface_tags.get(&cell).unwrap_or(&default_tag))
        }) {
            return Err("placement is incompatible with the effective surface".to_string());
        }
    }
    placements.sort_by_key(|placement| {
        let slot = catalog
            .asset(placement.asset_id)
            .map_or(MapAssetSlot::Marker, |asset| asset.slot);
        (
            placement.cell.y,
            placement.cell.x,
            slot,
            placement.asset_id,
            placement.placement_id,
        )
    });
    let mut mode_anchors = recipe.mode_anchors.clone();
    mode_anchors.sort_by_key(|anchor| (anchor.placement_id, anchor.anchor_id));
    let (objective_zone, heist_safes) = validate_and_resolve_mode_anchors(
        recipe.mode_definition_id,
        recipe.dimensions,
        &mode_anchors,
        &mut ids,
    )?;
    for team in 0..=1 {
        if placements
            .iter()
            .filter(|placement| {
                matches!(
                    placement.parameters,
                    MapPlacementParameters::PlayerSpawn { team_slot, .. } if team_slot == team
                )
            })
            .count()
            < 3
        {
            return Err("grid map lacks spawn capacity".to_string());
        }
    }
    validate_spawn_clearance(&placements, recipe.dimensions, catalog)?;
    validate_effect_tile_spawn_safety(&placements, catalog)?;
    validate_fighter_navigation(&placements, recipe.dimensions, catalog)?;
    if recipe.mode_definition_id == HEIST_MODE_DEFINITION {
        validate_heist_map_access(&placements, &mode_anchors, recipe.dimensions, catalog)?;
    }
    let fingerprint_material = postcard::to_allocvec(&(
        MAP_FINGERPRINT_FORMAT_VERSION,
        MAP_RECIPE_SCHEMA_VERSION,
        recipe.recipe_id,
        recipe.revision,
        recipe.mode_definition_id,
        recipe.presentation_theme_id,
        recipe.dimensions,
        recipe.default_surface_asset_id,
        &placements,
        &mode_anchors,
    ))
    .map_err(|error| format!("map recipe fingerprint serialization failed: {error}"))?;
    let identity = ResolvedMapIdentity {
        instance_id,
        source_preset_id: Some(preset_id),
        recipe_id: recipe.recipe_id,
        recipe_revision: recipe.revision,
        recipe_fingerprint: MapRecipeFingerprint(crate::content::fnv1a64(&fingerprint_material)),
    };
    let snapshot = ResolvedMapSnapshot {
        identity,
        catalog_schema_version: catalog.schema_version,
        recipe_schema_version: recipe.recipe_version,
        presentation_theme_id: recipe.presentation_theme_id,
        mode_definition_id: recipe.mode_definition_id,
        dimensions: recipe.dimensions,
        default_surface_asset_id: recipe.default_surface_asset_id,
        placements: placements.clone(),
        mode_anchors,
    };
    let bytes = postcard::to_allocvec(&snapshot)
        .map_err(|error| format!("map snapshot serialization failed: {error}"))?;
    if bytes.len() > MAX_RESOLVED_MAP_SNAPSHOT_BYTES {
        return Err("map snapshot exceeds the byte ceiling".to_string());
    }
    let damageable_count = placements
        .iter()
        .filter(|placement| {
            catalog
                .asset(placement.asset_id)
                .and_then(|asset| catalog.profile(asset.gameplay_profile_id))
                .is_some_and(|profile| {
                    matches!(profile.durability, MapDurabilityBehavior::HitPoints(_))
                })
        })
        .count();
    if damageable_count > super::MAX_DAMAGEABLE_MAP_OBJECTS {
        return Err("map recipe exceeds the damageable-object ceiling".to_string());
    }
    let (spawn_points_by_team, static_colliders) = derive_runtime_facts(&snapshot, catalog)?;
    let dynamic_placements: Vec<_> = placements
        .iter()
        .filter(|placement| {
            catalog
                .asset(placement.asset_id)
                .and_then(|asset| catalog.profile(asset.gameplay_profile_id))
                .is_some_and(|profile| {
                    profile.destruction != MapDestructionBehavior::Indestructible
                        || profile.durability != MapDurabilityBehavior::Indestructible
                })
        })
        .cloned()
        .collect();
    let player_only_surface_cells: BTreeSet<_> = placements
        .iter()
        .filter(|placement| {
            catalog
                .asset(placement.asset_id)
                .and_then(|asset| catalog.profile(asset.gameplay_profile_id))
                .is_some_and(|profile| {
                    profile.player_collision == PlayerCollision::Block
                        && profile.projectile_collision == ProjectileCollision::Pass
                })
        })
        .flat_map(|placement| {
            let asset = catalog
                .asset(placement.asset_id)
                .expect("resolved placement asset exists");
            placement_cells(recipe.dimensions, asset, placement).unwrap_or_default()
        })
        .collect();
    let player_only_surface_rects = merge_cells_to_rectangles(player_only_surface_cells);
    let effect_tiles = placements
        .iter()
        .filter_map(|placement| {
            let behavior = catalog
                .asset(placement.asset_id)
                .and_then(|asset| catalog.profile(asset.gameplay_profile_id))?
                .effect_tile;
            behavior.kind().map(|_| ResolvedEffectTile {
                placement_id: placement.placement_id,
                cell: placement.cell,
                behavior,
            })
        })
        .collect();
    Ok(ResolvedMap {
        snapshot,
        spawn_points_by_team,
        static_colliders,
        dynamic_placements,
        player_only_surface_rects,
        objective_zone,
        heist_safes,
        effect_tiles,
    })
}

fn validate_placement_capacity(
    dimensions: MapDimensions,
    placement_count: usize,
    concealment_placement_count: usize,
) -> Result<(), String> {
    if placement_count > dimensions.placement_capacity() {
        return Err("grid map exceeds the per-cell asset-slot capacity".to_string());
    }
    if concealment_placement_count > dimensions.cell_count() {
        return Err("grid map exceeds one concealment feature per cell".to_string());
    }
    Ok(())
}

fn validate_effect_tile_spawn_safety(
    placements: &[MapAssetPlacement],
    catalog: &MapContentCatalog,
) -> Result<(), String> {
    let spawn_cells: Vec<_> = placements
        .iter()
        .filter(|placement| {
            matches!(
                placement.parameters,
                MapPlacementParameters::PlayerSpawn { .. }
            )
        })
        .map(|placement| placement.cell)
        .collect();
    for placement in placements {
        let behavior = catalog
            .asset(placement.asset_id)
            .and_then(|asset| catalog.profile(asset.gameplay_profile_id))
            .map_or(MapEffectTileBehavior::None, |profile| profile.effect_tile);
        if behavior == MapEffectTileBehavior::None {
            continue;
        }
        for spawn in &spawn_cells {
            let dx = placement.cell.x.abs_diff(spawn.x);
            let dy = placement.cell.y.abs_diff(spawn.y);
            if placement.cell == *spawn
                || (matches!(behavior, MapEffectTileBehavior::Damage { .. }) && dx <= 1 && dy <= 1)
            {
                return Err("effect tile violates spawn safety clearance".to_string());
            }
        }
    }
    Ok(())
}

fn validate_and_resolve_mode_anchors(
    mode: ModeDefinitionId,
    dimensions: MapDimensions,
    anchors: &[MapModeAnchorPlacement],
    placement_ids: &mut BTreeSet<MapPlacementId>,
) -> Result<(Option<ResolvedMapObjective>, Vec<ResolvedHeistSafeAnchor>), String> {
    if mode == WIPEOUT_MODE_DEFINITION {
        return anchors
            .is_empty()
            .then_some((None, Vec::new()))
            .ok_or_else(|| "Wipeout maps cannot contain mode anchors".to_string());
    }
    if mode == HEIST_MODE_DEFINITION {
        return resolve_heist_safe_anchors(dimensions, anchors, placement_ids)
            .map(|resolved| (None, resolved));
    }
    if mode != HOT_ZONE_MODE_DEFINITION || anchors.len() != 1 {
        return Err("Hot Zone maps require exactly one objective anchor".to_string());
    }
    let anchor = anchors[0];
    if anchor.placement_id.0 == 0
        || anchor.anchor_id.0 == 0
        || !placement_ids.insert(anchor.placement_id)
    {
        return Err("invalid or duplicate mode anchor identity".to_string());
    }
    let MapModeAnchorKind::HotZoneCircle {
        center_half_cell,
        radius_half_cells,
    } = anchor.kind
    else {
        return Err("Hot Zone maps cannot contain non-zone anchors".to_string());
    };
    if radius_half_cells == 0 || radius_half_cells > 64 {
        return Err("invalid Hot Zone objective radius".to_string());
    }
    let center = dimensions
        .half_cell_world(center_half_cell)
        .ok_or_else(|| "Hot Zone objective center is out of bounds".to_string())?;
    let radius = f32::from(radius_half_cells) * (MAP_CELL_SIZE_WORLD * 0.5);
    let bounds = dimensions.bounds();
    if center.x - radius < bounds.min.x
        || center.x + radius > bounds.max.x
        || center.y - radius < bounds.min.y
        || center.y + radius > bounds.max.y
    {
        return Err("Hot Zone objective does not fit playable bounds".to_string());
    }
    Ok((
        Some(ResolvedMapObjective {
            anchor_id: anchor.anchor_id,
            area: super::NormalizedArea {
                center,
                shape: MapShape::Circle { radius },
            },
        }),
        Vec::new(),
    ))
}

fn resolve_heist_safe_anchors(
    dimensions: MapDimensions,
    anchors: &[MapModeAnchorPlacement],
    placement_ids: &mut BTreeSet<MapPlacementId>,
) -> Result<Vec<ResolvedHeistSafeAnchor>, String> {
    if anchors.len() != 2 {
        return Err("Heist maps require exactly two safe anchors".to_string());
    }
    let mut teams = BTreeSet::new();
    let mut resolved = Vec::with_capacity(2);
    for anchor in anchors {
        if anchor.placement_id.0 == 0
            || anchor.anchor_id.0 == 0
            || !placement_ids.insert(anchor.placement_id)
        {
            return Err("invalid or duplicate mode anchor identity".to_string());
        }
        let MapModeAnchorKind::HeistSafe {
            team_slot,
            origin_cell,
            width_cells,
            height_cells,
            quarter_turns,
            objective_visual_profile_id,
        } = anchor.kind
        else {
            return Err("Heist maps cannot contain non-safe anchors".to_string());
        };
        if team_slot > 1
            || !teams.insert(team_slot)
            || width_cells != 3
            || height_cells != 2
            || quarter_turns > 3
            || objective_visual_profile_id != HEIST_SAFE_VISUAL_PROFILE
            || origin_cell.x < 2
            || origin_cell.y < 2
            || origin_cell.x.saturating_add(width_cells).saturating_add(2) > dimensions.width
            || origin_cell.y.saturating_add(height_cells).saturating_add(2) > dimensions.height
        {
            return Err("invalid Heist safe anchor topology".to_string());
        }
        let size = Vec2::new(f32::from(width_cells), f32::from(height_cells)) * MAP_CELL_SIZE_WORLD;
        resolved.push(ResolvedHeistSafeAnchor {
            placement_id: anchor.placement_id,
            anchor_id: anchor.anchor_id,
            defending_team: crate::combat::TeamId(team_slot),
            center: dimensions.cell_min(origin_cell) + size * 0.5,
            half_extents: size * 0.5,
            quarter_turns,
            objective_visual_profile_id,
        });
    }
    resolved.sort_by_key(|safe| safe.defending_team);
    Ok(resolved)
}

#[derive(Clone, Copy)]
enum DerivedColliderShape {
    Rectangle { center: Vec2, half_extents: Vec2 },
    Circle { center: Vec2, radius: f32 },
}

fn placement_collider_shape(
    dimensions: MapDimensions,
    asset: &MapAssetDefinition,
    placement: &MapAssetPlacement,
    profile: MapGameplayProfile,
) -> Option<DerivedColliderShape> {
    let center = placement_world_center(dimensions, asset, placement);
    match profile.collider_shape {
        MapColliderShape::None => None,
        MapColliderShape::FootprintRectangle => {
            let footprint = asset.footprint_cells.rotated(placement.quarter_turns);
            Some(DerivedColliderShape::Rectangle {
                center,
                half_extents: Vec2::new(f32::from(footprint.width), f32::from(footprint.height))
                    * (MAP_CELL_SIZE_WORLD * 0.5),
            })
        }
        MapColliderShape::Circle { radius_world_units } => Some(DerivedColliderShape::Circle {
            center,
            radius: f32::from(radius_world_units),
        }),
    }
}

fn circle_overlaps_derived_shape(center: Vec2, radius: f32, shape: DerivedColliderShape) -> bool {
    match shape {
        DerivedColliderShape::Rectangle {
            center: obstacle_center,
            half_extents,
        } => {
            let min = obstacle_center - half_extents;
            let max = obstacle_center + half_extents;
            center.distance_squared(center.clamp(min, max)) < radius * radius
        }
        DerivedColliderShape::Circle {
            center: obstacle_center,
            radius: obstacle_radius,
        } => center.distance_squared(obstacle_center) < (radius + obstacle_radius).powi(2),
    }
}

/// Whether the current effective map state blocks a fighter-sized circle at `center`.
#[must_use]
pub fn circle_overlaps_blocking_map(
    center: Vec2,
    radius: f32,
    snapshot: &ResolvedMapSnapshot,
    state: &MapDynamicState,
    catalog: &MapContentCatalog,
) -> bool {
    snapshot.placements.iter().any(|placement| {
        effective_blocking_shape(placement, snapshot, state, catalog)
            .is_some_and(|shape| circle_overlaps_derived_shape(center, radius, shape))
    })
}

/// Clamp and relax a circle against the effective current map blockers.
#[must_use]
pub fn resolve_circle_against_blocking_map(
    position: Vec2,
    radius: f32,
    snapshot: &ResolvedMapSnapshot,
    state: &MapDynamicState,
    catalog: &MapContentCatalog,
) -> Vec2 {
    let bounds = snapshot.dimensions.bounds();
    let mut resolved = bounds.clamp_circle(position, radius);
    for _ in 0..2 {
        for placement in &snapshot.placements {
            if let Some(shape) = effective_blocking_shape(placement, snapshot, state, catalog) {
                resolved = push_circle_out_of_shape(resolved, radius, shape);
            }
        }
        resolved = bounds.clamp_circle(resolved, radius);
    }
    resolved
}

fn effective_blocking_shape(
    placement: &MapAssetPlacement,
    snapshot: &ResolvedMapSnapshot,
    state: &MapDynamicState,
    catalog: &MapContentCatalog,
) -> Option<DerivedColliderShape> {
    let asset_id = state
        .terminal_states
        .binary_search_by_key(&placement.placement_id, |transition| {
            transition.placement_id
        })
        .ok()
        .map_or(Some(placement.asset_id), |index| {
            match state.terminal_states[index].outcome {
                MapPlacementOutcome::Removed => None,
                MapPlacementOutcome::ReplacedWith(asset_id) => Some(asset_id),
            }
        })?;
    let asset = catalog.asset(asset_id)?;
    let profile = catalog.profile(asset.gameplay_profile_id).copied()?;
    if profile.player_collision != PlayerCollision::Block {
        return None;
    }
    let mut effective = placement.clone();
    effective.asset_id = asset_id;
    placement_collider_shape(snapshot.dimensions, asset, &effective, profile)
}

/// Current projectile-blocking collider for one authored placement.
///
/// This mirrors authoritative dynamic replacement/removal resolution while deliberately using
/// projectile collision policy rather than fighter collision policy. Client aim tracing consumes
/// the result as read-only presentation data; authority continues to use installed Avian bodies.
#[cfg(feature = "client")]
pub(crate) fn effective_projectile_collider(
    placement: &MapAssetPlacement,
    snapshot: &ResolvedMapSnapshot,
    state: &MapDynamicState,
    catalog: &MapContentCatalog,
) -> Option<ResolvedMapCollider> {
    let asset_id = state
        .terminal_states
        .binary_search_by_key(&placement.placement_id, |transition| {
            transition.placement_id
        })
        .ok()
        .map_or(Some(placement.asset_id), |index| {
            match state.terminal_states[index].outcome {
                MapPlacementOutcome::Removed => None,
                MapPlacementOutcome::ReplacedWith(asset_id) => Some(asset_id),
            }
        })?;
    let asset = catalog.asset(asset_id)?;
    let profile = catalog.profile(asset.gameplay_profile_id).copied()?;
    if profile.projectile_collision != ProjectileCollision::BlockAndConsume {
        return None;
    }
    let mut effective = placement.clone();
    effective.asset_id = asset_id;
    let shape = placement_collider_shape(snapshot.dimensions, asset, &effective, profile)?;
    let (position, shape) = match shape {
        DerivedColliderShape::Rectangle {
            center,
            half_extents,
        } => (center, MapShape::Rectangle { half_extents }),
        DerivedColliderShape::Circle { center, radius } => (center, MapShape::Circle { radius }),
    };
    Some(ResolvedMapCollider {
        placement_id: placement.placement_id,
        position,
        shape,
    })
}

fn push_circle_out_of_shape(position: Vec2, radius: f32, shape: DerivedColliderShape) -> Vec2 {
    match shape {
        DerivedColliderShape::Circle {
            center,
            radius: obstacle_radius,
        } => {
            let delta = position - center;
            let distance = delta.length();
            let minimum = radius + obstacle_radius;
            if distance >= minimum {
                position
            } else if distance <= f32::EPSILON {
                center + Vec2::X * minimum
            } else {
                center + delta / distance * minimum
            }
        }
        DerivedColliderShape::Rectangle {
            center,
            half_extents,
        } => {
            let local = position - center;
            let closest = local.clamp(-half_extents, half_extents);
            let offset = local - closest;
            let distance = offset.length();
            if distance >= radius {
                return position;
            }
            if distance > f32::EPSILON {
                return center + closest + offset / distance * radius;
            }
            let exit_x = half_extents.x - local.x.abs();
            let exit_y = half_extents.y - local.y.abs();
            if exit_x <= exit_y {
                let direction = if local.x < 0.0 { -1.0 } else { 1.0 };
                center + Vec2::new(direction * (half_extents.x + radius), local.y)
            } else {
                let direction = if local.y < 0.0 { -1.0 } else { 1.0 };
                center + Vec2::new(local.x, direction * (half_extents.y + radius))
            }
        }
    }
}

fn blocking_shapes(
    placements: &[MapAssetPlacement],
    dimensions: MapDimensions,
    catalog: &MapContentCatalog,
) -> Vec<DerivedColliderShape> {
    placements
        .iter()
        .filter_map(|placement| {
            let asset = catalog.asset(placement.asset_id)?;
            let profile = *catalog.profile(asset.gameplay_profile_id)?;
            (profile.player_collision == PlayerCollision::Block)
                .then(|| placement_collider_shape(dimensions, asset, placement, profile))
                .flatten()
        })
        .collect()
}

fn validate_spawn_clearance(
    placements: &[MapAssetPlacement],
    dimensions: MapDimensions,
    catalog: &MapContentCatalog,
) -> Result<(), String> {
    let blocked = blocking_shapes(placements, dimensions, catalog);
    for placement in placements {
        if matches!(
            placement.parameters,
            MapPlacementParameters::PlayerSpawn { .. }
        ) {
            let center = dimensions.cell_center(placement.cell);
            if blocked.iter().any(|shape| {
                circle_overlaps_derived_shape(
                    center,
                    crate::movement::STANDARD_FIGHTER_RADIUS,
                    *shape,
                )
            }) {
                return Err("spawn overlaps blocking feature".to_string());
            }
            if !dimensions.bounds().contains_with_inset(center, 32.0) {
                return Err("spawn lacks perimeter clearance".to_string());
            }
        }
    }
    Ok(())
}

fn validate_fighter_navigation(
    placements: &[MapAssetPlacement],
    dimensions: MapDimensions,
    catalog: &MapContentCatalog,
) -> Result<(), String> {
    let blocked = blocking_shapes(placements, dimensions, catalog);
    let center_is_clear = |cell: MapCell| {
        let center = dimensions.cell_center(cell);
        dimensions
            .bounds()
            .contains_with_inset(center, crate::movement::STANDARD_FIGHTER_RADIUS)
            && blocked.iter().all(|shape| {
                !circle_overlaps_derived_shape(
                    center,
                    crate::movement::STANDARD_FIGHTER_RADIUS,
                    *shape,
                )
            })
    };
    let spawns: Vec<_> = placements
        .iter()
        .filter(|placement| {
            matches!(
                placement.parameters,
                MapPlacementParameters::PlayerSpawn { .. }
            )
        })
        .map(|placement| placement.cell)
        .collect();
    let Some(start) = spawns.first().copied() else {
        return Err("map has no navigation start".to_string());
    };
    if !center_is_clear(start) {
        return Err("spawn is not fighter-radius navigable".to_string());
    }
    let mut reached = BTreeSet::from([start]);
    let mut frontier = std::collections::VecDeque::from([start]);
    while let Some(cell) = frontier.pop_front() {
        for candidate in [
            cell.y.checked_add(1).map(|y| MapCell::new(cell.x, y)),
            cell.x.checked_add(1).map(|x| MapCell::new(x, cell.y)),
            cell.y.checked_sub(1).map(|y| MapCell::new(cell.x, y)),
            cell.x.checked_sub(1).map(|x| MapCell::new(x, cell.y)),
        ]
        .into_iter()
        .flatten()
        {
            if dimensions.contains(candidate)
                && center_is_clear(candidate)
                && reached.insert(candidate)
            {
                frontier.push_back(candidate);
            }
        }
    }
    if spawns.into_iter().all(|spawn| reached.contains(&spawn)) {
        Ok(())
    } else {
        Err("fighter-radius navigation does not connect every spawn".to_string())
    }
}

#[derive(Clone)]
struct HeistSafeAccessGeometry {
    team_slot: u8,
    footprint: BTreeSet<MapCell>,
    shape: DerivedColliderShape,
    sectors: [Vec<MapCell>; 4],
}

fn heist_safe_access_geometry(
    anchor: &MapModeAnchorPlacement,
    dimensions: MapDimensions,
) -> Result<HeistSafeAccessGeometry, String> {
    let MapModeAnchorKind::HeistSafe {
        team_slot,
        origin_cell,
        width_cells,
        height_cells,
        ..
    } = anchor.kind
    else {
        return Err("Heist maps cannot contain non-safe anchors".to_string());
    };
    let footprint = (0..height_cells)
        .flat_map(|y| {
            (0..width_cells).map(move |x| MapCell::new(origin_cell.x + x, origin_cell.y + y))
        })
        .collect::<BTreeSet<_>>();
    let size = Vec2::new(f32::from(width_cells), f32::from(height_cells)) * MAP_CELL_SIZE_WORLD;
    let left_x = origin_cell
        .x
        .checked_sub(2)
        .ok_or_else(|| "Heist safe lacks a left attack sector".to_string())?;
    let bottom_y = origin_cell
        .y
        .checked_sub(2)
        .ok_or_else(|| "Heist safe lacks a lower attack sector".to_string())?;
    let right_x = origin_cell
        .x
        .checked_add(width_cells)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| "Heist safe right attack sector overflows".to_string())?;
    let top_y = origin_cell
        .y
        .checked_add(height_cells)
        .and_then(|value| value.checked_add(1))
        .ok_or_else(|| "Heist safe upper attack sector overflows".to_string())?;
    let vertical = |x| {
        (0..height_cells)
            .map(|y| MapCell::new(x, origin_cell.y + y))
            .collect::<Vec<_>>()
    };
    let horizontal = |y| {
        (0..width_cells)
            .map(|x| MapCell::new(origin_cell.x + x, y))
            .collect::<Vec<_>>()
    };
    Ok(HeistSafeAccessGeometry {
        team_slot,
        footprint,
        shape: DerivedColliderShape::Rectangle {
            center: dimensions.cell_min(origin_cell) + size * 0.5,
            half_extents: size * 0.5,
        },
        sectors: [
            vertical(left_x),
            vertical(right_x),
            horizontal(bottom_y),
            horizontal(top_y),
        ],
    })
}

fn reachable_clear_cells(
    spawn: MapCell,
    center_is_clear: &impl Fn(MapCell) -> bool,
) -> BTreeSet<MapCell> {
    let mut reached = BTreeSet::from([spawn]);
    let mut frontier = std::collections::VecDeque::from([spawn]);
    while let Some(cell) = frontier.pop_front() {
        for candidate in [
            cell.y.checked_add(1).map(|y| MapCell::new(cell.x, y)),
            cell.x.checked_add(1).map(|x| MapCell::new(x, cell.y)),
            cell.y.checked_sub(1).map(|y| MapCell::new(cell.x, y)),
            cell.x.checked_sub(1).map(|x| MapCell::new(x, cell.y)),
        ]
        .into_iter()
        .flatten()
        {
            if center_is_clear(candidate) && reached.insert(candidate) {
                frontier.push_back(candidate);
            }
        }
    }
    reached
}

fn validate_heist_map_access(
    placements: &[MapAssetPlacement],
    anchors: &[MapModeAnchorPlacement],
    dimensions: MapDimensions,
    catalog: &MapContentCatalog,
) -> Result<(), String> {
    let mut safes = Vec::with_capacity(2);
    for anchor in anchors {
        safes.push(heist_safe_access_geometry(anchor, dimensions)?);
    }
    safes.sort_by_key(|safe| safe.team_slot);
    if safes.len() != 2 || safes[0].team_slot != 0 || safes[1].team_slot != 1 {
        return Err("Heist safe access requires exact team slots 0 and 1".to_string());
    }
    if !safes[0].footprint.is_disjoint(&safes[1].footprint) {
        return Err("Heist safe reservations overlap".to_string());
    }
    for placement in placements {
        let asset = catalog
            .asset(placement.asset_id)
            .ok_or_else(|| "placement references an unknown asset".to_string())?;
        if asset.slot == MapAssetSlot::Surface {
            continue;
        }
        let cells = placement_cells(dimensions, asset, placement)
            .ok_or_else(|| "placement footprint disappeared".to_string())?;
        if safes
            .iter()
            .any(|safe| cells.iter().any(|cell| safe.footprint.contains(cell)))
        {
            return Err("Heist safe reservation overlaps a map placement".to_string());
        }
    }

    let mut blocked = blocking_shapes(placements, dimensions, catalog);
    blocked.extend(safes.iter().map(|safe| safe.shape));
    let center_is_clear = |cell: MapCell| {
        dimensions.contains(cell)
            && dimensions.bounds().contains_with_inset(
                dimensions.cell_center(cell),
                crate::movement::STANDARD_FIGHTER_RADIUS,
            )
            && blocked.iter().all(|shape| {
                !circle_overlaps_derived_shape(
                    dimensions.cell_center(cell),
                    crate::movement::STANDARD_FIGHTER_RADIUS,
                    *shape,
                )
            })
    };
    for safe in &safes {
        let open_sectors = safe
            .sectors
            .iter()
            .filter(|sector| {
                sector
                    .windows(2)
                    .any(|pair| pair.iter().copied().all(&center_is_clear))
            })
            .count();
        if open_sectors < 2 {
            return Err("Heist safe exposes fewer than two legal attack sectors".to_string());
        }
    }

    let spawns = placements
        .iter()
        .filter_map(|placement| match placement.parameters {
            MapPlacementParameters::PlayerSpawn { team_slot, .. } => {
                Some((team_slot, placement.cell))
            }
            MapPlacementParameters::None => None,
        })
        .collect::<Vec<_>>();
    for (spawn_team, spawn) in spawns {
        if !center_is_clear(spawn) {
            return Err("Heist spawn is not fighter-radius navigable".to_string());
        }
        let reached = reachable_clear_cells(spawn, &center_is_clear);
        for safe in &safes {
            let reaches_ring = safe
                .sectors
                .iter()
                .flatten()
                .any(|cell| reached.contains(cell) && center_is_clear(*cell));
            if !reaches_ring {
                let relation = if safe.team_slot == spawn_team {
                    "defence"
                } else {
                    "attack"
                };
                return Err(format!(
                    "Heist spawn cannot reach its required {relation} ring"
                ));
            }
        }
    }
    Ok(())
}

#[allow(
    clippy::too_many_lines,
    reason = "runtime derivation keeps collider merging and spawn indexing in one auditable pass"
)]
fn derive_runtime_facts(
    grid: &ResolvedMapSnapshot,
    catalog: &MapContentCatalog,
) -> Result<ResolvedRuntimeFacts, String> {
    let mut static_colliders = Vec::new();
    let mut spawn_points = Vec::new();
    let blocked_indestructible: BTreeMap<MapAssetId, BTreeSet<MapCell>> = catalog
        .assets
        .iter()
        .filter_map(|asset| {
            let profile = catalog.profile(asset.gameplay_profile_id)?;
            (asset.slot == MapAssetSlot::Feature
                && profile.player_collision == PlayerCollision::Block
                && profile.projectile_collision == ProjectileCollision::BlockAndConsume
                && profile.collider_shape == MapColliderShape::FootprintRectangle
                && profile.destruction == MapDestructionBehavior::Indestructible
                && profile.durability == MapDurabilityBehavior::Indestructible)
                .then_some((
                    asset.id,
                    grid.placements
                        .iter()
                        .filter(|placement| placement.asset_id == asset.id)
                        .flat_map(|placement| {
                            placement_cells(grid.dimensions, asset, placement).unwrap_or_default()
                        })
                        .collect(),
                ))
        })
        .collect();
    for (asset_id, mut cells) in blocked_indestructible {
        for rectangle in merge_cells_to_rectangles(std::mem::take(&mut cells)) {
            let start = rectangle.min;
            let width = rectangle.width;
            let height = rectangle.height;
            let mut placement_id = MapPlacementId(u32::MAX);
            for y in start.y..start.y + height {
                for x in start.x..start.x + width {
                    let cell = MapCell::new(x, y);
                    cells.remove(&cell);
                    if let Some(id) = grid
                        .placements
                        .iter()
                        .find(|placement| placement.asset_id == asset_id && placement.cell == cell)
                        .map(|placement| placement.placement_id)
                    {
                        placement_id = placement_id.min(id);
                    }
                }
            }
            let size = Vec2::new(f32::from(width), f32::from(height)) * MAP_CELL_SIZE_WORLD;
            let position = grid.dimensions.cell_min(start) + size * 0.5;
            static_colliders.push(ResolvedMapCollider {
                placement_id,
                position,
                shape: MapShape::Rectangle {
                    half_extents: size * 0.5,
                },
            });
        }
    }
    for placement in &grid.placements {
        let asset = catalog
            .asset(placement.asset_id)
            .ok_or_else(|| "circular obstacle asset disappeared".to_string())?;
        let profile = catalog
            .profile(asset.gameplay_profile_id)
            .ok_or_else(|| "circular obstacle profile disappeared".to_string())?;
        let MapColliderShape::Circle { radius_world_units } = profile.collider_shape else {
            continue;
        };
        if profile.destruction != MapDestructionBehavior::Indestructible
            || profile.durability != MapDurabilityBehavior::Indestructible
        {
            continue;
        }
        static_colliders.push(ResolvedMapCollider {
            placement_id: placement.placement_id,
            position: placement_world_center(grid.dimensions, asset, placement),
            shape: MapShape::Circle {
                radius: f32::from(radius_world_units),
            },
        });
    }
    static_colliders.sort_by_key(|placement| placement.placement_id);
    for placement in &grid.placements {
        let asset = catalog
            .asset(placement.asset_id)
            .ok_or_else(|| "resolved placement asset disappeared".to_string())?;
        if let (
            MapAssetSlot::Marker,
            MapPlacementParameters::PlayerSpawn {
                team_slot,
                ordinal,
                facing_quarter_turns,
            },
        ) = (asset.slot, placement.parameters)
        {
            spawn_points.push(TeamSpawnPoint {
                placement_id: placement.placement_id,
                spawn_point_id: SpawnPointId(u16::from(ordinal)),
                team_slot,
                position: grid.dimensions.cell_center(placement.cell),
                facing: f32::from(facing_quarter_turns) * core::f32::consts::FRAC_PI_2,
            });
        }
    }
    spawn_points.sort_by_key(|point| (point.team_slot, point.spawn_point_id));
    let mut spawn_points_by_team: BTreeMap<u8, Vec<TeamSpawnPoint>> = BTreeMap::new();
    for point in spawn_points {
        spawn_points_by_team
            .entry(point.team_slot)
            .or_default()
            .push(point);
    }
    Ok((spawn_points_by_team, static_colliders))
}

type ResolvedRuntimeFacts = (BTreeMap<u8, Vec<TeamSpawnPoint>>, Vec<ResolvedMapCollider>);

fn validate_asset_profile(
    asset: &MapAssetDefinition,
    profile: MapGameplayProfile,
) -> Result<(), String> {
    profile.effect_tile.validate()?;
    let inert = profile.player_collision == PlayerCollision::Pass
        && profile.projectile_collision == ProjectileCollision::Pass
        && profile.destruction == MapDestructionBehavior::Indestructible
        && profile.durability == MapDurabilityBehavior::Indestructible
        && profile.collider_shape == MapColliderShape::None;
    let blocks = profile.player_collision == PlayerCollision::Block
        || profile.projectile_collision == ProjectileCollision::BlockAndConsume;
    let concealment_is_consistent = match profile.concealment {
        MapConcealmentBehavior::None => true,
        MapConcealmentBehavior::HideOccupants => {
            asset.slot == MapAssetSlot::Feature
                && profile.player_collision == PlayerCollision::Pass
                && profile.projectile_collision == ProjectileCollision::Pass
                && profile.collider_shape == MapColliderShape::None
                && profile.destruction == MapDestructionBehavior::Indestructible
                && profile.durability == MapDurabilityBehavior::Indestructible
                && profile.interaction == MapInteractionBehavior::None
        }
    };
    let effect_tile_is_consistent = effect_tile_matches_asset(asset, profile, inert);
    let collider_is_consistent = match profile.collider_shape {
        MapColliderShape::None => !blocks,
        MapColliderShape::FootprintRectangle => blocks,
        MapColliderShape::Circle { radius_world_units } => {
            blocks
                && asset.slot == MapAssetSlot::Feature
                && radius_world_units > 0
                && f32::from(radius_world_units)
                    <= f32::from(
                        asset
                            .footprint_cells
                            .width
                            .min(asset.footprint_cells.height),
                    ) * MAP_CELL_SIZE_WORLD
                        * 0.5
        }
    };
    let durability_is_consistent = match profile.durability {
        MapDurabilityBehavior::Indestructible => true,
        MapDurabilityBehavior::HitPoints(_) => {
            asset.slot == MapAssetSlot::Feature
                && profile.destruction == MapDestructionBehavior::Indestructible
                && profile.interaction == MapInteractionBehavior::None
                && profile.concealment == MapConcealmentBehavior::None
                && profile.collider_shape != MapColliderShape::None
                && blocks
        }
    };
    let valid = match asset.slot {
        MapAssetSlot::Surface => {
            profile.destruction == MapDestructionBehavior::Indestructible
                && profile.interaction == MapInteractionBehavior::None
                && asset.surface_tag.is_some()
                && asset.parameter_kind == MapPlacementParameterKind::None
                && asset.footprint_cells
                    == MapFootprint {
                        width: 1,
                        height: 1,
                    }
        }
        MapAssetSlot::Decoration => {
            inert
                && profile.interaction == MapInteractionBehavior::None
                && asset.surface_tag.is_none()
                && asset.parameter_kind == MapPlacementParameterKind::None
                && asset.footprint_cells
                    == MapFootprint {
                        width: 1,
                        height: 1,
                    }
        }
        MapAssetSlot::Marker => {
            inert
                && profile.interaction == MapInteractionBehavior::PlayerSpawn
                && asset.surface_tag.is_none()
                && asset.parameter_kind == MapPlacementParameterKind::PlayerSpawn
                && asset.footprint_cells
                    == MapFootprint {
                        width: 1,
                        height: 1,
                    }
        }
        MapAssetSlot::Feature => {
            profile.interaction == MapInteractionBehavior::None && asset.surface_tag.is_none()
        }
    };
    (valid
        && collider_is_consistent
        && concealment_is_consistent
        && effect_tile_is_consistent
        && durability_is_consistent)
        .then_some(())
        .ok_or_else(|| format!("asset {} contradicts its slot/gameplay profile", asset.key))
}

fn effect_tile_matches_asset(
    asset: &MapAssetDefinition,
    profile: MapGameplayProfile,
    inert: bool,
) -> bool {
    profile.effect_tile == MapEffectTileBehavior::None
        || (asset.slot == MapAssetSlot::Feature
            && inert
            && profile.interaction == MapInteractionBehavior::None
            && profile.concealment == MapConcealmentBehavior::None
            && asset.surface_tag.is_none()
            && asset.parameter_kind == MapPlacementParameterKind::None
            && asset.footprint_cells
                == MapFootprint {
                    width: 1,
                    height: 1,
                })
}

fn validate_damageable_profiles(catalog: &MapContentCatalog) -> Result<(), String> {
    validate_sorted_ids(
        catalog.damage_profiles.iter().map(|profile| profile.id.0),
        "map damage profiles",
    )?;
    validate_sorted_ids(
        catalog
            .explosion_profiles
            .iter()
            .map(|profile| profile.id.0),
        "environment explosion profiles",
    )?;
    for explosion in &catalog.explosion_profiles {
        if explosion.damage == 0
            || explosion.radius_world_units == 0
            || explosion.radius_world_units > 512
            || explosion.maximum_targets == 0
            || explosion.maximum_targets > 16
            || explosion.maximum_chain_reactions == 0
            || explosion.maximum_chain_reactions > 16
        {
            return Err("invalid environment explosion profile".to_string());
        }
    }
    let explosion_ids: BTreeSet<_> = catalog
        .explosion_profiles
        .iter()
        .map(|profile| profile.id)
        .collect();
    for profile in &catalog.damage_profiles {
        let (known_terminal, outcome) = match profile.terminal {
            MapObjectTerminalBehavior::Explode {
                explosion_profile_id,
                outcome,
            } => (explosion_ids.contains(&explosion_profile_id), outcome),
            MapObjectTerminalBehavior::DropPickup {
                pickup_definition_id,
                outcome,
            } => (
                catalog.restoration_pickup(pickup_definition_id).is_some(),
                outcome,
            ),
        };
        if profile.maximum_health == 0
            || profile.maximum_health > MAX_MAP_OBJECT_HEALTH
            || !known_terminal
            || matches!(outcome, MapPlacementOutcome::ReplacedWith(MapAssetId(0)))
        {
            return Err("invalid map damage profile".to_string());
        }
    }
    let damage_ids: BTreeSet<_> = catalog
        .damage_profiles
        .iter()
        .map(|profile| profile.id)
        .collect();
    for profile in &catalog.gameplay_profiles {
        if let MapDurabilityBehavior::HitPoints(id) = profile.durability
            && !damage_ids.contains(&id)
        {
            return Err("map gameplay profile references unknown durability".to_string());
        }
    }
    Ok(())
}

fn validate_restoration_pickups(catalog: &MapContentCatalog) -> Result<(), String> {
    validate_sorted_ids(
        catalog
            .restoration_pickups
            .iter()
            .map(|definition| definition.id.0),
        "restoration pickup definitions",
    )?;
    for definition in &catalog.restoration_pickups {
        if !(1..=1_000).contains(&definition.restoration)
            || !(8..=64).contains(&definition.collection_radius_world_units)
            || !(60..=3_600).contains(&definition.lifetime_ticks)
            || definition.visual_profile_id.0 == 0
        {
            return Err("invalid restoration pickup definition".to_string());
        }
    }
    Ok(())
}

fn validate_sorted_ids(values: impl IntoIterator<Item = u16>, label: &str) -> Result<(), String> {
    let values: Vec<_> = values.into_iter().collect();
    if values.is_empty() || values.contains(&0) || values.windows(2).any(|pair| pair[0] >= pair[1])
    {
        return Err(format!(
            "{label} must be nonempty, nonzero, sorted, and unique"
        ));
    }
    Ok(())
}

fn valid_key(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 48
        && bytes.first().is_some_and(u8::is_ascii_lowercase)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && !value.contains("--")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonical_fighter_fits_a_one_cell_passage_with_safety_margin() {
        let passage_center = Vec2::new(MAP_CELL_SIZE_WORLD * 0.5, 0.0);
        let left_wall = DerivedColliderShape::Rectangle {
            center: Vec2::new(-MAP_CELL_SIZE_WORLD * 0.5, 0.0),
            half_extents: Vec2::splat(MAP_CELL_SIZE_WORLD * 0.5),
        };
        let right_wall = DerivedColliderShape::Rectangle {
            center: Vec2::new(MAP_CELL_SIZE_WORLD * 1.5, 0.0),
            half_extents: Vec2::splat(MAP_CELL_SIZE_WORLD * 0.5),
        };

        for wall in [left_wall, right_wall] {
            assert!(!circle_overlaps_derived_shape(
                passage_center,
                crate::movement::STANDARD_FIGHTER_RADIUS,
                wall
            ));
            assert!(!circle_overlaps_derived_shape(passage_center, 15.0, wall));
            assert!(
                !circle_overlaps_derived_shape(passage_center, 16.0, wall),
                "an exact-cell body is only tangent and has no movement safety margin"
            );
        }
        assert!(
            (MAP_CELL_SIZE_WORLD - crate::movement::STANDARD_FIGHTER_RADIUS * 2.0 - 4.0).abs()
                < f32::EPSILON
        );
    }

    #[test]
    fn feature_yard_variants_share_geometry_and_own_only_legal_mode_anchors() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let wipeout = &catalog.preset(FEATURE_YARD_WIPEOUT_PRESET).unwrap().recipe;
        let hot_zone = &catalog.preset(FEATURE_YARD_HOT_ZONE_PRESET).unwrap().recipe;
        let heist = &catalog.preset(FEATURE_YARD_HEIST_PRESET).unwrap().recipe;

        for variant in [hot_zone, heist] {
            assert_eq!(variant.presentation_theme_id, wipeout.presentation_theme_id);
            assert_eq!(variant.dimensions, wipeout.dimensions);
            assert_eq!(
                variant.default_surface_asset_id,
                wipeout.default_surface_asset_id
            );
            let wipeout_structural: Vec<_> = wipeout
                .placements
                .iter()
                .filter(|placement| {
                    catalog
                        .asset(placement.asset_id)
                        .and_then(|asset| catalog.profile(asset.gameplay_profile_id))
                        .is_some_and(|profile| profile.effect_tile == MapEffectTileBehavior::None)
                })
                .collect();
            assert_eq!(
                variant.placements.iter().collect::<Vec<_>>(),
                wipeout_structural
            );
            assert_eq!(variant.filled_rects, wipeout.filled_rects);
        }
        assert_eq!(wipeout.mode_definition_id, WIPEOUT_MODE_DEFINITION);
        assert!(wipeout.mode_anchors.is_empty());
        let resolved_wipeout = catalog
            .resolve_preset(FEATURE_YARD_WIPEOUT_PRESET, MapInstanceId(1))
            .unwrap();
        assert_eq!(resolved_wipeout.effect_tiles.len(), 92);
        for (kind, expected_count) in [
            (crate::map::EffectTileKind::Speed, 36),
            (crate::map::EffectTileKind::Slow, 36),
            (crate::map::EffectTileKind::Damage, 20),
        ] {
            assert_eq!(
                resolved_wipeout
                    .effect_tiles
                    .iter()
                    .filter(|tile| tile.behavior.kind() == Some(kind))
                    .count(),
                expected_count
            );
        }
        for (kind, min_x, min_y) in [
            (crate::map::EffectTileKind::Speed, 17, 18),
            (crate::map::EffectTileKind::Speed, 44, 18),
            (crate::map::EffectTileKind::Slow, 26, 22),
            (crate::map::EffectTileKind::Slow, 35, 22),
            (crate::map::EffectTileKind::Speed, 4, 12),
            (crate::map::EffectTileKind::Speed, 57, 12),
            (crate::map::EffectTileKind::Slow, 4, 18),
            (crate::map::EffectTileKind::Slow, 57, 18),
            (crate::map::EffectTileKind::Damage, 4, 24),
            (crate::map::EffectTileKind::Damage, 57, 24),
        ] {
            for y in min_y..min_y + 3 {
                for x in min_x..min_x + 3 {
                    assert!(resolved_wipeout.effect_tiles.iter().any(|tile| {
                        tile.behavior.kind() == Some(kind) && tile.cell == MapCell::new(x, y)
                    }));
                }
            }
        }
        for cell in [MapCell::new(31, 19), MapCell::new(32, 20)] {
            assert!(resolved_wipeout.effect_tiles.iter().any(|tile| {
                tile.behavior.kind() == Some(crate::map::EffectTileKind::Damage)
                    && tile.cell == cell
            }));
        }
        assert_eq!(hot_zone.mode_definition_id, HOT_ZONE_MODE_DEFINITION);
        assert_eq!(hot_zone.mode_anchors.len(), 1);
        assert!(matches!(
            hot_zone.mode_anchors[0].kind,
            MapModeAnchorKind::HotZoneCircle { .. }
        ));
        assert_eq!(heist.mode_definition_id, HEIST_MODE_DEFINITION);
        assert_eq!(heist.mode_anchors.len(), 2);
        assert!(
            heist
                .mode_anchors
                .iter()
                .all(|anchor| matches!(anchor.kind, MapModeAnchorKind::HeistSafe { .. }))
        );

        for preset in [
            FEATURE_YARD_WIPEOUT_PRESET,
            FEATURE_YARD_HOT_ZONE_PRESET,
            FEATURE_YARD_HEIST_PRESET,
        ] {
            let resolved = catalog.resolve_preset(preset, MapInstanceId(1)).unwrap();
            assert!(
                resolved
                    .spawn_points_by_team
                    .values()
                    .all(|spawns| spawns.len() == 3)
            );
        }
    }

    #[test]
    fn feature_yard_contains_every_completed_map_capability_with_bounded_terminal_states() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let resolved = catalog
            .resolve_preset(FEATURE_YARD_WIPEOUT_PRESET, MapInstanceId(1))
            .unwrap();
        let count = |asset_id| {
            resolved
                .snapshot
                .placements
                .iter()
                .filter(|placement| placement.asset_id == asset_id)
                .count()
        };
        assert_eq!(count(WATER_ASSET), 8);
        assert_eq!(count(TALL_GRASS_ASSET), 8);
        assert_eq!(count(BREAKABLE_BARRIER_ASSET), 4);
        assert_eq!(count(DESTRUCTIBLE_COVER_ASSET), 4);
        assert_eq!(count(OIL_BARREL_ASSET), 4);
        assert_eq!(count(TREASURE_CHEST_ASSET), 2);
        assert_eq!(count(PLAYER_SPAWN_ASSET), 6);
        assert_eq!(resolved.dynamic_placements.len(), 14);
        assert_eq!(
            resolved
                .player_only_surface_rects
                .iter()
                .map(|rectangle| usize::from(rectangle.width) * usize::from(rectangle.height))
                .sum::<usize>(),
            8
        );

        for placement in &resolved.dynamic_placements {
            let asset = catalog.asset(placement.asset_id).unwrap();
            let profile = catalog.profile(asset.gameplay_profile_id).unwrap();
            let terminal_asset = match profile.durability {
                MapDurabilityBehavior::HitPoints(id) => {
                    match catalog.damage_profile(id).unwrap().terminal {
                        MapObjectTerminalBehavior::Explode {
                            outcome: MapPlacementOutcome::ReplacedWith(id),
                            ..
                        }
                        | MapObjectTerminalBehavior::DropPickup {
                            outcome: MapPlacementOutcome::ReplacedWith(id),
                            ..
                        } => Some(id),
                        MapObjectTerminalBehavior::Explode {
                            outcome: MapPlacementOutcome::Removed,
                            ..
                        }
                        | MapObjectTerminalBehavior::DropPickup {
                            outcome: MapPlacementOutcome::Removed,
                            ..
                        } => None,
                    }
                }
                MapDurabilityBehavior::Indestructible => match profile.destruction {
                    MapDestructionBehavior::ReplaceOnMapDestruction(id) => Some(id),
                    MapDestructionBehavior::RemoveOnMapDestruction => None,
                    MapDestructionBehavior::Indestructible => {
                        panic!("resolved dynamic placement must terminate")
                    }
                },
            };
            if let Some(terminal_asset) = terminal_asset {
                let replacement = catalog.asset(terminal_asset).unwrap();
                let replacement_profile = catalog.profile(replacement.gameplay_profile_id).unwrap();
                assert_eq!(replacement_profile.player_collision, PlayerCollision::Pass);
            }
        }
    }

    #[test]
    fn crossroads_grid_resolves_exact_structural_bounds_and_counts() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let resolved = catalog
            .resolve_preset(CROSSROADS_PRESET, MapInstanceId(7))
            .unwrap();
        assert_eq!(
            resolved.snapshot.dimensions,
            MapDimensions {
                width: 56,
                height: 36
            }
        );
        assert_eq!(
            resolved.snapshot.dimensions.bounds().min,
            Vec2::new(-896.0, -576.0)
        );
        assert_eq!(resolved.static_colliders.len(), 6);
        assert_eq!(
            resolved
                .spawn_points_by_team
                .values()
                .map(Vec::len)
                .sum::<usize>(),
            8
        );
        assert_eq!(resolved.dynamic_placements.len(), 36);
        assert_eq!(
            resolved
                .dynamic_placements
                .iter()
                .map(|placement| placement.placement_id.0)
                .collect::<Vec<_>>(),
            (100..=135).collect::<Vec<_>>()
        );
        let wall_bounds: Vec<_> = resolved
            .static_colliders
            .iter()
            .map(|wall| (wall.position, wall.shape))
            .collect();
        assert_eq!(
            wall_bounds,
            vec![
                (
                    Vec2::new(0.0, -256.0),
                    MapShape::Rectangle {
                        half_extents: Vec2::new(160.0, 32.0)
                    }
                ),
                (
                    Vec2::new(0.0, 256.0),
                    MapShape::Rectangle {
                        half_extents: Vec2::new(160.0, 32.0)
                    }
                ),
                (
                    Vec2::new(-384.0, 0.0),
                    MapShape::Rectangle {
                        half_extents: Vec2::new(32.0, 128.0)
                    }
                ),
                (
                    Vec2::new(384.0, 0.0),
                    MapShape::Rectangle {
                        half_extents: Vec2::new(32.0, 128.0)
                    }
                ),
                (
                    Vec2::new(-576.0, 0.0),
                    MapShape::Rectangle {
                        half_extents: Vec2::new(32.0, 96.0)
                    }
                ),
                (
                    Vec2::new(576.0, 0.0),
                    MapShape::Rectangle {
                        half_extents: Vec2::new(32.0, 96.0)
                    }
                ),
            ]
        );
    }

    #[test]
    fn even_grid_uses_cell_centers_for_shifted_points() {
        let dimensions = MapDimensions {
            width: 56,
            height: 36,
        };
        assert_eq!(
            dimensions.cell_center(MapCell::new(4, 9)),
            Vec2::new(-752.0, -272.0)
        );
        assert_eq!(
            dimensions.cell_center(MapCell::new(51, 26)),
            Vec2::new(752.0, 272.0)
        );
    }

    #[test]
    fn half_cell_points_represent_odd_grid_centers_exactly() {
        let dimensions = MapDimensions {
            width: 25,
            height: 37,
        };

        assert_eq!(
            dimensions.half_cell_world(MapHalfCellPoint { x: 25, y: 37 }),
            Some(Vec2::ZERO)
        );
        assert_eq!(
            dimensions.half_cell_world(MapHalfCellPoint { x: 25, y: 37 }),
            Some(dimensions.cell_center(MapCell::new(12, 18)))
        );
        assert_eq!(
            dimensions.half_cell_world(MapHalfCellPoint { x: 50, y: 74 }),
            Some(dimensions.bounds().max)
        );
        assert_eq!(
            dimensions.half_cell_world(MapHalfCellPoint { x: 51, y: 37 }),
            None
        );
    }

    #[test]
    fn hot_zone_anchor_resolves_half_cell_center_and_radius() {
        let anchors = [MapModeAnchorPlacement {
            placement_id: MapPlacementId(1),
            anchor_id: ModeAnchorId(1),
            kind: MapModeAnchorKind::HotZoneCircle {
                center_half_cell: MapHalfCellPoint { x: 25, y: 37 },
                radius_half_cells: 7,
            },
        }];
        let (objective, heist_safes) = validate_and_resolve_mode_anchors(
            HOT_ZONE_MODE_DEFINITION,
            MapDimensions {
                width: 25,
                height: 37,
            },
            &anchors,
            &mut BTreeSet::new(),
        )
        .unwrap();

        assert!(heist_safes.is_empty());
        assert_eq!(
            objective.unwrap().area,
            crate::map::NormalizedArea {
                center: Vec2::ZERO,
                shape: MapShape::Circle { radius: 112.0 },
            }
        );
    }

    #[test]
    fn proper_three_vs_three_maps_resolve_exact_mode_topology() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let wipeout = catalog
            .resolve_preset(VERDANT_CROSSFIRE_PRESET, MapInstanceId(10))
            .unwrap();
        let hot_zone = catalog
            .resolve_preset(SWITCHBACK_BASIN_PRESET, MapInstanceId(11))
            .unwrap();
        let heist = catalog
            .resolve_preset(POWDERLINE_VAULT_PRESET, MapInstanceId(12))
            .unwrap();

        for resolved in [&wipeout, &hot_zone, &heist] {
            assert_eq!(
                resolved.snapshot.dimensions,
                MapDimensions {
                    width: 25,
                    height: 37,
                }
            );
            assert_eq!(resolved.spawn_points_by_team.len(), 2);
            assert!(
                resolved
                    .spawn_points_by_team
                    .values()
                    .all(|spawns| spawns.len() == 3)
            );
            assert!(
                resolved
                    .snapshot
                    .placements
                    .iter()
                    .any(|placement| placement.asset_id == TALL_GRASS_ASSET)
            );
        }

        assert_eq!(wipeout.snapshot.mode_definition_id, WIPEOUT_MODE_DEFINITION);
        assert_eq!(wipeout.snapshot.identity.recipe_revision, 2);
        assert!(wipeout.objective_zone.is_none());
        assert!(wipeout.heist_safes.is_empty());

        assert_eq!(
            hot_zone.snapshot.mode_definition_id,
            HOT_ZONE_MODE_DEFINITION
        );
        assert_eq!(hot_zone.snapshot.identity.recipe_revision, 2);
        assert_eq!(
            hot_zone.objective_zone.unwrap().area,
            crate::map::NormalizedArea {
                center: Vec2::ZERO,
                shape: MapShape::Circle { radius: 112.0 },
            }
        );
        assert!(hot_zone.heist_safes.is_empty());

        assert_eq!(heist.snapshot.mode_definition_id, HEIST_MODE_DEFINITION);
        assert_eq!(heist.snapshot.identity.recipe_revision, 3);
        assert!(heist.objective_zone.is_none());
        assert_eq!(heist.heist_safes.len(), 2);
        assert_eq!(
            heist.heist_safes[0].defending_team,
            crate::combat::TeamId(0)
        );
        assert_eq!(
            heist.heist_safes[1].defending_team,
            crate::combat::TeamId(1)
        );
        assert_eq!(heist.heist_safes[0].center, Vec2::new(0.0, -400.0));
        assert_eq!(heist.heist_safes[1].center, Vec2::new(0.0, 400.0));
        let cactus_cells = heist
            .snapshot
            .placements
            .iter()
            .filter(|placement| placement.asset_id == CACTUS_ASSET)
            .map(|placement| placement.cell)
            .collect::<BTreeSet<_>>();
        assert_eq!(
            cactus_cells,
            BTreeSet::from([MapCell::new(9, 20), MapCell::new(15, 17)])
        );
        assert_eq!(
            catalog.asset(CACTUS_ASSET).unwrap().gameplay_profile_id,
            catalog
                .asset(DESTRUCTIBLE_COVER_ASSET)
                .unwrap()
                .gameplay_profile_id
        );
    }

    #[test]
    fn proper_three_vs_three_maps_resolve_the_kaykit_visual_variants() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let wipeout = catalog
            .resolve_preset(VERDANT_CROSSFIRE_PRESET, MapInstanceId(10))
            .unwrap();
        let hot_zone = catalog
            .resolve_preset(SWITCHBACK_BASIN_PRESET, MapInstanceId(11))
            .unwrap();
        let heist = catalog
            .resolve_preset(POWDERLINE_VAULT_PRESET, MapInstanceId(12))
            .unwrap();

        for asset_id in [GREEN_SYMBOL_WALL_ASSET, YELLOW_STRIPED_COVER_ASSET] {
            assert!(
                wipeout
                    .snapshot
                    .placements
                    .iter()
                    .any(|placement| placement.asset_id == asset_id)
            );
        }
        for asset_id in [RED_BRICK_WALL_ASSET, GREEN_STRIPED_COVER_ASSET] {
            assert!(
                hot_zone
                    .snapshot
                    .placements
                    .iter()
                    .any(|placement| placement.asset_id == asset_id)
            );
        }

        let cells_for_asset = |asset_id| {
            heist
                .snapshot
                .placements
                .iter()
                .filter(|placement| placement.asset_id == asset_id)
                .map(|placement| placement.cell)
                .collect::<BTreeSet<_>>()
        };
        let expected_metal_cells = (18..=25)
            .map(|y| MapCell::new(20, y))
            .chain((12..=18).map(|y| MapCell::new(4, y)))
            .chain((2..=4).map(|x| MapCell::new(x, 22)))
            .chain((20..=22).map(|x| MapCell::new(x, 14)))
            .collect::<BTreeSet<_>>();
        let expected_wood_cells = (22..=26)
            .map(|y| MapCell::new(17, y))
            .chain((10..=14).map(|y| MapCell::new(7, y)))
            .collect::<BTreeSet<_>>();
        assert_eq!(cells_for_asset(METAL_WALL_ASSET), expected_metal_cells);
        assert_eq!(cells_for_asset(WOOD_WALL_ASSET), expected_wood_cells);
        assert!(!cells_for_asset(RED_BRICK_WALL_ASSET).is_empty());
        assert!(!cells_for_asset(YELLOW_STRIPED_COVER_ASSET).is_empty());
        for resolved in [&wipeout, &hot_zone, &heist] {
            assert!(resolved.snapshot.placements.iter().all(|placement| {
                !matches!(
                    placement.asset_id,
                    GARDEN_WALL_ASSET | DESTRUCTIBLE_COVER_ASSET
                )
            }));
        }
    }

    #[test]
    fn map_dimensions_separate_engine_safety_from_server_policy() {
        let limits = MapDimensionLimits::default();
        assert_eq!(limits.minimum_width, 20);
        assert_eq!(limits.minimum_height, 20);
        assert_eq!(limits.maximum_width, 512);
        assert_eq!(limits.maximum_height, 512);
        assert!(limits.validate().is_ok());
        assert!(
            limits
                .validate_dimensions(MapDimensions {
                    width: 20,
                    height: 20,
                })
                .is_ok()
        );
        assert!(
            limits
                .validate_dimensions(MapDimensions {
                    width: 512,
                    height: 512,
                })
                .is_ok()
        );
        assert!(
            limits
                .validate_dimensions(MapDimensions {
                    width: 19,
                    height: 20,
                })
                .is_err()
        );
        assert!(
            MapDimensions {
                width: 1,
                height: 1,
            }
            .validate()
            .is_ok()
        );
        assert!(
            MapDimensions {
                width: 513,
                height: 512,
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn map_dimension_policy_rejects_invalid_operator_envelopes() {
        for limits in [
            MapDimensionLimits {
                minimum_width: 0,
                ..MapDimensionLimits::default()
            },
            MapDimensionLimits {
                minimum_width: 100,
                maximum_width: 99,
                ..MapDimensionLimits::default()
            },
            MapDimensionLimits {
                maximum_height: 513,
                ..MapDimensionLimits::default()
            },
        ] {
            assert!(limits.validate().is_err());
        }
    }

    #[test]
    fn placement_capacity_scales_with_map_cells_and_asset_slots() {
        let dimensions = MapDimensions {
            width: 512,
            height: 512,
        };
        assert_eq!(dimensions.cell_count(), 262_144);
        assert_eq!(dimensions.placement_capacity(), 1_048_576);
        assert!(validate_placement_capacity(dimensions, 1_048_576, 262_144).is_ok());
        assert!(validate_placement_capacity(dimensions, 1_048_577, 262_144).is_err());
        assert!(validate_placement_capacity(dimensions, 262_144, 262_145).is_err());
    }

    #[test]
    fn tidal_garden_resolves_exact_authored_counts_and_mirrored_footprints() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let tall_grass = catalog.asset(TALL_GRASS_ASSET).unwrap();
        let tall_grass_profile = catalog.profile(tall_grass.gameplay_profile_id).unwrap();
        assert_eq!(
            tall_grass_profile.concealment,
            MapConcealmentBehavior::HideOccupants
        );
        assert_ne!(tall_grass.gameplay_profile_id, MapGameplayProfileId(1));
        assert_eq!(
            catalog
                .profile(MapGameplayProfileId(1))
                .unwrap()
                .concealment,
            MapConcealmentBehavior::None
        );
        let resolved = catalog
            .resolve_preset(TIDAL_GARDEN_PRESET, MapInstanceId(8))
            .unwrap();
        assert_eq!(
            resolved.snapshot.dimensions,
            MapDimensions {
                width: 40,
                height: 28,
            }
        );
        let count = |asset_id| {
            resolved
                .snapshot
                .placements
                .iter()
                .filter(|placement| placement.asset_id == asset_id)
                .count()
        };
        assert_eq!(count(WATER_ASSET), 48);
        assert_eq!(count(TALL_GRASS_ASSET), 40);
        assert_eq!(count(GARDEN_WALL_ASSET), 36);
        assert_eq!(count(BREAKABLE_BARRIER_ASSET), 4);
        assert_eq!(count(MapAssetId(15)), 6);
        assert_eq!(count(PLAYER_SPAWN_ASSET), 8);
        assert_eq!(resolved.dynamic_placements.len(), 4);
        assert_eq!(
            resolved
                .player_only_surface_rects
                .iter()
                .map(|rectangle| usize::from(rectangle.width) * usize::from(rectangle.height))
                .sum::<usize>(),
            48
        );

        for asset_id in [
            WATER_ASSET,
            TALL_GRASS_ASSET,
            GARDEN_WALL_ASSET,
            BREAKABLE_BARRIER_ASSET,
        ] {
            let cells: BTreeSet<_> = resolved
                .snapshot
                .placements
                .iter()
                .filter(|placement| placement.asset_id == asset_id)
                .flat_map(|placement| {
                    let asset = catalog.asset(asset_id).unwrap();
                    placement_cells(resolved.snapshot.dimensions, asset, placement).unwrap()
                })
                .collect();
            let mirrored: BTreeSet<_> = cells
                .iter()
                .map(|cell| MapCell::new(resolved.snapshot.dimensions.width - 1 - cell.x, cell.y))
                .collect();
            assert_eq!(cells, mirrored, "asset {} is not mirrored", asset_id.0);
        }
    }

    #[test]
    fn feature_yard_hot_zone_preserves_exact_objective_and_shared_topology() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let resolved = catalog
            .resolve_preset(FEATURE_YARD_HOT_ZONE_PRESET, MapInstanceId(9))
            .unwrap();
        assert_eq!(
            resolved.snapshot.dimensions,
            MapDimensions {
                width: 64,
                height: 40
            }
        );
        assert_eq!(resolved.static_colliders.len(), 4);
        assert_eq!(resolved.dynamic_placements.len(), 14);
        let grass = resolved
            .snapshot
            .placements
            .iter()
            .filter(|placement| placement.asset_id == TALL_GRASS_ASSET)
            .map(|placement| placement.cell)
            .collect::<BTreeSet<_>>();
        assert_eq!(grass.len(), 8);
        assert_eq!(
            grass
                .iter()
                .map(|cell| MapCell::new(resolved.snapshot.dimensions.width - 1 - cell.x, cell.y))
                .collect::<BTreeSet<_>>(),
            grass
        );
        assert_eq!(resolved.snapshot.mode_anchors.len(), 1);
        let objective = resolved.objective_zone.unwrap();
        assert_eq!(objective.anchor_id, ModeAnchorId(1));
        assert_eq!(
            objective.area,
            crate::map::NormalizedArea {
                center: Vec2::ZERO,
                shape: MapShape::Circle { radius: 160.0 },
            }
        );
    }

    #[test]
    fn feature_yard_resolves_exact_mirrored_heist_safe_anchors() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let resolved = catalog
            .resolve_preset(FEATURE_YARD_HEIST_PRESET, MapInstanceId(10))
            .unwrap();
        assert_eq!(resolved.snapshot.mode_definition_id, HEIST_MODE_DEFINITION);
        assert!(resolved.objective_zone.is_none());
        assert_eq!(resolved.heist_safes.len(), 2);
        assert_eq!(resolved.heist_safes[0].anchor_id, ModeAnchorId(1));
        assert_eq!(
            resolved.heist_safes[0].defending_team,
            crate::combat::TeamId(0)
        );
        assert_eq!(resolved.heist_safes[1].anchor_id, ModeAnchorId(2));
        assert_eq!(
            resolved.heist_safes[1].defending_team,
            crate::combat::TeamId(1)
        );
        assert_eq!(resolved.heist_safes[0].half_extents, Vec2::new(48.0, 32.0));
        assert!(
            (resolved.heist_safes[0].center.x + resolved.heist_safes[1].center.x).abs()
                < f32::EPSILON
        );
        assert!(
            (resolved.heist_safes[0].center.y - resolved.heist_safes[1].center.y).abs()
                < f32::EPSILON
        );
        assert_eq!(resolved.dynamic_placements.len(), 14);
        assert!(
            resolved
                .spawn_points_by_team
                .values()
                .all(|spawns| spawns.len() == 3)
        );
    }

    #[test]
    fn feature_yard_rejects_safe_overlap_wrong_visual_and_sealed_access() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let recipe = catalog
            .presets
            .iter()
            .find(|preset| preset.id == FEATURE_YARD_HEIST_PRESET)
            .unwrap()
            .recipe
            .clone();

        let mut overlap = recipe.clone();
        overlap.placements.push(MapAssetPlacement {
            placement_id: MapPlacementId(900),
            cell: MapCell::new(3, 19),
            asset_id: WALL_ARENA_ASSET,
            quarter_turns: 0,
            parameters: MapPlacementParameters::None,
        });
        assert!(
            resolve_grid_recipe(
                &overlap,
                FEATURE_YARD_HEIST_PRESET,
                MapInstanceId(1),
                &catalog
            )
            .unwrap_err()
            .contains("safe reservation overlaps")
        );

        let mut wrong_visual = recipe.clone();
        let MapModeAnchorKind::HeistSafe {
            ref mut objective_visual_profile_id,
            ..
        } = wrong_visual.mode_anchors[0].kind
        else {
            unreachable!()
        };
        *objective_visual_profile_id = MapVisualProfileId(1);
        assert!(
            resolve_grid_recipe(
                &wrong_visual,
                FEATURE_YARD_HEIST_PRESET,
                MapInstanceId(1),
                &catalog
            )
            .is_err()
        );

        let mut sealed = recipe;
        for (offset, cell) in [
            MapCell::new(1, 19),
            MapCell::new(1, 20),
            MapCell::new(3, 17),
            MapCell::new(4, 17),
            MapCell::new(5, 17),
            MapCell::new(3, 22),
            MapCell::new(4, 22),
            MapCell::new(5, 22),
        ]
        .into_iter()
        .enumerate()
        {
            sealed.placements.push(MapAssetPlacement {
                placement_id: MapPlacementId(910 + u32::try_from(offset).unwrap()),
                cell,
                asset_id: WALL_ARENA_ASSET,
                quarter_turns: 0,
                parameters: MapPlacementParameters::None,
            });
        }
        assert!(
            resolve_grid_recipe(
                &sealed,
                FEATURE_YARD_HEIST_PRESET,
                MapInstanceId(1),
                &catalog
            )
            .unwrap_err()
            .contains("fewer than two legal attack sectors")
        );
    }

    #[test]
    fn converted_ashen_preserves_walls_round_obstacles_and_reviewed_quantization() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let resolved = catalog
            .resolve_preset(ASHEN_COURT_PRESET, MapInstanceId(10))
            .unwrap();
        assert_eq!(
            resolved.snapshot.dimensions,
            MapDimensions {
                width: 48,
                height: 32
            }
        );
        assert_eq!(resolved.static_colliders.len(), 10);
        assert_eq!(
            resolved
                .spawn_points_by_team
                .values()
                .map(Vec::len)
                .sum::<usize>(),
            8
        );
        assert_eq!(resolved.dynamic_placements.len(), 18);
        assert!(resolved.objective_zone.is_none());

        let circles = resolved
            .static_colliders
            .iter()
            .filter_map(|collider| match collider.shape {
                MapShape::Circle { radius } => Some((collider.position, radius)),
                MapShape::Rectangle { .. } => None,
            })
            .collect::<Vec<_>>();
        assert_eq!(
            circles,
            vec![
                (Vec2::new(-128.0, 0.0), 28.0),
                (Vec2::new(128.0, 0.0), 28.0),
                (Vec2::new(0.0, -320.0), 28.0),
                (Vec2::new(0.0, 320.0), 28.0),
            ]
        );
        let spawns = resolved
            .spawn_points_by_team
            .values()
            .flatten()
            .collect::<Vec<_>>();
        assert_eq!(spawns[0].position, Vec2::new(-624.0, -240.0));
        assert_eq!(spawns[3].position, Vec2::new(-624.0, 240.0));
        assert_eq!(spawns[4].position, Vec2::new(624.0, -240.0));
        assert_eq!(spawns[7].position, Vec2::new(624.0, 240.0));

        let cover_cells = resolved
            .dynamic_placements
            .iter()
            .map(|placement| placement.cell)
            .collect::<BTreeSet<_>>();
        assert!(cover_cells.contains(&MapCell::new(17, 2)));
        assert!(cover_cells.contains(&MapCell::new(19, 4)));
        assert!(cover_cells.contains(&MapCell::new(28, 27)));
        assert!(cover_cells.contains(&MapCell::new(30, 29)));
    }

    #[test]
    fn catalog_rejects_false_collider_profiles_and_bad_mode_anchors() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let mut oversized_circle = catalog.clone();
        oversized_circle.gameplay_profiles[6].collider_shape = MapColliderShape::Circle {
            radius_world_units: 33,
        };
        assert!(oversized_circle.validate().is_err());

        let mut inert_collider = catalog.clone();
        inert_collider.gameplay_profiles[0].collider_shape = MapColliderShape::FootprintRectangle;
        assert!(inert_collider.validate().is_err());

        let hot_zone = catalog
            .preset(FEATURE_YARD_HOT_ZONE_PRESET)
            .unwrap()
            .recipe
            .clone();
        let mut missing = hot_zone.clone();
        missing.mode_anchors.clear();
        assert!(
            resolve_grid_recipe(
                &missing,
                FEATURE_YARD_HOT_ZONE_PRESET,
                MapInstanceId(1),
                &catalog
            )
            .is_err()
        );
        let mut outside = hot_zone;
        outside.mode_anchors[0].kind = MapModeAnchorKind::HotZoneCircle {
            center_half_cell: MapHalfCellPoint { x: 0, y: 0 },
            radius_half_cells: 10,
        };
        assert!(
            resolve_grid_recipe(
                &outside,
                FEATURE_YARD_HOT_ZONE_PRESET,
                MapInstanceId(1),
                &catalog
            )
            .is_err()
        );
    }

    #[test]
    fn damageable_profiles_reject_invalid_bounds_references_and_incompatible_behavior() {
        let catalog = MapContentCatalog::embedded().unwrap();

        let barrel = catalog.asset(OIL_BARREL_ASSET).unwrap();
        let damage = match catalog
            .profile(barrel.gameplay_profile_id)
            .unwrap()
            .durability
        {
            MapDurabilityBehavior::HitPoints(id) => catalog.damage_profile(id).unwrap(),
            MapDurabilityBehavior::Indestructible => panic!("oil barrel must be damageable"),
        };
        assert_eq!(
            damage.terminal,
            MapObjectTerminalBehavior::Explode {
                explosion_profile_id: EnvironmentExplosionProfileId(1),
                outcome: MapPlacementOutcome::ReplacedWith(BARREL_WOOD_DEBRIS_ASSET),
            }
        );
        let debris = catalog.asset(BARREL_WOOD_DEBRIS_ASSET).unwrap();
        assert_eq!(barrel.slot, debris.slot);
        assert_eq!(barrel.footprint_cells, debris.footprint_cells);
        assert_eq!(
            catalog.profile(debris.gameplay_profile_id).unwrap(),
            &MapGameplayProfile {
                id: MapGameplayProfileId(1),
                player_collision: PlayerCollision::Pass,
                projectile_collision: ProjectileCollision::Pass,
                collider_shape: MapColliderShape::None,
                destruction: MapDestructionBehavior::Indestructible,
                durability: MapDurabilityBehavior::Indestructible,
                interaction: MapInteractionBehavior::None,
                concealment: MapConcealmentBehavior::None,
                effect_tile: MapEffectTileBehavior::None,
            }
        );

        let mut zero_health = catalog.clone();
        zero_health.damage_profiles[0].maximum_health = 0;
        assert!(zero_health.validate().is_err());

        let mut excessive_health = catalog.clone();
        excessive_health.damage_profiles[0].maximum_health = MAX_MAP_OBJECT_HEALTH + 1;
        assert!(excessive_health.validate().is_err());

        let mut invalid_explosion = catalog.clone();
        invalid_explosion.explosion_profiles[0].maximum_chain_reactions = 17;
        assert!(invalid_explosion.validate().is_err());

        let barrel_profile = catalog
            .gameplay_profiles
            .iter()
            .position(|profile| profile.id == MapGameplayProfileId(9))
            .unwrap();
        let mut destroy_bypass = catalog.clone();
        destroy_bypass.gameplay_profiles[barrel_profile].destruction =
            MapDestructionBehavior::RemoveOnMapDestruction;
        assert!(destroy_bypass.validate().is_err());

        let mut concealed = catalog;
        concealed.gameplay_profiles[barrel_profile].concealment =
            MapConcealmentBehavior::HideOccupants;
        assert!(concealed.validate().is_err());
    }

    #[test]
    fn effect_tiles_reject_noncanonical_values_and_spawn_hazards() {
        let mut invalid_value = MapContentCatalog::embedded().unwrap();
        invalid_value.gameplay_profiles[10].effect_tile = MapEffectTileBehavior::Speed {
            movement_multiplier_milli: 1_249,
        };
        assert!(invalid_value.validate().is_err());

        let mut unsafe_spawn = MapContentCatalog::embedded().unwrap();
        let preset = unsafe_spawn
            .presets
            .iter_mut()
            .find(|preset| preset.id == FEATURE_YARD_WIPEOUT_PRESET)
            .unwrap();
        let damage_tile = preset
            .recipe
            .placements
            .iter_mut()
            .find(|placement| placement.asset_id == DAMAGE_TILE_ASSET)
            .unwrap();
        damage_tile.cell = MapCell::new(9, 10);
        assert!(
            unsafe_spawn
                .resolve_preset(FEATURE_YARD_WIPEOUT_PRESET, MapInstanceId(1))
                .is_err_and(|error| error.contains("spawn safety"))
        );
    }

    #[test]
    fn barrel_yard_places_one_dungeon_wall_directly_beside_the_reference_barrel() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let resolved = catalog
            .resolve_preset(BARREL_YARD_PRESET, MapInstanceId(1))
            .unwrap();
        let wall = resolved
            .snapshot
            .placements
            .iter()
            .find(|placement| placement.placement_id == MapPlacementId(90))
            .unwrap();
        let barrel = resolved
            .snapshot
            .placements
            .iter()
            .find(|placement| placement.placement_id == MapPlacementId(100))
            .unwrap();

        assert_eq!(wall.asset_id, WALL_DUNGEON_ASSET);
        assert_eq!(barrel.asset_id, OIL_BARREL_ASSET);
        assert_eq!(wall.cell.y, barrel.cell.y);
        assert_eq!(wall.cell.x + 1, barrel.cell.x);
    }

    #[test]
    fn barrier_footprint_rotates_and_replaces_with_matching_rubble() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let barrier = catalog.asset(BREAKABLE_BARRIER_ASSET).unwrap();
        let rubble = catalog.asset(RUBBLE_ASSET).unwrap();
        assert_eq!(
            barrier.footprint_cells,
            MapFootprint {
                width: 2,
                height: 1
            }
        );
        assert_eq!(
            barrier.footprint_cells.rotated(1),
            MapFootprint {
                width: 1,
                height: 2
            }
        );
        assert_eq!(barrier.footprint_cells, rubble.footprint_cells);
        assert_eq!(
            catalog
                .profile(barrier.gameplay_profile_id)
                .unwrap()
                .destruction,
            MapDestructionBehavior::ReplaceOnMapDestruction(RUBBLE_ASSET)
        );
    }

    #[test]
    fn cardinal_adjacency_mask_covers_all_sixteen_neighbor_shapes() {
        let center = MapCell::new(10, 10);
        let neighbors = [
            MapCell::new(10, 11),
            MapCell::new(11, 10),
            MapCell::new(10, 9),
            MapCell::new(9, 10),
        ];
        for expected in 0_u8..16 {
            let occupied = neighbors
                .iter()
                .enumerate()
                .filter_map(|(bit, cell)| (expected & (1 << bit) != 0).then_some(*cell))
                .collect();
            assert_eq!(cardinal_adjacency_mask(center, &occupied), expected);
        }
    }

    #[test]
    fn canonical_grid_fingerprint_ignores_source_placement_order() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let mut shuffled = catalog.clone();
        shuffled.presets[0].recipe.placements.reverse();
        shuffled.presets[0].recipe.filled_rects.reverse();
        let left = catalog
            .resolve_preset(CROSSROADS_PRESET, MapInstanceId(1))
            .unwrap();
        let right = shuffled
            .resolve_preset(CROSSROADS_PRESET, MapInstanceId(1))
            .unwrap();
        assert_eq!(left.snapshot, right.snapshot);
        assert_eq!(
            catalog.canonical_fingerprint_material().unwrap(),
            shuffled.canonical_fingerprint_material().unwrap()
        );
    }

    #[test]
    fn crossroads_wire_payloads_stay_bounded() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let resolved = catalog
            .resolve_preset(CROSSROADS_PRESET, MapInstanceId(1))
            .unwrap();
        let terminal_states: Vec<_> = resolved
            .dynamic_placements
            .iter()
            .map(|placement| MapPlacementTransition {
                placement_id: placement.placement_id,
                outcome: MapPlacementOutcome::Removed,
            })
            .collect();
        let generation = MapDynamicGeneration {
            map_instance_id: MapInstanceId(1),
            generation: 1,
        };
        let event = MapMutationEvent {
            generation,
            revision: 1,
            transitions: terminal_states.clone(),
        };
        let recovery = MapDynamicRecoverySnapshot {
            state: MapDynamicState {
                map_instance_id: MapInstanceId(1),
                generation: 1,
                revision: 1,
                terminal_states,
            },
        };
        let recipe_source_bytes =
            include_str!("../../content/maps/builtin/crossroads-facility.ron").len();
        let snapshot_bytes = postcard::to_allocvec(&resolved.snapshot).unwrap().len();
        let event_bytes = postcard::to_allocvec(&event).unwrap().len();
        let recovery_bytes = postcard::to_allocvec(&recovery).unwrap().len();

        println!(
            "crossroads bytes: recipe={recipe_source_bytes} snapshot={snapshot_bytes} max_event={event_bytes} full_recovery={recovery_bytes}"
        );
        assert!(recipe_source_bytes <= 96 * 1024);
        assert!(snapshot_bytes <= 64 * 1024);
        assert!(event_bytes <= 4 * 1024);
        assert!(recovery_bytes <= 4 * 1024);
    }

    #[test]
    fn tidal_garden_wire_payloads_stay_bounded() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let resolved = catalog
            .resolve_preset(TIDAL_GARDEN_PRESET, MapInstanceId(1))
            .unwrap();
        let transitions = resolved
            .dynamic_placements
            .iter()
            .map(|placement| MapPlacementTransition {
                placement_id: placement.placement_id,
                outcome: MapPlacementOutcome::ReplacedWith(RUBBLE_ASSET),
            })
            .collect::<Vec<_>>();
        let state = MapDynamicState {
            map_instance_id: MapInstanceId(1),
            generation: 1,
            revision: 1,
            terminal_states: transitions.clone(),
        };
        let event = MapMutationEvent {
            generation: MapDynamicGeneration {
                map_instance_id: state.map_instance_id,
                generation: state.generation,
            },
            revision: state.revision,
            transitions,
        };
        let recipe_source_bytes = include_str!("../../content/maps/builtin/tidal-garden.ron").len();
        let snapshot_bytes = postcard::to_allocvec(&resolved.snapshot).unwrap().len();
        let event_bytes = postcard::to_allocvec(&event).unwrap().len();
        let recovery_bytes = postcard::to_allocvec(&MapDynamicRecoverySnapshot { state })
            .unwrap()
            .len();

        println!(
            "tidal garden bytes: recipe={recipe_source_bytes} snapshot={snapshot_bytes} max_event={event_bytes} full_recovery={recovery_bytes}"
        );
        assert!(recipe_source_bytes <= 96 * 1024);
        assert!(snapshot_bytes <= 64 * 1024);
        assert!(event_bytes <= 4 * 1024);
        assert!(recovery_bytes <= 4 * 1024);
    }

    #[test]
    fn converted_map_wire_payloads_stay_bounded() {
        let catalog = MapContentCatalog::embedded().unwrap();
        for (preset_id, recipe_source) in [
            (
                FEATURE_YARD_HOT_ZONE_PRESET,
                include_str!("../../content/maps/builtin/feature-yard-hot-zone.ron"),
            ),
            (
                ASHEN_COURT_PRESET,
                include_str!("../../content/maps/builtin/ashen-court.ron"),
            ),
        ] {
            let resolved = catalog.resolve_preset(preset_id, MapInstanceId(1)).unwrap();
            let terminal_states = resolved
                .dynamic_placements
                .iter()
                .map(|placement| MapPlacementTransition {
                    placement_id: placement.placement_id,
                    outcome: MapPlacementOutcome::Removed,
                })
                .collect::<Vec<_>>();
            let state = MapDynamicState {
                map_instance_id: MapInstanceId(1),
                generation: 1,
                revision: 1,
                terminal_states: terminal_states.clone(),
            };
            let event = MapMutationEvent {
                generation: MapDynamicGeneration {
                    map_instance_id: state.map_instance_id,
                    generation: state.generation,
                },
                revision: state.revision,
                transitions: terminal_states,
            };
            let snapshot_bytes = postcard::to_allocvec(&resolved.snapshot).unwrap().len();
            let event_bytes = postcard::to_allocvec(&event).unwrap().len();
            let recovery_bytes = postcard::to_allocvec(&MapDynamicRecoverySnapshot { state })
                .unwrap()
                .len();

            println!(
                "converted preset {} bytes: recipe={} snapshot={snapshot_bytes} max_event={event_bytes} full_recovery={recovery_bytes}",
                preset_id.0,
                recipe_source.len()
            );
            assert!(recipe_source.len() <= 96 * 1024);
            assert!(snapshot_bytes <= 64 * 1024);
            assert!(event_bytes <= 4 * 1024);
            assert!(recovery_bytes <= 4 * 1024);
        }
    }

    #[test]
    fn grid_recipe_rejects_invalid_references_bounds_rotation_and_slot_conflicts() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let base = catalog.presets[0].recipe.clone();
        let mut cases = Vec::new();

        let mut unknown_asset = base.clone();
        unknown_asset.placements[0].asset_id = MapAssetId(u16::MAX);
        cases.push(unknown_asset);

        let mut out_of_bounds = base.clone();
        out_of_bounds.placements[0].cell = MapCell::new(base.dimensions.width, 0);
        cases.push(out_of_bounds);

        let mut bad_rotation = base.clone();
        bad_rotation.placements[0].quarter_turns = 4;
        cases.push(bad_rotation);

        let mut slot_conflict = base.clone();
        let mut duplicate = slot_conflict.placements[0].clone();
        duplicate.placement_id = MapPlacementId(9_999);
        duplicate.asset_id = slot_conflict.placements[1].asset_id;
        slot_conflict.placements.push(duplicate);
        cases.push(slot_conflict);

        let mut bad_schema = base;
        bad_schema.recipe_version = MAP_RECIPE_SCHEMA_VERSION + 1;
        cases.push(bad_schema);

        for recipe in cases {
            assert!(
                resolve_grid_recipe(&recipe, CROSSROADS_PRESET, MapInstanceId(1), &catalog)
                    .is_err()
            );
        }
    }
}
