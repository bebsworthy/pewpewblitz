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
            publish_map_dynamic_traffic.in_set(MapRuntimeSet::Publish),
        )
        .add_systems(Update, receive_map_recovery_requests);
    crate::matchplay::register_environment_reset_system(app, reset_map_on_match_restart);
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
        spawn_dynamic_collider(world, instance_id, &snapshot, asset, &placement);
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
    snapshot: &ResolvedMapSnapshot,
    asset: &super::MapAssetDefinition,
    placement: &super::MapAssetPlacement,
) {
    let footprint = asset.footprint_cells.rotated(placement.quarter_turns);
    let center = placement_world_center(snapshot.dimensions, asset, placement);
    world.spawn((
        ArenaWall,
        DestructibleMapCollider {
            placement_id: placement.placement_id,
        },
        MapInstanceMember {
            map_instance_id,
            placement_id: placement.placement_id,
        },
        RigidBody::Static,
        Collider::rectangle(
            f32::from(footprint.width) * super::MAP_CELL_SIZE_WORLD,
            f32::from(footprint.height) * super::MAP_CELL_SIZE_WORLD,
        ),
        destructible_map_collision_layers(),
        Position::from_xy(center.x, center.y),
        Rotation::default(),
        Transform::from_translation(center.extend(0.0)),
    ));
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
            .is_some_and(|profile| profile.destruction != MapDestructionBehavior::Indestructible);
        if dynamic && !existing.contains(&placement.placement_id) {
            spawn_dynamic_collider(world, state.map_instance_id, &snapshot, asset, placement);
        }
    }
    let previous_generation = state.generation_id();
    state.generation = state.generation.saturating_add(1);
    state.revision = 0;
    state.terminal_states.clear();
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
            .resolve_preset(super::super::CROSSROADS_PRESET, MapInstanceId(1))
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
        for placement in resolved.dynamic_placements {
            let asset = catalog.asset(placement.asset_id).unwrap();
            spawn_dynamic_collider(
                app.world_mut(),
                MapInstanceId(1),
                &snapshot,
                asset,
                &placement,
            );
        }
        app.world_mut()
            .resource_mut::<CombatWorldEffectFacts>()
            .0
            .push(destruction_fact(Vec2::ZERO, 48.0));
        apply_map_destruction(app.world_mut());
        let state = app.world().get::<MapDynamicState>(root).unwrap();
        assert!(state.revision > 0);
        assert!(!state.terminal_states.is_empty());
        assert!(state.terminal_states.len() < 36);
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
        assert_eq!(collider_count, 36);
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
            .resolve_preset(super::super::TIDAL_GARDEN_PRESET, MapInstanceId(2))
            .unwrap();
        let snapshot = resolved.snapshot.clone();
        let target = resolved
            .dynamic_placements
            .iter()
            .find(|placement| placement.placement_id == MapPlacementId(302))
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
                placement_id: MapPlacementId(302),
                outcome: MapPlacementOutcome::ReplacedWith(super::super::RUBBLE_ASSET),
            }]
        );
        let collider_count = {
            let world = app.world_mut();
            let mut query = world.query::<&DestructibleMapCollider>();
            query.iter(world).count()
        };
        assert_eq!(collider_count, 3);

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
        assert_eq!(restored_count, 4);
    }
}
