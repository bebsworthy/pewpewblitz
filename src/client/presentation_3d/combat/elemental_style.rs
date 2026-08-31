//! Retained primitive Cold/Fire styling shared by durable projectile and area roots.

use super::super::*;

#[derive(Component, Clone, Copy, Debug)]
pub(in crate::client::presentation_3d) struct ElementalMotionAccent3d {
    accent: vfx::VfxAccent,
    seed: u64,
    base_translation: Vec3,
    base_rotation: Quat,
    base_scale: Vec3,
}

pub(super) fn elemental_materials(
    signature: vfx::CombatVisualSignature,
    materials: &Material3dAssets,
) -> Option<(Handle<StandardMaterial>, Handle<StandardMaterial>)> {
    match primary_element(signature) {
        Some(vfx::VfxAccent::Cold) => Some((
            materials.elemental_cold.clone(),
            materials.elemental_cold_bright.clone(),
        )),
        Some(vfx::VfxAccent::Fire) => Some((
            materials.elemental_fire.clone(),
            materials.elemental_fire_bright.clone(),
        )),
        _ => None,
    }
}

#[allow(
    clippy::too_many_arguments,
    reason = "the style helper receives one complete retained projectile materialization profile"
)]
pub(super) fn spawn_elemental_projectile_style(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    signature: vfx::CombatVisualSignature,
    exact_mesh: Handle<Mesh>,
    exact_transform: Transform,
    fallback_relationship_material: Handle<StandardMaterial>,
    visual_radius: f32,
) -> bool {
    let Some(accent) = primary_element(signature) else {
        return false;
    };
    let Some((soft, bright)) = elemental_materials(signature, materials) else {
        return false;
    };
    parent.spawn((
        Mesh3d(exact_mesh.clone()),
        MeshMaterial3d(bright.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        exact_transform,
        Name::new("Elemental projectile exact geometry"),
    ));
    let mut relationship_transform = exact_transform;
    relationship_transform.scale *= 0.48;
    parent.spawn((
        Mesh3d(exact_mesh),
        MeshMaterial3d(relationship_material(
            signature.relationship,
            materials,
            fallback_relationship_material,
        )),
        NotShadowCaster,
        NotShadowReceiver,
        relationship_transform,
        Name::new("Elemental projectile relationship core"),
    ));

    let reduced = is_reduced(signature.variant);
    let seed = signature.stable_seed();
    match accent {
        vfx::VfxAccent::Cold => {
            let count: u8 = if reduced { 1 } else { 3 };
            for slot in 0..count {
                let angle =
                    core::f32::consts::FRAC_PI_4 + f32::from(slot) * core::f32::consts::FRAC_PI_3;
                let transform = Transform {
                    translation: Vec3::Y * 0.35,
                    rotation: Quat::from_rotation_y(angle),
                    scale: Vec3::new(visual_radius * 1.45, 1.15, visual_radius * 0.16),
                };
                parent.spawn((
                    ElementalMotionAccent3d::new(accent, seed_for(seed, slot), transform),
                    Mesh3d(primitives.unit_cuboid.clone()),
                    MeshMaterial3d(if slot == 0 {
                        bright.clone()
                    } else {
                        soft.clone()
                    }),
                    NotShadowCaster,
                    NotShadowReceiver,
                    transform,
                    Name::new("Cold projectile crystal fin"),
                ));
            }
        }
        vfx::VfxAccent::Fire => {
            let count: u8 = if reduced { 1 } else { 3 };
            for slot in 0..count {
                let distance = visual_radius * (1.0 + f32::from(slot) * 0.75);
                let scale = visual_radius * (0.62 - f32::from(slot) * 0.13);
                let transform = Transform::from_translation(Vec3::new(-distance, 0.4, 0.0))
                    .with_scale(Vec3::splat(scale.max(0.5)));
                parent.spawn((
                    ElementalMotionAccent3d::new(accent, seed_for(seed, slot), transform),
                    Mesh3d(primitives.effect_sphere.clone()),
                    MeshMaterial3d(if slot == 0 {
                        bright.clone()
                    } else {
                        soft.clone()
                    }),
                    NotShadowCaster,
                    NotShadowReceiver,
                    transform,
                    Name::new("Fire projectile trailing lobe"),
                ));
            }
        }
        _ => {}
    }
    true
}

pub(super) fn spawn_elemental_area_style(
    parent: &mut ChildSpawnerCommands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    signature: vfx::CombatVisualSignature,
    radius: f32,
) -> bool {
    let Some(accent) = primary_element(signature) else {
        return false;
    };
    let Some((soft, bright)) = elemental_materials(signature, materials) else {
        return false;
    };
    let ground_rotation = Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2);
    parent.spawn((
        Mesh3d(primitives.area_disc.clone()),
        MeshMaterial3d(soft.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        Transform::from_xyz(0.0, super::common::GROUND_EFFECT_HEIGHT, 0.0)
            .with_rotation(ground_rotation)
            .with_scale(Vec3::splat(radius)),
        Name::new("Elemental area subtle fill"),
    ));
    parent.spawn((
        Mesh3d(primitives.area_ring.clone()),
        MeshMaterial3d(bright.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        Transform::from_xyz(0.0, super::common::GROUND_EFFECT_HEIGHT + 0.65, 0.0)
            .with_rotation(ground_rotation)
            .with_scale(Vec3::splat(radius)),
        Name::new("Elemental area exact bright boundary"),
    ));
    if signature.relationship != vfx::VfxRelationship::Neutral {
        parent.spawn((
            Mesh3d(primitives.area_ring.clone()),
            MeshMaterial3d(relationship_material(
                signature.relationship,
                materials,
                materials.neutral.clone(),
            )),
            NotShadowCaster,
            NotShadowReceiver,
            Transform::from_xyz(0.0, super::common::GROUND_EFFECT_HEIGHT + 0.9, 0.0)
                .with_rotation(ground_rotation)
                .with_scale(Vec3::splat(radius * 0.9)),
            Name::new("Elemental area relationship inset"),
        ));
    }

    let count = area_accent_count(accent, signature.variant);
    let seed = signature.stable_seed();
    let offset = unit_sample(seed_for(seed, 31)) * core::f32::consts::TAU;
    for slot in 0..count {
        let angle = offset + f32::from(slot) * core::f32::consts::TAU / f32::from(count);
        let radial = radius * (0.76 + unit_sample(seed_for(seed, slot + 41)) * 0.12);
        let position = Vec3::new(angle.cos() * radial, 3.2, angle.sin() * radial);
        let transform = match accent {
            vfx::VfxAccent::Cold => Transform {
                translation: position,
                rotation: Quat::from_rotation_y(-angle),
                scale: Vec3::new(radius * 0.11, 2.0, radius * 0.035),
            },
            vfx::VfxAccent::Fire => Transform::from_translation(position + Vec3::Y * 1.2)
                .with_scale(Vec3::splat(radius * 0.065)),
            _ => unreachable!(),
        };
        parent.spawn((
            ElementalMotionAccent3d::new(accent, seed_for(seed, slot), transform),
            Mesh3d(if accent == vfx::VfxAccent::Cold {
                primitives.unit_cuboid.clone()
            } else {
                primitives.effect_sphere.clone()
            }),
            MeshMaterial3d(if slot % 2 == 0 {
                bright.clone()
            } else {
                soft.clone()
            }),
            NotShadowCaster,
            NotShadowReceiver,
            transform,
            Name::new(if accent == vfx::VfxAccent::Cold {
                "Cold area crystal accent"
            } else {
                "Fire area flame accent"
            }),
        ));
    }
    true
}

pub(in crate::client::presentation_3d) fn animate_elemental_motion_accents(
    ticks: Query<&AuthoritativeTick>,
    mut accents: Query<(&ElementalMotionAccent3d, &mut Transform)>,
) {
    let tick = ticks.iter().map(|tick| tick.0).max().unwrap_or_default();
    for (accent, mut transform) in &mut accents {
        let period = if accent.accent == vfx::VfxAccent::Cold {
            120
        } else {
            24
        };
        let phase_tick = u16::try_from((tick + accent.seed % period) % period)
            .expect("elemental accent phase is bounded by its short period");
        let period = u16::try_from(period).expect("elemental accent period fits u16");
        let progress = f32::from(phase_tick) / f32::from(period);
        match accent.accent {
            vfx::VfxAccent::Cold => {
                let wave = (progress * core::f32::consts::TAU).sin();
                transform.translation = accent.base_translation + Vec3::Y * (wave * 1.1);
                transform.rotation = accent.base_rotation * Quat::from_rotation_y(wave * 0.12);
                transform.scale = accent.base_scale * (0.94 + wave * 0.06);
            }
            vfx::VfxAccent::Fire => {
                transform.translation = accent.base_translation + Vec3::Y * (progress * 5.0);
                transform.rotation = accent.base_rotation;
                transform.scale = accent.base_scale * (1.15 - progress * 0.55);
            }
            _ => {}
        }
    }
}

impl ElementalMotionAccent3d {
    const fn new(accent: vfx::VfxAccent, seed: u64, transform: Transform) -> Self {
        Self {
            accent,
            seed,
            base_translation: transform.translation,
            base_rotation: transform.rotation,
            base_scale: transform.scale,
        }
    }
}

fn primary_element(signature: vfx::CombatVisualSignature) -> Option<vfx::VfxAccent> {
    signature
        .accents
        .into_iter()
        .flatten()
        .find(|accent| matches!(accent, vfx::VfxAccent::Cold | vfx::VfxAccent::Fire))
}

const fn is_reduced(variant: vfx::VfxPresentationVariant) -> bool {
    matches!(
        variant,
        vfx::VfxPresentationVariant::Reduced | vfx::VfxPresentationVariant::ReducedPrimitive
    )
}

const fn area_accent_count(accent: vfx::VfxAccent, variant: vfx::VfxPresentationVariant) -> u8 {
    match (accent, is_reduced(variant)) {
        (vfx::VfxAccent::Cold, false) => 6,
        (vfx::VfxAccent::Cold, true) => 3,
        (vfx::VfxAccent::Fire, false) => 8,
        (vfx::VfxAccent::Fire, true) => 4,
        _ => 0,
    }
}

fn relationship_material(
    relationship: vfx::VfxRelationship,
    materials: &Material3dAssets,
    neutral_fallback: Handle<StandardMaterial>,
) -> Handle<StandardMaterial> {
    match relationship {
        vfx::VfxRelationship::SelfOwned => materials.marker_local.clone(),
        vfx::VfxRelationship::Ally => materials.marker_ally.clone(),
        vfx::VfxRelationship::Enemy => materials.marker_enemy.clone(),
        vfx::VfxRelationship::Neutral => neutral_fallback,
    }
}

fn seed_for(seed: u64, slot: u8) -> u64 {
    vfx::vfx_counter_sample(
        seed,
        "durable.elemental",
        "mesh-accent",
        slot,
        u32::from(slot),
    )
}

fn unit_sample(seed: u64) -> f32 {
    let sample = u16::try_from(seed & 0xffff).expect("sample is masked to sixteen bits");
    f32::from(sample) / f32::from(u16::MAX)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stable_identity_drives_repeatable_bounded_offsets() {
        let seed = seed_for(77, 3);
        assert_eq!(seed, seed_for(77, 3));
        assert_ne!(seed, seed_for(78, 3));
        assert!((0.0..=1.0).contains(&unit_sample(seed)));
    }

    #[test]
    fn reduced_variants_preserve_a_bounded_elemental_identity_channel() {
        use vfx::VfxPresentationVariant::{Normal, Primitive, Reduced, ReducedPrimitive};

        assert_eq!(area_accent_count(vfx::VfxAccent::Cold, Normal), 6);
        assert_eq!(area_accent_count(vfx::VfxAccent::Cold, Primitive), 6);
        assert_eq!(area_accent_count(vfx::VfxAccent::Cold, Reduced), 3);
        assert_eq!(area_accent_count(vfx::VfxAccent::Cold, ReducedPrimitive), 3);
        assert_eq!(area_accent_count(vfx::VfxAccent::Fire, Normal), 8);
        assert_eq!(area_accent_count(vfx::VfxAccent::Fire, Reduced), 4);
        assert_eq!(area_accent_count(vfx::VfxAccent::Neutral, Normal), 0);
    }
}
