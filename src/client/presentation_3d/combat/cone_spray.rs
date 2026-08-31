use super::super::*;
use super::common::{CombatVfxContext, GROUND_EFFECT_HEIGHT};
use crate::combat::client::{
    AimTraceBlockerClass, AimTraceBlockerIndex, AimTraceDynamicBlocker, MAX_CONE_SPRAY_SEGMENTS,
    PreviewGeometry, cone_spray_primitives,
};
use std::collections::{HashMap, HashSet};

#[derive(Component)]
pub(in super::super) struct ConeSprayVisual3d;

#[derive(Component)]
pub(in super::super) struct ConeSpraySegmentVisual3d {
    owner: Entity,
    slot: u8,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "cone reconciliation joins one bounded authoritative carrier tuple with its visual root"
)]
pub(in super::super) fn reconcile_cone_spray_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    vfx: CombatVfxContext,
    sprays: Query<
        (
            Entity,
            &crate::combat::ConeSprayState,
            Option<&crate::combat::ReplicatedAttackSource>,
            Option<&crate::combat::AttackDelivery>,
        ),
        With<crate::combat::ConeSpray>,
    >,
    visuals: Query<(Entity, &CombatVisualOwner), With<ConeSprayVisual3d>>,
) {
    let roots = super::common::unique_roots(&mut commands, &visuals);
    for (owner, state, source, delivery) in &sprays {
        if roots.contains(&owner) {
            continue;
        }
        let geometry = vfx::VfxGeometry::Cone {
            origin: state.origin.as_vec2().to_array(),
            facing: state.facing,
            reach: state.maximum_reach,
            angle_degrees: state.angle_degrees,
        };
        let timing = vfx::VfxAuthoritativeTiming::persistent(
            state.emitted_at_tick,
            None,
            state.expires_at_tick,
        );
        let signature = source.map_or_else(
            || vfx::CombatVisualSignature {
                carrier: vfx::VfxCarrier::Cone,
                phase: vfx::VfxPhase::Active,
                accents: [Some(vfx::VfxAccent::Neutral), None],
                relationship: vfx::VfxRelationship::Neutral,
                context: None,
                geometry,
                identity: vfx::VfxPresentationIdentity::Neutral(state.emitted_at_tick),
                timing,
                variant: vfx.variant(),
            },
            |source| {
                vfx.cache().signature_for_source(
                    source.attack,
                    delivery.map_or(0, |delivery| delivery.delivery_index),
                    vfx::VfxCarrier::Cone,
                    vfx::VfxPhase::Active,
                    geometry,
                    timing,
                    vfx.variant(),
                )
            },
        );
        let root = commands
            .spawn((
                CombatVisualOwner(owner),
                ConeSprayVisual3d,
                signature,
                Transform::default(),
                Visibility::default(),
                Name::new("Spray gas visual root"),
            ))
            .id();
        commands.entity(root).with_children(|parent| {
            for slot in 0..u8::try_from(MAX_CONE_SPRAY_SEGMENTS)
                .expect("spray visual segment bound fits u8")
            {
                parent.spawn((
                    ConeSpraySegmentVisual3d { owner, slot },
                    Mesh3d(primitives.unit_cuboid.clone()),
                    MeshMaterial3d(materials.spray_gas.clone()),
                    NotShadowCaster,
                    NotShadowReceiver,
                    Transform::default(),
                    Visibility::Hidden,
                    Name::new("Spray gas clipped segment"),
                ));
            }
        });
    }
    for (root, owner) in &visuals {
        if sprays.get(owner.0).is_err() {
            commands.entity(root).despawn();
        }
    }
}

#[allow(
    clippy::too_many_arguments,
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "spray presentation reconstructs authoritative time against the current observed map and safe blockers"
)]
pub(in super::super) fn update_cone_spray_visuals(
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    mut index: ResMut<AimTraceBlockerIndex>,
    catalog: Res<crate::map::MapCatalogResource>,
    maps: Query<
        (
            &crate::map::ResolvedMapSnapshot,
            &crate::map::MapDynamicState,
        ),
        With<crate::map::MapRoot>,
    >,
    authoritative_ticks: Query<&AuthoritativeTick>,
    sprays: Query<&crate::combat::ConeSprayState>,
    safes: Query<(
        &Position,
        &Rotation,
        &crate::matchplay::HeistSafe,
        &crate::combat::CurrentHealth,
    )>,
    mut segments: Query<(
        &ConeSpraySegmentVisual3d,
        &mut Transform,
        &mut Visibility,
        &mut Mesh3d,
        &mut MeshMaterial3d<StandardMaterial>,
    )>,
) {
    if let Some((map, state)) = maps
        .iter()
        .max_by_key(|(snapshot, _)| snapshot.identity.instance_id)
    {
        index.refresh(map, state, &catalog.0);
    }
    let mut blockers = safes
        .iter()
        .filter(|(_, _, _, health)| health.0 > 0)
        .map(|(position, rotation, safe, _)| AimTraceDynamicBlocker {
            class: AimTraceBlockerClass::HeistSafe,
            stable_id: (safe.match_id.0 << 32) | u128::from(safe.anchor_id.0),
            position: position.0,
            rotation: rotation.as_radians(),
            shape: crate::map::MapShape::Rectangle {
                half_extents: crate::matchplay::HEIST_SAFE_HALF_EXTENTS,
            },
        })
        .collect::<Vec<_>>();
    blockers.sort_by_key(|blocker| blocker.stable_id);
    let observed_tick = authoritative_ticks.iter().map(|tick| tick.0).max();
    let owners = segments
        .iter()
        .map(|(segment, ..)| segment.owner)
        .collect::<HashSet<_>>();
    let primitives_by_owner = owners
        .into_iter()
        .filter_map(|owner| {
            sprays.get(owner).ok().map(|state| {
                let tick = observed_tick.unwrap_or(state.emitted_at_tick);
                (
                    owner,
                    cone_spray_primitives(*state, tick, &index, &blockers),
                )
            })
        })
        .collect::<HashMap<_, _>>();
    for (segment, mut transform, mut visibility, mut mesh, mut material) in &mut segments {
        let Some(primitives_for_spray) = primitives_by_owner.get(&segment.owner) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        let Some(primitive) = primitives_for_spray.get(usize::from(segment.slot)) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Inherited;
        material.0 = materials.spray_gas.clone();
        match primitive.geometry {
            PreviewGeometry::Corridor {
                center,
                angle,
                length,
                width,
            } => {
                mesh.0 = primitives.unit_cuboid.clone();
                transform.translation = ground_position(center) + Vec3::Y * GROUND_EFFECT_HEIGHT;
                transform.rotation = ground_rotation(Rotation::radians(angle));
                transform.scale = Vec3::new(length, 2.4, width.max(0.001));
            }
            PreviewGeometry::Disc { center, radius } => {
                mesh.0 = primitives.area_disc.clone();
                transform.translation = ground_position(center) + Vec3::Y * GROUND_EFFECT_HEIGHT;
                transform.rotation = Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2);
                transform.scale = Vec3::splat(radius);
            }
        }
    }
}
