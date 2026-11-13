//! Runtime handles, material preparation/tinting, imported-scene readiness, and fitting admission.

use super::*;
use std::collections::{BTreeMap, BTreeSet};

use super::fitting::{SceneBounds, fit_scene_to_footprint, world_asset_scene_bounds};

impl MapVisualCatalog {
    /// Toon materials for each theme's generated world surfaces (BRL-0150).
    pub(in crate::client::presentation_3d) fn build_theme_materials(
        &self,
        materials: &mut Assets<ToonMaterial>,
        shading: &ToonShading,
    ) -> BTreeMap<crate::map::MapPresentationThemeId, EnvironmentThemeMaterials> {
        let mut add = |material: StandardMaterial| materials.add(shading.material(material));
        let matte = |color: Color| StandardMaterial {
            base_color: color,
            perceptual_roughness: 0.82,
            metallic: 0.0,
            ..default()
        };
        self.themes
            .iter()
            .map(|(id, theme)| {
                let destructible_cover = add(StandardMaterial {
                    double_sided: true,
                    cull_mode: None,
                    ..matte(tuple_color(theme.destructible_cover))
                });
                (
                    *id,
                    EnvironmentThemeMaterials {
                        // Floor tones live in the tile mesh's vertex colours (BRL-0147).
                        floor: add(matte(Color::WHITE)),
                        floor_accent: add(matte(tuple_color(theme.ground_accent))),
                        outer_ground: add(matte(tuple_color(theme.outer_ground))),
                        wall: add(matte(tuple_color(theme.fallback_wall))),
                        perimeter: add(matte(tuple_color(theme.fallback_perimeter))),
                        destructible_cover,
                        water: add(StandardMaterial {
                            base_color: tuple_color(theme.water),
                            metallic: 0.05,
                            perceptual_roughness: 0.25,
                            ..default()
                        }),
                        vegetation: add(StandardMaterial {
                            double_sided: true,
                            cull_mode: None,
                            ..matte(tuple_color(theme.vegetation))
                        }),
                        rubble: add(matte(tuple_color(theme.rubble))),
                    },
                )
            })
            .collect()
    }
}

#[derive(Clone)]
pub(crate) struct EnvironmentThemeMaterials {
    pub(in crate::client) floor: Handle<ToonMaterial>,
    pub(in crate::client) floor_accent: Handle<ToonMaterial>,
    pub(in crate::client) outer_ground: Handle<ToonMaterial>,
    pub(in crate::client) wall: Handle<ToonMaterial>,
    pub(in crate::client) perimeter: Handle<ToonMaterial>,
    pub(in crate::client) destructible_cover: Handle<ToonMaterial>,
    pub(in crate::client) water: Handle<ToonMaterial>,
    pub(in crate::client) vegetation: Handle<ToonMaterial>,
    pub(in crate::client) rubble: Handle<ToonMaterial>,
}

#[derive(Resource, Clone)]
pub(crate) struct EnvironmentThemeMaterialCatalog {
    pub(in crate::client::presentation_3d) materials:
        BTreeMap<crate::map::MapPresentationThemeId, EnvironmentThemeMaterials>,
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

#[derive(Resource)]
pub(in crate::client::presentation_3d) struct EnvironmentAssetHandles {
    handles: BTreeMap<crate::map::MapVisualProfileId, Handle<Gltf>>,
}

/// Authored profile tint for an imported environment instance; such instances are toon shaded and
/// the toon conversion applies the tint.
#[derive(Component, Clone, Copy)]
#[require(ToonScene)]
pub(in crate::client::presentation_3d) struct EnvironmentMaterialTint(
    pub(in crate::client::presentation_3d) [f32; 3],
);

#[derive(Clone)]
struct EnvironmentImportedScene {
    scene: Handle<WorldAsset>,
    bounds: SceneBounds,
}

pub(in crate::client::presentation_3d) struct FittedEnvironmentScene {
    pub(in crate::client::presentation_3d) scene: Handle<WorldAsset>,
    pub(in crate::client::presentation_3d) transform: Transform,
}

#[derive(Resource, Default)]
pub(in crate::client::presentation_3d) struct EnvironmentImportedScenes {
    scenes: BTreeMap<crate::map::MapVisualProfileId, EnvironmentImportedScene>,
    rejected: BTreeSet<crate::map::MapVisualProfileId>,
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub(in crate::client::presentation_3d) enum EnvironmentAssetReadiness {
    #[default]
    Loading,
    Ready,
    Degraded(Vec<crate::map::MapVisualProfileId>),
}

impl EnvironmentImportedScenes {
    pub(in crate::client::presentation_3d) fn fitted(
        &self,
        id: crate::map::MapVisualProfileId,
        profile: &MapVisualProfile,
        footprint_world: Vec2,
    ) -> Option<FittedEnvironmentScene> {
        let imported = self.scenes.get(&id)?;
        let transform = fit_scene_to_footprint(
            imported.bounds,
            profile.fitting,
            profile.scale,
            footprint_world,
            profile.yaw_degrees.to_radians(),
            profile.vertical_offset,
        )
        .ok()?;
        Some(FittedEnvironmentScene {
            scene: imported.scene.clone(),
            transform,
        })
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "parameters are Bevy system parameters owned by the scheduling runtime"
)]
pub(in crate::client::presentation_3d) fn load_environment_assets(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    maps: Res<crate::map::MapCatalogResource>,
    mut materials: ResMut<Assets<ToonMaterial>>,
    shading: Res<ToonShading>,
) {
    let map_visuals =
        MapVisualCatalog::embedded(&maps.0).expect("embedded client map catalogs are valid");
    let theme_materials = EnvironmentThemeMaterialCatalog {
        materials: map_visuals.build_theme_materials(&mut materials, &shading),
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
}

fn admit_loaded_environment_scenes(
    handles: &EnvironmentAssetHandles,
    current: Option<&EnvironmentImportedScenes>,
    asset_server: &AssetServer,
    gltfs: &Assets<Gltf>,
    world_assets: &Assets<WorldAsset>,
    map_catalog: &crate::map::MapContentCatalog,
    visual_catalog: &MapVisualCatalog,
) -> EnvironmentImportedScenes {
    let (mut scenes, mut rejected) = current.map_or_else(
        || (BTreeMap::new(), BTreeSet::new()),
        |current| (current.scenes.clone(), current.rejected.clone()),
    );
    for (id, handle) in &handles.handles {
        if scenes.contains_key(id)
            || rejected.contains(id)
            || !asset_server.is_loaded_with_dependencies(handle)
        {
            continue;
        }
        let Some(scene) = gltfs
            .get(handle)
            .and_then(|gltf| gltf.default_scene.clone())
        else {
            warn!(
                visual_profile = id.0,
                "imported environment GLB has no default scene"
            );
            rejected.insert(*id);
            continue;
        };
        let Some(world_asset) = world_assets.get(&scene) else {
            continue;
        };
        let admission = world_asset_scene_bounds(world_asset).and_then(|bounds| {
            validate_scene_against_owned_map_assets(*id, bounds, map_catalog, visual_catalog)?;
            Ok(EnvironmentImportedScene { scene, bounds })
        });
        match admission {
            Ok(imported) => {
                scenes.insert(*id, imported);
            }
            Err(error) => {
                warn!(visual_profile = id.0, %error, "imported environment scene rejected");
                rejected.insert(*id);
            }
        }
    }
    EnvironmentImportedScenes { scenes, rejected }
}

fn environment_asset_readiness(
    handles: &EnvironmentAssetHandles,
    imported: &EnvironmentImportedScenes,
    asset_server: &AssetServer,
) -> EnvironmentAssetReadiness {
    let failed = handles.handles.iter().filter_map(|(id, handle)| {
        matches!(
            asset_server.load_state(handle),
            bevy::asset::LoadState::Failed(_)
        )
        .then_some(*id)
    });
    let degraded = failed
        .chain(imported.rejected.iter().copied())
        .collect::<BTreeSet<_>>()
        .into_iter()
        .collect::<Vec<_>>();
    let terminal_count = imported.scenes.len() + degraded.len();
    if terminal_count < handles.handles.len() {
        EnvironmentAssetReadiness::Loading
    } else if degraded.is_empty() {
        EnvironmentAssetReadiness::Ready
    } else {
        EnvironmentAssetReadiness::Degraded(degraded)
    }
}

fn publish_environment_scene_progress(
    commands: &mut Commands,
    handles: &EnvironmentAssetHandles,
    previous: Option<&EnvironmentImportedScenes>,
    imported: EnvironmentImportedScenes,
) {
    let (previous_scene_count, previous_rejected_count) = previous
        .map(|previous| (previous.scenes.len(), previous.rejected.len()))
        .unwrap_or_default();
    let changed = imported.scenes.len() != previous_scene_count
        || imported.rejected.len() != previous_rejected_count;
    if !changed {
        return;
    }
    info!(
        ready = imported.scenes.len(),
        total = handles.handles.len(),
        "environment GLB scenes became ready"
    );
    commands.insert_resource(imported);
    commands.remove_resource::<Presented3dMap>();
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "the parameters are distinct Bevy resources owned by this asset-readiness system"
)]
pub(in crate::client::presentation_3d) fn prepare_environment_scenes(
    mut commands: Commands,
    handles: Option<Res<EnvironmentAssetHandles>>,
    current: Option<Res<EnvironmentImportedScenes>>,
    asset_server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    world_assets: Res<Assets<WorldAsset>>,
    map_catalog: Res<crate::map::MapCatalogResource>,
    visual_catalog: Res<MapVisualCatalog>,
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
    let imported = admit_loaded_environment_scenes(
        &handles,
        current.as_deref(),
        &asset_server,
        &gltfs,
        &world_assets,
        &map_catalog.0,
        &visual_catalog,
    );
    if let Some(readiness) = readiness.as_deref_mut() {
        *readiness = environment_asset_readiness(&handles, &imported, &asset_server);
    }
    publish_environment_scene_progress(&mut commands, &handles, current.as_deref(), imported);
}

fn validate_scene_against_owned_map_assets(
    visual_id: crate::map::MapVisualProfileId,
    bounds: SceneBounds,
    map_catalog: &crate::map::MapContentCatalog,
    visual_catalog: &MapVisualCatalog,
) -> Result<(), String> {
    let profile = visual_catalog
        .profile(visual_id)
        .ok_or_else(|| "imported scene has no visual profile".to_string())?;
    for asset in map_catalog
        .assets
        .iter()
        .filter(|asset| asset.visual_profile_id == visual_id)
    {
        fit_scene_to_footprint(
            bounds,
            profile.fitting,
            profile.scale,
            Vec2::new(
                f32::from(asset.footprint_cells.width) * crate::map::MAP_CELL_SIZE_WORLD,
                f32::from(asset.footprint_cells.height) * crate::map::MAP_CELL_SIZE_WORLD,
            ),
            profile.yaw_degrees.to_radians(),
            profile.vertical_offset,
        )?;
    }
    Ok(())
}
