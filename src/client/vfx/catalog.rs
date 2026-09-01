//! Validated client-only VFX request mappings and renderer profiles.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

#[cfg(test)]
use super::recipes::VfxParticleProfile;
use super::recipes::{
    MAX_VFX_LAYERS, MAX_VFX_PARTICLE_LAYERS, MAX_VFX_RECIPES, ResolvedVfxRecipe, VfxMeshLayer,
    VfxParticleLayer, VfxReadabilityClass, VfxRecipe,
};
use super::request::valid_vfx_request_key;
use super::{CombatVisualSignature, VfxAccent, VfxCarrier, VfxContextOverride, VfxPhase};

pub(super) const VFX_CATALOG: &str = include_str!("../../../assets/catalogs/vfx.ron");
const VFX_SCHEMA_VERSION: u16 = 8;
const MAX_VFX_MAPPINGS: usize = 64;
const MAX_VFX_FIXED_WORLD: f32 =
    crate::map::MAX_MAP_DIMENSION_CELLS as f32 * crate::map::MAP_CELL_SIZE_WORLD;
const MAX_VFX_RADIUS_MULTIPLIER: f32 = 16.0;

#[derive(Clone, Copy, Debug, Serialize, PartialEq, Eq)]
pub(crate) struct VfxCatalogDiagnostics {
    pub(crate) request_mappings: usize,
    pub(crate) style_mappings: usize,
    pub(crate) mapping_capacity: usize,
    pub(crate) recipes: usize,
    pub(crate) recipe_capacity: usize,
    pub(crate) mesh_layers: usize,
    pub(crate) mesh_layer_capacity: usize,
    pub(crate) particle_layers: usize,
    pub(crate) particle_layer_capacity: usize,
    pub(crate) maximum_recipe_mesh_layers: usize,
    pub(crate) maximum_recipe_particle_layers: usize,
}

/// Stable evidence identity for the exact embedded client-only presentation catalog.
///
/// This deliberately does not contribute to the gameplay-content or protocol fingerprint. It
/// lets native captures and `VfxLab` previews name the presentation input that produced a frame.
pub(crate) fn embedded_vfx_catalog_fingerprint() -> u64 {
    crate::content::fnv1a64(VFX_CATALOG.as_bytes())
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) enum VfxRendererFamily {
    Sphere,
    GroundRing,
    GroundThinRing,
    GroundDisc,
    GroundSpark,
    MeleeArc,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) enum VfxLifetime {
    Millis(u32),
    AuthoritativeDeadline,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) enum VfxMaterialKey {
    EffectMuzzle,
    EffectImpact,
    EffectDamage,
    NeutralBright,
    NeutralSoft,
    ScanArea,
    ConcealmentBright,
    ConcealmentSoft,
    DemolitionArea,
    PickupGlow,
    ColdBright,
    ColdSoft,
    FireBright,
    FireSoft,
    PoisonBright,
    PoisonSoft,
    RestorationBright,
    RestorationSoft,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub(crate) enum VfxScalePolicy {
    FixedWorld(f32),
    AuthoritativeRadius(f32),
    GeometryReach(f32),
}

impl VfxScalePolicy {
    pub(crate) fn resolve(
        self,
        authoritative_radius: Option<f32>,
        geometry_reach: Option<f32>,
    ) -> Option<f32> {
        match self {
            Self::FixedWorld(scale) => Some(scale),
            Self::AuthoritativeRadius(multiplier) => authoritative_radius
                .map(|radius| radius * multiplier)
                .filter(|scale| scale.is_finite() && *scale > 0.0),
            Self::GeometryReach(multiplier) => geometry_reach
                .map(|reach| reach * multiplier)
                .filter(|scale| scale.is_finite() && *scale > 0.0),
        }
    }

    pub(super) const fn requires_authoritative_radius(self) -> bool {
        matches!(self, Self::AuthoritativeRadius(_))
    }

    pub(super) const fn requires_geometry_reach(self) -> bool {
        matches!(self, Self::GeometryReach(_))
    }

    pub(super) fn has_valid_bounds(self) -> bool {
        let value = match self {
            Self::FixedWorld(value)
            | Self::AuthoritativeRadius(value)
            | Self::GeometryReach(value) => value,
        };
        value.is_finite()
            && value > 0.0
            && match self {
                Self::FixedWorld(_) => value <= MAX_VFX_FIXED_WORLD,
                Self::AuthoritativeRadius(_) | Self::GeometryReach(_) => {
                    value <= MAX_VFX_RADIUS_MULTIPLIER
                }
            }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub(crate) enum VfxAnchorPolicy {
    FixedWorldHeight(f32),
    AuthoritativeRadiusHeight(f32),
    GroundOffset(f32),
}

impl VfxAnchorPolicy {
    pub(crate) fn resolve_height(self, authoritative_radius: Option<f32>) -> Option<f32> {
        match self {
            Self::FixedWorldHeight(height) | Self::GroundOffset(height) => Some(height),
            Self::AuthoritativeRadiusHeight(multiplier) => authoritative_radius
                .map(|radius| radius * multiplier)
                .filter(|height| height.is_finite() && *height >= 0.0),
        }
    }

    pub(super) const fn requires_authoritative_radius(self) -> bool {
        matches!(self, Self::AuthoritativeRadiusHeight(_))
    }

    pub(super) fn has_valid_bounds(self) -> bool {
        let value = match self {
            Self::FixedWorldHeight(value)
            | Self::AuthoritativeRadiusHeight(value)
            | Self::GroundOffset(value) => value,
        };
        value.is_finite()
            && value >= 0.0
            && match self {
                Self::FixedWorldHeight(_) | Self::GroundOffset(_) => value <= MAX_VFX_FIXED_WORLD,
                Self::AuthoritativeRadiusHeight(_) => value <= MAX_VFX_RADIUS_MULTIPLIER,
            }
    }
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct VfxCatalogSource {
    schema_version: u16,
    default_recipe: String,
    mappings: Vec<VfxRequestMapping>,
    style_mappings: Vec<VfxStyleMapping>,
    recipes: Vec<VfxRecipe>,
    mesh_layers: Vec<VfxMeshLayer>,
    #[serde(default)]
    particle_layers: Vec<VfxParticleLayer>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct VfxRequestMapping {
    key: String,
    recipe: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct VfxStyleMapping {
    key: String,
    accent: VfxAccent,
    #[serde(default)]
    carrier: Option<VfxCarrier>,
    phase: Option<VfxPhase>,
    ultimate: bool,
    recipe: String,
}

impl VfxStyleMapping {
    fn matches(&self, key: &str, signature: &CombatVisualSignature) -> bool {
        self.key == key
            && signature.accents.contains(&Some(self.accent))
            && self
                .carrier
                .is_none_or(|carrier| carrier == signature.carrier)
            && self.phase.is_none_or(|phase| phase == signature.phase)
            && (!self.ultimate || signature.context.is_some_and(is_ultimate_context))
    }

    const fn specificity(&self) -> u8 {
        1 + self.carrier.is_some() as u8 + self.phase.is_some() as u8 + self.ultimate as u8
    }
}

const fn is_ultimate_context(context: VfxContextOverride) -> bool {
    matches!(
        context,
        VfxContextOverride::Ultimate(_)
            | VfxContextOverride::Deployable { .. }
            | VfxContextOverride::StickyUltimateSecondary
            | VfxContextOverride::ElementalField(_)
            | VfxContextOverride::DemolitionStrike
            | VfxContextOverride::Sentry
    )
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct VfxRecipeRequirements {
    pub(super) authoritative_radius: bool,
    pub(super) authoritative_deadline: bool,
}

#[derive(Clone, Debug)]
pub(super) struct VfxCatalog {
    default_recipe: String,
    mappings: BTreeMap<String, String>,
    style_mappings: Vec<VfxStyleMapping>,
    recipes: BTreeMap<String, VfxRecipe>,
    mesh_layers: BTreeMap<String, VfxMeshLayer>,
    particle_layers: BTreeMap<String, VfxParticleLayer>,
}

impl VfxCatalog {
    pub(super) fn embedded() -> Result<Self, String> {
        Self::from_ron(VFX_CATALOG)
    }

    pub(super) fn from_ron(source: &str) -> Result<Self, String> {
        let source: VfxCatalogSource =
            ron::from_str(source).map_err(|error| format!("VFX catalog parse failed: {error}"))?;
        Self::from_source(source)
    }

    #[allow(
        clippy::too_many_lines,
        reason = "catalog sealing validates the complete bounded cross-reference graph atomically"
    )]
    pub(super) fn from_source(source: VfxCatalogSource) -> Result<Self, String> {
        if source.schema_version != VFX_SCHEMA_VERSION {
            return Err(format!(
                "unsupported VFX catalog schema {}, expected {VFX_SCHEMA_VERSION}",
                source.schema_version
            ));
        }
        if source.recipes.len() > MAX_VFX_RECIPES {
            return Err("VFX catalog exceeds recipe capacity".into());
        }
        if source.mesh_layers.len() > MAX_VFX_LAYERS {
            return Err("VFX catalog exceeds mesh-layer capacity".into());
        }
        if source.particle_layers.len() > MAX_VFX_PARTICLE_LAYERS {
            return Err("VFX catalog exceeds particle-layer capacity".into());
        }
        if source
            .mappings
            .len()
            .saturating_add(source.style_mappings.len())
            > MAX_VFX_MAPPINGS
        {
            return Err("VFX catalog exceeds request-mapping capacity".into());
        }

        let mut mesh_layers = BTreeMap::new();
        for layer in source.mesh_layers {
            layer.validate()?;
            if mesh_layers.insert(layer.id.clone(), layer).is_some() {
                return Err("duplicate VFX mesh-layer id".to_string());
            }
        }
        let mut particle_layers = BTreeMap::new();
        for layer in source.particle_layers {
            layer.validate()?;
            if mesh_layers.contains_key(&layer.id)
                || particle_layers.insert(layer.id.clone(), layer).is_some()
            {
                return Err("duplicate VFX layer id".to_string());
            }
        }
        let mut recipes = BTreeMap::new();
        for recipe in source.recipes {
            recipe.validate()?;
            if recipes.insert(recipe.id.clone(), recipe).is_some() {
                return Err("duplicate VFX recipe id".to_string());
            }
        }
        if !recipes.contains_key(&source.default_recipe) {
            return Err("VFX default recipe reference is missing".to_string());
        }
        for recipe in recipes.values() {
            for layer in &recipe.layers {
                if !mesh_layers.contains_key(layer) {
                    return Err(format!(
                        "VFX recipe {} references missing mesh layer {layer}",
                        recipe.id
                    ));
                }
            }
            for layer in &recipe.particle_layers {
                if !particle_layers.contains_key(layer) {
                    return Err(format!(
                        "VFX recipe {} references missing particle layer {layer}",
                        recipe.id
                    ));
                }
            }
            for reference in [
                Some(&recipe.fallback_recipe),
                recipe.reduced_recipe.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                if !recipes.contains_key(reference) {
                    return Err(format!(
                        "VFX recipe {} references missing recipe {reference}",
                        recipe.id
                    ));
                }
            }
        }
        validate_recipe_reference_graphs(&recipes, &mesh_layers)?;
        validate_reduced_readability(&recipes, &mesh_layers)?;
        if recipe_requires_deadline(&recipes[&source.default_recipe], &mesh_layers) {
            return Err("VFX default recipe must use fixed layer durations".to_string());
        }

        let mut mappings = BTreeMap::new();
        for mapping in source.mappings {
            if !valid_vfx_request_key(&mapping.key) {
                return Err(format!("invalid VFX request mapping key: {}", mapping.key));
            }
            if !recipes.contains_key(&mapping.recipe) {
                return Err(format!(
                    "VFX request {} references missing recipe {}",
                    mapping.key, mapping.recipe
                ));
            }
            if mappings
                .insert(mapping.key.clone(), mapping.recipe)
                .is_some()
            {
                return Err(format!("duplicate VFX request mapping: {}", mapping.key));
            }
        }
        if mappings.is_empty() {
            return Err("VFX catalog must contain at least one request mapping".into());
        }
        let mut style_selectors = std::collections::BTreeSet::new();
        for mapping in &source.style_mappings {
            if !valid_vfx_request_key(&mapping.key) {
                return Err(format!("invalid VFX style mapping key: {}", mapping.key));
            }
            if !mappings.contains_key(&mapping.key) {
                return Err(format!(
                    "VFX style mapping {} has no key-only fallback",
                    mapping.key
                ));
            }
            if !recipes.contains_key(&mapping.recipe) {
                return Err(format!(
                    "VFX style mapping {} references missing recipe {}",
                    mapping.key, mapping.recipe
                ));
            }
            if !style_selectors.insert((
                mapping.key.clone(),
                mapping.accent,
                mapping.carrier,
                mapping.phase,
                mapping.ultimate,
            )) {
                return Err(format!("duplicate VFX style mapping: {}", mapping.key));
            }
        }
        Ok(Self {
            default_recipe: source.default_recipe,
            mappings,
            style_mappings: source.style_mappings,
            recipes,
            mesh_layers,
            particle_layers,
        })
    }

    pub(super) fn mapping_keys(&self) -> impl Iterator<Item = &str> {
        self.mappings.keys().map(String::as_str)
    }

    pub(super) fn particle_layers(&self) -> impl Iterator<Item = &VfxParticleLayer> {
        self.particle_layers.values()
    }

    pub(super) fn diagnostics(&self) -> VfxCatalogDiagnostics {
        VfxCatalogDiagnostics {
            request_mappings: self.mappings.len(),
            style_mappings: self.style_mappings.len(),
            mapping_capacity: MAX_VFX_MAPPINGS,
            recipes: self.recipes.len(),
            recipe_capacity: MAX_VFX_RECIPES,
            mesh_layers: self.mesh_layers.len(),
            mesh_layer_capacity: MAX_VFX_LAYERS,
            particle_layers: self.particle_layers.len(),
            particle_layer_capacity: MAX_VFX_PARTICLE_LAYERS,
            maximum_recipe_mesh_layers: self
                .recipes
                .values()
                .map(|recipe| recipe.layers.len())
                .max()
                .unwrap_or_default(),
            maximum_recipe_particle_layers: self
                .recipes
                .values()
                .map(|recipe| recipe.particle_layers.len())
                .max()
                .unwrap_or_default(),
        }
    }

    pub(super) fn requirements(&self, key: &str) -> Option<VfxRecipeRequirements> {
        let fallback = self.mapped_recipe(key)?;
        let recipes = std::iter::once(fallback).chain(
            self.style_mappings
                .iter()
                .filter(|mapping| mapping.key == key)
                .map(|mapping| &self.recipes[&mapping.recipe]),
        );
        Some(
            recipes.fold(VfxRecipeRequirements::default(), |requirements, recipe| {
                let reduced = recipe
                    .reduced_recipe
                    .as_deref()
                    .and_then(|id| self.recipes.get(id));
                [Some(recipe), reduced].into_iter().flatten().fold(
                    requirements,
                    |requirements, recipe| {
                        let requirements = requirements.including(recipe, &self.mesh_layers);
                        if recipe_requires_deadline(recipe, &self.mesh_layers) {
                            requirements.including(
                                &self.recipes[&recipe.fallback_recipe],
                                &self.mesh_layers,
                            )
                        } else {
                            requirements
                        }
                    },
                )
            }),
        )
    }

    #[cfg(test)]
    pub(super) fn insert_test_mapping(&mut self, key: &str, recipe_id: &str) -> Result<(), String> {
        if !valid_vfx_request_key(key) {
            return Err(format!("invalid VFX request mapping key: {key}"));
        }
        if self.mappings.len() >= MAX_VFX_MAPPINGS {
            return Err("VFX catalog exceeds request-mapping capacity".into());
        }
        if !self.recipes.contains_key(recipe_id) {
            return Err(format!(
                "VFX request {key} references missing recipe {recipe_id}"
            ));
        }
        if self
            .mappings
            .insert(key.to_owned(), recipe_id.to_owned())
            .is_some()
        {
            return Err(format!("duplicate VFX request mapping: {key}"));
        }
        Ok(())
    }

    pub(super) fn resolve(
        &self,
        key: &str,
        signature: &CombatVisualSignature,
        reduced: bool,
        authoritative_deadline_available: bool,
    ) -> Option<ResolvedVfxRecipe> {
        let recipe = self
            .style_mappings
            .iter()
            .filter(|mapping| mapping.matches(key, signature))
            .max_by_key(|mapping| mapping.specificity())
            .and_then(|mapping| self.recipes.get(&mapping.recipe))
            .or_else(|| self.mapped_recipe(key))?;
        let recipe = if reduced {
            recipe
                .reduced_recipe
                .as_deref()
                .and_then(|id| self.recipes.get(id))
                .unwrap_or(recipe)
        } else {
            recipe
        };
        let recipe = if authoritative_deadline_available
            || !recipe_requires_deadline(recipe, &self.mesh_layers)
        {
            recipe
        } else {
            self.recipes
                .get(&recipe.fallback_recipe)
                .or_else(|| self.recipes.get(&self.default_recipe))?
        };
        self.resolved(recipe)
    }

    fn mapped_recipe(&self, key: &str) -> Option<&VfxRecipe> {
        self.mappings
            .get(key)
            .and_then(|recipe_id| self.recipes.get(recipe_id))
    }

    pub(super) fn resolved_fallback(
        &self,
        recipe: &ResolvedVfxRecipe,
    ) -> Option<ResolvedVfxRecipe> {
        self.recipes
            .get(&recipe.fallback_recipe)
            .and_then(|fallback| self.resolved(fallback))
    }

    fn resolved(&self, recipe: &VfxRecipe) -> Option<ResolvedVfxRecipe> {
        Some(ResolvedVfxRecipe {
            id: recipe.id.clone(),
            layers: recipe
                .layers
                .iter()
                .map(|id| self.mesh_layers.get(id).cloned())
                .collect::<Option<Vec<_>>>()?,
            particle_layers: recipe
                .particle_layers
                .iter()
                .map(|id| self.particle_layers.get(id).cloned())
                .collect::<Option<Vec<_>>>()?,
            concurrency_cap: recipe.concurrency_cap,
            fallback_recipe: recipe.fallback_recipe.clone(),
            reduced_recipe: recipe.reduced_recipe.clone(),
        })
    }
}

impl VfxRecipeRequirements {
    fn including(mut self, recipe: &VfxRecipe, layers: &BTreeMap<String, VfxMeshLayer>) -> Self {
        for layer in &recipe.layers {
            let layer = &layers[layer];
            self.authoritative_radius |= layer.requires_authoritative_radius();
            self.authoritative_deadline |= layer.requires_authoritative_deadline();
        }
        self
    }
}

fn recipe_requires_deadline(recipe: &VfxRecipe, layers: &BTreeMap<String, VfxMeshLayer>) -> bool {
    recipe
        .layers
        .iter()
        .any(|layer| layers[layer].requires_authoritative_deadline())
}

fn validate_recipe_reference_graphs(
    recipes: &BTreeMap<String, VfxRecipe>,
    layers: &BTreeMap<String, VfxMeshLayer>,
) -> Result<(), String> {
    for start in recipes.values() {
        let mut visited = std::collections::BTreeSet::new();
        let mut current = Some(start);
        while let Some(recipe) = current {
            if !visited.insert(recipe.id.as_str()) {
                return Err(format!("VFX reduced-recipe cycle reaches {}", recipe.id));
            }
            current = recipe
                .reduced_recipe
                .as_deref()
                .and_then(|id| recipes.get(id));
        }
    }
    for start in recipes.values() {
        if !recipe_requires_deadline(start, layers) {
            continue;
        }
        let mut visited = std::collections::BTreeSet::new();
        let mut current = start;
        loop {
            if !visited.insert(current.id.as_str()) {
                return Err(format!(
                    "VFX authoritative fallback cycle reaches {}",
                    current.id
                ));
            }
            let fallback = &recipes[&current.fallback_recipe];
            if !recipe_requires_deadline(fallback, layers) {
                break;
            }
            current = fallback;
        }
    }
    Ok(())
}

fn validate_reduced_readability(
    recipes: &BTreeMap<String, VfxRecipe>,
    layers: &BTreeMap<String, VfxMeshLayer>,
) -> Result<(), String> {
    for recipe in recipes.values() {
        let critical = recipe.layers.iter().any(|layer| {
            matches!(
                layers[layer].readability,
                VfxReadabilityClass::CriticalGeometry
            )
        });
        let Some(reduced) = recipe
            .reduced_recipe
            .as_deref()
            .and_then(|id| recipes.get(id))
        else {
            continue;
        };
        let reduced_critical = reduced.layers.iter().any(|layer| {
            matches!(
                layers[layer].readability,
                VfxReadabilityClass::CriticalGeometry
            )
        });
        if critical && !reduced_critical {
            return Err(format!(
                "VFX reduced recipe {} drops critical geometry from {}",
                reduced.id, recipe.id
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUILTIN_MAPPINGS: [(&str, &str); 17] = [
        ("ability.demolition-strike", "demolition"),
        ("ability.elemental-field", "elemental_field"),
        ("ability.forced-reveal", "forced_reveal"),
        ("ability.reveal-scan", "reveal_scan"),
        ("ability.self-cloak", "cloak_activation"),
        ("combat.damage", "damage"),
        ("combat.impact", "impact"),
        ("combat.muzzle", "muzzle"),
        ("combat.reset", "reset"),
        ("heist.critical", "heist_critical"),
        ("heist.damaged", "heist_damage"),
        ("heist.destroyed", "heist_destroyed"),
        ("pickup.collected", "pickup_collect"),
        ("pickup.expired", "pickup_expire"),
        ("pickup.spawned", "pickup_spawn"),
        ("world-object.damaged", "world_damage"),
        ("world-object.explosion", "world_explosion"),
    ];

    #[test]
    fn embedded_catalog_preserves_all_seventeen_recipe_mappings() {
        let catalog = VfxCatalog::embedded().unwrap();
        assert_eq!(catalog.mappings.len(), BUILTIN_MAPPINGS.len());
        for (key, recipe_id) in BUILTIN_MAPPINGS {
            assert_eq!(catalog.mapped_recipe(key).unwrap().id, recipe_id);
        }

        let signature = CombatVisualSignature::neutral_point(1, bevy::prelude::Vec2::ZERO);
        let normal = catalog
            .resolve("combat.muzzle", &signature, false, false)
            .unwrap();
        let reduced = catalog
            .resolve("combat.muzzle", &signature, true, false)
            .unwrap();
        assert_eq!(normal.id, "muzzle");
        assert_eq!(normal.layers.len(), 1);
        assert_eq!(normal.layers[0].material, VfxMaterialKey::EffectMuzzle);
        assert_eq!(normal.layers[0].scale, VfxScalePolicy::FixedWorld(8.0));
        assert_eq!(
            normal.layers[0].anchor,
            VfxAnchorPolicy::FixedWorldHeight(3.6)
        );
        assert_eq!(normal.layers[0].timeline.duration, VfxLifetime::Millis(180));
        assert_eq!(reduced.layers[0].scale, VfxScalePolicy::FixedWorld(5.2));
        assert_eq!(
            reduced.layers[0].timeline.duration,
            VfxLifetime::Millis(100)
        );
    }

    #[test]
    fn typed_style_selection_prefers_accent_phase_and_ultimate_specificity() {
        let catalog = VfxCatalog::embedded().unwrap();
        let mut cold = CombatVisualSignature::neutral_point(1, bevy::prelude::Vec2::ZERO);
        cold.accents = [Some(VfxAccent::Cold), None];
        cold.phase = VfxPhase::Activation;
        assert_eq!(
            catalog
                .resolve("combat.muzzle", &cold, false, false)
                .unwrap()
                .id,
            "cold_muzzle"
        );
        assert_eq!(
            catalog
                .resolve("combat.muzzle", &cold, true, false)
                .unwrap()
                .id,
            "cold_muzzle_reduced"
        );

        let mut fire = CombatVisualSignature::neutral_point(2, bevy::prelude::Vec2::ZERO);
        fire.accents = [Some(VfxAccent::Fire), None];
        fire.phase = VfxPhase::Impact;
        fire.context = Some(VfxContextOverride::Ultimate(
            crate::builds::UltimateDefinitionId(4),
        ));
        assert_eq!(
            catalog
                .resolve("combat.impact", &fire, false, false)
                .unwrap()
                .id,
            "fire_ultimate_impact"
        );

        let neutral = CombatVisualSignature::neutral_point(3, bevy::prelude::Vec2::ZERO);
        assert_eq!(
            catalog
                .resolve("combat.impact", &neutral, false, false)
                .unwrap()
                .id,
            "impact"
        );

        let mut lobbed = neutral;
        lobbed.carrier = VfxCarrier::LobbedProjectile;
        lobbed.phase = VfxPhase::Impact;
        lobbed.geometry = super::super::VfxGeometry::circle(bevy::prelude::Vec2::ZERO, 150.0);
        assert_eq!(
            catalog
                .resolve("combat.impact", &lobbed, false, false)
                .unwrap()
                .id,
            "lob_explosion"
        );
        let lobbed_normal = catalog
            .resolve("combat.impact", &lobbed, false, false)
            .unwrap();
        assert_eq!(lobbed_normal.layers.len(), 5);
        assert_eq!(lobbed_normal.particle_layers.len(), 2);
        assert_eq!(
            lobbed_normal.layers[0].renderer,
            VfxRendererFamily::GroundThinRing
        );
        assert_eq!(
            lobbed_normal.layers[0].readability,
            VfxReadabilityClass::CriticalGeometry
        );
        assert_eq!(
            catalog
                .resolve("combat.impact", &lobbed, true, false)
                .unwrap()
                .id,
            "lob_explosion_reduced"
        );
        assert!(
            catalog
                .resolve("combat.impact", &lobbed, true, false)
                .unwrap()
                .particle_layers
                .is_empty()
        );
        lobbed.accents = [Some(VfxAccent::Fire), None];
        lobbed.context = Some(VfxContextOverride::Ultimate(
            crate::builds::UltimateDefinitionId(4),
        ));
        assert_eq!(
            catalog
                .resolve("combat.impact", &lobbed, false, false)
                .unwrap()
                .id,
            "fire_lob_explosion_ultimate"
        );
    }

    #[test]
    fn demolition_recipe_retains_boundary_and_degrades_without_particles() {
        let catalog = VfxCatalog::embedded().unwrap();
        let mut signature = CombatVisualSignature::neutral_point(6, bevy::prelude::Vec2::ZERO);
        signature.accents = [Some(VfxAccent::Demolition), None];
        signature.carrier = VfxCarrier::PersistentArea;
        signature.phase = VfxPhase::Activation;
        signature.context = Some(VfxContextOverride::DemolitionStrike);

        let normal = catalog
            .resolve("ability.demolition-strike", &signature, false, false)
            .unwrap();
        assert_eq!(normal.id, "demolition");
        assert_eq!(normal.layers.len(), 6);
        assert_eq!(normal.particle_layers.len(), 2);
        assert_eq!(
            normal.layers[0].readability,
            VfxReadabilityClass::CriticalGeometry
        );
        assert_eq!(
            normal.layers[0].scale,
            VfxScalePolicy::AuthoritativeRadius(1.0)
        );

        let reduced = catalog
            .resolve("ability.demolition-strike", &signature, true, false)
            .unwrap();
        assert_eq!(reduced.id, "demolition_reduced");
        assert_eq!(reduced.layers.len(), 3);
        assert!(reduced.particle_layers.is_empty());
    }

    #[test]
    fn reveal_scan_keeps_deadline_boundary_and_fixed_activation_waves() {
        let catalog = VfxCatalog::embedded().unwrap();
        let mut signature = CombatVisualSignature::neutral_point(7, bevy::prelude::Vec2::ZERO);
        signature.accents = [Some(VfxAccent::Reveal), None];
        signature.carrier = VfxCarrier::PersistentArea;
        signature.phase = VfxPhase::Activation;
        signature.context = Some(VfxContextOverride::RevealScan);

        let normal = catalog
            .resolve("ability.reveal-scan", &signature, false, true)
            .unwrap();
        assert_eq!(normal.id, "reveal_scan");
        assert_eq!(normal.layers.len(), 4);
        assert_eq!(
            normal.layers[0].timeline.duration,
            VfxLifetime::AuthoritativeDeadline
        );
        assert!(
            normal.layers[1..]
                .iter()
                .all(|layer| matches!(layer.timeline.duration, VfxLifetime::Millis(_)))
        );

        let reduced = catalog
            .resolve("ability.reveal-scan", &signature, true, true)
            .unwrap();
        assert_eq!(reduced.id, "reveal_scan_reduced");
        assert_eq!(reduced.layers.len(), 2);
        assert_eq!(
            reduced.layers[0].readability,
            VfxReadabilityClass::CriticalGeometry
        );
        assert!(reduced.particle_layers.is_empty());
    }

    #[test]
    fn sentry_transients_route_fire_expiry_and_interruption_semantics() {
        let catalog = VfxCatalog::embedded().unwrap();
        let mut signature = CombatVisualSignature::neutral_point(8, bevy::prelude::Vec2::ZERO);
        signature.carrier = VfxCarrier::Deployable;
        signature.phase = VfxPhase::Activation;
        signature.context = Some(VfxContextOverride::Sentry);

        let fire = catalog
            .resolve("combat.muzzle", &signature, false, false)
            .unwrap();
        assert_eq!(fire.id, "sentry_fire");
        assert_eq!(fire.layers.len(), 4);
        assert_eq!(fire.particle_layers.len(), 1);
        let reduced_fire = catalog
            .resolve("combat.muzzle", &signature, true, false)
            .unwrap();
        assert_eq!(reduced_fire.id, "sentry_fire_reduced");
        assert!(reduced_fire.particle_layers.is_empty());

        signature.phase = VfxPhase::Expiry;
        let expiry = catalog
            .resolve("combat.impact", &signature, false, false)
            .unwrap();
        assert_eq!(expiry.id, "sentry_expiry");
        assert!(expiry.particle_layers.is_empty());

        signature.phase = VfxPhase::Interruption;
        let interruption = catalog
            .resolve("combat.impact", &signature, false, false)
            .unwrap();
        assert_eq!(interruption.id, "sentry_interruption");
        assert_eq!(interruption.particle_layers.len(), 1);
        assert_eq!(
            catalog
                .resolve("combat.impact", &signature, true, false)
                .unwrap()
                .id,
            "sentry_interruption_reduced"
        );
    }

    #[test]
    fn concealment_transients_route_lifecycle_and_reveal_semantics() {
        let catalog = VfxCatalog::embedded().unwrap();
        let mut signature = CombatVisualSignature::neutral_point(9, bevy::prelude::Vec2::ZERO);
        signature.carrier = VfxCarrier::AttachedStatus;
        signature.accents = [Some(VfxAccent::Concealment), None];
        signature.phase = VfxPhase::Activation;

        let activation = catalog
            .resolve("ability.self-cloak", &signature, false, false)
            .unwrap();
        assert_eq!(activation.id, "cloak_activation");
        assert_eq!(activation.layers.len(), 3);
        let reduced = catalog
            .resolve("ability.self-cloak", &signature, true, false)
            .unwrap();
        assert_eq!(reduced.id, "cloak_activation_reduced");
        assert_eq!(reduced.layers.len(), 2);

        signature.phase = VfxPhase::Expiry;
        assert_eq!(
            catalog
                .resolve("ability.self-cloak", &signature, false, false)
                .unwrap()
                .id,
            "cloak_expiry"
        );
        signature.phase = VfxPhase::Interruption;
        assert_eq!(
            catalog
                .resolve("ability.self-cloak", &signature, false, false)
                .unwrap()
                .id,
            "cloak_interruption"
        );

        signature.phase = VfxPhase::Activation;
        signature.accents = [Some(VfxAccent::Reveal), None];
        let reveal = catalog
            .resolve("ability.forced-reveal", &signature, false, false)
            .unwrap();
        assert_eq!(reveal.id, "forced_reveal");
        assert_eq!(reveal.layers.len(), 4);
    }

    #[test]
    fn neutral_straight_transients_use_layered_recipes_without_changing_fallbacks() {
        let catalog = VfxCatalog::embedded().unwrap();
        let mut signature = CombatVisualSignature::neutral_point(11, bevy::prelude::Vec2::ZERO);
        signature.carrier = VfxCarrier::StraightProjectile;
        signature.phase = VfxPhase::Activation;

        let muzzle = catalog
            .resolve("combat.muzzle", &signature, false, false)
            .unwrap();
        let reduced_muzzle = catalog
            .resolve("combat.muzzle", &signature, true, false)
            .unwrap();
        assert_eq!(muzzle.id, "neutral_straight_muzzle");
        assert_eq!(muzzle.layers.len(), 3);
        assert_eq!(muzzle.particle_layers.len(), 1);
        assert_eq!(
            muzzle.particle_layers[0].profile,
            VfxParticleProfile::NeutralSparks
        );
        assert_eq!(reduced_muzzle.id, "neutral_straight_muzzle_reduced");
        assert!(reduced_muzzle.particle_layers.is_empty());

        signature.phase = VfxPhase::Impact;
        let impact = catalog
            .resolve("combat.impact", &signature, false, false)
            .unwrap();
        let reduced_impact = catalog
            .resolve("combat.impact", &signature, true, false)
            .unwrap();
        assert_eq!(impact.id, "neutral_straight_impact");
        assert_eq!(impact.layers.len(), 3);
        assert_eq!(impact.particle_layers.len(), 1);
        assert_eq!(reduced_impact.id, "neutral_straight_impact_reduced");
        assert!(reduced_impact.particle_layers.is_empty());

        let fallback = CombatVisualSignature::neutral_point(12, bevy::prelude::Vec2::ZERO);
        assert_eq!(
            catalog
                .resolve("combat.impact", &fallback, false, false)
                .unwrap()
                .id,
            "impact"
        );
    }

    #[test]
    fn fighter_outcome_recipes_are_layered_and_degrade_without_particles() {
        let catalog = VfxCatalog::embedded().unwrap();
        let mut signature = CombatVisualSignature::neutral_point(13, bevy::prelude::Vec2::ZERO);
        signature.carrier = VfxCarrier::AttachedStatus;
        signature.phase = VfxPhase::Impact;

        let damage = catalog
            .resolve("combat.damage", &signature, false, false)
            .unwrap();
        let reduced_damage = catalog
            .resolve("combat.damage", &signature, true, false)
            .unwrap();
        assert_eq!(damage.id, "damage");
        assert_eq!(damage.layers.len(), 3);
        assert_eq!(damage.particle_layers.len(), 1);
        assert_eq!(reduced_damage.id, "damage_reduced");
        assert_eq!(reduced_damage.layers.len(), 2);
        assert!(reduced_damage.particle_layers.is_empty());

        signature.phase = VfxPhase::Interruption;
        let defeat = catalog
            .resolve("combat.damage", &signature, false, false)
            .unwrap();
        let reduced_defeat = catalog
            .resolve("combat.damage", &signature, true, false)
            .unwrap();
        assert_eq!(defeat.id, "defeat");
        assert_eq!(defeat.layers.len(), 4);
        assert_eq!(defeat.particle_layers.len(), 1);
        assert_eq!(reduced_defeat.id, "defeat_reduced");
        assert_eq!(reduced_defeat.layers.len(), 2);
        assert!(reduced_defeat.particle_layers.is_empty());

        signature.phase = VfxPhase::Expiry;
        let reset = catalog
            .resolve("combat.reset", &signature, false, false)
            .unwrap();
        let reduced_reset = catalog
            .resolve("combat.reset", &signature, true, false)
            .unwrap();
        assert_eq!(reset.id, "reset");
        assert_eq!(reset.layers.len(), 4);
        assert_eq!(reset.particle_layers.len(), 1);
        assert_eq!(reduced_reset.id, "reset_reduced");
        assert_eq!(reduced_reset.layers.len(), 2);
        assert!(reduced_reset.particle_layers.is_empty());
    }

    #[test]
    fn normal_elemental_impacts_add_bounded_particles_while_reduced_omits_them() {
        let catalog = VfxCatalog::embedded().unwrap();
        for (accent, expected_profile) in [
            (VfxAccent::Cold, VfxParticleProfile::ColdFragments),
            (VfxAccent::Fire, VfxParticleProfile::FireEmbers),
            (VfxAccent::Poison, VfxParticleProfile::PoisonBubbles),
            (
                VfxAccent::Restoration,
                VfxParticleProfile::RestorationPetals,
            ),
        ] {
            let mut signature = CombatVisualSignature::neutral_point(1, bevy::prelude::Vec2::ZERO);
            signature.accents = [Some(accent), None];
            signature.phase = VfxPhase::Impact;
            let normal = catalog
                .resolve("combat.impact", &signature, false, false)
                .unwrap();
            let reduced = catalog
                .resolve("combat.impact", &signature, true, false)
                .unwrap();

            assert_eq!(normal.particle_layers.len(), 1);
            assert_eq!(normal.particle_layers[0].profile, expected_profile);
            assert!(normal.particle_layers[0].spawn.amount() <= normal.particle_layers[0].capacity);
            assert!(reduced.particle_layers.is_empty());
        }
    }

    #[test]
    fn poison_lob_and_restoration_area_select_authored_transient_recipes() {
        let catalog = VfxCatalog::embedded().unwrap();
        let mut poison = CombatVisualSignature::neutral_point(7, bevy::prelude::Vec2::ZERO);
        poison.accents = [Some(VfxAccent::Poison), None];
        poison.carrier = VfxCarrier::LobbedProjectile;
        poison.phase = VfxPhase::Impact;
        poison.geometry = super::super::VfxGeometry::circle(bevy::prelude::Vec2::ZERO, 120.0);
        let poison_recipe = catalog
            .resolve("combat.impact", &poison, false, false)
            .unwrap();
        assert_eq!(poison_recipe.id, "poison_lob_explosion");
        assert_eq!(
            poison_recipe.layers[0].renderer,
            VfxRendererFamily::GroundThinRing
        );

        let mut restoration = CombatVisualSignature::neutral_point(8, bevy::prelude::Vec2::ZERO);
        restoration.accents = [Some(VfxAccent::Restoration), None];
        restoration.carrier = VfxCarrier::PersistentArea;
        restoration.phase = VfxPhase::Activation;
        restoration.geometry = super::super::VfxGeometry::circle(bevy::prelude::Vec2::ZERO, 160.0);
        restoration.context = Some(VfxContextOverride::Ultimate(
            crate::builds::UltimateDefinitionId(5),
        ));
        let restoration_recipe = catalog
            .resolve("ability.elemental-field", &restoration, false, false)
            .unwrap();
        assert_eq!(restoration_recipe.id, "restoration_area_activation");
        assert!(
            restoration_recipe
                .layers
                .iter()
                .any(|layer| layer.renderer == VfxRendererFamily::GroundSpark)
        );
    }

    #[test]
    fn strict_schema_keys_references_values_and_capacity_are_validated() {
        for (needle, replacement) in [
            ("schema_version: 8", "schema_version: 7"),
            ("key: \"combat.muzzle\"", "key: \"Combat/Muzzle\""),
            ("recipe: \"muzzle\"", "recipe: \"missing\""),
            ("scale: FixedWorld(8.0)", "scale: FixedWorld(0.0)"),
            ("duration: Millis(180)", "duration: Millis(0)"),
            ("concurrency_cap: 96", "concurrency_cap: 97"),
        ] {
            assert!(
                VfxCatalog::from_ron(&VFX_CATALOG.replacen(needle, replacement, 1)).is_err(),
                "{replacement}"
            );
        }
    }

    #[test]
    fn duplicate_mapping_recipe_and_layer_ids_are_rejected() {
        let mut duplicate_mapping: VfxCatalogSource = ron::from_str(VFX_CATALOG).unwrap();
        duplicate_mapping
            .mappings
            .push(duplicate_mapping.mappings[0].clone());
        assert!(VfxCatalog::from_source(duplicate_mapping).is_err());

        let mut duplicate_recipe: VfxCatalogSource = ron::from_str(VFX_CATALOG).unwrap();
        duplicate_recipe
            .recipes
            .push(duplicate_recipe.recipes[0].clone());
        assert!(VfxCatalog::from_source(duplicate_recipe).is_err());

        let mut duplicate_layer: VfxCatalogSource = ron::from_str(VFX_CATALOG).unwrap();
        duplicate_layer
            .mesh_layers
            .push(duplicate_layer.mesh_layers[0].clone());
        assert!(VfxCatalog::from_source(duplicate_layer).is_err());

        let mut duplicate_particle: VfxCatalogSource = ron::from_str(VFX_CATALOG).unwrap();
        duplicate_particle
            .particle_layers
            .push(duplicate_particle.particle_layers[0].clone());
        assert!(VfxCatalog::from_source(duplicate_particle).is_err());

        let mut duplicate_style: VfxCatalogSource = ron::from_str(VFX_CATALOG).unwrap();
        duplicate_style
            .style_mappings
            .push(duplicate_style.style_mappings[0].clone());
        assert!(VfxCatalog::from_source(duplicate_style).is_err());
    }

    #[test]
    fn style_mapping_requires_a_key_only_fallback() {
        let mut missing_fallback: VfxCatalogSource = ron::from_str(VFX_CATALOG).unwrap();
        missing_fallback
            .mappings
            .retain(|mapping| mapping.key != "combat.muzzle");
        assert!(
            VfxCatalog::from_source(missing_fallback)
                .unwrap_err()
                .contains("has no key-only fallback")
        );
    }

    #[test]
    fn missing_layers_expansion_bounds_and_reference_cycles_are_rejected() {
        let missing = VFX_CATALOG.replacen("layers: [\"muzzle\"]", "layers: [\"missing\"]", 1);
        assert!(
            VfxCatalog::from_ron(&missing)
                .unwrap_err()
                .contains("missing mesh layer")
        );

        let missing_particle = VFX_CATALOG.replacen(
            "particle_layers: [\"cold_impact_fragments\"]",
            "particle_layers: [\"missing_particle\"]",
            1,
        );
        assert!(
            VfxCatalog::from_ron(&missing_particle)
                .unwrap_err()
                .contains("missing particle layer")
        );

        let excessive = VFX_CATALOG.replacen(
            "layers: [\"muzzle\"]",
            "layers: [\"muzzle\", \"muzzle\", \"muzzle\", \"muzzle\", \"muzzle\", \"muzzle\", \"muzzle\", \"muzzle\", \"muzzle\"]",
            1,
        );
        assert!(
            VfxCatalog::from_ron(&excessive)
                .unwrap_err()
                .contains("unsafe runtime values")
        );

        let reduced_cycle = VFX_CATALOG.replacen(
            "id: \"muzzle_reduced\", layers: [\"muzzle_reduced\"], concurrency_cap: 96, fallback_recipe: \"impact\", reduced_recipe: None",
            "id: \"muzzle_reduced\", layers: [\"muzzle_reduced\"], concurrency_cap: 96, fallback_recipe: \"impact\", reduced_recipe: Some(\"muzzle\")",
            1,
        );
        assert!(
            VfxCatalog::from_ron(&reduced_cycle)
                .unwrap_err()
                .contains("reduced-recipe cycle")
        );

        let fallback_cycle = VFX_CATALOG.replacen(
            "id: \"reveal_scan\", layers: [\"reveal_scan_boundary\", \"reveal_scan_wave\", \"reveal_scan_echo\", \"reveal_scan_origin\"], concurrency_cap: 48, fallback_recipe: \"impact\"",
            "id: \"reveal_scan\", layers: [\"reveal_scan_boundary\", \"reveal_scan_wave\", \"reveal_scan_echo\", \"reveal_scan_origin\"], concurrency_cap: 48, fallback_recipe: \"reveal_scan\"",
            1,
        );
        assert!(
            VfxCatalog::from_ron(&fallback_cycle)
                .unwrap_err()
                .contains("authoritative fallback cycle")
        );

        let mut dropped_critical: VfxCatalogSource = ron::from_str(VFX_CATALOG).unwrap();
        dropped_critical
            .mesh_layers
            .iter_mut()
            .find(|layer| layer.id == "explosion_exact_boundary_reduced")
            .unwrap()
            .readability = VfxReadabilityClass::Identity;
        assert!(
            VfxCatalog::from_source(dropped_critical)
                .unwrap_err()
                .contains("drops critical geometry")
        );
    }

    #[test]
    fn frozen_recipe_and_layer_capacities_are_enforced() {
        let mut recipe_capacity: VfxCatalogSource = ron::from_str(VFX_CATALOG).unwrap();
        let template = recipe_capacity.recipes[0].clone();
        while recipe_capacity.recipes.len() <= MAX_VFX_RECIPES {
            let mut recipe = template.clone();
            recipe.id = format!("extra_recipe_{}", recipe_capacity.recipes.len());
            recipe.reduced_recipe = None;
            recipe_capacity.recipes.push(recipe);
        }
        assert!(
            VfxCatalog::from_source(recipe_capacity)
                .unwrap_err()
                .contains("recipe capacity")
        );

        let mut layer_capacity: VfxCatalogSource = ron::from_str(VFX_CATALOG).unwrap();
        let template = layer_capacity.mesh_layers[0].clone();
        while layer_capacity.mesh_layers.len() <= MAX_VFX_LAYERS {
            let mut layer = template.clone();
            layer.id = format!("extra_layer_{}", layer_capacity.mesh_layers.len());
            layer_capacity.mesh_layers.push(layer);
        }
        assert!(
            VfxCatalog::from_source(layer_capacity)
                .unwrap_err()
                .contains("mesh-layer capacity")
        );

        let mut particle_capacity: VfxCatalogSource = ron::from_str(VFX_CATALOG).unwrap();
        let template = particle_capacity.particle_layers[0].clone();
        while particle_capacity.particle_layers.len() <= MAX_VFX_PARTICLE_LAYERS {
            let mut layer = template.clone();
            layer.id = format!("extra_particle_{}", particle_capacity.particle_layers.len());
            particle_capacity.particle_layers.push(layer);
        }
        assert!(
            VfxCatalog::from_source(particle_capacity)
                .unwrap_err()
                .contains("particle-layer capacity")
        );
    }

    #[test]
    fn recipe_requirements_are_derived_without_a_closed_family_switch() {
        assert_eq!(
            VfxCatalog::embedded()
                .unwrap()
                .requirements("ability.reveal-scan"),
            Some(VfxRecipeRequirements {
                authoritative_radius: true,
                authoritative_deadline: true,
            })
        );
    }

    #[test]
    fn recipe_requirements_include_reachable_deadline_fallback_capabilities() {
        let source = VFX_CATALOG
            .replacen(
                "id: \"reveal_scan\", layers: [\"reveal_scan_boundary\", \"reveal_scan_wave\", \"reveal_scan_echo\", \"reveal_scan_origin\"], concurrency_cap: 48, fallback_recipe: \"impact\"",
                "id: \"reveal_scan\", layers: [\"reveal_scan_boundary\", \"reveal_scan_wave\", \"reveal_scan_echo\", \"reveal_scan_origin\"], concurrency_cap: 48, fallback_recipe: \"elemental_field\"",
                1,
            )
            .replacen(
                "id: \"reveal_scan_boundary\", renderer: GroundThinRing, material: ScanArea, scale: AuthoritativeRadius(1.0)",
                "id: \"reveal_scan_boundary\", renderer: GroundThinRing, material: ScanArea, scale: FixedWorld(10.0)",
                1,
            );
        let catalog = VfxCatalog::from_ron(&source).unwrap();

        assert_eq!(
            catalog.requirements("ability.reveal-scan"),
            Some(VfxRecipeRequirements {
                authoritative_radius: true,
                authoritative_deadline: true,
            })
        );
    }
}
