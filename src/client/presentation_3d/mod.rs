//! Default V3 world renderer over the unchanged planar simulation.
#![allow(clippy::wildcard_imports)]

use super::presentation::{spawn_client_hud, spawn_pause_overlay};
use super::*;
use crate::combat::client::CombatClientSet;
use bevy::{
    asset::RenderAssetUsages,
    camera::{CameraUpdateSystems, visibility::RenderLayers},
    core_pipeline::tonemapping::Tonemapping,
    gltf::Gltf,
    light::{GlobalAmbientLight, NotShadowCaster, NotShadowReceiver},
    math::primitives::{Annulus, Circle},
    mesh::Indices,
    render::render_resource::PrimitiveTopology,
    ui::UiSystems,
    world_serialization::{WorldAsset, WorldAssetRoot, WorldInstanceReady},
};
use core::time::Duration;

mod border;
mod camera;
mod combat;
pub(crate) mod coordinates;
mod diagnostics;
pub(crate) mod environment_assets;
mod map;

pub(super) use camera::cursor_ground_point;
use camera::{
    CAMERA_DISTANCE, CAMERA_ELEVATION_RADIANS, CAMERA_VERTICAL_FOV_RADIANS, follow_3d_camera,
};
use coordinates::{ground_point, ground_position, ground_rotation};
use map::{GeneratedMapMesh, Presented3dMap};

const WALL_HEIGHT: f32 = 72.0;
// Kenney's Mini Characters face local +Z, while Brawler fighter roots face local +X.
pub(crate) const KENNEY_CHARACTER_FORWARD_CORRECTION: f32 = core::f32::consts::FRAC_PI_2;
// Blaster Kit barrels also point local +Z, so the corrected character hierarchy needs no extra yaw.
pub(crate) const KENNEY_BLASTER_GRIP_ROTATION: f32 = 0.0;
// Keep a straight shot nearly on its authoritative plane. A larger lift produces a strong
// screen-space parallax offset under the tilted orthographic camera and makes it miss the muzzle.
const STRAIGHT_PROJECTILE_HEIGHT: f32 = 4.0;
const LOBBED_PROJECTILE_LAUNCH_HEIGHT: f32 = 20.0;
const STRAIGHT_PROJECTILE_CATCH_UP_MULTIPLIER: f32 = 3.0;
const FIGHTER_RING_INNER_RADIUS: f32 = 18.0;
const FIGHTER_RING_OUTER_RADIUS: f32 = 22.0;
const HOT_ZONE_RING_WIDTH: f32 = 10.0;
const GROUND_AREA_HEIGHT: f32 = 1.0;
const FIGHTER_FACING_TIP_RADIUS: f32 = 28.0;
const FIGHTER_FACING_HALF_ANGLE: f32 = 0.22;
const FIGHTER_FACING_ARC_SEGMENTS: u16 = 4;

#[derive(Resource)]
pub(crate) struct Primitive3dAssets {
    pub(crate) cover_block: Handle<Mesh>,
    pub(crate) map_entity: Handle<Mesh>,
    pub(crate) fighter: Handle<Mesh>,
    pub(crate) sentry_direction: Handle<Mesh>,
    pub(crate) fighter_facing: Handle<Mesh>,
    pub(crate) projectile: Handle<Mesh>,
    pub(crate) lobbed_projectile: Handle<Mesh>,
    pub(crate) unit_cuboid: Handle<Mesh>,
    pub(crate) sentry_base: Handle<Mesh>,
    pub(crate) sentry_body: Handle<Mesh>,
    pub(crate) ground_ring: Handle<Mesh>,
    pub(crate) area_ring: Handle<Mesh>,
    pub(crate) effect_sphere: Handle<Mesh>,
}

#[derive(Resource)]
pub(crate) struct Material3dAssets {
    pub(crate) team_blue: Handle<StandardMaterial>,
    pub(crate) team_red: Handle<StandardMaterial>,
    pub(crate) marker_local: Handle<StandardMaterial>,
    pub(crate) marker_ally: Handle<StandardMaterial>,
    pub(crate) marker_enemy: Handle<StandardMaterial>,
    pub(crate) neutral: Handle<StandardMaterial>,
    pub(crate) zone_fill: Handle<StandardMaterial>,
    pub(crate) zone_boundary: Handle<StandardMaterial>,
    pub(crate) preview: Handle<StandardMaterial>,
    pub(crate) preview_blocked: Handle<StandardMaterial>,
    pub(crate) status_slow: Handle<StandardMaterial>,
    pub(crate) status_knockback: Handle<StandardMaterial>,
    pub(crate) status_reveal: Handle<StandardMaterial>,
    pub(crate) scan_area: Handle<StandardMaterial>,
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
    bind_pose: Vec<(Entity, Transform)>,
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
pub(crate) struct Imported3dAssets {
    pub(crate) character_scene: Handle<WorldAsset>,
    pub(crate) blaster_scene: Handle<WorldAsset>,
    pub(crate) animation_graph: Handle<AnimationGraph>,
    pub(crate) idle: AnimationNodeIndex,
    walk: AnimationNodeIndex,
    pub(crate) holding: AnimationNodeIndex,
    shoot: AnimationNodeIndex,
    defeated: AnimationNodeIndex,
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
            .init_resource::<combat::ConcealedMaterialVariants>()
            .insert_resource(GlobalAmbientLight {
                color: Color::srgb(0.72, 0.78, 0.9),
                brightness: 350.0,
                ..default()
            })
            .add_systems(Startup, setup_3d_foundation)
            .add_systems(Startup, environment_assets::load_environment_assets)
            .add_systems(
                Update,
                (
                    prepare_imported_assets,
                    environment_assets::prepare_environment_scenes,
                    reconcile_3d_map.in_set(crate::map::MapPresentationSet::Materialize3d),
                    reconcile_dynamic_map_visuals,
                    combat::reconcile_combat_visuals,
                    upgrade_fighters_to_imported_models,
                    combat::update_fighter_concealment_visuals,
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
            .add_systems(
                PostUpdate,
                combat::project_fighter_overhead_ui
                    .after(TransformSystems::Propagate)
                    .after(CameraUpdateSystems)
                    .before(UiSystems::Prepare),
            )
            .add_observer(setup_imported_character)
            .add_observer(environment_assets::tint_environment_instance);
    }
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
struct DynamicMapVisual {
    placement_id: crate::map::MapPlacementId,
    generation: u64,
    asset_id: crate::map::MapAssetId,
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "parameters are Bevy system parameters owned by the presentation schedule"
)]
fn reconcile_dynamic_map_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    theme_materials: Res<environment_assets::EnvironmentThemeMaterialCatalog>,
    catalog: Res<crate::map::MapCatalogResource>,
    accepted: Option<Res<crate::map::PresentedMap>>,
    grids: Query<
        (
            &crate::map::ResolvedMapSnapshot,
            &crate::map::MapDynamicState,
        ),
        With<crate::map::MapRoot>,
    >,
    existing: Query<(
        Entity,
        &crate::map::MapPresentationMember,
        &DynamicMapVisual,
    )>,
) {
    let Some(accepted) = accepted else {
        return;
    };
    let Ok((snapshot, state)) = grids.get(accepted.source_root) else {
        return;
    };
    let terminal: std::collections::BTreeMap<_, _> = state
        .terminal_states
        .iter()
        .map(|transition| (transition.placement_id, transition.outcome))
        .collect();
    let desired_asset =
        |placement: &crate::map::MapAssetPlacement| match terminal.get(&placement.placement_id) {
            Some(crate::map::MapPlacementOutcome::Removed) => None,
            Some(crate::map::MapPlacementOutcome::ReplacedWith(asset_id)) => Some(*asset_id),
            None => Some(placement.asset_id),
        };
    let mut present = std::collections::BTreeSet::new();
    for (entity, member, visual) in &existing {
        let desired = snapshot
            .placements
            .iter()
            .find(|placement| placement.placement_id == visual.placement_id)
            .and_then(desired_asset);
        if member.instance_id != snapshot.identity.instance_id
            || visual.generation != state.generation
            || desired != Some(visual.asset_id)
        {
            commands.entity(entity).try_despawn();
        } else {
            present.insert(visual.placement_id);
        }
    }
    let Some(materials) = theme_materials.get(snapshot.presentation_theme_id) else {
        return;
    };
    let marker = crate::map::MapPresentationMember {
        instance_id: snapshot.identity.instance_id,
    };
    for placement in &snapshot.placements {
        let Some(source_asset) = catalog.0.asset(placement.asset_id) else {
            continue;
        };
        let dynamic = catalog
            .0
            .profile(source_asset.gameplay_profile_id)
            .is_some_and(|profile| {
                profile.destruction != crate::map::MapDestructionBehavior::Indestructible
            });
        if !dynamic || present.contains(&placement.placement_id) {
            continue;
        }
        let Some(asset_id) = desired_asset(placement) else {
            continue;
        };
        let Some(asset) = catalog.0.asset(asset_id) else {
            continue;
        };
        let footprint = asset.footprint_cells.rotated(placement.quarter_turns);
        let center = crate::map::placement_world_center(snapshot.dimensions, asset, placement);
        let material = if asset_id == crate::map::RUBBLE_ASSET {
            materials.rubble.clone()
        } else {
            materials.destructible_cover.clone()
        };
        commands.spawn((
            marker,
            DynamicMapVisual {
                placement_id: placement.placement_id,
                generation: state.generation,
                asset_id,
            },
            Mesh3d(primitives.cover_block.clone()),
            MeshMaterial3d(material),
            Transform {
                translation: ground_position(center)
                    + Vec3::Y
                        * if asset_id == crate::map::RUBBLE_ASSET {
                            4.0
                        } else {
                            16.0
                        },
                scale: Vec3::new(
                    f32::from(footprint.width) * 0.5,
                    if asset_id == crate::map::RUBBLE_ASSET {
                        0.25
                    } else {
                        1.0
                    },
                    f32::from(footprint.height) * 0.5,
                ),
                ..default()
            },
            Name::new("dynamic map asset"),
        ));
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
        cover_block: meshes.add(Cuboid::new(64.0, 32.0, 64.0)),
        map_entity: meshes.add(Cuboid::new(24.0, 24.0, 24.0)),
        fighter: meshes.add(Sphere::new(24.0)),
        sentry_direction: meshes.add(Cuboid::new(28.0, 7.0, 8.0)),
        fighter_facing: meshes.add(fighter_facing_mesh()),
        projectile: meshes.add(Cylinder::new(4.0, 28.0)),
        lobbed_projectile: meshes.add(Sphere::new(9.0)),
        unit_cuboid: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        sentry_base: meshes.add(Cylinder::new(22.0, 8.0)),
        sentry_body: meshes.add(Cylinder::new(15.0, 24.0)),
        ground_ring: meshes.add(Annulus::new(
            FIGHTER_RING_INNER_RADIUS,
            FIGHTER_RING_OUTER_RADIUS,
        )),
        area_ring: meshes.add(Annulus::new(0.93, 1.0)),
        effect_sphere: meshes.add(Sphere::new(1.0)),
    };
    let matte = |color: Color| StandardMaterial {
        base_color: color,
        perceptual_roughness: 0.82,
        metallic: 0.0,
        ..default()
    };
    let material_assets = Material3dAssets {
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
        status_reveal: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.12, 0.72),
            emissive: LinearRgba::new(2.2, 0.03, 0.8, 1.0),
            unlit: true,
            ..default()
        }),
        scan_area: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.12, 0.72),
            emissive: LinearRgba::new(1.8, 0.04, 0.7, 1.0),
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
        Projection::Perspective(PerspectiveProjection {
            fov: CAMERA_VERTICAL_FOV_RADIANS,
            near: 0.1,
            far: 4_000.0,
            ..default()
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

fn fighter_facing_mesh() -> Mesh {
    let mut positions = vec![[FIGHTER_FACING_TIP_RADIUS, 0.0, 0.0]];
    for step in 0..=FIGHTER_FACING_ARC_SEGMENTS {
        let progress = f32::from(step) / f32::from(FIGHTER_FACING_ARC_SEGMENTS);
        let angle = FIGHTER_FACING_HALF_ANGLE * (1.0 - 2.0 * progress);
        positions.push([
            FIGHTER_RING_OUTER_RADIUS * angle.cos(),
            FIGHTER_RING_OUTER_RADIUS * angle.sin(),
            0.0,
        ]);
    }
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    let uvs = positions
        .iter()
        .map(|position| {
            [
                position[0] / (FIGHTER_FACING_TIP_RADIUS * 2.0) + 0.5,
                position[1] / (FIGHTER_FACING_TIP_RADIUS * 2.0) + 0.5,
            ]
        })
        .collect::<Vec<_>>();
    let mut indices = Vec::with_capacity(FIGHTER_FACING_ARC_SEGMENTS as usize * 3);
    for segment in 0..FIGHTER_FACING_ARC_SEGMENTS {
        indices.extend([0, segment + 1, segment + 2]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U16(indices));
    mesh
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
    presentation_materials: Res<Material3dAssets>,
    theme_materials: Res<environment_assets::EnvironmentThemeMaterialCatalog>,
    grid_catalog: Res<crate::map::MapCatalogResource>,
    map_visuals: Option<Res<environment_assets::MapVisualCatalog>>,
    imported: Option<Res<environment_assets::EnvironmentImportedScenes>>,
    accepted: Option<Res<crate::map::PresentedMap>>,
    current: Option<Res<Presented3dMap>>,
    map_snapshots: Query<&crate::map::ResolvedMapSnapshot, With<crate::map::MapRoot>>,
    members: Query<(
        Entity,
        &crate::map::MapPresentationMember,
        Option<&GeneratedMapMesh>,
    )>,
    mut ambient: ResMut<GlobalAmbientLight>,
    mut directional_lights: Query<&mut DirectionalLight, With<V3WorldMember>>,
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
    let current_grid_snapshot = map_snapshots.get(accepted.source_root).ok().cloned();
    if let Some(grid_snapshot) = current_grid_snapshot.as_ref() {
        let presentation_key = Presented3dMap {
            instance_id: grid_snapshot.identity.instance_id,
            recipe_fingerprint: grid_snapshot.identity.recipe_fingerprint,
            theme_id: grid_snapshot.presentation_theme_id,
        };
        if current.is_some_and(|current| *current == presentation_key) {
            return;
        }
        let Some(environment_materials) = theme_materials.get(grid_snapshot.presentation_theme_id)
        else {
            error!(
                theme = grid_snapshot.presentation_theme_id.0,
                "accepted map has no client theme materials"
            );
            return;
        };
        let Some(grid_theme) = map_visuals
            .as_deref()
            .and_then(|catalog| catalog.theme(grid_snapshot.presentation_theme_id))
        else {
            error!(
                theme = grid_snapshot.presentation_theme_id.0,
                "accepted map has no client theme profile"
            );
            return;
        };
        ambient.color = grid_theme.ambient_color();
        ambient.brightness = grid_theme.ambient_brightness;
        for mut light in &mut directional_lights {
            light.color = grid_theme.directional_color();
            light.illuminance = grid_theme.directional_illuminance;
        }
        for (entity, _, generated) in &members {
            if let Some(generated) = generated {
                meshes.remove(generated.0.id());
            }
            commands.entity(entity).try_despawn();
        }
        let marker = crate::map::MapPresentationMember {
            instance_id: grid_snapshot.identity.instance_id,
        };
        map::spawn_ground_surfaces(
            &mut commands,
            &mut meshes,
            environment_materials,
            marker,
            grid_snapshot.dimensions.bounds(),
        );
        materialize_map_static_visuals(
            &mut commands,
            &mut meshes,
            &primitives,
            &presentation_materials,
            environment_materials,
            marker,
            grid_snapshot,
            &grid_catalog.0,
            map_visuals.as_deref(),
            imported.as_deref(),
        );
        commands.insert_resource(presentation_key);
    }
}

#[allow(clippy::too_many_arguments)]
fn materialize_map_static_visuals(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    primitives: &Primitive3dAssets,
    presentation_materials: &Material3dAssets,
    materials: &environment_assets::EnvironmentThemeMaterials,
    marker: crate::map::MapPresentationMember,
    snapshot: &crate::map::ResolvedMapSnapshot,
    catalog: &crate::map::MapContentCatalog,
    visual_catalog: Option<&environment_assets::MapVisualCatalog>,
    imported: Option<&environment_assets::EnvironmentImportedScenes>,
) {
    for placement in &snapshot.placements {
        let Some(asset) = catalog.asset(placement.asset_id) else {
            continue;
        };
        let dynamic = catalog
            .profile(asset.gameplay_profile_id)
            .is_some_and(|profile| {
                profile.destruction != crate::map::MapDestructionBehavior::Indestructible
            });
        if asset.slot == crate::map::MapAssetSlot::Marker || dynamic {
            continue;
        }
        if asset.slot == crate::map::MapAssetSlot::Surface
            && placement.asset_id != crate::map::WATER_ASSET
        {
            continue;
        }
        let center = crate::map::placement_world_center(snapshot.dimensions, asset, placement);
        let rotation = f32::from(placement.quarter_turns) * core::f32::consts::FRAC_PI_2;
        let profile = visual_catalog.and_then(|catalog| catalog.profile(asset.visual_profile_id));
        let adjacent_cells: std::collections::BTreeSet<_> = snapshot
            .placements
            .iter()
            .filter(|candidate| candidate.asset_id == placement.asset_id)
            .map(|candidate| candidate.cell)
            .collect();
        let adjacency = crate::map::cardinal_adjacency_mask(placement.cell, &adjacent_cells);
        let scene = imported.and_then(|scenes| scenes.scene(asset.visual_profile_id));
        if let (Some(profile), Some(scene)) = (profile, scene)
            && matches!(
                profile.kind,
                environment_assets::MapVisualKind::Imported { .. }
            )
        {
            commands.spawn((
                marker,
                environment_assets::EnvironmentMaterialTint([
                    profile.tint.0,
                    profile.tint.1,
                    profile.tint.2,
                ]),
                WorldAssetRoot(scene.clone()),
                Transform {
                    translation: ground_position(center) + Vec3::Y * profile.vertical_offset,
                    rotation: Quat::from_rotation_y(rotation + profile.yaw_degrees.to_radians()),
                    scale: Vec3::splat(profile.scale),
                },
                Name::new("imported map asset"),
            ));
        } else if placement.asset_id == crate::map::WATER_ASSET {
            spawn_map_water(commands, primitives, materials, marker, center, adjacency);
        } else if placement.asset_id == crate::map::TALL_GRASS_ASSET {
            spawn_map_grass(
                commands, primitives, materials, marker, center, rotation, adjacency,
            );
        } else if asset.slot == crate::map::MapAssetSlot::Feature {
            let footprint = asset.footprint_cells.rotated(placement.quarter_turns);
            commands.spawn((
                marker,
                Mesh3d(primitives.cover_block.clone()),
                MeshMaterial3d(materials.wall.clone()),
                Transform {
                    translation: ground_position(center) + Vec3::Y * (WALL_HEIGHT * 0.5),
                    rotation: Quat::from_rotation_y(rotation),
                    scale: Vec3::new(
                        f32::from(footprint.width) * 0.5,
                        1.0,
                        f32::from(footprint.height) * 0.5,
                    ),
                },
                Name::new(format!("primitive wall adjacency {adjacency:04b}")),
            ));
        } else {
            commands.spawn((
                marker,
                Mesh3d(primitives.map_entity.clone()),
                MeshMaterial3d(materials.wall.clone()),
                Transform {
                    translation: ground_position(center) + Vec3::Y * 12.0,
                    rotation: Quat::from_rotation_y(rotation),
                    scale: Vec3::splat(0.7),
                },
                Name::new("primitive decoration asset"),
            ));
        }
    }
    spawn_map_border(
        commands,
        primitives,
        materials,
        marker,
        snapshot.dimensions.bounds(),
    );
    materialize_hot_zone_objective(commands, meshes, presentation_materials, marker, snapshot);
}

fn hot_zone_visual_geometry(snapshot: &crate::map::ResolvedMapSnapshot) -> Option<(Vec2, f32)> {
    snapshot.mode_anchors.iter().find_map(|anchor| {
        let crate::map::MapModeAnchorKind::HotZoneCircle {
            center_vertex,
            radius_cells,
        } = anchor.kind;
        snapshot
            .dimensions
            .vertex_world(center_vertex)
            .map(|center| {
                (
                    center,
                    f32::from(radius_cells) * crate::map::MAP_CELL_SIZE_WORLD,
                )
            })
    })
}

fn materialize_hot_zone_objective(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &Material3dAssets,
    marker: crate::map::MapPresentationMember,
    snapshot: &crate::map::ResolvedMapSnapshot,
) {
    let Some((center, radius)) = hot_zone_visual_geometry(snapshot) else {
        return;
    };
    let rotation = Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2);
    let fill_mesh = meshes.add(Circle::new(radius));
    commands.spawn((
        marker,
        V3ZoneFill,
        Mesh3d(fill_mesh.clone()),
        GeneratedMapMesh(fill_mesh),
        MeshMaterial3d(materials.zone_fill.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        Transform {
            translation: ground_position(center) + Vec3::Y * GROUND_AREA_HEIGHT,
            rotation,
            ..default()
        },
        Name::new("Hot Zone objective fill"),
    ));
    let boundary_mesh = meshes.add(Annulus::new(
        (radius - HOT_ZONE_RING_WIDTH * 0.5).max(0.0),
        radius + HOT_ZONE_RING_WIDTH * 0.5,
    ));
    commands.spawn((
        marker,
        V3ZoneBoundary,
        Mesh3d(boundary_mesh.clone()),
        GeneratedMapMesh(boundary_mesh),
        MeshMaterial3d(materials.zone_boundary.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        Transform {
            translation: ground_position(center) + Vec3::Y * (GROUND_AREA_HEIGHT + 0.4),
            rotation,
            ..default()
        },
        Name::new("Hot Zone objective boundary"),
    ));
}

fn spawn_map_border(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &environment_assets::EnvironmentThemeMaterials,
    marker: crate::map::MapPresentationMember,
    bounds: crate::map::AxisAlignedMapRect,
) {
    for module in border::border_modules(bounds) {
        commands.spawn((
            marker,
            Mesh3d(primitives.cover_block.clone()),
            MeshMaterial3d(materials.perimeter.clone()),
            Transform {
                translation: ground_position(module.position) + Vec3::Y * 46.0,
                rotation: Quat::from_rotation_y(module.rotation),
                scale: if module.corner {
                    Vec3::splat(0.8)
                } else {
                    Vec3::new(1.0, 0.75, 0.45)
                },
            },
            Name::new("primitive arena edge module"),
        ));
    }
}

fn spawn_map_water(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &environment_assets::EnvironmentThemeMaterials,
    marker: crate::map::MapPresentationMember,
    center: Vec2,
    adjacency: u8,
) {
    commands.spawn((
        marker,
        Mesh3d(primitives.cover_block.clone()),
        MeshMaterial3d(materials.water.clone()),
        Transform {
            translation: ground_position(center) + Vec3::Y,
            scale: Vec3::new(0.5, 0.04, 0.5),
            ..default()
        },
        Name::new(format!("water tile adjacency {adjacency:04b}")),
    ));
    for (bit, offset, scale) in [
        (0, Vec2::new(0.0, 15.0), Vec3::new(0.5, 0.08, 0.04)),
        (1, Vec2::new(15.0, 0.0), Vec3::new(0.04, 0.08, 0.5)),
        (2, Vec2::new(0.0, -15.0), Vec3::new(0.5, 0.08, 0.04)),
        (3, Vec2::new(-15.0, 0.0), Vec3::new(0.04, 0.08, 0.5)),
    ] {
        if adjacency & (1 << bit) == 0 {
            commands.spawn((
                marker,
                Mesh3d(primitives.cover_block.clone()),
                MeshMaterial3d(materials.floor_accent.clone()),
                Transform {
                    translation: ground_position(center + offset) + Vec3::Y * 2.0,
                    scale,
                    ..default()
                },
                Name::new("derived water shore edge"),
            ));
        }
    }
}

fn spawn_map_grass(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &environment_assets::EnvironmentThemeMaterials,
    marker: crate::map::MapPresentationMember,
    center: Vec2,
    rotation: f32,
    adjacency: u8,
) {
    commands.spawn((
        marker,
        Mesh3d(primitives.map_entity.clone()),
        MeshMaterial3d(materials.vegetation.clone()),
        Transform {
            translation: ground_position(center) + Vec3::Y * 9.0,
            rotation: Quat::from_rotation_y(rotation + f32::from(adjacency) * 0.17),
            scale: Vec3::new(0.72, 0.75, 0.72),
        },
        Name::new(format!(
            "non-concealing tall grass adjacency {adjacency:04b}"
        )),
    ));
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
        "character={:?}/{:?} blaster={:?}/{:?}",
        asset_server.load_state(&handles.character),
        asset_server.recursive_dependency_load_state(&handles.character),
        asset_server.load_state(&handles.blaster),
        asset_server.recursive_dependency_load_state(&handles.blaster),
    );
    if *last_load_report != load_report {
        info!(state = %load_report, "V3 imported asset load state changed");
        *last_load_report = load_report;
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
    transforms: Query<&Transform>,
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
    let bind_pose = descendants
        .iter()
        .filter_map(|entity| {
            transforms
                .get(*entity)
                .ok()
                .map(|transform| (*entity, *transform))
        })
        .collect();
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
        bind_pose,
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
    owners: Query<
        (
            &crate::combat::CurrentHealth,
            Option<&crate::combat::Defeated>,
        ),
        With<Fighter>,
    >,
    mut visuals: Query<&mut V3FighterVisual>,
    mut players: Query<(&mut AnimationPlayer, &mut AnimationTransitions)>,
    mut transforms: Query<&mut Transform>,
) {
    let Some(imported) = imported else {
        return;
    };
    for mut runtime in &mut runtimes {
        let Ok((health, defeated)) = owners.get(runtime.owner) else {
            continue;
        };
        let Ok(mut visual) = visuals.get_mut(runtime.visual_root) else {
            continue;
        };
        visual.shoot_seconds = (visual.shoot_seconds - time.delta_secs()).max(0.0);
        let next = if character_is_visually_defeated(health.0, defeated.is_some()) {
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
        if runtime.current == CharacterMotion::Defeated && next != CharacterMotion::Defeated {
            for (entity, bind_transform) in &runtime.bind_pose {
                if let Ok(mut transform) = transforms.get_mut(*entity) {
                    restore_bind_transform(&mut transform, bind_transform);
                }
            }
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

fn restore_bind_transform(transform: &mut Transform, bind_transform: &Transform) {
    transform.clone_from(bind_transform);
}

fn character_is_visually_defeated(current_health: u16, has_defeated_marker: bool) -> bool {
    has_defeated_marker && current_health == 0
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
    fn crossroads_hot_zone_anchor_materializes_at_exact_world_scale() {
        let resolved = crate::map::MapContentCatalog::embedded()
            .unwrap()
            .resolve_preset(crate::map::MapPresetId(2), crate::map::MapInstanceId(1))
            .unwrap();

        assert_eq!(
            hot_zone_visual_geometry(&resolved.snapshot),
            Some((Vec2::ZERO, 5.0 * crate::map::MAP_CELL_SIZE_WORLD))
        );
    }

    #[test]
    fn imported_character_front_aligns_with_fighter_root_facing() {
        let corrected_front = Quat::from_rotation_y(KENNEY_CHARACTER_FORWARD_CORRECTION) * Vec3::Z;

        assert!(corrected_front.abs_diff_eq(Vec3::X, 1e-5));
    }

    #[test]
    fn fighter_facing_indicator_is_a_small_arrow_with_a_ring_matched_back_arc() {
        let mesh = fighter_facing_mesh();
        let bevy::mesh::VertexAttributeValues::Float32x3(positions) =
            mesh.attribute(Mesh::ATTRIBUTE_POSITION).unwrap()
        else {
            panic!("fighter facing positions use three-dimensional coordinates");
        };

        assert!(
            Vec3::from_array(positions[0])
                .abs_diff_eq(Vec3::new(FIGHTER_FACING_TIP_RADIUS, 0.0, 0.0), f32::EPSILON)
        );
        assert_eq!(positions.len(), FIGHTER_FACING_ARC_SEGMENTS as usize + 2);
        assert_eq!(
            mesh.indices().map(Indices::len),
            Some(FIGHTER_FACING_ARC_SEGMENTS as usize * 3)
        );
        for point in &positions[1..] {
            assert!(
                (Vec2::new(point[0], point[1]).length() - FIGHTER_RING_OUTER_RADIUS).abs() < 1e-4
            );
            assert!(point[0] < FIGHTER_FACING_TIP_RADIUS);
        }
        assert!((positions[1][1] + positions.last().unwrap()[1]).abs() < 1e-4);
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

    #[test]
    fn respawn_restores_bind_channels_missing_from_the_live_clip() {
        let bind_transform = Transform::from_translation(Vec3::new(0.0, 1.0, 0.0));
        let mut transform = Transform {
            translation: Vec3::new(0.0, -0.5, 0.4),
            rotation: Quat::from_rotation_x(core::f32::consts::FRAC_PI_2),
            ..default()
        };

        restore_bind_transform(&mut transform, &bind_transform);

        assert_eq!(transform, bind_transform);
    }

    #[test]
    fn restored_health_is_a_recovery_signal_if_marker_removal_arrives_late() {
        assert!(character_is_visually_defeated(0, true));
        assert!(!character_is_visually_defeated(100, true));
        assert!(!character_is_visually_defeated(0, false));
    }
}
