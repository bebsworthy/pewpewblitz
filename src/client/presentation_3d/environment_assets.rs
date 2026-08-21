//! Client-only environment visual profiles, handles, and imported-scene readiness.

use super::*;
use serde::Deserialize;
use std::collections::{BTreeMap, BTreeSet, HashMap};

pub(crate) const ENVIRONMENT_VISUAL_CATALOG: &str =
    include_str!("../../../assets/catalogs/environment_visuals.ron");

#[derive(Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum EnvironmentFallback {
    Wall,
    Block,
    Column,
    Decoration,
    Boundary,
}

#[derive(Deserialize, Clone, Debug)]
pub(super) struct EnvironmentVisualProfile {
    pub(super) variant_id: crate::map::MapVisualVariantId,
    pub(super) path: String,
    pub(super) scale: f32,
    pub(super) yaw_degrees: f32,
    pub(super) vertical_offset: f32,
    material_tint: (f32, f32, f32),
    pub(super) fallback: EnvironmentFallback,
}

#[derive(Deserialize)]
struct EnvironmentVisualCatalogSource {
    schema_version: u16,
    visuals: Vec<EnvironmentVisualProfile>,
    themes: Vec<EnvironmentThemeProfile>,
}

#[derive(Resource, Clone)]
pub(crate) struct EnvironmentVisualCatalog {
    profiles: BTreeMap<crate::map::MapVisualVariantId, EnvironmentVisualProfile>,
    themes: BTreeMap<crate::map::MapPresentationThemeId, EnvironmentThemeProfile>,
}

#[derive(Deserialize, Clone, Debug)]
pub(crate) struct EnvironmentThemeProfile {
    pub(crate) theme_id: crate::map::MapPresentationThemeId,
    playable_ground: (f32, f32, f32),
    ground_accent: (f32, f32, f32),
    outer_ground: (f32, f32, f32),
    fallback_wall: (f32, f32, f32),
    fallback_perimeter: (f32, f32, f32),
    terrain: (f32, f32, f32),
    ambient_color: (f32, f32, f32),
    pub(crate) ambient_brightness: f32,
    directional_color: (f32, f32, f32),
    pub(crate) directional_illuminance: f32,
}

impl EnvironmentThemeProfile {
    fn colors(&self) -> [[f32; 3]; 8] {
        [
            tuple_channels(self.playable_ground),
            tuple_channels(self.ground_accent),
            tuple_channels(self.outer_ground),
            tuple_channels(self.fallback_wall),
            tuple_channels(self.fallback_perimeter),
            tuple_channels(self.terrain),
            tuple_channels(self.ambient_color),
            tuple_channels(self.directional_color),
        ]
    }

    pub(crate) fn ambient_color(&self) -> Color {
        tuple_color(self.ambient_color)
    }

    pub(crate) fn directional_color(&self) -> Color {
        tuple_color(self.directional_color)
    }
}

#[derive(Clone)]
pub(crate) struct EnvironmentThemeMaterials {
    pub(crate) floor: Handle<StandardMaterial>,
    pub(crate) floor_accent: Handle<StandardMaterial>,
    pub(crate) outer_ground: Handle<StandardMaterial>,
    pub(crate) wall: Handle<StandardMaterial>,
    pub(crate) perimeter: Handle<StandardMaterial>,
    pub(crate) terrain: Handle<StandardMaterial>,
}

#[derive(Resource, Clone)]
pub(crate) struct EnvironmentThemeMaterialCatalog {
    materials: BTreeMap<crate::map::MapPresentationThemeId, EnvironmentThemeMaterials>,
}

impl EnvironmentThemeMaterialCatalog {
    pub(crate) fn get(
        &self,
        theme: crate::map::MapPresentationThemeId,
    ) -> Option<&EnvironmentThemeMaterials> {
        self.materials.get(&theme)
    }
}

impl EnvironmentVisualCatalog {
    pub(crate) fn embedded(objects: &crate::map::MapObjectCatalog) -> Result<Self, String> {
        let source: EnvironmentVisualCatalogSource = ron::from_str(ENVIRONMENT_VISUAL_CATALOG)
            .map_err(|error| format!("environment visual catalog parse failed: {error}"))?;
        if source.schema_version != 2 || source.visuals.is_empty() || source.themes.is_empty() {
            return Err("environment visual catalog schema or entries are invalid".to_string());
        }
        let mut profiles = BTreeMap::new();
        let mut paths = BTreeSet::new();
        for profile in source.visuals {
            if objects.variant(profile.variant_id).is_none()
                || profile.path.trim().is_empty()
                || !std::path::Path::new(&profile.path)
                    .extension()
                    .is_some_and(|extension| extension.eq_ignore_ascii_case("glb"))
                || !profile.scale.is_finite()
                || profile.scale <= 0.0
                || !profile.yaw_degrees.is_finite()
                || !profile.vertical_offset.is_finite()
                || profile
                    .tint()
                    .iter()
                    .any(|channel| !channel.is_finite() || !(0.0..=1.0).contains(channel))
                || !paths.insert(profile.path.clone())
                || profiles.insert(profile.variant_id, profile).is_some()
            {
                return Err("environment visual profile is invalid or duplicated".to_string());
            }
        }
        let known_themes = objects
            .themes()
            .map(|theme| theme.id)
            .collect::<BTreeSet<_>>();
        let mut themes = BTreeMap::new();
        for theme in source.themes {
            if !known_themes.contains(&theme.theme_id)
                || theme
                    .colors()
                    .into_iter()
                    .flatten()
                    .any(|channel| !channel.is_finite() || !(0.0..=1.0).contains(&channel))
                || !theme.ambient_brightness.is_finite()
                || theme.ambient_brightness < 0.0
                || !theme.directional_illuminance.is_finite()
                || theme.directional_illuminance < 0.0
                || themes.insert(theme.theme_id, theme).is_some()
            {
                return Err("environment theme profile is invalid or duplicated".to_string());
            }
        }
        if themes.keys().copied().collect::<BTreeSet<_>>() != known_themes {
            return Err(
                "environment theme profiles do not cover the shared theme catalog".to_string(),
            );
        }
        Ok(Self { profiles, themes })
    }

    pub(super) fn profile(
        &self,
        id: crate::map::MapVisualVariantId,
    ) -> Option<&EnvironmentVisualProfile> {
        self.profiles.get(&id)
    }

    pub(crate) fn theme(
        &self,
        id: crate::map::MapPresentationThemeId,
    ) -> Option<&EnvironmentThemeProfile> {
        self.themes.get(&id)
    }

    pub(crate) fn build_theme_materials(
        &self,
        materials: &mut Assets<StandardMaterial>,
    ) -> EnvironmentThemeMaterialCatalog {
        let matte = |color: Color| StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.82,
            metallic: 0.0,
            ..default()
        };
        let materials = self
            .themes
            .iter()
            .map(|(id, theme)| {
                (
                    *id,
                    EnvironmentThemeMaterials {
                        floor: materials.add(matte(tuple_color(theme.playable_ground))),
                        floor_accent: materials.add(matte(tuple_color(theme.ground_accent))),
                        outer_ground: materials.add(matte(tuple_color(theme.outer_ground))),
                        wall: materials.add(matte(tuple_color(theme.fallback_wall))),
                        perimeter: materials.add(matte(tuple_color(theme.fallback_perimeter))),
                        terrain: materials.add(StandardMaterial {
                            double_sided: true,
                            cull_mode: None,
                            ..matte(tuple_color(theme.terrain))
                        }),
                    },
                )
            })
            .collect();
        EnvironmentThemeMaterialCatalog { materials }
    }
}

fn tuple_color(color: (f32, f32, f32)) -> Color {
    Color::srgb(color.0, color.1, color.2)
}

fn tuple_channels(color: (f32, f32, f32)) -> [f32; 3] {
    [color.0, color.1, color.2]
}

impl EnvironmentVisualProfile {
    pub(super) fn tint(&self) -> [f32; 3] {
        [
            self.material_tint.0,
            self.material_tint.1,
            self.material_tint.2,
        ]
    }
}

#[cfg(test)]
pub(crate) fn environment_asset_paths() -> Result<Vec<String>, String> {
    let source: EnvironmentVisualCatalogSource = ron::from_str(ENVIRONMENT_VISUAL_CATALOG)
        .map_err(|error| format!("environment visual catalog parse failed: {error}"))?;
    Ok(source
        .visuals
        .into_iter()
        .map(|profile| profile.path)
        .collect())
}

#[derive(Resource)]
pub(super) struct EnvironmentAssetHandles {
    handles: BTreeMap<crate::map::MapVisualVariantId, Handle<Gltf>>,
}

#[derive(Component, Clone, Copy)]
pub(super) struct EnvironmentMaterialTint(pub(super) [f32; 3]);

#[derive(Resource, Default)]
pub(super) struct EnvironmentTintedMaterials {
    handles: HashMap<(AssetId<StandardMaterial>, [u32; 3]), Handle<StandardMaterial>>,
}

#[derive(Resource, Default)]
pub(super) struct EnvironmentImportedScenes {
    scenes: BTreeMap<crate::map::MapVisualVariantId, Handle<WorldAsset>>,
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub(super) enum EnvironmentAssetReadiness {
    #[default]
    Loading,
    Ready,
    Degraded(Vec<crate::map::MapVisualVariantId>),
}

impl EnvironmentImportedScenes {
    pub(super) fn scene(&self, id: crate::map::MapVisualVariantId) -> Option<&Handle<WorldAsset>> {
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
    objects: Res<crate::map::MapObjectCatalogResource>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let catalog = EnvironmentVisualCatalog::embedded(&objects.0)
        .expect("embedded environment visual catalog is valid");
    let theme_materials = catalog.build_theme_materials(&mut materials);
    let handles = catalog
        .profiles
        .iter()
        .map(|(id, profile)| (*id, asset_server.load(profile.path.clone())))
        .collect();
    commands.insert_resource(catalog);
    commands.insert_resource(theme_materials);
    commands.insert_resource(EnvironmentAssetHandles { handles });
    commands.init_resource::<EnvironmentImportedScenes>();
    commands.init_resource::<EnvironmentAssetReadiness>();
    commands.init_resource::<EnvironmentTintedMaterials>();
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
    fn embedded_environment_profiles_resolve_and_exist() {
        let objects = crate::map::MapObjectCatalog::embedded().unwrap();
        let catalog = EnvironmentVisualCatalog::embedded(&objects).unwrap();
        assert_eq!(catalog.profiles.len(), 22);
        assert_eq!(catalog.themes.len(), 2);
        for profile in catalog.profiles.values() {
            let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("assets")
                .join(&profile.path);
            assert!(
                path.is_file(),
                "missing promoted environment asset: {path:?}"
            );
        }
    }

    #[test]
    fn material_tint_preserves_alpha_and_scales_color_channels() {
        let tinted = multiply_color(Color::srgba(0.8, 0.6, 0.4, 0.5), [0.5, 0.75, 1.0]).to_srgba();

        assert!((tinted.red - 0.4).abs() < 0.001);
        assert!((tinted.green - 0.45).abs() < 0.001);
        assert!((tinted.blue - 0.4).abs() < 0.001);
        assert!((tinted.alpha - 0.5).abs() < 0.001);
    }
}
