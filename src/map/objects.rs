//! Stable game-object taxonomy and render-neutral visual-variant compatibility.

use super::{
    CollisionProfileId, EntityDefinitionId, MapObjectDefinitionId, MapPresentationProfileId,
    MapPresentationThemeId, MapShape, MapVisualVariantId,
};
use bevy::prelude::{FromWorld, Resource, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

pub const MAP_OBJECT_CATALOG_SCHEMA_VERSION: u16 = 3;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapObjectRole {
    Surface,
    Boundary,
    ObstacleIndestructible,
    ObstacleDestructible,
    Decoration,
    TerrainDestructible,
    Marker,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum MapObjectFitPolicy {
    Exact,
    Modular,
    Contained,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub enum MapObjectPlacementBinding {
    Obstacle {
        collision_profile_id: CollisionProfileId,
        presentation_profile_id: Option<MapPresentationProfileId>,
    },
    Decoration {
        definition_id: EntityDefinitionId,
        presentation_profile_id: MapPresentationProfileId,
    },
    Generated,
    Unsupported,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MapObjectDefinition {
    pub id: MapObjectDefinitionId,
    pub key: String,
    pub display_name: String,
    pub role: MapObjectRole,
    pub default_footprint: Option<MapShape>,
    pub rotation_step_degrees: u16,
    pub compatible_visual_variants: Vec<MapVisualVariantId>,
    pub tags: Vec<String>,
    pub placement_binding: MapObjectPlacementBinding,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MapVisualVariantDefinition {
    pub id: MapVisualVariantId,
    pub key: String,
    pub compatible_objects: Vec<MapObjectDefinitionId>,
    pub native_footprint: Vec2,
    pub fit: MapObjectFitPolicy,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct MapThemeVariantDefault {
    pub object_definition_id: MapObjectDefinitionId,
    pub visual_variant_id: MapVisualVariantId,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub struct MapThemeDefinition {
    pub id: MapPresentationThemeId,
    pub key: String,
    pub defaults: Vec<MapThemeVariantDefault>,
    pub outside_dressing_anchor_variants: Vec<MapVisualVariantId>,
    pub outside_dressing_detail_variants: Vec<MapVisualVariantId>,
}

#[derive(Serialize, Deserialize, Clone, Debug, Default, PartialEq)]
pub struct MapObjectCatalog {
    pub schema_version: u16,
    pub objects: Vec<MapObjectDefinition>,
    pub visual_variants: Vec<MapVisualVariantDefinition>,
    pub themes: Vec<MapThemeDefinition>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapObjectSource {
    schema_version: u16,
    objects: Vec<MapObjectDefinition>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapVisualVariantSource {
    schema_version: u16,
    visual_variants: Vec<MapVisualVariantDefinition>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapThemeSource {
    schema_version: u16,
    themes: Vec<MapThemeDefinition>,
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct MapObjectCatalogResource(pub MapObjectCatalog);

impl FromWorld for MapObjectCatalogResource {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self(MapObjectCatalog::embedded().expect("embedded map-object catalog is valid"))
    }
}

impl MapObjectCatalog {
    pub fn embedded() -> Result<Self, String> {
        let objects: MapObjectSource =
            ron::from_str(include_str!("../../content/v4/map_objects.ron")).map_err(|error| {
                format!("embedded map-object definitions parse failed: {error}")
            })?;
        let variants: MapVisualVariantSource =
            ron::from_str(include_str!("../../content/v4/map_visual_variants.ron"))
                .map_err(|error| format!("embedded map visual variants parse failed: {error}"))?;
        let themes: MapThemeSource = ron::from_str(include_str!("../../content/v4/map_themes.ron"))
            .map_err(|error| format!("embedded map themes parse failed: {error}"))?;
        if objects.schema_version != MAP_OBJECT_CATALOG_SCHEMA_VERSION
            || variants.schema_version != MAP_OBJECT_CATALOG_SCHEMA_VERSION
            || themes.schema_version != MAP_OBJECT_CATALOG_SCHEMA_VERSION
        {
            return Err("map object source schema versions do not match".to_string());
        }
        let catalog = Self {
            schema_version: MAP_OBJECT_CATALOG_SCHEMA_VERSION,
            objects: objects.objects,
            visual_variants: variants.visual_variants,
            themes: themes.themes,
        };
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != MAP_OBJECT_CATALOG_SCHEMA_VERSION
            || self.objects.is_empty()
            || self.visual_variants.is_empty()
            || self.themes.is_empty()
        {
            return Err("invalid map-object catalog schema or empty definitions".to_string());
        }
        validate_sorted_ids_and_keys(
            self.objects
                .iter()
                .map(|value| (value.id.0, value.key.as_str())),
            "map objects",
        )?;
        validate_sorted_ids_and_keys(
            self.visual_variants
                .iter()
                .map(|value| (value.id.0, value.key.as_str())),
            "visual variants",
        )?;
        validate_sorted_ids_and_keys(
            self.themes
                .iter()
                .map(|value| (value.id.0, value.key.as_str())),
            "map themes",
        )?;

        let objects: BTreeMap<_, _> = self.objects.iter().map(|value| (value.id, value)).collect();
        let variants: BTreeMap<_, _> = self
            .visual_variants
            .iter()
            .map(|value| (value.id, value))
            .collect();
        validate_objects(&self.objects, &variants)?;
        validate_variants(&self.visual_variants, &objects)?;
        validate_themes(&self.themes, &objects, &variants)?;
        Ok(())
    }

    #[must_use]
    pub fn object(&self, id: MapObjectDefinitionId) -> Option<&MapObjectDefinition> {
        self.objects.iter().find(|value| value.id == id)
    }

    #[must_use]
    pub fn variant(&self, id: MapVisualVariantId) -> Option<&MapVisualVariantDefinition> {
        self.visual_variants.iter().find(|value| value.id == id)
    }

    #[must_use]
    pub fn theme(&self, id: MapPresentationThemeId) -> Option<&MapThemeDefinition> {
        self.themes.iter().find(|value| value.id == id)
    }

    #[cfg(feature = "client")]
    pub(crate) fn themes(&self) -> impl Iterator<Item = &MapThemeDefinition> {
        self.themes.iter()
    }

    #[must_use]
    pub fn resolve_variant(
        &self,
        theme: MapPresentationThemeId,
        object: MapObjectDefinitionId,
        explicit: Option<MapVisualVariantId>,
    ) -> Option<MapVisualVariantId> {
        let definition = self.object(object)?;
        if let Some(explicit) = explicit {
            return definition
                .compatible_visual_variants
                .contains(&explicit)
                .then_some(explicit);
        }
        self.theme(theme)?
            .defaults
            .iter()
            .find(|value| value.object_definition_id == object)
            .map(|value| value.visual_variant_id)
    }
}

fn validate_objects(
    objects: &[MapObjectDefinition],
    variants: &BTreeMap<MapVisualVariantId, &MapVisualVariantDefinition>,
) -> Result<(), String> {
    for object in objects {
        if object.display_name.trim().is_empty()
            || object.rotation_step_degrees == 0
            || 360 % object.rotation_step_degrees != 0
            || object.compatible_visual_variants.is_empty()
            || !is_sorted_unique(&object.compatible_visual_variants)
            || object.tags.iter().any(|tag| !valid_key(tag))
            || !valid_role_footprint(object.role, object.default_footprint)
            || !valid_role_binding(object.role, &object.placement_binding)
        {
            return Err(format!("invalid map-object definition: {}", object.key));
        }
        for variant_id in &object.compatible_visual_variants {
            let Some(variant) = variants.get(variant_id) else {
                return Err(format!("object {} references unknown variant", object.key));
            };
            if !variant.compatible_objects.contains(&object.id) {
                return Err(format!(
                    "object/variant compatibility is not reciprocal: {}",
                    object.key
                ));
            }
        }
    }
    Ok(())
}

fn validate_variants(
    variants: &[MapVisualVariantDefinition],
    objects: &BTreeMap<MapObjectDefinitionId, &MapObjectDefinition>,
) -> Result<(), String> {
    for variant in variants {
        if variant.compatible_objects.is_empty()
            || !is_sorted_unique(&variant.compatible_objects)
            || !variant.native_footprint.is_finite()
            || variant.native_footprint.min_element() <= 0.0
            || variant
                .compatible_objects
                .iter()
                .any(|id| !objects.contains_key(id))
        {
            return Err(format!(
                "invalid visual-variant definition: {}",
                variant.key
            ));
        }
    }
    Ok(())
}

fn validate_themes(
    themes: &[MapThemeDefinition],
    objects: &BTreeMap<MapObjectDefinitionId, &MapObjectDefinition>,
    variants: &BTreeMap<MapVisualVariantId, &MapVisualVariantDefinition>,
) -> Result<(), String> {
    for theme in themes {
        if theme.defaults.is_empty()
            || !is_sorted_unique(&theme.outside_dressing_anchor_variants)
            || !is_sorted_unique(&theme.outside_dressing_detail_variants)
        {
            return Err(format!("invalid map theme: {}", theme.key));
        }
        let mut default_objects = BTreeSet::new();
        for default in &theme.defaults {
            let (Some(object), Some(variant)) = (
                objects.get(&default.object_definition_id),
                variants.get(&default.visual_variant_id),
            ) else {
                return Err(format!("theme {} references unknown default", theme.key));
            };
            if !default_objects.insert(default.object_definition_id)
                || !object.compatible_visual_variants.contains(&variant.id)
            {
                return Err(format!("theme {} has incompatible defaults", theme.key));
            }
        }
        if theme
            .outside_dressing_anchor_variants
            .iter()
            .chain(&theme.outside_dressing_detail_variants)
            .any(|id| !variants.contains_key(id))
        {
            return Err(format!("theme {} references unknown dressing", theme.key));
        }
    }
    Ok(())
}

fn valid_role_binding(role: MapObjectRole, binding: &MapObjectPlacementBinding) -> bool {
    matches!(
        (role, binding),
        (
            MapObjectRole::ObstacleIndestructible,
            MapObjectPlacementBinding::Obstacle { .. }
        ) | (
            MapObjectRole::Decoration,
            MapObjectPlacementBinding::Decoration { .. }
        ) | (
            MapObjectRole::Surface | MapObjectRole::Boundary,
            MapObjectPlacementBinding::Generated
        ) | (
            MapObjectRole::ObstacleDestructible
                | MapObjectRole::TerrainDestructible
                | MapObjectRole::Marker,
            MapObjectPlacementBinding::Unsupported
        )
    )
}

fn validate_sorted_ids_and_keys<'a>(
    values: impl Iterator<Item = (u16, &'a str)>,
    label: &str,
) -> Result<(), String> {
    let values = values.collect::<Vec<_>>();
    if values.iter().any(|(id, key)| *id == 0 || !valid_key(key))
        || values.windows(2).any(|pair| pair[0].0 >= pair[1].0)
    {
        return Err(format!(
            "{label} must have sorted unique IDs and valid keys"
        ));
    }
    let keys = values.iter().map(|(_, key)| *key).collect::<BTreeSet<_>>();
    if keys.len() != values.len() {
        return Err(format!("{label} keys must be unique"));
    }
    Ok(())
}

fn is_sorted_unique<T: Ord>(values: &[T]) -> bool {
    !values.is_empty() && !values.windows(2).any(|pair| pair[0] >= pair[1])
}

fn valid_key(value: &str) -> bool {
    !value.is_empty()
        && value.len() <= 64
        && value.bytes().all(|byte| {
            byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'-' | b'_' | b'.')
        })
}

fn valid_role_footprint(role: MapObjectRole, footprint: Option<MapShape>) -> bool {
    match (role, footprint) {
        (
            MapObjectRole::ObstacleIndestructible | MapObjectRole::ObstacleDestructible,
            Some(MapShape::Rectangle { half_extents }),
        ) => half_extents.is_finite() && half_extents.min_element() > 0.0,
        (
            MapObjectRole::ObstacleIndestructible | MapObjectRole::ObstacleDestructible,
            Some(MapShape::Circle { radius }),
        ) => radius.is_finite() && radius > 0.0,
        (MapObjectRole::ObstacleIndestructible | MapObjectRole::ObstacleDestructible, None) => {
            false
        }
        (_, _) => true,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_object_catalog_is_valid_and_supports_mixed_wall_styles() {
        let catalog = MapObjectCatalog::embedded().unwrap();
        let wall = MapObjectDefinitionId(1);
        assert_eq!(
            catalog.resolve_variant(MapPresentationThemeId(1), wall, None),
            Some(MapVisualVariantId(1))
        );
        assert_eq!(
            catalog.resolve_variant(MapPresentationThemeId(1), wall, Some(MapVisualVariantId(2))),
            Some(MapVisualVariantId(2))
        );
    }

    #[test]
    fn one_tree_asset_can_present_decorative_or_blocking_objects() {
        let catalog = MapObjectCatalog::embedded().unwrap();
        let tree_variant = catalog.variant(MapVisualVariantId(5)).unwrap();
        assert!(
            tree_variant
                .compatible_objects
                .contains(&MapObjectDefinitionId(4))
        );
        assert!(
            tree_variant
                .compatible_objects
                .contains(&MapObjectDefinitionId(104))
        );
    }
}
