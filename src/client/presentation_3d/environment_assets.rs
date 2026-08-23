//! Client-only environment visual profiles, handles, and imported-scene readiness.

use super::*;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};

const MAP_VISUAL_CATALOG: &str = include_str!("../../../assets/catalogs/map_asset_visuals.ron");
const MAP_THEME_CATALOG: &str =
    include_str!("../../../assets/catalogs/map_presentation_themes.ron");

#[derive(Deserialize, Clone)]
pub(super) enum MapVisualKind {
    Imported { path: String },
    GeneratedGround,
    GeneratedCover,
    GeneratedWater,
    GeneratedVegetation,
    GeneratedBarrier,
    GeneratedRubble,
    GeneratedDecoration,
    HiddenMarker,
}

#[derive(Deserialize, Clone)]
#[allow(
    dead_code,
    reason = "fallback identity is validated structurally in M01"
)]
enum MapVisualFallback {
    Wall,
    Decoration,
    Ground,
    Cover,
    Water,
    Vegetation,
    Barrier,
    Rubble,
    Hidden,
}

#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum MapVisualFitting {
    #[default]
    Exact,
    Tiled,
    Contained,
}

#[derive(Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum MapAdjacencyGroup {
    #[default]
    None,
    Water,
    Vegetation,
    Wall,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
pub(super) struct MapVisualProfile {
    pub(super) id: crate::map::MapVisualProfileId,
    pub(super) kind: MapVisualKind,
    pub(super) scale: f32,
    pub(super) yaw_degrees: f32,
    pub(super) vertical_offset: f32,
    pub(super) tint: (f32, f32, f32),
    fallback: MapVisualFallback,
    #[serde(default)]
    pub(super) fitting: MapVisualFitting,
    #[serde(default)]
    pub(super) adjacency_group: MapAdjacencyGroup,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapVisualSource {
    schema_version: u16,
    visuals: Vec<MapVisualProfile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct MapThemeSource {
    schema_version: u16,
    themes: Vec<MapThemeProfile>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
#[derive(Clone)]
pub(super) struct MapThemeProfile {
    id: crate::map::MapPresentationThemeId,
    playable_ground: (f32, f32, f32),
    ground_accent: (f32, f32, f32),
    outer_ground: (f32, f32, f32),
    fallback_wall: (f32, f32, f32),
    fallback_perimeter: (f32, f32, f32),
    destructible_cover: (f32, f32, f32),
    water: (f32, f32, f32),
    vegetation: (f32, f32, f32),
    rubble: (f32, f32, f32),
    ambient_color: (f32, f32, f32),
    pub(super) ambient_brightness: f32,
    directional_color: (f32, f32, f32),
    pub(super) directional_illuminance: f32,
}

impl MapThemeProfile {
    pub(super) fn ambient_color(&self) -> Color {
        tuple_color(self.ambient_color)
    }

    pub(super) fn directional_color(&self) -> Color {
        tuple_color(self.directional_color)
    }
}

fn validate_map_visuals(catalog: &crate::map::MapContentCatalog) -> Result<(), String> {
    let source: MapVisualSource = ron::from_str(MAP_VISUAL_CATALOG)
        .map_err(|error| format!("client map visual catalog parse failed: {error}"))?;
    let themes: MapThemeSource = ron::from_str(MAP_THEME_CATALOG)
        .map_err(|error| format!("client map theme catalog parse failed: {error}"))?;
    if source.schema_version != 3 || themes.schema_version != 3 {
        return Err("unsupported client map catalog schema".to_string());
    }
    let expected: BTreeSet<_> = catalog
        .assets
        .iter()
        .map(|asset| asset.visual_profile_id)
        .collect();
    let mut actual = BTreeSet::new();
    for profile in source.visuals {
        let kind_matches_fallback = matches!(
            (&profile.kind, &profile.fallback),
            (
                MapVisualKind::Imported { .. },
                MapVisualFallback::Wall | MapVisualFallback::Decoration
            ) | (MapVisualKind::GeneratedGround, MapVisualFallback::Ground)
                | (MapVisualKind::GeneratedCover, MapVisualFallback::Cover)
                | (MapVisualKind::GeneratedWater, MapVisualFallback::Water)
                | (
                    MapVisualKind::GeneratedVegetation,
                    MapVisualFallback::Vegetation
                )
                | (MapVisualKind::GeneratedBarrier, MapVisualFallback::Barrier)
                | (MapVisualKind::GeneratedRubble, MapVisualFallback::Rubble)
                | (
                    MapVisualKind::GeneratedDecoration,
                    MapVisualFallback::Decoration
                )
                | (MapVisualKind::HiddenMarker, MapVisualFallback::Hidden)
        );
        let fitting_matches_kind = match &profile.kind {
            MapVisualKind::GeneratedWater => {
                profile.fitting == MapVisualFitting::Tiled
                    && profile.adjacency_group == MapAdjacencyGroup::Water
            }
            MapVisualKind::GeneratedVegetation => {
                profile.fitting == MapVisualFitting::Tiled
                    && profile.adjacency_group == MapAdjacencyGroup::Vegetation
            }
            MapVisualKind::HiddenMarker => {
                profile.fitting == MapVisualFitting::Exact
                    && profile.adjacency_group == MapAdjacencyGroup::None
            }
            _ => true,
        };
        let colors = [profile.tint.0, profile.tint.1, profile.tint.2];
        if profile.id.0 == 0
            || !actual.insert(profile.id)
            || !profile.scale.is_finite()
            || profile.scale <= 0.0
            || !profile.yaw_degrees.is_finite()
            || !profile.vertical_offset.is_finite()
            || !kind_matches_fallback
            || !fitting_matches_kind
            || colors
                .into_iter()
                .any(|channel| !channel.is_finite() || !(0.0..=1.0).contains(&channel))
        {
            return Err("invalid or duplicate client map visual".to_string());
        }
        if let MapVisualKind::Imported { path } = profile.kind
            && (path.trim().is_empty()
                || std::path::Path::new(&path)
                    .extension()
                    .is_none_or(|extension| !extension.eq_ignore_ascii_case("glb")))
        {
            return Err("invalid imported client map visual path".to_string());
        }
    }
    if actual != expected {
        return Err("client visual catalog does not exactly cover shared map assets".to_string());
    }
    validate_map_themes(themes, catalog)?;
    Ok(())
}

fn validate_map_themes(
    themes: MapThemeSource,
    catalog: &crate::map::MapContentCatalog,
) -> Result<(), String> {
    let mut theme_ids = BTreeSet::new();
    for theme in themes.themes {
        let channels = [
            theme.playable_ground,
            theme.ground_accent,
            theme.outer_ground,
            theme.fallback_wall,
            theme.fallback_perimeter,
            theme.destructible_cover,
            theme.water,
            theme.vegetation,
            theme.rubble,
            theme.ambient_color,
            theme.directional_color,
        ];
        if theme.id.0 == 0
            || !theme_ids.insert(theme.id)
            || channels
                .into_iter()
                .flat_map(|color| [color.0, color.1, color.2])
                .any(|channel| !channel.is_finite() || !(0.0..=1.0).contains(&channel))
            || !theme.ambient_brightness.is_finite()
            || theme.ambient_brightness < 0.0
            || !theme.directional_illuminance.is_finite()
            || theme.directional_illuminance < 0.0
        {
            return Err("invalid or duplicate client map theme".to_string());
        }
    }
    let expected_themes: BTreeSet<_> = catalog
        .presets
        .iter()
        .map(|preset| preset.recipe.presentation_theme_id)
        .collect();
    if theme_ids != expected_themes {
        return Err("client theme catalog does not cover grid recipes".to_string());
    }
    Ok(())
}

#[derive(Resource)]
pub(super) struct MapVisualCatalog {
    profiles: BTreeMap<crate::map::MapVisualProfileId, MapVisualProfile>,
    themes: BTreeMap<crate::map::MapPresentationThemeId, MapThemeProfile>,
}

impl MapVisualCatalog {
    pub(super) fn embedded(catalog: &crate::map::MapContentCatalog) -> Result<Self, String> {
        validate_map_visuals(catalog)?;
        let source: MapVisualSource = ron::from_str(MAP_VISUAL_CATALOG)
            .map_err(|error| format!("client map visual catalog parse failed: {error}"))?;
        let themes: MapThemeSource = ron::from_str(MAP_THEME_CATALOG)
            .map_err(|error| format!("client map theme catalog parse failed: {error}"))?;
        Ok(Self {
            profiles: source
                .visuals
                .into_iter()
                .map(|profile| (profile.id, profile))
                .collect(),
            themes: themes
                .themes
                .into_iter()
                .map(|theme| (theme.id, theme))
                .collect(),
        })
    }

    pub(super) fn profile(&self, id: crate::map::MapVisualProfileId) -> Option<&MapVisualProfile> {
        self.profiles.get(&id)
    }

    pub(super) fn theme(&self, id: crate::map::MapPresentationThemeId) -> Option<&MapThemeProfile> {
        self.themes.get(&id)
    }

    pub(super) fn build_theme_materials(
        &self,
        materials: &mut Assets<StandardMaterial>,
    ) -> BTreeMap<crate::map::MapPresentationThemeId, EnvironmentThemeMaterials> {
        let matte = |color: Color| StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.82,
            metallic: 0.0,
            ..default()
        };
        self.themes
            .iter()
            .map(|(id, theme)| {
                let destructible_cover = materials.add(StandardMaterial {
                    double_sided: true,
                    cull_mode: None,
                    ..matte(tuple_color(theme.destructible_cover))
                });
                (
                    *id,
                    EnvironmentThemeMaterials {
                        floor: materials.add(matte(tuple_color(theme.playable_ground))),
                        floor_accent: materials.add(matte(tuple_color(theme.ground_accent))),
                        outer_ground: materials.add(matte(tuple_color(theme.outer_ground))),
                        wall: materials.add(matte(tuple_color(theme.fallback_wall))),
                        perimeter: materials.add(matte(tuple_color(theme.fallback_perimeter))),
                        destructible_cover,
                        water: materials.add(StandardMaterial {
                            base_color: tuple_color(theme.water),
                            metallic: 0.05,
                            perceptual_roughness: 0.25,
                            ..default()
                        }),
                        vegetation: materials.add(StandardMaterial {
                            double_sided: true,
                            cull_mode: None,
                            ..matte(tuple_color(theme.vegetation))
                        }),
                        rubble: materials.add(matte(tuple_color(theme.rubble))),
                    },
                )
            })
            .collect()
    }
}

#[derive(Clone)]
pub(crate) struct EnvironmentThemeMaterials {
    pub(crate) floor: Handle<StandardMaterial>,
    pub(crate) floor_accent: Handle<StandardMaterial>,
    pub(crate) outer_ground: Handle<StandardMaterial>,
    pub(crate) wall: Handle<StandardMaterial>,
    pub(crate) perimeter: Handle<StandardMaterial>,
    pub(crate) destructible_cover: Handle<StandardMaterial>,
    pub(crate) water: Handle<StandardMaterial>,
    pub(crate) vegetation: Handle<StandardMaterial>,
    pub(crate) rubble: Handle<StandardMaterial>,
}

#[derive(Resource, Clone)]
pub(crate) struct EnvironmentThemeMaterialCatalog {
    pub(super) materials: BTreeMap<crate::map::MapPresentationThemeId, EnvironmentThemeMaterials>,
}

impl EnvironmentThemeMaterialCatalog {
    pub(crate) fn get(
        &self,
        theme: crate::map::MapPresentationThemeId,
    ) -> Option<&EnvironmentThemeMaterials> {
        self.materials.get(&theme)
    }
}

fn tuple_color(color: (f32, f32, f32)) -> Color {
    Color::srgb(color.0, color.1, color.2)
}

#[cfg(test)]
pub(crate) fn environment_asset_paths() -> Result<Vec<String>, String> {
    let source: MapVisualSource = ron::from_str(MAP_VISUAL_CATALOG)
        .map_err(|error| format!("map visual catalog parse failed: {error}"))?;
    Ok(source
        .visuals
        .into_iter()
        .filter_map(|profile| match profile.kind {
            MapVisualKind::Imported { path } => Some(path),
            _ => None,
        })
        .collect())
}

#[derive(Resource)]
pub(super) struct EnvironmentAssetHandles {
    handles: BTreeMap<crate::map::MapVisualProfileId, Handle<Gltf>>,
}

#[derive(Component, Clone, Copy)]
pub(super) struct EnvironmentMaterialTint(pub(super) [f32; 3]);

#[derive(Resource, Default)]
pub(super) struct EnvironmentTintedMaterials {
    handles: HashMap<(AssetId<StandardMaterial>, [u32; 3]), Handle<StandardMaterial>>,
}

#[derive(Resource, Default)]
pub(super) struct EnvironmentImportedScenes {
    scenes: BTreeMap<crate::map::MapVisualProfileId, Handle<WorldAsset>>,
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub(super) enum EnvironmentAssetReadiness {
    #[default]
    Loading,
    Ready,
    Degraded(Vec<crate::map::MapVisualProfileId>),
}

impl EnvironmentImportedScenes {
    pub(super) fn scene(&self, id: crate::map::MapVisualProfileId) -> Option<&Handle<WorldAsset>> {
        self.scenes.get(&id)
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "parameters are Bevy system parameters owned by the scheduling runtime"
)]
pub(super) fn load_environment_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    maps: Res<crate::map::MapCatalogResource>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let map_visuals =
        MapVisualCatalog::embedded(&maps.0).expect("embedded client map catalogs are valid");
    let theme_materials = EnvironmentThemeMaterialCatalog {
        materials: map_visuals.build_theme_materials(&mut materials),
    };
    let mut handles = BTreeMap::new();
    for (id, profile) in &map_visuals.profiles {
        if let MapVisualKind::Imported { path } = &profile.kind {
            handles.insert(*id, asset_server.load(path.clone()));
        }
    }
    commands.insert_resource(map_visuals);
    commands.insert_resource(theme_materials);
    commands.insert_resource(EnvironmentAssetHandles { handles });
    commands.init_resource::<EnvironmentImportedScenes>();
    commands.init_resource::<EnvironmentAssetReadiness>();
    commands.init_resource::<EnvironmentTintedMaterials>();
}

#[cfg(test)]
mod grid_catalog_tests {
    #[test]
    fn client_grid_catalog_exactly_covers_shared_visuals_and_themes() {
        let shared = crate::map::MapContentCatalog::embedded().unwrap();
        let visuals = super::MapVisualCatalog::embedded(&shared).unwrap();
        assert_eq!(visuals.profiles.len(), shared.assets.len());
        assert!(
            visuals
                .theme(crate::map::MapPresentationThemeId(3))
                .is_some()
        );
        for profile in visuals.profiles.values() {
            if let super::MapVisualKind::Imported { path } = &profile.kind {
                assert!(
                    std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                        .join("assets")
                        .join(path)
                        .is_file(),
                    "missing promoted environment asset: {path}"
                );
            }
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the Bevy observer owns its triggered world-instance event"
)]
pub(super) fn tint_environment_instance(
    ready: On<WorldInstanceReady>,
    roots: Query<&EnvironmentMaterialTint>,
    children: Query<&Children>,
    mut scene_materials: Query<&mut MeshMaterial3d<StandardMaterial>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
    mut tinted: ResMut<EnvironmentTintedMaterials>,
) {
    let Ok(tint) = roots.get(ready.entity) else {
        return;
    };
    let tint_key = tint.0.map(f32::to_bits);
    if tint_key == [1.0_f32.to_bits(); 3] {
        return;
    }
    for entity in children.iter_descendants(ready.entity) {
        let Ok(mut scene_material) = scene_materials.get_mut(entity) else {
            continue;
        };
        let source = scene_material.0.clone();
        let key = (source.id(), tint_key);
        if let Some(handle) = tinted.handles.get(&key) {
            scene_material.0 = handle.clone();
            continue;
        }
        let Some(mut material) = materials.get(&source).cloned() else {
            continue;
        };
        material.base_color = multiply_color(material.base_color, tint.0);
        let handle = materials.add(material);
        tinted.handles.insert(key, handle.clone());
        scene_material.0 = handle;
    }
}

fn multiply_color(color: Color, tint: [f32; 3]) -> Color {
    let color = color.to_srgba();
    Color::srgba(
        color.red * tint[0],
        color.green * tint[1],
        color.blue * tint[2],
        color.alpha,
    )
}

#[allow(clippy::needless_pass_by_value)]
pub(super) fn prepare_environment_scenes(
    mut commands: Commands,
    handles: Option<Res<EnvironmentAssetHandles>>,
    current: Option<Res<EnvironmentImportedScenes>>,
    asset_server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    fallback_policy: Res<ImportedWorldFallbackPolicy>,
    mut readiness: Option<ResMut<EnvironmentAssetReadiness>>,
) {
    if *fallback_policy == ImportedWorldFallbackPolicy::ForcePrimitive {
        if let Some(readiness) = readiness.as_deref_mut() {
            *readiness = EnvironmentAssetReadiness::Ready;
        }
        return;
    }
    let Some(handles) = handles else { return };
    let mut scenes = current
        .as_deref()
        .map_or_else(BTreeMap::new, |current| current.scenes.clone());
    let previous_len = scenes.len();
    for (id, handle) in &handles.handles {
        if scenes.contains_key(id) || !asset_server.is_loaded_with_dependencies(handle) {
            continue;
        }
        if let Some(scene) = gltfs
            .get(handle)
            .and_then(|gltf| gltf.default_scene.clone())
        {
            scenes.insert(*id, scene);
        }
    }
    if let Some(readiness) = readiness.as_deref_mut() {
        let failed = handles
            .handles
            .iter()
            .filter_map(|(id, handle)| {
                matches!(
                    asset_server.load_state(handle),
                    bevy::asset::LoadState::Failed(_)
                )
                .then_some(*id)
            })
            .collect::<Vec<_>>();
        *readiness = if !failed.is_empty() {
            EnvironmentAssetReadiness::Degraded(failed)
        } else if scenes.len() == handles.handles.len() {
            EnvironmentAssetReadiness::Ready
        } else {
            EnvironmentAssetReadiness::Loading
        };
    }
    if scenes.len() != previous_len {
        info!(
            ready = scenes.len(),
            total = handles.handles.len(),
            "environment GLB scenes became ready"
        );
        commands.insert_resource(EnvironmentImportedScenes { scenes });
        commands.remove_resource::<Presented3dMap>();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn material_tint_preserves_alpha_and_scales_color_channels() {
        let tinted = multiply_color(Color::srgba(0.8, 0.6, 0.4, 0.5), [0.5, 0.75, 1.0]).to_srgba();

        assert!((tinted.red - 0.4).abs() < 0.001);
        assert!((tinted.green - 0.45).abs() < 0.001);
        assert!((tinted.blue - 0.4).abs() < 0.001);
        assert!((tinted.alpha - 0.5).abs() < 0.001);
    }
}
