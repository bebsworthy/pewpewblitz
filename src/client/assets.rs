//! Retained client asset handles, loading state, and manifest validation.
#![allow(clippy::wildcard_imports)]

use super::*;
use bevy::asset::LoadState;
use bevy::audio::AudioSource;
use serde::Deserialize;
use std::collections::HashSet;

pub const CLIENT_ASSET_MANIFEST: &str = include_str!("../../assets/manifest.ron");

#[derive(Resource, Clone)]
pub(crate) struct ClientAssetHandles {
    pub team_blue: Handle<Image>,
    pub team_red: Handle<Image>,
    pub facility_tileset: Handle<Image>,
    pub fire: Handle<AudioSource>,
    pub impact: Handle<AudioSource>,
    pub defeat: Handle<AudioSource>,
    pub ready: Handle<AudioSource>,
    pub error: Handle<AudioSource>,
}

impl ClientAssetHandles {
    fn load(asset_server: &AssetServer) -> Self {
        Self {
            team_blue: asset_server.load("brawler/fighters/team_blue.png"),
            team_red: asset_server.load("brawler/fighters/team_red.png"),
            facility_tileset: asset_server.load("brawler/maps/facility_tileset.png"),
            fire: asset_server.load("brawler/audio/fire.ogg"),
            impact: asset_server.load("brawler/audio/impact.ogg"),
            defeat: asset_server.load("brawler/audio/defeat.ogg"),
            ready: asset_server.load("brawler/audio/ready.ogg"),
            error: asset_server.load("brawler/audio/error.ogg"),
        }
    }

    fn states(&self, asset_server: &AssetServer) -> [(&'static str, bool, LoadState); 8] {
        [
            (
                "fighter.team_blue",
                true,
                asset_server.load_state(&self.team_blue),
            ),
            (
                "fighter.team_red",
                true,
                asset_server.load_state(&self.team_red),
            ),
            (
                "map.facility_tileset",
                false,
                asset_server.load_state(&self.facility_tileset),
            ),
            ("audio.fire", false, asset_server.load_state(&self.fire)),
            ("audio.impact", false, asset_server.load_state(&self.impact)),
            ("audio.defeat", false, asset_server.load_state(&self.defeat)),
            ("audio.ready", false, asset_server.load_state(&self.ready)),
            ("audio.error", false, asset_server.load_state(&self.error)),
        ]
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

fn load_client_assets(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(ClientAssetHandles::load(&asset_server));
}

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
            || entry.imported != "2026-08-15"
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn retained_asset_manifest_has_unique_complete_cc0_provenance() {
        validate_manifest(CLIENT_ASSET_MANIFEST).unwrap();
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
            readiness_from_observations(false, vec!["audio.fire", "map.facility_tileset"]),
            ClientAssetReadiness::Degraded(vec!["audio.fire", "map.facility_tileset"])
        );
    }
}
