//! Whole-cell destruction, restoration, mutation publication, and recovery admission.

use bevy::prelude::*;
use lightyear::prelude::{Disconnected, LinkOf, MessageReceiver, MessageSender};

use crate::{
    combat::{CombatWorldEffectFact, CombatWorldEffectFacts, WorldEffectDefinition},
    protocol::MapDynamicChannel,
    server::{ServerSession, ServerSessionPhase},
};

use super::installation::spawn_dynamic_collider;
use super::{
    DestructibleMapCollider, MAX_MAP_DYNAMIC_OUTBOX_EVENTS, MAX_RECOVERY_RESPONSES_PER_GENERATION,
    MapDynamicOutbox, MapDynamicTelemetry,
};
use crate::map::{
    MapCatalogResource, MapDestructionBehavior, MapDynamicGeneration, MapDynamicRecoveryRequest,
    MapDynamicRecoverySnapshot, MapDynamicResetEvent, MapDynamicState, MapMutationEvent,
    MapPlacementOutcome, MapPlacementTransition, ResolvedMapSnapshot, placement_cells,
};

fn fact_key(fact: &CombatWorldEffectFact) -> (u64, u64, u8, u8) {
    (
        fact.tick,
        fact.source.attack_id.0,
        fact.delivery_index,
        fact.effect_index,
    )
}

fn circle_overlaps_cell(center: Vec2, radius: f32, min: Vec2) -> bool {
    let max = min + Vec2::splat(crate::map::MAP_CELL_SIZE_WORLD);
    let closest = center.clamp(min, max);
    center.distance_squared(closest) <= radius * radius
}

fn destruction_outcome_at(
    catalog: &crate::map::MapContentCatalog,
    snapshot: &ResolvedMapSnapshot,
    placement: &crate::map::MapAssetPlacement,
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

pub(super) fn apply_map_destruction(world: &mut World) {
    let Some((root_entity, snapshot, mut state)) = world
        .query_filtered::<(Entity, &ResolvedMapSnapshot, &MapDynamicState), With<crate::map::MapRoot>>()
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

pub(super) fn reset_map_on_match_restart(world: &mut World) {
    if world
        .get_resource::<crate::matchplay::PendingMatchRestart>()
        .and_then(crate::matchplay::PendingMatchRestart::slot)
        .is_none()
    {
        return;
    }
    restore_map(world);
}

pub(super) fn restore_map(world: &mut World) {
    let Some((root_entity, snapshot, mut state)) = world
        .query_filtered::<(Entity, &ResolvedMapSnapshot, &MapDynamicState), With<crate::map::MapRoot>>()
        .iter(world)
        .next()
        .map(|(entity, snapshot, state)| (entity, snapshot.clone(), state.clone()))
    else {
        return;
    };
    world.resource_mut::<CombatWorldEffectFacts>().0.clear();
    if let Some(mut pending) = world.get_resource_mut::<crate::map::PendingWorldTargetDamages>() {
        pending.0.clear();
    }
    if let Some(mut facts) = world.get_resource_mut::<crate::map::WorldTargetDamageFacts>() {
        facts.0.clear();
    }
    if let Some(mut facts) = world.get_resource_mut::<crate::map::WorldObjectExplosionFacts>() {
        facts.0.clear();
    }
    if let Some(mut outbox) = world.get_resource_mut::<crate::map::WorldObjectOutbox>() {
        outbox.0.clear();
    }
    if let Some(mut facts) = world.get_resource_mut::<crate::map::PickupLifecycleFacts>() {
        facts.0.clear();
    }
    if let Some(mut outbox) = world.get_resource_mut::<crate::map::PickupOutbox>() {
        outbox.0.clear();
    }
    let pickups: Vec<_> = world
        .query_filtered::<Entity, With<crate::map::RestorationPickup>>()
        .iter(world)
        .collect();
    for entity in pickups {
        world.entity_mut(entity).despawn();
    }
    let damageable_entities: Vec<_> = world
        .query_filtered::<Entity, With<crate::map::DamageableWorldObject>>()
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
                    || profile.durability != crate::map::MapDurabilityBehavior::Indestructible
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
pub(super) fn receive_map_recovery_requests(
    roots: Query<&MapDynamicState, With<crate::map::MapRoot>>,
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

pub(super) fn recovery_request_is_admitted(
    active_session: bool,
    requested: MapDynamicGeneration,
    current: MapDynamicGeneration,
    responses: u8,
) -> bool {
    active_session && requested == current && responses < MAX_RECOVERY_RESPONSES_PER_GENERATION
}

pub(super) fn publish_map_dynamic_traffic(
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
