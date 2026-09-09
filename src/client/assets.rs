//! Retained client asset handles, loading state, and manifest validation.
#![allow(clippy::wildcard_imports)]

use super::*;
use bevy::asset::{LoadState, RecursiveDependencyLoadState};
use bevy::audio::AudioSource;
use bevy::gltf::Gltf;
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};

pub const CLIENT_ASSET_MANIFEST: &str = include_str!("../../assets/manifest.ron");

const RETAINED_AUDIO_ASSET_IDS: [&str; 5] = [
    "audio.fire",
    "audio.impact",
    "audio.defeat",
    "audio.ready",
    "audio.error",
];

#[derive(Resource, Clone)]
pub(crate) struct ClientAssetHandles {
    audio: BTreeMap<String, Handle<AudioSource>>,
    pub character: Handle<Gltf>,
    pub zombie_character: Handle<Gltf>,
    pub blaster: Handle<Gltf>,
    pub loading_logo: Handle<Image>,
    pub wordmark: Handle<Image>,
    pub dashboard_logo: Handle<Image>,
    pub dashboard_badges: [Handle<Image>; 4],
    pub dashboard_identity_panel: Handle<Image>,
    pub dashboard_icons: [Handle<Image>; 8],
    pub dashboard_font: Handle<Font>,
    pub settings_icon: Handle<Image>,
    pub menu_icon: Handle<Image>,
    pub build_icon: Handle<Image>,
    pub mode_icon: Handle<Image>,
    pub practice_icon: Handle<Image>,
    pub play_icon: Handle<Image>,
}

impl ClientAssetHandles {
    fn load(asset_server: &AssetServer) -> Self {
        let audio = audio_asset_paths()
            .expect("embedded client asset manifest is valid")
            .into_iter()
            .map(|(id, path)| (id, asset_server.load(path)))
            .collect();
        Self {
            audio,
            character: asset_server
                .load("brawler/models/kenney/mini-characters/character-male-a.glb"),
            zombie_character: asset_server
                .load("brawler/models/kenney/mini-characters/character-male-d.glb"),
            blaster: asset_server.load("brawler/models/kenney/blaster-kit/blaster-a.glb"),
            loading_logo: asset_server.load("brawler/ui/branding/pewpew-blitz-lockup.png"),
            wordmark: asset_server.load("brawler/ui/branding/pewpew-blitz-wordmark.png"),
            dashboard_logo: asset_server.load("brawler/ui/shell/logo.png"),
            dashboard_badges: ["wipeout", "hot-zone", "heist", "zombie"]
                .map(|name| asset_server.load(format!("brawler/ui/shell/{name}.png"))),
            dashboard_identity_panel: asset_server.load("brawler/ui/shell/panel-identity.png"),
            dashboard_icons: [
                "icon-lightning",
                "icon-crosshair",
                "icon-menu",
                "icon-player",
                "icon-arena",
                "icon-skull",
                "icon-chevron",
                "underline",
            ]
            .map(|name| asset_server.load(format!("brawler/ui/shell/{name}.png"))),
            dashboard_font: asset_server.load(ui::theme::FONT_PATH),
            settings_icon: asset_server.load("brawler/ui/icons/settings.png"),
            menu_icon: asset_server.load("brawler/ui/icons/menu.png"),
            build_icon: asset_server.load("brawler/ui/icons/build.png"),
            mode_icon: asset_server.load("brawler/ui/icons/mode.png"),
            practice_icon: asset_server.load("brawler/ui/icons/practice.png"),
            play_icon: asset_server.load("brawler/ui/icons/play.png"),
        }
    }

    fn states(&self, asset_server: &AssetServer) -> Vec<(&'static str, bool, LoadState)> {
        let mut states: Vec<_> = RETAINED_AUDIO_ASSET_IDS
            .into_iter()
            .map(|id| self.audio_state(asset_server, id))
            .collect();
        states.extend([
            (
                "model.character_male_a",
                false,
                dependency_aware_state(asset_server, &self.character),
            ),
            (
                "model.character_male_d",
                false,
                dependency_aware_state(asset_server, &self.zombie_character),
            ),
            (
                "model.blaster_a",
                false,
                dependency_aware_state(asset_server, &self.blaster),
            ),
            (
                "font.dashboard_display",
                false,
                asset_server.load_state(&self.dashboard_font),
            ),
        ]);
        states.extend(
            [
                ("ui.shell_logo", &self.dashboard_logo),
                ("ui.badge_wipeout", &self.dashboard_badges[0]),
                ("ui.badge_hot_zone", &self.dashboard_badges[1]),
                ("ui.badge_heist", &self.dashboard_badges[2]),
                ("ui.badge_zombie", &self.dashboard_badges[3]),
                ("ui.pewpew_blitz_loading_logo", &self.loading_logo),
                ("ui.identity_panel", &self.dashboard_identity_panel),
                ("ui.pewpew_blitz_wordmark", &self.wordmark),
                ("ui.icon_settings", &self.settings_icon),
                ("ui.icon_menu", &self.menu_icon),
                ("ui.icon_build", &self.build_icon),
                ("ui.icon_mode", &self.mode_icon),
                ("ui.icon_practice", &self.practice_icon),
                ("ui.icon_play", &self.play_icon),
            ]
            .into_iter()
            .map(|(name, handle)| (name, false, asset_server.load_state(handle))),
        );
        states.extend(
            self.dashboard_icons
                .iter()
                .map(|handle| ("ui.shell_icon", false, asset_server.load_state(handle))),
        );
        states
    }

    pub(crate) fn audio(&self, asset_id: &str) -> Option<&Handle<AudioSource>> {
        self.audio.get(asset_id)
    }

    fn audio_state(
        &self,
        asset_server: &AssetServer,
        asset_id: &'static str,
    ) -> (&'static str, bool, LoadState) {
        let handle = self
            .audio(asset_id)
            .expect("retained audio asset ID is present in the validated manifest");
        (asset_id, false, asset_server.load_state(handle))
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
    world_objects: Res<crate::map::ClientWorldObjectReadiness>,
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
        && matches!(
            *world_objects,
            crate::map::ClientWorldObjectReadiness::Ready
        )
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

pub(crate) fn audio_asset_paths() -> Result<BTreeMap<String, String>, String> {
    validate_manifest(CLIENT_ASSET_MANIFEST)?;
    let manifest: AssetManifest = ron::from_str(CLIENT_ASSET_MANIFEST)
        .map_err(|error| format!("asset manifest parse failed: {error}"))?;
    let paths = manifest
        .assets
        .into_iter()
        .filter(|entry| {
            entry.id.starts_with("audio.")
                && std::path::Path::new(&entry.path)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("ogg"))
        })
        .map(|entry| (entry.id, entry.path))
        .collect::<BTreeMap<_, _>>();
    if RETAINED_AUDIO_ASSET_IDS
        .iter()
        .any(|asset_id| !paths.contains_key(*asset_id))
    {
        return Err("asset manifest is missing a retained audio asset ID".to_string());
    }
    Ok(paths)
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
            || !matches!(entry.license.as_str(), "CC0-1.0" | "OFL-1.1")
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
    fn retained_asset_manifest_has_unique_complete_provenance() {
        validate_manifest(CLIENT_ASSET_MANIFEST).unwrap();
    }

    #[test]
    fn retained_audio_ids_resolve_to_the_manifest_owned_paths() {
        let paths = audio_asset_paths().unwrap();
        assert_eq!(paths["audio.fire"], "brawler/audio/fire.ogg");
        assert_eq!(paths["audio.impact"], "brawler/audio/impact.ogg");
        assert_eq!(paths["audio.defeat"], "brawler/audio/defeat.ogg");
        assert_eq!(paths["audio.ready"], "brawler/audio/ready.ogg");
        assert_eq!(paths["audio.error"], "brawler/audio/error.ogg");
    }

    #[test]
    fn dashboard_font_icons_and_licenses_are_shipped() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let promoted = [
            "brawler/ui/fonts/lilita-one-regular.ttf",
            "brawler/ui/icons/settings.png",
            "brawler/ui/icons/menu.png",
            "brawler/ui/icons/build.png",
            "brawler/ui/icons/mode.png",
            "brawler/ui/icons/practice.png",
            "brawler/ui/icons/play.png",
        ];
        let manifest: AssetManifest = ron::from_str(CLIENT_ASSET_MANIFEST).unwrap();
        let manifested = manifest
            .assets
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<HashSet<_>>();
        for file in promoted {
            assert!(
                root.join("assets").join(file).is_file(),
                "missing asset: {file}"
            );
            assert!(manifested.contains(file), "unmanifested asset: {file}");
        }
        for file in [
            "assets/licenses/LilitaOne-OFL-1.1.txt",
            "assets/licenses/kenney-game-icons.txt",
        ] {
            assert!(root.join(file).is_file(), "missing promoted asset: {file}");
        }
    }

    #[test]
    fn zombie_character_asset_is_promoted_manifested_and_licensed() {
        let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR"));
        let asset_path = "brawler/models/kenney/mini-characters/character-male-d.glb";
        let manifest: AssetManifest = ron::from_str(CLIENT_ASSET_MANIFEST).unwrap();

        assert!(root.join("assets").join(asset_path).is_file());
        assert!(
            manifest
                .assets
                .iter()
                .any(|entry| { entry.id == "model.character_male_d" && entry.path == asset_path })
        );
        assert!(
            root.join("assets/licenses/kenney-mini-characters.txt")
                .is_file()
        );
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
        assert!(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets/licenses/kaykit-block-bits.txt")
                .is_file(),
            "missing retained KayKit Block Bits license"
        );
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
