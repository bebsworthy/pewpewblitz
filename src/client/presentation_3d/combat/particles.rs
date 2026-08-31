//! Bounded Hanabi adapter for optional decorative VFX recipe layers.

use super::super::{Primitive3dAssets, ground_position};
use crate::client::vfx::{VfxParticleLayer, VfxParticleProfile, VfxRegistry, vfx_counter_sample};
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

pub(in crate::client) const MAX_ACTIVE_PARTICLE_EMITTERS: usize = 96;

#[derive(Resource, Default, Clone, Copy)]
pub(in crate::client) struct VfxParticleDiagnostics {
    pub(in crate::client) active: usize,
    pub(in crate::client) high_water: usize,
    pub(in crate::client) suppressed: u64,
}

#[derive(Resource, Default)]
pub(in crate::client) struct VfxParticleAssets {
    effects: BTreeMap<String, Handle<EffectAsset>>,
}

#[derive(Component)]
pub(in crate::client) struct VfxParticleEmitter3d {
    pub(in crate::client) root: Entity,
    pub(in crate::client) layer_id: String,
    pub(in crate::client) profile: VfxParticleProfile,
    pub(in crate::client) capacity: u16,
    pub(in crate::client) burst_count: u16,
    pub(in crate::client) seed: u32,
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

pub(in crate::client) struct PendingVfxParticleLayer {
    handle: Handle<EffectAsset>,
    mesh: Option<Handle<Mesh>>,
    transform: Transform,
    layer_id: String,
    profile: VfxParticleProfile,
    capacity: u16,
    burst_count: u16,
    seed: u32,
}

impl PendingVfxParticleLayer {
    pub(in crate::client) fn spawn(&self, parent: &mut ChildSpawnerCommands<'_>, root: Entity) {
        let mut particle_effect = ParticleEffect::new(self.handle.clone());
        particle_effect.prng_seed = Some(self.seed);
        let settings = particle_spawner_settings(self.burst_count);
        let mut entity = parent.spawn((
            VfxParticleEmitter3d {
                root,
                layer_id: self.layer_id.clone(),
                profile: self.profile,
                capacity: self.capacity,
                burst_count: self.burst_count,
                seed: self.seed,
            },
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

    #[cfg(test)]
    pub(in crate::client) fn test_layer(id: &str) -> Self {
        Self {
            handle: Handle::default(),
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
    pub(in crate::client) fn pending(
        &self,
        layer: &VfxParticleLayer,
        primitives: &Primitive3dAssets,
        position: Vec2,
        recipe_id: &str,
        presentation_identity: u64,
        occurrence: u8,
    ) -> Option<PendingVfxParticleLayer> {
        let seed = particle_seed(presentation_identity, recipe_id, &layer.id, occurrence);
        Some(PendingVfxParticleLayer {
            handle: self.effects.get(&layer.id)?.clone(),
            mesh: Some(primitives.fire_tongue.clone()),
            transform: Transform::from_translation(
                ground_position(position) + Vec3::Y * layer.anchor_height,
            ),
            layer_id: layer.id.clone(),
            profile: layer.profile,
            capacity: layer.capacity,
            burst_count: layer.burst_count,
            seed,
        })
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

pub(in crate::client) struct VfxParticleAdapterPlugin;

impl Plugin for VfxParticleAdapterPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(HanabiPlugin)
            .init_resource::<VfxParticleDiagnostics>()
            .add_systems(Startup, prepare_vfx_particle_assets)
            .add_systems(
                PostUpdate,
                activate_ready_particle_bursts.before(EffectSystems::TickSpawners),
            );
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

#[allow(
    clippy::needless_pass_by_value,
    reason = "resources are Bevy system parameters owned by the scheduler"
)]
fn prepare_vfx_particle_assets(
    mut commands: Commands,
    registry: Res<VfxRegistry>,
    mut effects: ResMut<Assets<EffectAsset>>,
) {
    let effects = registry
        .particle_layers()
        .map(|layer| (layer.id.clone(), effects.add(build_effect_asset(layer))))
        .collect();
    commands.insert_resource(VfxParticleAssets { effects });
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
    let acceleration = match layer.profile {
        VfxParticleProfile::ColdFragments => Vec3::new(0.0, -24.0, 0.0),
        VfxParticleProfile::FireEmbers => Vec3::new(0.0, -14.0, 0.0),
    };
    let acceleration = AccelModifier::new(writer.lit(acceleration).expr());
    let orientation = match layer.profile {
        VfxParticleProfile::ColdFragments => OrientMode::AlongVelocity,
        VfxParticleProfile::FireEmbers => OrientMode::FaceCameraPosition,
    };

    EffectAsset::new(
        u32::from(layer.capacity),
        particle_spawner_settings(layer.burst_count),
        writer.finish(),
    )
    .with_name(layer.id.clone())
    .with_alpha_mode(HanabiAlphaMode::Add)
    .with_simulation_space(SimulationSpace::Global)
    .init(position)
    .init(velocity)
    .init(lifetime)
    .init(age)
    .update(acceleration)
    .render(OrientModifier::new(orientation))
    .render(ColorOverLifetimeModifier::new(color_gradient(
        layer.profile,
    )))
    .render(SizeOverLifetimeModifier {
        gradient: size_gradient(layer),
        screen_space_size: false,
    })
}

fn color_gradient(profile: VfxParticleProfile) -> HanabiGradient<Vec4> {
    let mut gradient = HanabiGradient::new();
    match profile {
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
    }
    gradient
}

fn size_gradient(layer: &VfxParticleLayer) -> HanabiGradient<Vec3> {
    let mut gradient = HanabiGradient::new();
    let start = match layer.profile {
        VfxParticleProfile::ColdFragments => Vec3::new(
            layer.size_start,
            layer.size_start * 0.28,
            layer.size_start * 0.28,
        ),
        VfxParticleProfile::FireEmbers => Vec3::splat(layer.size_start),
    };
    gradient.add_key(0.0, start);
    gradient.add_key(0.72, start * 0.7);
    gradient.add_key(1.0, Vec3::splat(layer.size_end));
    gradient
}

#[cfg(test)]
mod tests {
    use super::*;

    fn layer(profile: VfxParticleProfile) -> VfxParticleLayer {
        VfxParticleLayer {
            id: "test_particles".to_string(),
            profile,
            capacity: 64,
            burst_count: 24,
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
            VfxParticleProfile::ColdFragments,
            VfxParticleProfile::FireEmbers,
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
    fn cold_pipeline_burst_waits_for_two_consecutive_ready_observations() {
        let mut pending = PendingParticleBurst::default();
        assert!(!pending.observe_ready(false));
        assert!(!pending.observe_ready(true));
        assert!(!pending.observe_ready(false));
        assert!(!pending.observe_ready(true));
        assert!(pending.observe_ready(true));
    }
}
