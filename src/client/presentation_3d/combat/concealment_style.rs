use super::super::Primitive3dAssets;
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

const MAX_CONCEALMENT_STYLE_LAYERS: usize = 10;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConcealmentStyleQuality {
    Normal,
    Reduced,
    Primitive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum ConcealmentMotion {
    InnerVeil,
    Wisp,
}

#[derive(Component)]
pub(in crate::client) struct ConcealmentMotionLayer3d {
    motion: ConcealmentMotion,
    radius: f32,
    base_height: f32,
    base_scale: Vec3,
    phase: f32,
    speed: f32,
}

pub(in crate::client) fn spawn_concealment_style(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    fill: &Handle<StandardMaterial>,
    boundary: &Handle<StandardMaterial>,
    radius: f32,
    signature: crate::client::vfx::CombatVisualSignature,
) {
    let quality = quality_for_variant(signature.variant);
    let ground_rotation = Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2);
    parent.spawn((
        Mesh3d(primitives.area_disc.clone()),
        MeshMaterial3d(fill.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        Transform::from_xyz(0.0, super::common::GROUND_EFFECT_HEIGHT, 0.0)
            .with_rotation(ground_rotation)
            .with_scale(Vec3::splat(radius)),
        Name::new("Concealment Field subtle veil floor"),
    ));
    parent.spawn((
        Mesh3d(primitives.area_thin_ring.clone()),
        MeshMaterial3d(boundary.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        Transform::from_xyz(0.0, super::common::GROUND_EFFECT_HEIGHT + 0.6, 0.0)
            .with_rotation(ground_rotation)
            .with_scale(Vec3::splat(radius)),
        Name::new("Concealment Field exact boundary"),
    ));

    if quality == ConcealmentStyleQuality::Primitive {
        return;
    }

    let inner_scale = Vec3::splat(radius * 0.84);
    parent.spawn((
        ConcealmentMotionLayer3d {
            motion: ConcealmentMotion::InnerVeil,
            radius: radius * 0.84,
            base_height: super::common::GROUND_EFFECT_HEIGHT + 0.35,
            base_scale: inner_scale,
            phase: seed_sample(signature.stable_seed()) * core::f32::consts::TAU,
            speed: 0.9,
        },
        Mesh3d(primitives.area_ring.clone()),
        MeshMaterial3d(fill.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        Transform::from_xyz(0.0, super::common::GROUND_EFFECT_HEIGHT + 0.35, 0.0)
            .with_rotation(ground_rotation)
            .with_scale(inner_scale),
        Name::new("Concealment Field breathing inner veil"),
    ));

    let wisp_count = wisp_count(quality);
    let seed = signature.stable_seed();
    for slot in 0..wisp_count {
        let slot_seed = mix_seed(seed, u64::from(slot) + 1);
        let phase = f32::from(slot) * core::f32::consts::TAU / f32::from(wisp_count)
            + seed_sample(slot_seed) * 0.45;
        let wisp_radius = radius * (0.60 + seed_sample(mix_seed(slot_seed, 11)) * 0.26);
        let size =
            (radius * (0.026 + seed_sample(mix_seed(slot_seed, 23)) * 0.018)).clamp(3.0, 7.0);
        let height =
            super::common::GROUND_EFFECT_HEIGHT + 1.8 + seed_sample(mix_seed(slot_seed, 37)) * 4.0;
        let scale = Vec3::new(size * 3.4, 1.0, size * 0.82);
        parent.spawn((
            ConcealmentMotionLayer3d {
                motion: ConcealmentMotion::Wisp,
                radius: wisp_radius,
                base_height: height,
                base_scale: scale,
                phase,
                speed: 0.22 + seed_sample(mix_seed(slot_seed, 47)) * 0.18,
            },
            Mesh3d(primitives.projectile_trail.clone()),
            MeshMaterial3d(if slot.is_multiple_of(3) {
                boundary.clone()
            } else {
                fill.clone()
            }),
            NotShadowCaster,
            NotShadowReceiver,
            Transform::from_xyz(phase.cos() * wisp_radius, height, phase.sin() * wisp_radius)
                .with_rotation(Quat::from_rotation_y(-phase + core::f32::consts::FRAC_PI_2))
                .with_scale(scale),
            Name::new("Concealment Field drifting veil wisp"),
        ));
    }

    debug_assert!(3 + usize::from(wisp_count) <= MAX_CONCEALMENT_STYLE_LAYERS);
}

pub(in crate::client) fn animate_concealment_style(
    ticks: Query<&crate::combat::AuthoritativeTick>,
    mut layers: Query<(&ConcealmentMotionLayer3d, &mut Transform)>,
) {
    let time = authoritative_visual_time(&ticks);
    for (layer, mut transform) in &mut layers {
        let wave = time.mul_add(layer.speed * 2.7, layer.phase).sin();
        match layer.motion {
            ConcealmentMotion::InnerVeil => {
                transform.scale = layer.base_scale * wave.mul_add(0.025, 1.0);
                transform.translation.y = layer.base_height;
            }
            ConcealmentMotion::Wisp => {
                let angle = time.mul_add(layer.speed, layer.phase);
                let drift = 1.0 + (time * 0.31 + layer.phase).sin() * 0.055;
                transform.translation = Vec3::new(
                    angle.cos() * layer.radius * drift,
                    layer.base_height + (wave + 1.0) * 1.8,
                    angle.sin() * layer.radius * drift,
                );
                transform.rotation = Quat::from_rotation_y(-angle + core::f32::consts::FRAC_PI_2);
                transform.scale = layer.base_scale * wave.mul_add(0.16, 0.88);
            }
        }
    }
}

const fn quality_for_variant(
    variant: crate::client::vfx::VfxPresentationVariant,
) -> ConcealmentStyleQuality {
    use crate::client::vfx::VfxPresentationVariant;

    match variant {
        VfxPresentationVariant::Normal => ConcealmentStyleQuality::Normal,
        VfxPresentationVariant::Reduced => ConcealmentStyleQuality::Reduced,
        VfxPresentationVariant::Primitive | VfxPresentationVariant::ReducedPrimitive => {
            ConcealmentStyleQuality::Primitive
        }
    }
}

const fn wisp_count(quality: ConcealmentStyleQuality) -> u8 {
    match quality {
        ConcealmentStyleQuality::Normal => 7,
        ConcealmentStyleQuality::Reduced => 3,
        ConcealmentStyleQuality::Primitive => 0,
    }
}

fn authoritative_visual_time(ticks: &Query<&crate::combat::AuthoritativeTick>) -> f32 {
    let tick = ticks.iter().map(|tick| tick.0).max().unwrap_or_default();
    visual_time_for_tick(tick)
}

fn visual_time_for_tick(tick: u64) -> f32 {
    let bounded_tick = u16::try_from(tick % (crate::timing::SIMULATION_TICK_HZ * 600))
        .expect("the ten-minute concealment visual cycle fits u16");
    let tick_hz = u16::try_from(crate::timing::SIMULATION_TICK_HZ)
        .expect("the simulation tick rate fits u16");
    f32::from(bounded_tick) / f32::from(tick_hz)
}

const fn mix_seed(mut value: u64, salt: u64) -> u64 {
    value ^= salt.wrapping_mul(0x9e37_79b9_7f4a_7c15);
    value ^= value >> 30;
    value = value.wrapping_mul(0xbf58_476d_1ce4_e5b9);
    value ^ (value >> 27)
}

fn seed_sample(seed: u64) -> f32 {
    let bits = u16::try_from(seed & 0xffff).expect("masked concealment seed fits u16");
    f32::from(bits) / f32::from(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concealment_style_degrades_with_bounded_layers() {
        assert_eq!(
            quality_for_variant(crate::client::vfx::VfxPresentationVariant::Normal),
            ConcealmentStyleQuality::Normal
        );
        assert_eq!(
            quality_for_variant(crate::client::vfx::VfxPresentationVariant::Reduced),
            ConcealmentStyleQuality::Reduced
        );
        assert_eq!(
            quality_for_variant(crate::client::vfx::VfxPresentationVariant::Primitive),
            ConcealmentStyleQuality::Primitive
        );
        assert_eq!(wisp_count(ConcealmentStyleQuality::Normal), 7);
        assert_eq!(wisp_count(ConcealmentStyleQuality::Reduced), 3);
        assert_eq!(wisp_count(ConcealmentStyleQuality::Primitive), 0);
        assert!(3 + usize::from(wisp_count(ConcealmentStyleQuality::Normal)) <= 10);
    }

    #[test]
    fn concealment_motion_time_is_authoritative_and_repeatable() {
        assert!((visual_time_for_tick(30) - 0.5).abs() < f32::EPSILON);
        assert!(visual_time_for_tick(crate::timing::SIMULATION_TICK_HZ * 600).abs() < f32::EPSILON);
    }

    #[test]
    fn concealment_motion_replays_identically_at_the_same_tick() {
        let mut app = App::new();
        app.add_systems(Update, animate_concealment_style);
        let clock = app
            .world_mut()
            .spawn(crate::combat::AuthoritativeTick(30))
            .id();
        let base_scale = Vec3::new(12.0, 1.0, 4.0);
        let layer = app
            .world_mut()
            .spawn((
                ConcealmentMotionLayer3d {
                    motion: ConcealmentMotion::Wisp,
                    radius: 80.0,
                    base_height: 4.0,
                    base_scale,
                    phase: 0.6,
                    speed: 0.3,
                },
                Transform::from_scale(base_scale),
            ))
            .id();

        app.update();
        let first = *app.world().get::<Transform>(layer).unwrap();
        app.world_mut()
            .get_mut::<crate::combat::AuthoritativeTick>(clock)
            .unwrap()
            .0 = 90;
        app.update();
        assert_ne!(*app.world().get::<Transform>(layer).unwrap(), first);

        app.world_mut()
            .get_mut::<crate::combat::AuthoritativeTick>(clock)
            .unwrap()
            .0 = 30;
        app.update();
        assert_eq!(*app.world().get::<Transform>(layer).unwrap(), first);
    }
}
