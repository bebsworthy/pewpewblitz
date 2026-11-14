use bevy::prelude::*;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, BTreeSet};

use super::request::{AudioCueKey, validate_audio_cue_key};

const AUDIO_PROFILE_CATALOG: &str = include_str!("../../../../assets/catalogs/audio_profiles.ron");
const AUDIO_PROFILE_SCHEMA_VERSION: u16 = 4;
const MAX_MUSIC_TRACKS: usize = 8;
const MAX_MUSIC_LOOP_SECONDS: f32 = 600.0;
const MAX_AUDIO_CONCURRENCY: usize = 24;

#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct AudioCatalogSource {
    schema_version: u16,
    default_profile: String,
    mappings: Vec<AudioFamilyMapping>,
    profiles: Vec<AudioProfile>,
    music: Vec<MusicTrack>,
}

/// One looping music track selected by a semantic music key.
#[derive(Clone, Debug, Deserialize, Serialize, PartialEq)]
#[serde(deny_unknown_fields)]
struct MusicTrack {
    key: String,
    asset: String,
    volume: f32,
    loop_seconds: f32,
}

/// Resolved looping playback for one music key.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct MusicPlaybackPlan<'a> {
    pub asset_id: &'a str,
    pub volume: f32,
    pub loop_seconds: f32,
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
    music: BTreeMap<String, MusicTrack>,
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
        let audio_assets = crate::assets::audio_asset_paths()?;
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
        let music = validate_music(source.music, &audio_assets)?;
        Ok(Self {
            mappings,
            profiles,
            music,
        })
    }

    /// Exact coverage between the music keys the client selects and the authored tracks.
    pub(super) fn validate_music_keys<'a>(
        &self,
        selected: impl IntoIterator<Item = &'a str>,
    ) -> Result<(), String> {
        let selected = selected.into_iter().collect::<BTreeSet<_>>();
        if let Some(missing) = selected.iter().find(|key| !self.music.contains_key(**key)) {
            return Err(format!("music key {missing} has no catalog track"));
        }
        if let Some(orphan) = self
            .music
            .keys()
            .find(|key| !selected.contains(key.as_str()))
        {
            return Err(format!("music track {orphan} has no selector"));
        }
        Ok(())
    }

    pub(super) fn music_plan(&self, key: &str) -> Option<MusicPlaybackPlan<'_>> {
        self.music.get(key).map(|track| MusicPlaybackPlan {
            asset_id: &track.asset,
            volume: track.volume,
            loop_seconds: track.loop_seconds,
        })
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

fn validate_music(
    tracks: Vec<MusicTrack>,
    audio_assets: &BTreeMap<String, String>,
) -> Result<BTreeMap<String, MusicTrack>, String> {
    if tracks.len() > MAX_MUSIC_TRACKS {
        return Err("too many music tracks".to_string());
    }
    let mut music = BTreeMap::new();
    for track in tracks {
        validate_audio_cue_key(&track.key)
            .map_err(|error| format!("invalid music key {}: {error}", track.key))?;
        if !audio_assets.contains_key(&track.asset) {
            return Err(format!(
                "music track {} references an unknown asset-manifest id",
                track.key
            ));
        }
        if !track.volume.is_finite() || !(0.0..=1.0).contains(&track.volume) {
            return Err(format!("music track {} has invalid volume", track.key));
        }
        if !track.loop_seconds.is_finite()
            || track.loop_seconds <= 0.0
            || track.loop_seconds > MAX_MUSIC_LOOP_SECONDS
        {
            return Err(format!("music track {} has invalid loop length", track.key));
        }
        if music.insert(track.key.clone(), track).is_some() {
            return Err("duplicate music key".to_string());
        }
    }
    Ok(music)
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
    fn every_builtin_cue_has_its_own_distinct_authored_clip() {
        let catalog = AudioProfileCatalog::embedded().unwrap();
        assert_eq!(catalog.mappings.len(), cue_keys::BUILTIN.len());
        let mut assets = BTreeSet::new();
        for cue_key in cue_keys::BUILTIN {
            let profile = catalog.mapped_profile(cue_key);
            let asset = profile
                .asset
                .as_deref()
                .unwrap_or_else(|| panic!("{} maps to silence", cue_key.as_str()));
            assert!(
                assets.insert(asset),
                "{} shares clip {asset} with another cue",
                cue_key.as_str()
            );
        }
        for key in ["menu", "match"] {
            let plan = catalog.music_plan(key).unwrap();
            assert!(assets.insert(plan.asset_id), "music {key} reuses a clip");
        }
    }

    #[test]
    fn every_weapon_delivery_family_has_its_own_fire_and_impact_cue() {
        let catalog = AudioProfileCatalog::embedded().unwrap();
        for cue_key in [
            cue_keys::FIRE_STRAIGHT,
            cue_keys::FIRE_STICKY,
            cue_keys::FIRE_LOBBED,
            cue_keys::FIRE_SPLASH,
            cue_keys::FIRE_MELEE,
            cue_keys::FIRE_SPRAY,
            cue_keys::IMPACT_PROJECTILE,
            cue_keys::IMPACT_LOB,
            cue_keys::IMPACT_MELEE,
            cue_keys::IMPACT_SPRAY,
            cue_keys::HIT_DEALT,
            cue_keys::HIT_TAKEN,
        ] {
            assert!(catalog.contains_mapping(cue_key), "{}", cue_key.as_str());
            assert!(catalog.playback_plan(cue_key, |_| true).is_some());
        }
        // Rapid spray and hit confirmations keep tight live limits.
        assert!(catalog.mapped_profile(cue_keys::HIT_DEALT).concurrency_cap <= 6);
        assert!(catalog.mapped_profile(cue_keys::HIT_TAKEN).concurrency_cap <= 6);
        assert!(catalog.mapped_profile(cue_keys::UI_CLICK).concurrency_cap <= 4);
    }

    #[test]
    fn music_tracks_cover_exactly_the_selected_keys_with_bounded_loops() {
        let catalog = AudioProfileCatalog::embedded().unwrap();
        catalog.validate_music_keys(["menu", "match"]).unwrap();
        assert_eq!(
            catalog.validate_music_keys(["menu"]).unwrap_err(),
            "music track match has no selector"
        );
        assert_eq!(
            catalog
                .validate_music_keys(["menu", "match", "credits"])
                .unwrap_err(),
            "music key credits has no catalog track"
        );
        let menu = catalog.music_plan("menu").unwrap();
        assert!((menu.loop_seconds - 19.2).abs() < f32::EPSILON);
        assert!(catalog.music_plan("missing").is_none());
    }

    /// The encoded loop decodes to its authored length plus at most one Vorbis block of
    /// padding, which `loop_seconds` trims at playback so the seam has no gap.
    #[test]
    fn music_loops_decode_to_their_authored_loop_length() {
        use bevy::audio::{AudioSource, Decodable, Source};
        let catalog = AudioProfileCatalog::embedded().unwrap();
        let paths = crate::assets::audio_asset_paths().unwrap();
        let root = std::path::Path::new(crate::REPOSITORY_ROOT);
        for key in ["menu", "match"] {
            let plan = catalog.music_plan(key).unwrap();
            let bytes = std::fs::read(root.join("assets").join(&paths[plan.asset_id])).unwrap();
            let decoder = AudioSource {
                bytes: bytes.into(),
            }
            .decoder();
            let rate = f64::from(decoder.sample_rate().get());
            let channels = f64::from(decoder.channels().get());
            let samples = f64::from(u32::try_from(decoder.count()).unwrap());
            let seconds = samples / channels / rate;
            let loop_seconds = f64::from(plan.loop_seconds);
            assert!(
                seconds > loop_seconds - 1e-4 && seconds - loop_seconds < 0.05,
                "{key} decodes to {seconds} s for a {} s loop",
                plan.loop_seconds
            );
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
            ("schema_version: 4", "schema_version: 3"),
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
            ("loop_seconds: 19.2", "loop_seconds: 0.0"),
            ("loop_seconds: 19.2", "loop_seconds: 601.0"),
            ("volume: 0.8, loop_seconds", "volume: 1.5, loop_seconds"),
            (
                "asset: \"audio.original.music-menu\"",
                "asset: \"audio.original.missing\"",
            ),
            ("key: \"menu\"", "key: \"Menu Theme\""),
            ("key: \"match\"", "key: \"menu\""),
        ] {
            assert!(
                AUDIO_PROFILE_CATALOG.contains(needle),
                "fixture needle missing: {needle}"
            );
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
            .playback_plan(cue_keys::FIRE_STRAIGHT, |_| true)
            .unwrap();
        assert_eq!(plan.asset_id, "audio.error");
        assert!((plan.speed - 0.8).abs() < f32::EPSILON);
        assert!((plan.volume - 0.5).abs() < f32::EPSILON);
    }

    #[test]
    fn missing_runtime_asset_follows_fallback_then_silences_deterministically() {
        let mut catalog = AudioProfileCatalog::embedded().unwrap();
        catalog
            .profiles
            .get_mut("fire-straight")
            .unwrap()
            .fallback_profile = "playable".to_string();
        let plan = catalog
            .playback_plan(cue_keys::FIRE_STRAIGHT, |asset| asset == "audio.ready")
            .unwrap();
        assert_eq!(plan.asset_id, "audio.ready");
        assert!(
            catalog
                .playback_plan(cue_keys::FIRE_STRAIGHT, |_| false)
                .is_none()
        );
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
            AUDIO_PROFILE_CATALOG.replacen("cue_key: \"respawn\"", "cue_key: \"Reset_Key\"", 1);
        assert!(AudioProfileCatalog::from_ron(&invalid_key).is_err());

        let mut fallback_cycle: AudioCatalogSource = ron::from_str(AUDIO_PROFILE_CATALOG).unwrap();
        fallback_cycle
            .profiles
            .iter_mut()
            .find(|profile| profile.id == "fire-straight")
            .unwrap()
            .fallback_profile = "impact-projectile".to_string();
        fallback_cycle
            .profiles
            .iter_mut()
            .find(|profile| profile.id == "impact-projectile")
            .unwrap()
            .fallback_profile = "fire-straight".to_string();
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
                        .filter(|key| *key != cue_keys::RESPAWN)
                        .map(AudioCueKey::as_str),
                )
                .unwrap_err(),
            "audio catalog mapping respawn has no registered producer"
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
