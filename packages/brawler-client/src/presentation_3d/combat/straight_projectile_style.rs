//! Retained neutral straight-bolt styling shared by single and spread weapon recipes. The
//! nucleus and wake carry the observer-relative weapon team tint (BRL-h270).

use super::super::{
    AuthoritativeTick, Material3dAssets, NotShadowCaster, NotShadowReceiver, Primitive3dAssets, vfx,
};
use bevy::prelude::*;
use bevy::{
    ecs::hierarchy::ChildSpawnerCommands, reflect::TypePath, render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

const STRAIGHT_PROJECTILE_TRAIL_SHADER_PATH: &str = "shaders/straight_projectile_trail.wgsl";

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(crate) struct StraightProjectileTrailMaterial {
    #[uniform(0)]
    params: Vec4,
    #[uniform(1)]
    color: Vec4,
}

impl StraightProjectileTrailMaterial {
    /// A wake in one linear HDR relation colour (ally blue or enemy red).
    pub(crate) fn new(color: LinearRgba, quality: TrailQuality) -> Self {
        Self {
            params: Vec4::new(0.0, quality.shader_index(), 0.0, 0.0),
            color: Vec4::new(color.red, color.green, color.blue, 1.0),
        }
    }
}

impl Material for StraightProjectileTrailMaterial {
    fn fragment_shader() -> ShaderRef {
        STRAIGHT_PROJECTILE_TRAIL_SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        8.0
    }

    fn enable_shadows() -> bool {
        false
    }
}

pub(crate) struct StraightProjectileTrailMaterialPlugin;

impl Plugin for StraightProjectileTrailMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<StraightProjectileTrailMaterial>::default());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TrailQuality {
    Normal,
    Reduced,
}

impl TrailQuality {
    const fn shader_index(self) -> f32 {
        match self {
            Self::Normal => 0.0,
            Self::Reduced => 1.0,
        }
    }

    const fn length_scale(self) -> f32 {
        match self {
            Self::Normal => 1.0,
            Self::Reduced => 0.68,
        }
    }
}

fn neutral_straight_trail_quality(signature: vfx::CombatVisualSignature) -> Option<TrailQuality> {
    if signature.carrier != vfx::VfxCarrier::StraightProjectile
        || !signature.accents.contains(&Some(vfx::VfxAccent::Neutral))
    {
        return None;
    }
    match signature.variant {
        vfx::VfxPresentationVariant::Normal => Some(TrailQuality::Normal),
        vfx::VfxPresentationVariant::Reduced => Some(TrailQuality::Reduced),
        vfx::VfxPresentationVariant::Primitive | vfx::VfxPresentationVariant::ReducedPrimitive => {
            None
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the retained bolt helper receives one complete immutable projectile visual profile"
)]
pub(super) fn spawn_neutral_straight_projectile_style(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    signature: vfx::CombatVisualSignature,
    core_transform: Transform,
    tint: vfx::WeaponTeamTint,
    visual_radius: f32,
) -> bool {
    let Some(quality) = neutral_straight_trail_quality(signature) else {
        return false;
    };

    let nucleus_transform = Transform::from_xyz(0.0, core_transform.scale.y * 0.48, 0.0)
        .with_scale(Vec3::new(visual_radius, visual_radius, visual_radius));
    parent.spawn((
        Mesh3d(primitives.effect_sphere.clone()),
        MeshMaterial3d(match tint {
            vfx::WeaponTeamTint::Ally => materials.projectile_hot_ally.clone(),
            vfx::WeaponTeamTint::Enemy => materials.projectile_hot_enemy.clone(),
        }),
        NotShadowCaster,
        NotShadowReceiver,
        nucleus_transform,
        Name::new("Neutral straight bolt exact-footprint dimensional nucleus"),
    ));

    let trail_length = visual_radius.mul_add(4.0, 24.0).clamp(36.0, 56.0) * quality.length_scale();
    let trail_width = visual_radius.mul_add(2.8, 5.0).clamp(18.0, 28.0);
    parent.spawn((
        Mesh3d(primitives.projectile_trail.clone()),
        MeshMaterial3d(projectile_trail_material(tint, quality, materials)),
        NotShadowCaster,
        NotShadowReceiver,
        Transform::from_xyz(0.0, core_transform.scale.y * 0.56, 0.0).with_scale(Vec3::new(
            trail_length,
            1.0,
            trail_width,
        )),
        Name::new("Neutral straight bolt animated wake"),
    ));
    true
}

fn projectile_trail_material(
    tint: vfx::WeaponTeamTint,
    quality: TrailQuality,
    materials: &Material3dAssets,
) -> Handle<StraightProjectileTrailMaterial> {
    use vfx::WeaponTeamTint::{Ally, Enemy};
    match (tint, quality) {
        (Ally, TrailQuality::Normal) => materials.projectile_trail_ally.clone(),
        (Enemy, TrailQuality::Normal) => materials.projectile_trail_enemy.clone(),
        (Ally, TrailQuality::Reduced) => materials.projectile_trail_ally_reduced.clone(),
        (Enemy, TrailQuality::Reduced) => materials.projectile_trail_enemy_reduced.clone(),
    }
}

pub(crate) fn animate_straight_projectile_trail_materials(
    ticks: Query<&AuthoritativeTick>,
    handles: Option<Res<Material3dAssets>>,
    mut materials: ResMut<Assets<StraightProjectileTrailMaterial>>,
) {
    const VISUAL_CYCLE_SECONDS: u64 = 900;

    let Some(handles) = handles else {
        return;
    };
    let tick = ticks.iter().map(|tick| tick.0).max().unwrap_or_default();
    let cycle_ticks = brawler::timing::SIMULATION_TICK_HZ * VISUAL_CYCLE_SECONDS;
    let visual_tick = u16::try_from(tick % cycle_ticks)
        .expect("the fifteen-minute projectile trail visual cycle fits u16");
    let tick_hz = u16::try_from(brawler::timing::SIMULATION_TICK_HZ)
        .expect("the simulation tick rate fits u16");
    let time = f32::from(visual_tick) / f32::from(tick_hz);
    for handle in [
        &handles.projectile_trail_ally,
        &handles.projectile_trail_enemy,
        &handles.projectile_trail_ally_reduced,
        &handles.projectile_trail_enemy_reduced,
    ] {
        if let Some(mut material) = materials.get_mut(handle) {
            material.params.x = time;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::presentation_3d::{
        ToonMaterial, ToonShading, WorldPresentationSet, configure_world_presentation_schedule,
        setup_3d_foundation,
    };

    fn signature(
        carrier: vfx::VfxCarrier,
        accent: vfx::VfxAccent,
        variant: vfx::VfxPresentationVariant,
    ) -> vfx::CombatVisualSignature {
        vfx::CombatVisualSignature {
            carrier,
            phase: vfx::VfxPhase::Travel,
            accents: [Some(accent), None],
            relationship: vfx::VfxRelationship::Neutral,
            context: None,
            geometry: vfx::VfxGeometry::Projectile { radius: 2.0 },
            identity: vfx::VfxPresentationIdentity::Neutral(1),
            timing: vfx::VfxAuthoritativeTiming::default(),
            variant,
        }
    }

    #[test]
    fn neutral_trail_is_straight_only_and_degrades_to_exact_core() {
        use vfx::VfxPresentationVariant::{Normal, Primitive, Reduced};

        assert_eq!(
            neutral_straight_trail_quality(signature(
                vfx::VfxCarrier::StraightProjectile,
                vfx::VfxAccent::Neutral,
                Normal,
            )),
            Some(TrailQuality::Normal)
        );
        assert_eq!(
            neutral_straight_trail_quality(signature(
                vfx::VfxCarrier::StraightProjectile,
                vfx::VfxAccent::Neutral,
                Reduced,
            )),
            Some(TrailQuality::Reduced)
        );
        assert_eq!(
            neutral_straight_trail_quality(signature(
                vfx::VfxCarrier::StraightProjectile,
                vfx::VfxAccent::Neutral,
                Primitive,
            )),
            None
        );
        assert_eq!(
            neutral_straight_trail_quality(signature(
                vfx::VfxCarrier::LobbedProjectile,
                vfx::VfxAccent::Neutral,
                Normal,
            )),
            None
        );
        assert_eq!(
            neutral_straight_trail_quality(signature(
                vfx::VfxCarrier::StraightProjectile,
                vfx::VfxAccent::Fire,
                Normal,
            )),
            None
        );
    }

    #[test]
    fn trail_materials_encode_quality_and_relation_color() {
        let blue = StraightProjectileTrailMaterial::new(
            LinearRgba::new(0.04, 1.4, 2.6, 1.0),
            TrailQuality::Normal,
        );
        let red = StraightProjectileTrailMaterial::new(
            LinearRgba::new(2.6, 0.07, 0.05, 1.0),
            TrailQuality::Reduced,
        );
        assert!(blue.params.y.abs() < f32::EPSILON);
        assert!((red.params.y - 1.0).abs() < f32::EPSILON);
        assert!(blue.color.z > blue.color.x);
        assert!(red.color.x > red.color.z);
    }

    fn foundation_app() -> App {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<super::super::elemental_style::FireAreaMaterial>>()
            .init_resource::<Assets<super::super::elemental_style::ElementalSupportAreaMaterial>>()
            .init_resource::<Assets<super::super::cone_spray::ConeSprayMaterial>>()
            .init_resource::<Assets<StraightProjectileTrailMaterial>>()
            .init_resource::<brawler::builds::BuildCatalogResource>()
            .init_resource::<Assets<ToonMaterial>>()
            .insert_resource(ToonShading::embedded().unwrap())
            .add_systems(Startup, setup_3d_foundation);
        app.update();
        app
    }

    #[test]
    fn bolt_nucleus_and_wake_use_the_viewer_relative_tint_in_rim_colors() {
        let mut app = foundation_app();
        let signature = signature(
            vfx::VfxCarrier::StraightProjectile,
            vfx::VfxAccent::Neutral,
            vfx::VfxPresentationVariant::Normal,
        );
        let mut roots = Vec::new();
        for tint in [vfx::WeaponTeamTint::Ally, vfx::WeaponTeamTint::Enemy] {
            let world = app.world_mut();
            let materials = world.resource::<Material3dAssets>();
            let (hot, trail) = match tint {
                vfx::WeaponTeamTint::Ally => (
                    materials.projectile_hot_ally.clone(),
                    materials.projectile_trail_ally.clone(),
                ),
                vfx::WeaponTeamTint::Enemy => (
                    materials.projectile_hot_enemy.clone(),
                    materials.projectile_trail_enemy.clone(),
                ),
            };
            let root = world.spawn_empty().id();
            world.resource_scope(|world, materials: Mut<Material3dAssets>| {
                world.resource_scope(|world, primitives: Mut<Primitive3dAssets>| {
                    world.commands().entity(root).with_children(|parent| {
                        assert!(spawn_neutral_straight_projectile_style(
                            parent,
                            &primitives,
                            &materials,
                            signature,
                            Transform::default(),
                            tint,
                            4.0,
                        ));
                    });
                    world.flush();
                });
            });
            roots.push((root, hot, trail));
        }
        let world = app.world_mut();
        for (root, hot, trail) in roots {
            let children = world.get::<Children>(root).unwrap().to_vec();
            assert!(children.iter().any(|child| {
                world
                    .get::<MeshMaterial3d<StandardMaterial>>(*child)
                    .is_some_and(|material| material.0 == hot)
            }));
            assert!(children.iter().any(|child| {
                world
                    .get::<MeshMaterial3d<StraightProjectileTrailMaterial>>(*child)
                    .is_some_and(|material| material.0 == trail)
            }));
        }

        // The hues are the toon rim relation colours shared with rings, nameplates, and rims.
        let shading = ToonShading::embedded().unwrap();
        let materials = app.world().resource::<Material3dAssets>();
        let standard = app.world().resource::<Assets<StandardMaterial>>();
        for (handle, relation) in [
            (
                &materials.weapon_body_ally,
                crate::presentation_3d::RimRelation::Ally,
            ),
            (
                &materials.weapon_body_enemy,
                crate::presentation_3d::RimRelation::Enemy,
            ),
        ] {
            let color = standard.get(handle).unwrap().base_color.to_srgba();
            let [red, green, blue] = shading.rim_rgb(relation);
            assert!((color.red - red).abs() < 1e-4);
            assert!((color.green - green).abs() < 1e-4);
            assert!((color.blue - blue).abs() < 1e-4);
        }
        let ally = standard
            .get(&materials.projectile_hot_ally)
            .unwrap()
            .emissive;
        let enemy = standard
            .get(&materials.projectile_hot_enemy)
            .unwrap()
            .emissive;
        assert!(ally.blue > ally.green && ally.green > ally.red);
        assert!(enemy.red > 10.0 * enemy.green && enemy.red > 10.0 * enemy.blue);
    }

    #[test]
    fn production_retained_material_schedule_advances_trail_time() {
        let mut app = App::new();
        app.init_resource::<Assets<Mesh>>()
            .init_resource::<Assets<StandardMaterial>>()
            .init_resource::<Assets<super::super::elemental_style::FireAreaMaterial>>()
            .init_resource::<Assets<super::super::elemental_style::ElementalSupportAreaMaterial>>()
            .init_resource::<Assets<super::super::cone_spray::ConeSprayMaterial>>()
            .init_resource::<Assets<StraightProjectileTrailMaterial>>()
            .init_resource::<brawler::builds::BuildCatalogResource>()
            .init_resource::<Assets<ToonMaterial>>()
            .insert_resource(ToonShading::embedded().unwrap())
            .add_systems(Startup, setup_3d_foundation);
        configure_world_presentation_schedule(&mut app);
        super::super::add_retained_material_animation_systems(
            &mut app,
            WorldPresentationSet::Animate,
        );
        app.world_mut()
            .spawn(AuthoritativeTick(brawler::timing::SIMULATION_TICK_HZ));

        app.update();

        let handle = app
            .world()
            .resource::<Material3dAssets>()
            .projectile_trail_ally
            .clone();
        let material = app
            .world()
            .resource::<Assets<StraightProjectileTrailMaterial>>()
            .get(&handle)
            .expect("startup prepares the production trail material");
        assert!((material.params.x - 1.0).abs() < f32::EPSILON);
    }
}
