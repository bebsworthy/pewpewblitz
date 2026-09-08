//! A separate render layer keeps the weapon viewport independent of the dashboard fighter.
use crate::client::flow::WeaponPreviewHost;
use bevy::{
    asset::RenderAssetUsages,
    camera::{ClearColorConfig, RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    render::render_resource::{TextureDimension, TextureFormat, TextureUsages},
    ui::widget::ViewportNode,
    world_serialization::{WorldAssetRoot, WorldInstanceReady},
};
const LAYER: usize = 28;
#[derive(Component)]
struct WeaponScene;
#[derive(Component)]
struct WeaponOwned;
#[derive(Resource)]
struct Target {
    image: Handle<Image>,
    camera: Entity,
    host: Entity,
}
pub(super) fn install(app: &mut App) {
    app.add_systems(Update, update).add_observer(ready);
}
#[allow(clippy::too_many_arguments, clippy::needless_pass_by_value)]
fn update(
    mut commands: Commands,
    hosts: Query<Entity, With<WeaponPreviewHost>>,
    target: Option<Res<Target>>,
    owned: Query<Entity, With<WeaponOwned>>,
    mut images: ResMut<Assets<Image>>,
    imported: Option<Res<super::super::presentation_3d::Imported3dAssets>>,
) {
    let host = hosts.iter().next();
    if let Some(target) = target {
        if let Some(host) = host {
            if host != target.host {
                commands
                    .entity(host)
                    .insert(ViewportNode::new(target.camera));
                commands.insert_resource(Target {
                    image: target.image.clone(),
                    camera: target.camera,
                    host,
                });
            }
        } else {
            for e in &owned {
                commands.entity(e).despawn();
            }
            images.remove(target.image.id());
            commands.remove_resource::<Target>();
        }
        return;
    }
    let (Some(host), Some(imported)) = (host, imported) else {
        return;
    };
    let mut image = Image::new_uninit(
        default(),
        TextureDimension::D2,
        TextureFormat::Bgra8UnormSrgb,
        RenderAssetUsages::all(),
    );
    image.texture_descriptor.usage =
        TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST | TextureUsages::RENDER_ATTACHMENT;
    let image = images.add(image);
    let camera = commands
        .spawn((
            WeaponOwned,
            Camera3d::default(),
            Camera {
                order: -3,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(image.clone().into()),
            Projection::Perspective(PerspectiveProjection {
                fov: 0.5,
                ..default()
            }),
            Tonemapping::None,
            Transform::from_xyz(0.8, 0.32, 0.9).looking_at(Vec3::new(0.0, -0.05, 0.0), Vec3::Y),
            RenderLayers::layer(LAYER),
        ))
        .id();
    commands.entity(host).insert(ViewportNode::new(camera));
    commands.insert_resource(Target {
        image,
        camera,
        host,
    });
    commands.spawn((
        WeaponOwned,
        WeaponScene,
        WorldAssetRoot(imported.blaster_scene.clone()),
        Transform::default(),
        RenderLayers::layer(LAYER),
    ));
    commands.spawn((
        WeaponOwned,
        DirectionalLight {
            illuminance: 12_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(2.0, 4.0, 3.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(LAYER),
    ));
    commands.spawn((
        WeaponOwned,
        DirectionalLight {
            illuminance: 5_000.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-3.0, 2.0, -2.0).looking_at(Vec3::ZERO, Vec3::Y),
        RenderLayers::layer(LAYER),
    ));
}
#[allow(clippy::needless_pass_by_value)]
fn ready(
    event: On<WorldInstanceReady>,
    mut commands: Commands,
    roots: Query<(), With<WeaponScene>>,
    children: Query<&Children>,
) {
    if roots.get(event.entity).is_err() {
        return;
    }
    for e in children.iter_descendants(event.entity) {
        commands.entity(e).insert(RenderLayers::layer(LAYER));
    }
}
