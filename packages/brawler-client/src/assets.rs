//! Retained client asset handles, loading state, and manifest validation.

#[allow(
    clippy::wildcard_imports,
    reason = "the child module consumes its parent module's composition surface"
)]
use super::*;
use bevy::asset::{LoadState, RecursiveDependencyLoadState};
use bevy::audio::AudioSource;
use bevy::gltf::Gltf;
use serde::Deserialize;
use std::collections::{BTreeMap, HashSet};

pub const CLIENT_ASSET_MANIFEST: &str = include_str!("../../../assets/manifest.ron");

const RETAINED_AUDIO_ASSET_IDS: [&str; 5] = [
    "audio.fire",
    "audio.impact",
    "audio.defeat",
    "audio.ready",
    "audio.error",
];

/// The branded lockup the connection screens show; also loaded directly by screens spawned before
/// the handle resource exists.
pub(crate) const LOADING_LOGO_PATH: &str = "brawler/ui/branding/pewpew-blitz-lockup.png";

#[derive(Resource, Clone)]
pub(crate) struct ClientAssetHandles {
    audio: BTreeMap<String, Handle<AudioSource>>,
    pub character: Handle<Gltf>,
    pub zombie_character: Handle<Gltf>,
    /// One held weapon per weapon family, in `WeaponFamily::index` order (BRL-q29d).
    pub weapons: [Handle<Gltf>; 4],
    pub loading_logo: Handle<Image>,
    pub wordmark: Handle<Image>,
    pub dashboard_logo: Handle<Image>,
    pub dashboard_badges: [Handle<Image>; 4],
    pub dashboard_identity_panel: Handle<Image>,
    pub dashboard_icons: [Handle<Image>; 7],
    pub dashboard_underline: Handle<Image>,
    pub dashboard_font: Handle<Font>,
}

impl ClientAssetHandles {
    /// Placeholder handles with a caller-chosen shell font, so headless flow tests can tell the
    /// shell typography from Bevy's default font.
    #[cfg(test)]
    pub(crate) fn with_font_for_test(dashboard_font: Handle<Font>) -> Self {
        Self {
            audio: BTreeMap::new(),
            character: Handle::default(),
            zombie_character: Handle::default(),
            weapons: std::array::from_fn(|_| Handle::default()),
            loading_logo: Handle::default(),
            wordmark: Handle::default(),
            dashboard_logo: Handle::default(),
            dashboard_badges: std::array::from_fn(|_| Handle::default()),
            dashboard_identity_panel: Handle::default(),
            dashboard_icons: std::array::from_fn(|_| Handle::default()),
            dashboard_underline: Handle::default(),
            dashboard_font,
        }
    }

    fn load(asset_server: &AssetServer) -> Self {
        let audio = audio_asset_paths()
            .expect("embedded client asset manifest is valid")
            .into_iter()
            .map(|(id, path)| (id, asset_server.load(path)))
            .collect();
        Self {
            audio,
            character: asset_server.load(presentation_3d::RIFF_ASSET_PATH),
            zombie_character: asset_server
                .load("brawler/models/kenney/mini-characters/character-male-d.glb"),
            weapons: {
                let catalog = presentation_3d::FighterWeaponCatalog::embedded()
                    .expect("embedded fighter weapon catalog is valid");
                presentation_3d::WeaponFamily::ALL
                    .map(|family| asset_server.load(catalog.model(family).model.clone()))
            },
            loading_logo: asset_server.load(LOADING_LOGO_PATH),
            wordmark: asset_server.load("brawler/ui/branding/pewpew-blitz-wordmark.png"),
            dashboard_logo: asset_server.load("brawler/ui/shell/logo.png"),
            dashboard_badges: ui::icons::MODE_PENNANTS
                .map(|name| asset_server.load(ui::icons::path(name))),
            dashboard_identity_panel: asset_server.load("brawler/ui/shell/panel-identity.png"),
            dashboard_icons: [
                "lightning",
                "crosshair",
                "menu",
                "player",
                "arena",
                "skull",
                "chevron-right",
            ]
            .map(|name| asset_server.load(ui::icons::path(name))),
            dashboard_underline: asset_server.load("brawler/ui/shell/underline.png"),
            dashboard_font: asset_server.load(ui::theme::FONT_PATH),
        }
    }

    fn states(&self, asset_server: &AssetServer) -> Vec<(&'static str, bool, LoadState)> {
        let mut states: Vec<_> = RETAINED_AUDIO_ASSET_IDS
            .into_iter()
            .map(|id| self.audio_state(asset_server, id))
            .collect();
        states.extend([
            (
                "model.riff",
                false,
                dependency_aware_state(asset_server, &self.character),
            ),
            (
                "model.character_male_d",
                false,
                dependency_aware_state(asset_server, &self.zombie_character),
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
                ("ui.shell_underline", &self.dashboard_underline),
            ]
            .into_iter()
            .map(|(name, handle)| (name, false, asset_server.load_state(handle))),
        );
        states.extend(
            presentation_3d::WeaponFamily::ALL
                .into_iter()
                .zip(&self.weapons)
                .map(|(family, handle)| {
                    (
                        family.asset_id(),
                        false,
                        dependency_aware_state(asset_server, handle),
                    )
                }),
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
                update_asset_readiness.after(brawler::map::MapPresentationSet::Readiness),
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
    map: Res<brawler::map::ClientMapReadiness>,
    world_objects: Res<brawler::map::ClientWorldObjectReadiness>,
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
        && matches!(*map, brawler::map::ClientMapReadiness::Ready)
        && matches!(
            *world_objects,
            brawler::map::ClientWorldObjectReadiness::Ready
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
            || !valid_license(&entry)
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

/// Third-party entries cite a public license and source. Original project audio (BRL-0158) has
/// no URL: it points at its checked-in generator and the repository's original-audio notice.
fn valid_license(entry: &AssetManifestEntry) -> bool {
    match entry.license.as_str() {
        "CC0-1.0" | "OFL-1.1" => {
            entry.license_url.starts_with("https://") && entry.source_url.starts_with("https://")
        }
        "Original" => {
            entry.original.starts_with("asset_src/")
                && entry.source_url.starts_with("asset_src/")
                && entry
                    .license_url
                    .starts_with("assets/licenses/pewpew-blitz-original-")
        }
        _ => false,
    }
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
    fn every_manifest_audio_file_ships_and_original_audio_cites_its_generator() {
        let root = std::path::Path::new(REPOSITORY_ROOT);
        let manifest: AssetManifest = ron::from_str(CLIENT_ASSET_MANIFEST).unwrap();
        let mut original = 0;
        for entry in manifest
            .assets
            .iter()
            .filter(|entry| entry.id.starts_with("audio."))
        {
            assert!(
                root.join("assets").join(&entry.path).is_file(),
                "{} is missing",
                entry.path
            );
            if entry.license == "Original" {
                original += 1;
                let (script, clip) = entry.original.split_once('#').unwrap();
                assert!(root.join(script).is_file());
                assert!(entry.path.ends_with(&format!("original/{clip}.ogg")));
                assert!(root.join(&entry.license_url).is_file());
                assert!(root.join(&entry.source_url).is_file());
            }
        }
        assert!(original > 0);

        let mut forged: AssetManifest = ron::from_str(CLIENT_ASSET_MANIFEST).unwrap();
        let entry = forged
            .assets
            .iter_mut()
            .find(|entry| entry.license == "Original")
            .unwrap();
        entry.original = "downloads/unknown.ogg".to_string();
        assert!(!valid_license(entry));
        entry.original = "asset_src/audio/build_audio.py#x".to_string();
        entry.license = "CC-BY-4.0".to_string();
        assert!(!valid_license(entry));
    }

    /// Every shipped clip decodes with the client's own Vorbis decoder (lewton via rodio).
    #[test]
    fn every_manifest_audio_clip_decodes_with_the_client_decoder() {
        use bevy::audio::{Decodable, Source};
        let root = std::path::Path::new(REPOSITORY_ROOT);
        for (id, path) in audio_asset_paths().unwrap() {
            let bytes = std::fs::read(root.join("assets").join(&path)).unwrap();
            let source = AudioSource {
                bytes: bytes.into(),
            };
            let decoder = source.decoder();
            let rate = decoder.sample_rate();
            let samples = decoder.count();
            assert_eq!(rate.get(), 44_100, "{id}");
            assert!(samples > 0, "{id} decoded no samples");
        }
    }

    #[test]
    fn dashboard_font_icons_and_licenses_are_shipped() {
        let root = std::path::Path::new(REPOSITORY_ROOT);
        let promoted = ["brawler/ui/fonts/lilita-one-regular.ttf"];
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
            "assets/licenses/pewpew-blitz-original-icons.txt",
        ] {
            assert!(root.join(file).is_file(), "missing promoted asset: {file}");
        }
    }

    #[test]
    fn zombie_character_asset_is_promoted_manifested_and_licensed() {
        let root = std::path::Path::new(REPOSITORY_ROOT);
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
        let root = std::path::Path::new(REPOSITORY_ROOT).join("assets/brawler/ui/branding");
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
    fn environment_catalog_assets_are_original_kit_pieces_and_retired_packs_are_gone() {
        let manifest: AssetManifest = ron::from_str(CLIENT_ASSET_MANIFEST).unwrap();
        let manifested = manifest
            .assets
            .iter()
            .map(|entry| entry.path.as_str())
            .collect::<HashSet<_>>();
        let root = std::path::Path::new(REPOSITORY_ROOT).join("assets");
        for asset_path in presentation_3d::environment_assets::environment_asset_paths().unwrap() {
            // Original kits are provenanced by their `asset_src/` builder, not the manifest.
            assert!(
                asset_path.starts_with("brawler/models/original/environment/"),
                "environment visual outside the original kits: {asset_path}"
            );
            assert!(
                root.join(&asset_path).is_file(),
                "missing original environment asset: {asset_path}"
            );
        }
        // Third-party environment packs are retired (BRL-0188): only original kits remain.
        let manifested_environment = manifested
            .iter()
            .filter(|path| {
                path.starts_with("brawler/models/")
                    && !path.starts_with("brawler/models/kenney/mini-characters/")
            })
            .collect::<Vec<_>>();
        assert!(
            manifested_environment.is_empty(),
            "retired third-party environment rows remain: {manifested_environment:?}"
        );
        for license in [
            "kenney-mini-arena.txt",
            "kenney-mini-dungeon.txt",
            "kenney-mini-forest.txt",
            "kenney-graveyard-kit.txt",
            "kaykit-block-bits.txt",
            "kenney-blaster-kit.txt",
        ] {
            assert!(
                !std::path::Path::new(REPOSITORY_ROOT)
                    .join("assets/licenses")
                    .join(license)
                    .exists(),
                "retired environment pack license still ships: {license}"
            );
        }
        assert!(
            std::path::Path::new(REPOSITORY_ROOT)
                .join("assets/licenses/pewpew-blitz-original-environment.txt")
                .is_file(),
            "missing original environment kit provenance"
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
