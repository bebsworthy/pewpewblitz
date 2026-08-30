use bevy::prelude::*;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

const VFX_CATALOG: &str = include_str!("../../../../assets/catalogs/vfx.ron");
const VFX_SCHEMA_VERSION: u16 = 1;
const MAX_VFX_LIFETIME_MILLIS: u32 = 10_000;
const MAX_VFX_CONCURRENCY: usize = 96;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum VfxCueFamily {
    CombatMuzzle,
    CombatImpact,
    CombatDamage,
    CombatReset,
    RevealScan,
    ElementalField,
    DemolitionStrike,
    WorldObjectDamaged,
    WorldObjectExploded,
    PickupSpawned,
    PickupCollected,
    PickupExpired,
    HeistDamaged,
    HeistCritical,
    HeistDestroyed,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub(super) enum VfxRendererFamily {
    Sphere,
    GroundRing,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub(super) enum VfxLifetime {
    Millis(u32),
    AuthoritativeDeadline,
}

#[derive(Clone, Debug, Deserialize)]
struct VfxCatalogSource {
    schema_version: u16,
    default_profile: String,
    mappings: Vec<VfxFamilyMapping>,
    profiles: Vec<VfxProfile>,
}

#[derive(Clone, Debug, Deserialize)]
struct VfxFamilyMapping {
    family: VfxCueFamily,
    profile: String,
}

#[derive(Clone, Debug, Deserialize)]
pub(super) struct VfxProfile {
    pub id: String,
    pub renderer: VfxRendererFamily,
    pub material: String,
    pub scale_multiplier: f32,
    pub lifetime: VfxLifetime,
    pub concurrency_cap: usize,
    pub fallback_profile: String,
    pub reduced_profile: Option<String>,
}

#[derive(Resource, Clone, Debug)]
pub(in super::super) struct VfxCatalog {
    default_profile: String,
    mappings: BTreeMap<VfxCueFamily, String>,
    profiles: BTreeMap<String, VfxProfile>,
}

impl VfxCatalog {
    pub(in super::super) fn embedded() -> Result<Self, String> {
        Self::from_ron(VFX_CATALOG)
    }

    fn from_ron(source: &str) -> Result<Self, String> {
        let source: VfxCatalogSource =
            ron::from_str(source).map_err(|error| format!("VFX catalog parse failed: {error}"))?;
        Self::from_source(source)
    }

    fn from_source(source: VfxCatalogSource) -> Result<Self, String> {
        if source.schema_version != VFX_SCHEMA_VERSION {
            return Err(format!(
                "unsupported VFX catalog schema {}, expected {VFX_SCHEMA_VERSION}",
                source.schema_version
            ));
        }
        let mut profiles = BTreeMap::new();
        for profile in source.profiles {
            validate_profile(&profile)?;
            if profiles.insert(profile.id.clone(), profile).is_some() {
                return Err("duplicate VFX profile id".to_string());
            }
        }
        if !profiles.contains_key(&source.default_profile) {
            return Err("VFX default profile reference is missing".to_string());
        }
        for profile in profiles.values() {
            for reference in [
                Some(&profile.fallback_profile),
                profile.reduced_profile.as_ref(),
            ]
            .into_iter()
            .flatten()
            {
                if !profiles.contains_key(reference) {
                    return Err(format!(
                        "VFX profile {} references missing profile {reference}",
                        profile.id
                    ));
                }
            }
        }
        let mut mappings = BTreeMap::new();
        for mapping in source.mappings {
            if !profiles.contains_key(&mapping.profile) {
                return Err(format!(
                    "VFX family {:?} references missing profile {}",
                    mapping.family, mapping.profile
                ));
            }
            if mappings.insert(mapping.family, mapping.profile).is_some() {
                return Err(format!(
                    "duplicate VFX family mapping: {:?}",
                    mapping.family
                ));
            }
        }
        Ok(Self {
            default_profile: source.default_profile,
            mappings,
            profiles,
        })
    }

    pub(super) fn resolve(&self, family: VfxCueFamily, reduced: bool) -> &VfxProfile {
        let requested = self.mappings.get(&family).unwrap_or(&self.default_profile);
        let profile = self.resolve_profile(requested);
        if reduced {
            profile
                .reduced_profile
                .as_deref()
                .map_or(profile, |id| self.resolve_profile(id))
        } else {
            profile
        }
    }

    fn resolve_profile(&self, requested: &str) -> &VfxProfile {
        let mut next = requested;
        let mut visited = BTreeSet::new();
        loop {
            let Some(profile) = self.profiles.get(next) else {
                return &self.profiles[&self.default_profile];
            };
            if valid_runtime_profile(profile) {
                return profile;
            }
            if !visited.insert(next) {
                return &self.profiles[&self.default_profile];
            }
            next = &profile.fallback_profile;
        }
    }
}

fn validate_profile(profile: &VfxProfile) -> Result<(), String> {
    if profile.id.trim().is_empty() || profile.fallback_profile.trim().is_empty() {
        return Err("VFX profile ids and fallback references must be non-empty".to_string());
    }
    if !known_material(&profile.material) {
        return Err(format!(
            "VFX profile {} references unknown material {}",
            profile.id, profile.material
        ));
    }
    if !valid_runtime_profile(profile) {
        return Err(format!(
            "VFX profile {} has unsafe runtime values",
            profile.id
        ));
    }
    Ok(())
}

fn valid_runtime_profile(profile: &VfxProfile) -> bool {
    profile.scale_multiplier.is_finite()
        && profile.scale_multiplier > 0.0
        && profile.concurrency_cap > 0
        && profile.concurrency_cap <= MAX_VFX_CONCURRENCY
        && match profile.lifetime {
            VfxLifetime::Millis(millis) => millis > 0 && millis <= MAX_VFX_LIFETIME_MILLIS,
            VfxLifetime::AuthoritativeDeadline => true,
        }
}

pub(super) fn known_material(material: &str) -> bool {
    matches!(
        material,
        "effect_muzzle"
            | "effect_impact"
            | "effect_damage"
            | "scan_area"
            | "demolition_area"
            | "pickup_glow"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_validates_and_maps_reduced_profiles() {
        let catalog = VfxCatalog::embedded().unwrap();
        let normal = catalog.resolve(VfxCueFamily::CombatMuzzle, false);
        let reduced = catalog.resolve(VfxCueFamily::CombatMuzzle, true);
        assert_eq!(normal.material, "effect_muzzle");
        assert_eq!(normal.renderer, VfxRendererFamily::Sphere);
        assert_eq!(normal.lifetime, VfxLifetime::Millis(180));
        assert_eq!(normal.concurrency_cap, 96);
        assert_eq!(reduced.lifetime, VfxLifetime::Millis(100));
        assert!((reduced.scale_multiplier - 0.65).abs() < f32::EPSILON);
        assert_eq!(catalog.mappings.len(), 15);
    }

    #[test]
    fn missing_family_uses_deterministic_default() {
        let mut catalog = VfxCatalog::embedded().unwrap();
        catalog.mappings.remove(&VfxCueFamily::CombatReset);
        let resolved = catalog.resolve(VfxCueFamily::CombatReset, false);
        assert_eq!(resolved.id, catalog.default_profile);
    }

    #[test]
    fn invalid_material_reference_lifetime_and_cap_are_rejected() {
        for (needle, replacement) in [
            ("material: \"effect_muzzle\"", "material: \"missing\""),
            ("lifetime: Millis(180)", "lifetime: Millis(0)"),
            ("concurrency_cap: 96", "concurrency_cap: 97"),
        ] {
            let source = VFX_CATALOG.replacen(needle, replacement, 1);
            assert!(VfxCatalog::from_ron(&source).is_err(), "{replacement}");
        }
    }

    #[test]
    fn invalid_profile_references_are_rejected() {
        let source = VFX_CATALOG.replacen(
            "fallback_profile: \"impact\"",
            "fallback_profile: \"missing\"",
            1,
        );
        assert!(VfxCatalog::from_ron(&source).is_err());
    }
}
