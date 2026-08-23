//! Sparse-grid map assets, authored recipes, canonical resolution, and shared wire state.
//!
//! This module is intentionally independent from the V4 object-role and region catalogs. During
//! the canonical runtime it derives the neutral runtime facts still consumed by existing
//! match systems; it never converts a grid placement into an authored V4 recipe or region.

use super::{
    AxisAlignedMapRect, MapInstanceId, MapPlacementId, MapPresentationThemeId, MapPresetId,
    MapRecipeFingerprint, MapRecipeId, MapShape, ModeAnchorId, ModeDefinitionId,
    ResolvedMapIdentity, SpawnPointId, TeamSpawnPoint,
};
use bevy::prelude::{App, Component, Plugin, Resource, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAP_CELL_SIZE_WORLD: f32 = 32.0;
pub const MAP_CATALOG_SCHEMA_VERSION: u16 = 4;
pub const MAP_RECIPE_SCHEMA_VERSION: u16 = 3;
pub const MAP_FINGERPRINT_FORMAT_VERSION: u16 = 4;
pub const CROSSROADS_PRESET: MapPresetId = MapPresetId(1);
pub const CROSSROADS_ADMISSION_REVISION: u16 = 5;
pub const CROSSROADS_HOT_ZONE_PRESET: MapPresetId = MapPresetId(2);
pub const CROSSROADS_HOT_ZONE_ADMISSION_REVISION: u16 = 3;
pub const ASHEN_COURT_PRESET: MapPresetId = MapPresetId(3);
pub const ASHEN_COURT_ADMISSION_REVISION: u16 = 2;
pub const TIDAL_GARDEN_PRESET: MapPresetId = MapPresetId(4);
pub const TIDAL_GARDEN_ADMISSION_REVISION: u16 = 1;
pub const WIPEOUT_MODE_DEFINITION: ModeDefinitionId = ModeDefinitionId(2);
pub const HOT_ZONE_MODE_DEFINITION: ModeDefinitionId = ModeDefinitionId(3);

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

impl MapDimensions {
    pub fn validate(self) -> Result<(), String> {
        if !(32..=128).contains(&self.width) || !(24..=96).contains(&self.height) {
            return Err("grid map dimensions exceed the supported bounds".to_string());
        }
        Ok(())
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
    pub interaction: MapInteractionBehavior,
    pub concealment: MapConcealmentBehavior,
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
pub struct MapGridVertex {
    pub x: u16,
    pub y: u16,
}

impl MapDimensions {
    #[must_use]
    pub fn vertex_world(self, vertex: MapGridVertex) -> Option<Vec2> {
        (vertex.x <= self.width && vertex.y <= self.height).then(|| {
            self.bounds().min
                + Vec2::new(f32::from(vertex.x), f32::from(vertex.y)) * MAP_CELL_SIZE_WORLD
        })
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Ord, PartialOrd)]
pub enum MapModeAnchorKind {
    HotZoneCircle {
        center_vertex: MapGridVertex,
        radius_cells: u16,
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

    pub fn canonical_fingerprint_material(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut canonical = self.clone();
        canonical
            .gameplay_profiles
            .sort_by_key(|profile| profile.id);
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
    let terminal_assets: BTreeSet<_> = profiles
        .values()
        .filter_map(|profile| match profile.destruction {
            MapDestructionBehavior::ReplaceOnMapDestruction(id) => Some(id),
            _ => None,
        })
        .collect();
    for source in &catalog.assets {
        let profile = profiles
            .get(&source.gameplay_profile_id)
            .expect("asset profiles were validated before replacements");
        let MapDestructionBehavior::ReplaceOnMapDestruction(replacement_id) = profile.destruction
        else {
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
            WIPEOUT_MODE_DEFINITION | HOT_ZONE_MODE_DEFINITION
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
    if placements.len() > 512 {
        return Err("grid map exceeds the expanded placement ceiling".to_string());
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
    if concealment_placement_count > 128 {
        return Err("grid map exceeds the concealment placement ceiling".to_string());
    }
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
    let objective_zone = validate_and_resolve_mode_anchors(
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
    validate_fighter_navigation(&placements, recipe.dimensions, catalog)?;
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
    if bytes.len() > 64 * 1024 {
        return Err("map snapshot exceeds the byte ceiling".to_string());
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
    Ok(ResolvedMap {
        snapshot,
        spawn_points_by_team,
        static_colliders,
        dynamic_placements,
        player_only_surface_rects,
        objective_zone,
    })
}

fn validate_and_resolve_mode_anchors(
    mode: ModeDefinitionId,
    dimensions: MapDimensions,
    anchors: &[MapModeAnchorPlacement],
    placement_ids: &mut BTreeSet<MapPlacementId>,
) -> Result<Option<ResolvedMapObjective>, String> {
    if mode == WIPEOUT_MODE_DEFINITION {
        return anchors
            .is_empty()
            .then_some(None)
            .ok_or_else(|| "Wipeout maps cannot contain mode anchors".to_string());
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
        center_vertex,
        radius_cells,
    } = anchor.kind;
    if radius_cells == 0 || radius_cells > 32 {
        return Err("invalid Hot Zone objective radius".to_string());
    }
    let center = dimensions
        .vertex_world(center_vertex)
        .ok_or_else(|| "Hot Zone objective center is out of bounds".to_string())?;
    let radius = f32::from(radius_cells) * MAP_CELL_SIZE_WORLD;
    let bounds = dimensions.bounds();
    if center.x - radius < bounds.min.x
        || center.x + radius > bounds.max.x
        || center.y - radius < bounds.min.y
        || center.y + radius > bounds.max.y
    {
        return Err("Hot Zone objective does not fit playable bounds".to_string());
    }
    Ok(Some(ResolvedMapObjective {
        anchor_id: anchor.anchor_id,
        area: super::NormalizedArea {
            center,
            shape: MapShape::Circle { radius },
        },
    }))
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
            if blocked
                .iter()
                .any(|shape| circle_overlaps_derived_shape(center, 24.0, *shape))
            {
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
        dimensions.bounds().contains_with_inset(center, 24.0)
            && blocked
                .iter()
                .all(|shape| !circle_overlaps_derived_shape(center, 24.0, *shape))
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
                && profile.destruction == MapDestructionBehavior::Indestructible)
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
        if profile.destruction != MapDestructionBehavior::Indestructible {
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
    let inert = profile.player_collision == PlayerCollision::Pass
        && profile.projectile_collision == ProjectileCollision::Pass
        && profile.destruction == MapDestructionBehavior::Indestructible
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
                && profile.interaction == MapInteractionBehavior::None
        }
    };
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
    (valid && collider_is_consistent && concealment_is_consistent)
        .then_some(())
        .ok_or_else(|| format!("asset {} contradicts its slot/gameplay profile", asset.key))
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
    fn converted_hot_zone_preserves_exact_objective_and_crossroads_topology() {
        let catalog = MapContentCatalog::embedded().unwrap();
        let resolved = catalog
            .resolve_preset(CROSSROADS_HOT_ZONE_PRESET, MapInstanceId(9))
            .unwrap();
        assert_eq!(
            resolved.snapshot.dimensions,
            MapDimensions {
                width: 56,
                height: 36
            }
        );
        assert_eq!(resolved.static_colliders.len(), 6);
        assert_eq!(resolved.dynamic_placements.len(), 36);
        let grass = resolved
            .snapshot
            .placements
            .iter()
            .filter(|placement| placement.asset_id == TALL_GRASS_ASSET)
            .map(|placement| placement.cell)
            .collect::<BTreeSet<_>>();
        assert_eq!(grass.len(), 30);
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

        let hot_zone = catalog.presets[1].recipe.clone();
        let mut missing = hot_zone.clone();
        missing.mode_anchors.clear();
        assert!(
            resolve_grid_recipe(
                &missing,
                CROSSROADS_HOT_ZONE_PRESET,
                MapInstanceId(1),
                &catalog
            )
            .is_err()
        );
        let mut outside = hot_zone;
        outside.mode_anchors[0].kind = MapModeAnchorKind::HotZoneCircle {
            center_vertex: MapGridVertex { x: 0, y: 0 },
            radius_cells: 5,
        };
        assert!(
            resolve_grid_recipe(
                &outside,
                CROSSROADS_HOT_ZONE_PRESET,
                MapInstanceId(1),
                &catalog
            )
            .is_err()
        );
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
                CROSSROADS_HOT_ZONE_PRESET,
                include_str!("../../content/maps/builtin/crossroads-facility-hot-zone.ron"),
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
