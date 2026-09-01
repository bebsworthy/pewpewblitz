//! Retained dual-purpose fluid styling for mixed restorative and damaging Splash areas.

use super::super::*;
use bevy::{
    ecs::hierarchy::ChildSpawnerCommands, reflect::TypePath, render::render_resource::AsBindGroup,
    shader::ShaderRef,
};

const SPLASH_AREA_SHADER_PATH: &str = "shaders/splash_area.wgsl";

#[derive(Asset, TypePath, AsBindGroup, Debug, Clone)]
pub(in crate::client) struct SplashAreaMaterial {
    #[uniform(0)]
    params: Vec4,
}

impl SplashAreaMaterial {
    pub(in crate::client) const fn normal() -> Self {
        Self {
            params: Vec4::new(0.0, 0.0, 0.0, 0.0),
        }
    }

    pub(in crate::client) const fn reduced() -> Self {
        Self {
            params: Vec4::new(0.0, 1.0, 0.0, 0.0),
        }
    }
}

impl Material for SplashAreaMaterial {
    fn fragment_shader() -> ShaderRef {
        SPLASH_AREA_SHADER_PATH.into()
    }

    fn alpha_mode(&self) -> AlphaMode {
        AlphaMode::Blend
    }

    fn depth_bias(&self) -> f32 {
        12.0
    }

    fn enable_shadows() -> bool {
        false
    }
}

pub(in crate::client) struct SplashAreaMaterialPlugin;

impl Plugin for SplashAreaMaterialPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(MaterialPlugin::<SplashAreaMaterial>::default());
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SplashAreaQuality {
    Normal,
    Reduced,
    Primitive,
}

#[derive(Component)]
pub(in crate::client::presentation_3d) struct SplashMotionAccent3d {
    seed: u64,
    base_translation: Vec3,
    base_scale: Vec3,
}

fn dual_purpose_quality(signature: vfx::CombatVisualSignature) -> Option<SplashAreaQuality> {
    if signature.carrier != vfx::VfxCarrier::PersistentArea
        || !signature
            .accents
            .contains(&Some(vfx::VfxAccent::Restoration))
        || !signature.accents.contains(&Some(vfx::VfxAccent::Neutral))
    {
        return None;
    }
    Some(match signature.variant {
        vfx::VfxPresentationVariant::Normal => SplashAreaQuality::Normal,
        vfx::VfxPresentationVariant::Reduced => SplashAreaQuality::Reduced,
        vfx::VfxPresentationVariant::Primitive | vfx::VfxPresentationVariant::ReducedPrimitive => {
            SplashAreaQuality::Primitive
        }
    })
}

pub(super) fn spawn_dual_purpose_splash_style(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    signature: vfx::CombatVisualSignature,
    radius: f32,
) -> bool {
    let Some(quality) = dual_purpose_quality(signature) else {
        return false;
    };
    let ground_rotation = Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2);
    let fill_material = match quality {
        SplashAreaQuality::Normal => materials.splash_area.clone(),
        SplashAreaQuality::Reduced => materials.splash_area_reduced.clone(),
        SplashAreaQuality::Primitive => {
            parent.spawn((
                Mesh3d(primitives.area_disc.clone()),
                MeshMaterial3d(materials.elemental_restoration.clone()),
                NotShadowCaster,
                NotShadowReceiver,
                Transform::from_xyz(0.0, super::common::GROUND_EFFECT_HEIGHT + 0.18, 0.0)
                    .with_rotation(ground_rotation)
                    .with_scale(Vec3::splat(radius)),
                Name::new("Dual-purpose Splash primitive fluid fill"),
            ));
            spawn_splash_boundaries(parent, primitives, materials, radius, ground_rotation);
            return true;
        }
    };
    parent.spawn((
        Mesh3d(primitives.area_disc.clone()),
        MeshMaterial3d(fill_material),
        NotShadowCaster,
        NotShadowReceiver,
        Transform::from_xyz(0.0, super::common::GROUND_EFFECT_HEIGHT + 0.24, 0.0)
            .with_rotation(ground_rotation)
            .with_scale(Vec3::splat(radius)),
        Name::new("Dual-purpose Splash animated fluid surface"),
    ));
    spawn_splash_boundaries(parent, primitives, materials, radius, ground_rotation);
    spawn_splash_droplets(parent, primitives, materials, signature, radius, quality);
    true
}

fn spawn_splash_boundaries(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    radius: f32,
    ground_rotation: Quat,
) {
    parent.spawn((
        Mesh3d(primitives.area_thin_ring.clone()),
        MeshMaterial3d(materials.neutral_energy_bright.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        Transform::from_xyz(0.0, super::common::GROUND_EFFECT_HEIGHT + 0.68, 0.0)
            .with_rotation(ground_rotation)
            .with_scale(Vec3::splat(radius)),
        Name::new("Dual-purpose Splash exact cyan boundary"),
    ));
    parent.spawn((
        Mesh3d(primitives.area_ring.clone()),
        MeshMaterial3d(materials.elemental_restoration_bright.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        Transform::from_xyz(0.0, super::common::GROUND_EFFECT_HEIGHT + 0.52, 0.0)
            .with_rotation(ground_rotation)
            .with_scale(Vec3::splat(radius * 0.91)),
        Name::new("Dual-purpose Splash restorative inner boundary"),
    ));
}

fn spawn_splash_droplets(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    signature: vfx::CombatVisualSignature,
    radius: f32,
    quality: SplashAreaQuality,
) {
    let count: u8 = match quality {
        SplashAreaQuality::Normal => 7,
        SplashAreaQuality::Reduced => 3,
        SplashAreaQuality::Primitive => 0,
    };
    let seed = signature.stable_seed();
    let offset = unit_sample(mix_seed(seed, 31)) * core::f32::consts::TAU;
    for slot in 0..count {
        let slot_seed = mix_seed(seed, u64::from(slot));
        let angle = offset + f32::from(slot) * core::f32::consts::TAU / f32::from(count);
        let radial = radius * (0.72 + unit_sample(mix_seed(slot_seed, 7)) * 0.15);
        let size = radius * (0.026 + unit_sample(mix_seed(slot_seed, 13)) * 0.024);
        let translation = Vec3::new(angle.cos() * radial, 2.4 + size * 0.4, angle.sin() * radial);
        let scale = Vec3::new(size * 1.35, size * 0.62, size);
        parent.spawn((
            SplashMotionAccent3d {
                seed: slot_seed,
                base_translation: translation,
                base_scale: scale,
            },
            Mesh3d(primitives.effect_sphere.clone()),
            MeshMaterial3d(if slot.is_multiple_of(2) {
                materials.neutral_energy_bright.clone()
            } else {
                materials.elemental_restoration_bright.clone()
            }),
            NotShadowCaster,
            NotShadowReceiver,
            Transform::from_translation(translation).with_scale(scale),
            Name::new("Dual-purpose Splash fluid droplet"),
        ));
    }
}

pub(in crate::client) fn animate_splash_area_materials(
    ticks: Query<&AuthoritativeTick>,
    handles: Option<Res<Material3dAssets>>,
    materials: Option<ResMut<Assets<SplashAreaMaterial>>>,
) {
    let (Some(handles), Some(mut materials)) = (handles, materials) else {
        return;
    };
    let time = bounded_visual_time(&ticks);
    for handle in [&handles.splash_area, &handles.splash_area_reduced] {
        if let Some(mut material) = materials.get_mut(handle) {
            material.params.x = time;
        }
    }
}

pub(in crate::client) fn animate_splash_motion_accents(
    ticks: Query<&AuthoritativeTick>,
    mut accents: Query<(&SplashMotionAccent3d, &mut Transform)>,
) {
    let time = bounded_visual_time(&ticks);
    for (accent, mut transform) in &mut accents {
        let phase = unit_sample(accent.seed) * core::f32::consts::TAU;
        let bob = (time * 2.7 + phase).sin();
        let pulse = 1.0 + (time * 3.4 + phase * 0.7).sin() * 0.11;
        transform.translation = accent.base_translation + Vec3::Y * (bob * 1.8);
        transform.scale = accent.base_scale * pulse;
    }
}

fn bounded_visual_time(ticks: &Query<&AuthoritativeTick>) -> f32 {
    const VISUAL_CYCLE_SECONDS: u64 = 900;
    let tick = ticks.iter().map(|tick| tick.0).max().unwrap_or_default();
    let cycle_ticks = crate::timing::SIMULATION_TICK_HZ * VISUAL_CYCLE_SECONDS;
    let visual_tick =
        u16::try_from(tick % cycle_ticks).expect("the fifteen-minute Splash visual cycle fits u16");
    let tick_hz = u16::try_from(crate::timing::SIMULATION_TICK_HZ)
        .expect("the simulation tick rate fits u16");
    f32::from(visual_tick) / f32::from(tick_hz)
}

const fn mix_seed(seed: u64, salt: u64) -> u64 {
    seed ^ salt.wrapping_mul(0x9e37_79b9_7f4a_7c15)
}

fn unit_sample(seed: u64) -> f32 {
    let folded = u16::try_from((seed ^ (seed >> 32)) & u64::from(u16::MAX))
        .expect("masked Splash seed fits u16");
    f32::from(folded) / f32::from(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signature(
        accents: [Option<vfx::VfxAccent>; 2],
        variant: vfx::VfxPresentationVariant,
    ) -> vfx::CombatVisualSignature {
        vfx::CombatVisualSignature {
            carrier: vfx::VfxCarrier::PersistentArea,
            phase: vfx::VfxPhase::Active,
            accents,
            relationship: vfx::VfxRelationship::Neutral,
            context: None,
            geometry: vfx::VfxGeometry::circle(Vec2::ZERO, 96.0),
            identity: vfx::VfxPresentationIdentity::Neutral(1),
            timing: vfx::VfxAuthoritativeTiming::default(),
            variant,
        }
    }

    #[test]
    fn dual_purpose_style_is_semantic_and_variant_bounded() {
        use vfx::VfxPresentationVariant::{Normal, Primitive, Reduced};
        let dual = [
            Some(vfx::VfxAccent::Restoration),
            Some(vfx::VfxAccent::Neutral),
        ];
        assert_eq!(
            dual_purpose_quality(signature(dual, Normal)),
            Some(SplashAreaQuality::Normal)
        );
        assert_eq!(
            dual_purpose_quality(signature(dual, Reduced)),
            Some(SplashAreaQuality::Reduced)
        );
        assert_eq!(
            dual_purpose_quality(signature(dual, Primitive)),
            Some(SplashAreaQuality::Primitive)
        );
        assert_eq!(
            dual_purpose_quality(signature([Some(vfx::VfxAccent::Restoration), None], Normal,)),
            None
        );
    }

    #[test]
    fn splash_materials_encode_normal_and_reduced_shader_paths() {
        assert!(SplashAreaMaterial::normal().params.y < 0.5);
        assert!(SplashAreaMaterial::reduced().params.y > 0.5);
    }
}
