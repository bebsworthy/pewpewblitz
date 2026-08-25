//! Pure geometry shared by server delivery systems and client previews.

#[allow(clippy::wildcard_imports)]
#[cfg(feature = "server")]
use super::*;
use bevy::prelude::Vec2;

#[must_use]
pub fn lob_height(progress: f32, visual_arc_height: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    4.0 * visual_arc_height * progress * (1.0 - progress)
}

#[must_use]
pub fn sector_contains(
    origin: Vec2,
    facing: f32,
    reach: f32,
    angle_degrees: f32,
    target_center: Vec2,
    target_radius: f32,
) -> bool {
    let delta = target_center - origin;
    let distance = delta.length();
    if !delta.is_finite() || !distance.is_finite() || distance > reach + target_radius {
        return false;
    }
    if distance <= f32::EPSILON {
        return true;
    }
    let half_angle = (angle_degrees.to_radians() / 2.0).clamp(0.0, std::f32::consts::PI);
    let angular_padding = (target_radius / distance).clamp(0.0, 1.0).asin();
    let difference = (delta.y.atan2(delta.x) - facing + std::f32::consts::PI)
        .rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    difference.abs() <= half_angle + angular_padding
}

#[must_use]
pub fn repaired_landing_point(
    launch: Vec2,
    desired: Vec2,
    minimum_distance: f32,
    mut is_clear: impl FnMut(Vec2) -> bool,
) -> Option<Vec2> {
    let ray = desired - launch;
    let distance = ray.length();
    if !distance.is_finite() || distance <= f32::EPSILON {
        return is_clear(launch).then_some(launch);
    }
    let direction = ray / distance;
    let minimum_distance = minimum_distance.clamp(0.0, distance);
    let mut furthest_clear = None;
    let mut blocked = distance;
    let mut sample = distance;
    for _ in 0..128 {
        let point = launch + direction * sample;
        if is_clear(point) {
            furthest_clear = Some(sample);
            break;
        }
        blocked = sample;
        if sample <= minimum_distance {
            break;
        }
        sample = (sample - 5.0).max(minimum_distance);
    }
    let mut clear = furthest_clear?;
    for _ in 0..8 {
        let middle = clear.midpoint(blocked);
        if is_clear(launch + direction * middle) {
            clear = middle;
        } else {
            blocked = middle;
        }
    }
    Some(launch + direction * clear)
}

#[cfg(feature = "server")]
pub(super) fn queue_damageable_target(
    world: &mut crate::map::PendingWorldTargetDamages,
    objectives: &mut crate::matchplay::PendingModeObjectiveDamages,
    request: crate::map::PendingWorldTargetDamage,
) {
    match request.target {
        crate::map::DamageableTargetIdentity::MapObject { .. } => world.0.push(request),
        crate::map::DamageableTargetIdentity::HeistSafe { .. } => {
            objectives
                .0
                .push(crate::matchplay::PendingModeObjectiveDamage {
                    target: request.target,
                    source: request.source,
                    requested_damage: request.requested_damage,
                    delivery_index: request.delivery_index,
                    bundle_index: request.bundle_index,
                    effect_index: request.effect_index,
                });
        }
    }
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime; the query declares this system's complete world view inline at its schedule boundary"
)]
pub(super) fn sweep_composed_projectiles(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    mut ids: ResMut<NextCombatIds>,
    mut trackers: ResMut<ActiveAttackTrackers>,
    mut telemetry: ResMut<WeaponTelemetry>,
    mut pending: MessageWriter<PendingPayload>,
    mut world_pending: ResMut<crate::map::PendingWorldTargetDamages>,
    mut objective_pending: ResMut<crate::matchplay::PendingModeObjectiveDamages>,
    mut deliveries: MessageWriter<PendingDelivery>,
    mut projectiles: Query<(
        Entity,
        &Position,
        &mut ComposedProjectileRuntime,
        Option<&LobbedFlight>,
    )>,
    fighters: Query<
        (
            Entity,
            &Position,
            &TeamId,
            &NetworkEntityId,
            Option<&Defeated>,
            Option<&lightyear::prelude::ControlledBy>,
        ),
        Or<(With<Fighter>, With<crate::abilities::Sentry>)>,
    >,
    objects: Query<
        (
            Entity,
            &Position,
            &crate::map::DamageableTargetIdentity,
            &CurrentHealth,
            &crate::map::DamageableLifeState,
        ),
        Or<(
            With<crate::map::DamageableWorldObject>,
            With<crate::matchplay::HeistSafe>,
        )>,
    >,
    disconnected: Query<Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
    walls: Query<Entity, With<ArenaWall>>,
    spatial_query: avian2d::prelude::SpatialQuery,
) {
    let disconnected: HashSet<_> = disconnected.iter().collect();
    let fighter_lookup: HashMap<_, _> = fighters
        .iter()
        .map(
            |(entity, position, team, network_id, defeated, controlled)| {
                (
                    entity,
                    (
                        position.0,
                        *team,
                        *network_id,
                        defeated.is_some(),
                        controlled
                            .is_some_and(|controlled| disconnected.contains(&controlled.owner)),
                    ),
                )
            },
        )
        .collect();
    let object_lookup: HashMap<_, _> = objects
        .iter()
        .map(|(entity, position, identity, health, life)| {
            (entity, (position.0, *identity, *health, *life))
        })
        .collect();
    // Static authoritative geometry stops projectiles: permanent map colliders (the
    // ArenaWall entities) and destructible chunk colliders alike, so cover works and
    // carved lanes are the only way through.
    let blocking_geometry: HashSet<Entity> = walls.iter().collect();
    let mut ordered: Vec<_> = projectiles.iter_mut().collect();
    ordered.sort_by_key(|(_, _, runtime, lob)| {
        (
            runtime.source.attack_id.0,
            runtime.delivery_index,
            lob.is_some(),
        )
    });
    for (entity, position, mut runtime, lob) in ordered {
        let Some((_, _, _, _, owner_disconnected)) = fighter_lookup
            .values()
            .find(|(_, _, network_id, _, _)| *network_id == runtime.source.owner_network_entity_id)
        else {
            record_delivery_termination(
                &mut ids,
                &mut telemetry,
                tick.0,
                &runtime,
                position.0,
                WeaponTelemetryOutcome::DeliveryCancelled,
            );
            commands.entity(entity).try_despawn();
            finish_attack_delivery(&mut trackers, runtime.source.attack_id);
            continue;
        };
        if *owner_disconnected {
            record_delivery_termination(
                &mut ids,
                &mut telemetry,
                tick.0,
                &runtime,
                position.0,
                WeaponTelemetryOutcome::DeliveryCancelled,
            );
            commands.entity(entity).try_despawn();
            finish_attack_delivery(&mut trackers, runtime.source.attack_id);
            continue;
        }
        if let Some(lob) = lob {
            if tick.0 < lob.lands_at_tick {
                let progress = (tick.0.saturating_sub(lob.launched_at_tick) as f32)
                    / (lob
                        .lands_at_tick
                        .saturating_sub(lob.launched_at_tick)
                        .max(1) as f32);
                let launch = lob.launch.as_vec2();
                let landing = lob.landing.as_vec2();
                commands
                    .entity(entity)
                    .insert(Position(launch.lerp(landing, progress.clamp(0.0, 1.0))));
                continue;
            }
            let landing = lob.landing.as_vec2();
            let _queued_payloads = queue_area_payloads(
                landing,
                runtime.source,
                runtime.delivery_index,
                &runtime.recipe,
                &fighters,
                &objects,
                &disconnected,
                &spatial_query,
                &mut pending,
                &mut world_pending,
                &mut objective_pending,
            );
            deliveries.write(PendingDelivery {
                entity: Some(entity),
                source: runtime.source,
                delivery_index: runtime.delivery_index,
                tick: tick.0,
                engagement_distance: 0.0,
                delivery_travel: lob_launch_point(runtime.source, &runtime.recipe)
                    .distance(landing),
                kind: PendingDeliveryKind::LobLanded {
                    position: WorldPoint::from(landing),
                },
                world_effects: runtime.recipe.world_effects.clone(),
            });
            continue;
        }
        if tick.0 >= runtime.expires_at_tick || runtime.travelled >= runtime.maximum_range {
            record_delivery_termination(
                &mut ids,
                &mut telemetry,
                tick.0,
                &runtime,
                position.0,
                WeaponTelemetryOutcome::DeliveryExpired,
            );
            commands.entity(entity).try_despawn();
            finish_attack_delivery(&mut trackers, runtime.source.attack_id);
            continue;
        }
        let step = (runtime.velocity.length() / 60.0)
            .min((runtime.maximum_range - runtime.travelled).max(0.0));
        let Some(direction) = Dir2::new(runtime.velocity.normalize_or_zero()).ok() else {
            record_delivery_termination(
                &mut ids,
                &mut telemetry,
                tick.0,
                &runtime,
                position.0,
                WeaponTelemetryOutcome::DeliveryCancelled,
            );
            commands.entity(entity).try_despawn();
            finish_attack_delivery(&mut trackers, runtime.source.attack_id);
            continue;
        };
        let filter = avian2d::prelude::SpatialQueryFilter::from_mask(
            FIGHTER_LAYER
                | crate::movement::DEPLOYABLE_LAYER
                | STATIC_MAP_LAYER
                | DESTRUCTIBLE_MAP_LAYER,
        )
        .with_excluded_entities([entity, runtime.owner_entity, runtime.source_entity]);
        let hit = spatial_query.cast_shape_predicate(
            &Collider::circle(runtime.radius),
            position.0,
            0.0,
            direction,
            &avian2d::prelude::ShapeCastConfig::from_max_distance(step),
            &filter,
            &|candidate| {
                fighter_lookup.get(&candidate).map_or_else(
                    || {
                        object_lookup.get(&candidate).map_or_else(
                            || blocking_geometry.contains(&candidate),
                            |(_, _, health, life)| crate::map::object_is_live(*health, *life),
                        )
                    },
                    |(_, team, _, defeated, owner_disconnected)| {
                        teams_are_hostile(runtime.source.team_id, *team)
                            && !defeated
                            && !owner_disconnected
                    },
                )
            },
        );
        let Some(hit) = hit else {
            runtime.travelled += step;
            commands
                .entity(entity)
                .insert(Position(position.0 + direction.as_vec2() * step));
            continue;
        };
        runtime.travelled += hit.distance.clamp(0.0, step);
        let target = fighter_lookup.get(&hit.entity).copied();
        if let Some((target_position, target_team, target_network_id, defeated, _)) = target
            && !defeated
            && teams_are_hostile(runtime.source.team_id, target_team)
        {
            for (bundle_index, bundle) in
                runtime
                    .recipe
                    .payload_bundles
                    .iter()
                    .enumerate()
                    .filter(|(_, bundle)| {
                        matches!(bundle.target, TargetSelection::Direct)
                            && payload_can_affect_target(
                                bundle,
                                runtime.source,
                                target_team,
                                target_network_id,
                            )
                    })
            {
                pending.write(PendingPayload {
                    source: runtime.source,
                    delivery_index: runtime.delivery_index,
                    bundle_index: u8::try_from(bundle_index).unwrap_or(u8::MAX),
                    target: hit.entity,
                    target_network_id,
                    position: hit.point2,
                    engagement_distance: runtime.source.origin.as_vec2().distance(target_position),
                    delivery_travel: runtime.travelled,
                    contact_fraction: (hit.distance / step.max(f32::EPSILON)).clamp(0.0, 1.0),
                    bundle: bundle.clone(),
                });
            }
        }
        if let Some((_, identity, health, life)) = object_lookup.get(&hit.entity).copied()
            && crate::map::object_is_live(health, life)
        {
            for (bundle_index, bundle) in runtime
                .recipe
                .payload_bundles
                .iter()
                .enumerate()
                .filter(|(_, bundle)| matches!(bundle.target, TargetSelection::Direct))
            {
                for (effect_index, effect) in bundle.effects.iter().enumerate() {
                    let PayloadEffectDefinition::Damage {
                        amount, falloff, ..
                    } = *effect
                    else {
                        continue;
                    };
                    queue_damageable_target(
                        &mut world_pending,
                        &mut objective_pending,
                        crate::map::PendingWorldTargetDamage {
                            target: identity,
                            source: runtime.source,
                            attack_id: runtime.source.attack_id,
                            requested_damage: effects::requested_damage(
                                amount,
                                falloff,
                                runtime.travelled,
                                1.0,
                                false,
                                runtime.source.origin.as_vec2().distance(hit.point2),
                            ),
                            delivery_index: runtime.delivery_index,
                            bundle_index: u8::try_from(bundle_index).unwrap_or(u8::MAX),
                            effect_index: u8::try_from(effect_index).unwrap_or(u8::MAX),
                        },
                    );
                }
            }
        }
        deliveries.write(PendingDelivery {
            entity: Some(entity),
            source: runtime.source,
            delivery_index: runtime.delivery_index,
            tick: tick.0,
            engagement_distance: target.map_or(0.0, |(position, ..)| {
                runtime.source.origin.as_vec2().distance(position)
            }),
            delivery_travel: runtime.travelled,
            kind: PendingDeliveryKind::StraightImpact {
                target: target.map(|(_, _, network_id, _, _)| network_id),
                position: WorldPoint::from(hit.point2),
                normal: WorldPoint::from(hit.normal1),
                distance_band: distance_band(runtime.travelled),
            },
            world_effects: runtime.recipe.world_effects.clone(),
        });
    }
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime; the query declares this system's complete world view inline at its schedule boundary"
)]
#[allow(
    clippy::too_many_lines,
    reason = "the authoritative melee system keeps combatant and world-object sector, occlusion, and shared payload planning together"
)]
pub(super) fn resolve_melee_attacks(
    mut attacks: MessageReader<MeleeAttack>,
    mut pending: MessageWriter<PendingPayload>,
    mut world_pending: ResMut<crate::map::PendingWorldTargetDamages>,
    mut objective_pending: ResMut<crate::matchplay::PendingModeObjectiveDamages>,
    mut deliveries: MessageWriter<PendingDelivery>,
    mut trackers: ResMut<ActiveAttackTrackers>,
    disconnected: Query<Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
    fighters: Query<
        (
            Entity,
            &Position,
            &TeamId,
            &NetworkEntityId,
            Option<&Defeated>,
            Option<&lightyear::prelude::ControlledBy>,
        ),
        Or<(With<Fighter>, With<crate::abilities::Sentry>)>,
    >,
    objects: Query<
        (
            Entity,
            &Position,
            &crate::map::DamageableTargetIdentity,
            &CurrentHealth,
            &crate::map::DamageableLifeState,
        ),
        Or<(
            With<crate::map::DamageableWorldObject>,
            With<crate::matchplay::HeistSafe>,
        )>,
    >,
    spatial_query: avian2d::prelude::SpatialQuery,
    tuning: Res<MovementTuning>,
) {
    let disconnected: HashSet<_> = disconnected.iter().collect();
    for attack in attacks.read() {
        let owner_connected = fighters.iter().any(|(_, _, _, network_id, _, controlled)| {
            *network_id == attack.source.owner_network_entity_id
                && controlled.is_none_or(|controlled| !disconnected.contains(&controlled.owner))
        });
        if !owner_connected {
            finish_attack_delivery(&mut trackers, attack.source.attack_id);
            continue;
        }
        let Some((reach, angle)) = (match attack.recipe.delivery {
            DeliveryMethod::MeleeArc {
                reach,
                angle_degrees,
            } => Some((reach, angle_degrees)),
            _ => None,
        }) else {
            continue;
        };
        let mut queued_payloads = false;
        let fighter_filter = avian2d::prelude::SpatialQueryFilter::from_mask(
            FIGHTER_LAYER | crate::movement::DEPLOYABLE_LAYER,
        );
        let mut candidates: Vec<_> = spatial_query
            .shape_intersections(
                &Collider::circle(reach),
                attack.origin,
                0.0,
                &fighter_filter,
            )
            .into_iter()
            .filter_map(|entity| fighters.get(entity).ok())
            .collect();
        candidates.sort_by_key(|(_, _, _, network_id, _, _)| network_id.0);
        for (target, position, team, network_id, defeated, controlled) in candidates {
            if defeated.is_some()
                || controlled.is_some_and(|controlled| disconnected.contains(&controlled.owner))
                || !payload_target_visible(attack.source, *team, *network_id)
                || !sector_contains(
                    attack.origin,
                    attack.facing,
                    reach,
                    angle,
                    position.0,
                    tuning.radius,
                )
                || !area_line_of_sight_clear(attack.origin, position.0, &spatial_query)
            {
                continue;
            }
            let valid_bundles: Vec<_> = attack
                .recipe
                .payload_bundles
                .iter()
                .enumerate()
                .filter(|(_, bundle)| {
                    matches!(bundle.target, TargetSelection::Direct)
                        && payload_can_affect_target(bundle, attack.source, *team, *network_id)
                })
                .collect();
            if valid_bundles.is_empty() {
                continue;
            }
            deliveries.write(PendingDelivery {
                entity: None,
                source: attack.source,
                delivery_index: 0,
                tick: attack.tick,
                engagement_distance: attack.origin.distance(position.0),
                delivery_travel: 0.0,
                kind: PendingDeliveryKind::MeleeContact {
                    target: *network_id,
                    position: WorldPoint::from(position.0),
                },
                world_effects: attack.recipe.world_effects.clone(),
            });
            for (bundle_index, bundle) in valid_bundles {
                pending.write(PendingPayload {
                    source: attack.source,
                    delivery_index: 0,
                    bundle_index: u8::try_from(bundle_index).unwrap_or(u8::MAX),
                    target,
                    target_network_id: *network_id,
                    position: position.0,
                    engagement_distance: attack.origin.distance(position.0),
                    delivery_travel: 0.0,
                    contact_fraction: 1.0,
                    bundle: bundle.clone(),
                });
                queued_payloads = true;
            }
        }
        let mut object_candidates: Vec<_> = objects
            .iter()
            .filter(|(_, position, _, health, life)| {
                crate::map::object_is_live(**health, **life)
                    && sector_contains(attack.origin, attack.facing, reach, angle, position.0, 16.0)
            })
            .collect();
        object_candidates.sort_by_key(|(_, _, identity, ..)| identity.stable_order_key());
        for (entity, position, identity, _, _) in object_candidates {
            if !area_line_of_sight_clear_excluding(
                attack.origin,
                position.0,
                entity,
                &spatial_query,
            ) {
                continue;
            }
            for (bundle_index, bundle) in attack
                .recipe
                .payload_bundles
                .iter()
                .enumerate()
                .filter(|(_, bundle)| matches!(bundle.target, TargetSelection::Direct))
            {
                for (effect_index, effect) in bundle.effects.iter().enumerate() {
                    let PayloadEffectDefinition::Damage {
                        amount, falloff, ..
                    } = *effect
                    else {
                        continue;
                    };
                    queue_damageable_target(
                        &mut world_pending,
                        &mut objective_pending,
                        crate::map::PendingWorldTargetDamage {
                            target: *identity,
                            source: attack.source,
                            attack_id: attack.source.attack_id,
                            requested_damage: effects::requested_damage(
                                amount,
                                falloff,
                                0.0,
                                1.0,
                                false,
                                attack.origin.distance(position.0),
                            ),
                            delivery_index: 0,
                            bundle_index: u8::try_from(bundle_index).unwrap_or(u8::MAX),
                            effect_index: u8::try_from(effect_index).unwrap_or(u8::MAX),
                        },
                    );
                }
            }
        }
        if !queued_payloads {
            finish_attack_delivery(&mut trackers, attack.source.attack_id);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arc_height_peaks_at_half_progress() {
        assert!((lob_height(0.5, 140.0) - 140.0).abs() < 0.001);
    }
    #[test]
    fn sector_includes_tangent_target_radius() {
        assert!(sector_contains(
            Vec2::ZERO,
            0.0,
            100.0,
            60.0,
            Vec2::new(90.0, 20.0),
            20.0
        ));
    }
    #[test]
    fn landing_repair_returns_furthest_clear_point() {
        let point =
            repaired_landing_point(Vec2::ZERO, Vec2::X * 20.0, 0.0, |p| p.x < 12.0).unwrap();
        assert!(point.x < 12.1 && point.x > 6.0);
    }
}
