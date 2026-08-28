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

const WALL_HEIGHT: f32 = 32.0;
// Kenney's Mini Characters face local +Z, while Brawler fighter roots face local +X.
pub(crate) const KENNEY_CHARACTER_FORWARD_CORRECTION: f32 = core::f32::consts::FRAC_PI_2;
// Blaster Kit barrels also point local +Z, so the corrected character hierarchy needs no extra yaw.
pub(crate) const KENNEY_BLASTER_GRIP_ROTATION: f32 = 0.0;
// Keep a straight shot nearly on its authoritative plane. A larger lift produces a strong
// screen-space parallax offset under the tilted orthographic camera and makes it miss the muzzle.
const STRAIGHT_PROJECTILE_HEIGHT: f32 = 4.0;
const LOBBED_PROJECTILE_LAUNCH_HEIGHT: f32 = 20.0;
const STRAIGHT_PROJECTILE_CATCH_UP_MULTIPLIER: f32 = 3.0;
const FIGHTER_FALLBACK_RADIUS: f32 = crate::movement::STANDARD_FIGHTER_RADIUS;
const FIGHTER_RING_INNER_RADIUS: f32 = crate::movement::STANDARD_FIGHTER_RADIUS - 3.0;
const FIGHTER_RING_OUTER_RADIUS: f32 = crate::movement::STANDARD_FIGHTER_RADIUS;
const HOT_ZONE_RING_WIDTH: f32 = 10.0;
const GROUND_AREA_HEIGHT: f32 = 1.0;
// The direction arrow is a flat UI marker rather than body geometry. Keep its tip inside the
// fighter's one-cell allocation while extending it beyond the exact collision ring.
const FIGHTER_FACING_TIP_RADIUS: f32 = crate::map::MAP_CELL_SIZE_WORLD * 0.5;
const FIGHTER_FACING_HALF_ANGLE: f32 = 0.22;
const FIGHTER_FACING_ARC_SEGMENTS: u16 = 4;
/// Reviewed maximum bind-pose ground radius of the promoted Kenney body/head mesh AABBs.
const KENNEY_CHARACTER_SOURCE_PLANAR_RADIUS: f32 = 0.420_55;
/// Reviewed bind-pose height of the promoted Kenney character mesh.
const KENNEY_CHARACTER_SOURCE_HEIGHT: f32 = 0.671_325;
const KENNEY_CHARACTER_PLANAR_WORLD_SCALE: f32 =
    crate::movement::STANDARD_FIGHTER_RADIUS / KENNEY_CHARACTER_SOURCE_PLANAR_RADIUS;
const KENNEY_CHARACTER_HEIGHT_WORLD_SCALE: f32 = 64.0;
const KENNEY_CHARACTER_WORLD_HEIGHT: f32 =
    KENNEY_CHARACTER_SOURCE_HEIGHT * KENNEY_CHARACTER_HEIGHT_WORLD_SCALE;
const HEIST_IDOL_FOOTPRINT_WIDTH: f32 = 96.0;
const HEIST_IDOL_FOOTPRINT_DEPTH: f32 = 64.0;

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
    pub(crate) area_disc: Handle<Mesh>,
    pub(crate) effect_sphere: Handle<Mesh>,
    pub(crate) barrel_body: Handle<Mesh>,
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
    pub(crate) concealment_field_blue_fill: Handle<StandardMaterial>,
    pub(crate) concealment_field_blue_boundary: Handle<StandardMaterial>,
    pub(crate) concealment_field_red_fill: Handle<StandardMaterial>,
    pub(crate) concealment_field_red_boundary: Handle<StandardMaterial>,
    pub(crate) effect_muzzle: Handle<StandardMaterial>,
    pub(crate) effect_impact: Handle<StandardMaterial>,
    pub(crate) effect_damage: Handle<StandardMaterial>,
    pub(crate) dash: Handle<StandardMaterial>,
    pub(crate) barrel: Handle<StandardMaterial>,
    pub(crate) barrel_damaged: Handle<StandardMaterial>,
    pub(crate) pickup_glow: Handle<StandardMaterial>,
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

const OBJECT_HEALTH_BAR_WIDTH: f32 = 76.8;
const OBJECT_HEALTH_BAR_HEIGHT: f32 = 11.0;
const OBJECT_HEALTH_WORLD_HEIGHT: f32 = 52.0;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
enum DamageableObjectHealthKey {
    Map {
        map_instance_id: crate::map::MapInstanceId,
        generation: u64,
        placement_id: crate::map::MapPlacementId,
    },
    HeistSafe {
        match_id: crate::matchplay::MatchId,
        anchor_id: crate::map::ModeAnchorId,
    },
}

#[derive(Component)]
struct DamageableObjectHealthUi {
    key: DamageableObjectHealthKey,
    fill: Entity,
}

#[derive(Component)]
struct DamageableObjectHealthFillUi;

#[derive(Resource, Default)]
struct DamageableObjectHealthUiIndex(std::collections::BTreeMap<DamageableObjectHealthKey, Entity>);

/// Client-only 3D world composition. Gameplay authority remains planar and server-owned.
pub(super) struct WorldPresentationPlugin;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum WorldPresentationSet {
    PrepareAssets,
    MaterializeTopology,
    ReconcileState,
    ConsumeCues,
    Animate,
    Cleanup,
}

fn configure_world_presentation_schedule(app: &mut App) {
    app.configure_sets(
        Update,
        (
            WorldPresentationSet::PrepareAssets,
            WorldPresentationSet::MaterializeTopology,
            WorldPresentationSet::ReconcileState,
            WorldPresentationSet::ConsumeCues,
            WorldPresentationSet::Animate,
            WorldPresentationSet::Cleanup,
        )
            .chain()
            .after(CombatClientSet::Sync),
    );
}

impl Plugin for WorldPresentationPlugin {
    #[allow(
        clippy::too_many_lines,
        reason = "the plugin build method is the visible composition point for six documented presentation phases"
    )]
    fn build(&self, app: &mut App) {
        if let Some(config) = app
            .world()
            .resource::<ClientNetworkConfig>()
            .render_measurement
            .clone()
        {
            app.add_plugins(diagnostics::RenderMeasurementPlugin(config));
        }
        configure_world_presentation_schedule(app);
        app.insert_resource(ImportedWorldFallbackPolicy::from_environment())
            .add_message::<combat::PendingCombatEffect>()
            .init_resource::<combat::ConcealedMaterialVariants>()
            .init_resource::<crate::combat::client::AimTraceBlockerIndex>()
            .init_resource::<DamageableObjectHealthUiIndex>()
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
                )
                    .chain()
                    .in_set(WorldPresentationSet::PrepareAssets),
            )
            .add_systems(
                Update,
                reconcile_3d_map
                    .in_set(crate::map::MapPresentationSet::Materialize3d)
                    .in_set(WorldPresentationSet::MaterializeTopology),
            )
            .add_systems(
                Update,
                (
                    reconcile_dynamic_map_visuals,
                    reconcile_restoration_pickup_visuals,
                    reconcile_heist_safe_visuals,
                    (
                        combat::reconcile_combat_visuals,
                        upgrade_fighters_to_imported_models,
                    )
                        .chain(),
                )
                    .in_set(WorldPresentationSet::ReconcileState),
            )
            .add_systems(
                Update,
                (
                    combat::consume_combat_cues,
                    combat::consume_world_object_cues,
                    combat::consume_pickup_cues,
                    combat::consume_heist_objective_cues,
                    combat::materialize_combat_effects,
                )
                    .chain()
                    .in_set(WorldPresentationSet::ConsumeCues),
            )
            .add_systems(
                Update,
                (
                    update_heist_safe_status_visuals,
                    update_damageable_map_visuals,
                    combat::update_fighter_concealment_visuals,
                    combat::update_fighter_overhead_state,
                    combat::reconcile_status_visuals,
                    combat::reconcile_dash_trails,
                    combat::update_aim_preview,
                    update_character_animation,
                    tint_3d_zone,
                )
                    .in_set(WorldPresentationSet::Animate),
            )
            .add_systems(
                Update,
                combat::cleanup_combat_effects.in_set(WorldPresentationSet::Cleanup),
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
                (
                    combat::project_fighter_overhead_ui,
                    project_damageable_object_health_ui,
                )
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

#[derive(Default)]
struct DynamicMapReconcileState {
    stamp: Option<DynamicMapReconcileStamp>,
}

impl DynamicMapReconcileState {
    fn accepts(&mut self, stamp: DynamicMapReconcileStamp) -> bool {
        if self.stamp == Some(stamp) {
            return false;
        }
        self.stamp = Some(stamp);
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct DynamicMapReconcileStamp {
    source_root: Entity,
    instance_id: crate::map::MapInstanceId,
    generation: u64,
    revision: u64,
    imported_readiness: u8,
    visual_count: usize,
}

#[derive(Component)]
struct RestorationPickupVisual {
    owner: Entity,
}

#[derive(Component)]
struct HeistSafeVisual {
    owner: Entity,
}

#[derive(Component)]
struct HeistSafeStatusVisual {
    owner: Entity,
    team_material: Handle<StandardMaterial>,
}

struct HeistSafeVisualAssets<'a> {
    primitives: &'a Primitive3dAssets,
    materials: &'a Material3dAssets,
    imported_core: Option<environment_assets::FittedEnvironmentScene>,
    profile: Option<&'a environment_assets::MapVisualProfile>,
}

fn spawn_heist_safe_visual(
    commands: &mut Commands,
    owner: Entity,
    position: Vec2,
    vertical_scale: f32,
    team_material: &Handle<StandardMaterial>,
    assets: &HeistSafeVisualAssets<'_>,
) {
    let root = commands
        .spawn((
            HeistSafeVisual { owner },
            Transform {
                translation: ground_position(position),
                scale: Vec3::new(1.0, vertical_scale, 1.0),
                ..default()
            },
            Visibility::default(),
            Name::new("Heist team idol"),
        ))
        .id();
    commands.entity(root).with_children(|parent| {
        parent.spawn((
            Mesh3d(assets.primitives.unit_cuboid.clone()),
            MeshMaterial3d(assets.materials.neutral.clone()),
            Transform::from_xyz(0.0, 8.0, 0.0).with_scale(Vec3::new(
                HEIST_IDOL_FOOTPRINT_WIDTH,
                16.0,
                HEIST_IDOL_FOOTPRINT_DEPTH,
            )),
        ));
        if let (Some(fitted), Some(profile)) = (assets.imported_core.as_ref(), assets.profile) {
            let mut transform = fitted.transform;
            transform.translation.y += 16.0;
            parent.spawn((
                environment_assets::EnvironmentMaterialTint([
                    profile.tint.0,
                    profile.tint.1,
                    profile.tint.2,
                ]),
                WorldAssetRoot(fitted.scene.clone()),
                transform,
                Name::new("imported Heist team idol"),
            ));
        } else {
            parent.spawn((
                Mesh3d(assets.primitives.unit_cuboid.clone()),
                MeshMaterial3d(assets.materials.neutral.clone()),
                Transform::from_xyz(0.0, 54.0, 0.0).with_scale(Vec3::new(32.0, 64.0, 32.0)),
                Name::new("primitive Heist idol body"),
            ));
            parent.spawn((
                Mesh3d(assets.primitives.effect_sphere.clone()),
                MeshMaterial3d(assets.materials.neutral.clone()),
                Transform::from_xyz(0.0, 102.0, 0.0).with_scale(Vec3::splat(20.0)),
                Name::new("primitive Heist idol head"),
            ));
        }
        parent.spawn((
            HeistSafeStatusVisual {
                owner,
                team_material: team_material.clone(),
            },
            Mesh3d(assets.primitives.unit_cuboid.clone()),
            MeshMaterial3d(team_material.clone()),
            Transform::from_xyz(0.0, 19.0, 0.0).with_scale(Vec3::new(88.0, 6.0, 56.0)),
        ));
    });
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "Bevy injects the asset, readiness, safe, and visual state owned by this reconciliation phase"
)]
fn reconcile_heist_safe_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    visual_catalog: Option<Res<environment_assets::MapVisualCatalog>>,
    imported: Option<Res<environment_assets::EnvironmentImportedScenes>>,
    readiness: Res<hud::ClientHeistReadiness>,
    safes: Query<(
        Entity,
        &crate::matchplay::HeistSafe,
        &Position,
        &crate::map::DamageableLifeState,
    )>,
    mut visuals: Query<(Entity, &HeistSafeVisual, &mut Transform, &mut Visibility)>,
) {
    let mut existing = std::collections::BTreeSet::new();
    for (visual_entity, visual, mut transform, mut visibility) in &mut visuals {
        let Ok((_, _, position, life)) = safes.get(visual.owner) else {
            commands.entity(visual_entity).despawn();
            continue;
        };
        existing.insert(visual.owner);
        *visibility = if matches!(*readiness, hud::ClientHeistReadiness::Ready) {
            Visibility::Inherited
        } else {
            Visibility::Hidden
        };
        transform.translation = ground_position(position.0);
        transform.scale.y = if matches!(life, crate::map::DamageableLifeState::Live) {
            1.0
        } else {
            0.35
        };
    }
    if !matches!(*readiness, hud::ClientHeistReadiness::Ready) {
        return;
    }
    for (owner, safe, position, life) in &safes {
        if existing.contains(&owner) {
            continue;
        }
        let team_material = if safe.defending_team.0 == 0 {
            materials.team_blue.clone()
        } else {
            materials.team_red.clone()
        };
        let profile_id = crate::map::HEIST_SAFE_VISUAL_PROFILE;
        let profile = visual_catalog
            .as_deref()
            .and_then(|catalog| catalog.profile(profile_id));
        let imported_core = profile.and_then(|profile| {
            imported.as_deref().and_then(|scenes| {
                scenes.fitted(
                    profile_id,
                    profile,
                    Vec2::new(HEIST_IDOL_FOOTPRINT_WIDTH, HEIST_IDOL_FOOTPRINT_DEPTH),
                )
            })
        });
        spawn_heist_safe_visual(
            &mut commands,
            owner,
            position.0,
            if matches!(life, crate::map::DamageableLifeState::Live) {
                1.0
            } else {
                0.35
            },
            &team_material,
            &HeistSafeVisualAssets {
                primitives: &primitives,
                materials: &materials,
                imported_core,
                profile,
            },
        );
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy system parameters are injected by value"
)]
fn update_heist_safe_status_visuals(
    materials: Res<Material3dAssets>,
    safes: Query<
        (
            Ref<crate::combat::CurrentHealth>,
            Ref<crate::map::DamageableMaximumHealth>,
            Ref<crate::map::DamageableLifeState>,
        ),
        With<crate::matchplay::HeistSafe>,
    >,
    mut status_visuals: Query<(
        Ref<HeistSafeStatusVisual>,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    for (status, mut material) in &mut status_visuals {
        let Ok((health, maximum, life)) = safes.get(status.owner) else {
            continue;
        };
        if !status.is_added() && !health.is_changed() && !maximum.is_changed() && !life.is_changed()
        {
            continue;
        }
        let critical = maximum.0 > 0
            && u32::from(health.0) * 100
                <= u32::from(maximum.0)
                    * u32::from(crate::matchplay::HEIST_CRITICAL_HEALTH_PERCENT);
        material.0 =
            if critical || matches!(*life, crate::map::DamageableLifeState::TerminalCommitted) {
                materials.effect_damage.clone()
            } else {
                status.team_material.clone()
            };
    }
}

fn map_profile_has_dynamic_runtime(profile: &crate::map::MapGameplayProfile) -> bool {
    profile.destruction != crate::map::MapDestructionBehavior::Indestructible
        || profile.durability != crate::map::MapDurabilityBehavior::Indestructible
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "parameters are Bevy system parameters owned by the presentation schedule"
)]
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the Bevy reconciliation system atomically compares one dynamic map generation and materializes its bounded visual set"
)]
fn reconcile_dynamic_map_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    theme_materials: Res<environment_assets::EnvironmentThemeMaterialCatalog>,
    presentation_materials: Res<Material3dAssets>,
    map_visuals: Option<Res<environment_assets::MapVisualCatalog>>,
    imported_scenes: Option<Res<environment_assets::EnvironmentImportedScenes>>,
    imported_readiness: Option<Res<environment_assets::EnvironmentAssetReadiness>>,
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
    mut reconciliation: Local<DynamicMapReconcileState>,
) {
    let Some(accepted) = accepted else {
        return;
    };
    let Ok((snapshot, state)) = grids.get(accepted.source_root) else {
        return;
    };
    let imported_readiness_stamp = match imported_readiness.as_deref() {
        None => 0,
        Some(environment_assets::EnvironmentAssetReadiness::Loading) => 1,
        Some(environment_assets::EnvironmentAssetReadiness::Ready) => 2,
        Some(environment_assets::EnvironmentAssetReadiness::Degraded(_)) => 3,
    };
    let stamp = DynamicMapReconcileStamp {
        source_root: accepted.source_root,
        instance_id: snapshot.identity.instance_id,
        generation: state.generation,
        revision: state.revision,
        imported_readiness: imported_readiness_stamp,
        visual_count: existing.iter().count(),
    };
    if !reconciliation.accepts(stamp) {
        return;
    }
    let terminal: std::collections::BTreeMap<_, _> = state
        .terminal_states
        .iter()
        .map(|transition| (transition.placement_id, transition.outcome))
        .collect();
    let placements_by_id: std::collections::BTreeMap<_, _> = snapshot
        .placements
        .iter()
        .map(|placement| (placement.placement_id, placement))
        .collect();
    let desired_asset =
        |placement: &crate::map::MapAssetPlacement| match terminal.get(&placement.placement_id) {
            Some(crate::map::MapPlacementOutcome::Removed) => None,
            Some(crate::map::MapPlacementOutcome::ReplacedWith(asset_id)) => Some(*asset_id),
            None => Some(placement.asset_id),
        };
    let mut present = std::collections::BTreeSet::new();
    for (entity, member, visual) in &existing {
        let desired = placements_by_id
            .get(&visual.placement_id)
            .and_then(|placement| desired_asset(placement));
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
            .is_some_and(map_profile_has_dynamic_runtime);
        if !dynamic || present.contains(&placement.placement_id) {
            continue;
        }
        let Some(asset_id) = desired_asset(placement) else {
            continue;
        };
        let Some(asset) = catalog.0.asset(asset_id) else {
            continue;
        };
        let profile = map_visuals
            .as_deref()
            .and_then(|visuals| visuals.profile(asset.visual_profile_id));
        let footprint = asset.footprint_cells.rotated(placement.quarter_turns);
        let imported_requested = profile.is_some_and(|profile| {
            matches!(
                profile.kind,
                environment_assets::MapVisualKind::Imported { .. }
            )
        });
        let fitted = profile.and_then(|profile| {
            imported_scenes.as_deref().and_then(|scenes| {
                scenes.fitted(
                    asset.visual_profile_id,
                    profile,
                    Vec2::new(
                        f32::from(footprint.width) * crate::map::MAP_CELL_SIZE_WORLD,
                        f32::from(footprint.height) * crate::map::MAP_CELL_SIZE_WORLD,
                    ),
                )
            })
        });
        if imported_requested
            && fitted.is_none()
            && imported_readiness.as_deref().is_some_and(|readiness| {
                matches!(
                    readiness,
                    environment_assets::EnvironmentAssetReadiness::Loading
                )
            })
        {
            continue;
        }
        let center = crate::map::placement_world_center(snapshot.dimensions, asset, placement);
        let terminal_debris = asset_id == crate::map::BARREL_WOOD_DEBRIS_ASSET;
        let material = if asset_id == crate::map::OIL_BARREL_ASSET {
            presentation_materials.barrel.clone()
        } else if asset_id == crate::map::RUBBLE_ASSET || terminal_debris {
            materials.rubble.clone()
        } else {
            materials.destructible_cover.clone()
        };
        let visual = commands
            .spawn((
                marker,
                DynamicMapVisual {
                    placement_id: placement.placement_id,
                    generation: state.generation,
                    asset_id,
                },
                Transform {
                    translation: ground_position(center),
                    rotation: Quat::from_rotation_y(
                        f32::from(placement.quarter_turns) * core::f32::consts::FRAC_PI_2,
                    ),
                    scale: Vec3::ONE,
                },
                Visibility::default(),
                Name::new("dynamic map asset"),
            ))
            .id();
        if let (Some(profile), Some(fitted)) = (profile, fitted) {
            commands.entity(visual).with_children(|parent| {
                parent.spawn((
                    environment_assets::EnvironmentMaterialTint([
                        profile.tint.0,
                        profile.tint.1,
                        profile.tint.2,
                    ]),
                    WorldAssetRoot(fitted.scene),
                    fitted.transform,
                    Name::new("imported dynamic map asset"),
                ));
            });
        } else {
            commands.entity(visual).with_children(|parent| {
                parent.spawn((
                    Mesh3d(if asset_id == crate::map::OIL_BARREL_ASSET {
                        primitives.barrel_body.clone()
                    } else {
                        primitives.cover_block.clone()
                    }),
                    MeshMaterial3d(material),
                    primitive_dynamic_visual_transform(asset_id, footprint),
                    Name::new("primitive dynamic map asset"),
                ));
            });
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    reason = "pickup presentation reconciles durable replicated owners with imported and primitive assets"
)]
fn reconcile_restoration_pickup_visuals(
    mut commands: Commands,
    time: Res<Time>,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    visual_catalog: Option<Res<environment_assets::MapVisualCatalog>>,
    imported: Option<Res<environment_assets::EnvironmentImportedScenes>>,
    catalog: Res<crate::map::MapCatalogResource>,
    pickups: Query<(
        Entity,
        &Position,
        &crate::map::RestorationPickupDefinitionId,
        Ref<crate::map::RestorationPickup>,
    )>,
    mut visuals: Query<(Entity, &RestorationPickupVisual, &mut Transform)>,
) {
    for (visual_entity, visual, mut transform) in &mut visuals {
        let Ok((_, position, _, _)) = pickups.get(visual.owner) else {
            commands.entity(visual_entity).despawn();
            continue;
        };
        let bob = (time.elapsed_secs() * 2.8).sin() * 3.0;
        transform.translation = ground_position(position.0) + Vec3::Y * (16.0 + bob);
        transform.rotation = Quat::from_rotation_y(time.elapsed_secs() * 0.8);
    }
    for (owner, position, definition_id, pickup) in &pickups {
        if !pickup.is_added() || visuals.iter().any(|(_, visual, _)| visual.owner == owner) {
            continue;
        }
        let Some(definition) = catalog.0.restoration_pickup(*definition_id) else {
            continue;
        };
        let profile = visual_catalog
            .as_deref()
            .and_then(|visuals| visuals.profile(definition.visual_profile_id));
        let fitted = profile.and_then(|profile| {
            imported.as_deref().and_then(|scenes| {
                scenes.fitted(
                    definition.visual_profile_id,
                    profile,
                    Vec2::splat(crate::map::MAP_CELL_SIZE_WORLD),
                )
            })
        });
        let root = commands
            .spawn((
                RestorationPickupVisual { owner },
                Transform::from_translation(ground_position(position.0) + Vec3::Y * 16.0),
                Visibility::default(),
                Name::new("restoration pickup visual"),
            ))
            .id();
        commands.entity(root).with_children(|parent| {
            parent.spawn((
                Mesh3d(primitives.area_disc.clone()),
                MeshMaterial3d(materials.pickup_glow.clone()),
                Transform::from_xyz(0.0, -15.0, 0.0).with_scale(Vec3::splat(36.0)),
                Name::new("restoration pickup ground glow"),
            ));
            if let (Some(profile), Some(fitted)) = (profile, fitted) {
                parent.spawn((
                    environment_assets::EnvironmentMaterialTint([
                        profile.tint.0,
                        profile.tint.1,
                        profile.tint.2,
                    ]),
                    WorldAssetRoot(fitted.scene),
                    fitted.transform,
                    Name::new("imported restoration potion"),
                ));
            } else {
                parent.spawn((
                    Mesh3d(primitives.effect_sphere.clone()),
                    MeshMaterial3d(materials.pickup_glow.clone()),
                    Transform::from_scale(Vec3::new(16.0, 28.0, 16.0)),
                    Name::new("primitive restoration pickup"),
                ));
            }
        });
    }
}

fn primitive_dynamic_visual_transform(
    asset_id: crate::map::MapAssetId,
    footprint: crate::map::MapFootprint,
) -> Transform {
    if asset_id == crate::map::OIL_BARREL_ASSET {
        Transform::from_translation(Vec3::Y * 14.0)
    } else {
        let low = asset_id == crate::map::RUBBLE_ASSET
            || asset_id == crate::map::BARREL_WOOD_DEBRIS_ASSET;
        Transform {
            translation: Vec3::Y * if low { 4.0 } else { 16.0 },
            scale: Vec3::new(
                f32::from(footprint.width) * 0.5,
                if low { 0.25 } else { 1.0 },
                f32::from(footprint.height) * 0.5,
            ),
            ..default()
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "Bevy systems receive resource system parameters by value"
)]
fn update_damageable_map_visuals(
    materials: Res<Material3dAssets>,
    objects: Query<
        (
            &crate::map::DamageableTargetIdentity,
            Ref<crate::combat::CurrentHealth>,
            Ref<crate::map::DamageableMaximumHealth>,
        ),
        With<crate::map::DamageableWorldObject>,
    >,
    mut visuals: Query<(Ref<DynamicMapVisual>, &mut MeshMaterial3d<StandardMaterial>)>,
) {
    for (visual, mut material) in &mut visuals {
        if visual.asset_id != crate::map::OIL_BARREL_ASSET {
            continue;
        }
        let Some((_, health, maximum)) = objects
            .iter()
            .find(|(identity, ..)| identity.placement_id() == visual.placement_id)
        else {
            continue;
        };
        if !visual.is_added() && !health.is_changed() && !maximum.is_changed() {
            continue;
        }
        material.0 = if health.0 < maximum.0 {
            materials.barrel_damaged.clone()
        } else {
            materials.barrel.clone()
        };
    }
}

fn damageable_object_health_fraction(current: u16, maximum: u16) -> Option<f32> {
    (maximum > 0 && current > 0 && current < maximum)
        .then(|| f32::from(current) / f32::from(maximum))
}

fn spawn_damageable_object_health_ui(
    commands: &mut Commands,
    index: &mut DamageableObjectHealthUiIndex,
    key: DamageableObjectHealthKey,
    fraction: f32,
) {
    let fill = commands
        .spawn((
            DamageableObjectHealthFillUi,
            Node {
                width: percent(fraction * 100.0),
                height: percent(100.0),
                border_radius: BorderRadius::all(px(5.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.96, 0.48, 0.08)),
            Name::new("damageable object floating health fill"),
        ))
        .id();
    let root = commands
        .spawn((
            DamageableObjectHealthUi { key, fill },
            Node {
                position_type: PositionType::Absolute,
                width: px(OBJECT_HEALTH_BAR_WIDTH),
                height: px(OBJECT_HEALTH_BAR_HEIGHT),
                padding: UiRect::all(px(2.0)),
                overflow: Overflow::clip(),
                border_radius: BorderRadius::all(px(6.0)),
                ..default()
            },
            BackgroundColor(Color::srgb(0.025, 0.03, 0.04)),
            GlobalZIndex(119),
            Visibility::Hidden,
            Name::new("damageable object projected floating health bar"),
        ))
        .add_child(fill)
        .id();
    index.0.insert(key, root);
}

fn projected_object_health_top_left(viewport: Vec2, anchor: Vec2) -> Option<Vec2> {
    let top_left = anchor
        - Vec2::new(
            OBJECT_HEALTH_BAR_WIDTH * 0.5,
            OBJECT_HEALTH_BAR_HEIGHT * 0.5,
        );
    (top_left.x + OBJECT_HEALTH_BAR_WIDTH >= 0.0
        && top_left.x <= viewport.x
        && top_left.y + OBJECT_HEALTH_BAR_HEIGHT >= 0.0
        && top_left.y <= viewport.y)
        .then_some(top_left)
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "the projection system reconciles replicated object health with dynamic world visuals and screen-space UI"
)]
fn project_damageable_object_health_ui(
    mut commands: Commands,
    mut index: ResMut<DamageableObjectHealthUiIndex>,
    cameras: Query<(&Camera, &GlobalTransform), With<ArenaCamera>>,
    heist_readiness: Res<hud::ClientHeistReadiness>,
    objects: Query<
        (
            &crate::map::DamageableTargetIdentity,
            &crate::combat::CurrentHealth,
            &crate::map::DamageableMaximumHealth,
        ),
        With<crate::map::DamageableWorldObject>,
    >,
    safes: Query<(
        &crate::matchplay::HeistSafe,
        &Position,
        &crate::combat::CurrentHealth,
        &crate::map::DamageableMaximumHealth,
    )>,
    visuals: Query<(
        &DynamicMapVisual,
        &crate::map::MapPresentationMember,
        &GlobalTransform,
    )>,
    mut overheads: Query<(&DamageableObjectHealthUi, &mut Node, &mut Visibility)>,
    mut fills: Query<
        &mut Node,
        (
            With<DamageableObjectHealthFillUi>,
            Without<DamageableObjectHealthUi>,
        ),
    >,
) {
    let damaged: std::collections::BTreeMap<_, _> = objects
        .iter()
        .filter_map(|(identity, health, maximum)| {
            damageable_object_health_fraction(health.0, maximum.0).map(|fraction| {
                let generation = identity.generation();
                (
                    DamageableObjectHealthKey::Map {
                        map_instance_id: generation.map_instance_id,
                        generation: generation.generation,
                        placement_id: identity.placement_id(),
                    },
                    fraction,
                )
            })
        })
        .collect();
    let mut desired: std::collections::BTreeMap<_, _> = visuals
        .iter()
        .filter_map(|(visual, member, transform)| {
            let key = DamageableObjectHealthKey::Map {
                map_instance_id: member.instance_id,
                generation: visual.generation,
                placement_id: visual.placement_id,
            };
            damaged
                .get(&key)
                .map(|fraction| (key, (transform.translation(), *fraction)))
        })
        .collect();
    if matches!(*heist_readiness, hud::ClientHeistReadiness::Ready) {
        for (safe, position, health, maximum) in &safes {
            if maximum.0 == 0 {
                continue;
            }
            desired.insert(
                DamageableObjectHealthKey::HeistSafe {
                    match_id: safe.match_id,
                    anchor_id: safe.anchor_id,
                },
                (
                    ground_position(position.0),
                    f32::from(health.0) / f32::from(maximum.0),
                ),
            );
        }
    }
    let projection = cameras.single().ok().and_then(|(camera, transform)| {
        camera
            .logical_viewport_size()
            .map(|viewport| (camera, transform, viewport))
    });
    let stale: Vec<_> = index
        .0
        .iter()
        .filter_map(|(key, entity)| (!desired.contains_key(key)).then_some((*key, *entity)))
        .collect();
    for (key, entity) in stale {
        commands.entity(entity).try_despawn();
        index.0.remove(&key);
    }
    for (key, (world_position, fraction)) in &desired {
        let Some(entity) = index.0.get(key).copied() else {
            spawn_damageable_object_health_ui(&mut commands, &mut index, *key, *fraction);
            continue;
        };
        let Ok((overhead, mut node, mut visibility)) = overheads.get_mut(entity) else {
            index.0.remove(key);
            spawn_damageable_object_health_ui(&mut commands, &mut index, *key, *fraction);
            continue;
        };
        debug_assert_eq!(overhead.key, *key);
        if let Ok(mut fill) = fills.get_mut(overhead.fill) {
            fill.width = percent(fraction * 100.0);
        }
        let Some((camera, camera_transform, viewport)) = projection else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Ok(anchor) = camera.world_to_viewport(
            camera_transform,
            *world_position + Vec3::Y * OBJECT_HEALTH_WORLD_HEIGHT,
        ) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(top_left) = projected_object_health_top_left(viewport, anchor) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        node.left = px(top_left.x);
        node.top = px(top_left.y);
        *visibility = Visibility::Inherited;
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
        fighter: meshes.add(Sphere::new(FIGHTER_FALLBACK_RADIUS)),
        sentry_direction: meshes.add(Cuboid::new(28.0, 7.0, 8.0)),
        fighter_facing: meshes.add(fighter_facing_mesh()),
        projectile: meshes.add(Cylinder::new(1.0, 1.0)),
        lobbed_projectile: meshes.add(Sphere::new(9.0)),
        unit_cuboid: meshes.add(Cuboid::new(1.0, 1.0, 1.0)),
        sentry_base: meshes.add(Cylinder::new(22.0, 8.0)),
        sentry_body: meshes.add(Cylinder::new(15.0, 24.0)),
        ground_ring: meshes.add(Annulus::new(
            FIGHTER_RING_INNER_RADIUS,
            FIGHTER_RING_OUTER_RADIUS,
        )),
        area_ring: meshes.add(Annulus::new(0.93, 1.0)),
        area_disc: meshes.add(Circle::new(1.0)),
        effect_sphere: meshes.add(Sphere::new(1.0)),
        barrel_body: meshes.add(Cylinder::new(16.0, 28.0)),
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
        concealment_field_blue_fill: materials.add(StandardMaterial {
            base_color: Color::srgba(0.08, 0.62, 1.0, 0.22),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        concealment_field_blue_boundary: materials.add(StandardMaterial {
            base_color: Color::srgb(0.08, 0.72, 1.0),
            emissive: LinearRgba::new(0.05, 0.75, 1.5, 1.0),
            unlit: true,
            ..default()
        }),
        concealment_field_red_fill: materials.add(StandardMaterial {
            base_color: Color::srgba(1.0, 0.2, 0.16, 0.22),
            alpha_mode: AlphaMode::Blend,
            unlit: true,
            ..default()
        }),
        concealment_field_red_boundary: materials.add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.24, 0.16),
            emissive: LinearRgba::new(1.5, 0.12, 0.05, 1.0),
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
        barrel: materials.add(matte(Color::srgb(0.92, 0.38, 0.06))),
        barrel_damaged: materials.add(matte(Color::srgb(0.34, 0.12, 0.04))),
        pickup_glow: materials.add(StandardMaterial {
            base_color: Color::srgba(0.18, 1.0, 0.42, 0.58),
            emissive: LinearRgba::new(0.12, 2.4, 0.42, 1.0),
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
            .is_some_and(map_profile_has_dynamic_runtime);
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
        let footprint = asset.footprint_cells.rotated(placement.quarter_turns);
        let fitted = profile.and_then(|profile| {
            imported.and_then(|scenes| {
                scenes.fitted(
                    asset.visual_profile_id,
                    profile,
                    Vec2::new(
                        f32::from(footprint.width) * crate::map::MAP_CELL_SIZE_WORLD,
                        f32::from(footprint.height) * crate::map::MAP_CELL_SIZE_WORLD,
                    ),
                )
            })
        });
        if let (Some(profile), Some(fitted)) = (profile, fitted) {
            spawn_imported_map_asset(commands, marker, center, rotation, profile, fitted);
        } else if placement.asset_id == crate::map::WATER_ASSET {
            spawn_map_water(commands, primitives, materials, marker, center, adjacency);
        } else if placement.asset_id == crate::map::TALL_GRASS_ASSET {
            spawn_map_grass(
                commands, primitives, materials, marker, center, rotation, adjacency,
            );
        } else if asset.slot == crate::map::MapAssetSlot::Feature {
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

fn spawn_imported_map_asset(
    commands: &mut Commands,
    marker: crate::map::MapPresentationMember,
    center: Vec2,
    rotation: f32,
    profile: &environment_assets::MapVisualProfile,
    fitted: environment_assets::FittedEnvironmentScene,
) {
    let root = commands
        .spawn((
            marker,
            Transform {
                translation: ground_position(center),
                rotation: Quat::from_rotation_y(rotation),
                ..default()
            },
            Visibility::default(),
            Name::new("imported map asset root"),
        ))
        .id();
    commands.entity(root).with_children(|parent| {
        parent.spawn((
            environment_assets::EnvironmentMaterialTint([
                profile.tint.0,
                profile.tint.1,
                profile.tint.2,
            ]),
            WorldAssetRoot(fitted.scene),
            fitted.transform,
            Name::new("imported map asset"),
        ));
    });
}

fn hot_zone_visual_geometry(snapshot: &crate::map::ResolvedMapSnapshot) -> Option<(Vec2, f32)> {
    snapshot.mode_anchors.iter().find_map(|anchor| {
        let crate::map::MapModeAnchorKind::HotZoneCircle {
            center_half_cell,
            radius_half_cells,
        } = anchor.kind
        else {
            return None;
        };
        snapshot
            .dimensions
            .half_cell_world(center_half_cell)
            .map(|center| {
                (
                    center,
                    f32::from(radius_half_cells) * (crate::map::MAP_CELL_SIZE_WORLD * 0.5),
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
        Name::new(format!("tall grass adjacency {adjacency:04b}")),
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
                    scale: Vec3::new(
                        KENNEY_CHARACTER_PLANAR_WORLD_SCALE,
                        KENNEY_CHARACTER_HEIGHT_WORLD_SCALE,
                        KENNEY_CHARACTER_PLANAR_WORLD_SCALE,
                    ),
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

    #[derive(Resource, Default)]
    struct PresentationScheduleTrace(Vec<&'static str>);

    fn presentation_probe(
        label: &'static str,
    ) -> impl FnMut(ResMut<PresentationScheduleTrace>) + 'static {
        move |mut trace: ResMut<PresentationScheduleTrace>| trace.0.push(label)
    }

    #[test]
    fn presentation_phases_preserve_sync_and_semantic_order() {
        let mut app = App::new();
        app.init_resource::<PresentationScheduleTrace>();
        configure_world_presentation_schedule(&mut app);
        app.add_systems(
            Update,
            (
                presentation_probe("sync").in_set(CombatClientSet::Sync),
                presentation_probe("assets").in_set(WorldPresentationSet::PrepareAssets),
                presentation_probe("topology").in_set(WorldPresentationSet::MaterializeTopology),
                presentation_probe("state").in_set(WorldPresentationSet::ReconcileState),
                presentation_probe("cues").in_set(WorldPresentationSet::ConsumeCues),
                presentation_probe("animate").in_set(WorldPresentationSet::Animate),
                presentation_probe("cleanup").in_set(WorldPresentationSet::Cleanup),
            ),
        );
        crate::test_app::reject_owned_schedule_ambiguities(&mut app, Update);

        app.update();

        assert_eq!(
            app.world().resource::<PresentationScheduleTrace>().0,
            vec![
                "sync", "assets", "topology", "state", "cues", "animate", "cleanup"
            ]
        );
    }

    #[test]
    fn maximum_builtin_dynamic_map_reconciles_once_across_stable_frames() {
        let catalog = crate::map::MapContentCatalog::embedded().unwrap();
        let maximum_dynamic_visuals = catalog
            .presets
            .iter()
            .map(|preset| {
                let resolved = catalog
                    .resolve_preset(preset.id, crate::map::MapInstanceId(1))
                    .unwrap();
                resolved
                    .snapshot
                    .placements
                    .iter()
                    .filter(|placement| {
                        catalog
                            .asset(placement.asset_id)
                            .and_then(|asset| catalog.profile(asset.gameplay_profile_id))
                            .is_some_and(map_profile_has_dynamic_runtime)
                    })
                    .count()
            })
            .max()
            .unwrap();
        assert!(maximum_dynamic_visuals > 0);

        let mut reconciliation = DynamicMapReconcileState::default();
        let mut stamp = DynamicMapReconcileStamp {
            source_root: Entity::PLACEHOLDER,
            instance_id: crate::map::MapInstanceId(1),
            generation: 1,
            revision: 0,
            imported_readiness: 2,
            visual_count: maximum_dynamic_visuals,
        };
        let stable_runs = (0..600).filter(|_| reconciliation.accepts(stamp)).count();
        assert_eq!(
            stable_runs, 1,
            "stable topology does one reconciliation pass"
        );

        stamp.revision = 1;
        assert!(reconciliation.accepts(stamp));
        stamp.imported_readiness = 3;
        assert!(reconciliation.accepts(stamp));
        stamp.visual_count -= 1;
        assert!(reconciliation.accepts(stamp));
    }

    #[test]
    fn feature_yard_hot_zone_anchor_materializes_at_exact_world_scale() {
        let resolved = crate::map::MapContentCatalog::embedded()
            .unwrap()
            .resolve_preset(
                crate::map::FEATURE_YARD_HOT_ZONE_PRESET,
                crate::map::MapInstanceId(1),
            )
            .unwrap();

        assert_eq!(
            hot_zone_visual_geometry(&resolved.snapshot),
            Some((Vec2::ZERO, 5.0 * crate::map::MAP_CELL_SIZE_WORLD))
        );
    }

    #[test]
    fn switchback_basin_anchor_materializes_half_cell_radius() {
        let resolved = crate::map::MapContentCatalog::embedded()
            .unwrap()
            .resolve_preset(
                crate::map::SWITCHBACK_BASIN_PRESET,
                crate::map::MapInstanceId(1),
            )
            .unwrap();

        assert_eq!(
            hot_zone_visual_geometry(&resolved.snapshot),
            Some((Vec2::ZERO, 3.5 * crate::map::MAP_CELL_SIZE_WORLD))
        );
    }

    #[test]
    fn imported_character_front_aligns_with_fighter_root_facing() {
        let corrected_front = Quat::from_rotation_y(KENNEY_CHARACTER_FORWARD_CORRECTION) * Vec3::Z;

        assert!(corrected_front.abs_diff_eq(Vec3::X, 1e-5));
    }

    #[test]
    fn damageable_object_health_bar_exists_only_between_full_and_terminal_health() {
        assert_eq!(damageable_object_health_fraction(60, 60), None);
        assert_eq!(damageable_object_health_fraction(0, 60), None);
        assert_eq!(damageable_object_health_fraction(20, 0), None);
        assert_eq!(damageable_object_health_fraction(30, 60), Some(0.5));
    }

    #[test]
    fn hp_durable_map_assets_are_excluded_from_static_materialization() {
        let catalog = crate::map::MapContentCatalog::embedded().unwrap();
        let barrel = catalog.asset(crate::map::OIL_BARREL_ASSET).unwrap();
        let profile = catalog.profile(barrel.gameplay_profile_id).unwrap();

        assert_eq!(
            profile.destruction,
            crate::map::MapDestructionBehavior::Indestructible
        );
        assert!(matches!(
            profile.durability,
            crate::map::MapDurabilityBehavior::HitPoints(_)
        ));
        assert!(map_profile_has_dynamic_runtime(profile));
    }

    #[test]
    fn primitive_barrel_and_debris_are_grounded_at_their_actual_half_heights() {
        let footprint = crate::map::MapFootprint {
            width: 1,
            height: 1,
        };
        let barrel = primitive_dynamic_visual_transform(crate::map::OIL_BARREL_ASSET, footprint);
        let debris =
            primitive_dynamic_visual_transform(crate::map::BARREL_WOOD_DEBRIS_ASSET, footprint);

        assert_eq!(barrel.translation, Vec3::Y * 14.0);
        assert_eq!(barrel.scale, Vec3::ONE);
        assert_eq!(debris.translation, Vec3::Y * 4.0);
        assert_eq!(debris.scale, Vec3::new(0.5, 0.25, 0.5));
    }

    #[test]
    fn primitive_chest_matches_its_one_cell_authoritative_footprint() {
        let footprint = crate::map::MapFootprint {
            width: 1,
            height: 1,
        };
        let transform =
            primitive_dynamic_visual_transform(crate::map::TREASURE_CHEST_ASSET, footprint);

        assert_eq!(transform.translation, Vec3::Y * 16.0);
        assert_eq!(transform.scale, Vec3::new(0.5, 1.0, 0.5));
    }

    #[test]
    fn heist_idol_presentation_stays_inside_authoritative_footprint() {
        let resolved = crate::map::MapContentCatalog::embedded()
            .unwrap()
            .resolve_preset(
                crate::map::FEATURE_YARD_HEIST_PRESET,
                crate::map::MapInstanceId(1),
            )
            .unwrap();
        let safe = resolved.heist_safes.first().unwrap();
        assert!((safe.half_extents.x * 2.0 - HEIST_IDOL_FOOTPRINT_WIDTH).abs() <= f32::EPSILON);
        assert!((safe.half_extents.y * 2.0 - HEIST_IDOL_FOOTPRINT_DEPTH).abs() <= f32::EPSILON);

        let catalog = crate::map::MapContentCatalog::embedded().unwrap();
        let visuals = environment_assets::MapVisualCatalog::embedded(&catalog).unwrap();
        let profile = visuals
            .profile(crate::map::HEIST_SAFE_VISUAL_PROFILE)
            .unwrap();
        assert_eq!(
            profile.fitting,
            environment_assets::MapVisualFitting::Contained
        );
        assert!(profile.scale <= 1.0);
    }

    #[test]
    fn damageable_object_health_projection_rejects_offscreen_anchors() {
        assert!(
            projected_object_health_top_left(Vec2::new(640.0, 360.0), Vec2::new(320.0, 180.0))
                .unwrap()
                .abs_diff_eq(Vec2::new(281.6, 174.5), 1e-4)
        );
        assert_eq!(
            projected_object_health_top_left(Vec2::new(640.0, 360.0), Vec2::new(700.0, 180.0)),
            None
        );
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
    fn fighter_visual_footprint_matches_the_authoritative_body() {
        assert!(
            (FIGHTER_FALLBACK_RADIUS - crate::movement::STANDARD_FIGHTER_RADIUS).abs()
                < f32::EPSILON
        );
        assert!(
            (FIGHTER_RING_OUTER_RADIUS - crate::movement::STANDARD_FIGHTER_RADIUS).abs()
                < f32::EPSILON
        );
        assert!(
            (FIGHTER_FACING_TIP_RADIUS - crate::map::MAP_CELL_SIZE_WORLD * 0.5).abs()
                < f32::EPSILON
        );
        assert!(
            (KENNEY_CHARACTER_PLANAR_WORLD_SCALE * KENNEY_CHARACTER_SOURCE_PLANAR_RADIUS
                - crate::movement::STANDARD_FIGHTER_RADIUS)
                .abs()
                < 1e-4
        );
        assert!((KENNEY_CHARACTER_WORLD_HEIGHT - 42.965).abs() < 0.001);
        assert!((KENNEY_CHARACTER_HEIGHT_WORLD_SCALE - 64.0).abs() < f32::EPSILON);
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
