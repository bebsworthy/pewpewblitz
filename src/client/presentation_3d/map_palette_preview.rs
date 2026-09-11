//! Small production-asset viewports for the Map Maker palette.

use super::*;
use crate::client::flow::{MapPalettePreviewHost, MapPaletteVisual};
use bevy::{
    camera::{ClearColorConfig, RenderTarget, visibility::RenderLayers},
    render::render_resource::{TextureDimension, TextureFormat, TextureUsages},
    ui::widget::ViewportNode,
};

const FIRST_LAYER: usize = 18;

#[derive(Component)]
struct PalettePreviewOwned {
    host: Entity,
}

#[derive(Component)]
struct PalettePreviewTarget {
    host: Entity,
    image: Handle<Image>,
}

#[derive(Component)]
struct PaletteImportedRoot(usize);

pub(super) fn install(app: &mut App) {
    app.add_systems(Update, (spawn_previews, cleanup_previews).chain())
        .add_observer(imported_ready);
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "the Bevy system coordinates independently owned preview resources and a filtered host query"
)]
fn spawn_previews(
    mut commands: Commands,
    hosts: Query<(Entity, &MapPalettePreviewHost), Without<ViewportNode>>,
    catalog: Option<Res<crate::map::MapCatalogResource>>,
    visuals: Option<Res<environment_assets::MapVisualCatalog>>,
    imported: Option<Res<environment_assets::EnvironmentImportedScenes>>,
    readiness: Option<Res<environment_assets::EnvironmentAssetReadiness>>,
    themes: Option<Res<environment_assets::EnvironmentThemeMaterialCatalog>>,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    mut images: ResMut<Assets<Image>>,
) {
    let (Some(catalog), Some(visuals), Some(imported), Some(readiness), Some(themes)) =
        (catalog, visuals, imported, readiness, themes)
    else {
        return;
    };
    if matches!(
        *readiness,
        environment_assets::EnvironmentAssetReadiness::Loading
    ) {
        return;
    }
    for (host, preview) in &hosts {
        let layer_index = FIRST_LAYER + preview.slot.min(VISIBLE_LAYER_COUNT - 1);
        let layer = RenderLayers::layer(layer_index);
        let mut image = Image::new_uninit(
            default(),
            TextureDimension::D2,
            TextureFormat::Bgra8UnormSrgb,
            RenderAssetUsages::all(),
        );
        image.texture_descriptor.usage = TextureUsages::TEXTURE_BINDING
            | TextureUsages::COPY_DST
            | TextureUsages::RENDER_ATTACHMENT;
        let image = images.add(image);
        let camera = commands
            .spawn((
                PalettePreviewTarget {
                    host,
                    image: image.clone(),
                },
                Camera3d::default(),
                Camera {
                    order: -5,
                    clear_color: ClearColorConfig::Custom(Color::NONE),
                    ..default()
                },
                RenderTarget::Image(image.into()),
                Tonemapping::None,
                Projection::Perspective(PerspectiveProjection {
                    fov: 0.52,
                    ..default()
                }),
                Transform::from_xyz(96.0, 76.0, 96.0).looking_at(Vec3::new(0.0, 9.0, 0.0), Vec3::Y),
                layer.clone(),
            ))
            .id();
        commands.entity(host).insert(ViewportNode::new(camera));
        commands.spawn((
            PalettePreviewOwned { host },
            DirectionalLight {
                illuminance: 14_000.0,
                shadow_maps_enabled: false,
                ..default()
            },
            Transform::from_xyz(40.0, 70.0, 35.0).looking_at(Vec3::ZERO, Vec3::Y),
            layer.clone(),
        ));
        spawn_visual(
            &mut commands,
            host,
            preview,
            &catalog.0,
            &visuals,
            &imported,
            &themes,
            &primitives,
            &materials,
            layer,
        );
    }
}

const VISIBLE_LAYER_COUNT: usize = 7;

#[allow(clippy::too_many_arguments)]
fn spawn_visual(
    commands: &mut Commands,
    host: Entity,
    preview: &MapPalettePreviewHost,
    catalog: &crate::map::MapContentCatalog,
    visuals: &environment_assets::MapVisualCatalog,
    imported: &environment_assets::EnvironmentImportedScenes,
    themes: &environment_assets::EnvironmentThemeMaterialCatalog,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    layer: RenderLayers,
) {
    match preview.visual {
        MapPaletteVisual::Asset(id) => spawn_asset_visual(
            commands, host, preview, id, catalog, visuals, imported, themes, primitives, layer,
        ),
        MapPaletteVisual::Spawn(team) => {
            commands.spawn((
                PalettePreviewOwned { host },
                Mesh3d(primitives.ground_ring.clone()),
                MeshMaterial3d(if team == 0 {
                    materials.team_blue.clone()
                } else {
                    materials.team_red.clone()
                }),
                Transform::from_xyz(0.0, 2.0, 0.0)
                    .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2))
                    .with_scale(Vec3::splat(1.5)),
                layer,
            ));
        }
        MapPaletteVisual::HotZone => {
            commands.spawn((
                PalettePreviewOwned { host },
                Mesh3d(primitives.area_ring.clone()),
                MeshMaterial3d(materials.zone_boundary.clone()),
                Transform::from_xyz(0.0, 2.0, 0.0)
                    .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2)),
                layer,
            ));
        }
        MapPaletteVisual::HeistSafe(team) => {
            commands.spawn((
                PalettePreviewOwned { host },
                Mesh3d(primitives.unit_cuboid.clone()),
                MeshMaterial3d(if team == 0 {
                    materials.team_blue.clone()
                } else {
                    materials.team_red.clone()
                }),
                Transform::from_xyz(0.0, 10.0, 0.0).with_scale(Vec3::new(54.0, 20.0, 36.0)),
                layer,
            ));
        }
    }
}

#[allow(clippy::too_many_arguments)]
fn spawn_asset_visual(
    commands: &mut Commands,
    host: Entity,
    preview: &MapPalettePreviewHost,
    id: crate::map::MapAssetId,
    catalog: &crate::map::MapContentCatalog,
    visuals: &environment_assets::MapVisualCatalog,
    imported: &environment_assets::EnvironmentImportedScenes,
    themes: &environment_assets::EnvironmentThemeMaterialCatalog,
    primitives: &Primitive3dAssets,
    layer: RenderLayers,
) {
    let Some(asset) = catalog.asset(id) else {
        return;
    };
    let footprint = asset.footprint_cells;
    let profile = visuals.profile(asset.visual_profile_id);
    if let Some((profile, fitted)) = profile.zip(profile.and_then(|profile| {
        imported.fitted(
            asset.visual_profile_id,
            profile,
            Vec2::new(
                f32::from(footprint.width) * crate::map::MAP_CELL_SIZE_WORLD,
                f32::from(footprint.height) * crate::map::MAP_CELL_SIZE_WORLD,
            ),
        )
    })) {
        commands.spawn((
            PalettePreviewOwned { host },
            PaletteImportedRoot(preview.slot),
            environment_assets::EnvironmentMaterialTint([
                profile.tint.0,
                profile.tint.1,
                profile.tint.2,
            ]),
            WorldAssetRoot(fitted.scene),
            fitted.transform,
            layer,
            Name::new("Map Maker palette imported asset"),
        ));
        return;
    }
    let Some(theme) = themes.get(preview.theme) else {
        return;
    };
    let (mesh, material, transform) = if id == crate::map::WATER_ASSET {
        (
            primitives.unit_cuboid.clone(),
            theme.water.clone(),
            Transform::from_xyz(0.0, 1.0, 0.0).with_scale(Vec3::new(30.0, 2.0, 30.0)),
        )
    } else if id == crate::map::TALL_GRASS_ASSET {
        (
            primitives.map_entity.clone(),
            theme.vegetation.clone(),
            Transform::from_xyz(0.0, 11.0, 0.0).with_scale(Vec3::splat(0.8)),
        )
    } else {
        (
            primitives.cover_block.clone(),
            theme.wall.clone(),
            Transform::from_xyz(0.0, 16.0, 0.0),
        )
    };
    commands.spawn((
        PalettePreviewOwned { host },
        Mesh3d(mesh),
        MeshMaterial3d(material),
        transform,
        layer,
    ));
}

fn cleanup_previews(
    mut commands: Commands,
    hosts: Query<(), With<MapPalettePreviewHost>>,
    owned: Query<(Entity, &PalettePreviewOwned)>,
    targets: Query<(Entity, &PalettePreviewTarget)>,
    mut images: ResMut<Assets<Image>>,
) {
    for (entity, owned) in &owned {
        if hosts.get(owned.host).is_err() {
            commands.entity(entity).try_despawn();
        }
    }
    for (entity, target) in &targets {
        if hosts.get(target.host).is_err() {
            images.remove(target.image.id());
            commands.entity(entity).try_despawn();
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy observer event parameter"
)]
fn imported_ready(
    event: On<WorldInstanceReady>,
    mut commands: Commands,
    roots: Query<&PaletteImportedRoot>,
    children: Query<&Children>,
) {
    let Ok(root) = roots.get(event.entity) else {
        return;
    };
    let layer = RenderLayers::layer(FIRST_LAYER + root.0.min(VISIBLE_LAYER_COUNT - 1));
    for entity in children.iter_descendants(event.entity) {
        commands.entity(entity).insert(layer.clone());
    }
}
