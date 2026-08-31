//! Authoritative post-physics projectile snapshot, planning, and sequential commit.

#![allow(clippy::wildcard_imports)]

use super::*;

type FighterSweepQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        &'static TeamId,
        &'static NetworkEntityId,
        Option<&'static Defeated>,
        Option<&'static lightyear::prelude::ControlledBy>,
    ),
    Or<(With<Fighter>, With<crate::abilities::Sentry>)>,
>;

type ObjectSweepQuery<'w, 's> = Query<
    'w,
    's,
    (
        Entity,
        &'static Position,
        &'static crate::map::DamageableTargetIdentity,
        &'static CurrentHealth,
        &'static crate::map::DamageableLifeState,
    ),
    Or<(
        With<crate::map::DamageableWorldObject>,
        With<crate::matchplay::HeistSafe>,
    )>,
>;

#[derive(bevy::ecs::system::SystemParam)]
pub(in crate::combat) struct ProjectileEnvironmentState<'w, 's> {
    active_splashes: Query<'w, 's, &'static PersistentSplashRuntime>,
    roots: Query<'w, 's, &'static crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>,
    walls: Query<'w, 's, Entity, With<ArenaWall>>,
}

#[derive(bevy::ecs::system::SystemParam)]
pub(in crate::combat) struct ProjectileCommitState<'w> {
    ids: ResMut<'w, NextCombatIds>,
    trackers: ResMut<'w, ActiveAttackTrackers>,
    telemetry: ResMut<'w, WeaponTelemetry>,
    pending: MessageWriter<'w, PendingPayload>,
    world_pending: ResMut<'w, crate::map::PendingWorldTargetDamages>,
    objective_pending: ResMut<'w, crate::matchplay::PendingModeObjectiveDamages>,
    deliveries: MessageWriter<'w, PendingDelivery>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ProjectileFighterSnapshot {
    position: Vec2,
    team: TeamId,
    network_id: NetworkEntityId,
    defeated: bool,
    disconnected: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
struct ProjectileObjectSnapshot {
    position: Vec2,
    identity: crate::map::DamageableTargetIdentity,
    health: CurrentHealth,
    life: crate::map::DamageableLifeState,
}

/// Immutable world facts shared by every projectile plan in one fixed sweep.
///
/// Active splash counts intentionally reflect only entities visible before the sweep. Newly
/// spawned splash areas remain deferred until the existing post-sweep `ApplyDeferred` boundary.
struct ProjectileSweepSnapshot {
    fighters: HashMap<Entity, ProjectileFighterSnapshot>,
    objects: HashMap<Entity, ProjectileObjectSnapshot>,
    blocking_geometry: HashSet<Entity>,
    connected_owners: HashSet<u64>,
    active_splashes_by_owner: HashMap<u64, usize>,
    active_splash_total: usize,
    match_id: Option<crate::matchplay::MatchId>,
}

impl ProjectileSweepSnapshot {
    fn collect(
        fighters: &FighterSweepQuery,
        objects: &ObjectSweepQuery,
        disconnected: &Query<Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
        environment: &ProjectileEnvironmentState,
    ) -> Self {
        let disconnected_links = disconnected.iter().collect::<HashSet<_>>();
        let mut connected_owners = HashSet::new();
        let fighters = fighters
            .iter()
            .map(
                |(entity, position, team, network_id, defeated, controlled)| {
                    let disconnected = controlled
                        .is_some_and(|controlled| disconnected_links.contains(&controlled.owner));
                    if !disconnected {
                        connected_owners.insert(network_id.0);
                    }
                    (
                        entity,
                        ProjectileFighterSnapshot {
                            position: position.0,
                            team: *team,
                            network_id: *network_id,
                            defeated: defeated.is_some(),
                            disconnected,
                        },
                    )
                },
            )
            .collect();
        let objects = objects
            .iter()
            .map(|(entity, position, identity, health, life)| {
                (
                    entity,
                    ProjectileObjectSnapshot {
                        position: position.0,
                        identity: *identity,
                        health: *health,
                        life: *life,
                    },
                )
            })
            .collect();
        let mut active_splashes_by_owner = HashMap::new();
        let mut active_splash_total = 0_usize;
        for splash in &environment.active_splashes {
            *active_splashes_by_owner
                .entry(splash.source.owner_network_entity_id.0)
                .or_default() += 1;
            active_splash_total = active_splash_total.saturating_add(1);
        }
        Self {
            fighters,
            objects,
            // Permanent map colliders and destructible chunks both carry ArenaWall. Keeping the
            // complete entity set means carved lanes are the only non-blocking route.
            blocking_geometry: environment.walls.iter().collect(),
            connected_owners,
            active_splashes_by_owner,
            active_splash_total,
            match_id: environment.roots.single().ok().map(|root| root.match_id),
        }
    }

    fn owner_is_connected(&self, owner: NetworkEntityId) -> bool {
        self.connected_owners.contains(&owner.0)
    }

    fn active_splashes_for(&self, owner: NetworkEntityId) -> usize {
        self.active_splashes_by_owner
            .get(&owner.0)
            .copied()
            .unwrap_or_default()
    }

    fn accepts_candidate(&self, candidate: Entity, runtime: &ComposedProjectileRuntime) -> bool {
        self.fighters.get(&candidate).map_or_else(
            || {
                self.objects.get(&candidate).map_or_else(
                    || self.blocking_geometry.contains(&candidate),
                    |object| crate::map::object_is_live(object.health, object.life),
                )
            },
            |fighter| {
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
                        && payload_can_affect_target(
                            bundle,
                            runtime.source,
                            fighter.team,
                            fighter.network_id,
                        )
                });
                has_contact_delivery
                    && has_affecting_payload
                    && !fighter.defeated
                    && !fighter.disconnected
            },
        )
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
enum LobTrajectoryPlan {
    InFlight(Vec2),
    Landed(Vec2),
}

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

struct StraightTrajectoryPlan {
    body: ProjectileBody,
    step: f32,
    direction: Dir2,
}

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

#[derive(Clone, Copy, Debug, PartialEq)]
struct ProjectileShapeHitPlan {
    entity: Entity,
    point: Vec2,
    normal: Vec2,
    distance: f32,
}

#[derive(Clone, Debug, PartialEq)]
struct PlannedDeliveryOutputs {
    payloads: Vec<PendingPayload>,
    world_damages: Vec<crate::map::PendingWorldTargetDamage>,
    delivery: PendingDelivery,
}

#[derive(Clone, Debug, PartialEq)]
struct SplashActivationPlan {
    state: PersistentSplashState,
    runtime: PersistentSplashRuntime,
    match_member: Option<crate::matchplay::MatchMember>,
    delivery: PendingDelivery,
}

#[derive(Clone, Debug, PartialEq)]
enum ProjectileStepPlan {
    Cancelled,
    LobInFlight {
        position: Vec2,
    },
    LobAreaLanding(PlannedDeliveryOutputs),
    SplashRejected {
        landing: Vec2,
    },
    SplashActivated(Box<SplashActivationPlan>),
    StickyArm(sticky::StickyArmPlan),
    Expired,
    InvalidStraight,
    StraightAdvance {
        position: Vec2,
        travelled: f32,
    },
    StraightImpact {
        travelled: f32,
        outputs: PlannedDeliveryOutputs,
    },
}

#[derive(Clone, Copy)]
struct ProjectilePlanningInput<'a> {
    tick: u64,
    entity: Entity,
    position: Vec2,
    runtime: &'a ComposedProjectileRuntime,
    body: Option<&'a ProjectileBody>,
    lob: Option<&'a LobbedFlight>,
    snapshot: &'a ProjectileSweepSnapshot,
}

fn plan_projectile_step(
    input: ProjectilePlanningInput<'_>,
    sticky_ledger: &mut sticky::StickyPlanningLedger,
    mut plan_landing: impl FnMut(Vec2) -> ProjectileStepPlan,
    mut cast_straight: impl FnMut(&StraightTrajectoryPlan) -> Option<ProjectileShapeHitPlan>,
) -> ProjectileStepPlan {
    let ProjectilePlanningInput {
        tick,
        entity,
        position,
        runtime,
        body,
        lob,
        snapshot,
    } = input;
    if !snapshot.owner_is_connected(runtime.source.owner_network_entity_id) {
        return ProjectileStepPlan::Cancelled;
    }
    if let Some(lob) = lob {
        return match plan_lob_trajectory(tick, lob) {
            LobTrajectoryPlan::InFlight(position) => ProjectileStepPlan::LobInFlight { position },
            LobTrajectoryPlan::Landed(landing) => plan_landing(landing),
        };
    }
    if tick >= runtime.expires_at_tick || runtime.travelled >= runtime.maximum_range {
        if let Some(plan) =
            sticky_ledger.try_plan_arm(entity, runtime, position, None, tick, runtime.travelled)
        {
            return ProjectileStepPlan::StickyArm(plan);
        }
        return ProjectileStepPlan::Expired;
    }
    let Some(trajectory) = plan_straight_trajectory(runtime, body) else {
        return ProjectileStepPlan::InvalidStraight;
    };
    let Some(hit) = cast_straight(&trajectory) else {
        return ProjectileStepPlan::StraightAdvance {
            position: position + trajectory.direction.as_vec2() * trajectory.step,
            travelled: runtime.travelled + trajectory.step,
        };
    };
    let travelled = runtime.travelled + hit.distance.clamp(0.0, trajectory.step);
    let fighter = snapshot.fighters.get(&hit.entity).copied();
    let attached_to = fighter.and_then(|target| (!target.defeated).then_some(target.network_id));
    let armed_position = fighter.map_or(hit.point, |target| target.position);
    if let Some(plan) = sticky_ledger.try_plan_arm(
        entity,
        runtime,
        armed_position,
        attached_to,
        tick,
        travelled,
    ) {
        return ProjectileStepPlan::StickyArm(plan);
    }
    let payloads = fighter.map_or_else(Vec::new, |target| {
        direct_fighter_payloads(
            runtime,
            hit.entity,
            target,
            hit.point,
            hit.distance,
            trajectory.step,
            travelled,
        )
    });
    let world_damages = snapshot
        .objects
        .get(&hit.entity)
        .filter(|object| crate::map::object_is_live(object.health, object.life))
        .map_or_else(Vec::new, |object| {
            direct_world_damage_requests(runtime, object.identity, hit.point, travelled)
        });
    ProjectileStepPlan::StraightImpact {
        travelled,
        outputs: PlannedDeliveryOutputs {
            payloads,
            world_damages,
            delivery: straight_impact_delivery(
                runtime, entity, fighter, tick, hit.point, hit.normal, travelled,
            ),
        },
    }
}

fn projectile_order_key(
    runtime: &ComposedProjectileRuntime,
    lob: Option<&LobbedFlight>,
) -> (u64, u8, bool) {
    (
        runtime.source.attack_id.0,
        runtime.delivery_index,
        lob.is_some(),
    )
}

#[derive(Clone, Debug)]
struct ProjectileSweepFact {
    entity: Entity,
    position: Vec2,
    runtime: ComposedProjectileRuntime,
    body: Option<ProjectileBody>,
    lob: Option<LobbedFlight>,
}

struct PlannedProjectileStep {
    entity: Entity,
    position: Vec2,
    plan: ProjectileStepPlan,
}

struct ProjectileTerminationContext<'a, 'w, 's> {
    commands: &'a mut Commands<'w, 's>,
    ids: &'a mut NextCombatIds,
    trackers: &'a mut ActiveAttackTrackers,
    telemetry: &'a mut WeaponTelemetry,
}

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

fn direct_world_damage_requests(
    runtime: &ComposedProjectileRuntime,
    target: crate::map::DamageableTargetIdentity,
    hit_point: Vec2,
    delivery_travel: f32,
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
                    delivery_travel,
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

#[allow(clippy::too_many_arguments)]
fn direct_fighter_payloads(
    runtime: &ComposedProjectileRuntime,
    target_entity: Entity,
    target: ProjectileFighterSnapshot,
    hit_point: Vec2,
    hit_distance: f32,
    step: f32,
    delivery_travel: f32,
) -> Vec<PendingPayload> {
    if target.defeated {
        return Vec::new();
    }
    runtime
        .recipe
        .payload_bundles
        .iter()
        .enumerate()
        .filter(|(_, bundle)| {
            matches!(bundle.target, TargetSelection::Direct)
                && payload_can_affect_target(bundle, runtime.source, target.team, target.network_id)
        })
        .map(|(bundle_index, bundle)| PendingPayload {
            source: runtime.source,
            delivery_index: runtime.delivery_index,
            bundle_index: u8::try_from(bundle_index).unwrap_or(u8::MAX),
            target: target_entity,
            target_network_id: target.network_id,
            position: hit_point,
            engagement_distance: runtime.source.origin.as_vec2().distance(target.position),
            delivery_travel,
            contact_fraction: (hit_distance / step.max(f32::EPSILON)).clamp(0.0, 1.0),
            bundle: bundle.clone(),
        })
        .collect()
}

fn straight_impact_delivery(
    runtime: &ComposedProjectileRuntime,
    entity: Entity,
    target: Option<ProjectileFighterSnapshot>,
    tick: u64,
    point: Vec2,
    normal: Vec2,
    delivery_travel: f32,
) -> PendingDelivery {
    PendingDelivery {
        entity: Some(entity),
        source: runtime.source,
        delivery_index: runtime.delivery_index,
        tick,
        engagement_distance: target.map_or(0.0, |target| {
            runtime.source.origin.as_vec2().distance(target.position)
        }),
        delivery_travel,
        kind: PendingDeliveryKind::StraightImpact {
            target: target.map(|target| target.network_id),
            position: WorldPoint::from(point),
            normal: WorldPoint::from(normal),
            distance_band: distance_band(delivery_travel),
        },
        world_effects: runtime.recipe.world_effects.clone(),
    }
}

fn plan_area_landing_outputs(
    landing: Vec2,
    source: AttackSource,
    delivery_index: u8,
    recipe: &WeaponRecipe,
    snapshot: &ProjectileSweepSnapshot,
    spatial_query: &avian2d::prelude::SpatialQuery,
) -> PlannedAreaPayloads {
    let fighter_filter = avian2d::prelude::SpatialQueryFilter::from_mask(
        FIGHTER_LAYER | crate::movement::DEPLOYABLE_LAYER,
    );
    let candidates =
        collect_area_bundle_candidates(
            recipe,
            |radius, map_occlusion| {
                spatial_query
                    .shape_intersections(&Collider::circle(radius), landing, 0.0, &fighter_filter)
                    .into_iter()
                    .filter_map(|entity| {
                        snapshot.fighters.get(&entity).copied().map(|fighter| {
                            AreaFighterCandidate {
                                entity,
                                position: fighter.position,
                                team: fighter.team,
                                network_id: fighter.network_id,
                                defeated: fighter.defeated,
                                disconnected: fighter.disconnected,
                                line_of_sight_clear: !map_occlusion
                                    || area_line_of_sight_clear(
                                        landing,
                                        fighter.position,
                                        spatial_query,
                                    ),
                            }
                        })
                    })
                    .collect()
            },
            |radius, map_occlusion| {
                snapshot
                    .objects
                    .iter()
                    .filter(|(_, object)| {
                        crate::map::object_is_live(object.health, object.life)
                            && object.position.distance_squared(landing) <= radius * radius
                    })
                    .map(|(entity, object)| AreaObjectCandidate {
                        position: object.position,
                        identity: object.identity,
                        line_of_sight_clear: !map_occlusion
                            || area_line_of_sight_clear_excluding(
                                landing,
                                object.position,
                                *entity,
                                spatial_query,
                            ),
                    })
                    .collect()
            },
        );
    plan_area_payloads(landing, source, delivery_index, recipe, candidates)
}

fn plan_lob_landing(
    tick: u64,
    entity: Entity,
    landing: Vec2,
    runtime: &ComposedProjectileRuntime,
    snapshot: &ProjectileSweepSnapshot,
    spatial_query: &avian2d::prelude::SpatialQuery,
) -> ProjectileStepPlan {
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
        if snapshot.active_splashes_for(runtime.source.owner_network_entity_id)
            >= usize::from(max_active_per_owner)
            || snapshot.active_splash_total >= splash::MAX_ACTIVE_PERSISTENT_SPLASHES
        {
            return ProjectileStepPlan::SplashRejected { landing };
        }
        let (expires_at_tick, _) =
            splash::splash_timing(tick, duration_ticks, pulse_interval_ticks);
        return ProjectileStepPlan::SplashActivated(Box::new(SplashActivationPlan {
            state: PersistentSplashState {
                center: WorldPoint::from(landing),
                facing: runtime.source.facing,
                shape,
                activated_at_tick: tick,
                next_pulse_tick: tick,
                expires_at_tick,
                pulse_interval_ticks,
                map_occlusion,
                max_targets,
                effects: splash::presentation_effects(&runtime.recipe),
            },
            runtime: PersistentSplashRuntime {
                source: runtime.source,
                recipe: runtime.recipe.clone(),
                next_delivery_index: 1,
                match_id: snapshot.match_id,
            },
            match_member: snapshot.match_id.map(crate::matchplay::MatchMember),
            delivery: PendingDelivery {
                entity: None,
                source: runtime.source,
                delivery_index: 0,
                tick,
                engagement_distance: 0.0,
                delivery_travel: lob_launch_point(runtime.source, &runtime.recipe)
                    .distance(landing),
                kind: PendingDeliveryKind::LobLanded {
                    position: WorldPoint::from(landing),
                },
                world_effects: Vec::new(),
            },
        }));
    }
    let area = plan_area_landing_outputs(
        landing,
        runtime.source,
        runtime.delivery_index,
        &runtime.recipe,
        snapshot,
        spatial_query,
    );
    ProjectileStepPlan::LobAreaLanding(PlannedDeliveryOutputs {
        payloads: area.payloads,
        world_damages: area.world_damages,
        delivery: PendingDelivery {
            entity: Some(entity),
            source: runtime.source,
            delivery_index: runtime.delivery_index,
            tick,
            engagement_distance: 0.0,
            delivery_travel: lob_launch_point(runtime.source, &runtime.recipe).distance(landing),
            kind: PendingDeliveryKind::LobLanded {
                position: WorldPoint::from(landing),
            },
            world_effects: runtime.recipe.world_effects.clone(),
        },
    })
}

fn commit_outputs(outputs: PlannedDeliveryOutputs, commit: &mut ProjectileCommitState) {
    for payload in outputs.payloads {
        commit.pending.write(payload);
    }
    for request in outputs.world_damages {
        queue_damageable_target(
            &mut commit.world_pending,
            &mut commit.objective_pending,
            request,
        );
    }
    commit.deliveries.write(outputs.delivery);
}

fn commit_projectile_step(
    plan: ProjectileStepPlan,
    commands: &mut Commands,
    tick: u64,
    entity: Entity,
    position: Vec2,
    runtime: &mut ComposedProjectileRuntime,
    commit: &mut ProjectileCommitState,
) {
    match plan {
        ProjectileStepPlan::Cancelled | ProjectileStepPlan::InvalidStraight => {
            ProjectileTerminationContext {
                commands,
                ids: &mut commit.ids,
                trackers: &mut commit.trackers,
                telemetry: &mut commit.telemetry,
            }
            .commit(
                tick,
                entity,
                position,
                runtime,
                WeaponTelemetryOutcome::DeliveryCancelled,
            );
        }
        ProjectileStepPlan::LobInFlight { position } => {
            commands.entity(entity).insert(Position(position));
        }
        ProjectileStepPlan::LobAreaLanding(outputs) => commit_outputs(outputs, commit),
        ProjectileStepPlan::SplashRejected { landing } => {
            record_delivery_termination(
                &mut commit.ids,
                &mut commit.telemetry,
                tick,
                runtime,
                landing,
                WeaponTelemetryOutcome::DeliveryCancelled,
            );
            commands.entity(entity).try_despawn();
            splash::settle_unresolved_splash(&mut commit.trackers, runtime.source.attack_id);
        }
        ProjectileStepPlan::SplashActivated(plan) => {
            let mut splash_entity = commands.spawn((
                PersistentSplash,
                plan.state,
                ReplicatedAttackSource {
                    attack: plan.runtime.source,
                },
                plan.runtime,
                Replicate::to_clients(NetworkTarget::All),
            ));
            if let Some(match_member) = plan.match_member {
                splash_entity.insert(match_member);
            }
            commands.entity(entity).try_despawn();
            commit.deliveries.write(plan.delivery);
        }
        ProjectileStepPlan::StickyArm(plan) => {
            sticky::commit_arm_plan(commands, runtime, plan);
        }
        ProjectileStepPlan::Expired => {
            ProjectileTerminationContext {
                commands,
                ids: &mut commit.ids,
                trackers: &mut commit.trackers,
                telemetry: &mut commit.telemetry,
            }
            .commit(
                tick,
                entity,
                position,
                runtime,
                WeaponTelemetryOutcome::DeliveryExpired,
            );
        }
        ProjectileStepPlan::StraightAdvance {
            position,
            travelled,
        } => {
            runtime.travelled = travelled;
            commands.entity(entity).insert(Position(position));
        }
        ProjectileStepPlan::StraightImpact { travelled, outputs } => {
            runtime.travelled = travelled;
            commit_outputs(outputs, commit);
        }
    }
}

#[allow(clippy::too_many_arguments)]
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "the schedule-facing coordinator exposes its complete fixed-tick world view while delegating all planning and family commits"
)]
pub(in crate::combat) fn sweep_composed_projectiles(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    mut commit: ProjectileCommitState,
    mut projectiles: ParamSet<(
        Query<(
            Entity,
            &Position,
            &ComposedProjectileRuntime,
            Option<&ProjectileBody>,
            Option<&LobbedFlight>,
        )>,
        Query<&mut ComposedProjectileRuntime>,
    )>,
    sticky_blobs: Query<(Entity, &StickyBlobState, &StickyBlobRuntime)>,
    environment: ProjectileEnvironmentState,
    fighters: FighterSweepQuery,
    objects: ObjectSweepQuery,
    disconnected: Query<Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
    spatial_query: avian2d::prelude::SpatialQuery,
) {
    let snapshot =
        ProjectileSweepSnapshot::collect(&fighters, &objects, &disconnected, &environment);
    let mut sticky_ledger = sticky::StickyPlanningLedger::from_active(&sticky_blobs);
    let mut ordered = {
        let projectiles = projectiles.p0();
        projectiles
            .iter()
            .map(
                |(entity, position, runtime, body, lob)| ProjectileSweepFact {
                    entity,
                    position: position.0,
                    runtime: runtime.clone(),
                    body: body.copied(),
                    lob: lob.copied(),
                },
            )
            .collect::<Vec<_>>()
    };
    ordered.sort_by_key(|fact| projectile_order_key(&fact.runtime, fact.lob.as_ref()));
    let planned = ordered
        .into_iter()
        .map(|fact| {
            let plan = plan_projectile_step(
                ProjectilePlanningInput {
                    tick: tick.0,
                    entity: fact.entity,
                    position: fact.position,
                    runtime: &fact.runtime,
                    body: fact.body.as_ref(),
                    lob: fact.lob.as_ref(),
                    snapshot: &snapshot,
                },
                &mut sticky_ledger,
                |landing| {
                    plan_lob_landing(
                        tick.0,
                        fact.entity,
                        landing,
                        &fact.runtime,
                        &snapshot,
                        &spatial_query,
                    )
                },
                |trajectory| {
                    let filter = avian2d::prelude::SpatialQueryFilter::from_mask(
                        FIGHTER_LAYER
                            | crate::movement::DEPLOYABLE_LAYER
                            | STATIC_MAP_LAYER
                            | DESTRUCTIBLE_MAP_LAYER,
                    )
                    .with_excluded_entities([
                        fact.entity,
                        fact.runtime.owner_entity,
                        fact.runtime.source_entity,
                    ]);
                    spatial_query
                        .cast_shape_predicate(
                            &trajectory.body.collider(),
                            fact.position,
                            0.0,
                            trajectory.direction,
                            &avian2d::prelude::ShapeCastConfig::from_max_distance(trajectory.step),
                            &filter,
                            &|candidate| snapshot.accepts_candidate(candidate, &fact.runtime),
                        )
                        .map(|hit| ProjectileShapeHitPlan {
                            entity: hit.entity,
                            point: hit.point2,
                            normal: hit.normal1,
                            distance: hit.distance,
                        })
                },
            );
            PlannedProjectileStep {
                entity: fact.entity,
                position: fact.position,
                plan,
            }
        })
        .collect::<Vec<_>>();
    let mut projectile_runtimes = projectiles.p1();
    for planned in planned {
        let mut runtime = projectile_runtimes
            .get_mut(planned.entity)
            .expect("the immutable projectile batch remains valid through planning");
        commit_projectile_step(
            planned.plan,
            &mut commands,
            tick.0,
            planned.entity,
            planned.position,
            &mut runtime,
            &mut commit,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn projectile_sweep_app(tick: u64) -> App {
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

    fn test_source(attack_id: u64, owner: NetworkEntityId) -> AttackSource {
        AttackSource {
            kind: CombatSourceKind::PrimaryWeapon,
            attack_id: AttackId(attack_id),
            player_id: PlayerId(owner.0),
            owner_network_entity_id: owner,
            team_id: TeamId(0),
            recipe_fingerprint: WeaponRecipeFingerprint(11),
            legacy_compatibility: false,
            source_preset_id: Some(WeaponPresetId(1)),
            origin: WorldPoint::from(Vec2::ZERO),
            facing: 0.0,
        }
    }

    fn test_runtime(owner_entity: Entity, owner: NetworkEntityId) -> ComposedProjectileRuntime {
        ComposedProjectileRuntime {
            owner_entity,
            source_entity: owner_entity,
            source: test_source(41, owner),
            delivery_index: 2,
            velocity: Vec2::new(600.0, 0.0),
            travelled: 0.0,
            expires_at_tick: 100,
            maximum_range: 1_000.0,
            landing: None,
            recipe: WeaponCatalog::embedded()
                .unwrap()
                .preset(WeaponPresetId(1))
                .unwrap()
                .configuration
                .recipe
                .clone(),
        }
    }

    fn snapshot_with_owner(owner: NetworkEntityId) -> ProjectileSweepSnapshot {
        ProjectileSweepSnapshot {
            fighters: HashMap::new(),
            objects: HashMap::new(),
            blocking_geometry: HashSet::new(),
            connected_owners: HashSet::from([owner.0]),
            active_splashes_by_owner: HashMap::new(),
            active_splash_total: 0,
            match_id: None,
        }
    }

    fn splash_recipe() -> WeaponRecipe {
        WeaponCatalog::embedded()
            .unwrap()
            .preset(WeaponPresetId(7))
            .unwrap()
            .configuration
            .recipe
            .clone()
    }

    fn spawn_landed_splash(app: &mut App, owner_entity: Entity, owner: NetworkEntityId) -> Entity {
        let recipe = splash_recipe();
        app.world_mut()
            .spawn((
                Projectile,
                Position(Vec2::ZERO),
                LobbedFlight {
                    launch: Vec2::ZERO.into(),
                    landing: Vec2::new(10.0, 0.0).into(),
                    launched_at_tick: 10,
                    lands_at_tick: 20,
                    visual_arc_height: 10.0,
                },
                ComposedProjectileRuntime {
                    owner_entity,
                    source_entity: owner_entity,
                    source: AttackSource {
                        source_preset_id: Some(WeaponPresetId(7)),
                        ..test_source(51, owner)
                    },
                    delivery_index: 0,
                    velocity: Vec2::ZERO,
                    travelled: 0.0,
                    expires_at_tick: 20,
                    maximum_range: 480.0,
                    landing: Some(Vec2::new(10.0, 0.0)),
                    recipe,
                },
            ))
            .id()
    }

    #[test]
    fn scrambled_projectile_facts_sort_by_attack_delivery_and_lob_priority() {
        let mut world = World::new();
        let owner = world.spawn_empty().id();
        let lob = LobbedFlight {
            launch: Vec2::ZERO.into(),
            landing: Vec2::X.into(),
            launched_at_tick: 1,
            lands_at_tick: 2,
            visual_arc_height: 1.0,
        };
        let runtime = |attack_id, delivery_index| {
            let mut runtime = test_runtime(owner, NetworkEntityId(7));
            runtime.source.attack_id = AttackId(attack_id);
            runtime.delivery_index = delivery_index;
            runtime
        };
        let mut scrambled = [
            ("latest", runtime(9, 3), None),
            ("lob", runtime(2, 4), Some(lob)),
            ("first", runtime(2, 1), None),
            ("straight", runtime(2, 4), None),
        ];

        scrambled.sort_by_key(|(_, runtime, lob)| projectile_order_key(runtime, lob.as_ref()));

        assert_eq!(
            scrambled
                .iter()
                .map(|(label, _, _)| *label)
                .collect::<Vec<_>>(),
            ["first", "straight", "lob", "latest"]
        );
    }

    #[test]
    fn disconnected_owner_cancels_before_lob_or_collision_planning() {
        let mut world = World::new();
        let owner_entity = world.spawn_empty().id();
        let projectile_entity = world.spawn_empty().id();
        let owner = NetworkEntityId(7);
        let runtime = test_runtime(owner_entity, owner);
        let snapshot = ProjectileSweepSnapshot {
            connected_owners: HashSet::new(),
            ..snapshot_with_owner(owner)
        };
        let lob = LobbedFlight {
            launch: Vec2::ZERO.into(),
            landing: Vec2::X.into(),
            launched_at_tick: 1,
            lands_at_tick: 1,
            visual_arc_height: 1.0,
        };
        let landing_planned = std::cell::Cell::new(false);
        let collision_planned = std::cell::Cell::new(false);
        let mut sticky_ledger = sticky::StickyPlanningLedger::empty();

        let plan = plan_projectile_step(
            ProjectilePlanningInput {
                tick: 1,
                entity: projectile_entity,
                position: Vec2::ZERO,
                runtime: &runtime,
                body: Some(&ProjectileBody::circle(2.0)),
                lob: Some(&lob),
                snapshot: &snapshot,
            },
            &mut sticky_ledger,
            |_| {
                landing_planned.set(true);
                ProjectileStepPlan::Expired
            },
            |_| {
                collision_planned.set(true);
                None
            },
        );

        assert_eq!(plan, ProjectileStepPlan::Cancelled);
        assert!(!landing_planned.get());
        assert!(!collision_planned.get());
    }

    #[test]
    fn expiry_precedes_invalid_straight_geometry_and_collision() {
        let mut world = World::new();
        let owner_entity = world.spawn_empty().id();
        let owner = NetworkEntityId(7);
        let mut runtime = test_runtime(owner_entity, owner);
        runtime.expires_at_tick = 10;
        let snapshot = snapshot_with_owner(owner);
        let projectile_entity = world.spawn_empty().id();
        let mut sticky_ledger = sticky::StickyPlanningLedger::empty();
        let mut collision_planned = false;

        let plan = plan_projectile_step(
            ProjectilePlanningInput {
                tick: 10,
                entity: projectile_entity,
                position: Vec2::ZERO,
                runtime: &runtime,
                body: None,
                lob: None,
                snapshot: &snapshot,
            },
            &mut sticky_ledger,
            |_| panic!("a straight projectile cannot plan a lob landing"),
            |_| {
                collision_planned = true;
                None
            },
        );

        assert_eq!(plan, ProjectileStepPlan::Expired);
        assert!(!collision_planned);
    }

    #[test]
    fn straight_advance_clamps_to_remaining_range() {
        let mut world = World::new();
        let owner_entity = world.spawn_empty().id();
        let owner = NetworkEntityId(7);
        let mut runtime = test_runtime(owner_entity, owner);
        runtime.travelled = 8.0;
        runtime.maximum_range = 11.0;
        let snapshot = snapshot_with_owner(owner);
        let projectile_entity = world.spawn_empty().id();
        let mut sticky_ledger = sticky::StickyPlanningLedger::empty();

        let plan = plan_projectile_step(
            ProjectilePlanningInput {
                tick: 2,
                entity: projectile_entity,
                position: Vec2::new(8.0, 0.0),
                runtime: &runtime,
                body: Some(&ProjectileBody::circle(2.0)),
                lob: None,
                snapshot: &snapshot,
            },
            &mut sticky_ledger,
            |_| panic!("a straight projectile cannot plan a lob landing"),
            |_| None,
        );

        assert_eq!(
            plan,
            ProjectileStepPlan::StraightAdvance {
                position: Vec2::new(11.0, 0.0),
                travelled: 11.0,
            }
        );

        runtime.travelled = 11.0;
        let mut sticky_ledger = sticky::StickyPlanningLedger::empty();
        assert_eq!(
            plan_projectile_step(
                ProjectilePlanningInput {
                    tick: 3,
                    entity: projectile_entity,
                    position: Vec2::new(11.0, 0.0),
                    runtime: &runtime,
                    body: Some(&ProjectileBody::circle(2.0)),
                    lob: None,
                    snapshot: &snapshot,
                },
                &mut sticky_ledger,
                |_| panic!("a straight projectile cannot plan a lob landing"),
                |_| panic!("a projectile at maximum range must expire before collision"),
            ),
            ProjectileStepPlan::Expired
        );
    }

    #[test]
    fn snapshot_candidate_rules_preserve_fighter_object_and_cover_acceptance() {
        use crate::map::{
            DamageableLifeState, DamageableTargetIdentity, MapDynamicGeneration, MapInstanceId,
            MapPlacementId,
        };

        let mut world = World::new();
        let owner_entity = world.spawn_empty().id();
        let fighter = world.spawn_empty().id();
        let object = world.spawn_empty().id();
        let cover = world.spawn_empty().id();
        let owner = NetworkEntityId(7);
        let runtime = test_runtime(owner_entity, owner);
        let mut snapshot = snapshot_with_owner(owner);
        snapshot.fighters.insert(
            fighter,
            ProjectileFighterSnapshot {
                position: Vec2::X,
                team: TeamId(1),
                network_id: NetworkEntityId(8),
                defeated: false,
                disconnected: false,
            },
        );
        snapshot.objects.insert(
            object,
            ProjectileObjectSnapshot {
                position: Vec2::X * 2.0,
                identity: DamageableTargetIdentity::MapObject {
                    generation: MapDynamicGeneration {
                        map_instance_id: MapInstanceId(1),
                        generation: 1,
                    },
                    placement_id: MapPlacementId(2),
                },
                health: CurrentHealth(10),
                life: DamageableLifeState::Live,
            },
        );
        snapshot.blocking_geometry.insert(cover);

        assert!(snapshot.accepts_candidate(fighter, &runtime));
        assert!(snapshot.accepts_candidate(object, &runtime));
        assert!(snapshot.accepts_candidate(cover, &runtime));

        snapshot.fighters.get_mut(&fighter).unwrap().defeated = true;
        snapshot.objects.get_mut(&object).unwrap().life = DamageableLifeState::TerminalCommitted;
        assert!(!snapshot.accepts_candidate(fighter, &runtime));
        assert!(!snapshot.accepts_candidate(object, &runtime));
    }

    #[test]
    fn straight_impact_plan_copies_hit_and_target_before_commit() {
        let mut world = World::new();
        let owner_entity = world.spawn_empty().id();
        let projectile_entity = world.spawn_empty().id();
        let target_entity = world.spawn_empty().id();
        let owner = NetworkEntityId(7);
        let runtime = test_runtime(owner_entity, owner);
        let mut snapshot = snapshot_with_owner(owner);
        let fighter = ProjectileFighterSnapshot {
            position: Vec2::new(9.0, 0.0),
            team: TeamId(1),
            network_id: NetworkEntityId(8),
            defeated: false,
            disconnected: false,
        };
        snapshot.fighters.insert(target_entity, fighter);
        let hit = ProjectileShapeHitPlan {
            entity: target_entity,
            point: Vec2::new(4.0, 0.0),
            normal: -Vec2::X,
            distance: 4.0,
        };
        let mut sticky_ledger = sticky::StickyPlanningLedger::empty();

        let plan = plan_projectile_step(
            ProjectilePlanningInput {
                tick: 2,
                entity: projectile_entity,
                position: Vec2::ZERO,
                runtime: &runtime,
                body: Some(&ProjectileBody::circle(2.0)),
                lob: None,
                snapshot: &snapshot,
            },
            &mut sticky_ledger,
            |_| panic!("a straight projectile cannot plan a lob landing"),
            |_| Some(hit),
        );

        let ProjectileStepPlan::StraightImpact { travelled, outputs } = plan else {
            panic!("expected a fully owned straight-impact plan");
        };
        assert!((travelled - 4.0).abs() < f32::EPSILON);
        assert_eq!(outputs.payloads.len(), 1);
        assert_eq!(outputs.payloads[0].target, target_entity);
        assert_eq!(outputs.payloads[0].target_network_id, fighter.network_id);
        assert!((outputs.payloads[0].delivery_travel - 4.0).abs() < f32::EPSILON);
        assert!(outputs.world_damages.is_empty());
        assert_eq!(outputs.delivery.entity, Some(projectile_entity));
        assert!((outputs.delivery.delivery_travel - 4.0).abs() < f32::EPSILON);
        assert!(matches!(
            outputs.delivery.kind,
            PendingDeliveryKind::StraightImpact {
                target: Some(NetworkEntityId(8)),
                position,
                normal,
                ..
            } if position.as_vec2() == hit.point && normal.as_vec2() == hit.normal
        ));
    }

    #[test]
    fn splash_landing_preserves_success_and_capacity_rejection_publication() {
        let owner = NetworkEntityId(7);
        let mut app = projectile_sweep_app(20);
        let owner_entity = app
            .world_mut()
            .spawn((Fighter, Position(Vec2::ZERO), TeamId(0), owner))
            .id();
        let projectile = spawn_landed_splash(&mut app, owner_entity, owner);

        app.world_mut().run_schedule(FixedPostUpdate);

        assert!(app.world().get_entity(projectile).is_err());
        let mut areas = app
            .world_mut()
            .query_filtered::<Entity, With<PersistentSplash>>();
        assert_eq!(areas.iter(app.world()).count(), 1);
        let deliveries = app
            .world_mut()
            .resource_mut::<Messages<PendingDelivery>>()
            .drain()
            .collect::<Vec<_>>();
        assert_eq!(deliveries.len(), 1);
        assert_eq!(deliveries[0].entity, None);
        assert!(deliveries[0].world_effects.is_empty());
        assert!(matches!(
            deliveries[0].kind,
            PendingDeliveryKind::LobLanded { .. }
        ));

        let mut app = projectile_sweep_app(20);
        let owner_entity = app
            .world_mut()
            .spawn((Fighter, Position(Vec2::ZERO), TeamId(0), owner))
            .id();
        let recipe = splash_recipe();
        for attack_id in 1..=2 {
            app.world_mut().spawn(PersistentSplashRuntime {
                source: AttackSource {
                    source_preset_id: Some(WeaponPresetId(7)),
                    ..test_source(attack_id, owner)
                },
                recipe: recipe.clone(),
                next_delivery_index: 1,
                match_id: None,
            });
        }
        let projectile = spawn_landed_splash(&mut app, owner_entity, owner);

        app.world_mut().run_schedule(FixedPostUpdate);

        assert!(app.world().get_entity(projectile).is_err());
        let deliveries = app
            .world_mut()
            .resource_mut::<Messages<PendingDelivery>>()
            .drain()
            .collect::<Vec<_>>();
        assert!(deliveries.is_empty());
        assert!(
            app.world()
                .resource::<WeaponTelemetry>()
                .bounded_records
                .iter()
                .any(|record| record.outcome == WeaponTelemetryOutcome::DeliveryCancelled)
        );
    }

    #[test]
    fn lob_trajectory_interpolates_then_lands_at_the_exact_tick() {
        let flight = LobbedFlight {
            launch: WorldPoint::from(Vec2::ZERO),
            landing: WorldPoint::from(Vec2::new(120.0, 60.0)),
            launched_at_tick: 10,
            lands_at_tick: 16,
            visual_arc_height: 40.0,
        };

        assert_eq!(
            plan_lob_trajectory(13, &flight),
            LobTrajectoryPlan::InFlight(Vec2::new(60.0, 30.0))
        );
        assert_eq!(
            plan_lob_trajectory(16, &flight),
            LobTrajectoryPlan::Landed(Vec2::new(120.0, 60.0))
        );
    }
}
