//! Authoritative restoration-pickup lifecycle owned by the active map generation.

#[cfg(feature = "server")]
use super::{MapCatalogResource, MapInstanceMember};
use super::{MapDynamicGeneration, MapPlacementId, RestorationPickupDefinitionId};
#[cfg(feature = "server")]
use crate::combat::CurrentHealth;
use crate::combat::{CombatEventId, TeamId, WorldPoint};
#[cfg(feature = "server")]
use avian2d::prelude::Position;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub const MAX_LIVE_RESTORATION_PICKUPS: usize = 16;
pub const MAX_PICKUP_FACTS: usize = 256;
pub const MAX_PICKUP_CUES: usize = 256;

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct RestorationPickupIdentity {
    pub generation: MapDynamicGeneration,
    pub source_placement_id: MapPlacementId,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct RestorationPickup;

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickupAvailableAtTick(pub u64);

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PickupExpiresAtTick(pub u64);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PickupLifecycleKind {
    Spawned,
    Collected,
    Expired,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PickupLifecycleFact {
    pub event_id: CombatEventId,
    pub tick: u64,
    pub identity: RestorationPickupIdentity,
    pub definition_id: RestorationPickupDefinitionId,
    pub position: Vec2,
    pub kind: PickupLifecycleKind,
    pub collector: Option<crate::protocol::NetworkEntityId>,
    pub collector_team: Option<TeamId>,
    pub requested_restoration: u16,
    pub applied_restoration: u16,
    pub health_after: Option<u16>,
}

#[derive(Resource, Default)]
pub struct PickupLifecycleFacts(pub Vec<PickupLifecycleFact>);

#[derive(Message, Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub enum PickupCue {
    Spawned {
        event_id: CombatEventId,
        tick: u64,
        identity: RestorationPickupIdentity,
        definition_id: RestorationPickupDefinitionId,
        position: WorldPoint,
    },
    Collected {
        event_id: CombatEventId,
        tick: u64,
        identity: RestorationPickupIdentity,
        position: WorldPoint,
        collector: Option<crate::protocol::NetworkEntityId>,
        applied_restoration: u16,
    },
    Expired {
        event_id: CombatEventId,
        tick: u64,
        identity: RestorationPickupIdentity,
        position: WorldPoint,
    },
}

impl PickupCue {
    #[must_use]
    pub const fn event_id(self) -> CombatEventId {
        match self {
            Self::Spawned { event_id, .. }
            | Self::Collected { event_id, .. }
            | Self::Expired { event_id, .. } => event_id,
        }
    }
}

#[cfg(feature = "server")]
#[derive(Resource, Default)]
pub struct PickupOutbox(pub Vec<PickupCue>);

#[cfg(feature = "client")]
#[derive(Resource, Default)]
pub struct ReceivedPickupCues(pub Vec<PickupCue>);

#[cfg(feature = "server")]
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PickupTelemetry {
    pub spawned: u64,
    pub collected: u64,
    pub expired: u64,
    pub requested_restoration: u64,
    pub applied_restoration: u64,
    pub wasted_restoration: u64,
    pub total_collection_lifetime_ticks: u64,
    pub collections_by_team: [u64; 2],
    pub capacity_rejections: u64,
}

#[cfg(feature = "server")]
pub(crate) fn register_pickup_runtime(app: &mut App) {
    app.init_resource::<PickupLifecycleFacts>()
        .init_resource::<PickupOutbox>()
        .init_resource::<PickupTelemetry>()
        .add_systems(
            FixedPostUpdate,
            apply_restoration_pickups
                .in_set(crate::combat::CombatDamageSet::EnvironmentReactions)
                .after(super::runtime::effect_tiles::apply_damage_tile_pulses),
        )
        .add_systems(
            FixedPostUpdate,
            send_pickup_cues
                .in_set(crate::combat::CombatSet::TelemetryAndCues)
                .after(crate::concealment::ConcealmentSet::DecideObservers),
        )
        .add_systems(
            FixedPostUpdate,
            clear_pickup_tick_facts
                .in_set(crate::combat::CombatSet::Finalize)
                .before(crate::gameplay::advance_simulation_tick),
        );
}

#[cfg(feature = "server")]
pub(crate) fn spawn_restoration_pickup(
    world: &mut World,
    identity: RestorationPickupIdentity,
    definition_id: RestorationPickupDefinitionId,
    position: Vec2,
    tick: u64,
    event_id: CombatEventId,
) -> Result<(), &'static str> {
    use lightyear::prelude::{NetworkTarget, Replicate};

    let live = world
        .query_filtered::<Entity, With<RestorationPickup>>()
        .iter(world)
        .count();
    if live >= MAX_LIVE_RESTORATION_PICKUPS
        || world.resource::<PickupLifecycleFacts>().0.len() >= MAX_PICKUP_FACTS
        || world.resource::<PickupOutbox>().0.len() >= MAX_PICKUP_CUES
    {
        let mut telemetry = world.resource_mut::<PickupTelemetry>();
        telemetry.capacity_rejections = telemetry.capacity_rejections.saturating_add(1);
        return Err("restoration pickup capacity exhausted");
    }
    let definition = *world
        .resource::<MapCatalogResource>()
        .0
        .restoration_pickup(definition_id)
        .ok_or("unknown restoration pickup definition")?;
    let available_at = tick.saturating_add(1);
    let expires_at = tick.saturating_add(u64::from(definition.lifetime_ticks));
    world.spawn((
        RestorationPickup,
        identity,
        definition_id,
        PickupAvailableAtTick(available_at),
        PickupExpiresAtTick(expires_at),
        MapInstanceMember {
            map_instance_id: identity.generation.map_instance_id,
            placement_id: identity.source_placement_id,
        },
        Position::from_xy(position.x, position.y),
        Transform::from_translation(position.extend(0.0)),
        Replicate::to_clients(NetworkTarget::All),
    ));
    world
        .resource_mut::<PickupLifecycleFacts>()
        .0
        .push(PickupLifecycleFact {
            event_id,
            tick,
            identity,
            definition_id,
            position,
            kind: PickupLifecycleKind::Spawned,
            collector: None,
            collector_team: None,
            requested_restoration: 0,
            applied_restoration: 0,
            health_after: None,
        });
    world
        .resource_mut::<PickupOutbox>()
        .0
        .push(PickupCue::Spawned {
            event_id,
            tick,
            identity,
            definition_id,
            position: WorldPoint::from(position),
        });
    let mut telemetry = world.resource_mut::<PickupTelemetry>();
    telemetry.spawned = telemetry.spawned.saturating_add(1);
    Ok(())
}

#[cfg(feature = "server")]
fn maximum_health(
    fighter_id: crate::combat::FighterDefinitionId,
    loadout: Option<&crate::builds::ResolvedMatchLoadout>,
    definitions: &crate::combat::FighterDefinitions,
) -> Option<u16> {
    loadout
        .map(|loadout| loadout.fighter_stats.maximum_health)
        .or_else(|| {
            definitions
                .get(fighter_id)
                .map(|fighter| fighter.maximum_health)
        })
}

#[cfg(feature = "server")]
#[allow(
    clippy::too_many_lines,
    reason = "one exclusive transaction deterministically orders pickup collection, healing, and expiry"
)]
pub(super) fn apply_restoration_pickups(world: &mut World) {
    let Some(match_id) = world
        .query::<&crate::matchplay::MatchState>()
        .iter(world)
        .find_map(|state| {
            matches!(state.phase, crate::matchplay::MatchPhase::Active { .. })
                .then_some(state.match_id)
        })
    else {
        return;
    };
    let tick = world.resource::<crate::timing::SimulationTick>().0;
    let Some(current_generation) = world
        .query_filtered::<&super::MapDynamicState, With<super::MapRoot>>()
        .iter(world)
        .next()
        .map(super::MapDynamicState::generation_id)
    else {
        return;
    };
    let definitions = world
        .resource::<crate::combat::FighterDefinitions>()
        .clone();
    let catalog = world.resource::<MapCatalogResource>().0.clone();
    let mut pickups: Vec<_> = world
        .query_filtered::<(
            Entity,
            &RestorationPickupIdentity,
            &RestorationPickupDefinitionId,
            &PickupAvailableAtTick,
            &PickupExpiresAtTick,
            &Position,
        ), With<RestorationPickup>>()
        .iter(world)
        .map(
            |(entity, identity, definition, available, expires, position)| {
                (
                    entity,
                    *identity,
                    *definition,
                    available.0,
                    expires.0,
                    position.0,
                )
            },
        )
        .collect();
    pickups.sort_by_key(|pickup| {
        (
            pickup.1.generation.map_instance_id.0,
            pickup.1.generation.generation,
            pickup.1.source_placement_id.0,
        )
    });
    for (pickup_entity, identity, definition_id, available_at, expires_at, pickup_position) in
        pickups
    {
        if identity.generation != current_generation {
            continue;
        }
        if tick < available_at {
            continue;
        }
        let definition = *catalog
            .restoration_pickup(definition_id)
            .expect("validated pickup definition exists");
        let mut candidates: Vec<_> = world
            .query_filtered::<(
                Entity,
                &crate::protocol::NetworkEntityId,
                &TeamId,
                &Position,
                &CurrentHealth,
                &crate::combat::FighterDefinitionId,
                Option<&crate::builds::ResolvedMatchLoadout>,
                &crate::matchplay::MatchMember,
                &super::SpawnAssignment,
                Option<&super::EffectTileOccupancy>,
            ), (
                With<crate::protocol::Fighter>,
                With<crate::matchplay::ActiveCombatant>,
                Without<crate::combat::Defeated>,
            )>()
            .iter(world)
            .filter_map(
                |(
                    entity,
                    network_id,
                    team,
                    position,
                    health,
                    fighter_id,
                    loadout,
                    member,
                    spawn,
                    effect_tile,
                )| {
                    let maximum = maximum_health(*fighter_id, loadout, &definitions)?;
                    (member.0 == match_id
                        && health.0 > 0
                        && health.0 < maximum
                        && !effect_tile.is_some_and(super::EffectTileOccupancy::blocks_healing)
                        && spawn.map_instance_id == identity.generation.map_instance_id
                        && position.0.distance_squared(pickup_position)
                            <= f32::from(definition.collection_radius_world_units).powi(2))
                    .then_some((network_id.0, entity, *network_id, *team, health.0, maximum))
                },
            )
            .collect();
        candidates.sort_by_key(|candidate| candidate.0);
        if let Some((_, fighter_entity, collector, collector_team, health, maximum)) =
            candidates.first().copied()
        {
            let applied = definition.restoration.min(maximum.saturating_sub(health));
            let health_after = health.saturating_add(applied);
            let Some(event_id) = crate::combat::server::reserve_event_ids(
                &mut world.resource_mut::<crate::combat::NextCombatIds>(),
                1,
            )
            .map(|ids| ids[0]) else {
                continue;
            };
            world
                .entity_mut(fighter_entity)
                .insert(CurrentHealth(health_after));
            world.entity_mut(pickup_entity).despawn();
            world
                .resource_mut::<PickupLifecycleFacts>()
                .0
                .push(PickupLifecycleFact {
                    event_id,
                    tick,
                    identity,
                    definition_id,
                    position: pickup_position,
                    kind: PickupLifecycleKind::Collected,
                    collector: Some(collector),
                    collector_team: Some(collector_team),
                    requested_restoration: definition.restoration,
                    applied_restoration: applied,
                    health_after: Some(health_after),
                });
            world
                .resource_mut::<PickupOutbox>()
                .0
                .push(PickupCue::Collected {
                    event_id,
                    tick,
                    identity,
                    position: WorldPoint::from(pickup_position),
                    collector: Some(collector),
                    applied_restoration: applied,
                });
            let mut telemetry = world.resource_mut::<PickupTelemetry>();
            telemetry.collected = telemetry.collected.saturating_add(1);
            telemetry.requested_restoration = telemetry
                .requested_restoration
                .saturating_add(u64::from(definition.restoration));
            telemetry.applied_restoration = telemetry
                .applied_restoration
                .saturating_add(u64::from(applied));
            telemetry.wasted_restoration = telemetry
                .wasted_restoration
                .saturating_add(u64::from(definition.restoration - applied));
            telemetry.total_collection_lifetime_ticks = telemetry
                .total_collection_lifetime_ticks
                .saturating_add(tick.saturating_sub(available_at.saturating_sub(1)));
            if let Some(team_collections) = telemetry
                .collections_by_team
                .get_mut(usize::from(collector_team.0))
            {
                *team_collections = team_collections.saturating_add(1);
            }
        } else if tick >= expires_at {
            let Some(event_id) = crate::combat::server::reserve_event_ids(
                &mut world.resource_mut::<crate::combat::NextCombatIds>(),
                1,
            )
            .map(|ids| ids[0]) else {
                continue;
            };
            world.entity_mut(pickup_entity).despawn();
            world
                .resource_mut::<PickupLifecycleFacts>()
                .0
                .push(PickupLifecycleFact {
                    event_id,
                    tick,
                    identity,
                    definition_id,
                    position: pickup_position,
                    kind: PickupLifecycleKind::Expired,
                    collector: None,
                    collector_team: None,
                    requested_restoration: 0,
                    applied_restoration: 0,
                    health_after: None,
                });
            world
                .resource_mut::<PickupOutbox>()
                .0
                .push(PickupCue::Expired {
                    event_id,
                    tick,
                    identity,
                    position: WorldPoint::from(pickup_position),
                });
            let mut telemetry = world.resource_mut::<PickupTelemetry>();
            telemetry.expired = telemetry.expired.saturating_add(1);
        }
    }
}

#[cfg(feature = "server")]
fn clear_pickup_tick_facts(mut facts: ResMut<PickupLifecycleFacts>) {
    facts.0.clear();
}

#[cfg(feature = "server")]
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy systems receive Res<T> by value"
)]
fn send_pickup_cues(
    mut outbox: ResMut<PickupOutbox>,
    links: Query<
        (
            Entity,
            &crate::server::ServerSession,
            Has<lightyear::prelude::Disconnected>,
        ),
        With<lightyear::prelude::LinkOf>,
    >,
    mut senders: Query<
        &mut lightyear::prelude::MessageSender<PickupCue>,
        With<lightyear::prelude::LinkOf>,
    >,
    visibility: Res<crate::concealment::ObserverVisibilityCache>,
    fighters: Query<(Entity, &crate::protocol::NetworkEntityId), With<crate::protocol::Fighter>>,
) {
    use crate::server::ServerSessionPhase;
    if outbox.0.is_empty() {
        return;
    }
    outbox.0.sort_by_key(|cue| cue.event_id().0);
    let fighter_entities: std::collections::BTreeMap<_, _> =
        fighters.iter().map(|(entity, id)| (id.0, entity)).collect();
    for (connection, session, disconnected) in &links {
        if disconnected || !matches!(session.phase, ServerSessionPhase::Active { .. }) {
            continue;
        }
        let Ok(mut sender) = senders.get_mut(connection) else {
            continue;
        };
        for cue in &outbox.0 {
            let filtered = match *cue {
                PickupCue::Collected {
                    event_id,
                    tick,
                    identity,
                    position,
                    collector: Some(collector),
                    applied_restoration,
                } if !fighter_entities
                    .get(&collector.0)
                    .is_some_and(|entity| visibility.permits(connection, *entity)) =>
                {
                    PickupCue::Collected {
                        event_id,
                        tick,
                        identity,
                        position,
                        collector: None,
                        applied_restoration,
                    }
                }
                cue => cue,
            };
            sender.send::<crate::protocol::CombatChannel>(filtered);
        }
    }
    outbox.0.clear();
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    fn fixture(tick: u64) -> World {
        let mut world = World::new();
        world.insert_resource(crate::timing::SimulationTick(tick));
        world.insert_resource(crate::combat::FighterDefinitions::default());
        world.insert_resource(crate::combat::NextCombatIds::default());
        world.insert_resource(MapCatalogResource(
            crate::map::MapContentCatalog::embedded().unwrap(),
        ));
        world.insert_resource(PickupLifecycleFacts::default());
        world.insert_resource(PickupOutbox::default());
        world.insert_resource(PickupTelemetry::default());
        world.spawn((
            crate::map::MapRoot,
            crate::map::MapDynamicState {
                map_instance_id: crate::map::MapInstanceId(1),
                generation: 1,
                revision: 0,
                terminal_states: Vec::new(),
            },
        ));
        world.spawn(crate::matchplay::MatchState {
            match_id: crate::matchplay::MatchId(7),
            mode_definition_id: crate::map::WIPEOUT_MODE_DEFINITION,
            phase: crate::matchplay::MatchPhase::Active { ends_at_tick: 999 },
            rules_revision: 1,
        });
        world
    }

    fn spawn_fighter(world: &mut World, network_id: u64, health: u16, position: Vec2) -> Entity {
        world
            .spawn((
                crate::protocol::Fighter,
                crate::protocol::NetworkEntityId(network_id),
                TeamId(u8::try_from(network_id % 2).unwrap()),
                Position::from_xy(position.x, position.y),
                CurrentHealth(health),
                crate::combat::STANDARD_FIGHTER_DEFINITION,
                crate::matchplay::MatchMember(crate::matchplay::MatchId(7)),
                crate::matchplay::ActiveCombatant,
                crate::map::SpawnAssignment {
                    map_instance_id: crate::map::MapInstanceId(1),
                    spawn_point_id: crate::map::SpawnPointId(u16::try_from(network_id).unwrap()),
                },
            ))
            .id()
    }

    fn spawn_pickup_entity(world: &mut World, expires_at: u64) -> Entity {
        world
            .spawn((
                RestorationPickup,
                RestorationPickupIdentity {
                    generation: MapDynamicGeneration {
                        map_instance_id: crate::map::MapInstanceId(1),
                        generation: 1,
                    },
                    source_placement_id: MapPlacementId(260),
                },
                RestorationPickupDefinitionId(1),
                PickupAvailableAtTick(1),
                PickupExpiresAtTick(expires_at),
                Position::from_xy(0.0, 0.0),
            ))
            .id()
    }

    #[test]
    fn lowest_network_identity_collects_and_healing_caps_at_maximum() {
        let mut world = fixture(10);
        let higher = spawn_fighter(&mut world, 9, 25, Vec2::ZERO);
        let lower = spawn_fighter(&mut world, 2, 75, Vec2::ZERO);
        let pickup = spawn_pickup_entity(&mut world, 20);

        apply_restoration_pickups(&mut world);

        assert!(world.get_entity(pickup).is_err());
        assert_eq!(world.get::<CurrentHealth>(lower), Some(&CurrentHealth(100)));
        assert_eq!(world.get::<CurrentHealth>(higher), Some(&CurrentHealth(25)));
        let facts = &world.resource::<PickupLifecycleFacts>().0;
        assert_eq!(facts.len(), 1);
        assert_eq!(
            facts[0].collector,
            Some(crate::protocol::NetworkEntityId(2))
        );
        assert_eq!(facts[0].requested_restoration, 40);
        assert_eq!(facts[0].applied_restoration, 25);
    }

    #[test]
    fn full_health_does_not_consume_and_expiry_runs_after_collection_check() {
        let mut world = fixture(10);
        let fighter = spawn_fighter(&mut world, 3, 100, Vec2::ZERO);
        let pickup = spawn_pickup_entity(&mut world, 10);

        apply_restoration_pickups(&mut world);

        assert!(world.get_entity(pickup).is_err());
        assert_eq!(
            world.get::<CurrentHealth>(fighter),
            Some(&CurrentHealth(100))
        );
        assert_eq!(
            world.resource::<PickupLifecycleFacts>().0[0].kind,
            PickupLifecycleKind::Expired
        );
    }

    #[test]
    fn exact_expiry_tick_still_allows_an_eligible_collection() {
        let mut world = fixture(10);
        let fighter = spawn_fighter(&mut world, 4, 50, Vec2::ZERO);
        spawn_pickup_entity(&mut world, 10);

        apply_restoration_pickups(&mut world);

        assert_eq!(
            world.get::<CurrentHealth>(fighter),
            Some(&CurrentHealth(90))
        );
        assert_eq!(
            world.resource::<PickupLifecycleFacts>().0[0].kind,
            PickupLifecycleKind::Collected
        );
    }

    #[test]
    fn damage_tile_blocks_collection_without_consuming_the_pickup() {
        let mut world = fixture(10);
        let fighter = spawn_fighter(&mut world, 4, 50, Vec2::ZERO);
        world
            .entity_mut(fighter)
            .insert(crate::map::EffectTileOccupancy {
                generation: MapDynamicGeneration {
                    map_instance_id: crate::map::MapInstanceId(1),
                    generation: 1,
                },
                placement_id: MapPlacementId(1),
                kind: crate::map::EffectTileKind::Damage,
                entered_at_tick: 1,
                next_pulse_at_tick: Some(30),
            });
        let pickup = spawn_pickup_entity(&mut world, 20);

        apply_restoration_pickups(&mut world);

        assert!(world.get_entity(pickup).is_ok());
        assert_eq!(
            world.get::<CurrentHealth>(fighter),
            Some(&CurrentHealth(50))
        );
        assert!(world.resource::<PickupLifecycleFacts>().0.is_empty());
        assert!(world.resource::<PickupOutbox>().0.is_empty());
        assert_eq!(world.resource::<PickupTelemetry>().collected, 0);
    }
}
