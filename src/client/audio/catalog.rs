use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

const AUDIO_PROFILE_CATALOG: &str = include_str!("../../../assets/catalogs/audio_profiles.ron");
const AUDIO_PROFILE_SCHEMA_VERSION: u16 = 2;
const MAX_AUDIO_CONCURRENCY: usize = 24;

#[derive(Clone, Copy, Debug, Deserialize, Serialize, PartialEq, Eq, PartialOrd, Ord)]
pub(super) enum AudioCueFamily {
    Fire,
    Impact,
    Defeat,
    Reset,
    Ready,
    Error,
    Dash,
    Sentry,
    SentrySpawn,
    ConcealmentFieldSpawn,
    ChargeReady,
    Passive,
    ObjectiveHit,
    ObjectiveCritical,
    ObjectiveDestroyed,
    Reload,
}

const ALL_AUDIO_CUE_FAMILIES: [AudioCueFamily; 16] = [
    AudioCueFamily::Fire,
    AudioCueFamily::Impact,
    AudioCueFamily::Defeat,
    AudioCueFamily::Reset,
    AudioCueFamily::Ready,
    AudioCueFamily::Error,
    AudioCueFamily::Dash,
    AudioCueFamily::Sentry,
    AudioCueFamily::SentrySpawn,
    AudioCueFamily::ConcealmentFieldSpawn,
    AudioCueFamily::ChargeReady,
    AudioCueFamily::Passive,
    AudioCueFamily::ObjectiveHit,
    AudioCueFamily::ObjectiveCritical,
    AudioCueFamily::ObjectiveDestroyed,
    AudioCueFamily::Reload,
];

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct AudioCatalogSource {
    schema_version: u16,
    default_profile: String,
    mappings: Vec<AudioFamilyMapping>,
    profiles: Vec<AudioProfile>,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
struct AudioFamilyMapping {
    family: AudioCueFamily,
    profile: String,
}

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct AudioProfile {
    id: String,
    asset: Option<String>,
    speed: f32,
    volume: f32,
    concurrency_cap: usize,
    fallback_profile: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AudioPlaybackPlan<'a> {
    pub asset_id: &'a str,
    pub speed: f32,
    pub volume: f32,
    pub concurrency_cap: usize,
}

#[derive(Resource, Clone, Debug)]
pub(super) struct AudioProfileCatalog {
    mappings: BTreeMap<AudioCueFamily, String>,
    profiles: BTreeMap<String, AudioProfile>,
}

impl AudioProfileCatalog {
    pub(super) fn embedded() -> Result<Self, String> {
        Self::from_ron(AUDIO_PROFILE_CATALOG)
    }

    fn from_ron(source: &str) -> Result<Self, String> {
        let source: AudioCatalogSource = ron::from_str(source)
            .map_err(|error| format!("audio profile catalog parse failed: {error}"))?;
        Self::from_source(source)
    }

    fn from_source(source: AudioCatalogSource) -> Result<Self, String> {
        if source.schema_version != AUDIO_PROFILE_SCHEMA_VERSION {
            return Err(format!(
                "unsupported audio profile schema {}, expected {AUDIO_PROFILE_SCHEMA_VERSION}",
                source.schema_version
            ));
        }
        let mut profiles = BTreeMap::new();
        let audio_assets = crate::client::assets::audio_asset_paths()?;
        for profile in source.profiles {
            validate_profile(&profile)?;
            if profile
                .asset
                .as_ref()
                .is_some_and(|asset_id| !audio_assets.contains_key(asset_id))
            {
                return Err(format!(
                    "audio profile {} references an unknown asset-manifest id",
                    profile.id
                ));
            }
            if profiles.insert(profile.id.clone(), profile).is_some() {
                return Err("duplicate audio profile id".to_string());
            }
        }
        let Some(default) = profiles.get(&source.default_profile) else {
            return Err("audio default profile reference is missing".to_string());
        };
        if default.asset.is_some() {
            return Err("audio default profile must degrade to silence".to_string());
        }
        if default.fallback_profile != source.default_profile {
            return Err("audio default profile must fall back to itself".to_string());
        }
        for profile in profiles.values() {
            if !profiles.contains_key(&profile.fallback_profile) {
                return Err(format!(
                    "audio profile {} references missing fallback {}",
                    profile.id, profile.fallback_profile
                ));
            }
        }
        for profile in profiles.values() {
            let mut next = profile.id.as_str();
            let mut visited = BTreeSet::new();
            while next != source.default_profile.as_str() {
                if !visited.insert(next) {
                    return Err(format!(
                        "audio profile {} has a fallback cycle that does not reach the default",
                        profile.id
                    ));
                }
                next = profiles[next].fallback_profile.as_str();
            }
        }
        let mut mappings = BTreeMap::new();
        for mapping in source.mappings {
            if !profiles.contains_key(&mapping.profile) {
                return Err(format!(
                    "audio family {:?} references missing profile {}",
                    mapping.family, mapping.profile
                ));
            }
            if mappings.insert(mapping.family, mapping.profile).is_some() {
                return Err(format!(
                    "duplicate audio family mapping: {:?}",
                    mapping.family
                ));
            }
        }
        if mappings.len() != ALL_AUDIO_CUE_FAMILIES.len()
            || ALL_AUDIO_CUE_FAMILIES
                .iter()
                .any(|family| !mappings.contains_key(family))
        {
            return Err(
                "audio catalog must map every registered cue family exactly once".to_string(),
            );
        }
        Ok(Self { mappings, profiles })
    }

    /// Resolves a profile through its bounded fallback chain. Catalog validation guarantees that
    /// every cue family has a mapping; only unavailable runtime assets degrade through fallbacks.
    pub(super) fn playback_plan(
        &self,
        family: AudioCueFamily,
        mut asset_is_loaded: impl FnMut(&str) -> bool,
    ) -> Option<AudioPlaybackPlan<'_>> {
        let mut next = self
            .mappings
            .get(&family)
            .expect("validated audio catalogs map every cue family")
            .as_str();
        let mut visited = BTreeSet::new();
        loop {
            let profile = self
                .profiles
                .get(next)
                .expect("validated audio fallback references remain available");
            let asset_id = profile.asset.as_deref()?;
            if asset_is_loaded(asset_id) {
                return Some(AudioPlaybackPlan {
                    asset_id,
                    speed: profile.speed,
                    volume: profile.volume,
                    concurrency_cap: profile.concurrency_cap,
                });
            }
            if !visited.insert(next) {
                return None;
            }
            next = &profile.fallback_profile;
        }
    }

    #[cfg(test)]
    fn mapped_profile(&self, family: AudioCueFamily) -> &AudioProfile {
        &self.profiles[self
            .mappings
            .get(&family)
            .expect("validated audio catalogs map every cue family")]
    }
}

fn validate_profile(profile: &AudioProfile) -> Result<(), String> {
    if profile.id.trim().is_empty() || profile.fallback_profile.trim().is_empty() {
        return Err("audio profile ids and fallback references must be non-empty".to_string());
    }
    if !profile.speed.is_finite() || profile.speed <= 0.0 || profile.speed > 4.0 {
        return Err(format!("audio profile {} has invalid speed", profile.id));
    }
    if !profile.volume.is_finite() || !(0.0..=1.0).contains(&profile.volume) {
        return Err(format!("audio profile {} has invalid volume", profile.id));
    }
    if profile.concurrency_cap == 0 || profile.concurrency_cap > MAX_AUDIO_CONCURRENCY {
        return Err(format!(
            "audio profile {} has invalid concurrency cap",
            profile.id
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_catalog_preserves_builtin_variants_and_caps() {
        let catalog = AudioProfileCatalog::embedded().unwrap();
        let dash = catalog.mapped_profile(AudioCueFamily::Dash);
        assert_eq!(dash.asset.as_deref(), Some("audio.defeat"));
        assert!((dash.speed - 1.45).abs() < f32::EPSILON);
        assert!((dash.volume - 1.0).abs() < f32::EPSILON);
        assert_eq!(
            catalog.mapped_profile(AudioCueFamily::Fire).concurrency_cap,
            20
        );
        assert_eq!(
            catalog
                .mapped_profile(AudioCueFamily::ObjectiveHit)
                .concurrency_cap,
            6
        );
    }

    #[test]
    fn catalog_source_round_trips_without_losing_policy() {
        let source: AudioCatalogSource = ron::from_str(AUDIO_PROFILE_CATALOG).unwrap();
        let encoded = ron::to_string(&source).unwrap();
        let decoded: AudioCatalogSource = ron::from_str(&encoded).unwrap();
        assert_eq!(decoded, source);
        AudioProfileCatalog::from_source(decoded).unwrap();
    }

    #[test]
    fn bad_schema_invalid_asset_reference_values_and_caps_are_rejected() {
        for (needle, replacement) in [
            ("schema_version: 2", "schema_version: 3"),
            ("id: \"silent\"", "id: \"silent\", playback: OneShot"),
            (
                "asset: Some(\"audio.fire\")",
                "asset: Some(\"audio.missing\")",
            ),
            ("speed: 1.0", "speed: 0.0"),
            ("volume: 1.0", "volume: 1.1"),
            ("concurrency_cap: 20", "concurrency_cap: 25"),
            (
                "fallback_profile: \"silent\"",
                "fallback_profile: \"missing\"",
            ),
        ] {
            let source = AUDIO_PROFILE_CATALOG.replacen(needle, replacement, 1);
            assert!(
                AudioProfileCatalog::from_ron(&source).is_err(),
                "{replacement}"
            );
        }
    }

    #[test]
    fn supported_family_variant_is_selected_from_catalog_data() {
        let source = AUDIO_PROFILE_CATALOG.replacen(
            "asset: Some(\"audio.fire\"), speed: 1.0, volume: 1.0",
            "asset: Some(\"audio.error\"), speed: 0.8, volume: 0.5",
            1,
        );
        let catalog = AudioProfileCatalog::from_ron(&source).unwrap();
        let plan = catalog
            .playback_plan(AudioCueFamily::Fire, |_| true)
            .unwrap();
        assert_eq!(plan.asset_id, "audio.error");
        assert!((plan.speed - 0.8).abs() < f32::EPSILON);
        assert!((plan.volume - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn missing_runtime_asset_follows_fallback_then_silences_deterministically() {
        let mut catalog = AudioProfileCatalog::embedded().unwrap();
        catalog.profiles.get_mut("fire").unwrap().fallback_profile = "ready".to_string();
        let plan = catalog
            .playback_plan(AudioCueFamily::Fire, |asset| asset == "audio.ready")
            .unwrap();
        assert_eq!(plan.asset_id, "audio.ready");
        assert!(
            catalog
                .playback_plan(AudioCueFamily::Fire, |_| false)
                .is_none()
        );
    }

    #[test]
    fn duplicate_ids_mappings_and_missing_family_are_rejected() {
        let mut duplicate_profile: AudioCatalogSource =
            ron::from_str(AUDIO_PROFILE_CATALOG).unwrap();
        duplicate_profile
            .profiles
            .push(duplicate_profile.profiles[0].clone());
        assert!(AudioProfileCatalog::from_source(duplicate_profile).is_err());

        let mut duplicate_mapping: AudioCatalogSource =
            ron::from_str(AUDIO_PROFILE_CATALOG).unwrap();
        duplicate_mapping
            .mappings
            .push(duplicate_mapping.mappings[0].clone());
        assert!(AudioProfileCatalog::from_source(duplicate_mapping).is_err());

        let mut missing_family: AudioCatalogSource = ron::from_str(AUDIO_PROFILE_CATALOG).unwrap();
        missing_family
            .mappings
            .retain(|mapping| mapping.family != AudioCueFamily::Reset);
        assert!(AudioProfileCatalog::from_source(missing_family).is_err());

        let mut fallback_cycle: AudioCatalogSource = ron::from_str(AUDIO_PROFILE_CATALOG).unwrap();
        fallback_cycle
            .profiles
            .iter_mut()
            .find(|profile| profile.id == "fire")
            .unwrap()
            .fallback_profile = "impact".to_string();
        fallback_cycle
            .profiles
            .iter_mut()
            .find(|profile| profile.id == "impact")
            .unwrap()
            .fallback_profile = "fire".to_string();
        assert!(AudioProfileCatalog::from_source(fallback_cycle).is_err());
    }
}
