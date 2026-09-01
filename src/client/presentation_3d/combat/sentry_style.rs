use super::super::{Material3dAssets, Primitive3dAssets};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum SentryStyleQuality {
    Normal,
    Reduced,
}

#[derive(Component)]
pub(in crate::client) struct SentryPowerCore3d {
    base_scale: Vec3,
    phase: f32,
}

#[derive(Component)]
pub(in crate::client) struct SentryOrbitNode3d {
    radius: f32,
    phase: f32,
    speed: f32,
}

fn sentry_style_quality(
    signature: crate::client::vfx::CombatVisualSignature,
) -> Option<SentryStyleQuality> {
    use crate::client::vfx::{VfxAccent, VfxCarrier, VfxPhase, VfxPresentationVariant};

    if signature.carrier != VfxCarrier::Deployable
        || signature.phase != VfxPhase::Active
        || !signature.accents.contains(&Some(VfxAccent::Neutral))
    {
        return None;
    }
    match signature.variant {
        VfxPresentationVariant::Normal => Some(SentryStyleQuality::Normal),
        VfxPresentationVariant::Reduced => Some(SentryStyleQuality::Reduced),
        VfxPresentationVariant::Primitive | VfxPresentationVariant::ReducedPrimitive => None,
    }
}

pub(in crate::client) fn spawn_sentry_style(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    signature: crate::client::vfx::CombatVisualSignature,
    team_material: Handle<StandardMaterial>,
) -> bool {
    let Some(quality) = sentry_style_quality(signature) else {
        return false;
    };

    spawn_grounded_chassis(parent, primitives, materials, team_material);
    spawn_power_assembly(parent, primitives, materials, quality);
    true
}

fn spawn_grounded_chassis(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    team_material: Handle<StandardMaterial>,
) {
    parent.spawn((
        Mesh3d(primitives.area_thin_ring.clone()),
        MeshMaterial3d(materials.neutral_energy_soft.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        Transform::from_xyz(0.0, 1.1, 0.0)
            .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2))
            .with_scale(Vec3::splat(23.0)),
        Name::new("Sentry grounded power footprint"),
    ));
    parent.spawn((
        Mesh3d(primitives.sentry_base.clone()),
        MeshMaterial3d(materials.sentry_armor_dark.clone()),
        Transform::from_xyz(0.0, 4.0, 0.0).with_scale(Vec3::new(1.08, 1.0, 1.08)),
        Name::new("Sentry armored plinth"),
    ));
    parent.spawn((
        Mesh3d(primitives.sentry_body.clone()),
        MeshMaterial3d(materials.sentry_armor.clone()),
        Transform::from_xyz(0.0, 17.0, 0.0).with_scale(Vec3::new(1.04, 0.82, 1.04)),
        Name::new("Sentry team armor"),
    ));

    for side in [-1.0_f32, 1.0] {
        parent.spawn((
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(team_material.clone()),
            Transform::from_xyz(-2.0, 13.0, side * 16.0)
                .with_rotation(Quat::from_rotation_y(side * 0.18))
                .with_scale(Vec3::new(24.0, 8.0, 4.0)),
            Name::new("Sentry stabilizer armor"),
        ));
    }

    parent.spawn((
        Mesh3d(primitives.sentry_direction.clone()),
        MeshMaterial3d(materials.sentry_armor.clone()),
        Transform::from_xyz(21.0, 27.0, 0.0).with_scale(Vec3::new(1.15, 1.28, 1.22)),
        Name::new("Sentry aim-aligned barrel housing"),
    ));
    parent.spawn((
        Mesh3d(primitives.unit_cuboid.clone()),
        MeshMaterial3d(team_material),
        Transform::from_xyz(19.0, 32.0, 0.0).with_scale(Vec3::new(24.0, 2.2, 8.5)),
        Name::new("Sentry team armor barrel crest"),
    ));
    parent.spawn((
        Mesh3d(primitives.unit_cuboid.clone()),
        MeshMaterial3d(materials.neutral_energy_bright.clone()),
        NotShadowCaster,
        Transform::from_xyz(37.0, 27.0, 0.0).with_scale(Vec3::new(10.0, 3.2, 4.2)),
        Name::new("Sentry emissive muzzle rail"),
    ));
}

fn spawn_power_assembly(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    quality: SentryStyleQuality,
) {
    let core_scale = Vec3::splat(7.5);
    parent.spawn((
        SentryPowerCore3d {
            base_scale: core_scale,
            phase: 0.7,
        },
        Mesh3d(primitives.effect_sphere.clone()),
        MeshMaterial3d(materials.neutral_energy_bright.clone()),
        NotShadowCaster,
        Transform::from_xyz(0.0, 27.0, 0.0).with_scale(core_scale),
        Name::new("Sentry pulsing power core"),
    ));
    parent.spawn((
        Mesh3d(primitives.area_ring.clone()),
        MeshMaterial3d(materials.neutral_energy_soft.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        Transform::from_xyz(0.0, 27.0, 0.0)
            .with_rotation(Quat::from_rotation_y(core::f32::consts::FRAC_PI_2))
            .with_scale(Vec3::splat(14.0)),
        Name::new("Sentry vertical energy gimbal"),
    ));

    let node_count: u16 = match quality {
        SentryStyleQuality::Normal => 4,
        SentryStyleQuality::Reduced => 1,
    };
    for slot in 0..node_count {
        let phase = f32::from(slot) * core::f32::consts::TAU / f32::from(node_count);
        let radius = 14.0;
        parent.spawn((
            SentryOrbitNode3d {
                radius,
                phase,
                speed: 1.7,
            },
            Mesh3d(primitives.effect_sphere.clone()),
            MeshMaterial3d(materials.neutral_energy_bright.clone()),
            NotShadowCaster,
            Transform::from_xyz(0.0, 27.0 + phase.cos() * radius, phase.sin() * radius)
                .with_scale(Vec3::splat(if node_count == 1 { 2.2 } else { 2.8 })),
            Name::new("Sentry orbiting charge node"),
        ));
    }
}

pub(in crate::client) fn animate_sentry_style(
    ticks: Query<&crate::combat::AuthoritativeTick>,
    mut cores: Query<(&SentryPowerCore3d, &mut Transform), Without<SentryOrbitNode3d>>,
    mut nodes: Query<(&SentryOrbitNode3d, &mut Transform), Without<SentryPowerCore3d>>,
) {
    let time = visual_time(&ticks);
    for (core, mut transform) in &mut cores {
        let pulse = 1.0 + (time * 4.2 + core.phase).sin() * 0.12;
        transform.scale = core.base_scale * pulse;
    }
    for (node, mut transform) in &mut nodes {
        let angle = node.phase + time * node.speed;
        transform.translation = Vec3::new(
            0.0,
            27.0 + angle.cos() * node.radius,
            angle.sin() * node.radius,
        );
    }
}

fn visual_time(ticks: &Query<&crate::combat::AuthoritativeTick>) -> f32 {
    let tick = ticks.iter().map(|tick| tick.0).max().unwrap_or_default();
    let bounded_tick = u16::try_from(tick % (crate::timing::SIMULATION_TICK_HZ * 600))
        .expect("the ten-minute Sentry visual cycle fits u16");
    let tick_hz = u16::try_from(crate::timing::SIMULATION_TICK_HZ)
        .expect("the simulation tick rate fits u16");
    f32::from(bounded_tick) / f32::from(tick_hz)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn signature(
        variant: crate::client::vfx::VfxPresentationVariant,
    ) -> crate::client::vfx::CombatVisualSignature {
        crate::client::vfx::CombatVisualSignature {
            carrier: crate::client::vfx::VfxCarrier::Deployable,
            phase: crate::client::vfx::VfxPhase::Active,
            accents: [Some(crate::client::vfx::VfxAccent::Neutral), None],
            relationship: crate::client::vfx::VfxRelationship::Neutral,
            context: Some(crate::client::vfx::VfxContextOverride::Sentry),
            geometry: crate::client::vfx::VfxGeometry::point(Vec2::ZERO),
            identity: crate::client::vfx::VfxPresentationIdentity::Deployable(1),
            timing: crate::client::vfx::VfxAuthoritativeTiming::default(),
            variant,
        }
    }

    #[test]
    fn semantic_sentry_style_degrades_to_primitive_fallback() {
        assert_eq!(
            sentry_style_quality(signature(
                crate::client::vfx::VfxPresentationVariant::Normal
            )),
            Some(SentryStyleQuality::Normal)
        );
        assert_eq!(
            sentry_style_quality(signature(
                crate::client::vfx::VfxPresentationVariant::Reduced
            )),
            Some(SentryStyleQuality::Reduced)
        );
        assert_eq!(
            sentry_style_quality(signature(
                crate::client::vfx::VfxPresentationVariant::Primitive
            )),
            None
        );
    }
}
