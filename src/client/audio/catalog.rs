use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::request::{AudioCueKey, validate_audio_cue_key};

const AUDIO_PROFILE_CATALOG: &str = include_str!("../../../assets/catalogs/audio_profiles.ron");
const AUDIO_PROFILE_SCHEMA_VERSION: u16 = 3;
const MAX_AUDIO_CONCURRENCY: usize = 24;

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
    cue_key: String,
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
    mappings: BTreeMap<String, String>,
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
            validate_audio_cue_key(&mapping.cue_key)
                .map_err(|error| format!("invalid audio cue key {}: {error}", mapping.cue_key))?;
            if !profiles.contains_key(&mapping.profile) {
                return Err(format!(
                    "audio cue {} references missing profile {}",
                    mapping.cue_key, mapping.profile
                ));
            }
            if mappings
                .insert(mapping.cue_key.clone(), mapping.profile)
                .is_some()
            {
                return Err(format!("duplicate audio cue mapping: {}", mapping.cue_key));
            }
        }
        Ok(Self { mappings, profiles })
    }

    pub(super) fn validate_registered_keys<'a>(
        &self,
        registered: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), String> {
        let registered = registered.into_iter().collect::<BTreeSet<_>>();
        if let Some(missing) = registered
            .iter()
            .find(|key| !self.mappings.contains_key(**key))
        {
            return Err(format!(
                "registered audio cue {missing} has no catalog mapping"
            ));
        }
        if let Some(orphan) = self
            .mappings
            .keys()
            .find(|key| !registered.contains(key.as_str()))
        {
            return Err(format!(
                "audio catalog mapping {orphan} has no registered producer"
            ));
        }
        Ok(())
    }

    pub(super) fn contains_mapping(&self, cue_key: AudioCueKey) -> bool {
        validate_audio_cue_key(cue_key.as_str()).is_ok()
            && self.mappings.contains_key(cue_key.as_str())
    }

    #[cfg(test)]
    pub(super) fn with_test_mapping(
        mut self,
        cue_key: AudioCueKey,
        profile_id: &str,
    ) -> Result<Self, String> {
        validate_audio_cue_key(cue_key.as_str())
            .map_err(|error| format!("invalid audio cue key {}: {error}", cue_key.as_str()))?;
        if !self.profiles.contains_key(profile_id) {
            return Err(format!(
                "audio cue {} references missing profile {profile_id}",
                cue_key.as_str()
            ));
        }
        if self
            .mappings
            .insert(cue_key.as_str().to_string(), profile_id.to_string())
            .is_some()
        {
            return Err(format!("duplicate audio cue mapping: {}", cue_key.as_str()));
        }
        Ok(self)
    }

    /// Resolves a profile through its bounded fallback chain. Registry finalization guarantees
    /// exact stable-key coverage; malformed or unknown runtime keys fail closed here.
    pub(super) fn playback_plan(
        &self,
        cue_key: AudioCueKey,
        mut asset_is_loaded: impl FnMut(&str) -> bool,
    ) -> Option<AudioPlaybackPlan<'_>> {
        if validate_audio_cue_key(cue_key.as_str()).is_err() {
            return None;
        }
        let mut next = self.mappings.get(cue_key.as_str())?.as_str();
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
    fn mapped_profile(&self, cue_key: AudioCueKey) -> &AudioProfile {
        &self.profiles[self
            .mappings
            .get(cue_key.as_str())
            .expect("test cue key has an embedded mapping")]
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
    use super::super::request::cue_keys;
    use super::*;

    #[test]
    fn embedded_catalog_preserves_all_builtin_mappings_and_profile_values() {
        let catalog = AudioProfileCatalog::embedded().unwrap();
        let expected_mappings = [
            (cue_keys::FIRE, "fire"),
            (cue_keys::IMPACT, "impact"),
            (cue_keys::DEFEAT, "defeat"),
            (cue_keys::RESET, "reset"),
            (cue_keys::READY, "ready"),
            (cue_keys::ERROR, "error"),
            (cue_keys::DASH, "dash"),
            (cue_keys::SENTRY, "sentry"),
            (cue_keys::SENTRY_SPAWN, "sentry_spawn"),
            (cue_keys::CONCEALMENT_FIELD_SPAWN, "concealment_field_spawn"),
            (cue_keys::CHARGE_READY, "charge_ready"),
            (cue_keys::PASSIVE, "passive"),
            (cue_keys::OBJECTIVE_HIT, "objective_hit"),
            (cue_keys::OBJECTIVE_CRITICAL, "objective_critical"),
            (cue_keys::OBJECTIVE_DESTROYED, "objective_destroyed"),
            (cue_keys::RELOAD, "reload"),
        ];
        assert_eq!(catalog.mappings.len(), expected_mappings.len());
        for (cue_key, profile) in expected_mappings {
            assert_eq!(
                catalog.mappings.get(cue_key.as_str()).map(String::as_str),
                Some(profile)
            );
        }

        let source: AudioCatalogSource = ron::from_str(AUDIO_PROFILE_CATALOG).unwrap();
        assert_eq!(source.default_profile, "silent");
        assert_eq!(
            source.profiles,
            vec![
                profile("silent", None, 1.0, 1.0, 24, "silent"),
                profile("fire", Some("audio.fire"), 1.0, 1.0, 20, "silent"),
                profile("impact", Some("audio.impact"), 1.0, 1.0, 22, "silent"),
                profile("defeat", Some("audio.defeat"), 1.0, 1.0, 24, "silent"),
                profile("reset", Some("audio.ready"), 1.0, 1.0, 24, "silent"),
                profile("ready", Some("audio.ready"), 1.0, 1.0, 24, "silent"),
                profile("error", Some("audio.error"), 1.0, 1.0, 24, "silent"),
                profile("dash", Some("audio.defeat"), 1.45, 1.0, 24, "silent"),
                profile("sentry", Some("audio.ready"), 1.0, 1.0, 24, "silent"),
                profile("sentry_spawn", Some("audio.ready"), 0.75, 1.0, 24, "silent",),
                profile(
                    "concealment_field_spawn",
                    Some("audio.ready"),
                    0.9,
                    1.0,
                    24,
                    "silent",
                ),
                profile("charge_ready", Some("audio.ready"), 1.25, 1.0, 24, "silent",),
                profile("passive", Some("audio.impact"), 1.35, 1.0, 24, "silent",),
                profile("objective_hit", Some("audio.impact"), 1.0, 1.0, 6, "silent",),
                profile(
                    "objective_critical",
                    Some("audio.ready"),
                    1.0,
                    1.0,
                    24,
                    "silent",
                ),
                profile(
                    "objective_destroyed",
                    Some("audio.defeat"),
                    1.0,
                    1.0,
                    24,
                    "silent",
                ),
                profile("reload", Some("audio.ready"), 1.0, 1.0, 24, "silent"),
            ]
        );

        let dash = catalog.mapped_profile(cue_keys::DASH);
        assert_eq!(dash.asset.as_deref(), Some("audio.defeat"));
        assert!((dash.speed - 1.45).abs() < f32::EPSILON);
        assert!((dash.volume - 1.0).abs() < f32::EPSILON);
        assert_eq!(catalog.mapped_profile(cue_keys::FIRE).concurrency_cap, 20);
        assert_eq!(
            catalog
                .mapped_profile(cue_keys::OBJECTIVE_HIT)
                .concurrency_cap,
            6
        );
    }

    fn profile(
        id: &str,
        asset: Option<&str>,
        speed: f32,
        volume: f32,
        concurrency_cap: usize,
        fallback_profile: &str,
    ) -> AudioProfile {
        AudioProfile {
            id: id.to_string(),
            asset: asset.map(str::to_string),
            speed,
            volume,
            concurrency_cap,
            fallback_profile: fallback_profile.to_string(),
        }
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
            ("schema_version: 3", "schema_version: 4"),
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
        let plan = catalog.playback_plan(cue_keys::FIRE, |_| true).unwrap();
        assert_eq!(plan.asset_id, "audio.error");
        assert!((plan.speed - 0.8).abs() < f32::EPSILON);
        assert!((plan.volume - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn missing_runtime_asset_follows_fallback_then_silences_deterministically() {
        let mut catalog = AudioProfileCatalog::embedded().unwrap();
        catalog.profiles.get_mut("fire").unwrap().fallback_profile = "ready".to_string();
        let plan = catalog
            .playback_plan(cue_keys::FIRE, |asset| asset == "audio.ready")
            .unwrap();
        assert_eq!(plan.asset_id, "audio.ready");
        assert!(catalog.playback_plan(cue_keys::FIRE, |_| false).is_none());
    }

    #[test]
    fn duplicate_ids_mappings_invalid_keys_and_fallback_cycles_are_rejected() {
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

        let invalid_key =
            AUDIO_PROFILE_CATALOG.replacen("cue_key: \"reset\"", "cue_key: \"Reset_Key\"", 1);
        assert!(AudioProfileCatalog::from_ron(&invalid_key).is_err());

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

    #[test]
    fn exact_registration_coverage_rejects_missing_and_orphan_mappings() {
        let catalog = AudioProfileCatalog::embedded().unwrap();
        catalog
            .validate_registered_keys(cue_keys::BUILTIN.map(AudioCueKey::as_str))
            .unwrap();

        assert_eq!(
            catalog
                .validate_registered_keys(
                    cue_keys::BUILTIN
                        .into_iter()
                        .filter(|key| *key != cue_keys::RESET)
                        .map(AudioCueKey::as_str),
                )
                .unwrap_err(),
            "audio catalog mapping reset has no registered producer"
        );
        assert_eq!(
            catalog
                .validate_registered_keys(
                    cue_keys::BUILTIN
                        .map(AudioCueKey::as_str)
                        .into_iter()
                        .chain(["synthetic"]),
                )
                .unwrap_err(),
            "registered audio cue synthetic has no catalog mapping"
        );
    }

    #[test]
    fn malformed_and_unknown_runtime_keys_fail_closed() {
        let catalog = AudioProfileCatalog::embedded().unwrap();
        assert!(
            catalog
                .playback_plan(AudioCueKey::new("Missing"), |_| true)
                .is_none()
        );
        assert!(
            catalog
                .playback_plan(AudioCueKey::new("synthetic"), |_| true)
                .is_none()
        );
    }
}
