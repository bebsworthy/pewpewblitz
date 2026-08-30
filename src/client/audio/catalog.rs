use bevy::prelude::*;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet};

const AUDIO_PROFILE_CATALOG: &str = include_str!("../../../assets/catalogs/audio_profiles.ron");
const AUDIO_PROFILE_SCHEMA_VERSION: u16 = 1;
const MAX_AUDIO_CONCURRENCY: usize = 24;

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq, PartialOrd, Ord)]
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

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
pub(super) enum AudioAssetKey {
    Fire,
    Impact,
    Defeat,
    Ready,
    Error,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
enum AudioPlaybackFamily {
    OneShot,
}

#[derive(Clone, Copy, Debug, Deserialize, PartialEq, Eq)]
enum AudioLifetime {
    UntilComplete,
}

#[derive(Clone, Debug, Deserialize)]
struct AudioCatalogSource {
    schema_version: u16,
    default_profile: String,
    mappings: Vec<AudioFamilyMapping>,
    profiles: Vec<AudioProfile>,
}

#[derive(Clone, Debug, Deserialize)]
struct AudioFamilyMapping {
    family: AudioCueFamily,
    profile: String,
}

#[derive(Clone, Debug, Deserialize)]
struct AudioProfile {
    id: String,
    asset: Option<AudioAssetKey>,
    playback: AudioPlaybackFamily,
    speed: f32,
    volume: f32,
    lifetime: AudioLifetime,
    concurrency_cap: usize,
    fallback_profile: String,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct AudioPlaybackPlan {
    pub asset: AudioAssetKey,
    pub speed: f32,
    pub volume: f32,
    pub concurrency_cap: usize,
}

#[derive(Resource, Clone, Debug)]
pub(super) struct AudioProfileCatalog {
    default_profile: String,
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
        for profile in source.profiles {
            validate_profile(&profile)?;
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
        for profile in profiles.values() {
            if !profiles.contains_key(&profile.fallback_profile) {
                return Err(format!(
                    "audio profile {} references missing fallback {}",
                    profile.id, profile.fallback_profile
                ));
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
        Ok(Self {
            default_profile: source.default_profile,
            mappings,
            profiles,
        })
    }

    /// Resolves a profile through its bounded fallback chain. An absent family, missing runtime
    /// asset, or malformed fallback cycle deterministically degrades to the silent default.
    pub(super) fn playback_plan(
        &self,
        family: AudioCueFamily,
        mut asset_is_loaded: impl FnMut(AudioAssetKey) -> bool,
    ) -> Option<AudioPlaybackPlan> {
        let mut next = self
            .mappings
            .get(&family)
            .unwrap_or(&self.default_profile)
            .as_str();
        let mut visited = BTreeSet::new();
        loop {
            let profile = self
                .profiles
                .get(next)
                .unwrap_or_else(|| &self.profiles[&self.default_profile]);
            let asset = profile.asset?;
            if asset_is_loaded(asset) {
                return Some(AudioPlaybackPlan {
                    asset,
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
        &self.profiles[self.mappings.get(&family).unwrap_or(&self.default_profile)]
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
    if !matches!(profile.playback, AudioPlaybackFamily::OneShot)
        || !matches!(profile.lifetime, AudioLifetime::UntilComplete)
    {
        return Err(format!(
            "audio profile {} uses an unsupported playback recipe",
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
        assert_eq!(dash.asset, Some(AudioAssetKey::Defeat));
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
    fn invalid_asset_reference_values_and_caps_are_rejected() {
        for (needle, replacement) in [
            ("asset: Some(Fire)", "asset: Some(Missing)"),
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
            "asset: Some(Fire), playback: OneShot, speed: 1.0",
            "asset: Some(Error), playback: OneShot, speed: 0.8",
            1,
        );
        let catalog = AudioProfileCatalog::from_ron(&source).unwrap();
        let plan = catalog
            .playback_plan(AudioCueFamily::Fire, |_| true)
            .unwrap();
        assert_eq!(plan.asset, AudioAssetKey::Error);
        assert!((plan.speed - 0.8).abs() < f32::EPSILON);
    }

    #[test]
    fn missing_runtime_asset_follows_fallback_then_silences_deterministically() {
        let mut catalog = AudioProfileCatalog::embedded().unwrap();
        catalog.profiles.get_mut("fire").unwrap().fallback_profile = "ready".to_string();
        let plan = catalog
            .playback_plan(AudioCueFamily::Fire, |asset| asset == AudioAssetKey::Ready)
            .unwrap();
        assert_eq!(plan.asset, AudioAssetKey::Ready);
        assert!(
            catalog
                .playback_plan(AudioCueFamily::Fire, |_| false)
                .is_none()
        );
    }

    #[test]
    fn missing_family_mapping_uses_silent_default() {
        let mut catalog = AudioProfileCatalog::embedded().unwrap();
        catalog.mappings.remove(&AudioCueFamily::Reset);
        assert!(
            catalog
                .playback_plan(AudioCueFamily::Reset, |_| true)
                .is_none()
        );
    }
}
