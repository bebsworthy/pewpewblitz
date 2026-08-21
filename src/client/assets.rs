//! Retained client asset handles, loading state, and manifest validation.
#![allow(clippy::wildcard_imports)]

use super::*;
use bevy::asset::{LoadState, RecursiveDependencyLoadState};
use bevy::audio::AudioSource;
use bevy::gltf::Gltf;
use serde::Deserialize;
use std::collections::HashSet;

pub const CLIENT_ASSET_MANIFEST: &str = include_str!("../../assets/manifest.ron");

#[derive(Resource, Clone)]
pub(crate) struct ClientAssetHandles {
    pub fire: Handle<AudioSource>,
    pub impact: Handle<AudioSource>,
    pub defeat: Handle<AudioSource>,
    pub ready: Handle<AudioSource>,
    pub error: Handle<AudioSource>,
    pub character: Handle<Gltf>,
    pub blaster: Handle<Gltf>,
    pub loading_logo: Handle<Image>,
    pub wordmark: Handle<Image>,
}

impl ClientAssetHandles {
    fn load(asset_server: &AssetServer) -> Self {
        Self {
            fire: asset_server.load("brawler/audio/fire.ogg"),
            impact: asset_server.load("brawler/audio/impact.ogg"),
            defeat: asset_server.load("brawler/audio/defeat.ogg"),
            ready: asset_server.load("brawler/audio/ready.ogg"),
            error: asset_server.load("brawler/audio/error.ogg"),
            character: asset_server
                .load("brawler/models/kenney/mini-characters/character-male-a.glb"),
            blaster: asset_server.load("brawler/models/kenney/blaster-kit/blaster-a.glb"),
            loading_logo: asset_server.load("brawler/ui/branding/pewpew-blitz-lockup.png"),
            wordmark: asset_server.load("brawler/ui/branding/pewpew-blitz-wordmark.png"),
        }
    }

    fn states(&self, asset_server: &AssetServer) -> [(&'static str, bool, LoadState); 9] {
        [
            ("audio.fire", false, asset_server.load_state(&self.fire)),
            ("audio.impact", false, asset_server.load_state(&self.impact)),
            ("audio.defeat", false, asset_server.load_state(&self.defeat)),
            ("audio.ready", false, asset_server.load_state(&self.ready)),
            ("audio.error", false, asset_server.load_state(&self.error)),
            (
                "model.character_male_a",
                false,
                dependency_aware_state(asset_server, &self.character),
            ),
            (
                "model.blaster_a",
                false,
                dependency_aware_state(asset_server, &self.blaster),
            ),
            (
                "ui.pewpew_blitz_loading_logo",
                false,
                asset_server.load_state(&self.loading_logo),
            ),
            (
                "ui.pewpew_blitz_wordmark",
                false,
                asset_server.load_state(&self.wordmark),
            ),
        ]
    }
}

fn dependency_aware_state<T: Asset>(asset_server: &AssetServer, handle: &Handle<T>) -> LoadState {
    match (
        asset_server.load_state(handle),
        asset_server.recursive_dependency_load_state(handle),
    ) {
        (LoadState::Failed(error), _) | (_, RecursiveDependencyLoadState::Failed(error)) => {
            LoadState::Failed(error)
        }
        (LoadState::Loaded, RecursiveDependencyLoadState::Loaded) => LoadState::Loaded,
        (LoadState::NotLoaded, RecursiveDependencyLoadState::NotLoaded) => LoadState::NotLoaded,
        _ => LoadState::Loading,
    }
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub enum ClientAssetReadiness {
    #[default]
    Loading,
    Ready,
    Degraded(Vec<&'static str>),
}

pub struct ClientAssetPlugin;

impl Plugin for ClientAssetPlugin {
    fn build(&self, app: &mut App) {
        validate_manifest(CLIENT_ASSET_MANIFEST).expect("embedded client asset manifest is valid");
        app.init_resource::<ClientAssetReadiness>()
            .add_systems(Startup, load_client_assets)
            .add_systems(
                Update,
                update_asset_readiness.after(crate::map::MapPresentationSet::Readiness),
            );
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn load_client_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(ClientAssetHandles::load(&asset_server));
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn update_asset_readiness(
    handles: Option<Res<ClientAssetHandles>>,
    asset_server: Res<AssetServer>,
    map: Res<crate::map::ClientMapReadiness>,
    joins: Query<&ClientJoinStatus>,
    mut readiness: ResMut<ClientAssetReadiness>,
    mut playable: ResMut<ClientPlayableGate>,
) {
    let Some(handles) = handles else {
        playable.0 = false;
        return;
    };
    let states = handles.states(&asset_server);
    let loading = states.iter().any(|(_, required, state)| {
        *required && matches!(state, LoadState::NotLoaded | LoadState::Loading)
    });
    let failed: Vec<_> = states
        .iter()
        .filter_map(|(id, _, state)| matches!(state, LoadState::Failed(_)).then_some(*id))
        .collect();
    let next = readiness_from_observations(loading, failed);
    if *readiness != next {
        match &next {
            ClientAssetReadiness::Degraded(failed) => {
                warn!(
                    ?failed,
                    "client assets degraded; declared fallbacks are active"
                );
            }
            ClientAssetReadiness::Ready => info!("client assets ready"),
            ClientAssetReadiness::Loading => {}
        }
        *readiness = next;
    }
    let joined = joins
        .iter()
        .any(|status| matches!(status.phase, ClientJoinPhase::Active { .. }));
    playable.0 = joined
        && matches!(*map, crate::map::ClientMapReadiness::Ready)
        && !matches!(*readiness, ClientAssetReadiness::Loading);
}

fn readiness_from_observations(loading: bool, failed: Vec<&'static str>) -> ClientAssetReadiness {
    if loading {
        ClientAssetReadiness::Loading
    } else if failed.is_empty() {
        ClientAssetReadiness::Ready
    } else {
        // Every retained asset declares a deterministic primitive or silence fallback.
        ClientAssetReadiness::Degraded(failed)
    }
}

#[derive(Deserialize)]
struct AssetManifest {
    schema_version: u16,
    assets: Vec<AssetManifestEntry>,
}

#[derive(Deserialize)]
struct AssetManifestEntry {
    id: String,
    path: String,
    pack: String,
    original: String,
    author: String,
    license: String,
    license_url: String,
    source_url: String,
    imported: String,
    required: bool,
    fallback: String,
}

fn validate_manifest(source: &str) -> Result<(), String> {
    let manifest: AssetManifest =
        ron::from_str(source).map_err(|error| format!("asset manifest parse failed: {error}"))?;
    if manifest.schema_version != 1 || manifest.assets.is_empty() {
        return Err("asset manifest schema or entries are invalid".to_string());
    }
    let mut ids = HashSet::new();
    let mut paths = HashSet::new();
    for entry in manifest.assets {
        if !ids.insert(entry.id.clone()) || !paths.insert(entry.path.clone()) {
            return Err(format!("duplicate asset id or path: {}", entry.id));
        }
        if entry.pack.is_empty()
            || entry.original.is_empty()
            || entry.author.is_empty()
            || entry.license != "CC0-1.0"
            || !entry.license_url.starts_with("https://")
            || !entry.source_url.starts_with("https://")
            || !valid_import_date(&entry.imported)
            || entry.fallback.is_empty()
        {
            return Err(format!("asset provenance is incomplete: {}", entry.id));
        }
        if entry.required && entry.fallback == "silence" {
            return Err(format!(
                "required visual has no visual fallback: {}",
                entry.id
            ));
        }
    }
    Ok(())
}

fn valid_import_date(value: &str) -> bool {
    let bytes = value.as_bytes();
    if bytes.len() != 10 || bytes[4] != b'-' || bytes[7] != b'-' {
        return false;
    }
    let Ok(year) = value[0..4].parse::<u16>() else {
        return false;
    };
    let Ok(month) = value[5..7].parse::<u8>() else {
        return false;
    };
    let Ok(day) = value[8..10].parse::<u8>() else {
        return false;
    };
    year >= 2026 && (1..=12).contains(&month) && (1..=31).contains(&day)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_asset_manifest_has_unique_complete_cc0_provenance() {
        validate_manifest(CLIENT_ASSET_MANIFEST).unwrap();
    }

    #[test]
    fn pewpew_blitz_branding_assets_are_shipped_rgba_pngs() {
        let root =
            std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets/brawler/ui/branding");
        for file in ["pewpew-blitz-lockup.png", "pewpew-blitz-wordmark.png"] {
            let bytes = std::fs::read(root.join(file)).unwrap();
            assert!(bytes.len() > 33, "{file} is not a complete PNG");
            assert_eq!(&bytes[..8], b"\x89PNG\r\n\x1a\n");
            assert_eq!(&bytes[12..16], b"IHDR");
            let width = u32::from_be_bytes(bytes[16..20].try_into().unwrap());
            let height = u32::from_be_bytes(bytes[20..24].try_into().unwrap());
            assert!(width > 0 && height > 0);
            assert_eq!(bytes[25], 6, "{file} must retain RGBA color data");
        }
    }

    #[test]
    fn environment_catalog_assets_are_promoted_manifested_and_dependency_complete() {
        let manifest: AssetManifest = ron::from_str(CLIENT_ASSET_MANIFEST).unwrap();
        let manifested = manifest
            .assets
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<HashSet<_>>();
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("assets");
        for asset_path in presentation_3d::environment_assets::environment_asset_paths().unwrap() {
            assert!(
                manifested.contains(asset_path.as_str()),
                "unmanifested environment asset: {asset_path}"
            );
            assert!(
                root.join(&asset_path).is_file(),
                "missing promoted environment asset: {asset_path}"
            );
        }
        for dependency in [
            "brawler/models/kenney/mini-arena/Textures/colormap.png",
            "brawler/models/kenney/mini-dungeon/Textures/colormap.png",
            "brawler/models/kenney/mini-forest/Textures/colormap.png",
            "brawler/models/kenney/graveyard/Textures/colormap.png",
        ] {
            assert!(
                manifested.contains(dependency),
                "unmanifested GLB dependency: {dependency}"
            );
            assert!(
                root.join(dependency).is_file(),
                "missing GLB dependency: {dependency}"
            );
        }
    }

    #[test]
    fn import_dates_are_strict_iso_calendar_shapes() {
        assert!(valid_import_date("2026-08-20"));
        assert!(!valid_import_date("2026-8-20"));
        assert!(!valid_import_date("2026-13-20"));
        assert!(!valid_import_date("not-a-date"));
    }

    #[test]
    fn readiness_waits_for_loading_and_reports_exact_degraded_ids() {
        assert_eq!(
            readiness_from_observations(true, vec!["audio.fire"]),
            ClientAssetReadiness::Loading
        );
        assert_eq!(
            readiness_from_observations(false, Vec::new()),
            ClientAssetReadiness::Ready
        );
        assert_eq!(
            readiness_from_observations(false, vec!["audio.fire", "audio.impact"]),
            ClientAssetReadiness::Degraded(vec!["audio.fire", "audio.impact"])
        );
    }
}
