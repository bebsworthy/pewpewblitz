//! Client-only Dashboard fighter viewport and presentation lifecycle.

use super::{ClientFlow, flow::DashboardPreviewHost};
use bevy::{
    asset::RenderAssetUsages,
    camera::{ClearColorConfig, RenderTarget, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    prelude::*,
    reflect::TypePath,
    render::render_resource::{AsBindGroup, TextureDimension, TextureFormat, TextureUsages},
    shader::ShaderRef,
    ui::widget::ViewportNode,
    world_serialization::{WorldAssetRoot, WorldInstanceReady},
};
use core::time::Duration;

const DASHBOARD_RENDER_LAYER: usize = 29;

#[derive(Resource)]
struct DashboardPreviewTarget(Handle<Image>);

#[derive(Resource)]
struct DashboardBackgroundTarget(Handle<DashboardBackgroundMaterial>);

#[derive(Component)]
struct DashboardBackground;

#[derive(AsBindGroup, Asset, TypePath, Debug, Clone)]
struct DashboardBackgroundMaterial {
    #[uniform(0)]
    time_motion: Vec4,
    #[uniform(1)]
    palette_dark: Vec4,
    #[uniform(2)]
    palette_light: Vec4,
    #[uniform(3)]
    glow: Vec4,
}

impl UiMaterial for DashboardBackgroundMaterial {
    fn fragment_shader() -> ShaderRef {
        "brawler/shaders/dashboard_background.wgsl".into()
    }
}

#[derive(Component)]
struct DashboardPreviewRoot;

#[derive(Component)]
struct DashboardPreviewCamera;

#[derive(Component)]
struct DashboardPreviewFallback;

#[derive(Component)]
struct DashboardImportedSpawned;

#[derive(Component, Clone, Copy)]
enum DashboardImportedScene {
    Character,
    Blaster,
}

pub(super) struct ClientDashboardPlugin;

impl Plugin for ClientDashboardPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(UiMaterialPlugin::<DashboardBackgroundMaterial>::default())
            .add_systems(
                Update,
                (
                    ensure_dashboard_background,
                    update_dashboard_background,
                    spawn_dashboard_preview,
                    upgrade_dashboard_preview,
                )
                    .chain()
                    .run_if(in_state(ClientFlow::Dashboard)),
            )
            .add_systems(
                OnExit(ClientFlow::Dashboard),
                (
                    release_dashboard_preview_target,
                    release_dashboard_background_target,
                ),
            )
            .add_observer(setup_dashboard_imported_scene);
    }
}

fn ensure_dashboard_background(
    mut commands: Commands,
    existing: Query<(), With<DashboardBackground>>,
    mut materials: ResMut<Assets<DashboardBackgroundMaterial>>,
) {
    if !existing.is_empty() {
        return;
    }
    commands.spawn((
        DashboardBackground,
        DespawnOnExit(ClientFlow::Dashboard),
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            ..default()
        },
        BackgroundColor(Color::srgb(0.018, 0.035, 0.065)),
        GlobalZIndex(408),
    ));
    let material = materials.add(DashboardBackgroundMaterial {
        time_motion: Vec4::new(0.0, 1.0, 0.0, 0.0),
        palette_dark: Color::srgb(0.012, 0.025, 0.055).to_linear().to_vec4(),
        palette_light: Color::srgb(0.035, 0.15, 0.22).to_linear().to_vec4(),
        glow: Vec4::new(0.5, 0.46, 0.18, 0.27),
    });
    commands.spawn((
        DashboardBackground,
        DespawnOnExit(ClientFlow::Dashboard),
        Node {
            position_type: PositionType::Absolute,
            left: px(0),
            right: px(0),
            top: px(0),
            bottom: px(0),
            ..default()
        },
        MaterialNode(material.clone()),
        GlobalZIndex(409),
    ));
    commands.insert_resource(DashboardBackgroundTarget(material));
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy owns presentation time and persisted settings as system resources"
)]
fn update_dashboard_background(
    time: Res<Time>,
    settings: Res<super::ClientShellSettings>,
    target: Option<Res<DashboardBackgroundTarget>>,
    mut materials: ResMut<Assets<DashboardBackgroundMaterial>>,
) {
    let Some(target) = target else {
        return;
    };
    let Some(mut material) = materials.get_mut(&target.0) else {
        return;
    };
    material.time_motion.x = if settings.reduced_motion {
        5.0
    } else {
        time.elapsed_secs()
    };
    material.time_motion.y = if settings.reduced_motion { 0.0 } else { 1.0 };
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "dashboard entry creates one camera, target, light, and bounded preview hierarchy"
)]
fn spawn_dashboard_preview(
    mut commands: Commands,
    hosts: Query<Entity, With<DashboardPreviewHost>>,
    mut images: ResMut<Assets<Image>>,
    primitives: Res<super::presentation_3d::Primitive3dAssets>,
    materials: Res<super::presentation_3d::Material3dAssets>,
    imported: Option<Res<super::presentation_3d::Imported3dAssets>>,
    existing: Option<Res<DashboardPreviewTarget>>,
) {
    if existing.is_some() {
        return;
    }
    let Some(host) = hosts.iter().next() else {
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
    let target = images.add(image);
    let layer = RenderLayers::layer(DASHBOARD_RENDER_LAYER);
    let camera = commands
        .spawn((
            DashboardPreviewCamera,
            DespawnOnExit(ClientFlow::Dashboard),
            Camera3d::default(),
            Camera {
                order: -2,
                clear_color: ClearColorConfig::Custom(Color::NONE),
                ..default()
            },
            RenderTarget::Image(target.clone().into()),
            Projection::Perspective(PerspectiveProjection {
                fov: 0.48,
                ..default()
            }),
            Tonemapping::None,
            Transform::from_xyz(235.0, 65.0, 80.0).looking_at(Vec3::new(0.0, 38.0, 0.0), Vec3::Y),
            layer.clone(),
        ))
        .id();
    commands.entity(host).insert(ViewportNode::new(camera));
    commands.insert_resource(DashboardPreviewTarget(target));

    commands.spawn((
        DespawnOnExit(ClientFlow::Dashboard),
        DirectionalLight {
            illuminance: 9_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_rotation(Quat::from_euler(EulerRot::XYZ, -0.65, -0.55, 0.0)),
        layer.clone(),
    ));
    commands.spawn((
        DespawnOnExit(ClientFlow::Dashboard),
        PointLight {
            color: Color::srgb(0.05, 0.72, 0.92),
            intensity: 1_800_000.0,
            range: 220.0,
            shadow_maps_enabled: false,
            ..default()
        },
        Transform::from_xyz(-58.0, 88.0, -28.0),
        layer.clone(),
    ));
    let root = commands
        .spawn((
            DashboardPreviewRoot,
            DespawnOnExit(ClientFlow::Dashboard),
            Transform::default(),
            Visibility::default(),
            layer.clone(),
        ))
        .with_children(|root| {
            root.spawn((
                Mesh3d(primitives.sentry_base.clone()),
                MeshMaterial3d(materials.zone_fill.clone()),
                Transform::from_xyz(0.0, -2.0, 0.0).with_scale(Vec3::new(2.25, 0.45, 2.25)),
                layer.clone(),
            ));
            root.spawn((
                DashboardPreviewFallback,
                Mesh3d(primitives.fighter.clone()),
                MeshMaterial3d(materials.team_blue.clone()),
                Transform::from_xyz(0.0, 72.0, 0.0).with_scale(Vec3::splat(1.55)),
                layer.clone(),
            ));
            root.spawn((
                DashboardPreviewFallback,
                Mesh3d(primitives.unit_cuboid.clone()),
                MeshMaterial3d(materials.neutral.clone()),
                Transform::from_xyz(34.0, 75.0, 5.0).with_scale(Vec3::new(48.0, 11.0, 14.0)),
                layer.clone(),
            ));
        })
        .id();
    if let Some(imported) = imported {
        spawn_imported_character(&mut commands, root, &imported, layer);
    }
}

fn spawn_imported_character(
    commands: &mut Commands,
    root: Entity,
    imported: &super::presentation_3d::Imported3dAssets,
    layer: RenderLayers,
) {
    commands.entity(root).with_children(|parent| {
        parent.spawn((
            DashboardImportedScene::Character,
            WorldAssetRoot(imported.character_scene.clone()),
            Transform {
                rotation: Quat::from_rotation_y(
                    super::presentation_3d::KENNEY_CHARACTER_FORWARD_CORRECTION - 0.12,
                ),
                scale: Vec3::splat(88.0),
                ..default()
            },
            layer,
            Name::new("Dashboard imported brawler"),
        ));
    });
    commands.entity(root).insert(DashboardImportedSpawned);
}

fn upgrade_dashboard_preview(
    mut commands: Commands,
    imported: Option<Res<super::presentation_3d::Imported3dAssets>>,
    roots: Query<
        Entity,
        (
            With<DashboardPreviewRoot>,
            Without<DashboardImportedSpawned>,
        ),
    >,
) {
    let Some(imported) = imported else {
        return;
    };
    for root in &roots {
        spawn_imported_character(
            &mut commands,
            root,
            &imported,
            RenderLayers::layer(DASHBOARD_RENDER_LAYER),
        );
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "the scene-ready boundary validates and binds the imported dashboard hierarchy"
)]
fn setup_dashboard_imported_scene(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    imported: Option<Res<super::presentation_3d::Imported3dAssets>>,
    scene_roots: Query<&DashboardImportedScene>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut players: Query<&mut AnimationPlayer>,
    mut fallbacks: Query<&mut Visibility, With<DashboardPreviewFallback>>,
) {
    let Ok(scene) = scene_roots.get(ready.entity) else {
        return;
    };
    let layer = RenderLayers::layer(DASHBOARD_RENDER_LAYER);
    let descendants = children.iter_descendants(ready.entity).collect::<Vec<_>>();
    commands.entity(ready.entity).insert(layer.clone());
    for descendant in &descendants {
        commands.entity(*descendant).insert(layer.clone());
    }
    if matches!(scene, DashboardImportedScene::Blaster) {
        return;
    }
    let Some(imported) = imported else {
        return;
    };
    let arm = descendants.iter().copied().find(|entity| {
        names
            .get(*entity)
            .is_ok_and(|name| name.as_str() == "arm-right")
    });
    let player = descendants
        .iter()
        .copied()
        .find(|entity| players.get(*entity).is_ok());
    let (Some(arm), Some(player)) = (arm, player) else {
        return;
    };
    commands.entity(arm).with_children(|parent| {
        parent.spawn((
            DashboardImportedScene::Blaster,
            WorldAssetRoot(imported.blaster_scene.clone()),
            Transform {
                translation: Vec3::new(0.08, 0.0, 0.0),
                rotation: Quat::from_rotation_y(
                    super::presentation_3d::KENNEY_BLASTER_GRIP_ROTATION,
                ),
                ..default()
            },
            layer,
            Name::new("Dashboard attached blaster-a"),
        ));
    });
    if let Ok(mut animation_player) = players.get_mut(player) {
        let mut transitions = AnimationTransitions::new();
        transitions
            .play(&mut animation_player, imported.holding, Duration::ZERO)
            .repeat();
        commands
            .entity(player)
            .insert(AnimationGraphHandle(imported.animation_graph.clone()))
            .insert(transitions);
    }
    for mut visibility in &mut fallbacks {
        *visibility = Visibility::Hidden;
    }
}

fn release_dashboard_preview_target(
    mut commands: Commands,
    target: Option<Res<DashboardPreviewTarget>>,
    mut images: ResMut<Assets<Image>>,
) {
    if let Some(target) = target {
        images.remove(target.0.id());
        commands.remove_resource::<DashboardPreviewTarget>();
    }
}

fn release_dashboard_background_target(
    mut commands: Commands,
    target: Option<Res<DashboardBackgroundTarget>>,
    mut materials: ResMut<Assets<DashboardBackgroundMaterial>>,
) {
    if let Some(target) = target {
        materials.remove(target.0.id());
        commands.remove_resource::<DashboardBackgroundTarget>();
    }
}
