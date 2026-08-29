//! Authored sparse-grid map definitions, embedded loading, validation, and fingerprints.
//!
//! This module is intentionally independent from the V4 object-role and region catalogs. Its
//! sibling resolution module derives the neutral runtime facts consumed by existing match systems;
//! neither concern converts a grid placement into an authored V4 recipe or region.

use super::resolution::resolve_grid_recipe;
use super::{
    MapEffectTileBehavior, MapInstanceId, MapPlacementId, MapPlacementOutcome,
    MapPresentationThemeId, MapPresetId, MapRecipeId, ModeAnchorId, ModeDefinitionId, ResolvedMap,
};
use bevy::prelude::{App, Component, Plugin, Resource};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAP_CELL_SIZE_WORLD: f32 = 32.0;
pub const MAX_MAP_OBJECT_HEALTH: u16 = 1_000;
pub const MAX_RESOLVED_MAP_SNAPSHOT_BYTES: usize = 32 * 1024 * 1024;
pub const MAP_CATALOG_SCHEMA_VERSION: u16 = 7;
pub const MAP_RECIPE_SCHEMA_VERSION: u16 = 5;
pub const MAP_FINGERPRINT_FORMAT_VERSION: u16 = 9;
#[cfg(test)]
pub(super) const CROSSROADS_PRESET: MapPresetId = MapPresetId(1);
pub const ASHEN_COURT_PRESET: MapPresetId = MapPresetId(3);
pub const ASHEN_COURT_ADMISSION_REVISION: u16 = 2;
#[cfg(test)]
pub(super) const TIDAL_GARDEN_PRESET: MapPresetId = MapPresetId(4);
#[cfg(test)]
pub(super) const BARREL_YARD_PRESET: MapPresetId = MapPresetId(5);
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
    pub(super) const fn placement_capacity(self) -> usize {
        self.cell_count() * MAP_ASSET_SLOT_COUNT
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
        if !replacement_is_compatible(source, profile, replacement, replacement_profile) {
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

fn replacement_is_compatible(
    source: &MapAssetDefinition,
    source_profile: &MapGameplayProfile,
    replacement: &MapAssetDefinition,
    replacement_profile: &MapGameplayProfile,
) -> bool {
    let preserves_authored_shape = source.slot == replacement.slot
        && source.footprint_cells == replacement.footprint_cells
        && source.allowed_surface_tags == replacement.allowed_surface_tags;
    let replacement_is_terminal = replacement.parameter_kind == MapPlacementParameterKind::None
        && replacement_profile.destruction == MapDestructionBehavior::Indestructible
        && replacement_profile.durability == MapDurabilityBehavior::Indestructible;
    let does_not_add_collision = !(source_profile.player_collision == PlayerCollision::Pass
        && replacement_profile.player_collision == PlayerCollision::Block
        || source_profile.projectile_collision == ProjectileCollision::Pass
            && replacement_profile.projectile_collision == ProjectileCollision::BlockAndConsume);
    let blocking_shape_matches = (replacement_profile.player_collision != PlayerCollision::Block
        && replacement_profile.projectile_collision != ProjectileCollision::BlockAndConsume)
        || source_profile.collider_shape == replacement_profile.collider_shape;
    preserves_authored_shape
        && replacement_is_terminal
        && does_not_add_collision
        && blocking_shape_matches
}

fn validate_asset_profile(
    asset: &MapAssetDefinition,
    profile: MapGameplayProfile,
) -> Result<(), String> {
    profile.effect_tile.validate()?;
    let inert = profile_is_inert(profile);
    let blocks = profile_blocks(profile);
    let concealment_is_consistent = concealment_matches_asset(asset, profile);
    let effect_tile_is_consistent = effect_tile_matches_asset(asset, profile, inert);
    let collider_is_consistent = collider_matches_asset(asset, profile, blocks);
    let durability_is_consistent = durability_matches_asset(asset, profile, blocks);
    let slot_is_consistent = slot_matches_profile(asset, profile, inert);
    (slot_is_consistent
        && collider_is_consistent
        && concealment_is_consistent
        && effect_tile_is_consistent
        && durability_is_consistent)
        .then_some(())
        .ok_or_else(|| format!("asset {} contradicts its slot/gameplay profile", asset.key))
}

fn profile_is_inert(profile: MapGameplayProfile) -> bool {
    profile.player_collision == PlayerCollision::Pass
        && profile.projectile_collision == ProjectileCollision::Pass
        && profile.destruction == MapDestructionBehavior::Indestructible
        && profile.durability == MapDurabilityBehavior::Indestructible
        && profile.collider_shape == MapColliderShape::None
}

fn profile_blocks(profile: MapGameplayProfile) -> bool {
    profile.player_collision == PlayerCollision::Block
        || profile.projectile_collision == ProjectileCollision::BlockAndConsume
}

fn concealment_matches_asset(asset: &MapAssetDefinition, profile: MapGameplayProfile) -> bool {
    match profile.concealment {
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
    }
}

fn collider_matches_asset(
    asset: &MapAssetDefinition,
    profile: MapGameplayProfile,
    blocks: bool,
) -> bool {
    match profile.collider_shape {
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
    }
}

fn durability_matches_asset(
    asset: &MapAssetDefinition,
    profile: MapGameplayProfile,
    blocks: bool,
) -> bool {
    match profile.durability {
        MapDurabilityBehavior::Indestructible => true,
        MapDurabilityBehavior::HitPoints(_) => {
            asset.slot == MapAssetSlot::Feature
                && profile.destruction == MapDestructionBehavior::Indestructible
                && profile.interaction == MapInteractionBehavior::None
                && profile.concealment == MapConcealmentBehavior::None
                && profile.collider_shape != MapColliderShape::None
                && blocks
        }
    }
}

fn slot_matches_profile(
    asset: &MapAssetDefinition,
    profile: MapGameplayProfile,
    inert: bool,
) -> bool {
    match asset.slot {
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
    }
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
