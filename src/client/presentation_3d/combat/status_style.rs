use super::{
    super::{Material3dAssets, Primitive3dAssets},
    fighter_feedback::StatusKind,
};
use bevy::light::{NotShadowCaster, NotShadowReceiver};
use bevy::prelude::*;

const STATUS_ACCENT_RADIUS: f32 = 25.0;
const STATUS_LAYER_LIMIT: usize = 8;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatusStyleQuality {
    Normal,
    Reduced,
    Primitive,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum StatusMotion {
    BoundaryPulse,
    Orbit,
    RadialPulse,
    Bob,
    Flicker,
    Shiver,
}

#[derive(Component)]
pub(in crate::client) struct StatusMotionAccent3d {
    owner: Entity,
    motion: StatusMotion,
    base_translation: Vec3,
    base_scale: Vec3,
    phase: f32,
    speed: f32,
}

pub(super) fn spawn_status_style(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    owner: Entity,
    kind: StatusKind,
    signature: crate::client::vfx::CombatVisualSignature,
) {
    let quality = quality_for_variant(signature.variant);
    let radius = super::fighter_feedback::status_scale(kind);
    let boundary_transform = Transform::from_xyz(0.0, 0.0, 0.0)
        .with_rotation(Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2))
        .with_scale(Vec3::splat(radius));
    let mut boundary = parent.spawn((
        Mesh3d(primitives.ground_ring.clone()),
        MeshMaterial3d(super::fighter_feedback::status_material(kind, materials)),
        NotShadowCaster,
        NotShadowReceiver,
        boundary_transform,
        Name::new("Attached status exact boundary"),
    ));
    if quality != StatusStyleQuality::Primitive {
        boundary.insert(StatusMotionAccent3d {
            owner,
            motion: StatusMotion::BoundaryPulse,
            base_translation: boundary_transform.translation,
            base_scale: boundary_transform.scale,
            phase: status_phase(kind),
            speed: 1.0,
        });
    }

    let motif_count = status_motif_count(kind, quality);
    for slot in 0..motif_count {
        spawn_status_motif(
            parent,
            primitives,
            materials,
            owner,
            kind,
            slot,
            motif_count,
        );
    }

    debug_assert!(usize::from(motif_count) < STATUS_LAYER_LIMIT);
}

fn spawn_status_motif(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    owner: Entity,
    kind: StatusKind,
    slot: u8,
    count: u8,
) {
    let phase = f32::from(slot) * core::f32::consts::TAU / f32::from(count.max(1));
    let radial = STATUS_ACCENT_RADIUS * accent_radius_factor(kind);
    let translation = Vec3::new(
        phase.cos() * radial,
        accent_height(kind),
        phase.sin() * radial,
    );
    let scale = accent_scale(kind, slot);
    let transform = Transform::from_translation(translation)
        .with_rotation(accent_rotation(kind, phase))
        .with_scale(scale);
    parent.spawn((
        StatusMotionAccent3d {
            owner,
            motion: accent_motion(kind),
            base_translation: translation,
            base_scale: scale,
            phase,
            speed: accent_speed(kind),
        },
        Mesh3d(accent_mesh(kind, primitives)),
        MeshMaterial3d(accent_material(kind, materials)),
        NotShadowCaster,
        NotShadowReceiver,
        transform,
        Name::new(accent_name(kind)),
    ));
}

pub(in crate::client) fn animate_status_style(
    ticks: Query<&crate::combat::AuthoritativeTick>,
    mut accents: Query<(&StatusMotionAccent3d, &mut Transform)>,
) {
    for (accent, mut transform) in &mut accents {
        let tick = ticks.get(accent.owner).map_or(0, |tick| tick.0);
        let time = status_visual_time(tick);
        let wave = time.mul_add(accent.speed, accent.phase).sin();
        match accent.motion {
            StatusMotion::BoundaryPulse => {
                transform.scale = accent.base_scale * wave.mul_add(0.035, 1.0);
            }
            StatusMotion::Orbit => {
                let angle = time.mul_add(accent.speed, accent.phase);
                let radial =
                    Vec2::new(accent.base_translation.x, accent.base_translation.z).length();
                transform.translation.x = angle.cos() * radial;
                transform.translation.z = angle.sin() * radial;
                transform.scale = accent.base_scale * wave.mul_add(0.08, 1.0);
            }
            StatusMotion::RadialPulse => {
                let pulse = (wave + 1.0) * 0.5;
                let factor = pulse.mul_add(0.28, 0.82);
                transform.translation.x = accent.base_translation.x * factor;
                transform.translation.z = accent.base_translation.z * factor;
                transform.scale = accent.base_scale * (1.15 - pulse * 0.35);
            }
            StatusMotion::Bob => {
                transform.translation.y = accent.base_translation.y + (wave + 1.0) * 2.4;
                transform.scale = accent.base_scale * wave.mul_add(0.12, 1.0);
            }
            StatusMotion::Flicker => {
                transform.scale = accent.base_scale * wave.abs().mul_add(0.24, 0.82);
                transform.translation.y = accent.base_translation.y + wave.abs() * 2.2;
            }
            StatusMotion::Shiver => {
                transform.translation.x = accent.base_translation.x + wave * 1.2;
                transform.translation.z = accent.base_translation.z + (wave * 1.7).cos() * 0.8;
                transform.scale = accent.base_scale * wave.mul_add(0.05, 1.0);
            }
        }
    }
}

const fn quality_for_variant(
    variant: crate::client::vfx::VfxPresentationVariant,
) -> StatusStyleQuality {
    use crate::client::vfx::VfxPresentationVariant;

    match variant {
        VfxPresentationVariant::Normal => StatusStyleQuality::Normal,
        VfxPresentationVariant::Reduced => StatusStyleQuality::Reduced,
        VfxPresentationVariant::Primitive | VfxPresentationVariant::ReducedPrimitive => {
            StatusStyleQuality::Primitive
        }
    }
}

const fn status_motif_count(kind: StatusKind, quality: StatusStyleQuality) -> u8 {
    match quality {
        StatusStyleQuality::Primitive => 0,
        StatusStyleQuality::Reduced => 1,
        StatusStyleQuality::Normal => match kind {
            StatusKind::Frozen => 6,
            StatusKind::Reveal => 4,
            StatusKind::Slow | StatusKind::Cold | StatusKind::Poison | StatusKind::Fire => 3,
            StatusKind::Knockback => 2,
            StatusKind::TileSpeed | StatusKind::TileSlow | StatusKind::TileDamage => 0,
        },
    }
}

const fn status_phase(kind: StatusKind) -> f32 {
    match kind {
        StatusKind::Slow
        | StatusKind::TileSpeed
        | StatusKind::TileSlow
        | StatusKind::TileDamage => 0.0,
        StatusKind::Knockback => 0.7,
        StatusKind::Reveal => 1.4,
        StatusKind::Cold => 2.1,
        StatusKind::Frozen => 2.8,
        StatusKind::Poison => 3.5,
        StatusKind::Fire => 4.2,
    }
}

const fn accent_motion(kind: StatusKind) -> StatusMotion {
    match kind {
        StatusKind::Slow | StatusKind::Cold | StatusKind::Reveal => StatusMotion::Orbit,
        StatusKind::Knockback => StatusMotion::RadialPulse,
        StatusKind::Frozen => StatusMotion::Shiver,
        StatusKind::Poison => StatusMotion::Bob,
        StatusKind::Fire => StatusMotion::Flicker,
        StatusKind::TileSpeed | StatusKind::TileSlow | StatusKind::TileDamage => {
            StatusMotion::BoundaryPulse
        }
    }
}

const fn accent_speed(kind: StatusKind) -> f32 {
    match kind {
        StatusKind::Slow => 0.55,
        StatusKind::Knockback => 5.4,
        StatusKind::Reveal => 2.3,
        StatusKind::Cold => -0.72,
        StatusKind::Frozen => 7.2,
        StatusKind::Poison => 2.0,
        StatusKind::Fire => 6.5,
        StatusKind::TileSpeed | StatusKind::TileSlow | StatusKind::TileDamage => 1.0,
    }
}

const fn accent_radius_factor(kind: StatusKind) -> f32 {
    match kind {
        StatusKind::Slow => 0.92,
        StatusKind::Knockback
        | StatusKind::TileSpeed
        | StatusKind::TileSlow
        | StatusKind::TileDamage => 1.0,
        StatusKind::Reveal => 1.18,
        StatusKind::Cold => 0.98,
        StatusKind::Frozen => 1.08,
        StatusKind::Poison => 0.9,
        StatusKind::Fire => 1.02,
    }
}

const fn accent_height(kind: StatusKind) -> f32 {
    match kind {
        StatusKind::Frozen => 5.8,
        StatusKind::Poison => 4.0,
        StatusKind::Fire => 5.0,
        _ => 1.0,
    }
}

fn accent_scale(kind: StatusKind, slot: u8) -> Vec3 {
    match kind {
        StatusKind::Slow => Vec3::splat(3.4),
        StatusKind::Knockback => Vec3::new(8.0, 1.5, 3.2),
        StatusKind::Reveal => Vec3::splat(if slot.is_multiple_of(2) { 4.6 } else { 3.4 }),
        StatusKind::Cold => Vec3::new(2.4, 6.0, 2.4),
        StatusKind::Frozen => Vec3::new(2.8, 8.0, 2.8),
        StatusKind::Poison => Vec3::splat(3.4 + f32::from(slot) * 0.7),
        StatusKind::Fire => Vec3::new(4.5, 7.5, 4.5),
        StatusKind::TileSpeed | StatusKind::TileSlow | StatusKind::TileDamage => Vec3::ONE,
    }
}

fn accent_rotation(kind: StatusKind, phase: f32) -> Quat {
    match kind {
        StatusKind::Slow | StatusKind::Reveal => {
            Quat::from_rotation_y(-phase) * Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2)
        }
        StatusKind::Knockback | StatusKind::Cold | StatusKind::Frozen | StatusKind::Fire => {
            Quat::from_rotation_y(-phase)
        }
        StatusKind::Poison
        | StatusKind::TileSpeed
        | StatusKind::TileSlow
        | StatusKind::TileDamage => Quat::IDENTITY,
    }
}

fn accent_mesh(kind: StatusKind, primitives: &Primitive3dAssets) -> Handle<Mesh> {
    match kind {
        StatusKind::Slow | StatusKind::Reveal => primitives.vitality_spark.clone(),
        StatusKind::Poison => primitives.effect_sphere.clone(),
        StatusKind::Fire => primitives.fire_tongue.clone(),
        StatusKind::Knockback | StatusKind::Cold | StatusKind::Frozen => {
            primitives.unit_cuboid.clone()
        }
        StatusKind::TileSpeed | StatusKind::TileSlow | StatusKind::TileDamage => {
            primitives.effect_sphere.clone()
        }
    }
}

fn accent_material(kind: StatusKind, materials: &Material3dAssets) -> Handle<StandardMaterial> {
    match kind {
        StatusKind::Slow => materials.status_slow.clone(),
        StatusKind::Cold | StatusKind::Frozen => materials.elemental_cold_bright.clone(),
        _ => super::fighter_feedback::status_material(kind, materials),
    }
}

const fn accent_name(kind: StatusKind) -> &'static str {
    match kind {
        StatusKind::Slow => "Slow orbiting drag mote",
        StatusKind::Knockback => "Knockback outward impulse bar",
        StatusKind::Reveal => "Reveal rotating detection spark",
        StatusKind::Cold => "Cold orbiting ice splinter",
        StatusKind::Frozen => "Frozen locking ice shard",
        StatusKind::Poison => "Poison rising status bubble",
        StatusKind::Fire => "Fire flickering status tongue",
        StatusKind::TileSpeed | StatusKind::TileSlow | StatusKind::TileDamage => {
            "Effect tile status accent"
        }
    }
}

fn status_visual_time(tick: u64) -> f32 {
    let bounded_tick = u16::try_from(tick % (crate::timing::SIMULATION_TICK_HZ * 600))
        .expect("the ten-minute status visual cycle fits u16");
    let tick_hz = u16::try_from(crate::timing::SIMULATION_TICK_HZ)
        .expect("the simulation tick rate fits u16");
    f32::from(bounded_tick) / f32::from(tick_hz)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn attached_status_layer_plans_are_bounded_and_degrade() {
        let kinds = [
            StatusKind::Slow,
            StatusKind::Knockback,
            StatusKind::Reveal,
            StatusKind::Cold,
            StatusKind::Frozen,
            StatusKind::Poison,
            StatusKind::Fire,
        ];
        for kind in kinds {
            assert!(usize::from(status_motif_count(kind, StatusStyleQuality::Normal)) < 8);
            assert_eq!(status_motif_count(kind, StatusStyleQuality::Reduced), 1);
            assert_eq!(status_motif_count(kind, StatusStyleQuality::Primitive), 0);
        }
    }

    #[test]
    fn status_time_is_authoritative_tick_driven_and_bounded() {
        assert!((status_visual_time(30) - 0.5).abs() < f32::EPSILON);
        assert!(status_visual_time(crate::timing::SIMULATION_TICK_HZ * 600).abs() < f32::EPSILON);
    }

    #[test]
    fn status_motion_replays_identically_at_the_same_authoritative_tick() {
        let mut app = App::new();
        app.add_systems(Update, animate_status_style);
        let owner = app
            .world_mut()
            .spawn(crate::combat::AuthoritativeTick(30))
            .id();
        let base_translation = Vec3::new(20.0, 4.0, 0.0);
        let base_scale = Vec3::splat(3.0);
        let accent = app
            .world_mut()
            .spawn((
                StatusMotionAccent3d {
                    owner,
                    motion: StatusMotion::Bob,
                    base_translation,
                    base_scale,
                    phase: 0.7,
                    speed: 2.0,
                },
                Transform::from_translation(base_translation).with_scale(base_scale),
            ))
            .id();

        app.update();
        let first = *app.world().get::<Transform>(accent).unwrap();
        app.world_mut()
            .get_mut::<crate::combat::AuthoritativeTick>(owner)
            .unwrap()
            .0 = 90;
        app.update();
        let later = *app.world().get::<Transform>(accent).unwrap();
        assert_ne!(first, later);

        app.world_mut()
            .get_mut::<crate::combat::AuthoritativeTick>(owner)
            .unwrap()
            .0 = 30;
        app.update();
        assert_eq!(*app.world().get::<Transform>(accent).unwrap(), first);
    }
}
