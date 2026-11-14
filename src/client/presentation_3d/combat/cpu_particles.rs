//! Non-compute particle fallback (BRL-0161) behind the same recipe particle-layer boundary.
//!
//! A burst layer becomes a few seeded mesh particles under the effect root, with the layer's
//! mesh, profile color, acceleration, lifetime, and size curve, simulated on the CPU. It runs
//! when the GPU cannot run Hanabi's compute particles or the quality tier selects it. The
//! particles despawn with their effect root, so eviction and the emitter budget still bound them.

use super::super::Primitive3dAssets;
use super::particles::{particle_acceleration, particle_color_gradient, particle_size_endpoints};
use crate::client::vfx::{VfxParticleLayer, VfxParticleProfile};
use bevy::{ecs::hierarchy::ChildSpawnerCommands, prelude::*};

/// Code-owned ceiling on mesh particles one fallback burst spawns (authored bursts are larger;
/// the fallback keeps their silhouette, not their density).
pub(in crate::client) const MAX_CPU_PARTICLES_PER_BURST: u16 = 10;
/// Point on the color-over-lifetime gradient the fallback's single flat color samples.
const FALLBACK_COLOR_SAMPLE: f32 = 0.25;
const PROFILE_COUNT: usize = 8;

const fn profile_index(profile: VfxParticleProfile) -> usize {
    match profile {
        VfxParticleProfile::NeutralSparks => 0,
        VfxParticleProfile::ColdFragments => 1,
        VfxParticleProfile::FireEmbers => 2,
        VfxParticleProfile::PoisonBubbles => 3,
        VfxParticleProfile::RestorationPetals => 4,
        VfxParticleProfile::ExplosionSparks => 5,
        VfxParticleProfile::ExplosionSmoke => 6,
        VfxParticleProfile::ProjectileSmoke => 7,
    }
}

const ALL_PROFILES: [VfxParticleProfile; PROFILE_COUNT] = [
    VfxParticleProfile::NeutralSparks,
    VfxParticleProfile::ColdFragments,
    VfxParticleProfile::FireEmbers,
    VfxParticleProfile::PoisonBubbles,
    VfxParticleProfile::RestorationPetals,
    VfxParticleProfile::ExplosionSparks,
    VfxParticleProfile::ExplosionSmoke,
    VfxParticleProfile::ProjectileSmoke,
];

/// Additive profiles glow like their Hanabi counterparts; smoke, bubbles, and petals blend.
pub(super) const fn particle_is_additive(profile: VfxParticleProfile) -> bool {
    !matches!(
        profile,
        VfxParticleProfile::PoisonBubbles
            | VfxParticleProfile::RestorationPetals
            | VfxParticleProfile::ExplosionSmoke
            | VfxParticleProfile::ProjectileSmoke
    )
}

/// Sparks and fragments stretch along their velocity; the rest keep their mesh orientation.
pub(super) const fn particle_aligns_to_velocity(profile: VfxParticleProfile) -> bool {
    matches!(
        profile,
        VfxParticleProfile::NeutralSparks
            | VfxParticleProfile::ColdFragments
            | VfxParticleProfile::ExplosionSparks
    )
}

/// One flat unlit material per particle profile, shared by every fallback particle.
#[derive(Clone)]
pub(super) struct CpuParticleMaterials([Handle<StandardMaterial>; PROFILE_COUNT]);

impl CpuParticleMaterials {
    pub(super) fn new(materials: &mut Assets<StandardMaterial>) -> Self {
        Self(ALL_PROFILES.map(|profile| {
            let color = particle_color_gradient(profile).sample(FALLBACK_COLOR_SAMPLE);
            materials.add(StandardMaterial {
                base_color: Color::LinearRgba(LinearRgba::new(color.x, color.y, color.z, color.w)),
                alpha_mode: if particle_is_additive(profile) {
                    AlphaMode::Add
                } else {
                    AlphaMode::Blend
                },
                unlit: true,
                ..default()
            })
        }))
    }

    pub(super) fn get(&self, profile: VfxParticleProfile) -> Handle<StandardMaterial> {
        self.0[profile_index(profile)].clone()
    }
}

/// One simulated fallback particle, positioned relative to its stationary emitter.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(in crate::client) struct CpuParticle3d {
    velocity: Vec3,
    acceleration: Vec3,
    age: f32,
    lifetime: f32,
    size_start: Vec3,
    size_end: Vec3,
    align_to_velocity: bool,
}

impl CpuParticle3d {
    fn scale(&self) -> Vec3 {
        let progress = (self.age / self.lifetime).clamp(0.0, 1.0);
        self.size_start.lerp(self.size_end, progress)
    }

    fn rotation(&self) -> Quat {
        if self.align_to_velocity && self.velocity.length_squared() > f32::EPSILON {
            Quat::from_rotation_arc(Vec3::X, self.velocity.normalize())
        } else {
            Quat::IDENTITY
        }
    }
}

/// Deterministic per-burst random stream (`SplitMix64`), so a burst replays identically.
struct ParticleRandom(u64);

impl ParticleRandom {
    fn next_unit(&mut self) -> f32 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut value = self.0;
        value = (value ^ (value >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        value = (value ^ (value >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        value ^= value >> 31;
        // The top 24 bits are exactly representable in an f32 mantissa.
        #[allow(
            clippy::cast_precision_loss,
            reason = "a 24-bit integer is exact in f32"
        )]
        let unit = (value >> 40) as f32 / (1_u32 << 24) as f32;
        unit
    }
}

/// The particles one fallback burst starts with: Hanabi's spawn sphere and velocity expression.
pub(super) fn burst_particles(
    layer: &VfxParticleLayer,
    burst_count: u16,
    seed: u32,
) -> Vec<(Vec3, CpuParticle3d)> {
    let mut random = ParticleRandom(u64::from(seed));
    let (size_start, size_end) = particle_size_endpoints(layer);
    let lifetime = f32::from(layer.lifetime_millis) / 1_000.0;
    (0..burst_count.min(MAX_CPU_PARTICLES_PER_BURST))
        .map(|_| {
            let direction = Vec3::new(
                random.next_unit() * 2.0 - 1.0,
                random.next_unit() * 2.0 - 1.0,
                random.next_unit() * 2.0 - 1.0,
            )
            .normalize_or(Vec3::Y);
            let offset = direction * layer.spawn_radius * random.next_unit().cbrt();
            let launch_speed =
                random.next_unit() * (layer.speed_max - layer.speed_min) + layer.speed_min;
            let angle = random.next_unit() * core::f32::consts::TAU;
            let horizontal = launch_speed * 0.72;
            let particle = CpuParticle3d {
                velocity: Vec3::new(
                    angle.cos() * horizontal,
                    launch_speed * 0.68 + 8.0,
                    angle.sin() * horizontal,
                ),
                acceleration: particle_acceleration(layer.profile),
                age: 0.0,
                lifetime,
                size_start,
                size_end,
                align_to_velocity: particle_aligns_to_velocity(layer.profile),
            };
            (offset, particle)
        })
        .collect()
}

/// Spawns one fallback burst's particles under an emitter.
pub(super) fn spawn_burst_children(
    emitter: &mut ChildSpawnerCommands<'_>,
    particles: Vec<(Vec3, CpuParticle3d)>,
    mesh: &Handle<Mesh>,
    material: &Handle<StandardMaterial>,
) {
    for (offset, particle) in particles {
        emitter.spawn((
            particle,
            Mesh3d(mesh.clone()),
            MeshMaterial3d(material.clone()),
            bevy::light::NotShadowCaster,
            bevy::light::NotShadowReceiver,
            Transform {
                translation: offset,
                rotation: particle.rotation(),
                scale: particle.scale(),
            },
        ));
    }
}

/// The mesh a profile's particles use in both backends.
pub(super) fn particle_mesh(
    profile: VfxParticleProfile,
    primitives: &Primitive3dAssets,
) -> Handle<Mesh> {
    match profile {
        VfxParticleProfile::ColdFragments => primitives.unit_cuboid.clone(),
        VfxParticleProfile::NeutralSparks
        | VfxParticleProfile::FireEmbers
        | VfxParticleProfile::ExplosionSparks => primitives.fire_tongue.clone(),
        VfxParticleProfile::PoisonBubbles => primitives.effect_sphere.clone(),
        VfxParticleProfile::RestorationPetals => primitives.vitality_spark.clone(),
        VfxParticleProfile::ExplosionSmoke | VfxParticleProfile::ProjectileSmoke => {
            primitives.effect_sphere.clone()
        }
    }
}

/// Integrates fallback particles; an expired particle hides until its effect root despawns.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Time is a Bevy system resource parameter"
)]
pub(super) fn simulate_cpu_particles(
    time: Res<Time>,
    mut particles: Query<(&mut CpuParticle3d, &mut Transform, &mut Visibility)>,
) {
    let delta = time.delta_secs();
    for (mut particle, mut transform, mut visibility) in &mut particles {
        if particle.age >= particle.lifetime {
            visibility.set_if_neq(Visibility::Hidden);
            continue;
        }
        particle.age += delta;
        let acceleration = particle.acceleration;
        particle.velocity += acceleration * delta;
        transform.translation += particle.velocity * delta;
        transform.rotation = particle.rotation();
        transform.scale = particle.scale();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::vfx::VfxParticleSpawn;
    use std::time::Duration;

    fn layer(profile: VfxParticleProfile, burst: u16) -> VfxParticleLayer {
        VfxParticleLayer {
            id: "test_particles".to_string(),
            profile,
            capacity: 64,
            spawn: VfxParticleSpawn::Burst(burst),
            lifetime_millis: 500,
            spawn_radius: 4.0,
            speed_min: 12.0,
            speed_max: 30.0,
            size_start: 3.0,
            size_end: 0.0,
            anchor_height: 5.0,
        }
    }

    #[test]
    fn bursts_are_bounded_seeded_and_inside_the_spawn_sphere() {
        let layer = layer(VfxParticleProfile::FireEmbers, 48);
        let first = burst_particles(&layer, 48, 7);
        assert_eq!(first.len(), usize::from(MAX_CPU_PARTICLES_PER_BURST));
        assert_eq!(first, burst_particles(&layer, 48, 7));
        assert_ne!(first, burst_particles(&layer, 48, 8));
        assert_eq!(burst_particles(&layer, 3, 7).len(), 3);
        for (offset, particle) in &first {
            assert!(offset.length() <= layer.spawn_radius + 1.0e-4);
            assert!(particle.velocity.y >= 8.0);
            let horizontal = particle.velocity.xz().length();
            assert!(horizontal <= layer.speed_max * 0.72 + 1.0e-3);
        }
    }

    #[test]
    fn particles_fall_shrink_and_hide_when_expired() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .add_systems(Update, simulate_cpu_particles);
        let layer = layer(VfxParticleProfile::ExplosionSparks, 1);
        let (offset, particle) = burst_particles(&layer, 1, 3)[0];
        let entity = app
            .world_mut()
            .spawn((
                particle,
                Transform::from_translation(offset),
                Visibility::Inherited,
            ))
            .id();
        let start_scale = particle.scale();
        for _ in 0..5 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(80));
            app.update();
        }
        let simulated = *app.world().get::<CpuParticle3d>(entity).unwrap();
        assert!(
            simulated.velocity.y < particle.velocity.y,
            "gravity pulls sparks down"
        );
        let transform = app.world().get::<Transform>(entity).unwrap();
        assert!(transform.scale.x < start_scale.x);
        assert_eq!(
            app.world().get::<Visibility>(entity),
            Some(&Visibility::Inherited)
        );
        for _ in 0..3 {
            app.world_mut()
                .resource_mut::<Time>()
                .advance_by(Duration::from_millis(80));
            app.update();
        }
        assert_eq!(
            app.world().get::<Visibility>(entity),
            Some(&Visibility::Hidden)
        );
    }
}
