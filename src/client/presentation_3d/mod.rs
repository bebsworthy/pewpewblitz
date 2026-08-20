//! Default V3 world renderer over the unchanged planar simulation.
#![allow(clippy::wildcard_imports)]

use super::presentation::{spawn_client_hud, spawn_pause_overlay};
use super::*;
use crate::combat::client::CombatClientSet;
use bevy::{
    camera::{ScalingMode, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    gltf::Gltf,
    light::{GlobalAmbientLight, NotShadowCaster, NotShadowReceiver},
    math::primitives::Annulus,
    world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady},
};
use core::time::Duration;

mod camera;
mod combat;
pub(crate) mod coordinates;
mod diagnostics;

#[cfg(test)]
use camera::clamp_3d_camera_center;
pub(super) use camera::cursor_ground_point;
use camera::{
    CAMERA_DISTANCE, CAMERA_ELEVATION_RADIANS, CAMERA_VERTICAL_SPAN_3D, follow_3d_camera,
};
use coordinates::{ground_extents, ground_point, ground_position, ground_rotation};

const WALL_HEIGHT: f32 = 72.0;
const GROUND_OFFSET: f32 = 1.0;
const ZONE_RING_WIDTH: f32 = 28.0;
// Kenney's Mini Characters face local +Z, while Brawler fighter roots face local +X.
const KENNEY_CHARACTER_FORWARD_CORRECTION: f32 = core::f32::consts::FRAC_PI_2;
// Blaster Kit barrels also point local +Z, so the corrected character hierarchy needs no extra yaw.
const KENNEY_BLASTER_GRIP_ROTATION: f32 = 0.0;
// Keep a straight shot nearly on its authoritative plane. A larger lift produces a strong
// screen-space parallax offset under the tilted orthographic camera and makes it miss the muzzle.
const STRAIGHT_PROJECTILE_HEIGHT: f32 = 4.0;
const LOBBED_PROJECTILE_LAUNCH_HEIGHT: f32 = 20.0;
const STRAIGHT_PROJECTILE_CATCH_UP_MULTIPLIER: f32 = 3.0;

#[derive(Resource)]
pub(crate) struct Primitive3dAssets {
    pub(crate) floor_tile: Handle<Mesh>,
    pub(crate) cover_block: Handle<Mesh>,
    pub(crate) map_entity: Handle<Mesh>,
    pub(crate) debris: Handle<Mesh>,
    pub(crate) fighter: Handle<Mesh>,
    pub(crate) direction: Handle<Mesh>,
    pub(crate) projectile: Handle<Mesh>,
    pub(crate) lobbed_projectile: Handle<Mesh>,
    pub(crate) unit_cuboid: Handle<Mesh>,
    pub(crate) sentry_base: Handle<Mesh>,
    pub(crate) sentry_body: Handle<Mesh>,
    pub(crate) ground_ring: Handle<Mesh>,
    pub(crate) effect_sphere: Handle<Mesh>,
}

#[derive(Resource)]
pub(crate) struct Material3dAssets {
    pub(crate) floor: Handle<StandardMaterial>,
    pub(crate) wall: Handle<StandardMaterial>,
    pub(crate) perimeter: Handle<StandardMaterial>,
    pub(crate) team_blue: Handle<StandardMaterial>,
    pub(crate) team_red: Handle<StandardMaterial>,
    pub(crate) marker_local: Handle<StandardMaterial>,
    pub(crate) marker_ally: Handle<StandardMaterial>,
    pub(crate) marker_enemy: Handle<StandardMaterial>,
    pub(crate) neutral: Handle<StandardMaterial>,
    pub(crate) zone_fill: Handle<StandardMaterial>,
    pub(crate) zone_boundary: Handle<StandardMaterial>,
    pub(crate) terrain: Handle<StandardMaterial>,
    pub(crate) health_back: Handle<StandardMaterial>,
    pub(crate) health_fill: Handle<StandardMaterial>,
    pub(crate) preview: Handle<StandardMaterial>,
    pub(crate) preview_blocked: Handle<StandardMaterial>,
    pub(crate) status_slow: Handle<StandardMaterial>,
    pub(crate) status_knockback: Handle<StandardMaterial>,
    pub(crate) effect_muzzle: Handle<StandardMaterial>,
    pub(crate) effect_impact: Handle<StandardMaterial>,
    pub(crate) effect_damage: Handle<StandardMaterial>,
    pub(crate) dash: Handle<StandardMaterial>,
}

#[derive(Component)]
struct V3WorldMember;

#[derive(Component)]
struct V3FighterVisual {
    last_position: Vec2,
    moving: bool,
    shoot_seconds: f32,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(crate) struct CombatVisualOwner(pub(crate) Entity);

#[derive(Component)]
struct V3FallbackVisual {
    owner: Entity,
}

#[derive(Component)]
struct V3CharacterScene {
    owner: Entity,
    visual_root: Entity,
}

#[derive(Component)]
struct V3CharacterRuntime {
    owner: Entity,
    visual_root: Entity,
    player: Entity,
    current: CharacterMotion,
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum CharacterMotion {
    Idle,
    Holding,
    Walk,
    Shoot,
    Defeated,
}

#[derive(Resource)]
struct Imported3dAssets {
    character_scene: Handle<WorldAsset>,
    blaster_scene: Handle<WorldAsset>,
    animation_graph: Handle<AnimationGraph>,
    idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    holding: AnimationNodeIndex,
    shoot: AnimationNodeIndex,
    defeated: AnimationNodeIndex,
}

#[derive(Resource)]
struct ImportedArenaAssets {
    block_scene: Handle<WorldAsset>,
}

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) enum ImportedWorldFallbackPolicy {
    #[default]
    Auto,
    ForcePrimitive,
}

impl ImportedWorldFallbackPolicy {
    fn from_environment() -> Self {
        Self::from_value(env::var("BRAWLER_FORCE_PRIMITIVE_WORLD").ok().as_deref())
    }

    fn from_value(value: Option<&str>) -> Self {
        match value {
            Some("1" | "true" | "yes") => Self::ForcePrimitive,
            _ => Self::Auto,
        }
    }
}

#[derive(Component)]
struct V3ProjectileVisual {
    planar_position: Vec2,
}

#[derive(Component)]
struct V3ZoneFill;

#[derive(Component)]
struct V3ZoneBoundary;

#[derive(Component)]
struct GeneratedMapMesh(Handle<Mesh>);

#[derive(Resource, Clone, Copy)]
struct Presented3dMap(crate::map::MapInstanceId);

/// Client-only 3D world composition. Gameplay authority remains planar and server-owned.
pub(super) struct WorldPresentationPlugin;

impl Plugin for WorldPresentationPlugin {
    fn build(&self, app: &mut App) {
        if let Some(config) = app
            .world()
            .resource::<ClientNetworkConfig>()
            .render_measurement
            .clone()
        {
            app.add_plugins(diagnostics::RenderMeasurementPlugin(config));
        }
        app.insert_resource(ImportedWorldFallbackPolicy::from_environment())
            .insert_resource(GlobalAmbientLight {
                color: Color::srgb(0.72, 0.78, 0.9),
                brightness: 350.0,
                ..default()
            })
            .add_systems(Startup, setup_3d_foundation)
            .add_systems(
                Update,
                (
                    prepare_imported_assets,
                    reconcile_3d_map.in_set(crate::map::MapPresentationSet::Materialize3d),
                    combat::reconcile_combat_visuals,
                    upgrade_fighters_to_imported_models,
                    combat::consume_combat_cues,
                    combat::update_combat_visual_state,
                    update_character_animation,
                    combat::cleanup_combat_effects,
                    tint_3d_zone,
                )
                    .chain()
                    .after(CombatClientSet::Sync),
            )
            .add_systems(
                PostUpdate,
                (combat::write_combat_visual_poses, follow_3d_camera)
                    .chain()
                    .after(InterpolationSystems::Interpolate)
                    .after(PhysicsSystems::Writeback)
                    .before(TransformSystems::Propagate),
            )
            .add_observer(setup_imported_character);
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "startup constructs one bounded shared mesh/material palette and the two cameras"
)]
fn setup_3d_foundation(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let primitives = Primitive3dAssets {
        floor_tile: meshes.add(Cuboid::new(64.0, 1.0, 64.0)),
        cover_block: meshes.add(Cuboid::new(64.0, 32.0, 64.0)),
        map_entity: meshes.add(Cuboid::new(24.0, 24.0, 24.0)),
        debris: meshes.add(Cuboid::new(12.0, 8.0, 12.0)),
        fighter: meshes.add(Sphere::new(24.0)),
        direction: meshes.add(Cuboid::new(28.0, 7.0, 8.0)),
        projectile: meshes.add(Cylinder::new(4.0, 28.0)),
        lobbed_projectile: meshes.add(Sphere::new(9.0)),
        unit_cuboid: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        sentry_base: meshes.add(Cylinder::new(22.0, 8.0)),
        sentry_body: meshes.add(Cylinder::new(15.0, 24.0)),
        ground_ring: meshes.add(Annulus::new(18.0, 22.0)),
        effect_sphere: meshes.add(Sphere::new(1.0)),
    };
    let matte = |color: Color| StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.82,
        metallic: 0.0,
        ..default()
    };
    let material_assets = Material3dAssets {
        floor: materials.add(matte(Color::srgb(0.055, 0.075, 0.10))),
        wall: materials.add(matte(Color::srgb(0.10, 0.36, 0.58))),
        perimeter: materials.add(matte(Color::srgb(0.25, 0.72, 0.92))),
        team_blue: materials.add(matte(Color::srgb(0.12, 0.72, 0.96))),
        team_red: materials.add(matte(Color::srgb(1.0, 0.42, 0.12))),
        marker_local: materials.add(StandardMaterial {
            base_color: Color::srgb(0.18, 0.95, 0.36),
            unlit: true,
            ..default()
        }),
        marker_ally: materials.add(StandardMaterial {
            base_color: Color::srgb(0.12, 0.72, 0.96),
            unlit: true,
            ..default()
        }),
        marker_enemy: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.18, 0.14),
            unlit: true,
            ..default()
        }),
        neutral: materials.add(matte(Color::srgb(0.72, 0.76, 0.82))),
        zone_fill: materials.add(StandardMaterial {
            base_color: Color::srgba(0.2, 0.5, 0.95, 0.30),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        zone_boundary: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.82, 0.2),
            emissive: LinearRgba::new(1.2, 0.8, 0.08, 1.0),
            unlit: true,
            ..default()
        }),
        terrain: materials.add(StandardMaterial {
            double_sided: true,
            cull_mode: None,
            ..matte(Color::srgb(0.44, 0.38, 0.29))
        }),
        health_back: materials.add(StandardMaterial {
            base_color: Color::srgb(0.025, 0.03, 0.04),
            unlit: true,
            ..default()
        }),
        health_fill: materials.add(StandardMaterial {
            base_color: Color::srgb(0.2, 0.95, 0.35),
            unlit: true,
            ..default()
        }),
        preview: materials.add(StandardMaterial {
            base_color: Color::srgba(0.95, 0.78, 0.22, 0.38),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        preview_blocked: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.16, 0.16, 0.55),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        status_slow: materials.add(StandardMaterial {
            base_color: Color::srgba(0.25, 0.75, 1.0, 0.82),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        status_knockback: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.55, 0.18, 0.82),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        effect_muzzle: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.8, 0.2),
            emissive: LinearRgba::new(2.0, 1.2, 0.1, 1.0),
            unlit: true,
            ..default()
        }),
        effect_impact: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.3, 0.08),
            emissive: LinearRgba::new(1.5, 0.2, 0.02, 1.0),
            unlit: true,
            ..default()
        }),
        effect_damage: materials.add(StandardMaterial {
            base_color: Color::srgb(0.95, 0.08, 0.12),
            emissive: LinearRgba::new(1.2, 0.02, 0.02, 1.0),
            unlit: true,
            ..default()
        }),
        dash: materials.add(StandardMaterial {
            base_color: Color::srgba(0.25, 0.9, 1.0, 0.48),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
    };
    commands.insert_resource(primitives);
    commands.insert_resource(material_assets);

    let offset = Vec3::new(
        0.0,
        CAMERA_ELEVATION_RADIANS.sin(),
        CAMERA_ELEVATION_RADIANS.cos(),
    ) * CAMERA_DISTANCE;
    commands.spawn((
        Camera3d::default(),
        Msaa::Sample4,
        ArenaCamera,
        Projection::Orthographic(OrthographicProjection {
            scaling_mode: ScalingMode::FixedVertical {
                viewport_height: CAMERA_VERTICAL_SPAN_3D,
            },
            near: 0.1,
            far: 3_000.0,
            ..OrthographicProjection::default_3d()
        }),
        Tonemapping::None,
        Transform::from_translation(offset).looking_at(Vec3::ZERO, Vec3::Y),
        V3WorldMember,
    ));
    commands.spawn((
        Camera2d,
        IsDefaultUiCamera,
        Camera {
            order: 1,
            clear_color: ClearColorConfig::None,
            ..default()
        },
        Tonemapping::None,
        RenderLayers::layer(31),
    ));
    commands.spawn((
        DirectionalLight {
            illuminance: 8_000.0,
            shadow_maps_enabled: true,
            ..default()
        },
        Transform::from_xyz(-500.0, 900.0, 650.0).looking_at(Vec3::ZERO, Vec3::Y),
        V3WorldMember,
    ));
    spawn_pause_overlay(commands.reborrow());
    spawn_client_hud(commands);
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the map materializer needs each distinct ECS owner and retained asset resource"
)]
fn reconcile_3d_map(
    mut commands: Commands,
    mut meshes: ResMut<Assets<Mesh>>,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    imported: Option<Res<ImportedArenaAssets>>,
    accepted: Option<Res<crate::map::PresentedMap>>,
    current: Option<Res<Presented3dMap>>,
    snapshots: Query<&crate::map::ResolvedMapSnapshot, With<crate::map::MapRoot>>,
    members: Query<(
        Entity,
        &crate::map::MapPresentationMember,
        Option<&GeneratedMapMesh>,
    )>,
) {
    let Some(accepted) = accepted else {
        for (entity, _, generated) in &members {
            if let Some(generated) = generated {
                meshes.remove(generated.0.id());
            }
            commands.entity(entity).try_despawn();
        }
        commands.remove_resource::<Presented3dMap>();
        return;
    };
    let Ok(snapshot) = snapshots.get(accepted.source_root) else {
        return;
    };
    if current.is_some_and(|current| current.0 == snapshot.identity.instance_id) {
        return;
    }
    for (entity, _, generated) in &members {
        if let Some(generated) = generated {
            meshes.remove(generated.0.id());
        }
        commands.entity(entity).try_despawn();
    }
    let marker = crate::map::MapPresentationMember {
        instance_id: snapshot.identity.instance_id,
    };
    let bounds = snapshot.playable_bounds;
    for visual in &snapshot.visual_instances {
        let mut translation = ground_position(visual.position);
        translation.y = -0.5;
        commands.spawn((
            marker,
            Mesh3d(primitives.floor_tile.clone()),
            MeshMaterial3d(materials.floor.clone()),
            NotShadowCaster,
            Transform {
                translation,
                rotation: Quat::from_rotation_y(visual.rotation),
                ..default()
            },
            Name::new("V3 resolved floor tile"),
        ));
    }
    for geometry in &snapshot.geometry {
        match geometry.shape {
            crate::map::MapShape::Rectangle { half_extents } => {
                let size = half_extents * 2.0;
                if let Some(imported) = imported.as_deref()
                    && spawn_imported_wall_modules(
                        &mut commands,
                        imported,
                        geometry.position,
                        geometry.rotation,
                        size,
                        snapshot.identity.instance_id,
                    )
                {
                    // The selected Mini Arena block is exactly 1x0.5x1 source units;
                    // uniform scale 64 gives the authoritative 64x64 module footprint.
                } else {
                    spawn_fallback_wall_modules(
                        &mut commands,
                        &mut meshes,
                        &primitives,
                        &materials,
                        geometry.position,
                        geometry.rotation,
                        size,
                        snapshot.identity.instance_id,
                    );
                }
            }
            crate::map::MapShape::Circle { radius } => {
                let mut translation = ground_position(geometry.position);
                translation.y = WALL_HEIGHT * 0.5;
                let mesh = meshes.add(Cylinder::new(radius, WALL_HEIGHT));
                commands.spawn((
                    marker,
                    Mesh3d(mesh.clone()),
                    GeneratedMapMesh(mesh),
                    MeshMaterial3d(materials.wall.clone()),
                    Transform::from_translation(translation),
                    Name::new("V3 circular cover"),
                ));
            }
        }
    }
    for (position, size) in crate::map::perimeter_visual_shapes(bounds) {
        let mut translation = ground_position(position);
        translation.y = WALL_HEIGHT * 0.5;
        let mesh = meshes.add(Cuboid::new(size.x, WALL_HEIGHT, size.y));
        commands.spawn((
            marker,
            Mesh3d(mesh.clone()),
            GeneratedMapMesh(mesh),
            MeshMaterial3d(materials.perimeter.clone()),
            Transform::from_translation(translation),
            Name::new("V3 arena perimeter"),
        ));
    }
    for entity in &snapshot.entities {
        commands.spawn((
            marker,
            Mesh3d(primitives.map_entity.clone()),
            MeshMaterial3d(materials.neutral.clone()),
            Transform {
                translation: ground_position(entity.position) + Vec3::Y * 12.0,
                rotation: Quat::from_rotation_y(entity.rotation),
                ..default()
            },
            Name::new("V3 placed map entity fallback"),
        ));
    }
    for anchor in &snapshot.mode_anchors {
        let crate::map::ModeAnchorShape::Area { position, shape } = anchor.shape else {
            continue;
        };
        match shape {
            crate::map::MapShape::Circle { radius } => {
                let rotation = Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2);
                let mut translation = ground_position(position);
                translation.y = GROUND_OFFSET;
                let fill_mesh = meshes.add(Circle::new(radius));
                commands.spawn((
                    marker,
                    V3ZoneFill,
                    Mesh3d(fill_mesh.clone()),
                    GeneratedMapMesh(fill_mesh),
                    MeshMaterial3d(materials.zone_fill.clone()),
                    NotShadowCaster,
                    Transform {
                        translation,
                        rotation,
                        ..default()
                    },
                ));
                let boundary_mesh = meshes.add(Annulus::new(
                    (radius - ZONE_RING_WIDTH * 0.5).max(0.0),
                    radius + ZONE_RING_WIDTH * 0.5,
                ));
                commands.spawn((
                    marker,
                    V3ZoneBoundary,
                    Mesh3d(boundary_mesh.clone()),
                    GeneratedMapMesh(boundary_mesh),
                    MeshMaterial3d(materials.zone_boundary.clone()),
                    NotShadowCaster,
                    Transform {
                        translation: translation + Vec3::Y * 0.2,
                        rotation,
                        ..default()
                    },
                ));
            }
            crate::map::MapShape::Rectangle { half_extents } => {
                let size = half_extents * 2.0;
                let mut translation = ground_position(position);
                translation.y = GROUND_OFFSET;
                let mesh = meshes.add(Plane3d::default().mesh().size(size.x, size.y));
                commands.spawn((
                    marker,
                    V3ZoneFill,
                    Mesh3d(mesh.clone()),
                    GeneratedMapMesh(mesh),
                    MeshMaterial3d(materials.zone_fill.clone()),
                    NotShadowCaster,
                    Transform::from_translation(translation),
                ));
            }
        }
    }
    commands.insert_resource(Presented3dMap(snapshot.identity.instance_id));
}

fn team_material(
    team: crate::combat::TeamId,
    materials: &Material3dAssets,
) -> Handle<StandardMaterial> {
    if team.0 == 1 {
        materials.team_red.clone()
    } else {
        materials.team_blue.clone()
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "Bevy system parameters own asynchronous asset readiness for independent families"
)]
fn prepare_imported_assets(
    mut commands: Commands,
    handles: Option<Res<ClientAssetHandles>>,
    asset_server: Res<AssetServer>,
    gltfs: Res<Assets<Gltf>>,
    mut graphs: ResMut<Assets<AnimationGraph>>,
    prepared: Option<Res<Imported3dAssets>>,
    prepared_arena: Option<Res<ImportedArenaAssets>>,
    fallback_policy: Res<ImportedWorldFallbackPolicy>,
    mut last_load_report: Local<String>,
) {
    if *fallback_policy == ImportedWorldFallbackPolicy::ForcePrimitive {
        return;
    }
    let Some(handles) = handles else {
        return;
    };
    let load_report = format!(
        "character={:?}/{:?} blaster={:?}/{:?} wall={:?}/{:?}",
        asset_server.load_state(&handles.character),
        asset_server.recursive_dependency_load_state(&handles.character),
        asset_server.load_state(&handles.blaster),
        asset_server.recursive_dependency_load_state(&handles.blaster),
        asset_server.load_state(&handles.arena_block),
        asset_server.recursive_dependency_load_state(&handles.arena_block),
    );
    if *last_load_report != load_report {
        info!(state = %load_report, "V3 imported asset load state changed");
        *last_load_report = load_report;
    }
    if prepared_arena.is_none()
        && asset_server.is_loaded_with_dependencies(&handles.arena_block)
        && let Some(block) = gltfs.get(&handles.arena_block)
        && let Some(block_scene) = block.default_scene.clone()
    {
        commands.insert_resource(ImportedArenaAssets { block_scene });
        commands.remove_resource::<Presented3dMap>();
        info!("curated Mini Arena block and colormap are ready");
    }
    if prepared.is_some()
        || !asset_server.is_loaded_with_dependencies(&handles.character)
        || !asset_server.is_loaded_with_dependencies(&handles.blaster)
    {
        return;
    }
    let (Some(character), Some(blaster)) =
        (gltfs.get(&handles.character), gltfs.get(&handles.blaster))
    else {
        return;
    };
    let required = [
        "idle",
        "walk",
        "holding-right",
        "holding-right-shoot",
        "die",
    ];
    let Some(clips) = required
        .iter()
        .map(|name| character.named_animations.get(*name).cloned())
        .collect::<Option<Vec<_>>>()
    else {
        warn!("Mini Characters GLB is missing the M01 named animation contract; using fallback");
        return;
    };
    let (graph, nodes) = AnimationGraph::from_clips(clips);
    let [idle, walk_node, holding, shoot, defeated] = nodes.as_slice() else {
        return;
    };
    let (Some(character_scene), Some(blaster_scene)) = (
        character.default_scene.clone(),
        blaster.default_scene.clone(),
    ) else {
        warn!("selected Kenney GLB lacks a default scene; using primitive fallbacks");
        return;
    };
    commands.insert_resource(Imported3dAssets {
        character_scene,
        blaster_scene,
        animation_graph: graphs.add(graph),
        idle: *idle,
        walk: *walk_node,
        holding: *holding,
        shoot: *shoot,
        defeated: *defeated,
    });
    commands.remove_resource::<Presented3dMap>();
    info!("curated V3 GLB dependencies and named clips are ready");
}

#[allow(
    clippy::type_complexity,
    reason = "the query declares imported-scene promotion eligibility on fighter visual roots"
)]
fn upgrade_fighters_to_imported_models(
    mut commands: Commands,
    imported: Option<Res<Imported3dAssets>>,
    fighters: Query<
        (Entity, &CombatVisualOwner),
        (With<V3FighterVisual>, Without<V3CharacterScene>),
    >,
) {
    let Some(imported) = imported else {
        return;
    };
    for (visual_root, owner) in &fighters {
        commands.entity(visual_root).with_children(|parent| {
            parent.spawn((
                V3CharacterScene {
                    owner: owner.0,
                    visual_root,
                },
                WorldAssetRoot(imported.character_scene.clone()),
                Transform {
                    rotation: Quat::from_rotation_y(KENNEY_CHARACTER_FORWARD_CORRECTION),
                    scale: Vec3::splat(64.0),
                    ..default()
                },
                Name::new("V3 imported fighter"),
            ));
        });
        commands.entity(visual_root).insert(V3CharacterScene {
            owner: owner.0,
            visual_root,
        });
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "the scene-ready observer validates and installs one bounded imported hierarchy"
)]
fn setup_imported_character(
    ready: On<WorldInstanceReady>,
    mut commands: Commands,
    imported: Res<Imported3dAssets>,
    scene_roots: Query<&V3CharacterScene>,
    children: Query<&Children>,
    names: Query<&Name>,
    mut players: Query<&mut AnimationPlayer>,
    mut fallbacks: Query<(&V3FallbackVisual, &mut Visibility)>,
) {
    let Ok(scene) = scene_roots.get(ready.entity) else {
        return;
    };
    let descendants = children.iter_descendants(ready.entity).collect::<Vec<_>>();
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
        warn!(owner = ?scene.owner, "imported fighter hierarchy misses arm-right or AnimationPlayer; retaining fallback");
        return;
    };
    commands.entity(arm).with_children(|parent| {
        parent.spawn((
            WorldAssetRoot(imported.blaster_scene.clone()),
            Transform {
                translation: Vec3::new(0.08, 0.0, 0.0),
                rotation: Quat::from_rotation_y(KENNEY_BLASTER_GRIP_ROTATION),
                ..default()
            },
            Name::new("V3 attached blaster-a"),
        ));
    });
    if let Ok(mut animation_player) = players.get_mut(player) {
        let mut transitions = AnimationTransitions::new();
        transitions
            .play(&mut animation_player, imported.idle, Duration::ZERO)
            .repeat();
        commands
            .entity(player)
            .insert(AnimationGraphHandle(imported.animation_graph.clone()))
            .insert(transitions);
    }
    commands.entity(ready.entity).insert(V3CharacterRuntime {
        owner: scene.owner,
        visual_root: scene.visual_root,
        player,
        current: CharacterMotion::Idle,
    });
    for (fallback, mut visibility) in &mut fallbacks {
        if fallback.owner == scene.owner {
            *visibility = Visibility::Hidden;
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters own render-time animation state"
)]
fn update_character_animation(
    imported: Option<Res<Imported3dAssets>>,
    time: Res<Time>,
    mut runtimes: Query<&mut V3CharacterRuntime>,
    owners: Query<Option<&crate::combat::Defeated>, With<Fighter>>,
    mut visuals: Query<&mut V3FighterVisual>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
) {
    let Some(imported) = imported else {
        return;
    };
    for mut runtime in &mut runtimes {
        let Ok(defeated) = owners.get(runtime.owner) else {
            continue;
        };
        let Ok(mut visual) = visuals.get_mut(runtime.visual_root) else {
            continue;
        };
        visual.shoot_seconds = (visual.shoot_seconds - time.delta_secs()).max(0.0);
        let next = if defeated.is_some() {
            CharacterMotion::Defeated
        } else if visual.shoot_seconds > 0.0 {
            CharacterMotion::Shoot
        } else if visual.moving {
            CharacterMotion::Walk
        } else {
            CharacterMotion::Holding
        };
        if next == runtime.current {
            continue;
        }
        let Ok((mut player, mut transitions)) = players.get_mut(runtime.player) else {
            continue;
        };
        let node = match next {
            CharacterMotion::Idle => imported.idle,
            CharacterMotion::Holding => imported.holding,
            CharacterMotion::Walk => imported.walk,
            CharacterMotion::Shoot => imported.shoot,
            CharacterMotion::Defeated => imported.defeated,
        };
        play_character_motion(&mut player, &mut transitions, node, runtime.current, next);
        runtime.current = next;
    }
}

fn play_character_motion(
    player: &mut AnimationPlayer,
    transitions: &mut AnimationTransitions,
    node: AnimationNodeIndex,
    previous: CharacterMotion,
    next: CharacterMotion,
) {
    let recovered_from_defeat =
        previous == CharacterMotion::Defeated && next != CharacterMotion::Defeated;
    if recovered_from_defeat {
        player.stop_all();
        *transitions = AnimationTransitions::new();
    }
    let duration = if recovered_from_defeat {
        Duration::ZERO
    } else {
        Duration::from_millis(100)
    };
    let animation = transitions.play(player, node, duration);
    if !matches!(next, CharacterMotion::Shoot | CharacterMotion::Defeated) {
        animation.repeat();
    }
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_arguments,
    reason = "validated positive map dimensions cap module counts far below u16"
)]
fn spawn_imported_wall_modules(
    commands: &mut Commands,
    imported: &ImportedArenaAssets,
    center: Vec2,
    rotation: f32,
    size: Vec2,
    instance_id: crate::map::MapInstanceId,
) -> bool {
    const MODULE: f32 = 64.0;
    let counts = (size / MODULE).round();
    if (counts * MODULE).distance(size) > 0.01 || counts.x < 1.0 || counts.y < 1.0 {
        return false;
    }
    let basis_x = Vec2::from_angle(rotation);
    let basis_y = Vec2::new(-basis_x.y, basis_x.x);
    for y in 0..counts.y as u16 {
        for x in 0..counts.x as u16 {
            let local = Vec2::new(
                (f32::from(x) + 0.5) * MODULE - size.x * 0.5,
                (f32::from(y) + 0.5) * MODULE - size.y * 0.5,
            );
            let position = center + basis_x * local.x + basis_y * local.y;
            commands.spawn((
                crate::map::MapPresentationMember { instance_id },
                WorldAssetRoot(imported.block_scene.clone()),
                Transform {
                    translation: ground_position(position),
                    rotation: Quat::from_rotation_y(rotation),
                    scale: Vec3::splat(MODULE),
                },
                Name::new("V3 imported Mini Arena block module"),
            ));
        }
    }
    true
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_sign_loss,
    clippy::too_many_arguments,
    reason = "validated positive map dimensions cap module counts far below u16"
)]
fn spawn_fallback_wall_modules(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    center: Vec2,
    rotation: f32,
    size: Vec2,
    instance_id: crate::map::MapInstanceId,
) {
    const MODULE: f32 = 64.0;
    let counts = (size / MODULE).round();
    if (counts * MODULE).distance(size) > 0.01 || counts.x < 1.0 || counts.y < 1.0 {
        let extents = ground_extents(size);
        let mesh = meshes.add(Cuboid::new(extents.x, WALL_HEIGHT, extents.z));
        commands.spawn((
            crate::map::MapPresentationMember { instance_id },
            Mesh3d(mesh.clone()),
            GeneratedMapMesh(mesh),
            MeshMaterial3d(materials.wall.clone()),
            Transform {
                translation: ground_position(center) + Vec3::Y * (WALL_HEIGHT * 0.5),
                rotation: Quat::from_rotation_y(rotation),
                ..default()
            },
            Name::new("V3 exact rectangular cover fallback"),
        ));
        return;
    }
    let basis_x = Vec2::from_angle(rotation);
    let basis_y = Vec2::new(-basis_x.y, basis_x.x);
    for y in 0..counts.y as u16 {
        for x in 0..counts.x as u16 {
            let local = Vec2::new(
                (f32::from(x) + 0.5) * MODULE - size.x * 0.5,
                (f32::from(y) + 0.5) * MODULE - size.y * 0.5,
            );
            let position = center + basis_x * local.x + basis_y * local.y;
            commands.spawn((
                crate::map::MapPresentationMember { instance_id },
                Mesh3d(primitives.cover_block.clone()),
                MeshMaterial3d(materials.wall.clone()),
                Transform {
                    translation: ground_position(position) + Vec3::Y * 16.0,
                    rotation: Quat::from_rotation_y(rotation),
                    ..default()
                },
                Name::new("V3 exact cover block fallback"),
            ));
        }
    }
}

fn catch_up_projectile_position(
    presented: Vec2,
    authoritative: Vec2,
    projectile_speed: f32,
    delta_seconds: f32,
) -> Vec2 {
    let delta = authoritative - presented;
    let maximum_step = projectile_speed * STRAIGHT_PROJECTILE_CATCH_UP_MULTIPLIER * delta_seconds;
    if !maximum_step.is_finite() || maximum_step <= 0.0 || delta.length_squared() == 0.0 {
        return presented;
    }
    if delta.length_squared() <= maximum_step * maximum_step {
        authoritative
    } else {
        presented + delta.normalize() * maximum_step
    }
}

fn tint_3d_zone(
    roots: Query<(&MatchState, &crate::matchplay::HotZoneState), With<MatchRoot>>,
    fills: Query<&MeshMaterial3d<StandardMaterial>, With<V3ZoneFill>>,
    boundaries: Query<&MeshMaterial3d<StandardMaterial>, With<V3ZoneBoundary>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let Ok((state, hot_zone)) = roots.single() else {
        return;
    };
    if state.match_id != hot_zone.match_id {
        return;
    }
    let (fill, boundary) = match hot_zone.status {
        crate::matchplay::HotZoneStatus::Empty => (
            Color::srgba(0.2, 0.5, 0.95, 0.30),
            Color::srgb(1.0, 0.82, 0.2),
        ),
        crate::matchplay::HotZoneStatus::Contested => (
            Color::srgba(0.95, 0.2, 0.45, 0.32),
            Color::srgb(1.0, 0.35, 0.6),
        ),
        crate::matchplay::HotZoneStatus::Controlled { team } => {
            let color = if team.0 == 1 {
                Color::srgb(1.0, 0.42, 0.12)
            } else {
                Color::srgb(0.12, 0.72, 0.96)
            };
            (color.with_alpha(0.32), color)
        }
    };
    for handle in &fills {
        if let Some(mut material) = materials.get_mut(handle.id()) {
            material.base_color = fill;
        }
    }
    for handle in &boundaries {
        if let Some(mut material) = materials.get_mut(handle.id()) {
            material.base_color = boundary;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn imported_character_front_aligns_with_fighter_root_facing() {
        let corrected_front = Quat::from_rotation_y(KENNEY_CHARACTER_FORWARD_CORRECTION) * Vec3::Z;

        assert!(corrected_front.abs_diff_eq(Vec3::X, 1e-5));
    }

    #[test]
    fn attached_blaster_remains_aligned_with_fighter_root_facing() {
        let corrected_barrel = Quat::from_rotation_y(KENNEY_CHARACTER_FORWARD_CORRECTION)
            * Quat::from_rotation_y(KENNEY_BLASTER_GRIP_ROTATION)
            * Vec3::Z;

        assert!(corrected_barrel.abs_diff_eq(Vec3::X, 1e-5));
    }

    #[test]
    fn respawn_stops_defeated_pose_before_starting_live_loop() {
        let defeated = AnimationNodeIndex::new(0);
        let holding = AnimationNodeIndex::new(1);
        let mut player = AnimationPlayer::default();
        let mut transitions = AnimationTransitions::new();
        transitions.play(&mut player, defeated, Duration::ZERO);

        play_character_motion(
            &mut player,
            &mut transitions,
            holding,
            CharacterMotion::Defeated,
            CharacterMotion::Holding,
        );

        assert!(!player.is_playing_animation(defeated));
        assert!(player.is_playing_animation(holding));
        assert_eq!(transitions.get_main_animation(), Some(holding));
        assert!(!player.animation(holding).unwrap().is_finished());
    }

    fn map_app(snapshot: crate::map::ResolvedMapSnapshot) -> App {
        let mut app = App::new();
        app.add_plugins((
            MinimalPlugins,
            crate::map::MapContentPlugin,
            crate::map::MapPresentationPlugin,
        ))
        .init_resource::<Assets<Mesh>>()
        .init_resource::<Assets<StandardMaterial>>()
        .add_systems(
            Update,
            reconcile_3d_map.in_set(crate::map::MapPresentationSet::Materialize3d),
        );
        let (floor_tile, cover_block) = {
            let mut meshes = app.world_mut().resource_mut::<Assets<Mesh>>();
            (
                meshes.add(Cuboid::new(64.0, 1.0, 64.0)),
                meshes.add(Cuboid::new(64.0, 32.0, 64.0)),
            )
        };
        let material = app
            .world_mut()
            .resource_mut::<Assets<StandardMaterial>>()
            .add(StandardMaterial::default());
        app.insert_resource(Primitive3dAssets {
            floor_tile,
            cover_block,
            map_entity: Handle::default(),
            debris: Handle::default(),
            fighter: Handle::default(),
            direction: Handle::default(),
            projectile: Handle::default(),
            lobbed_projectile: Handle::default(),
            unit_cuboid: Handle::default(),
            sentry_base: Handle::default(),
            sentry_body: Handle::default(),
            ground_ring: Handle::default(),
            effect_sphere: Handle::default(),
        })
        .insert_resource(Material3dAssets {
            floor: material.clone(),
            wall: material.clone(),
            perimeter: material.clone(),
            team_blue: material.clone(),
            team_red: material.clone(),
            marker_local: material.clone(),
            marker_ally: material.clone(),
            marker_enemy: material.clone(),
            neutral: material.clone(),
            zone_fill: material.clone(),
            zone_boundary: material.clone(),
            terrain: material.clone(),
            health_back: material.clone(),
            health_fill: material.clone(),
            preview: material.clone(),
            preview_blocked: material.clone(),
            status_slow: material.clone(),
            status_knockback: material.clone(),
            effect_muzzle: material.clone(),
            effect_impact: material.clone(),
            effect_damage: material.clone(),
            dash: material,
        });
        app.world_mut().spawn((
            crate::map::MapRoot,
            snapshot.identity.instance_id,
            snapshot.identity,
            snapshot,
        ));
        app
    }

    #[test]
    fn camera_clamp_centers_small_maps_and_clamps_large_maps() {
        let small = crate::map::AxisAlignedMapRect {
            min: Vec2::splat(-100.0),
            max: Vec2::splat(100.0),
        };
        assert_eq!(
            clamp_3d_camera_center(Vec2::new(90.0, 90.0), small, Vec2::new(1600.0, 900.0)),
            Vec2::ZERO
        );
        let large = crate::map::AxisAlignedMapRect {
            min: Vec2::splat(-2000.0),
            max: Vec2::splat(2000.0),
        };
        let clamped = clamp_3d_camera_center(Vec2::splat(2000.0), large, Vec2::new(1600.0, 900.0));
        assert!(clamped.x < 2000.0 && clamped.y < 2000.0);
    }

    #[test]
    fn camera_clamp_is_finite_for_supported_aspects_and_missing_viewport() {
        let bounds = crate::map::AxisAlignedMapRect {
            min: Vec2::splat(-3_000.0),
            max: Vec2::splat(3_000.0),
        };
        for viewport in [
            Vec2::new(1_280.0, 720.0),
            Vec2::new(1_024.0, 768.0),
            Vec2::new(1_680.0, 720.0),
            Vec2::ZERO,
            Vec2::new(f32::NAN, 720.0),
        ] {
            let center = clamp_3d_camera_center(Vec2::splat(3_000.0), bounds, viewport);
            assert!(
                center.is_finite(),
                "viewport {viewport:?} produced {center:?}"
            );
            assert!(bounds.contains(center));
        }
    }

    #[test]
    fn straight_projectile_visual_starts_at_muzzle_and_catches_up_without_overshoot() {
        let origin = Vec2::ZERO;
        let authoritative = Vec2::new(90.0, 0.0);
        let first = catch_up_projectile_position(origin, authoritative, 900.0, 1.0 / 60.0);
        assert!(first.abs_diff_eq(Vec2::new(45.0, 0.0), 0.0001));
        assert_eq!(
            catch_up_projectile_position(first, authoritative, 900.0, 1.0 / 60.0),
            authoritative
        );
        assert_eq!(
            catch_up_projectile_position(authoritative, authoritative, 900.0, 1.0 / 60.0),
            authoritative
        );
    }

    #[test]
    fn imported_world_fallback_policy_has_an_explicit_verification_override() {
        assert_eq!(
            ImportedWorldFallbackPolicy::from_value(Some("1")),
            ImportedWorldFallbackPolicy::ForcePrimitive
        );
        assert_eq!(
            ImportedWorldFallbackPolicy::from_value(Some("true")),
            ImportedWorldFallbackPolicy::ForcePrimitive
        );
        assert_eq!(
            ImportedWorldFallbackPolicy::from_value(None),
            ImportedWorldFallbackPolicy::Auto
        );
    }

    #[test]
    fn built_in_wipeout_materializes_exact_floor_cover_and_perimeter_counts() {
        let snapshot = crate::map::MapContentCatalog::embedded()
            .unwrap()
            .resolve_preset(
                crate::map::MapPresetId(1),
                crate::map::MapInstanceId(1),
                &crate::map::MapLayoutRequirements::wipeout(),
            )
            .unwrap()
            .snapshot;
        let mut app = map_app(snapshot);
        app.update();
        app.update();
        let world = app.world_mut();
        let mut names = world.query::<(&crate::map::MapPresentationMember, &Name)>();
        let presented: Vec<_> = names.iter(world).map(|(_, name)| name.as_str()).collect();
        assert_eq!(
            presented
                .iter()
                .filter(|name| **name == "V3 resolved floor tile")
                .count(),
            504
        );
        assert_eq!(
            presented
                .iter()
                .filter(|name| **name == "V3 exact cover block fallback")
                .count(),
            24
        );
        assert_eq!(
            presented
                .iter()
                .filter(|name| **name == "V3 arena perimeter")
                .count(),
            4
        );
    }

    #[test]
    fn hot_zone_is_generation_owned_and_invalid_snapshots_materialize_nothing() {
        let snapshot = crate::map::MapContentCatalog::embedded()
            .unwrap()
            .resolve_preset(
                crate::map::MapPresetId(2),
                crate::map::MapInstanceId(7),
                &crate::map::MapLayoutRequirements::hot_zone(),
            )
            .unwrap()
            .snapshot;
        let mut app = map_app(snapshot);
        app.update();
        app.update();
        let world = app.world_mut();
        assert_eq!(world.query::<&V3ZoneFill>().iter(world).count(), 1);
        assert_eq!(world.query::<&V3ZoneBoundary>().iter(world).count(), 1);
        assert!(
            world
                .query::<&crate::map::MapPresentationMember>()
                .iter(world)
                .all(|member| member.instance_id == crate::map::MapInstanceId(7))
        );

        let mut invalid = crate::map::MapContentCatalog::embedded()
            .unwrap()
            .resolve_preset(
                crate::map::MapPresetId(1),
                crate::map::MapInstanceId(9),
                &crate::map::MapLayoutRequirements::wipeout(),
            )
            .unwrap()
            .snapshot;
        invalid.visual_instances[0].presentation_profile_id =
            crate::map::MapPresentationProfileId(999);
        let mut invalid_app = map_app(invalid);
        invalid_app.update();
        invalid_app.update();
        assert_eq!(
            invalid_app
                .world_mut()
                .query::<&crate::map::MapPresentationMember>()
                .iter(invalid_app.world())
                .count(),
            0
        );
    }

    #[test]
    fn repeated_map_replacement_keeps_generated_meshes_and_entities_bounded() {
        let catalog = crate::map::MapContentCatalog::embedded().unwrap();
        let first = catalog
            .resolve_preset(
                crate::map::MapPresetId(1),
                crate::map::MapInstanceId(1),
                &crate::map::MapLayoutRequirements::wipeout(),
            )
            .unwrap()
            .snapshot;
        let mut app = map_app(first);
        app.update();
        app.update();
        let baseline_meshes = app.world().resource::<Assets<Mesh>>().len();
        let baseline_entities = app
            .world_mut()
            .query::<&crate::map::MapPresentationMember>()
            .iter(app.world())
            .count();

        for generation in 2..=40 {
            let hot_zone = generation % 2 == 0;
            let requirements = if hot_zone {
                crate::map::MapLayoutRequirements::hot_zone()
            } else {
                crate::map::MapLayoutRequirements::wipeout()
            };
            let snapshot = catalog
                .resolve_preset(
                    crate::map::MapPresetId(if hot_zone { 2 } else { 1 }),
                    crate::map::MapInstanceId(generation),
                    &requirements,
                )
                .unwrap()
                .snapshot;
            app.world_mut().spawn((
                crate::map::MapRoot,
                snapshot.identity.instance_id,
                snapshot.identity,
                snapshot,
            ));
            app.update();
            app.update();
            assert!(
                app.world().resource::<Assets<Mesh>>().len() <= baseline_meshes + 2,
                "generation {generation} leaked generated meshes"
            );
            assert!(
                app.world_mut()
                    .query::<&crate::map::MapPresentationMember>()
                    .iter(app.world())
                    .count()
                    <= baseline_entities + 2,
                "generation {generation} leaked presentation entities"
            );
        }
    }
}
