use super::super::*;
use super::common::GROUND_EFFECT_HEIGHT;
use super::fighter_feedback::DashTrailVisual3d;
use super::fighter_ui::{FighterAmmoRowUi, FighterHealthFillUi, FighterOverheadUi};
use crate::combat::client::{
    AimTraceBlockerClass, AimTraceBlockerIndex, AimTraceDynamicBlocker, MAX_PREVIEW_SEGMENTS,
    PreviewGeometry, PreviewPrimitive, preview_primitives,
};

#[derive(Component)]
pub(in super::super) struct WeaponPreviewVisual3d {
    slot: u8,
}
type AimFighterQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static NetworkEntityId,
        &'static Position,
        &'static Rotation,
        Option<&'static crate::combat::Defeated>,
        Option<&'static crate::builds::AbilityState>,
        Option<&'static crate::builds::ResolvedMatchLoadout>,
        &'static crate::combat::TeamId,
        Has<Controlled>,
    ),
    With<Fighter>,
>;

type PreviewVisualQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static WeaponPreviewVisual3d,
        &'static mut Transform,
        &'static mut Visibility,
        &'static mut Mesh3d,
        &'static mut MeshMaterial3d<StandardMaterial>,
    ),
    (
        Without<FighterHealthFillUi>,
        Without<DashTrailVisual3d>,
        Without<FighterOverheadUi>,
        Without<FighterAmmoRowUi>,
    ),
>;

type ControlledAim<'a> = (
    Vec2,
    f32,
    &'a crate::builds::ResolvedMatchLoadout,
    Option<&'a crate::builds::AbilityState>,
);
type ActivePreviewMap<'a> = (
    &'a crate::map::ResolvedMapSnapshot,
    &'a crate::map::MapDynamicState,
);

type SentryAimQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static crate::abilities::SentryIdentity,
        &'static crate::combat::CurrentHealth,
    ),
    With<crate::abilities::Sentry>,
>;

type SafeAimQuery<'w, 's> = Query<
    'w,
    's,
    (
        &'static Position,
        &'static Rotation,
        &'static crate::matchplay::HeistSafe,
        &'static crate::combat::CurrentHealth,
    ),
>;

#[derive(bevy::ecs::system::SystemParam)]
pub(in super::super) struct AimPreviewInputs<'w, 's> {
    maps: Query<
        'w,
        's,
        (
            &'static crate::map::ResolvedMapSnapshot,
            &'static crate::map::MapDynamicState,
        ),
        With<crate::map::MapRoot>,
    >,
    catalog: Res<'w, crate::map::MapCatalogResource>,
    pending: Res<'w, PendingLocalActions>,
    index: ResMut<'w, AimTraceBlockerIndex>,
    sentries: SentryAimQuery<'w, 's>,
    safes: SafeAimQuery<'w, 's>,
}

fn ensure_targeting_previews(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    previews: &Query<&WeaponPreviewVisual3d>,
) {
    if !previews.is_empty() {
        return;
    }
    for slot in 0..u8::try_from(MAX_PREVIEW_SEGMENTS).expect("preview slot bound fits u8") {
        commands.spawn((
            WeaponPreviewVisual3d { slot },
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(materials.preview.clone()),
            NotShadowCaster,
            Transform::default(),
            Visibility::Hidden,
            Name::new("V3 weapon preview slot"),
        ));
    }
    commands.spawn((
        WeaponPreviewVisual3d { slot: u8::MAX },
        Mesh3d(primitives.area_ring.clone()),
        MeshMaterial3d(materials.preview.clone()),
        NotShadowCaster,
        NotShadowReceiver,
        Transform::default(),
        Visibility::Hidden,
        Name::new("V9 targeted ultimate area ring"),
    ));
}

#[allow(clippy::needless_pass_by_value)]
pub(in super::super) fn reconcile_aim_preview_visuals(
    mut commands: Commands,
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    previews: Query<&WeaponPreviewVisual3d>,
) {
    ensure_targeting_previews(&mut commands, &primitives, &materials, &previews);
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::type_complexity,
    reason = "aim preview owns controlled-loadout geometry and the complete bounded blocker set"
)]
pub(in super::super) fn update_aim_preview(
    primitives: Res<Primitive3dAssets>,
    materials: Res<Material3dAssets>,
    mut aim: AimPreviewInputs,
    fighters: AimFighterQuery,
    mut previews: PreviewVisualQuery,
) {
    let mut controlled = None;
    let mut dynamic_blockers = Vec::new();
    let controlled_team = fighters
        .iter()
        .find_map(|(_, _, _, _, _, _, team, is_controlled)| is_controlled.then_some(*team));
    for (network_id, position, rotation, defeated, ability, loadout, team, is_controlled) in
        &fighters
    {
        if defeated.is_none()
            && controlled_team.is_some_and(|controlled_team| {
                crate::combat::teams_are_hostile(controlled_team, *team)
            })
        {
            dynamic_blockers.push(AimTraceDynamicBlocker {
                class: AimTraceBlockerClass::Fighter,
                stable_id: u128::from(network_id.0),
                position: position.0,
                rotation: 0.0,
                shape: crate::map::MapShape::Circle {
                    radius: crate::movement::STANDARD_FIGHTER_RADIUS,
                },
            });
        }
        if is_controlled {
            controlled =
                loadout.map(|loadout| (position.0, rotation.as_radians(), loadout, ability));
        }
    }

    append_nonfighter_aim_blockers(
        &aim.sentries,
        &aim.safes,
        controlled_team,
        &mut dynamic_blockers,
    );
    dynamic_blockers.sort_by_key(|blocker| (blocker.class, blocker.stable_id));

    let map = aim
        .maps
        .iter()
        .max_by_key(|(snapshot, _)| snapshot.identity.instance_id);
    if let Some((snapshot, state)) = map {
        aim.index.refresh(snapshot, state, &aim.catalog.0);
    }
    let scan_preview = targeted_ultimate_preview(map, controlled, &aim.pending);
    let segments = if let Some((origin, center, _, _)) = scan_preview {
        let delta = center - origin;
        vec![PreviewPrimitive {
            geometry: PreviewGeometry::Corridor {
                center: origin.midpoint(center),
                angle: delta.y.atan2(delta.x),
                length: delta.length().max(0.001),
                width: 3.0,
            },
            blocked: false,
        }]
    } else {
        match (map, controlled) {
            (Some((map, state)), Some((origin, facing, loadout, _))) => preview_primitives(
                origin,
                facing,
                aim.pending.aim_distance,
                &loadout.primary_weapon,
                map,
                state,
                &aim.catalog.0,
                &aim.index,
                &dynamic_blockers,
            ),
            _ => Vec::new(),
        }
    };
    apply_aim_preview_visuals(
        &mut previews,
        &primitives,
        &materials,
        scan_preview,
        &segments,
    );
}

fn append_nonfighter_aim_blockers(
    sentries: &SentryAimQuery,
    safes: &SafeAimQuery,
    controlled_team: Option<crate::combat::TeamId>,
    blockers: &mut Vec<AimTraceDynamicBlocker>,
) {
    for (position, identity, health) in sentries.iter() {
        if health.0 > 0
            && controlled_team.is_some_and(|controlled_team| {
                crate::combat::teams_are_hostile(controlled_team, identity.team_id)
            })
        {
            blockers.push(AimTraceDynamicBlocker {
                class: AimTraceBlockerClass::Sentry,
                stable_id: u128::from(identity.deployable_id.0),
                position: position.0,
                rotation: 0.0,
                shape: crate::map::MapShape::Circle {
                    radius: crate::abilities::SENTRY_RADIUS,
                },
            });
        }
    }
    for (position, rotation, safe, health) in safes.iter() {
        if health.0 > 0 {
            blockers.push(AimTraceDynamicBlocker {
                class: AimTraceBlockerClass::HeistSafe,
                stable_id: (safe.match_id.0 << 32) | u128::from(safe.anchor_id.0),
                position: position.0,
                rotation: rotation.as_radians(),
                shape: crate::map::MapShape::Rectangle {
                    half_extents: crate::matchplay::HEIST_SAFE_HALF_EXTENTS,
                },
            });
        }
    }
}

fn targeted_ultimate_preview(
    map: Option<ActivePreviewMap<'_>>,
    controlled: Option<ControlledAim<'_>>,
    pending: &PendingLocalActions,
) -> Option<(Vec2, Vec2, f32, bool)> {
    match (map, controlled) {
        (Some((map, _)), Some((origin, facing, loadout, Some(ability))))
            if matches!(
                loadout.ultimate.kind,
                crate::builds::UltimateKind::RevealScan
                    | crate::builds::UltimateKind::ConcealmentField
                    | crate::builds::UltimateKind::DemolitionStrike
                    | crate::builds::UltimateKind::CryogenicField
                    | crate::builds::UltimateKind::FireField
                    | crate::builds::UltimateKind::PoisonField
                    | crate::builds::UltimateKind::RestorationField
                    | crate::builds::UltimateKind::BigBlob
            ) && pending.targeted_ultimate.is_targeting(loadout.ultimate.id)
                && matches!(ability.phase, crate::builds::AbilityPhase::Ready) =>
        {
            let (
                crate::builds::UltimateParameters::RevealScan {
                    maximum_range_milliunits,
                    radius_milliunits,
                    ..
                }
                | crate::builds::UltimateParameters::ConcealmentField {
                    maximum_range_milliunits,
                    radius_milliunits,
                    ..
                }
                | crate::builds::UltimateParameters::DemolitionStrike {
                    maximum_range_milliunits,
                    radius_milliunits,
                }
                | crate::builds::UltimateParameters::ElementalField {
                    maximum_range_milliunits,
                    radius_milliunits,
                    ..
                }
                | crate::builds::UltimateParameters::BigBlob {
                    maximum_range_milliunits,
                    child_explosion_radius_milliunits: radius_milliunits,
                    ..
                },
            ) = (loadout.ultimate.parameters,)
            else {
                unreachable!()
            };
            crate::builds::world_units_from_milliunits(maximum_range_milliunits)
                .and_then(|maximum_range| {
                    crate::abilities::targeted_ultimate_center(
                        origin,
                        Vec2::from_angle(facing),
                        pending.aim_axis,
                        pending.aim_distance,
                        maximum_range,
                        map.dimensions.bounds(),
                    )
                })
                .zip(crate::builds::world_units_from_milliunits(
                    radius_milliunits,
                ))
                .map(|(center, radius)| {
                    (
                        origin,
                        center,
                        radius,
                        loadout.ultimate.kind == crate::builds::UltimateKind::DemolitionStrike,
                    )
                })
        }
        _ => None,
    }
}

fn apply_aim_preview_visuals(
    previews: &mut PreviewVisualQuery,
    primitives: &Primitive3dAssets,
    materials: &Material3dAssets,
    scan_preview: Option<(Vec2, Vec2, f32, bool)>,
    segments: &[PreviewPrimitive],
) {
    for (slot, mut transform, mut visibility, mut mesh, mut material) in previews.iter_mut() {
        if slot.slot == u8::MAX {
            let Some((_, center, radius, demolition)) = scan_preview else {
                *visibility = Visibility::Hidden;
                continue;
            };
            *visibility = Visibility::Inherited;
            transform.translation = ground_position(center) + Vec3::Y * GROUND_EFFECT_HEIGHT;
            transform.rotation = Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2);
            transform.scale = Vec3::splat(radius);
            material.0 = if demolition {
                materials.demolition_area.clone()
            } else {
                materials.preview.clone()
            };
            continue;
        }
        let Some(primitive) = segments.get(usize::from(slot.slot)) else {
            *visibility = Visibility::Hidden;
            continue;
        };
        *visibility = Visibility::Inherited;
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
                transform.scale = Vec3::new(length, 1.2, width.max(0.001));
            }
            PreviewGeometry::Disc { center, radius } => {
                mesh.0 = primitives.area_disc.clone();
                transform.translation = ground_position(center) + Vec3::Y * GROUND_EFFECT_HEIGHT;
                transform.rotation = Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2);
                transform.scale = Vec3::splat(radius);
            }
        }
        material.0 = if primitive.blocked {
            materials.preview_blocked.clone()
        } else {
            materials.preview.clone()
        };
    }
}
