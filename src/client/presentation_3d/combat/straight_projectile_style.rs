//! Retained neutral straight-bolt styling shared by single and spread weapon recipes.

use super::super::*;
use bevy::{
    ecs::hierarchy::ChildSpawnerCommands, reflect::TypePath, render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

const STRAIGHT_PROJECTILE_TRAIL_SHADER_PATH: &str = "shaders/straight_projectile_trail.wgsl";

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(in crate::client) struct StraightProjectileTrailMaterial {
    #[uniform(0)]
    params: Vec4,
    #[uniform(1)]
    color: Vec4,
}

impl StraightProjectileTrailMaterial {
    pub(in crate::client) const fn blue(quality: TrailQuality) -> Self {
        Self {
            params: Vec4::new(0.0, quality.shader_index(), 0.0, 0.0),
            color: Vec4::new(0.055, 1.35, 2.45, 1.0),
        }
    }

    pub(in crate::client) const fn red(quality: TrailQuality) -> Self {
        Self {
            params: Vec4::new(0.0, quality.shader_index(), 0.0, 0.0),
            color: Vec4::new(2.8, 0.34, 0.025, 1.0),
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

pub(in crate::client) struct StraightProjectileTrailMaterialPlugin;

impl Plugin for StraightProjectileTrailMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<StraightProjectileTrailMaterial>::default());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::client) enum TrailQuality {
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
    team: crate::combat::TeamId,
    visual_radius: f32,
) -> bool {
    let Some(quality) = neutral_straight_trail_quality(signature) else {
        return false;
    };

    let nucleus_transform = Transform::from_xyz(0.0, core_transform.scale.y * 0.48, 0.0)
        .with_scale(Vec3::new(visual_radius, visual_radius, visual_radius));
    parent.spawn((
        Mesh3d(primitives.effect_sphere.clone()),
        MeshMaterial3d(if team.0 == 1 {
            materials.projectile_hot_red.clone()
        } else {
            materials.projectile_hot_blue.clone()
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
        MeshMaterial3d(projectile_trail_material(team, quality, materials)),
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
    team: crate::combat::TeamId,
    quality: TrailQuality,
    materials: &Material3dAssets,
) -> Handle<StraightProjectileTrailMaterial> {
    match (team.0 == 1, quality) {
        (false, TrailQuality::Normal) => materials.projectile_trail_blue.clone(),
        (true, TrailQuality::Normal) => materials.projectile_trail_red.clone(),
        (false, TrailQuality::Reduced) => materials.projectile_trail_blue_reduced.clone(),
        (true, TrailQuality::Reduced) => materials.projectile_trail_red_reduced.clone(),
    }
}

pub(in crate::client) fn animate_straight_projectile_trail_materials(
    ticks: Query<&AuthoritativeTick>,
    handles: Option<Res<Material3dAssets>>,
    mut materials: ResMut<Assets<StraightProjectileTrailMaterial>>,
) {
    const VISUAL_CYCLE_SECONDS: u64 = 900;

    let Some(handles) = handles else {
        return;
    };
    let tick = ticks.iter().map(|tick| tick.0).max().unwrap_or_default();
    let cycle_ticks = crate::timing::SIMULATION_TICK_HZ * VISUAL_CYCLE_SECONDS;
    let visual_tick = u16::try_from(tick % cycle_ticks)
        .expect("the fifteen-minute projectile trail visual cycle fits u16");
    let tick_hz = u16::try_from(crate::timing::SIMULATION_TICK_HZ)
        .expect("the simulation tick rate fits u16");
    let time = f32::from(visual_tick) / f32::from(tick_hz);
    for handle in [
        &handles.projectile_trail_blue,
        &handles.projectile_trail_red,
        &handles.projectile_trail_blue_reduced,
        &handles.projectile_trail_red_reduced,
    ] {
        if let Some(mut material) = materials.get_mut(handle) {
            material.params.x = time;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn authored_trail_materials_encode_quality_and_team_color() {
        let blue = StraightProjectileTrailMaterial::blue(TrailQuality::Normal);
        let red = StraightProjectileTrailMaterial::red(TrailQuality::Reduced);
        assert!(blue.params.y.abs() < f32::EPSILON);
        assert!((red.params.y - 1.0).abs() < f32::EPSILON);
        assert!(blue.color.z > blue.color.x);
        assert!(red.color.x > red.color.z);
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
            .init_resource::<crate::builds::BuildCatalogResource>()
            .add_systems(Startup, setup_3d_foundation);
        configure_world_presentation_schedule(&mut app);
        super::super::add_retained_material_animation_systems(
            &mut app,
            WorldPresentationSet::Animate,
        );
        app.world_mut()
            .spawn(AuthoritativeTick(crate::timing::SIMULATION_TICK_HZ));

        app.update();

        let handle = app
            .world()
            .resource::<Material3dAssets>()
            .projectile_trail_blue
            .clone();
        let material = app
            .world()
            .resource::<Assets<StraightProjectileTrailMaterial>>()
            .get(&handle)
            .expect("startup prepares the production trail material");
        assert!((material.params.x - 1.0).abs() < f32::EPSILON);
    }
}
