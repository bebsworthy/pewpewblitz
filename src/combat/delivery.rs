//! Pure geometry shared by server delivery systems and client previews.

#[allow(clippy::wildcard_imports)]
#[cfg(feature = "server")]
use super::*;
use bevy::prelude::Vec2;

#[must_use]
#[cfg(feature = "client")]
pub fn lob_height(progress: f32, visual_arc_height: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    4.0 * visual_arc_height * progress * (1.0 - progress)
}

#[must_use]
#[cfg(feature = "server")]
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
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct ProjectileEnvironmentState<'w, 's> {
    active_splashes: Query<'w, 's, &'static PersistentSplashRuntime>,
    roots: Query<'w, 's, &'static crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>,
    walls: Query<'w, 's, Entity, With<ArenaWall>>,
}

#[cfg(feature = "server")]
type FighterSweepSnapshot = (Vec2, TeamId, NetworkEntityId, bool, bool);

#[cfg(feature = "server")]
type ObjectSweepSnapshot = (
    Vec2,
    crate::map::DamageableTargetIdentity,
    CurrentHealth,
    crate::map::DamageableLifeState,
);

#[cfg(feature = "server")]
struct ProjectileSweepIndex {
    fighters: HashMap<Entity, FighterSweepSnapshot>,
    objects: HashMap<Entity, ObjectSweepSnapshot>,
    blocking_geometry: HashSet<Entity>,
}

#[cfg(feature = "server")]
impl ProjectileSweepIndex {
    fn owner_is_connected(&self, owner: NetworkEntityId) -> bool {
        self.fighters
            .values()
            .any(|(_, _, network_id, _, disconnected)| *network_id == owner && !disconnected)
    }

    fn accepts_candidate(&self, candidate: Entity, runtime: &ComposedProjectileRuntime) -> bool {
        self.fighters.get(&candidate).map_or_else(
            || {
                self.objects.get(&candidate).map_or_else(
                    || self.blocking_geometry.contains(&candidate),
                    |(_, _, health, life)| crate::map::object_is_live(*health, *life),
                )
            },
            |(_, team, network_id, defeated, disconnected)| {
                let has_contact_delivery = runtime.recipe.payload_bundles.iter().any(|bundle| {
                    matches!(bundle.target, TargetSelection::Direct)
                        || (matches!(
                            runtime.recipe.delivery,
                            DeliveryMethod::StickyStraight { .. }
                        ) && matches!(bundle.target, TargetSelection::Area { .. }))
                });
                let has_affecting_payload = runtime.recipe.payload_bundles.iter().any(|bundle| {
                    (matches!(bundle.target, TargetSelection::Direct)
                        || matches!(bundle.target, TargetSelection::Area { .. }))
                        && payload_can_affect_target(bundle, runtime.source, *team, *network_id)
                });
                has_contact_delivery && has_affecting_payload && !defeated && !disconnected
            },
        )
    }
}

#[cfg(feature = "server")]
enum LobTrajectoryPlan {
    InFlight(Vec2),
    Landed(Vec2),
}

#[cfg(feature = "server")]
fn plan_lob_trajectory(tick: u64, lob: &LobbedFlight) -> LobTrajectoryPlan {
    let launch = lob.launch.as_vec2();
    let landing = lob.landing.as_vec2();
    if tick >= lob.lands_at_tick {
        return LobTrajectoryPlan::Landed(landing);
    }
    let progress = (tick.saturating_sub(lob.launched_at_tick) as f32)
        / (lob
            .lands_at_tick
            .saturating_sub(lob.launched_at_tick)
            .max(1) as f32);
    LobTrajectoryPlan::InFlight(launch.lerp(landing, progress.clamp(0.0, 1.0)))
}

#[cfg(feature = "server")]
struct StraightTrajectoryPlan {
    body: ProjectileBody,
    step: f32,
    direction: Dir2,
}

#[cfg(feature = "server")]
fn plan_straight_trajectory(
    runtime: &ComposedProjectileRuntime,
    body: Option<&ProjectileBody>,
) -> Option<StraightTrajectoryPlan> {
    let body = body.copied().filter(|body| body.shape.is_valid())?;
    let step = (runtime.velocity.length() / crate::timing::SIMULATION_TICK_HZ as f32)
        .min((runtime.maximum_range - runtime.travelled).max(0.0));
    let direction = Dir2::new(runtime.velocity.normalize_or_zero()).ok()?;
    Some(StraightTrajectoryPlan {
        body,
        step,
        direction,
    })
}

#[cfg(feature = "server")]
struct ProjectileTerminationContext<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    ids: &'a mut NextCombatIds,
    trackers: &'a mut ActiveAttackTrackers,
    telemetry: &'a mut WeaponTelemetry,
}

#[cfg(feature = "server")]
impl ProjectileTerminationContext<'_, '_, '_> {
    fn commit(
        &mut self,
        tick: u64,
        entity: Entity,
        position: Vec2,
        runtime: &ComposedProjectileRuntime,
        outcome: WeaponTelemetryOutcome,
    ) {
        record_delivery_termination(self.ids, self.telemetry, tick, runtime, position, outcome);
        self.commands.entity(entity).try_despawn();
        finish_attack_delivery(self.trackers, runtime.source.attack_id);
    }
}

#[cfg(feature = "server")]
fn direct_world_damage_requests(
    runtime: &ComposedProjectileRuntime,
    target: crate::map::DamageableTargetIdentity,
    hit_point: Vec2,
) -> Vec<crate::map::PendingWorldTargetDamage> {
    let mut requests = Vec::new();
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
            requests.push(crate::map::PendingWorldTargetDamage {
                target,
                source: runtime.source,
                attack_id: runtime.source.attack_id,
                requested_damage: effects::requested_damage(
                    amount,
                    falloff,
                    runtime.travelled,
                    1.0,
                    None,
                    runtime.source.origin.as_vec2().distance(hit_point),
                ),
                delivery_index: runtime.delivery_index,
                bundle_index: u8::try_from(bundle_index).unwrap_or(u8::MAX),
                effect_index: u8::try_from(effect_index).unwrap_or(u8::MAX),
            });
        }
    }
    requests
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_arguments)]
fn queue_direct_fighter_payloads(
    pending: &mut MessageWriter<PendingPayload>,
    runtime: &ComposedProjectileRuntime,
    target_entity: Entity,
    target: FighterSweepSnapshot,
    hit_point: Vec2,
    hit_distance: f32,
    step: f32,
) {
    let (target_position, target_team, target_network_id, defeated, _) = target;
    if defeated {
        return;
    }
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
            target: target_entity,
            target_network_id,
            position: hit_point,
            engagement_distance: runtime.source.origin.as_vec2().distance(target_position),
            delivery_travel: runtime.travelled,
            contact_fraction: (hit_distance / step.max(f32::EPSILON)).clamp(0.0, 1.0),
            bundle: bundle.clone(),
        });
    }
}

#[cfg(feature = "server")]
fn write_straight_impact(
    deliveries: &mut MessageWriter<PendingDelivery>,
    runtime: &ComposedProjectileRuntime,
    entity: Entity,
    target: Option<FighterSweepSnapshot>,
    tick: u64,
    point: Vec2,
    normal: Vec2,
) {
    deliveries.write(PendingDelivery {
        entity: Some(entity),
        source: runtime.source,
        delivery_index: runtime.delivery_index,
        tick,
        engagement_distance: target.map_or(0.0, |(position, ..)| {
            runtime.source.origin.as_vec2().distance(position)
        }),
        delivery_travel: runtime.travelled,
        kind: PendingDeliveryKind::StraightImpact {
            target: target.map(|(_, _, network_id, _, _)| network_id),
            position: WorldPoint::from(point),
            normal: WorldPoint::from(normal),
            distance_band: distance_band(runtime.travelled),
        },
        world_effects: runtime.recipe.world_effects.clone(),
    });
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
        Option<&ProjectileBody>,
        Option<&LobbedFlight>,
    )>,
    mut sticky_blobs: Query<(&mut StickyBlobState, &StickyBlobRuntime)>,
    environment: ProjectileEnvironmentState,
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
    spatial_query: avian2d::prelude::SpatialQuery,
) {
    let disconnected: HashSet<_> = disconnected.iter().collect();
    let fighter_lookup = fighters
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
    let object_lookup = objects
        .iter()
        .map(|(entity, position, identity, health, life)| {
            (entity, (position.0, *identity, *health, *life))
        })
        .collect();
    // Static authoritative geometry stops projectiles: permanent map colliders (the
    // ArenaWall entities) and destructible chunk colliders alike, so cover works and
    // carved lanes are the only way through.
    let index = ProjectileSweepIndex {
        fighters: fighter_lookup,
        objects: object_lookup,
        blocking_geometry: environment.walls.iter().collect(),
    };
    let mut sticky_sweep = sticky::StickySweepState::from_active(&sticky_blobs);
    let mut ordered: Vec<_> = projectiles.iter_mut().collect();
    ordered.sort_by_key(|(_, _, runtime, _, lob)| {
        (
            runtime.source.attack_id.0,
            runtime.delivery_index,
            lob.is_some(),
        )
    });
    for (entity, position, mut runtime, body, lob) in ordered {
        if !index.owner_is_connected(runtime.source.owner_network_entity_id) {
            ProjectileTerminationContext {
                commands: &mut commands,
                ids: &mut ids,
                trackers: &mut trackers,
                telemetry: &mut telemetry,
            }
            .commit(
                tick.0,
                entity,
                position.0,
                &runtime,
                WeaponTelemetryOutcome::DeliveryCancelled,
            );
            continue;
        }
        if let Some(lob) = lob {
            let landing = match plan_lob_trajectory(tick.0, lob) {
                LobTrajectoryPlan::InFlight(position) => {
                    commands.entity(entity).insert(Position(position));
                    continue;
                }
                LobTrajectoryPlan::Landed(landing) => landing,
            };
            if let DeliveryMethod::Splash {
                shape,
                duration_ticks,
                pulse_interval_ticks,
                map_occlusion,
                max_targets,
                max_active_per_owner,
                ..
            } = runtime.recipe.delivery
            {
                let owner_active = environment
                    .active_splashes
                    .iter()
                    .filter(|splash| {
                        splash.source.owner_network_entity_id
                            == runtime.source.owner_network_entity_id
                    })
                    .count();
                if owner_active >= usize::from(max_active_per_owner)
                    || environment.active_splashes.iter().count()
                        >= splash::MAX_ACTIVE_PERSISTENT_SPLASHES
                {
                    record_delivery_termination(
                        &mut ids,
                        &mut telemetry,
                        tick.0,
                        &runtime,
                        landing,
                        WeaponTelemetryOutcome::DeliveryCancelled,
                    );
                    commands.entity(entity).try_despawn();
                    splash::settle_unresolved_splash(&mut trackers, runtime.source.attack_id);
                    continue;
                }
                let (expires_at_tick, _) =
                    splash::splash_timing(tick.0, duration_ticks, pulse_interval_ticks);
                let mut splash_entity = commands.spawn((
                    PersistentSplash,
                    PersistentSplashState {
                        center: WorldPoint::from(landing),
                        facing: runtime.source.facing,
                        shape,
                        activated_at_tick: tick.0,
                        next_pulse_tick: tick.0,
                        expires_at_tick,
                        pulse_interval_ticks,
                        map_occlusion,
                        max_targets,
                        effects: splash::presentation_effects(&runtime.recipe),
                    },
                    ReplicatedAttackSource {
                        attack: runtime.source,
                    },
                    PersistentSplashRuntime {
                        source: runtime.source,
                        recipe: runtime.recipe.clone(),
                        next_delivery_index: 1,
                        match_id: environment.roots.single().ok().map(|root| root.match_id),
                    },
                    Replicate::to_clients(NetworkTarget::All),
                ));
                if let Ok(root) = environment.roots.single() {
                    splash_entity.insert(crate::matchplay::MatchMember(root.match_id));
                }
                commands.entity(entity).try_despawn();
                deliveries.write(PendingDelivery {
                    entity: None,
                    source: runtime.source,
                    delivery_index: 0,
                    tick: tick.0,
                    engagement_distance: 0.0,
                    delivery_travel: lob_launch_point(runtime.source, &runtime.recipe)
                        .distance(landing),
                    kind: PendingDeliveryKind::LobLanded {
                        position: WorldPoint::from(landing),
                    },
                    world_effects: Vec::new(),
                });
                continue;
            }
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
            if sticky_sweep.try_arm_expired(&mut commands, entity, &runtime, position.0, tick.0) {
                continue;
            }
            ProjectileTerminationContext {
                commands: &mut commands,
                ids: &mut ids,
                trackers: &mut trackers,
                telemetry: &mut telemetry,
            }
            .commit(
                tick.0,
                entity,
                position.0,
                &runtime,
                WeaponTelemetryOutcome::DeliveryExpired,
            );
            continue;
        }
        let Some(trajectory) = plan_straight_trajectory(&runtime, body) else {
            ProjectileTerminationContext {
                commands: &mut commands,
                ids: &mut ids,
                trackers: &mut trackers,
                telemetry: &mut telemetry,
            }
            .commit(
                tick.0,
                entity,
                position.0,
                &runtime,
                WeaponTelemetryOutcome::DeliveryCancelled,
            );
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
            &trajectory.body.collider(),
            position.0,
            0.0,
            trajectory.direction,
            &avian2d::prelude::ShapeCastConfig::from_max_distance(trajectory.step),
            &filter,
            &|candidate| index.accepts_candidate(candidate, &runtime),
        );
        let Some(hit) = hit else {
            runtime.travelled += trajectory.step;
            commands.entity(entity).insert(Position(
                position.0 + trajectory.direction.as_vec2() * trajectory.step,
            ));
            continue;
        };
        runtime.travelled += hit.distance.clamp(0.0, trajectory.step);
        let target = index.fighters.get(&hit.entity).copied();
        let attached_to =
            target.and_then(|(_, _, network_id, defeated, _)| (!defeated).then_some(network_id));
        let armed_position = target.map_or(hit.point2, |(position, ..)| position);
        if sticky_sweep.try_arm_impact(
            &mut commands,
            entity,
            &runtime,
            armed_position,
            attached_to,
            tick.0,
            &mut sticky_blobs,
        ) {
            continue;
        }
        if let Some(target) = target {
            queue_direct_fighter_payloads(
                &mut pending,
                &runtime,
                hit.entity,
                target,
                hit.point2,
                hit.distance,
                trajectory.step,
            );
        }
        if let Some((_, identity, health, life)) = index.objects.get(&hit.entity).copied()
            && crate::map::object_is_live(health, life)
        {
            for request in direct_world_damage_requests(&runtime, identity, hit.point2) {
                queue_damageable_target(&mut world_pending, &mut objective_pending, request);
            }
        }
        write_straight_impact(
            &mut deliveries,
            &runtime,
            entity,
            target,
            tick.0,
            hit.point2,
            hit.normal1,
        );
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
    builds: Res<crate::builds::BuildCatalogResource>,
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
                    builds.0.fighter_body.radius,
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
                                None,
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

    #[cfg(feature = "server")]
    fn sticky_sweep_app(tick: u64) -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(avian2d::prelude::PhysicsPlugins::default())
            .insert_resource(SimulationTick(tick))
            .init_resource::<NextCombatIds>()
            .init_resource::<ActiveAttackTrackers>()
            .init_resource::<WeaponTelemetry>()
            .init_resource::<crate::map::PendingWorldTargetDamages>()
            .init_resource::<crate::matchplay::PendingModeObjectiveDamages>()
            .add_message::<PendingPayload>()
            .add_message::<PendingDelivery>()
            .add_systems(
                FixedPostUpdate,
                (
                    sweep_composed_projectiles
                        .after(avian2d::prelude::PhysicsSystems::StepSimulation),
                    ApplyDeferred.after(sweep_composed_projectiles),
                ),
            );
        app.finish();
        app.cleanup();
        app.update();
        app
    }

    #[cfg(feature = "server")]
    fn sticky_recipe() -> WeaponRecipe {
        WeaponCatalog::embedded()
            .unwrap()
            .preset(WeaponPresetId(5))
            .unwrap()
            .configuration
            .recipe
            .clone()
    }

    #[cfg(feature = "server")]
    fn sticky_attack_source(attack_id: u64, owner: NetworkEntityId) -> AttackSource {
        AttackSource {
            kind: CombatSourceKind::PrimaryWeapon,
            attack_id: AttackId(attack_id),
            player_id: PlayerId(owner.0),
            owner_network_entity_id: owner,
            team_id: TeamId(0),
            recipe_fingerprint: WeaponRecipeFingerprint(11),
            legacy_compatibility: false,
            source_preset_id: Some(WeaponPresetId(5)),
            origin: WorldPoint::from(Vec2::ZERO),
            facing: 0.0,
        }
    }

    #[cfg(feature = "server")]
    fn spawn_sticky_owner(app: &mut App, owner: NetworkEntityId) -> Entity {
        app.world_mut()
            .spawn((Fighter, Position(Vec2::ZERO), TeamId(0), owner))
            .id()
    }

    #[cfg(feature = "server")]
    fn spawn_sticky_projectile(
        app: &mut App,
        owner_entity: Entity,
        source: AttackSource,
        position: Vec2,
        expires_at_tick: u64,
    ) -> Entity {
        let recipe = sticky_recipe();
        app.world_mut()
            .spawn((
                Projectile,
                Position(position),
                ProjectileBody::circle(2.0),
                Collider::circle(2.0),
                CollisionLayers::new(
                    PROJECTILE_LAYER,
                    FIGHTER_LAYER | STATIC_MAP_LAYER | DESTRUCTIBLE_MAP_LAYER,
                ),
                ComposedProjectileRuntime {
                    owner_entity,
                    source_entity: owner_entity,
                    source,
                    delivery_index: 0,
                    velocity: Vec2::new(600.0, 0.0),
                    travelled: 0.0,
                    expires_at_tick,
                    maximum_range: 1_000.0,
                    landing: None,
                    recipe,
                },
            ))
            .id()
    }

    #[cfg(feature = "server")]
    fn run_sticky_sweep(app: &mut App) {
        app.world_mut().run_schedule(FixedPostUpdate);
    }

    #[cfg(feature = "server")]
    fn test_attack_source() -> AttackSource {
        AttackSource {
            kind: CombatSourceKind::PrimaryWeapon,
            attack_id: AttackId(41),
            player_id: PlayerId(3),
            owner_network_entity_id: NetworkEntityId(7),
            team_id: TeamId(0),
            recipe_fingerprint: WeaponRecipeFingerprint(11),
            legacy_compatibility: false,
            source_preset_id: Some(WeaponPresetId(1)),
            origin: WorldPoint::from(Vec2::ZERO),
            facing: 0.0,
        }
    }

    #[cfg(feature = "server")]
    fn target_damage(
        target: crate::map::DamageableTargetIdentity,
    ) -> crate::map::PendingWorldTargetDamage {
        crate::map::PendingWorldTargetDamage {
            target,
            source: test_attack_source(),
            attack_id: AttackId(41),
            requested_damage: 17,
            delivery_index: 2,
            bundle_index: 3,
            effect_index: 4,
        }
    }

    #[test]
    #[cfg(feature = "server")]
    fn projectile_world_hits_route_to_exactly_one_authority_owner() {
        use crate::map::{
            DamageableTargetIdentity, MapDynamicGeneration, MapInstanceId, MapPlacementId,
            ModeAnchorId,
        };
        use crate::matchplay::MatchId;

        let map_target = DamageableTargetIdentity::MapObject {
            generation: MapDynamicGeneration {
                map_instance_id: MapInstanceId(9),
                generation: 2,
            },
            placement_id: MapPlacementId(12),
        };
        let safe_target = DamageableTargetIdentity::HeistSafe {
            match_id: MatchId(13),
            anchor_id: ModeAnchorId(4),
            defending_team: TeamId(1),
        };
        let mut world = crate::map::PendingWorldTargetDamages::default();
        let mut objectives = crate::matchplay::PendingModeObjectiveDamages::default();

        queue_damageable_target(&mut world, &mut objectives, target_damage(map_target));
        queue_damageable_target(&mut world, &mut objectives, target_damage(safe_target));

        assert_eq!(world.0.len(), 1);
        assert_eq!(world.0[0].target, map_target);
        assert_eq!(objectives.0.len(), 1);
        assert_eq!(objectives.0[0].target, safe_target);
        assert_eq!(objectives.0[0].requested_damage, 17);
        assert_eq!(objectives.0[0].delivery_index, 2);
        assert_eq!(objectives.0[0].bundle_index, 3);
        assert_eq!(objectives.0[0].effect_index, 4);
    }

    #[test]
    #[cfg(feature = "server")]
    fn production_sweep_arms_an_expired_sticky_at_its_last_position() {
        let mut app = sticky_sweep_app(20);
        let owner_id = NetworkEntityId(70);
        let owner = spawn_sticky_owner(&mut app, owner_id);
        let projectile = spawn_sticky_projectile(
            &mut app,
            owner,
            sticky_attack_source(51, owner_id),
            Vec2::new(17.0, 9.0),
            20,
        );

        run_sticky_sweep(&mut app);

        assert!(app.world().get::<Projectile>(projectile).is_none());
        assert_eq!(
            app.world().get::<Position>(projectile).unwrap().0,
            Vec2::new(17.0, 9.0)
        );
        let state = app.world().get::<StickyBlobState>(projectile).unwrap();
        assert_eq!(state.kind, StickyBlobKind::Primary);
        assert_eq!(state.attached_to, None);
        assert_eq!(state.armed_at_tick, 20);
        assert_eq!(state.detonates_at_tick, 89);
    }

    #[test]
    #[cfg(feature = "server")]
    fn production_sweep_enforces_per_owner_and_global_sticky_caps() {
        let mut app = sticky_sweep_app(20);
        let owner_id = NetworkEntityId(70);
        let owner = spawn_sticky_owner(&mut app, owner_id);
        let recipe = sticky_recipe();
        for attack_id in 1..=6 {
            app.world_mut().spawn((
                StickyBlobState {
                    kind: StickyBlobKind::Primary,
                    attached_to: None,
                    armed_at_tick: 1,
                    detonates_at_tick: 100,
                    explosion_radius: 85.44,
                },
                StickyBlobRuntime {
                    source: sticky_attack_source(attack_id, owner_id),
                    delivery_index: 0,
                    recipe: recipe.clone(),
                },
            ));
        }
        let owner_capped = spawn_sticky_projectile(
            &mut app,
            owner,
            sticky_attack_source(80, owner_id),
            Vec2::ZERO,
            20,
        );

        run_sticky_sweep(&mut app);

        assert!(app.world().get_entity(owner_capped).is_err());

        let mut app = sticky_sweep_app(20);
        let owner_id = NetworkEntityId(70);
        let owner = spawn_sticky_owner(&mut app, owner_id);
        for index in 0..sticky::MAX_ACTIVE_STICKY_BLOBS {
            let existing_owner = NetworkEntityId(1_000 + index as u64);
            app.world_mut().spawn((
                StickyBlobState {
                    kind: StickyBlobKind::Primary,
                    attached_to: None,
                    armed_at_tick: 1,
                    detonates_at_tick: 100,
                    explosion_radius: 85.44,
                },
                StickyBlobRuntime {
                    source: sticky_attack_source(1_000 + index as u64, existing_owner),
                    delivery_index: 0,
                    recipe: recipe.clone(),
                },
            ));
        }
        let globally_capped = spawn_sticky_projectile(
            &mut app,
            owner,
            sticky_attack_source(90, owner_id),
            Vec2::ZERO,
            20,
        );

        run_sticky_sweep(&mut app);

        assert!(app.world().get_entity(globally_capped).is_err());
    }

    #[test]
    #[cfg(feature = "server")]
    fn production_sweep_attaches_and_chains_primary_stickies_on_one_carrier() {
        let mut app = sticky_sweep_app(20);
        let owner_id = NetworkEntityId(70);
        let owner = spawn_sticky_owner(&mut app, owner_id);
        let target_id = NetworkEntityId(71);
        app.world_mut().spawn((
            Fighter,
            Position(Vec2::new(8.0, 0.0)),
            TeamId(1),
            target_id,
            RigidBody::Static,
            Collider::circle(2.0),
            CollisionLayers::new(FIGHTER_LAYER, PROJECTILE_LAYER),
        ));
        let existing = app
            .world_mut()
            .spawn((
                Position(Vec2::new(8.0, 0.0)),
                StickyBlobState {
                    kind: StickyBlobKind::Primary,
                    attached_to: Some(target_id),
                    armed_at_tick: 1,
                    detonates_at_tick: 100,
                    explosion_radius: 85.44,
                },
                StickyBlobRuntime {
                    source: sticky_attack_source(50, owner_id),
                    delivery_index: 0,
                    recipe: sticky_recipe(),
                },
            ))
            .id();
        let incoming = spawn_sticky_projectile(
            &mut app,
            owner,
            sticky_attack_source(51, owner_id),
            Vec2::ZERO,
            100,
        );

        run_sticky_sweep(&mut app);

        assert_eq!(
            app.world()
                .get::<StickyBlobState>(existing)
                .unwrap()
                .detonates_at_tick,
            20
        );
        let incoming_state = app.world().get::<StickyBlobState>(incoming).unwrap();
        assert_eq!(incoming_state.attached_to, Some(target_id));
        assert_eq!(incoming_state.detonates_at_tick, 89);
    }

    #[test]
    #[cfg(feature = "server")]
    fn lob_trajectory_plan_interpolates_then_lands_at_the_exact_tick() {
        let flight = LobbedFlight {
            launch: WorldPoint::from(Vec2::ZERO),
            landing: WorldPoint::from(Vec2::new(120.0, 60.0)),
            launched_at_tick: 10,
            lands_at_tick: 16,
            visual_arc_height: 40.0,
        };

        assert!(matches!(
            plan_lob_trajectory(13, &flight),
            LobTrajectoryPlan::InFlight(position) if position == Vec2::new(60.0, 30.0)
        ));
        assert!(matches!(
            plan_lob_trajectory(16, &flight),
            LobTrajectoryPlan::Landed(position) if position == Vec2::new(120.0, 60.0)
        ));
    }

    #[test]
    #[cfg(feature = "client")]
    fn arc_height_peaks_at_half_progress() {
        assert!((lob_height(0.5, 140.0) - 140.0).abs() < 0.001);
    }
    #[test]
    #[cfg(feature = "server")]
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
