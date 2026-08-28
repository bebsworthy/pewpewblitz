//! Damageable world-object combat, terminal reactions, explosions, cues, and telemetry.

use avian2d::prelude::Position;
use bevy::prelude::*;
use lightyear::prelude::{Disconnected, LinkOf, MessageSender};

use crate::{
    map::{MapCatalogResource, MapDynamicState, MapMutationEvent, MapPlacementTransition},
    server::{ServerSession, ServerSessionPhase},
};

use super::{MAX_MAP_DYNAMIC_OUTBOX_EVENTS, MapDynamicOutbox};

pub(super) fn clear_world_object_tick_facts(
    mut damage: ResMut<crate::map::WorldTargetDamageFacts>,
    mut explosions: ResMut<crate::map::WorldObjectExplosionFacts>,
) {
    damage.0.clear();
    explosions.0.clear();
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy systems receive resource system parameters by value"
)]
pub(super) fn send_world_object_cues(
    mut outbox: ResMut<crate::map::WorldObjectOutbox>,
    links: Query<(Entity, &ServerSession, Has<Disconnected>), With<LinkOf>>,
    mut senders: Query<&mut MessageSender<crate::map::WorldObjectCue>, With<LinkOf>>,
    visibility: Res<crate::concealment::ObserverVisibilityCache>,
    fighters: Query<(Entity, &crate::protocol::NetworkEntityId), With<crate::protocol::Fighter>>,
) {
    if outbox.0.is_empty() {
        return;
    }
    outbox.0.sort_by_key(|cue| cue.event_id().0);
    let fighter_entities: std::collections::BTreeMap<_, _> = fighters
        .iter()
        .map(|(entity, network_id)| (network_id.0, entity))
        .collect();
    for (connection, session, disconnected) in &links {
        if disconnected || !matches!(session.phase, ServerSessionPhase::Active { .. }) {
            continue;
        }
        let Ok(mut sender) = senders.get_mut(connection) else {
            continue;
        };
        for cue in &outbox.0 {
            let permitted = cue.source_subject().is_none_or(|subject| {
                fighter_entities
                    .get(&subject.0)
                    .is_some_and(|entity| visibility.permits(connection, *entity))
            });
            if permitted {
                sender.send::<crate::protocol::CombatChannel>(*cue);
            }
        }
    }
    outbox.0.clear();
}

fn pending_world_damage_key(
    pending: &crate::map::PendingWorldTargetDamage,
) -> (u64, u8, u8, u8, u32) {
    (
        pending.attack_id.0,
        pending.delivery_index,
        pending.bundle_index,
        pending.effect_index,
        pending.target.placement_id().0,
    )
}

fn update_world_object_telemetry(
    world: &mut World,
    update: impl FnOnce(&mut crate::map::WorldObjectTelemetry),
) {
    update(&mut world.resource_mut::<crate::map::WorldObjectTelemetry>());
}

pub(super) fn process_world_target_damage(world: &mut World) {
    let Some(pending) = take_admitted_world_damage_batch(world) else {
        return;
    };
    apply_world_damage_batch(world, pending);
}

fn take_admitted_world_damage_batch(
    world: &mut World,
) -> Option<Vec<crate::map::PendingWorldTargetDamage>> {
    let active = world
        .query::<&crate::matchplay::MatchState>()
        .iter(world)
        .any(|state| matches!(state.phase, crate::matchplay::MatchPhase::Active { .. }));
    if !active {
        let rejected = world
            .resource::<crate::map::PendingWorldTargetDamages>()
            .0
            .len();
        update_world_object_telemetry(world, |telemetry| {
            telemetry.stale_or_invalid_rejections = telemetry
                .stale_or_invalid_rejections
                .saturating_add(u64::try_from(rejected).unwrap_or(u64::MAX));
        });
        world
            .resource_mut::<crate::map::PendingWorldTargetDamages>()
            .0
            .clear();
        return None;
    }
    let mut pending = std::mem::take(
        &mut world
            .resource_mut::<crate::map::PendingWorldTargetDamages>()
            .0,
    );
    if pending.is_empty() {
        return None;
    }
    update_world_object_telemetry(world, |telemetry| {
        telemetry.primary_requests = telemetry
            .primary_requests
            .saturating_add(u64::try_from(pending.len()).unwrap_or(u64::MAX));
    });
    pending.sort_by_key(pending_world_damage_key);
    pending.dedup_by_key(|request| pending_world_damage_key(request));
    if pending.len() > crate::map::MAX_WORLD_TARGET_FACTS {
        error!(
            requests = pending.len(),
            "world-target damage batch exceeds capacity"
        );
        update_world_object_telemetry(world, |telemetry| {
            telemetry.capacity_rejections = telemetry
                .capacity_rejections
                .saturating_add(u64::try_from(pending.len()).unwrap_or(u64::MAX));
        });
        return None;
    }
    if world.resource::<MapDynamicOutbox>().mutations.len() >= MAX_MAP_DYNAMIC_OUTBOX_EVENTS {
        error!("map dynamic outbox capacity exhausted; world-target batch rejected");
        update_world_object_telemetry(world, |telemetry| {
            telemetry.capacity_rejections = telemetry
                .capacity_rejections
                .saturating_add(u64::try_from(pending.len()).unwrap_or(u64::MAX));
        });
        return None;
    }
    Some(pending)
}

#[allow(
    clippy::too_many_lines,
    reason = "the bounded batch transaction keeps health, terminal reactions, chain damage, and cues ordered"
)]
fn apply_world_damage_batch(world: &mut World, pending: Vec<crate::map::PendingWorldTargetDamage>) {
    let Some((root, state)) = world
        .query_filtered::<(Entity, &MapDynamicState), With<crate::map::MapRoot>>()
        .iter(world)
        .next()
        .map(|(root, state)| (root, state.clone()))
    else {
        return;
    };
    let tick = world.resource::<crate::timing::SimulationTick>().0;
    let catalog = world.resource::<MapCatalogResource>().0.clone();
    let mut queue = std::collections::VecDeque::from(pending);
    let mut transitions = Vec::new();
    let mut reaction_count = 0_usize;
    let mut secondary_count = 0_usize;
    while let Some(request) = queue.pop_front() {
        if world
            .resource::<crate::map::WorldTargetDamageFacts>()
            .0
            .len()
            >= crate::map::MAX_WORLD_TARGET_FACTS
        {
            error!("world-target fact capacity exhausted");
            break;
        }
        if request.attack_id != request.source.attack_id
            || request.target.generation() != state.generation_id()
        {
            update_world_object_telemetry(world, |telemetry| {
                telemetry.stale_or_invalid_rejections =
                    telemetry.stale_or_invalid_rejections.saturating_add(1);
            });
            continue;
        }
        let target = {
            let mut objects = world.query_filtered::<(
                Entity,
                &crate::map::DamageableTargetIdentity,
                &Position,
                &crate::combat::CurrentHealth,
                &crate::map::DamageableLifeState,
                &crate::map::DamageableObjectProfile,
            ), With<crate::map::DamageableWorldObject>>();
            objects
                .iter(world)
                .find(|(_, identity, ..)| **identity == request.target)
                .map(|(entity, _, position, health, life, profile)| {
                    (entity, position.0, *health, *life, profile.0)
                })
        };
        let Some((entity, position, health, life, damage_profile_id)) = target else {
            update_world_object_telemetry(world, |telemetry| {
                telemetry.stale_or_invalid_rejections =
                    telemetry.stale_or_invalid_rejections.saturating_add(1);
            });
            continue;
        };
        if !crate::map::object_is_live(health, life) || request.requested_damage == 0 {
            update_world_object_telemetry(world, |telemetry| {
                telemetry.stale_or_invalid_rejections =
                    telemetry.stale_or_invalid_rejections.saturating_add(1);
            });
            continue;
        }
        let applied = request.requested_damage.min(health.0);
        let health_after = health.0 - applied;
        let damage_profile = *catalog
            .damage_profile(damage_profile_id)
            .expect("validated object damage profile exists");
        let terminal = (health_after == 0).then_some(match damage_profile.terminal {
            crate::map::MapObjectTerminalBehavior::Explode { outcome, .. }
            | crate::map::MapObjectTerminalBehavior::DropPickup { outcome, .. } => outcome,
        });
        if terminal.is_some() && reaction_count >= crate::map::MAX_TERMINAL_REACTIONS_PER_TICK {
            error!("barrel reaction ceiling reached; terminal request rejected");
            update_world_object_telemetry(world, |telemetry| {
                telemetry.capacity_rejections = telemetry.capacity_rejections.saturating_add(1);
            });
            continue;
        }
        if terminal.is_some()
            && matches!(
                damage_profile.terminal,
                crate::map::MapObjectTerminalBehavior::DropPickup { .. }
            )
        {
            let live_pickups = world
                .query_filtered::<Entity, With<crate::map::RestorationPickup>>()
                .iter(world)
                .count();
            if live_pickups >= crate::map::MAX_LIVE_RESTORATION_PICKUPS
                || world.resource::<crate::map::PickupLifecycleFacts>().0.len()
                    >= crate::map::MAX_PICKUP_FACTS
                || world.resource::<crate::map::PickupOutbox>().0.len()
                    >= crate::map::MAX_PICKUP_CUES
            {
                error!("pickup capacity exhausted; chest terminal request rejected");
                update_world_object_telemetry(world, |telemetry| {
                    telemetry.capacity_rejections = telemetry.capacity_rejections.saturating_add(1);
                });
                let mut pickup_telemetry = world.resource_mut::<crate::map::PickupTelemetry>();
                pickup_telemetry.capacity_rejections =
                    pickup_telemetry.capacity_rejections.saturating_add(1);
                continue;
            }
        }
        let Some(event_ids) = crate::combat::server::reserve_event_ids(
            &mut world.resource_mut::<crate::combat::NextCombatIds>(),
            if terminal.is_some() { 2 } else { 1 },
        ) else {
            error!("world-object event identity exhausted");
            break;
        };
        world
            .entity_mut(entity)
            .insert(crate::combat::CurrentHealth(health_after));
        {
            let mut telemetry = world.resource_mut::<crate::map::WorldObjectTelemetry>();
            telemetry.damage_applications = telemetry.damage_applications.saturating_add(1);
            telemetry.applied_damage = telemetry.applied_damage.saturating_add(u64::from(applied));
        }
        world
            .resource_mut::<crate::map::WorldTargetDamageFacts>()
            .0
            .push(crate::map::WorldTargetDamageFact {
                event_id: event_ids[0],
                tick,
                attack_id: request.attack_id,
                source: request.source,
                target: request.target,
                requested_damage: request.requested_damage,
                applied_damage: applied,
                health_after,
                terminal: terminal.map(crate::map::WorldTargetTerminalFact::MapPlacement),
            });
        world
            .resource_mut::<crate::map::WorldObjectOutbox>()
            .0
            .push(crate::map::WorldObjectCue::Damaged {
                event_id: event_ids[0],
                tick,
                attack_id: request.attack_id,
                source_subject: Some(request.source.owner_network_entity_id),
                target: request.target,
                position: crate::combat::WorldPoint::from(position),
                amount: applied,
                health_after,
            });
        let Some(outcome) = terminal else {
            continue;
        };
        reaction_count += 1;
        update_world_object_telemetry(world, |telemetry| {
            telemetry.terminal_reactions = telemetry.terminal_reactions.saturating_add(1);
        });
        world
            .entity_mut(entity)
            .insert(crate::map::DamageableLifeState::TerminalCommitted);
        transitions.push(MapPlacementTransition {
            placement_id: request.target.placement_id(),
            outcome,
        });
        let explosion_profile_id = match damage_profile.terminal {
            crate::map::MapObjectTerminalBehavior::Explode {
                explosion_profile_id,
                ..
            } => explosion_profile_id,
            crate::map::MapObjectTerminalBehavior::DropPickup {
                pickup_definition_id,
                ..
            } => {
                let identity = crate::map::RestorationPickupIdentity {
                    generation: request.target.generation(),
                    source_placement_id: request.target.placement_id(),
                };
                crate::map::pickups::spawn_restoration_pickup(
                    world,
                    identity,
                    pickup_definition_id,
                    position,
                    tick,
                    event_ids[1],
                )
                .expect("pickup capacity was reserved before committing the chest");
                world.entity_mut(entity).despawn();
                continue;
            }
        };
        let explosion = *catalog
            .explosion_profile(explosion_profile_id)
            .expect("validated explosion profile exists");
        world
            .resource_mut::<crate::map::WorldObjectExplosionFacts>()
            .0
            .push(crate::map::WorldObjectExplosionFact {
                event_id: event_ids[1],
                tick,
                source: request.source,
                target: request.target,
                position,
                radius: f32::from(explosion.radius_world_units),
                damage: explosion.damage,
            });
        world.resource_mut::<crate::combat::CombatOutbox>().0.push(
            crate::combat::CombatCue::Impact {
                event_id: event_ids[1],
                tick,
                source: request.source.owner_network_entity_id,
                shot_id: crate::combat::ShotId(request.attack_id.0),
                weapon_definition_id: crate::combat::WeaponDefinitionId(0),
                target: None,
                position: crate::combat::WorldPoint::from(position),
                normal: crate::combat::WorldPoint { x: 0.0, y: 1.0 },
                distance_band: crate::combat::DistanceBand::Close,
            },
        );
        world
            .resource_mut::<crate::map::WorldObjectOutbox>()
            .0
            .push(crate::map::WorldObjectCue::Exploded {
                event_id: event_ids[1],
                tick,
                attack_id: request.attack_id,
                source_subject: Some(request.source.owner_network_entity_id),
                target: request.target,
                position: crate::combat::WorldPoint::from(position),
                radius_world_units: explosion.radius_world_units,
                damage: explosion.damage,
            });
        let mut candidates: Vec<_> = {
            let mut objects = world.query_filtered::<(
                Entity,
                &crate::map::DamageableTargetIdentity,
                &Position,
                &crate::combat::CurrentHealth,
                &crate::map::DamageableLifeState,
            ), With<crate::map::DamageableWorldObject>>();
            objects
                .iter(world)
                .filter(|(_, identity, candidate_position, health, life)| {
                    **identity != request.target
                        && crate::map::object_is_live(**health, **life)
                        && candidate_position.0.distance_squared(position)
                            <= f32::from(explosion.radius_world_units).powi(2)
                })
                .map(|(candidate_entity, identity, candidate_position, ..)| {
                    (
                        candidate_position.0.distance_squared(position),
                        identity.placement_id(),
                        *identity,
                        candidate_entity,
                        candidate_position.0,
                    )
                })
                .collect()
        };
        candidates.sort_by(|left, right| {
            left.0
                .total_cmp(&right.0)
                .then_with(|| left.1.cmp(&right.1))
        });
        candidates.retain(|(_, _, _, candidate_entity, candidate_position)| {
            explosion_line_of_sight_clear(
                world,
                position,
                *candidate_position,
                entity,
                *candidate_entity,
            )
        });
        let selected_objects: Vec<_> = candidates
            .into_iter()
            .take(usize::from(explosion.maximum_targets))
            .collect();
        let remaining_targets =
            usize::from(explosion.maximum_targets).saturating_sub(selected_objects.len());
        for (_, _, target, _, _) in selected_objects {
            if secondary_count >= crate::map::MAX_SECONDARY_DAMAGE_APPLICATIONS {
                break;
            }
            secondary_count += 1;
            update_world_object_telemetry(world, |telemetry| {
                telemetry.chained_object_applications =
                    telemetry.chained_object_applications.saturating_add(1);
            });
            queue.push_back(crate::map::PendingWorldTargetDamage {
                target,
                source: request.source,
                attack_id: request.attack_id,
                requested_damage: explosion.damage,
                delivery_index: request.delivery_index,
                bundle_index: request.bundle_index,
                effect_index: u8::MAX,
            });
        }
        let combatant_applications = apply_explosion_to_combatants(
            world,
            ExplosionCombatantPlan {
                tick,
                source: request.source,
                cause: request.target,
                cause_entity: entity,
                position,
                damage: explosion.damage,
                radius: f32::from(explosion.radius_world_units),
                maximum_targets: remaining_targets,
            },
        );
        update_world_object_telemetry(world, |telemetry| {
            telemetry.secondary_combatant_applications = telemetry
                .secondary_combatant_applications
                .saturating_add(u64::try_from(combatant_applications).unwrap_or(u64::MAX));
        });
        world.entity_mut(entity).despawn();
    }
    commit_world_damage_transitions(world, root, state, transitions);
}

fn commit_world_damage_transitions(
    world: &mut World,
    root: Entity,
    mut state: MapDynamicState,
    mut transitions: Vec<MapPlacementTransition>,
) {
    if transitions.is_empty() {
        return;
    }
    transitions.sort_by_key(|transition| transition.placement_id);
    transitions.dedup_by_key(|transition| transition.placement_id);
    state.revision = state.revision.saturating_add(1);
    state.terminal_states.extend(transitions.iter().copied());
    state
        .terminal_states
        .sort_by_key(|transition| transition.placement_id);
    let event = MapMutationEvent {
        generation: state.generation_id(),
        revision: state.revision,
        transitions,
    };
    world
        .resource_mut::<MapDynamicOutbox>()
        .mutations
        .push(event);
    world.entity_mut(root).insert(state);
}

fn explosion_line_of_sight_clear(
    world: &mut World,
    origin: Vec2,
    target: Vec2,
    source_entity: Entity,
    target_entity: Entity,
) -> bool {
    let delta = target - origin;
    let distance = delta.length();
    let Some(direction) = Dir2::new(delta.normalize_or_zero()).ok() else {
        return true;
    };
    let mut system_state =
        bevy::ecs::system::SystemState::<avian2d::prelude::SpatialQuery>::new(world);
    let Ok(spatial_query) = system_state.get(world) else {
        error!("authoritative spatial-query state is unavailable for an environment explosion");
        return false;
    };
    let filter = avian2d::prelude::SpatialQueryFilter::from_mask(
        crate::movement::STATIC_MAP_LAYER | crate::movement::DESTRUCTIBLE_MAP_LAYER,
    )
    .with_excluded_entities([source_entity, target_entity]);
    spatial_query
        .cast_ray(origin, direction, distance.max(0.0), true, &filter)
        .is_none()
}

#[derive(Clone, Copy)]
struct ExplosionCombatantPlan {
    tick: u64,
    source: crate::combat::AttackSource,
    cause: crate::map::DamageableTargetIdentity,
    cause_entity: Entity,
    position: Vec2,
    damage: u16,
    radius: f32,
    maximum_targets: usize,
}

type ExplosionCombatantCandidate = (
    f32,
    u64,
    Entity,
    Vec2,
    crate::combat::CurrentHealth,
    crate::combat::TeamId,
    crate::protocol::NetworkEntityId,
    bool,
);

fn plan_explosion_combatants(
    world: &mut World,
    plan: &ExplosionCombatantPlan,
) -> (bool, Vec<ExplosionCombatantCandidate>) {
    let source = plan.source;
    let origin = plan.position;
    let radius = plan.radius;
    let cause_entity = plan.cause_entity;
    let active_match = world
        .query::<&crate::matchplay::MatchState>()
        .iter(world)
        .find_map(|state| {
            matches!(state.phase, crate::matchplay::MatchPhase::Active { .. })
                .then_some(state.match_id)
        });
    let lineage_is_current = active_match.is_some_and(|match_id| {
        world
            .query_filtered::<(
                &crate::protocol::PlayerId,
                &crate::protocol::NetworkEntityId,
                &crate::combat::TeamId,
                &crate::matchplay::MatchMember,
            ), (
                With<crate::protocol::Fighter>,
                With<crate::matchplay::ActiveCombatant>,
            )>()
            .iter(world)
            .any(|(player, network_id, team, member)| {
                *player == source.player_id
                    && *network_id == source.owner_network_entity_id
                    && *team == source.team_id
                    && member.0 == match_id
            })
    });
    let mut candidates: Vec<_> = {
        let mut query = world.query_filtered::<(
            Entity,
            &Position,
            &crate::combat::CurrentHealth,
            &crate::combat::TeamId,
            &crate::protocol::NetworkEntityId,
            Has<crate::abilities::Sentry>,
            Has<crate::combat::Defeated>,
        ), Or<(
            With<crate::protocol::Fighter>,
            With<crate::abilities::Sentry>,
        )>>();
        query
            .iter(world)
            .filter(|(_, position, health, _, _, _, defeated)| {
                !defeated && health.0 > 0 && position.0.distance_squared(origin) <= radius * radius
            })
            .map(|(entity, position, health, team, network_id, sentry, _)| {
                (
                    position.0.distance_squared(origin),
                    network_id.0,
                    entity,
                    position.0,
                    *health,
                    *team,
                    *network_id,
                    sentry,
                )
            })
            .collect()
    };
    candidates.sort_by(|left, right| {
        left.0
            .total_cmp(&right.0)
            .then_with(|| left.1.cmp(&right.1))
    });
    candidates.retain(|candidate| {
        explosion_line_of_sight_clear(world, origin, candidate.3, cause_entity, candidate.2)
    });
    (lineage_is_current, candidates)
}
#[allow(
    clippy::too_many_lines,
    reason = "the authoritative commit keeps combat-owned health, lifecycle, cues, and facts together"
)]
fn apply_explosion_to_combatants(world: &mut World, plan: ExplosionCombatantPlan) -> usize {
    let ExplosionCombatantPlan {
        tick,
        source,
        cause,
        cause_entity: _,
        position: origin,
        damage,
        radius: _,
        maximum_targets,
    } = plan;
    if maximum_targets == 0 {
        return 0;
    }
    let (lineage_is_current, candidates) = plan_explosion_combatants(world, &plan);
    let mut applied_targets = 0;
    for (_, _, entity, position, health, target_team, target_network_id, sentry) in
        candidates.into_iter().take(maximum_targets)
    {
        let applied = damage.min(health.0);
        let health_after = health.0 - applied;
        let defeated = health_after == 0;
        let event_count = if defeated { 2 } else { 1 };
        let Some(event_ids) = crate::combat::server::reserve_event_ids(
            &mut world.resource_mut::<crate::combat::NextCombatIds>(),
            event_count,
        ) else {
            error!("environment damage event identity exhausted");
            return applied_targets;
        };
        applied_targets += 1;
        world
            .entity_mut(entity)
            .insert(crate::combat::CurrentHealth(health_after));
        let target_kind = if sentry {
            crate::combat::CombatTargetKind::Deployable
        } else {
            crate::combat::CombatTargetKind::Fighter
        };
        let hostile_credit = lineage_is_current
            && source.team_id != target_team
            && source.owner_network_entity_id != target_network_id
            && source.team_id.0 <= 1;
        let source_team = hostile_credit.then_some(source.team_id);
        let source_kind = crate::combat::CombatSourceKind::Environment;
        let distance = origin.distance(position);
        world
            .resource_mut::<crate::combat::CombatOutcomeFacts>()
            .0
            .push(crate::combat::CombatOutcomeFact {
                event_id: event_ids[0],
                tick,
                attack_id: source.attack_id,
                source_kind,
                source_player: lineage_is_current.then_some(source.player_id),
                source_network_id: lineage_is_current.then_some(source.owner_network_entity_id),
                source_team,
                target_network_id,
                target_kind,
                target_team,
                preset_id: None,
                recipe_fingerprint: None,
                position: crate::combat::WorldPoint::from(position),
                engagement_distance: distance,
                kind: crate::combat::CombatOutcomeKind::Damage { amount: applied },
            });
        let generation = cause.generation();
        let damage_source = crate::combat::DamageSource::Environment {
            map_instance_id: generation.map_instance_id.0,
            generation: generation.generation,
            placement_id: cause.placement_id().0,
            initiating_player: lineage_is_current.then_some(source.player_id),
            initiating_fighter: lineage_is_current.then_some(source.owner_network_entity_id),
        };
        world.resource_mut::<crate::combat::CombatOutbox>().0.push(
            crate::combat::CombatCue::Damage {
                event_id: event_ids[0],
                tick,
                source: damage_source,
                target: target_network_id,
                amount: applied,
                health_after,
                distance_band: crate::combat::DistanceBand::Close,
            },
        );
        if !defeated {
            continue;
        }
        let defeat_event = event_ids[1];
        world
            .entity_mut(entity)
            .insert((
                crate::combat::Defeated {
                    event_id: defeat_event,
                },
                avian2d::prelude::CollisionLayers::new(
                    if sentry {
                        crate::movement::DEPLOYABLE_LAYER
                    } else {
                        crate::movement::FIGHTER_LAYER
                    },
                    avian2d::prelude::LayerMask::NONE,
                ),
                crate::combat::ActiveEffects::default(),
            ))
            .remove::<crate::combat::ExternalMotion>()
            .remove::<crate::combat::KnockbackFeedback>();
        world
            .resource_mut::<crate::combat::CombatOutcomeFacts>()
            .0
            .push(crate::combat::CombatOutcomeFact {
                event_id: defeat_event,
                tick,
                attack_id: source.attack_id,
                source_kind,
                source_player: lineage_is_current.then_some(source.player_id),
                source_network_id: lineage_is_current.then_some(source.owner_network_entity_id),
                source_team,
                target_network_id,
                target_kind,
                target_team,
                preset_id: None,
                recipe_fingerprint: None,
                position: crate::combat::WorldPoint::from(position),
                engagement_distance: distance,
                kind: if sentry {
                    crate::combat::CombatOutcomeKind::DeployableDestroyed
                } else {
                    crate::combat::CombatOutcomeKind::Defeat
                },
            });
        world.resource_mut::<crate::combat::CombatOutbox>().0.push(
            crate::combat::CombatCue::Defeat {
                event_id: defeat_event,
                tick,
                source: Some(damage_source),
                target: target_network_id,
            },
        );
    }
    applied_targets
}
