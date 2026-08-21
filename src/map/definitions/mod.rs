//! Embedded map catalog parsing, validation, canonicalization, and resolution.
#![allow(clippy::wildcard_imports)]

use bevy::prelude::{App, FromWorld, Plugin, Resource};
use serde::{Deserialize, Serialize};

use super::model::*;
use super::objects::{MapObjectCatalog, MapObjectCatalogResource, MapObjectRole};

pub const MAP_CATALOG_SCHEMA_VERSION: u16 = 3;
pub const MAP_RECIPE_SCHEMA_VERSION: u16 = 2;
pub const MAP_FINGERPRINT_FORMAT_VERSION: u16 = 3;
pub const SANDBOX_LAYOUT_SCHEMA_VERSION: u16 = 1;
pub const WIPEOUT_LAYOUT_SCHEMA_VERSION: u16 = 1;
pub const HOT_ZONE_LAYOUT_SCHEMA_VERSION: u16 = 1;
pub const HOT_ZONE_MAP_PRESET: MapPresetId = MapPresetId(2);
pub const PRACTICE_DUMMY_ANCHOR_DEFINITION: ModeAnchorDefinitionId = ModeAnchorDefinitionId(1);
pub const HOT_ZONE_OBJECTIVE_ANCHOR_DEFINITION: ModeAnchorDefinitionId = ModeAnchorDefinitionId(2);
pub const SANDBOX_MODE_DEFINITION: ModeDefinitionId = ModeDefinitionId(1);
pub const WIPEOUT_MODE_DEFINITION: ModeDefinitionId = ModeDefinitionId(2);
pub const HOT_ZONE_MODE_DEFINITION: ModeDefinitionId = ModeDefinitionId(3);

/// Code-owned presentation profile for the Hot Zone objective visual.
pub const HOT_ZONE_OBJECTIVE_PRESENTATION_PROFILE: MapPresentationProfileId =
    MapPresentationProfileId(6);

/// Resolve the code-owned presentation profile for one objective anchor definition. The
/// anchor owns exact geometry and identity; the profile owns only color/material styling.
#[must_use]
pub fn objective_presentation_profile(
    anchor: ModeAnchorDefinitionId,
) -> Option<MapPresentationProfileId> {
    match anchor {
        HOT_ZONE_OBJECTIVE_ANCHOR_DEFINITION => Some(HOT_ZONE_OBJECTIVE_PRESENTATION_PROFILE),
        _ => None,
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct EngineMapLimits {
    pub max_absolute_coordinate: f32,
    pub min_playable_width: f32,
    pub max_playable_width: f32,
    pub min_playable_height: f32,
    pub max_playable_height: f32,
    pub min_shape_extent: f32,
    pub max_shape_extent: f32,
    pub max_geometry: usize,
    pub max_visual_instances: usize,
    pub max_entities: usize,
    pub max_regions: usize,
    pub max_spawn_areas: usize,
    pub max_spawn_points: usize,
    pub max_mode_anchors: usize,
    pub max_destructible_reservations: usize,
    pub max_destructible_cells: usize,
    pub max_destructible_chunks: usize,
    pub max_terrain_recovery_bytes: usize,
    pub max_recipe_bytes: usize,
    pub max_snapshot_bytes: usize,
}

impl Default for EngineMapLimits {
    fn default() -> Self {
        Self {
            max_absolute_coordinate: 4_096.0,
            min_playable_width: 1_024.0,
            max_playable_width: 4_096.0,
            min_playable_height: 720.0,
            max_playable_height: 3_072.0,
            min_shape_extent: 8.0,
            max_shape_extent: 2_048.0,
            max_geometry: 256,
            max_visual_instances: 1_024,
            max_entities: 128,
            max_regions: 32,
            max_spawn_areas: 8,
            max_spawn_points: 32,
            max_mode_anchors: 32,
            max_destructible_reservations: 4,
            max_destructible_cells: crate::terrain::MAX_TERRAIN_CELLS,
            max_destructible_chunks: crate::terrain::MAX_TERRAIN_CHUNKS,
            max_terrain_recovery_bytes: crate::terrain::MAX_TERRAIN_RECOVERY_BYTES,
            max_recipe_bytes: 96 * 1_024,
            max_snapshot_bytes: 64 * 1_024,
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct StableDefinition<Id> {
    pub id: Id,
    pub key: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapRecipePolicy {
    pub max_geometry: usize,
    pub max_visual_instances: usize,
    pub max_entities: usize,
    pub max_regions: usize,
    pub max_spawn_areas: usize,
    pub max_spawn_points: usize,
    pub max_mode_anchors: usize,
    pub permitted_collision_profiles: Vec<CollisionProfileId>,
    pub permitted_presentation_profiles: Vec<MapPresentationProfileId>,
    pub permitted_region_profiles: Vec<RegionProfileId>,
    pub permitted_entity_definitions: Vec<EntityDefinitionId>,
    pub permitted_mode_definitions: Vec<ModeDefinitionId>,
    pub permitted_anchor_definitions: Vec<ModeAnchorDefinitionId>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapPreset {
    pub id: MapPresetId,
    pub key: String,
    pub display_name: String,
    pub recipe: MapRecipe,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct MapContentCatalog {
    pub schema_version: u16,
    #[serde(default)]
    pub object_catalog: MapObjectCatalog,
    pub policy: MapRecipePolicy,
    pub presentation_themes: Vec<StableDefinition<MapPresentationThemeId>>,
    pub presentation_profiles: Vec<StableDefinition<MapPresentationProfileId>>,
    pub collision_profiles: Vec<StableDefinition<CollisionProfileId>>,
    pub region_profiles: Vec<StableDefinition<RegionProfileId>>,
    pub entity_definitions: Vec<StableDefinition<EntityDefinitionId>>,
    pub mode_definitions: Vec<StableDefinition<ModeDefinitionId>>,
    pub anchor_definitions: Vec<StableDefinition<ModeAnchorDefinitionId>>,
    pub presets: Vec<MapPreset>,
}

/// The shape constraint one required mode anchor must satisfy.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RequiredAnchorShape {
    Point,
    Area,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RequiredAnchor {
    pub definition_id: ModeAnchorDefinitionId,
    pub minimum: usize,
    pub maximum: usize,
    pub shape: RequiredAnchorShape,
}

#[derive(Clone, Debug, PartialEq)]
pub struct MapLayoutRequirements {
    pub mode_definition_id: ModeDefinitionId,
    pub schema_version: u16,
    pub allowed_team_slots: Vec<u8>,
    pub spawn_areas_per_team: std::ops::RangeInclusive<usize>,
    pub spawn_points_per_team: std::ops::RangeInclusive<usize>,
    pub required_anchors: Vec<RequiredAnchor>,
    pub allowed_region_profiles: Vec<RegionProfileId>,
    pub allowed_entity_profiles: Vec<EntityDefinitionId>,
}

impl MapLayoutRequirements {
    #[must_use]
    pub fn wipeout() -> Self {
        Self {
            mode_definition_id: WIPEOUT_MODE_DEFINITION,
            schema_version: WIPEOUT_LAYOUT_SCHEMA_VERSION,
            allowed_team_slots: vec![0, 1],
            spawn_areas_per_team: 1..=1,
            spawn_points_per_team: 3..=8,
            required_anchors: Vec::new(),
            allowed_region_profiles: vec![DESTRUCTIBLE_TERRAIN_REGION_PROFILE],
            allowed_entity_profiles: vec![EntityDefinitionId(1)],
        }
    }

    #[must_use]
    pub fn hot_zone() -> Self {
        Self {
            mode_definition_id: HOT_ZONE_MODE_DEFINITION,
            schema_version: HOT_ZONE_LAYOUT_SCHEMA_VERSION,
            allowed_team_slots: vec![0, 1],
            spawn_areas_per_team: 1..=1,
            spawn_points_per_team: 3..=8,
            required_anchors: vec![RequiredAnchor {
                definition_id: HOT_ZONE_OBJECTIVE_ANCHOR_DEFINITION,
                minimum: 1,
                maximum: 1,
                shape: RequiredAnchorShape::Area,
            }],
            allowed_region_profiles: vec![DESTRUCTIBLE_TERRAIN_REGION_PROFILE],
            allowed_entity_profiles: vec![EntityDefinitionId(1)],
        }
    }

    #[must_use]
    pub fn sandbox() -> Self {
        Self {
            mode_definition_id: SANDBOX_MODE_DEFINITION,
            schema_version: SANDBOX_LAYOUT_SCHEMA_VERSION,
            allowed_team_slots: vec![0, 1],
            spawn_areas_per_team: 1..=1,
            spawn_points_per_team: 3..=8,
            required_anchors: vec![RequiredAnchor {
                definition_id: PRACTICE_DUMMY_ANCHOR_DEFINITION,
                minimum: 1,
                maximum: 1,
                shape: RequiredAnchorShape::Point,
            }],
            allowed_region_profiles: vec![DESTRUCTIBLE_TERRAIN_REGION_PROFILE],
            allowed_entity_profiles: vec![EntityDefinitionId(1)],
        }
    }

    /// The layout requirements for one stable mode definition, if the mode is known.
    #[must_use]
    pub fn for_mode_definition(mode: ModeDefinitionId) -> Option<Self> {
        match mode {
            SANDBOX_MODE_DEFINITION => Some(Self::sandbox()),
            WIPEOUT_MODE_DEFINITION => Some(Self::wipeout()),
            HOT_ZONE_MODE_DEFINITION => Some(Self::hot_zone()),
            _ => None,
        }
    }
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct MapCatalogResource(pub MapContentCatalog);

impl FromWorld for MapCatalogResource {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self(MapContentCatalog::embedded().expect("embedded map catalog is valid"))
    }
}

pub struct MapContentPlugin;

impl Plugin for MapContentPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<MapCatalogResource>()
            .init_resource::<MapObjectCatalogResource>();
    }
}

impl MapContentCatalog {
    pub fn embedded() -> Result<Self, String> {
        let mut catalog: Self = ron::from_str(include_str!("../../../content/v1/maps.ron"))
            .map_err(|error| format!("embedded map catalog parse failed: {error}"))?;
        catalog.object_catalog = MapObjectCatalog::embedded()?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), String> {
        let limits = EngineMapLimits::default();
        if self.schema_version != MAP_CATALOG_SCHEMA_VERSION {
            return Err("unsupported map catalog schema".to_string());
        }
        self.object_catalog.validate()?;
        validate_policy(&self.policy, limits)?;
        validate_definitions(&self.presentation_themes, "presentation theme")?;
        validate_definitions(&self.presentation_profiles, "presentation profile")?;
        validate_definitions(&self.collision_profiles, "collision profile")?;
        validate_definitions(&self.region_profiles, "region profile")?;
        validate_definitions(&self.entity_definitions, "entity definition")?;
        validate_definitions(&self.mode_definitions, "mode definition")?;
        validate_definitions(&self.anchor_definitions, "anchor definition")?;
        if self.presets.len() != 2
            || self.presets[0].id != MapPresetId(1)
            || self.presets[1].id != MapPresetId(2)
        {
            return Err(
                "the gate accepts exactly built-in map presets 1 and 2 in ascending order"
                    .to_string(),
            );
        }
        for preset in &self.presets {
            if !valid_key(&preset.key) || !valid_display_name(&preset.display_name) {
                return Err("invalid map preset metadata".to_string());
            }
            validate_recipe_references(&preset.recipe, self)?;
        }
        // Every area objective anchor must have a code-owned presentation profile mapping.
        if self.presets.iter().any(|preset| {
            preset.recipe.mode_anchors.iter().any(|anchor| {
                matches!(anchor.shape, ModeAnchorShape::Area { .. })
                    && objective_presentation_profile(anchor.definition_id).is_none()
            })
        }) {
            return Err("an area objective anchor has no presentation profile mapping".to_string());
        }
        Ok(())
    }

    #[must_use]
    pub fn preset(&self, id: MapPresetId) -> Option<&MapPreset> {
        self.presets.iter().find(|preset| preset.id == id)
    }

    pub fn canonical_fingerprint_material(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        let mut canonical = self.clone();
        canonical.presets.sort_by_key(|preset| preset.id);
        for preset in &mut canonical.presets {
            normalize_recipe(&mut preset.recipe)?;
        }
        postcard::to_allocvec(&(
            MAP_FINGERPRINT_FORMAT_VERSION,
            EngineMapLimits::default(),
            (
                SANDBOX_LAYOUT_SCHEMA_VERSION,
                WIPEOUT_LAYOUT_SCHEMA_VERSION,
                HOT_ZONE_LAYOUT_SCHEMA_VERSION,
            ),
            canonical,
        ))
        .map_err(|error| format!("map catalog fingerprint serialization failed: {error}"))
    }

    pub fn resolve_preset(
        &self,
        preset_id: MapPresetId,
        instance_id: MapInstanceId,
        requirements: &MapLayoutRequirements,
    ) -> Result<ResolvedMap, String> {
        let preset = self
            .preset(preset_id)
            .ok_or_else(|| "unknown map preset".to_string())?;
        resolve_map_recipe(
            &preset.recipe,
            Some(preset.id),
            instance_id,
            self,
            requirements,
            EngineMapLimits::default(),
        )
    }
}

fn validate_policy(policy: &MapRecipePolicy, limits: EngineMapLimits) -> Result<(), String> {
    if policy.max_geometry == 0
        || policy.max_geometry > limits.max_geometry
        || policy.max_visual_instances == 0
        || policy.max_visual_instances > limits.max_visual_instances
        || policy.max_entities > limits.max_entities
        || policy.max_regions > limits.max_regions
        || policy.max_spawn_areas == 0
        || policy.max_spawn_areas > limits.max_spawn_areas
        || policy.max_spawn_points == 0
        || policy.max_spawn_points > limits.max_spawn_points
        || policy.max_mode_anchors == 0
        || policy.max_mode_anchors > limits.max_mode_anchors
    {
        return Err("map policy exceeds code-owned engine limits".to_string());
    }
    validate_sorted_unique(&policy.permitted_collision_profiles, "collision profiles")?;
    validate_sorted_unique(
        &policy.permitted_presentation_profiles,
        "presentation profiles",
    )?;
    validate_sorted_unique(&policy.permitted_region_profiles, "region profiles")?;
    validate_sorted_unique(&policy.permitted_entity_definitions, "entity definitions")?;
    validate_sorted_unique(&policy.permitted_mode_definitions, "mode definitions")?;
    validate_sorted_unique(&policy.permitted_anchor_definitions, "anchor definitions")?;
    Ok(())
}

fn validate_definitions<Id: Copy + Ord + std::fmt::Debug>(
    definitions: &[StableDefinition<Id>],
    name: &str,
) -> Result<(), String> {
    if definitions.is_empty()
        || definitions.windows(2).any(|pair| pair[0].id >= pair[1].id)
        || definitions
            .iter()
            .any(|definition| !valid_key(&definition.key))
    {
        return Err(format!("invalid or noncanonical {name} catalog"));
    }
    Ok(())
}

fn validate_sorted_unique<T: Ord>(values: &[T], name: &str) -> Result<(), String> {
    if values.is_empty() || values.windows(2).any(|pair| pair[0] >= pair[1]) {
        return Err(format!("{name} must be nonempty, sorted, and unique"));
    }
    Ok(())
}

fn validate_recipe_references(
    recipe: &MapRecipe,
    catalog: &MapContentCatalog,
) -> Result<(), String> {
    if !catalog
        .presentation_themes
        .iter()
        .any(|definition| definition.id == recipe.presentation_theme_id)
        || !catalog
            .mode_definitions
            .iter()
            .any(|definition| definition.id == recipe.mode_definition_id)
    {
        return Err("map recipe references an unknown theme or mode".to_string());
    }
    if catalog
        .object_catalog
        .theme(recipe.presentation_theme_id)
        .is_none()
    {
        return Err("map recipe references an unknown object theme".to_string());
    }
    if recipe.geometry.iter().any(|placement| {
        let object = catalog
            .object_catalog
            .object(placement.object_definition_id);
        let variant = catalog.object_catalog.resolve_variant(
            recipe.presentation_theme_id,
            placement.object_definition_id,
            placement.visual_variant_id,
        );
        object.is_none_or(|object| object.role != MapObjectRole::ObstacleIndestructible)
            || variant.is_none()
            || !catalog
                .policy
                .permitted_collision_profiles
                .contains(&placement.collision_profile_id)
            || placement
                .presentation_profile_id
                .is_some_and(|id| !catalog.policy.permitted_presentation_profiles.contains(&id))
    }) || recipe.visuals.iter().any(|placement| {
        !catalog
            .policy
            .permitted_presentation_profiles
            .contains(&placement.presentation_profile_id)
    }) || recipe.regions.iter().any(|placement| {
        !catalog
            .policy
            .permitted_region_profiles
            .contains(&placement.profile_id)
            || !catalog
                .policy
                .permitted_presentation_profiles
                .contains(&placement.presentation_profile_id)
    }) || recipe.entities.iter().any(|placement| {
        let object = catalog
            .object_catalog
            .object(placement.object_definition_id);
        let variant = catalog.object_catalog.resolve_variant(
            recipe.presentation_theme_id,
            placement.object_definition_id,
            placement.visual_variant_id,
        );
        object.is_none_or(|object| object.role != MapObjectRole::Decoration)
            || variant.is_none()
            || !catalog
                .policy
                .permitted_entity_definitions
                .contains(&placement.definition_id)
            || !catalog
                .policy
                .permitted_presentation_profiles
                .contains(&placement.presentation_profile_id)
    }) || recipe.mode_anchors.iter().any(|placement| {
        !catalog
            .policy
            .permitted_anchor_definitions
            .contains(&placement.definition_id)
    }) {
        return Err("map recipe references an unsupported stable definition".to_string());
    }
    Ok(())
}

fn valid_key(value: &str) -> bool {
    let bytes = value.as_bytes();
    !bytes.is_empty()
        && bytes.len() <= 32
        && bytes.first().is_some_and(u8::is_ascii_alphanumeric)
        && bytes.last().is_some_and(u8::is_ascii_alphanumeric)
        && bytes
            .iter()
            .all(|byte| byte.is_ascii_lowercase() || byte.is_ascii_digit() || *byte == b'-')
        && !value.contains("--")
}

fn valid_display_name(value: &str) -> bool {
    !value.is_empty() && value.len() <= 64 && value.chars().all(|value| !value.is_control())
}

mod resolver;
mod terrain;
use resolver::normalize_recipe;
pub use resolver::overlaps_geometry;
pub use resolver::resolve_map_recipe;
pub use terrain::{
    DESTRUCTIBLE_TERRAIN_REGION_PROFILE, InitialTerrainLayout, resolve_initial_terrain,
};

#[cfg(test)]
mod tests;
