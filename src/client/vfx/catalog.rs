//! Validated client-only VFX request mappings and renderer profiles.

use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;

use super::request::valid_vfx_request_key;

pub(super) const VFX_CATALOG: &str = include_str!("../../../assets/catalogs/vfx.ron");
const VFX_SCHEMA_VERSION: u16 = 3;
const MAX_VFX_MAPPINGS: usize = 32;
const MAX_VFX_PROFILES: usize = 64;
const MAX_VFX_LIFETIME_MILLIS: u32 = 10_000;
const MAX_VFX_CONCURRENCY: usize = 96;
const MAX_VFX_FIXED_WORLD: f32 =
    crate::map::MAX_MAP_DIMENSION_CELLS as f32 * crate::map::MAP_CELL_SIZE_WORLD;
const MAX_VFX_RADIUS_MULTIPLIER: f32 = 16.0;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq)]
pub(crate) enum VfxRendererFamily {
    Sphere,
    GroundRing,
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
    ScanArea,
    DemolitionArea,
    PickupGlow,
}

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq)]
pub(crate) enum VfxScalePolicy {
    FixedWorld(f32),
    AuthoritativeRadius(f32),
}

impl VfxScalePolicy {
    pub(crate) fn resolve(self, authoritative_radius: Option<f32>) -> Option<f32> {
        match self {
            Self::FixedWorld(scale) => Some(scale),
            Self::AuthoritativeRadius(multiplier) => authoritative_radius
                .map(|radius| radius * multiplier)
                .filter(|scale| scale.is_finite() && *scale > 0.0),
        }
    }

    pub(super) const fn requires_authoritative_radius(self) -> bool {
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
pub(super) struct VfxCatalogSource {
    schema_version: u16,
    default_profile: String,
    mappings: Vec<VfxRequestMapping>,
    profiles: Vec<VfxProfile>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct VfxRequestMapping {
    key: String,
    profile: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct VfxProfile {
    pub(crate) id: String,
    pub(crate) renderer: VfxRendererFamily,
    pub(crate) material: VfxMaterialKey,
    pub(crate) scale: VfxScalePolicy,
    pub(crate) anchor: VfxAnchorPolicy,
    pub(crate) lifetime: VfxLifetime,
    pub(crate) concurrency_cap: usize,
    pub(crate) fallback_profile: String,
    pub(crate) reduced_profile: Option<String>,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct VfxProfileRequirements {
    pub(super) authoritative_radius: bool,
    pub(super) authoritative_deadline: bool,
}

#[derive(Clone, Debug)]
pub(super) struct VfxCatalog {
    default_profile: String,
    mappings: BTreeMap<String, String>,
    profiles: BTreeMap<String, VfxProfile>,
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

    pub(super) fn from_source(source: VfxCatalogSource) -> Result<Self, String> {
        if source.schema_version != VFX_SCHEMA_VERSION {
            return Err(format!(
                "unsupported VFX catalog schema {}, expected {VFX_SCHEMA_VERSION}",
                source.schema_version
            ));
        }
        if source.profiles.len() > MAX_VFX_PROFILES {
            return Err("VFX catalog exceeds profile capacity".into());
        }
        if source.mappings.len() > MAX_VFX_MAPPINGS {
            return Err("VFX catalog exceeds request-mapping capacity".into());
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
            if !valid_vfx_request_key(&mapping.key) {
                return Err(format!("invalid VFX request mapping key: {}", mapping.key));
            }
            if !profiles.contains_key(&mapping.profile) {
                return Err(format!(
                    "VFX request {} references missing profile {}",
                    mapping.key, mapping.profile
                ));
            }
            if mappings
                .insert(mapping.key.clone(), mapping.profile)
                .is_some()
            {
                return Err(format!("duplicate VFX request mapping: {}", mapping.key));
            }
        }
        if mappings.is_empty() {
            return Err("VFX catalog must contain at least one request mapping".into());
        }
        Ok(Self {
            default_profile: source.default_profile,
            mappings,
            profiles,
        })
    }

    pub(super) fn mapping_keys(&self) -> impl Iterator<Item = &str> {
        self.mappings.keys().map(String::as_str)
    }

    pub(super) fn requirements(&self, key: &str) -> Option<VfxProfileRequirements> {
        let profile = self.mapped_profile(key)?;
        let reduced = profile
            .reduced_profile
            .as_deref()
            .and_then(|id| self.profiles.get(id));
        Some([Some(profile), reduced].into_iter().flatten().fold(
            VfxProfileRequirements::default(),
            |requirements, profile| {
                let requirements = requirements.including(profile);
                if matches!(profile.lifetime, VfxLifetime::AuthoritativeDeadline) {
                    requirements.including(&self.profiles[&profile.fallback_profile])
                } else {
                    requirements
                }
            },
        ))
    }

    #[cfg(test)]
    pub(super) fn insert_test_mapping(
        &mut self,
        key: &str,
        profile_id: &str,
    ) -> Result<(), String> {
        if !valid_vfx_request_key(key) {
            return Err(format!("invalid VFX request mapping key: {key}"));
        }
        if self.mappings.len() >= MAX_VFX_MAPPINGS {
            return Err("VFX catalog exceeds request-mapping capacity".into());
        }
        if !self.profiles.contains_key(profile_id) {
            return Err(format!(
                "VFX request {key} references missing profile {profile_id}"
            ));
        }
        if self
            .mappings
            .insert(key.to_owned(), profile_id.to_owned())
            .is_some()
        {
            return Err(format!("duplicate VFX request mapping: {key}"));
        }
        Ok(())
    }

    pub(super) fn resolve(
        &self,
        key: &str,
        reduced: bool,
        authoritative_deadline_available: bool,
    ) -> Option<&VfxProfile> {
        let profile = self.mapped_profile(key)?;
        let profile = if reduced {
            profile
                .reduced_profile
                .as_deref()
                .and_then(|id| self.profiles.get(id))
                .unwrap_or(profile)
        } else {
            profile
        };
        if authoritative_deadline_available
            || !matches!(profile.lifetime, VfxLifetime::AuthoritativeDeadline)
        {
            return Some(profile);
        }
        self.profiles
            .get(&profile.fallback_profile)
            .or_else(|| self.profiles.get(&self.default_profile))
    }

    fn mapped_profile(&self, key: &str) -> Option<&VfxProfile> {
        self.mappings
            .get(key)
            .and_then(|profile_id| self.profiles.get(profile_id))
    }
}

impl VfxProfileRequirements {
    fn including(mut self, profile: &VfxProfile) -> Self {
        self.authoritative_radius |= profile.scale.requires_authoritative_radius()
            || profile.anchor.requires_authoritative_radius();
        self.authoritative_deadline |=
            matches!(profile.lifetime, VfxLifetime::AuthoritativeDeadline);
        self
    }
}

fn validate_profile(profile: &VfxProfile) -> Result<(), String> {
    if profile.id.trim().is_empty() || profile.fallback_profile.trim().is_empty() {
        return Err("VFX profile ids and fallback references must be non-empty".to_string());
    }
    if !(profile.scale.has_valid_bounds()
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
        })
    {
        return Err(format!(
            "VFX profile {} has unsafe runtime values",
            profile.id
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    const BUILTIN_MAPPINGS: [(&str, &str); 15] = [
        ("ability.demolition-strike", "demolition"),
        ("ability.elemental-field", "elemental_field"),
        ("ability.reveal-scan", "reveal_scan"),
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
    fn embedded_catalog_preserves_all_fifteen_profile_mappings() {
        let catalog = VfxCatalog::embedded().unwrap();
        assert_eq!(catalog.mappings.len(), BUILTIN_MAPPINGS.len());
        for (key, profile_id) in BUILTIN_MAPPINGS {
            assert_eq!(catalog.mapped_profile(key).unwrap().id, profile_id);
        }

        let normal = catalog.resolve("combat.muzzle", false, false).unwrap();
        let reduced = catalog.resolve("combat.muzzle", true, false).unwrap();
        assert_eq!(normal.material, VfxMaterialKey::EffectMuzzle);
        assert_eq!(normal.scale, VfxScalePolicy::FixedWorld(8.0));
        assert_eq!(normal.anchor, VfxAnchorPolicy::FixedWorldHeight(3.6));
        assert_eq!(normal.lifetime, VfxLifetime::Millis(180));
        assert_eq!(reduced.scale, VfxScalePolicy::FixedWorld(5.2));
        assert_eq!(reduced.lifetime, VfxLifetime::Millis(100));
    }

    #[test]
    fn strict_schema_keys_references_values_and_capacity_are_validated() {
        for (needle, replacement) in [
            ("schema_version: 3", "schema_version: 2"),
            ("key: \"combat.muzzle\"", "key: \"Combat/Muzzle\""),
            ("profile: \"muzzle\"", "profile: \"missing\""),
            ("scale: FixedWorld(8.0)", "scale: FixedWorld(0.0)"),
            ("lifetime: Millis(180)", "lifetime: Millis(0)"),
            ("concurrency_cap: 96", "concurrency_cap: 97"),
        ] {
            assert!(
                VfxCatalog::from_ron(&VFX_CATALOG.replacen(needle, replacement, 1)).is_err(),
                "{replacement}"
            );
        }
    }

    #[test]
    fn duplicate_mapping_and_profile_ids_are_rejected() {
        let mut duplicate_mapping: VfxCatalogSource = ron::from_str(VFX_CATALOG).unwrap();
        duplicate_mapping
            .mappings
            .push(duplicate_mapping.mappings[0].clone());
        assert!(VfxCatalog::from_source(duplicate_mapping).is_err());

        let mut duplicate_profile: VfxCatalogSource = ron::from_str(VFX_CATALOG).unwrap();
        duplicate_profile
            .profiles
            .push(duplicate_profile.profiles[0].clone());
        assert!(VfxCatalog::from_source(duplicate_profile).is_err());
    }

    #[test]
    fn profile_requirements_are_derived_without_a_closed_family_switch() {
        assert_eq!(
            VfxCatalog::embedded()
                .unwrap()
                .requirements("ability.reveal-scan"),
            Some(VfxProfileRequirements {
                authoritative_radius: true,
                authoritative_deadline: true,
            })
        );
    }

    #[test]
    fn profile_requirements_include_reachable_deadline_fallback_capabilities() {
        let source = VFX_CATALOG.replacen(
            "scale: AuthoritativeRadius(1.0), anchor: GroundOffset(2.5), lifetime: AuthoritativeDeadline, concurrency_cap: 96, fallback_profile: \"impact\"",
            "scale: FixedWorld(10.0), anchor: GroundOffset(2.5), lifetime: AuthoritativeDeadline, concurrency_cap: 96, fallback_profile: \"elemental_field\"",
            1,
        );
        let catalog = VfxCatalog::from_ron(&source).unwrap();

        assert_eq!(
            catalog.requirements("ability.reveal-scan"),
            Some(VfxProfileRequirements {
                authoritative_radius: true,
                authoritative_deadline: true,
            })
        );
    }
}
