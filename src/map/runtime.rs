//! Authoritative map installation and whole-cell destruction.

use super::{
    MapCatalogResource, MapDestructionBehavior, MapDynamicGeneration, MapDynamicRecoveryRequest,
    MapDynamicRecoverySnapshot, MapDynamicResetEvent, MapDynamicState, MapInstanceId,
    MapInstanceMember, MapMutationEvent, MapPlacementId, MapPlacementOutcome,
    MapPlacementTransition, ResolvedMap, ResolvedMapSnapshot, placement_cells,
    placement_world_center,
};
use crate::combat::{CombatWorldEffectFact, CombatWorldEffectFacts, WorldEffectDefinition};
use crate::movement::{
    ArenaWall, destructible_map_collision_layers, player_only_map_collision_layers,
};
use crate::protocol::MapDynamicChannel;
use crate::server::{ServerSession, ServerSessionPhase};
use avian2d::prelude::{Collider, Position, RigidBody, Rotation};
use bevy::prelude::*;
use lightyear::prelude::{
    Disconnected, LinkOf, MessageReceiver, MessageSender, NetworkTarget, Replicate,
};

const MAX_MAP_DYNAMIC_OUTBOX_EVENTS: usize = 256;
const MAX_RECOVERY_RESPONSES_PER_GENERATION: u8 = 4;

#[derive(Resource, Default)]
struct MapDynamicOutbox {
    mutations: Vec<MapMutationEvent>,
    reset: Option<MapDynamicResetEvent>,
}

/// Bounded process-lifetime evidence for map destruction and recovery traffic.
#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct MapDynamicTelemetry {
    pub destruction_requests: u64,
    pub destruction_applied: u64,
    pub destruction_no_ops: u64,
    pub placements_changed: u64,
    pub recovery_requests: u64,
    pub recovery_responses: u64,
    pub recovery_rejections: u64,
    recovery_admission: std::collections::BTreeMap<Entity, (MapDynamicGeneration, u8)>,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct DestructibleMapCollider {
    pub placement_id: MapPlacementId,
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
struct PlayerOnlyMapCollider;

#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum MapRuntimeSet {
    ApplyDestruction,
    Publish,
}

pub fn register_map_runtime(app: &mut App) {
    app.init_resource::<CombatWorldEffectFacts>()
        .init_resource::<super::PendingWorldTargetDamages>()
        .init_resource::<super::WorldTargetDamageFacts>()
        .init_resource::<super::WorldObjectExplosionFacts>()
        .init_resource::<super::WorldObjectOutbox>()
        .init_resource::<super::WorldObjectTelemetry>()
        .init_resource::<MapDynamicOutbox>()
        .init_resource::<MapDynamicTelemetry>()
        .configure_sets(
            FixedPostUpdate,
            (
                MapRuntimeSet::ApplyDestruction
                    .after(crate::abilities::AbilitySet::ObserveOutcomes)
                    .after(crate::combat::CombatSet::Damage),
                MapRuntimeSet::Publish.after(MapRuntimeSet::ApplyDestruction),
            )
                .before(crate::matchplay::MatchSet::ModeRules),
        )
        .add_systems(
            FixedPostUpdate,
            apply_map_destruction.in_set(MapRuntimeSet::ApplyDestruction),
        )
        .add_systems(
            FixedPostUpdate,
            process_world_target_damage.in_set(crate::combat::CombatDamageSet::WorldTargets),
        )
        .add_systems(
            FixedPostUpdate,
            publish_map_dynamic_traffic.in_set(MapRuntimeSet::Publish),
        )
        .add_systems(
            FixedPostUpdate,
            send_world_object_cues
                .in_set(crate::combat::CombatSet::TelemetryAndCues)
                .after(crate::concealment::ConcealmentSet::DecideObservers),
        )
        .add_systems(
            FixedPostUpdate,
            clear_world_object_tick_facts
                .in_set(crate::combat::CombatSet::Finalize)
                .before(crate::gameplay::advance_simulation_tick),
        )
        .add_systems(Update, receive_map_recovery_requests);
    crate::matchplay::register_environment_reset_system(app, reset_map_on_match_restart);
}

fn clear_world_object_tick_facts(
    mut damage: ResMut<super::WorldTargetDamageFacts>,
    mut explosions: ResMut<super::WorldObjectExplosionFacts>,
) {
    damage.0.clear();
    explosions.0.clear();
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy systems receive resource system parameters by value"
)]
fn send_world_object_cues(
    mut outbox: ResMut<super::WorldObjectOutbox>,
    links: Query<(Entity, &ServerSession, Has<Disconnected>), With<LinkOf>>,
    mut senders: Query<&mut MessageSender<super::WorldObjectCue>, With<LinkOf>>,
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

fn pending_world_damage_key(pending: &super::PendingWorldTargetDamage) -> (u64, u8, u8, u8, u32) {
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
    update: impl FnOnce(&mut super::WorldObjectTelemetry),
) {
    update(&mut world.resource_mut::<super::WorldObjectTelemetry>());
}

#[allow(
    clippy::too_many_lines,
    reason = "the exclusive system commits one bounded health, terminal, collider, chain, and map-state transaction"
)]
fn process_world_target_damage(world: &mut World) {
    let active = world
        .query::<&crate::matchplay::MatchState>()
        .iter(world)
        .any(|state| matches!(state.phase, crate::matchplay::MatchPhase::Active { .. }));
    if !active {
        let rejected = world.resource::<super::PendingWorldTargetDamages>().0.len();
        update_world_object_telemetry(world, |telemetry| {
            telemetry.stale_or_invalid_rejections = telemetry
                .stale_or_invalid_rejections
                .saturating_add(u64::try_from(rejected).unwrap_or(u64::MAX));
        });
        world
            .resource_mut::<super::PendingWorldTargetDamages>()
            .0
            .clear();
        return;
    }
    let mut pending =
        std::mem::take(&mut world.resource_mut::<super::PendingWorldTargetDamages>().0);
    if pending.is_empty() {
        return;
    }
    update_world_object_telemetry(world, |telemetry| {
        telemetry.primary_requests = telemetry
            .primary_requests
            .saturating_add(u64::try_from(pending.len()).unwrap_or(u64::MAX));
    });
    pending.sort_by_key(pending_world_damage_key);
    pending.dedup_by_key(|request| pending_world_damage_key(request));
    if pending.len() > super::MAX_WORLD_TARGET_FACTS {
        error!(
            requests = pending.len(),
            "world-target damage batch exceeds capacity"
        );
        update_world_object_telemetry(world, |telemetry| {
            telemetry.capacity_rejections = telemetry
                .capacity_rejections
                .saturating_add(u64::try_from(pending.len()).unwrap_or(u64::MAX));
        });
        return;
    }
    if world.resource::<MapDynamicOutbox>().mutations.len() >= MAX_MAP_DYNAMIC_OUTBOX_EVENTS {
        error!("map dynamic outbox capacity exhausted; world-target batch rejected");
        update_world_object_telemetry(world, |telemetry| {
            telemetry.capacity_rejections = telemetry
                .capacity_rejections
                .saturating_add(u64::try_from(pending.len()).unwrap_or(u64::MAX));
        });
        return;
    }
    let Some((root, mut state)) = world
        .query_filtered::<(Entity, &MapDynamicState), With<super::MapRoot>>()
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
        if world.resource::<super::WorldTargetDamageFacts>().0.len()
            >= super::MAX_WORLD_TARGET_FACTS
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
                &super::DamageableTargetIdentity,
                &Position,
                &crate::combat::CurrentHealth,
                &super::DamageableLifeState,
                &super::DamageableObjectProfile,
            ), With<super::DamageableWorldObject>>();
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
        if !super::object_is_live(health, life) || request.requested_damage == 0 {
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
            super::MapObjectTerminalBehavior::Explode { outcome, .. } => outcome,
        });
        if terminal.is_some() && reaction_count >= super::MAX_TERMINAL_REACTIONS_PER_TICK {
            error!("barrel reaction ceiling reached; terminal request rejected");
            update_world_object_telemetry(world, |telemetry| {
                telemetry.capacity_rejections = telemetry.capacity_rejections.saturating_add(1);
            });
            continue;
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
            let mut telemetry = world.resource_mut::<super::WorldObjectTelemetry>();
            telemetry.damage_applications = telemetry.damage_applications.saturating_add(1);
            telemetry.applied_damage = telemetry.applied_damage.saturating_add(u64::from(applied));
        }
        world
            .resource_mut::<super::WorldTargetDamageFacts>()
            .0
            .push(super::WorldTargetDamageFact {
                event_id: event_ids[0],
                tick,
                attack_id: request.attack_id,
                source: request.source,
                target: request.target,
                requested_damage: request.requested_damage,
                applied_damage: applied,
                health_after,
                terminal: terminal.map(super::WorldTargetTerminalFact::MapPlacement),
            });
        world
            .resource_mut::<super::WorldObjectOutbox>()
            .0
            .push(super::WorldObjectCue::Damaged {
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
            .insert(super::DamageableLifeState::TerminalCommitted);
        transitions.push(MapPlacementTransition {
            placement_id: request.target.placement_id(),
            outcome,
        });
        let super::MapObjectTerminalBehavior::Explode {
            explosion_profile_id,
            ..
        } = damage_profile.terminal;
        let explosion = *catalog
            .explosion_profile(explosion_profile_id)
            .expect("validated explosion profile exists");
        world
            .resource_mut::<super::WorldObjectExplosionFacts>()
            .0
            .push(super::WorldObjectExplosionFact {
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
            .resource_mut::<super::WorldObjectOutbox>()
            .0
            .push(super::WorldObjectCue::Exploded {
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
                &super::DamageableTargetIdentity,
                &Position,
                &crate::combat::CurrentHealth,
                &super::DamageableLifeState,
            ), With<super::DamageableWorldObject>>();
            objects
                .iter(world)
                .filter(|(_, identity, candidate_position, health, life)| {
                    **identity != request.target
                        && super::object_is_live(**health, **life)
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
            if secondary_count >= super::MAX_SECONDARY_DAMAGE_APPLICATIONS {
                break;
            }
            secondary_count += 1;
            update_world_object_telemetry(world, |telemetry| {
                telemetry.chained_object_applications =
                    telemetry.chained_object_applications.saturating_add(1);
            });
            queue.push_back(super::PendingWorldTargetDamage {
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
            tick,
            request.source,
            request.target,
            entity,
            position,
            explosion.damage,
            f32::from(explosion.radius_world_units),
            remaining_targets,
        );
        update_world_object_telemetry(world, |telemetry| {
            telemetry.secondary_combatant_applications = telemetry
                .secondary_combatant_applications
                .saturating_add(u64::try_from(combatant_applications).unwrap_or(u64::MAX));
        });
        world.entity_mut(entity).despawn();
    }
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

#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "environmental damage must update combat-owned health, lifecycle, cues, and facts together"
)]
fn apply_explosion_to_combatants(
    world: &mut World,
    tick: u64,
    source: crate::combat::AttackSource,
    cause: super::DamageableTargetIdentity,
    cause_entity: Entity,
    origin: Vec2,
    damage: u16,
    radius: f32,
    maximum_targets: usize,
) -> usize {
    if maximum_targets == 0 {
        return 0;
    }
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

pub fn install_resolved_map(world: &mut World, resolved: ResolvedMap) -> Result<(), String> {
    let instance_id = resolved.snapshot.identity.instance_id;
    if instance_id.0 == 0 {
        return Err("cannot install a zero map instance".to_string());
    }
    let snapshot = resolved.snapshot.clone();
    let dynamic_placements = resolved.dynamic_placements.clone();
    let player_only_surface_rects = resolved.player_only_surface_rects.clone();
    let static_colliders = resolved.static_colliders.clone();
    let spawn_points = resolved.spawn_points_by_team.clone();
    let objective_zone = resolved.objective_zone;
    super::server::teardown_authoritative_map(world);
    if let Some(anchor) = objective_zone {
        world.insert_resource(crate::matchplay::ResolvedObjectiveZone {
            anchor_id: anchor.anchor_id,
            area: anchor.area,
        });
    }
    *world.resource_mut::<MapDynamicOutbox>() = MapDynamicOutbox::default();
    world.spawn((
        super::MapRoot,
        instance_id,
        snapshot.identity,
        snapshot.clone(),
        MapDynamicState {
            map_instance_id: instance_id,
            generation: 1,
            revision: 0,
            terminal_states: Vec::new(),
        },
        Replicate::to_clients(NetworkTarget::All),
    ));
    for (index, (position, size)) in
        super::server::perimeter_wall_shapes(snapshot.dimensions.bounds())
            .into_iter()
            .enumerate()
    {
        spawn_static_collider(
            world,
            instance_id,
            MapPlacementId(u32::MAX - u32::try_from(index).expect("four perimeter indices fit")),
            position,
            super::MapShape::Rectangle {
                half_extents: size * 0.5,
            },
        );
    }
    for collider in static_colliders {
        spawn_static_collider(
            world,
            instance_id,
            collider.placement_id,
            collider.position,
            collider.shape,
        );
    }
    world.insert_resource(super::PlayableBounds(snapshot.dimensions.bounds()));
    world.insert_resource(super::SpawnPointCatalog(spawn_points));
    world.insert_resource(resolved);
    let catalog = world.resource::<MapCatalogResource>().0.clone();
    for placement in dynamic_placements {
        let asset = catalog
            .asset(placement.asset_id)
            .ok_or_else(|| "resolved dynamic asset disappeared".to_string())?;
        spawn_dynamic_collider(world, instance_id, 1, &snapshot, asset, &placement);
    }
    for rectangle in player_only_surface_rects {
        let size = Vec2::new(f32::from(rectangle.width), f32::from(rectangle.height))
            * super::MAP_CELL_SIZE_WORLD;
        let center = snapshot.dimensions.cell_min(rectangle.min) + size * 0.5;
        let placement_id = snapshot
            .placements
            .iter()
            .filter(|placement| {
                placement.cell.x >= rectangle.min.x
                    && placement.cell.x < rectangle.min.x + rectangle.width
                    && placement.cell.y >= rectangle.min.y
                    && placement.cell.y < rectangle.min.y + rectangle.height
            })
            .map(|placement| placement.placement_id)
            .min()
            .ok_or_else(|| "player-only surface placement disappeared".to_string())?;
        world.spawn((
            ArenaWall,
            PlayerOnlyMapCollider,
            MapInstanceMember {
                map_instance_id: instance_id,
                placement_id,
            },
            RigidBody::Static,
            Collider::rectangle(size.x, size.y),
            player_only_map_collision_layers(),
            Position::from_xy(center.x, center.y),
            Rotation::default(),
            Transform::from_translation(center.extend(0.0)),
        ));
    }
    Ok(())
}

fn spawn_static_collider(
    world: &mut World,
    instance_id: MapInstanceId,
    placement_id: MapPlacementId,
    position: Vec2,
    shape: super::MapShape,
) {
    let collider = match shape {
        super::MapShape::Rectangle { half_extents } => {
            Collider::rectangle(half_extents.x * 2.0, half_extents.y * 2.0)
        }
        super::MapShape::Circle { radius } => Collider::circle(radius),
    };
    world.spawn((
        ArenaWall,
        MapInstanceMember {
            map_instance_id: instance_id,
            placement_id,
        },
        RigidBody::Static,
        collider,
        crate::movement::map_collision_layers(),
        Position::from_xy(position.x, position.y),
        Rotation::default(),
        Transform::from_translation(position.extend(0.0)),
    ));
}

fn spawn_dynamic_collider(
    world: &mut World,
    map_instance_id: MapInstanceId,
    map_generation: u64,
    snapshot: &ResolvedMapSnapshot,
    asset: &super::MapAssetDefinition,
    placement: &super::MapAssetPlacement,
) {
    let footprint = asset.footprint_cells.rotated(placement.quarter_turns);
    let center = placement_world_center(snapshot.dimensions, asset, placement);
    let profile = world
        .resource::<MapCatalogResource>()
        .0
        .profile(asset.gameplay_profile_id)
        .copied()
        .expect("resolved dynamic asset profile exists");
    let collider = match profile.collider_shape {
        super::MapColliderShape::FootprintRectangle => Collider::rectangle(
            f32::from(footprint.width) * super::MAP_CELL_SIZE_WORLD,
            f32::from(footprint.height) * super::MAP_CELL_SIZE_WORLD,
        ),
        super::MapColliderShape::Circle { radius_world_units } => {
            Collider::circle(f32::from(radius_world_units))
        }
        super::MapColliderShape::None => return,
    };
    let entity = world
        .spawn((
            ArenaWall,
            MapInstanceMember {
                map_instance_id,
                placement_id: placement.placement_id,
            },
            RigidBody::Static,
            collider,
            destructible_map_collision_layers(),
            Position::from_xy(center.x, center.y),
            Rotation::default(),
            Transform::from_translation(center.extend(0.0)),
        ))
        .id();
    match profile.durability {
        super::MapDurabilityBehavior::Indestructible => {
            world.entity_mut(entity).insert(DestructibleMapCollider {
                placement_id: placement.placement_id,
            });
        }
        super::MapDurabilityBehavior::HitPoints(damage_profile_id) => {
            let maximum_health = world
                .resource::<MapCatalogResource>()
                .0
                .damage_profile(damage_profile_id)
                .expect("validated damage profile exists")
                .maximum_health;
            world.entity_mut(entity).insert((
                super::DamageableWorldObject,
                super::DamageableTargetIdentity::MapObject {
                    generation: MapDynamicGeneration {
                        map_instance_id,
                        generation: map_generation,
                    },
                    placement_id: placement.placement_id,
                },
                super::DamageableTargetClass::EnvironmentObject,
                super::DamageableMaximumHealth(maximum_health),
                super::DamageableObjectProfile(damage_profile_id),
                super::DamageableObjectAsset(asset.id),
                super::DamageableLifeState::Live,
                crate::combat::CurrentHealth(maximum_health),
                Replicate::to_clients(NetworkTarget::All),
            ));
        }
    }
}

fn fact_key(fact: &CombatWorldEffectFact) -> (u64, u64, u8, u8) {
    (
        fact.tick,
        fact.source.attack_id.0,
        fact.delivery_index,
        fact.effect_index,
    )
}

fn circle_overlaps_cell(center: Vec2, radius: f32, min: Vec2) -> bool {
    let max = min + Vec2::splat(super::MAP_CELL_SIZE_WORLD);
    let closest = center.clamp(min, max);
    center.distance_squared(closest) <= radius * radius
}

fn destruction_outcome_at(
    catalog: &super::MapContentCatalog,
    snapshot: &ResolvedMapSnapshot,
    placement: &super::MapAssetPlacement,
    position: Vec2,
    radius: f32,
) -> Option<MapPlacementOutcome> {
    let asset = catalog.asset(placement.asset_id)?;
    let profile = catalog.profile(asset.gameplay_profile_id)?;
    let outcome = match profile.destruction {
        MapDestructionBehavior::Indestructible => return None,
        MapDestructionBehavior::RemoveOnMapDestruction => MapPlacementOutcome::Removed,
        MapDestructionBehavior::ReplaceOnMapDestruction(asset_id) => {
            MapPlacementOutcome::ReplacedWith(asset_id)
        }
    };
    placement_cells(snapshot.dimensions, asset, placement)
        .is_some_and(|cells| {
            cells.into_iter().any(|cell| {
                circle_overlaps_cell(position, radius, snapshot.dimensions.cell_min(cell))
            })
        })
        .then_some(outcome)
}

fn record_destruction_telemetry(
    world: &mut World,
    requests: u64,
    applied: u64,
    placements_changed: u64,
) {
    let mut telemetry = world.resource_mut::<MapDynamicTelemetry>();
    telemetry.destruction_applied = telemetry.destruction_applied.saturating_add(applied);
    telemetry.destruction_no_ops = telemetry
        .destruction_no_ops
        .saturating_add(requests.saturating_sub(applied));
    telemetry.placements_changed = telemetry
        .placements_changed
        .saturating_add(placements_changed);
}

fn apply_map_destruction(world: &mut World) {
    let Some((root_entity, snapshot, mut state)) = world
        .query_filtered::<(Entity, &ResolvedMapSnapshot, &MapDynamicState), With<super::MapRoot>>()
        .iter(world)
        .next()
        .map(|(entity, snapshot, state)| (entity, snapshot.clone(), state.clone()))
    else {
        return;
    };
    if world.resource::<MapDynamicOutbox>().mutations.len() >= MAX_MAP_DYNAMIC_OUTBOX_EVENTS {
        error!("map dynamic outbox capacity exceeded; destruction batch deferred");
        return;
    }
    let mut facts = std::mem::take(&mut world.resource_mut::<CombatWorldEffectFacts>().0);
    facts.sort_by_key(fact_key);
    facts.dedup_by_key(|fact| fact_key(fact));
    let requests = u64::try_from(facts.len()).unwrap_or(u64::MAX);
    let mut telemetry = world.resource_mut::<MapDynamicTelemetry>();
    telemetry.destruction_requests = telemetry.destruction_requests.saturating_add(requests);
    let available = MAX_MAP_DYNAMIC_OUTBOX_EVENTS
        .saturating_sub(world.resource::<MapDynamicOutbox>().mutations.len());
    if facts.len() > available {
        world.resource_mut::<CombatWorldEffectFacts>().0 = facts;
        error!("map destruction batch exceeds the bounded transaction capacity");
        return;
    }
    let catalog = world.resource::<MapCatalogResource>().0.clone();
    let mut terminal: std::collections::BTreeMap<_, _> = state
        .terminal_states
        .iter()
        .map(|transition| (transition.placement_id, transition.outcome))
        .collect();
    let mut newly_terminal = std::collections::BTreeSet::new();
    let mut events = Vec::new();
    for fact in facts {
        let WorldEffectDefinition::DestroyMap { radius } = fact.effect;
        let mut transitions = Vec::new();
        for placement in &snapshot.placements {
            if terminal.contains_key(&placement.placement_id) {
                continue;
            }
            if let Some(outcome) = destruction_outcome_at(
                &catalog,
                &snapshot,
                placement,
                fact.position.as_vec2(),
                radius,
            ) {
                terminal.insert(placement.placement_id, outcome);
                newly_terminal.insert(placement.placement_id);
                transitions.push(MapPlacementTransition {
                    placement_id: placement.placement_id,
                    outcome,
                });
            }
        }
        transitions.sort_by_key(|transition| transition.placement_id);
        if !transitions.is_empty() {
            state.revision = state
                .revision
                .checked_add(1)
                .expect("map dynamic revision space is available");
            events.push(MapMutationEvent {
                generation: state.generation_id(),
                revision: state.revision,
                transitions,
            });
        }
    }
    let applied = u64::try_from(events.len()).unwrap_or(u64::MAX);
    let changed = u64::try_from(newly_terminal.len()).unwrap_or(u64::MAX);
    record_destruction_telemetry(world, requests, applied, changed);
    if newly_terminal.is_empty() {
        return;
    }
    let collider_entities: Vec<_> = world
        .query::<(Entity, &DestructibleMapCollider)>()
        .iter(world)
        .filter(|(_, collider)| newly_terminal.contains(&collider.placement_id))
        .map(|(entity, _)| entity)
        .collect();
    for entity in collider_entities {
        world.entity_mut(entity).despawn();
    }
    state.terminal_states = terminal
        .into_iter()
        .map(|(placement_id, outcome)| MapPlacementTransition {
            placement_id,
            outcome,
        })
        .collect();
    world
        .resource_mut::<MapDynamicOutbox>()
        .mutations
        .extend(events);
    world.entity_mut(root_entity).insert(state);
}

fn reset_map_on_match_restart(world: &mut World) {
    if world
        .get_resource::<crate::matchplay::PendingMatchRestart>()
        .and_then(crate::matchplay::PendingMatchRestart::slot)
        .is_none()
    {
        return;
    }
    restore_map(world);
}

fn restore_map(world: &mut World) {
    let Some((root_entity, snapshot, mut state)) = world
        .query_filtered::<(Entity, &ResolvedMapSnapshot, &MapDynamicState), With<super::MapRoot>>()
        .iter(world)
        .next()
        .map(|(entity, snapshot, state)| (entity, snapshot.clone(), state.clone()))
    else {
        return;
    };
    world.resource_mut::<CombatWorldEffectFacts>().0.clear();
    if let Some(mut pending) = world.get_resource_mut::<super::PendingWorldTargetDamages>() {
        pending.0.clear();
    }
    if let Some(mut facts) = world.get_resource_mut::<super::WorldTargetDamageFacts>() {
        facts.0.clear();
    }
    if let Some(mut facts) = world.get_resource_mut::<super::WorldObjectExplosionFacts>() {
        facts.0.clear();
    }
    if let Some(mut outbox) = world.get_resource_mut::<super::WorldObjectOutbox>() {
        outbox.0.clear();
    }
    let damageable_entities: Vec<_> = world
        .query_filtered::<Entity, With<super::DamageableWorldObject>>()
        .iter(world)
        .collect();
    for entity in damageable_entities {
        world.entity_mut(entity).despawn();
    }
    let previous_generation = state.generation_id();
    state.generation = state.generation.saturating_add(1);
    state.revision = 0;
    state.terminal_states.clear();
    let existing: std::collections::BTreeSet<_> = world
        .query::<&DestructibleMapCollider>()
        .iter(world)
        .map(|collider| collider.placement_id)
        .collect();
    let catalog = world.resource::<MapCatalogResource>().0.clone();
    for placement in &snapshot.placements {
        let Some(asset) = catalog.asset(placement.asset_id) else {
            continue;
        };
        let dynamic = catalog
            .profile(asset.gameplay_profile_id)
            .is_some_and(|profile| {
                profile.destruction != MapDestructionBehavior::Indestructible
                    || profile.durability != super::MapDurabilityBehavior::Indestructible
            });
        if dynamic && !existing.contains(&placement.placement_id) {
            spawn_dynamic_collider(
                world,
                state.map_instance_id,
                state.generation,
                &snapshot,
                asset,
                placement,
            );
        }
    }
    let mut outbox = world.resource_mut::<MapDynamicOutbox>();
    outbox.mutations.clear();
    outbox.reset = Some(MapDynamicResetEvent {
        previous_generation,
        next_generation: state.generation_id(),
    });
    world.entity_mut(root_entity).insert(state);
}

#[allow(
    clippy::type_complexity,
    reason = "the request receiver and response sender are distinct Lightyear link components"
)]
fn receive_map_recovery_requests(
    roots: Query<&MapDynamicState, With<super::MapRoot>>,
    links: Query<(Entity, &ServerSession, Has<Disconnected>), With<LinkOf>>,
    mut requests: Query<
        (
            Entity,
            &mut MessageReceiver<MapDynamicRecoveryRequest>,
            &mut MessageSender<MapDynamicRecoverySnapshot>,
        ),
        With<LinkOf>,
    >,
    mut telemetry: ResMut<MapDynamicTelemetry>,
) {
    let Ok(state) = roots.single() else {
        return;
    };
    telemetry
        .recovery_admission
        .retain(|link, _| links.get(*link).is_ok());
    for (link, mut receiver, mut sender) in &mut requests {
        let accepted = links.get(link).is_ok_and(|(_, session, disconnected)| {
            matches!(session.phase, ServerSessionPhase::Active { .. }) && !disconnected
        });
        for request in receiver.receive() {
            telemetry.recovery_requests = telemetry.recovery_requests.saturating_add(1);
            let admission = telemetry
                .recovery_admission
                .entry(link)
                .or_insert((request.generation, 0));
            if admission.0 != request.generation {
                *admission = (request.generation, 0);
            }
            if recovery_request_is_admitted(
                accepted,
                request.generation,
                state.generation_id(),
                admission.1,
            ) {
                admission.1 += 1;
                sender.send::<MapDynamicChannel>(MapDynamicRecoverySnapshot {
                    state: state.clone(),
                });
                telemetry.recovery_responses = telemetry.recovery_responses.saturating_add(1);
            } else {
                telemetry.recovery_rejections = telemetry.recovery_rejections.saturating_add(1);
            }
        }
    }
}

fn recovery_request_is_admitted(
    active_session: bool,
    requested: MapDynamicGeneration,
    current: MapDynamicGeneration,
    responses: u8,
) -> bool {
    active_session && requested == current && responses < MAX_RECOVERY_RESPONSES_PER_GENERATION
}

fn publish_map_dynamic_traffic(
    mut outbox: ResMut<MapDynamicOutbox>,
    links: Query<(Entity, &ServerSession, Has<Disconnected>), With<LinkOf>>,
    mut mutation_senders: Query<&mut MessageSender<MapMutationEvent>, With<LinkOf>>,
    mut reset_senders: Query<&mut MessageSender<MapDynamicResetEvent>, With<LinkOf>>,
) {
    let accepted: Vec<_> = links
        .iter()
        .filter(|(_, session, disconnected)| {
            matches!(session.phase, ServerSessionPhase::Active { .. }) && !disconnected
        })
        .map(|(entity, _, _)| entity)
        .collect();
    if let Some(reset) = outbox.reset.take() {
        for link in &accepted {
            if let Ok(mut sender) = reset_senders.get_mut(*link) {
                sender.send::<MapDynamicChannel>(reset);
            }
        }
    }
    for event in outbox.mutations.drain(..) {
        for link in &accepted {
            if let Ok(mut sender) = mutation_senders.get_mut(*link) {
                sender.send::<MapDynamicChannel>(event.clone());
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::combat::{AttackId, AttackSource, CombatSourceKind, WorldPoint};
    use crate::map::{MapCatalogResource, MapRoot};

    fn test_attack_source() -> AttackSource {
        AttackSource {
            kind: CombatSourceKind::PrimaryWeapon,
            attack_id: AttackId(41),
            player_id: crate::protocol::PlayerId(7),
            owner_network_entity_id: crate::protocol::NetworkEntityId(70),
            team_id: crate::combat::TeamId(0),
            recipe_fingerprint: crate::combat::WeaponRecipeFingerprint::default(),
            presentation_profile_id: crate::combat::WeaponPresentationProfileId(3),
            legacy_compatibility: false,
            source_preset_id: None,
            origin: WorldPoint { x: 0.0, y: 0.0 },
            facing: 0.0,
        }
    }

    fn barrel_test_app() -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_plugins(avian2d::prelude::PhysicsPlugins::default())
            .init_resource::<CombatWorldEffectFacts>()
            .init_resource::<MapDynamicOutbox>()
            .init_resource::<MapDynamicTelemetry>()
            .init_resource::<MapCatalogResource>()
            .init_resource::<super::super::PendingWorldTargetDamages>()
            .init_resource::<super::super::WorldTargetDamageFacts>()
            .init_resource::<super::super::WorldObjectExplosionFacts>()
            .init_resource::<super::super::WorldObjectOutbox>()
            .init_resource::<super::super::WorldObjectTelemetry>()
            .init_resource::<crate::combat::CombatOutcomeFacts>()
            .init_resource::<crate::combat::CombatOutbox>()
            .init_resource::<crate::combat::NextCombatIds>()
            .insert_resource(crate::timing::SimulationTick(9));
        let resolved = app
            .world()
            .resource::<MapCatalogResource>()
            .0
            .resolve_preset(super::super::FEATURE_YARD_WIPEOUT_PRESET, MapInstanceId(11))
            .unwrap();
        let snapshot = resolved.snapshot.clone();
        let root = app
            .world_mut()
            .spawn((
                MapRoot,
                snapshot.clone(),
                MapDynamicState {
                    map_instance_id: MapInstanceId(11),
                    generation: 1,
                    revision: 0,
                    terminal_states: Vec::new(),
                },
            ))
            .id();
        let catalog = app.world().resource::<MapCatalogResource>().0.clone();
        for placement in &resolved.dynamic_placements {
            spawn_dynamic_collider(
                app.world_mut(),
                MapInstanceId(11),
                1,
                &snapshot,
                catalog.asset(placement.asset_id).unwrap(),
                placement,
            );
        }
        app.world_mut().spawn(crate::matchplay::MatchState {
            match_id: crate::matchplay::MatchId(1),
            mode_definition_id: snapshot.mode_definition_id,
            phase: crate::matchplay::MatchPhase::Active { ends_at_tick: 999 },
            rules_revision: 1,
        });
        (app, root)
    }

    fn barrel_identity(app: &mut App, placement_id: u32) -> super::super::DamageableTargetIdentity {
        let world = app.world_mut();
        let mut query = world.query::<&super::super::DamageableTargetIdentity>();
        *query
            .iter(world)
            .find(|identity| identity.placement_id() == MapPlacementId(placement_id))
            .unwrap()
    }

    fn barrel_health(app: &mut App, placement_id: u32) -> Option<u16> {
        let world = app.world_mut();
        let mut query = world.query::<(
            &super::super::DamageableTargetIdentity,
            &crate::combat::CurrentHealth,
        )>();
        query
            .iter(world)
            .find(|(identity, _)| identity.placement_id() == MapPlacementId(placement_id))
            .map(|(_, health)| health.0)
    }

    fn destruction_fact(position: Vec2, radius: f32) -> CombatWorldEffectFact {
        CombatWorldEffectFact {
            tick: 1,
            source: AttackSource {
                kind: CombatSourceKind::PrimaryWeapon,
                attack_id: AttackId(1),
                player_id: crate::protocol::PlayerId(1),
                owner_network_entity_id: crate::protocol::NetworkEntityId(1),
                team_id: crate::combat::TeamId(0),
                recipe_fingerprint: crate::combat::WeaponRecipeFingerprint::default(),
                presentation_profile_id: crate::combat::WeaponPresentationProfileId(3),
                legacy_compatibility: false,
                source_preset_id: None,
                origin: WorldPoint { x: 0.0, y: 0.0 },
                facing: 0.0,
            },
            delivery_index: 0,
            effect_index: 0,
            position: WorldPoint {
                x: position.x,
                y: position.y,
            },
            effect: WorldEffectDefinition::DestroyMap { radius },
        }
    }

    #[test]
    fn recovery_admission_rejects_inactive_stale_and_rate_exhausted_requests() {
        let current = MapDynamicGeneration {
            map_instance_id: MapInstanceId(2),
            generation: 3,
        };
        assert!(recovery_request_is_admitted(true, current, current, 0));
        assert!(!recovery_request_is_admitted(false, current, current, 0));
        assert!(!recovery_request_is_admitted(
            true,
            MapDynamicGeneration {
                generation: 2,
                ..current
            },
            current,
            0,
        ));
        assert!(!recovery_request_is_admitted(
            true,
            current,
            current,
            MAX_RECOVERY_RESPONSES_PER_GENERATION,
        ));
    }

    #[test]
    fn radius_brush_removes_whole_grid_cells_and_restart_restores_them() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<CombatWorldEffectFacts>()
            .init_resource::<MapDynamicOutbox>()
            .init_resource::<MapDynamicTelemetry>()
            .init_resource::<MapCatalogResource>();
        let resolved = app
            .world()
            .resource::<MapCatalogResource>()
            .0
            .resolve_preset(super::super::FEATURE_YARD_WIPEOUT_PRESET, MapInstanceId(1))
            .unwrap();
        // Install directly without Lightyear replication in this focused rule test.
        let snapshot = resolved.snapshot.clone();
        let root = app
            .world_mut()
            .spawn((
                MapRoot,
                snapshot.clone(),
                MapDynamicState {
                    map_instance_id: MapInstanceId(1),
                    generation: 1,
                    revision: 0,
                    terminal_states: Vec::new(),
                },
            ))
            .id();
        let catalog = app.world().resource::<MapCatalogResource>().0.clone();
        for placement in &resolved.dynamic_placements {
            let asset = catalog.asset(placement.asset_id).unwrap();
            spawn_dynamic_collider(
                app.world_mut(),
                MapInstanceId(1),
                1,
                &snapshot,
                asset,
                placement,
            );
        }
        let target = resolved
            .dynamic_placements
            .iter()
            .find(|placement| placement.placement_id == MapPlacementId(220))
            .unwrap();
        let target_asset = catalog.asset(target.asset_id).unwrap();
        let target_center = placement_world_center(snapshot.dimensions, target_asset, target);
        app.world_mut()
            .resource_mut::<CombatWorldEffectFacts>()
            .0
            .push(destruction_fact(target_center, 1.0));
        apply_map_destruction(app.world_mut());
        let state = app.world().get::<MapDynamicState>(root).unwrap();
        assert!(state.revision > 0);
        assert!(!state.terminal_states.is_empty());
        assert_eq!(state.terminal_states.len(), 1);
        assert!(
            state
                .terminal_states
                .iter()
                .all(|transition| transition.outcome == MapPlacementOutcome::Removed)
        );
        let removed_count = state.terminal_states.len();

        restore_map(app.world_mut());
        let state = app.world().get::<MapDynamicState>(root).unwrap();
        assert_eq!(state.generation, 2);
        assert_eq!(state.revision, 0);
        assert!(state.terminal_states.is_empty());
        let collider_count = {
            let world = app.world_mut();
            let mut query = world.query::<&DestructibleMapCollider>();
            query.iter(world).count()
        };
        assert_eq!(collider_count, 8);
        assert!(removed_count > 0);
        let telemetry = app.world().resource::<MapDynamicTelemetry>();
        assert_eq!(telemetry.destruction_requests, 1);
        assert_eq!(telemetry.destruction_applied, 1);
        assert_eq!(telemetry.placements_changed, removed_count as u64);
    }

    #[test]
    fn one_hit_replaces_an_entire_rotated_barrier_and_restart_restores_it() {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .init_resource::<CombatWorldEffectFacts>()
            .init_resource::<MapDynamicOutbox>()
            .init_resource::<MapDynamicTelemetry>()
            .init_resource::<MapCatalogResource>();
        let catalog = app.world().resource::<MapCatalogResource>().0.clone();
        let resolved = catalog
            .resolve_preset(super::super::FEATURE_YARD_WIPEOUT_PRESET, MapInstanceId(2))
            .unwrap();
        let snapshot = resolved.snapshot.clone();
        let target = resolved
            .dynamic_placements
            .iter()
            .find(|placement| placement.placement_id == MapPlacementId(200))
            .unwrap()
            .clone();
        let target_asset = catalog.asset(target.asset_id).unwrap();
        let target_center = placement_world_center(snapshot.dimensions, target_asset, &target);
        let root = app
            .world_mut()
            .spawn((
                MapRoot,
                snapshot.clone(),
                MapDynamicState {
                    map_instance_id: MapInstanceId(2),
                    generation: 1,
                    revision: 0,
                    terminal_states: Vec::new(),
                },
            ))
            .id();
        for placement in &resolved.dynamic_placements {
            spawn_dynamic_collider(
                app.world_mut(),
                MapInstanceId(2),
                1,
                &snapshot,
                catalog.asset(placement.asset_id).unwrap(),
                placement,
            );
        }
        app.world_mut()
            .resource_mut::<CombatWorldEffectFacts>()
            .0
            .push(destruction_fact(target_center, 1.0));
        apply_map_destruction(app.world_mut());

        let state = app.world().get::<MapDynamicState>(root).unwrap();
        assert_eq!(
            state.terminal_states,
            vec![MapPlacementTransition {
                placement_id: MapPlacementId(200),
                outcome: MapPlacementOutcome::ReplacedWith(super::super::RUBBLE_ASSET),
            }]
        );
        let collider_count = {
            let world = app.world_mut();
            let mut query = world.query::<&DestructibleMapCollider>();
            query.iter(world).count()
        };
        assert_eq!(collider_count, 7);

        restore_map(app.world_mut());
        assert!(
            app.world()
                .get::<MapDynamicState>(root)
                .unwrap()
                .terminal_states
                .is_empty()
        );
        let restored_count = {
            let world = app.world_mut();
            let mut query = world.query::<&DestructibleMapCollider>();
            query.iter(world).count()
        };
        assert_eq!(restored_count, 8);
    }

    #[test]
    fn barrel_damage_explodes_once_chains_and_restart_restores_a_new_generation() {
        let (mut app, root) = barrel_test_app();
        let target = barrel_identity(&mut app, 240);
        let source = test_attack_source();
        app.world_mut()
            .resource_mut::<super::super::PendingWorldTargetDamages>()
            .0
            .push(super::super::PendingWorldTargetDamage {
                target,
                source,
                attack_id: source.attack_id,
                requested_damage: 60,
                delivery_index: 0,
                bundle_index: 0,
                effect_index: 0,
            });

        process_world_target_damage(app.world_mut());

        assert_eq!(barrel_health(&mut app, 240), None);
        assert_eq!(barrel_health(&mut app, 241), Some(25));
        let state = app.world().get::<MapDynamicState>(root).unwrap();
        assert_eq!(state.revision, 1);
        assert_eq!(
            state.terminal_states,
            vec![MapPlacementTransition {
                placement_id: MapPlacementId(240),
                outcome: MapPlacementOutcome::ReplacedWith(super::super::BARREL_WOOD_DEBRIS_ASSET,),
            }]
        );
        assert_eq!(
            app.world()
                .resource::<super::super::WorldObjectExplosionFacts>()
                .0
                .len(),
            1
        );
        assert_eq!(
            app.world()
                .resource::<super::super::WorldTargetDamageFacts>()
                .0
                .len(),
            2
        );
        let destroyed_collider_exists = {
            let world = app.world_mut();
            let mut colliders = world.query::<&DestructibleMapCollider>();
            colliders
                .iter(world)
                .any(|collider| collider.placement_id == MapPlacementId(101))
        };
        assert!(
            !destroyed_collider_exists,
            "the debris replacement is visual-only and nonblocking"
        );

        app.world_mut()
            .resource_mut::<super::super::PendingWorldTargetDamages>()
            .0
            .push(super::super::PendingWorldTargetDamage {
                target,
                source,
                attack_id: source.attack_id,
                requested_damage: 60,
                delivery_index: 0,
                bundle_index: 0,
                effect_index: 0,
            });
        process_world_target_damage(app.world_mut());
        assert_eq!(
            app.world()
                .resource::<super::super::WorldObjectExplosionFacts>()
                .0
                .len(),
            1,
            "the stale terminal identity cannot explode twice"
        );

        restore_map(app.world_mut());
        let state = app.world().get::<MapDynamicState>(root).unwrap();
        assert_eq!(state.generation, 2);
        assert_eq!(state.revision, 0);
        assert!(state.terminal_states.is_empty());
        let world = app.world_mut();
        let mut query = world.query::<(
            &super::super::DamageableTargetIdentity,
            &crate::combat::CurrentHealth,
        )>();
        let restored: Vec<_> = query
            .iter(world)
            .map(|(identity, health)| (identity.generation().generation, health.0))
            .collect();
        assert_eq!(restored.len(), 4);
        assert!(restored.iter().all(|entry| *entry == (2, 60)));
    }

    #[test]
    fn barrel_explosion_respects_authoritative_map_occlusion() {
        let (mut app, _) = barrel_test_app();
        let target = barrel_identity(&mut app, 240);
        let source_position = {
            let world = app.world_mut();
            let mut query = world.query::<(&super::super::DamageableTargetIdentity, &Position)>();
            query
                .iter(world)
                .find(|(identity, _)| **identity == target)
                .unwrap()
                .1
                .0
        };
        let chained_position = {
            let chained = barrel_identity(&mut app, 241);
            let world = app.world_mut();
            let mut query = world.query::<(&super::super::DamageableTargetIdentity, &Position)>();
            query
                .iter(world)
                .find(|(identity, _)| **identity == chained)
                .unwrap()
                .1
                .0
        };
        let midpoint = (source_position + chained_position) * 0.5;
        app.world_mut().spawn((
            ArenaWall,
            RigidBody::Static,
            Collider::rectangle(16.0, 64.0),
            destructible_map_collision_layers(),
            Position::from_xy(midpoint.x, midpoint.y),
            Rotation::default(),
        ));
        app.update();
        let source = test_attack_source();
        app.world_mut()
            .resource_mut::<super::super::PendingWorldTargetDamages>()
            .0
            .push(super::super::PendingWorldTargetDamage {
                target,
                source,
                attack_id: source.attack_id,
                requested_damage: 60,
                delivery_index: 0,
                bundle_index: 0,
                effect_index: 0,
            });

        process_world_target_damage(app.world_mut());

        assert_eq!(barrel_health(&mut app, 241), Some(60));
    }

    #[test]
    fn barrel_explosion_damages_combatants_as_environment_without_object_outcome_leakage() {
        let (mut app, _) = barrel_test_app();
        let target = barrel_identity(&mut app, 240);
        let position = {
            let world = app.world_mut();
            let mut query = world.query::<(&super::super::DamageableTargetIdentity, &Position)>();
            query
                .iter(world)
                .find(|(identity, _)| **identity == target)
                .unwrap()
                .1
                .0
        };
        app.world_mut().spawn((
            crate::protocol::Fighter,
            crate::protocol::PlayerId(7),
            crate::protocol::NetworkEntityId(70),
            crate::combat::TeamId(0),
            crate::combat::CurrentHealth(100),
            crate::matchplay::MatchMember(crate::matchplay::MatchId(1)),
            crate::matchplay::ActiveCombatant,
            Position::from_xy(position.x - 256.0, position.y),
        ));
        let fighter = app
            .world_mut()
            .spawn((
                crate::protocol::Fighter,
                crate::protocol::NetworkEntityId(88),
                crate::combat::TeamId(1),
                crate::combat::CurrentHealth(100),
                Position::from_xy(position.x, position.y + 48.0),
            ))
            .id();
        app.update();
        let source = test_attack_source();
        app.world_mut()
            .resource_mut::<super::super::PendingWorldTargetDamages>()
            .0
            .push(super::super::PendingWorldTargetDamage {
                target,
                source,
                attack_id: source.attack_id,
                requested_damage: 60,
                delivery_index: 0,
                bundle_index: 0,
                effect_index: 0,
            });

        process_world_target_damage(app.world_mut());

        assert_eq!(
            app.world()
                .get::<crate::combat::CurrentHealth>(fighter)
                .unwrap()
                .0,
            65
        );
        let outcomes = &app
            .world()
            .resource::<crate::combat::CombatOutcomeFacts>()
            .0;
        assert_eq!(outcomes.len(), 1);
        assert_eq!(outcomes[0].source_kind, CombatSourceKind::Environment);
        assert_eq!(outcomes[0].source_team, Some(crate::combat::TeamId(0)));
        assert_eq!(
            app.world()
                .resource::<super::super::WorldTargetDamageFacts>()
                .0
                .len(),
            2,
            "only the primary barrel and chained barrel use the object-fact channel"
        );
    }
}
