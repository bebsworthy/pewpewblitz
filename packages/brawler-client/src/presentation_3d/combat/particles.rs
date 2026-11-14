//! Bounded Hanabi adapter for optional decorative VFX recipe layers.
//!
//! Hanabi needs compute shaders and storage buffers, so it is installed only in `finish`, after
//! the render device exists and [`RenderCapabilities`] says the GPU can run it (BRL-0161). Without
//! it, or when the quality tier selects the CPU backend, burst layers use the mesh-particle
//! fallback in `cpu_particles` instead.

use super::super::render_quality::{ParticleBackend, RenderCapabilities};
use super::super::{Primitive3dAssets, ground_position};
use super::cpu_particles::{
    CpuParticleMaterials, burst_particles, particle_aligns_to_velocity, particle_is_additive,
    particle_mesh, simulate_cpu_particles, spawn_burst_children,
};
use crate::vfx::{
    VfxParticleLayer, VfxParticleProfile, VfxParticleSpawn, VfxRegistry, vfx_counter_sample,
};
use bevy::{ecs::hierarchy::ChildSpawnerCommands, prelude::*};
use bevy_hanabi::{
    AlphaMode as HanabiAlphaMode, CompiledParticleEffect, EffectSpawner, EffectSystems,
    Gradient as HanabiGradient,
    prelude::{
        AccelModifier, Attribute, ColorOverLifetimeModifier, EffectAsset, EffectMesh, ExprWriter,
        HanabiPlugin, OrientMode, OrientModifier, ParticleEffect, ScalarType, SetAttributeModifier,
        SetPositionSphereModifier, ShapeDimension, SimulationSpace, SizeOverLifetimeModifier,
        SpawnerSettings,
    },
};
use std::collections::BTreeMap;

pub(crate) const MAX_ACTIVE_PARTICLE_EMITTERS: usize = 96;

#[derive(Resource, Default, Clone, Copy)]
pub(crate) struct VfxParticleDiagnostics {
    pub(crate) active: usize,
    pub(crate) high_water: usize,
    pub(crate) suppressed: u64,
}

#[derive(Resource)]
pub(crate) struct VfxParticleAssets {
    /// Compiled Hanabi effects; empty when Hanabi is not installed.
    effects: BTreeMap<String, Handle<EffectAsset>>,
    layers: BTreeMap<String, VfxParticleLayer>,
    cpu_materials: CpuParticleMaterials,
}

#[derive(Component)]
pub(crate) struct VfxParticleEmitter3d {
    pub(crate) root: Entity,
    pub(crate) layer_id: String,
    pub(crate) profile: VfxParticleProfile,
    pub(crate) capacity: u16,
    pub(crate) spawn: VfxParticleSpawn,
    pub(crate) seed: u32,
}

#[derive(Component)]
pub(crate) struct RetainedProjectileSmoke3d {
    owner: Entity,
    linger_seconds: f32,
    remaining_seconds: f32,
}

/// Holds a one-shot burst until Hanabi has reported a stable ready pipeline.
///
/// Hanabi 0.19 can report the compiled effect ready one main-world frame before
/// its render-world allocation can consume a burst on a cold pipeline. Waiting
/// for the second ready observation keeps that first burst from disappearing.
#[derive(Component, Default)]
struct PendingParticleBurst {
    ready_observations: u8,
}

impl PendingParticleBurst {
    fn observe_ready(&mut self, ready: bool) -> bool {
        if !ready {
            self.ready_observations = 0;
            return false;
        }
        self.ready_observations = self.ready_observations.saturating_add(1);
        self.ready_observations >= 2
    }
}

pub(crate) struct PendingVfxParticleLayer {
    backend: PendingParticleBackend,
    mesh: Option<Handle<Mesh>>,
    transform: Transform,
    layer_id: String,
    profile: VfxParticleProfile,
    capacity: u16,
    burst_count: u16,
    seed: u32,
}

enum PendingParticleBackend {
    Gpu(Handle<EffectAsset>),
    /// The fallback's starting particles and their shared material.
    Cpu {
        particles: Vec<(Vec3, super::cpu_particles::CpuParticle3d)>,
        material: Handle<StandardMaterial>,
    },
}

impl PendingVfxParticleLayer {
    pub(crate) fn spawn(&self, parent: &mut ChildSpawnerCommands<'_>, root: Entity) {
        let emitter = VfxParticleEmitter3d {
            root,
            layer_id: self.layer_id.clone(),
            profile: self.profile,
            capacity: self.capacity,
            spawn: VfxParticleSpawn::Burst(self.burst_count),
            seed: self.seed,
        };
        let handle = match &self.backend {
            PendingParticleBackend::Gpu(handle) => handle.clone(),
            PendingParticleBackend::Cpu {
                particles,
                material,
            } => {
                let mesh = self.mesh.clone().unwrap_or_default();
                parent
                    .spawn((
                        emitter,
                        self.transform,
                        Visibility::default(),
                        Name::new(format!("VFX fallback particles / {}", self.layer_id)),
                    ))
                    .with_children(|burst| {
                        spawn_burst_children(burst, particles.clone(), &mesh, material);
                    });
                return;
            }
        };
        let mut particle_effect = ParticleEffect::new(handle);
        particle_effect.prng_seed = Some(self.seed);
        let settings = particle_spawner_settings(self.burst_count);
        let mut entity = parent.spawn((
            emitter,
            particle_effect,
            EffectSpawner::new(&settings),
            PendingParticleBurst::default(),
            self.transform,
            Name::new(format!("VFX particle / {}", self.layer_id)),
        ));
        if let Some(mesh) = self.mesh.as_ref() {
            entity.insert(EffectMesh(mesh.clone()));
        }
    }

    /// A fallback burst with the authored test layer's starting particles.
    #[cfg(test)]
    pub(crate) fn test_cpu_layer(id: &str, burst_count: u16) -> Self {
        let mut layer = Self::test_layer(id);
        let source = VfxParticleLayer {
            id: id.to_string(),
            profile: layer.profile,
            capacity: layer.capacity,
            spawn: VfxParticleSpawn::Burst(burst_count),
            lifetime_millis: 400,
            spawn_radius: 4.0,
            speed_min: 10.0,
            speed_max: 20.0,
            size_start: 3.0,
            size_end: 0.0,
            anchor_height: 5.0,
        };
        layer.burst_count = burst_count;
        layer.backend = PendingParticleBackend::Cpu {
            particles: burst_particles(&source, burst_count, layer.seed),
            material: Handle::default(),
        };
        layer
    }

    #[cfg(test)]
    pub(crate) fn test_layer(id: &str) -> Self {
        Self {
            backend: PendingParticleBackend::Gpu(Handle::default()),
            mesh: None,
            transform: Transform::default(),
            layer_id: id.to_string(),
            profile: VfxParticleProfile::FireEmbers,
            capacity: 16,
            burst_count: 8,
            seed: 7,
        }
    }
}

impl VfxParticleAssets {
    #[allow(
        clippy::too_many_arguments,
        reason = "one burst layer resolves against its recipe identity, position, and backend"
    )]
    pub(crate) fn pending(
        &self,
        layer: &VfxParticleLayer,
        primitives: &Primitive3dAssets,
        position: Vec2,
        recipe_id: &str,
        presentation_identity: u64,
        occurrence: u8,
        backend: ParticleBackend,
    ) -> Option<PendingVfxParticleLayer> {
        let VfxParticleSpawn::Burst(burst_count) = layer.spawn else {
            return None;
        };
        let seed = particle_seed(presentation_identity, recipe_id, &layer.id, occurrence);
        let backend = match (backend, self.effects.get(&layer.id)) {
            (ParticleBackend::Gpu, Some(handle)) => PendingParticleBackend::Gpu(handle.clone()),
            _ => PendingParticleBackend::Cpu {
                particles: burst_particles(layer, burst_count, seed),
                material: self.cpu_materials.get(layer.profile),
            },
        };
        Some(PendingVfxParticleLayer {
            backend,
            mesh: Some(particle_mesh(layer.profile, primitives)),
            transform: Transform::from_translation(
                ground_position(position) + Vec3::Y * layer.anchor_height,
            ),
            layer_id: layer.id.clone(),
            profile: layer.profile,
            capacity: layer.capacity,
            burst_count,
            seed,
        })
    }

    pub(crate) fn spawn_projectile_smoke(
        &self,
        commands: &mut Commands,
        primitives: &Primitive3dAssets,
        owner: Entity,
        root: Entity,
        seed: u64,
    ) -> bool {
        const LAYER_ID: &str = "fire_projectile_smoke";
        let Some(handle) = self.effects.get(LAYER_ID) else {
            return false;
        };
        let Some(layer) = self.layers.get(LAYER_ID) else {
            return false;
        };
        let VfxParticleSpawn::RatePerSecond(rate) = layer.spawn else {
            return false;
        };
        let settings = SpawnerSettings::rate(f32::from(rate).into());
        let mut particle_effect = ParticleEffect::new(handle.clone());
        particle_effect.prng_seed =
            Some(u32::try_from(seed & u64::from(u32::MAX)).expect("masked VFX seed fits u32"));
        commands.spawn((
            RetainedProjectileSmoke3d {
                owner,
                linger_seconds: layer.lifetime().as_secs_f32(),
                remaining_seconds: layer.lifetime().as_secs_f32(),
            },
            VfxParticleEmitter3d {
                root,
                layer_id: layer.id.clone(),
                profile: layer.profile,
                capacity: layer.capacity,
                spawn: layer.spawn,
                seed: particle_effect
                    .prng_seed
                    .expect("projectile smoke has a seed"),
            },
            particle_effect,
            EffectSpawner::new(&settings),
            EffectMesh(primitives.effect_sphere.clone()),
            Transform::default(),
            Name::new("Retained Fire projectile smoke emitter"),
        ));
        true
    }
}

fn particle_seed(
    presentation_identity: u64,
    recipe_id: &str,
    layer_id: &str,
    occurrence: u8,
) -> u32 {
    let sample = vfx_counter_sample(
        presentation_identity,
        recipe_id,
        layer_id,
        occurrence,
        0x5041_5254,
    );
    u32::try_from(sample & u64::from(u32::MAX)).expect("masked particle seed fits u32")
}

pub(crate) struct VfxParticleAdapterPlugin;

impl Plugin for VfxParticleAdapterPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<VfxParticleDiagnostics>()
            .add_systems(Startup, prepare_vfx_particle_assets)
            .add_systems(Update, simulate_cpu_particles)
            .add_systems(
                PostUpdate,
                (
                    update_retained_projectile_smoke,
                    activate_ready_particle_bursts,
                )
                    .before(EffectSystems::TickSpawners),
            );
    }

    /// Installs Hanabi only for a GPU that can run it. Bevy finishes plugins in registration
    /// order and does not finish plugins added during this pass, so Hanabi is finished here.
    fn finish(&self, app: &mut App) {
        let capabilities = app
            .world()
            .get_resource::<RenderCapabilities>()
            .copied()
            .unwrap_or_else(|| RenderCapabilities::detect(app.world()));
        if capabilities.gpu_particles {
            app.add_plugins(HanabiPlugin);
            HanabiPlugin.finish(app);
        } else {
            info!("GPU particles unavailable; decorative particles use the CPU fallback");
        }
    }
}

#[allow(clippy::cast_precision_loss, clippy::needless_pass_by_value)]
fn update_retained_projectile_smoke(
    mut commands: Commands,
    time: Res<Time>,
    ticks: Query<&brawler::combat::AuthoritativeTick>,
    owners: Query<
        (&avian2d::prelude::Position, &brawler::combat::LobbedFlight),
        With<brawler::combat::Projectile>,
    >,
    mut emitters: Query<(
        Entity,
        &mut RetainedProjectileSmoke3d,
        &mut EffectSpawner,
        &mut Transform,
    )>,
) {
    let current_tick = ticks.iter().map(|tick| tick.0).max().unwrap_or_default();
    for (entity, mut retained, mut spawner, mut transform) in &mut emitters {
        if let Ok((position, flight)) = owners.get(retained.owner) {
            let duration = flight
                .lands_at_tick
                .saturating_sub(flight.launched_at_tick)
                .max(1);
            let elapsed = current_tick
                .saturating_sub(flight.launched_at_tick)
                .min(duration);
            let progress = elapsed as f32 / duration as f32;
            let height = super::super::LOBBED_PROJECTILE_LAUNCH_HEIGHT
                + brawler::combat::delivery::lob_height(progress, flight.visual_arc_height);
            transform.translation = ground_position(position.0) + Vec3::Y * height;
            spawner.active = true;
            retained.remaining_seconds = retained.linger_seconds;
            continue;
        }
        spawner.active = false;
        retained.remaining_seconds -= time.delta_secs();
        if retained.remaining_seconds <= 0.0 {
            commands.entity(entity).despawn();
        }
    }
}

fn activate_ready_particle_bursts(
    mut commands: Commands,
    mut emitters: Query<(
        Entity,
        &CompiledParticleEffect,
        &mut EffectSpawner,
        &mut PendingParticleBurst,
    )>,
) {
    for (entity, compiled, mut spawner, mut pending) in &mut emitters {
        if !pending.observe_ready(compiled.is_ready()) {
            continue;
        }
        spawner.reset();
        commands.entity(entity).remove::<PendingParticleBurst>();
    }
}

fn particle_spawner_settings(burst_count: u16) -> SpawnerSettings {
    SpawnerSettings::once(f32::from(burst_count).into()).with_emit_on_start(false)
}

fn particle_spawner_for_layer(layer: &VfxParticleLayer) -> SpawnerSettings {
    match layer.spawn {
        VfxParticleSpawn::Burst(count) => particle_spawner_settings(count),
        VfxParticleSpawn::RatePerSecond(rate) => SpawnerSettings::rate(f32::from(rate).into()),
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "resources are Bevy system parameters owned by the scheduler"
)]
fn prepare_vfx_particle_assets(
    mut commands: Commands,
    registry: Res<VfxRegistry>,
    effects: Option<ResMut<Assets<EffectAsset>>>,
    mut materials: ResMut<Assets<StandardMaterial>>,
) {
    let layers = registry
        .particle_layers()
        .map(|layer| (layer.id.clone(), layer.clone()))
        .collect::<BTreeMap<_, _>>();
    let compiled = effects.map_or_else(BTreeMap::new, |mut effects| {
        layers
            .iter()
            .map(|(id, layer)| (id.clone(), effects.add(build_effect_asset(layer))))
            .collect()
    });
    commands.insert_resource(VfxParticleAssets {
        effects: compiled,
        layers,
        cpu_materials: CpuParticleMaterials::new(&mut materials),
    });
}

fn build_effect_asset(layer: &VfxParticleLayer) -> EffectAsset {
    let writer = ExprWriter::new();
    let position = SetPositionSphereModifier {
        center: writer.lit(Vec3::ZERO).expr(),
        radius: writer.lit(layer.spawn_radius).expr(),
        dimension: ShapeDimension::Volume,
    };
    let speed = writer.rand(ScalarType::Float) * writer.lit(layer.speed_max - layer.speed_min)
        + writer.lit(layer.speed_min);
    let angle = writer.rand(ScalarType::Float) * writer.lit(core::f32::consts::TAU);
    let horizontal_speed = speed.clone() * writer.lit(0.72);
    let upward_speed = speed * writer.lit(0.68) + writer.lit(8.0);
    let velocity = SetAttributeModifier::new(
        Attribute::VELOCITY,
        (angle.clone().cos() * horizontal_speed.clone())
            .vec3(upward_speed, angle.sin() * horizontal_speed)
            .expr(),
    );
    let lifetime = SetAttributeModifier::new(
        Attribute::LIFETIME,
        writer
            .lit(f32::from(layer.lifetime_millis) / 1_000.0)
            .expr(),
    );
    let age = SetAttributeModifier::new(Attribute::AGE, writer.lit(0.0).expr());
    let acceleration = AccelModifier::new(writer.lit(particle_acceleration(layer.profile)).expr());
    let orientation = if particle_aligns_to_velocity(layer.profile) {
        OrientMode::AlongVelocity
    } else {
        OrientMode::FaceCameraPosition
    };
    let alpha_mode = if particle_is_additive(layer.profile) {
        HanabiAlphaMode::Add
    } else {
        HanabiAlphaMode::Blend
    };

    EffectAsset::new(
        u32::from(layer.capacity),
        particle_spawner_for_layer(layer),
        writer.finish(),
    )
    .with_name(layer.id.clone())
    .with_alpha_mode(alpha_mode)
    .with_simulation_space(SimulationSpace::Global)
    .init(position)
    .init(velocity)
    .init(lifetime)
    .init(age)
    .update(acceleration)
    .render(OrientModifier::new(orientation))
    .render(ColorOverLifetimeModifier::new(particle_color_gradient(
        layer.profile,
    )))
    .render(SizeOverLifetimeModifier {
        gradient: size_gradient(layer),
        screen_space_size: false,
    })
}

/// Constant acceleration per profile, shared by the Hanabi and CPU backends.
pub(super) const fn particle_acceleration(profile: VfxParticleProfile) -> Vec3 {
    match profile {
        VfxParticleProfile::NeutralSparks => Vec3::new(0.0, -38.0, 0.0),
        VfxParticleProfile::ColdFragments => Vec3::new(0.0, -24.0, 0.0),
        VfxParticleProfile::FireEmbers => Vec3::new(0.0, -14.0, 0.0),
        VfxParticleProfile::PoisonBubbles => Vec3::new(0.0, 7.0, 0.0),
        VfxParticleProfile::RestorationPetals => Vec3::new(0.0, 2.0, 0.0),
        VfxParticleProfile::ExplosionSparks => Vec3::new(0.0, -72.0, 0.0),
        VfxParticleProfile::ExplosionSmoke => Vec3::new(0.0, 6.0, 0.0),
        VfxParticleProfile::ProjectileSmoke => Vec3::new(0.0, 4.0, 0.0),
    }
}

/// Color over lifetime per profile, shared by the Hanabi and CPU backends.
pub(super) fn particle_color_gradient(profile: VfxParticleProfile) -> HanabiGradient<Vec4> {
    let mut gradient = HanabiGradient::new();
    match profile {
        VfxParticleProfile::NeutralSparks => {
            gradient.add_key(0.0, Vec4::new(0.94, 1.0, 1.0, 1.0));
            gradient.add_key(0.28, Vec4::new(0.18, 0.9, 1.0, 0.95));
            gradient.add_key(1.0, Vec4::new(0.02, 0.32, 0.52, 0.0));
        }
        VfxParticleProfile::ColdFragments => {
            gradient.add_key(0.0, Vec4::new(0.9, 1.0, 1.0, 1.0));
            gradient.add_key(0.35, Vec4::new(0.2, 0.82, 1.0, 0.95));
            gradient.add_key(1.0, Vec4::new(0.05, 0.32, 0.7, 0.0));
        }
        VfxParticleProfile::FireEmbers => {
            gradient.add_key(0.0, Vec4::new(1.0, 0.98, 0.35, 1.0));
            gradient.add_key(0.3, Vec4::new(1.0, 0.35, 0.025, 0.95));
            gradient.add_key(1.0, Vec4::new(0.72, 0.025, 0.0, 0.0));
        }
        VfxParticleProfile::PoisonBubbles => {
            gradient.add_key(0.0, Vec4::new(0.68, 1.0, 0.18, 0.95));
            gradient.add_key(0.42, Vec4::new(0.12, 0.82, 0.20, 0.78));
            gradient.add_key(1.0, Vec4::new(0.03, 0.24, 0.08, 0.0));
        }
        VfxParticleProfile::RestorationPetals => {
            gradient.add_key(0.0, Vec4::new(0.38, 1.0, 0.62, 0.78));
            gradient.add_key(0.38, Vec4::new(0.06, 0.74, 0.52, 0.66));
            gradient.add_key(1.0, Vec4::new(0.02, 0.38, 0.34, 0.0));
        }
        VfxParticleProfile::ExplosionSparks => {
            gradient.add_key(0.0, Vec4::new(1.0, 1.0, 0.72, 1.0));
            gradient.add_key(0.18, Vec4::new(1.0, 0.58, 0.06, 1.0));
            gradient.add_key(1.0, Vec4::new(0.62, 0.04, 0.005, 0.0));
        }
        VfxParticleProfile::ExplosionSmoke => {
            gradient.add_key(0.0, Vec4::new(0.22, 0.16, 0.13, 0.0));
            gradient.add_key(0.18, Vec4::new(0.20, 0.16, 0.15, 0.58));
            gradient.add_key(0.7, Vec4::new(0.11, 0.12, 0.14, 0.34));
            gradient.add_key(1.0, Vec4::new(0.07, 0.08, 0.10, 0.0));
        }
        VfxParticleProfile::ProjectileSmoke => {
            gradient.add_key(0.0, Vec4::new(0.24, 0.15, 0.10, 0.0));
            gradient.add_key(0.12, Vec4::new(0.22, 0.16, 0.14, 0.46));
            gradient.add_key(0.55, Vec4::new(0.13, 0.13, 0.14, 0.30));
            gradient.add_key(1.0, Vec4::new(0.08, 0.09, 0.10, 0.0));
        }
    }
    gradient
}

/// Start and end particle size, shared by the Hanabi and CPU backends.
pub(super) fn particle_size_endpoints(layer: &VfxParticleLayer) -> (Vec3, Vec3) {
    let start = match layer.profile {
        VfxParticleProfile::ColdFragments => Vec3::new(
            layer.size_start,
            layer.size_start * 0.28,
            layer.size_start * 0.28,
        ),
        VfxParticleProfile::FireEmbers
        | VfxParticleProfile::PoisonBubbles
        | VfxParticleProfile::RestorationPetals
        | VfxParticleProfile::ExplosionSmoke
        | VfxParticleProfile::ProjectileSmoke => Vec3::splat(layer.size_start),
        VfxParticleProfile::NeutralSparks | VfxParticleProfile::ExplosionSparks => Vec3::new(
            layer.size_start * 1.8,
            layer.size_start * 0.22,
            layer.size_start * 0.22,
        ),
    };
    (start, Vec3::splat(layer.size_end))
}

fn size_gradient(layer: &VfxParticleLayer) -> HanabiGradient<Vec3> {
    let (start, end) = particle_size_endpoints(layer);
    let mut gradient = HanabiGradient::new();
    gradient.add_key(0.0, start);
    gradient.add_key(0.72, start * 0.7);
    gradient.add_key(1.0, end);
    gradient
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    fn layer(profile: VfxParticleProfile) -> VfxParticleLayer {
        VfxParticleLayer {
            id: "test_particles".to_string(),
            profile,
            capacity: 64,
            spawn: if profile == VfxParticleProfile::ProjectileSmoke {
                VfxParticleSpawn::RatePerSecond(18)
            } else {
                VfxParticleSpawn::Burst(24)
            },
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
    fn authored_particle_profiles_compile_to_bounded_assets() {
        for profile in [
            VfxParticleProfile::NeutralSparks,
            VfxParticleProfile::ColdFragments,
            VfxParticleProfile::FireEmbers,
            VfxParticleProfile::ExplosionSparks,
            VfxParticleProfile::ExplosionSmoke,
            VfxParticleProfile::ProjectileSmoke,
        ] {
            let layer = layer(profile);
            let asset = build_effect_asset(&layer);
            assert_eq!(asset.capacity(), u32::from(layer.capacity));
        }
    }

    #[test]
    fn particle_seed_is_stable_and_identity_local() {
        let first = particle_seed(7, "fire_impact", "embers", 0);
        assert_eq!(first, particle_seed(7, "fire_impact", "embers", 0));
        assert_ne!(first, particle_seed(8, "fire_impact", "embers", 0));
        assert_ne!(first, particle_seed(7, "fire_impact", "embers", 1));
    }

    #[test]
    fn retained_smoke_follows_its_owner_then_lingers_until_particles_expire() {
        let mut app = App::new();
        app.init_resource::<Time>()
            .add_systems(PostUpdate, update_retained_projectile_smoke);
        app.world_mut()
            .spawn(brawler::combat::AuthoritativeTick(60));
        let owner = app
            .world_mut()
            .spawn((
                brawler::combat::Projectile,
                avian2d::prelude::Position(Vec2::ZERO),
                brawler::combat::LobbedFlight {
                    launch: brawler::combat::WorldPoint::from(Vec2::new(-100.0, 0.0)),
                    landing: brawler::combat::WorldPoint::from(Vec2::new(100.0, 0.0)),
                    launched_at_tick: 20,
                    lands_at_tick: 100,
                    visual_arc_height: 120.0,
                },
            ))
            .id();
        let settings = SpawnerSettings::rate(18.0.into());
        let emitter = app
            .world_mut()
            .spawn((
                RetainedProjectileSmoke3d {
                    owner,
                    linger_seconds: 0.9,
                    remaining_seconds: 0.9,
                },
                EffectSpawner::new(&settings),
                Transform::default(),
            ))
            .id();

        app.world_mut().run_schedule(PostUpdate);
        let transform = app.world().entity(emitter).get::<Transform>().unwrap();
        assert!((transform.translation.y - 140.0).abs() < f32::EPSILON);

        app.world_mut().despawn(owner);
        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(500));
        app.world_mut().run_schedule(PostUpdate);
        assert!(
            !app.world()
                .entity(emitter)
                .get::<EffectSpawner>()
                .unwrap()
                .active
        );

        app.world_mut()
            .resource_mut::<Time>()
            .advance_by(Duration::from_millis(500));
        app.world_mut().run_schedule(PostUpdate);
        assert!(app.world().get_entity(emitter).is_err());
    }

    #[test]
    fn cold_pipeline_burst_waits_for_two_consecutive_ready_observations() {
        let mut pending = PendingParticleBurst::default();
        assert!(!pending.observe_ready(false));
        assert!(!pending.observe_ready(true));
        assert!(!pending.observe_ready(false));
        assert!(!pending.observe_ready(true));
        assert!(pending.observe_ready(true));
    }
}
