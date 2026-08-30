use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

const VFX_CATALOG: &str = include_str!("../../../../assets/catalogs/vfx.ron");
const VFX_SCHEMA_VERSION: u16 = 2;
const MAX_VFX_LIFETIME_MILLIS: u32 = 10_000;
const MAX_VFX_CONCURRENCY: usize = 96;
const MAX_VFX_FIXED_WORLD: f32 =
    crate::map::MAX_MAP_DIMENSION_CELLS as f32 * crate::map::MAP_CELL_SIZE_WORLD;
const MAX_VFX_RADIUS_MULTIPLIER: f32 = 16.0;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
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

const ALL_VFX_CUE_FAMILIES: [VfxCueFamily; 15] = [
    VfxCueFamily::CombatMuzzle,
    VfxCueFamily::CombatImpact,
    VfxCueFamily::CombatDamage,
    VfxCueFamily::CombatReset,
    VfxCueFamily::RevealScan,
    VfxCueFamily::ElementalField,
    VfxCueFamily::DemolitionStrike,
    VfxCueFamily::WorldObjectDamaged,
    VfxCueFamily::WorldObjectExploded,
    VfxCueFamily::PickupSpawned,
    VfxCueFamily::PickupCollected,
    VfxCueFamily::PickupExpired,
    VfxCueFamily::HeistDamaged,
    VfxCueFamily::HeistCritical,
    VfxCueFamily::HeistDestroyed,
];

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(super) enum VfxRendererFamily {
    Sphere,
    GroundRing,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(super) enum VfxLifetime {
    Millis(u32),
    AuthoritativeDeadline,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(super) enum VfxMaterialKey {
    EffectMuzzle,
    EffectImpact,
    EffectDamage,
    ScanArea,
    DemolitionArea,
    PickupGlow,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub(super) enum VfxScalePolicy {
    FixedWorld(f32),
    AuthoritativeRadius(f32),
}

impl VfxScalePolicy {
    pub(super) fn resolve(self, authoritative_radius: Option<f32>) -> Option<f32> {
        match self {
            Self::FixedWorld(scale) => Some(scale),
            Self::AuthoritativeRadius(multiplier) => authoritative_radius
                .map(|radius| radius * multiplier)
                .filter(|scale| scale.is_finite() && *scale > 0.0),
        }
    }

    const fn requires_authoritative_radius(self) -> bool {
        matches!(self, Self::AuthoritativeRadius(_))
    }

    fn has_valid_bounds(self) -> bool {
        let value = match self {
            Self::FixedWorld(value) | Self::AuthoritativeRadius(value) => value,
        };
        value.is_finite()
            && value > 0.0
            && match self {
                Self::FixedWorld(_) => value <= MAX_VFX_FIXED_WORLD,
                Self::AuthoritativeRadius(_) => value <= MAX_VFX_RADIUS_MULTIPLIER,
            }
    }
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub(super) enum VfxAnchorPolicy {
    FixedWorldHeight(f32),
    AuthoritativeRadiusHeight(f32),
    GroundOffset(f32),
}

impl VfxAnchorPolicy {
    pub(super) fn resolve_height(self, authoritative_radius: Option<f32>) -> Option<f32> {
        match self {
            Self::FixedWorldHeight(height) | Self::GroundOffset(height) => Some(height),
            Self::AuthoritativeRadiusHeight(multiplier) => authoritative_radius
                .map(|radius| radius * multiplier)
                .filter(|height| height.is_finite() && *height >= 0.0),
        }
    }

    const fn requires_authoritative_radius(self) -> bool {
        matches!(self, Self::AuthoritativeRadiusHeight(_))
    }

    fn has_valid_bounds(self) -> bool {
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
struct VfxCatalogSource {
    schema_version: u16,
    default_profile: String,
    mappings: Vec<VfxFamilyMapping>,
    profiles: Vec<VfxProfile>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct VfxFamilyMapping {
    family: VfxCueFamily,
    profile: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(super) struct VfxProfile {
    pub id: String,
    pub renderer: VfxRendererFamily,
    pub material: VfxMaterialKey,
    pub scale: VfxScalePolicy,
    pub anchor: VfxAnchorPolicy,
    pub lifetime: VfxLifetime,
    pub concurrency_cap: usize,
    pub fallback_profile: String,
    pub reduced_profile: Option<String>,
}

#[derive(Resource, Clone, Debug)]
pub(in super::super) struct VfxCatalog {
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
        if matches!(
            profiles[&source.default_profile].lifetime,
            VfxLifetime::AuthoritativeDeadline
        ) {
            return Err("VFX default profile must use a fixed lifetime".to_string());
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
            if matches!(profile.lifetime, VfxLifetime::AuthoritativeDeadline)
                && matches!(
                    profiles[&profile.fallback_profile].lifetime,
                    VfxLifetime::AuthoritativeDeadline
                )
            {
                return Err(format!(
                    "VFX authoritative profile {} must fall back to a fixed lifetime",
                    profile.id
                ));
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
        if mappings.len() != ALL_VFX_CUE_FAMILIES.len()
            || ALL_VFX_CUE_FAMILIES
                .iter()
                .any(|family| !mappings.contains_key(family))
        {
            return Err(
                "VFX catalog must map every registered cue family exactly once".to_string(),
            );
        }
        validate_family_profiles(&mappings, &profiles)?;
        Ok(Self { mappings, profiles })
    }

    pub(super) fn resolve(&self, family: VfxCueFamily, reduced: bool) -> &VfxProfile {
        let requested = self
            .mappings
            .get(&family)
            .expect("validated VFX catalogs map every cue family");
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

    pub(super) fn resolve_for_request(
        &self,
        family: VfxCueFamily,
        reduced: bool,
        authoritative_deadline_available: bool,
    ) -> &VfxProfile {
        let profile = self.resolve(family, reduced);
        if authoritative_deadline_available
            || !matches!(profile.lifetime, VfxLifetime::AuthoritativeDeadline)
        {
            return profile;
        }
        self.resolve_profile(&profile.fallback_profile)
    }

    fn resolve_profile(&self, requested: &str) -> &VfxProfile {
        self.profiles
            .get(requested)
            .expect("validated VFX profile references remain available")
    }
}

fn validate_family_profiles(
    mappings: &BTreeMap<VfxCueFamily, String>,
    profiles: &BTreeMap<String, VfxProfile>,
) -> Result<(), String> {
    for (&family, profile_id) in mappings {
        let profile = &profiles[profile_id];
        let reduced = profile
            .reduced_profile
            .as_ref()
            .map(|reduced_id| &profiles[reduced_id]);
        if !family_supports_authoritative_deadline(family)
            && (matches!(profile.lifetime, VfxLifetime::AuthoritativeDeadline)
                || reduced.is_some_and(|profile| {
                    matches!(profile.lifetime, VfxLifetime::AuthoritativeDeadline)
                }))
        {
            return Err(format!(
                "VFX family {family:?} cannot use an authoritative-deadline profile"
            ));
        }
        for candidate in [Some(profile), reduced].into_iter().flatten() {
            if (candidate.scale.requires_authoritative_radius()
                || candidate.anchor.requires_authoritative_radius())
                && !family_supports_authoritative_radius(family)
            {
                return Err(format!(
                    "VFX family {family:?} cannot use authoritative-radius geometry"
                ));
            }
        }
    }
    Ok(())
}

const fn family_supports_authoritative_deadline(family: VfxCueFamily) -> bool {
    matches!(family, VfxCueFamily::RevealScan)
}

const fn family_supports_authoritative_radius(family: VfxCueFamily) -> bool {
    matches!(
        family,
        VfxCueFamily::RevealScan
            | VfxCueFamily::ElementalField
            | VfxCueFamily::DemolitionStrike
            | VfxCueFamily::WorldObjectExploded
    )
}

fn validate_profile(profile: &VfxProfile) -> Result<(), String> {
    if profile.id.trim().is_empty() || profile.fallback_profile.trim().is_empty() {
        return Err("VFX profile ids and fallback references must be non-empty".to_string());
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
    profile.scale.has_valid_bounds()
        && profile.anchor.has_valid_bounds()
        && matches!(
            (profile.renderer, profile.anchor),
            (
                VfxRendererFamily::Sphere,
                VfxAnchorPolicy::FixedWorldHeight(_)
                    | VfxAnchorPolicy::AuthoritativeRadiusHeight(_)
            ) | (
                VfxRendererFamily::GroundRing,
                VfxAnchorPolicy::GroundOffset(_)
            )
        )
        && profile.concurrency_cap > 0
        && profile.concurrency_cap <= MAX_VFX_CONCURRENCY
        && match profile.lifetime {
            VfxLifetime::Millis(millis) => millis > 0 && millis <= MAX_VFX_LIFETIME_MILLIS,
            VfxLifetime::AuthoritativeDeadline => true,
        }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_validates_and_maps_reduced_profiles() {
        let catalog = VfxCatalog::embedded().unwrap();
        let normal = catalog.resolve(VfxCueFamily::CombatMuzzle, false);
        let reduced = catalog.resolve(VfxCueFamily::CombatMuzzle, true);
        assert_eq!(normal.material, VfxMaterialKey::EffectMuzzle);
        assert_eq!(normal.renderer, VfxRendererFamily::Sphere);
        assert_eq!(normal.scale, VfxScalePolicy::FixedWorld(8.0));
        assert_eq!(normal.anchor, VfxAnchorPolicy::FixedWorldHeight(3.6));
        assert_eq!(normal.lifetime, VfxLifetime::Millis(180));
        assert_eq!(normal.concurrency_cap, 96);
        assert_eq!(reduced.lifetime, VfxLifetime::Millis(100));
        assert_eq!(reduced.scale, VfxScalePolicy::FixedWorld(5.2));
        assert_eq!(reduced.anchor, normal.anchor);
        assert_eq!(catalog.mappings.len(), ALL_VFX_CUE_FAMILIES.len());
    }

    #[test]
    fn catalog_source_round_trips_without_losing_policy() {
        let source: VfxCatalogSource = ron::from_str(VFX_CATALOG).unwrap();
        let encoded = ron::to_string(&source).unwrap();
        let decoded: VfxCatalogSource = ron::from_str(&encoded).unwrap();
        assert_eq!(decoded, source);
        VfxCatalog::from_source(decoded).unwrap();
    }

    #[test]
    fn bad_schema_unknown_material_invalid_geometry_lifetime_and_cap_are_rejected() {
        for (needle, replacement) in [
            ("schema_version: 2", "schema_version: 3"),
            ("material: EffectMuzzle", "material: Missing"),
            (
                "material: EffectMuzzle",
                "material: EffectMuzzle, scale_multiplier: 1.0",
            ),
            ("scale: FixedWorld(8.0)", "scale: FixedWorld(0.0)"),
            ("scale: FixedWorld(8.0)", "scale: FixedWorld(3.4e38)"),
            (
                "scale: AuthoritativeRadius(1.0)",
                "scale: AuthoritativeRadius(3.4e38)",
            ),
            ("anchor: FixedWorldHeight(3.6)", "anchor: GroundOffset(3.6)"),
            (
                "anchor: AuthoritativeRadiusHeight(0.45)",
                "anchor: AuthoritativeRadiusHeight(3.4e38)",
            ),
            ("lifetime: Millis(180)", "lifetime: Millis(0)"),
            ("concurrency_cap: 96", "concurrency_cap: 97"),
        ] {
            let source = VFX_CATALOG.replacen(needle, replacement, 1);
            assert!(VfxCatalog::from_ron(&source).is_err(), "{replacement}");
        }
        assert_eq!(
            VfxScalePolicy::AuthoritativeRadius(MAX_VFX_RADIUS_MULTIPLIER).resolve(Some(f32::MAX)),
            None
        );
        assert_eq!(
            VfxAnchorPolicy::AuthoritativeRadiusHeight(MAX_VFX_RADIUS_MULTIPLIER)
                .resolve_height(Some(f32::MAX)),
            None
        );
    }

    #[test]
    fn missing_profile_references_are_rejected() {
        let missing_fallback = VFX_CATALOG.replacen(
            "fallback_profile: \"impact\"",
            "fallback_profile: \"missing\"",
            1,
        );
        assert!(VfxCatalog::from_ron(&missing_fallback).is_err());
        let missing_mapping =
            VFX_CATALOG.replacen("(family: CombatReset, profile: \"reset\"),", "", 1);
        assert!(VfxCatalog::from_ron(&missing_mapping).is_err());
    }

    #[test]
    fn duplicate_profile_ids_and_family_mappings_are_rejected() {
        let mut duplicate_profile: VfxCatalogSource = ron::from_str(VFX_CATALOG).unwrap();
        duplicate_profile
            .profiles
            .push(duplicate_profile.profiles[0].clone());
        assert!(VfxCatalog::from_source(duplicate_profile).is_err());

        let mut duplicate_mapping: VfxCatalogSource = ron::from_str(VFX_CATALOG).unwrap();
        duplicate_mapping
            .mappings
            .push(duplicate_mapping.mappings[0].clone());
        assert!(VfxCatalog::from_source(duplicate_mapping).is_err());
    }

    #[test]
    fn deadline_and_authoritative_radius_profiles_are_restricted_to_compatible_cues() {
        let source = VFX_CATALOG.replacen(
            "(family: CombatMuzzle, profile: \"muzzle\")",
            "(family: CombatMuzzle, profile: \"reveal_scan\")",
            1,
        );
        assert!(VfxCatalog::from_ron(&source).is_err());

        let mut catalog = VfxCatalog::embedded().unwrap();
        catalog
            .mappings
            .insert(VfxCueFamily::CombatMuzzle, "reveal_scan".to_string());
        let resolved = catalog.resolve_for_request(VfxCueFamily::CombatMuzzle, false, false);
        assert_eq!(resolved.id, "impact");
        assert!(matches!(resolved.lifetime, VfxLifetime::Millis(_)));

        let radius_source = VFX_CATALOG.replacen(
            "scale: FixedWorld(8.0)",
            "scale: AuthoritativeRadius(1.0)",
            1,
        );
        assert!(VfxCatalog::from_ron(&radius_source).is_err());
    }
}
