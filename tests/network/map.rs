//! Authoritative map replication, recovery, and no-client-authority scenarios.

use super::*;

fn client_snapshot(harness: &mut Harness, index: usize) -> Option<ResolvedMapSnapshot> {
    let world = harness.clients[index].world_mut();
    let mut query = world.query_filtered::<&ResolvedMapSnapshot, With<MapRoot>>();
    query.iter(world).next().cloned()
}

fn client_grid_state(
    harness: &mut Harness,
    index: usize,
) -> Option<(ResolvedMapSnapshot, MapDynamicState)> {
    let world = harness.clients[index].world_mut();
    let mut query =
        world.query_filtered::<(&ResolvedMapSnapshot, &MapDynamicState), With<MapRoot>>();
    query
        .iter(world)
        .next()
        .map(|(snapshot, state)| (snapshot.clone(), state.clone()))
}

fn server_match_state(harness: &mut Harness) -> MatchState {
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
    *query.single(world).unwrap()
}

fn client_barrel_health(harness: &mut Harness, index: usize, placement_id: u32) -> Option<u16> {
    let world = harness.clients[index].world_mut();
    let mut query = world
        .query_filtered::<(&DamageableTargetIdentity, &CurrentHealth), With<DamageableWorldObject>>(
        );
    query
        .iter(world)
        .find(|(identity, _)| identity.placement_id() == MapPlacementId(placement_id))
        .map(|(_, health)| health.0)
}

fn client_fighter_health(
    harness: &mut Harness,
    index: usize,
    network_id: NetworkEntityId,
) -> Option<(u16, bool)> {
    let world = harness.clients[index].world_mut();
    let mut query = world
        .query_filtered::<(&NetworkEntityId, &CurrentHealth, Option<&Defeated>), With<Fighter>>();
    query
        .iter(world)
        .find(|(candidate, _, _)| **candidate == network_id)
        .map(|(_, health, defeated)| (health.0, defeated.is_some()))
}

fn client_pickup_count(harness: &mut Harness, index: usize) -> usize {
    let world = harness.clients[index].world_mut();
    world
        .query_filtered::<Entity, With<RestorationPickup>>()
        .iter(world)
        .count()
}

#[test]
fn two_clients_receive_identical_map_snapshot_without_authoritative_colliders() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && client_snapshot(harness, 0).is_some()
            && client_snapshot(harness, 1).is_some()
            && client_grid_state(harness, 0).is_some()
            && client_grid_state(harness, 1).is_some()
    });
    let first = client_snapshot(&mut harness, 0).unwrap();
    let second = client_snapshot(&mut harness, 1).unwrap();
    let first_grid = client_grid_state(&mut harness, 0).unwrap();
    let second_grid = client_grid_state(&mut harness, 1).unwrap();
    assert_eq!(first, second);
    assert_eq!(first_grid, second_grid);
    assert_eq!(first.identity.instance_id, MapInstanceId(1));
    assert!(!first.placements.is_empty());
    for client in &mut harness.clients {
        let world = client.world_mut();
        let mut walls = world.query_filtered::<Entity, With<ArenaWall>>();
        assert_eq!(walls.iter(world).count(), 0);
    }
    let world = harness.server.world_mut();
    assert_eq!(world.resource::<ResolvedMap>().snapshot, first);
    assert!(world.query::<&MapInstanceMember>().iter(world).count() >= 4);
}

#[test]
fn late_join_and_map_root_replacement_converge_from_durable_state() {
    let mut harness = Harness::new(1);
    harness
        .step_until(|harness| harness.client_is_active(0) && client_snapshot(harness, 0).is_some());
    harness.add_client(2);
    harness
        .step_until(|harness| harness.client_is_active(1) && client_snapshot(harness, 1).is_some());
    assert_eq!(
        client_snapshot(&mut harness, 0),
        client_snapshot(&mut harness, 1)
    );

    let catalog = harness
        .server
        .world()
        .resource::<MapCatalogResource>()
        .0
        .clone();
    let replacement = catalog
        .resolve_preset(ArenaPresetId(1), MapInstanceId(2))
        .unwrap();
    install_resolved_map(harness.server.world_mut(), replacement).unwrap();
    harness.step_until(|harness| {
        (0..2).all(|index| {
            client_snapshot(harness, index)
                .is_some_and(|snapshot| snapshot.identity.instance_id == MapInstanceId(2))
        })
    });
    for index in 0..2 {
        let world = harness.clients[index].world_mut();
        let mut roots = world.query_filtered::<&MapInstanceId, With<MapRoot>>();
        let instances: Vec<_> = roots.iter(world).copied().collect();
        assert_eq!(instances, vec![MapInstanceId(2)]);
    }
}

#[test]
fn map_cover_destruction_converges_for_connected_and_late_joining_clients() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0) && client_grid_state(harness, 0).is_some()
    });

    harness.inject_map_destruction(900, (0.0, 0.0), 40.0);
    harness.step_until(|harness| {
        client_grid_state(harness, 0)
            .is_some_and(|(_, state)| state.revision == 1 && !state.terminal_states.is_empty())
    });
    let first = client_grid_state(&mut harness, 0).unwrap();
    assert_eq!(first.1.generation, 1);
    assert!(
        first
            .1
            .terminal_states
            .windows(2)
            .all(|pair| pair[0].placement_id < pair[1].placement_id)
    );

    harness.add_client(2);
    harness.step_until(|harness| {
        harness.client_is_active(1)
            && client_grid_state(harness, 1).is_some_and(|state| state == first)
    });
    assert_eq!(client_grid_state(&mut harness, 1), Some(first));
}

#[test]
fn feature_yard_barrier_replacement_converges_for_connected_and_late_joining_clients() {
    let catalog = brawler::map::MapContentCatalog::embedded().unwrap();
    let resolved = catalog
        .resolve_preset(brawler::map::FEATURE_YARD_WIPEOUT_PRESET, MapInstanceId(1))
        .unwrap();
    let placement = resolved
        .dynamic_placements
        .iter()
        .find(|placement| placement.placement_id == MapPlacementId(200))
        .unwrap();
    let center = brawler::map::placement_world_center(
        resolved.snapshot.dimensions,
        catalog.asset(placement.asset_id).unwrap(),
        placement,
    );
    let mut harness = Harness::new_feature_yard(1);
    harness.step_until(|harness| {
        harness.client_is_active(0) && client_grid_state(harness, 0).is_some()
    });

    harness.inject_map_destruction(901, (center.x, center.y), 1.0);
    harness.step_until(|harness| {
        client_grid_state(harness, 0).is_some_and(|(_, state)| {
            state.terminal_states
                == vec![brawler::map::MapPlacementTransition {
                    placement_id: MapPlacementId(200),
                    outcome: brawler::map::MapPlacementOutcome::ReplacedWith(
                        brawler::map::RUBBLE_ASSET,
                    ),
                }]
        })
    });
    let first = client_grid_state(&mut harness, 0).unwrap();

    harness.add_client(2);
    harness.step_until(|harness| {
        harness.client_is_active(1)
            && client_grid_state(harness, 1).is_some_and(|state| state == first)
    });
}

#[test]
fn hot_zone_and_ashen_snapshots_converge_from_canonical_content() {
    let mut hot_zone = Harness::new_hot_zone_match(1);
    hot_zone.step_until(|harness| {
        harness.client_is_active(0) && client_grid_state(harness, 0).is_some()
    });
    let (hot_zone_snapshot, _) = client_grid_state(&mut hot_zone, 0).unwrap();
    assert_eq!(
        hot_zone_snapshot.identity.source_preset_id,
        Some(brawler::map::FEATURE_YARD_HOT_ZONE_PRESET)
    );
    assert_eq!(hot_zone_snapshot.mode_anchors.len(), 1);

    let mut ashen = Harness::new_ashen_court(1);
    ashen.step_until(|harness| {
        harness.client_is_active(0) && client_grid_state(harness, 0).is_some()
    });
    let (ashen_snapshot, _) = client_grid_state(&mut ashen, 0).unwrap();
    assert_eq!(
        ashen_snapshot.identity.source_preset_id,
        Some(ArenaPresetId(3))
    );
    assert_eq!(ashen_snapshot.placements.len(), 108);
    assert!(ashen_snapshot.mode_anchors.is_empty());

    ashen.add_client(2);
    ashen.step_until(|harness| {
        harness.client_is_active(1)
            && client_grid_state(harness, 1).is_some_and(|(snapshot, _)| snapshot == ashen_snapshot)
    });
}

#[test]
fn map_content_mismatch_rejects_before_fighter_spawn() {
    let mut harness = Harness::new(1);
    harness.clients[0]
        .world_mut()
        .resource_mut::<MapCatalogResource>()
        .0
        .presets[0]
        .recipe
        .revision = 99;
    harness.step_until(|harness| {
        let world = harness.clients[0].world_mut();
        let mut query = world.query_filtered::<&ClientJoinStatus, With<Client>>();
        query.iter(world).next().is_some_and(|status| {
            matches!(
                status.phase,
                ClientJoinPhase::Rejected(brawler::protocol::MatchJoinRejection::ContentMismatch)
            )
        })
    });
    assert!(harness.server_ids().is_empty());
}

#[test]
fn client_local_snapshot_edit_has_no_authoritative_map_path() {
    let mut harness = Harness::new(1);
    harness
        .step_until(|harness| harness.client_is_active(0) && client_snapshot(harness, 0).is_some());
    let authoritative = harness
        .server
        .world()
        .resource::<ResolvedMap>()
        .snapshot
        .clone();
    let client_world = harness.clients[0].world_mut();
    let mut query = client_world.query_filtered::<&mut ResolvedMapSnapshot, With<MapRoot>>();
    query.single_mut(client_world).unwrap().dimensions.width = 128;
    for _ in 0..30 {
        harness.step();
    }
    assert_eq!(
        harness.server.world().resource::<ResolvedMap>().snapshot,
        authoritative
    );
    assert_eq!(
        harness.server.world().resource::<PlayableBounds>().0,
        authoritative.dimensions.bounds()
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the scenario proves partial barrel health and terminal removal convergence across both clients"
)]
fn barrel_partial_health_and_terminal_absence_converge_for_two_clients() {
    let mut harness = Harness::new_feature_yard_match(2);
    harness.step_until(|harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.loadout_is_ready(index))
    });
    let waiting = server_match_state(&mut harness);
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 1,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness.step_until(|harness| {
        matches!(server_match_state(harness).phase, MatchPhase::Active { .. })
            && (0..2).all(|index| {
                client_barrel_health(harness, index, 240) == Some(60)
                    && *harness.clients[index]
                        .world()
                        .resource::<ClientWorldObjectReadiness>()
                        == ClientWorldObjectReadiness::Ready
            })
    });

    let (target, source) = {
        let world = harness.server.world_mut();
        let mut objects = world.query::<&DamageableTargetIdentity>();
        let target = *objects
            .iter(world)
            .find(|identity| identity.placement_id() == MapPlacementId(240))
            .unwrap();
        let mut fighters = world
            .query_filtered::<(&PlayerId, &NetworkEntityId, &TeamId, &Position), With<Fighter>>();
        let (player, network_id, team, position) = fighters.iter(world).next().unwrap();
        (
            target,
            AttackSource {
                kind: CombatSourceKind::PrimaryWeapon,
                attack_id: AttackId(700),
                player_id: *player,
                owner_network_entity_id: *network_id,
                team_id: *team,
                recipe_fingerprint: WeaponRecipeFingerprint::default(),
                legacy_compatibility: false,
                source_preset_id: None,
                origin: WorldPoint::from(position.0),
                facing: 0.0,
            },
        )
    };
    let terminal_source = AttackSource {
        attack_id: AttackId(701),
        ..source
    };
    harness
        .server
        .world_mut()
        .resource_mut::<PendingWorldTargetDamages>()
        .0
        .push(PendingWorldTargetDamage {
            target,
            source,
            attack_id: source.attack_id,
            requested_damage: 20,
            delivery_index: 0,
            bundle_index: 0,
            effect_index: 0,
        });
    harness.step_until(|harness| {
        (0..2).all(|index| client_barrel_health(harness, index, 240) == Some(40))
    });

    harness
        .server
        .world_mut()
        .resource_mut::<PendingWorldTargetDamages>()
        .0
        .push(PendingWorldTargetDamage {
            target,
            source: terminal_source,
            attack_id: AttackId(701),
            requested_damage: 40,
            delivery_index: 0,
            bundle_index: 0,
            effect_index: 0,
        });
    harness.step_until(|harness| {
        (0..2).all(|index| {
            client_barrel_health(harness, index, 240).is_none()
                && *harness.clients[index]
                    .world()
                    .resource::<ClientWorldObjectReadiness>()
                    == ClientWorldObjectReadiness::Ready
        })
    });
    let transition = client_grid_state(&mut harness, 0)
        .unwrap()
        .1
        .terminal_states
        .iter()
        .find(|transition| transition.placement_id == MapPlacementId(240))
        .copied()
        .unwrap();
    assert_eq!(
        transition.outcome,
        brawler::map::MapPlacementOutcome::ReplacedWith(brawler::map::BARREL_WOOD_DEBRIS_ASSET,)
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the scenario proves one complete barrel, combat, replication, and telemetry transaction"
)]
fn barrel_explosion_damage_is_combat_owned_and_replicates_to_both_clients() {
    let mut harness = Harness::new_feature_yard_match(2);
    harness.step_until(|harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.loadout_is_ready(index))
    });
    let waiting = server_match_state(&mut harness);
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 1,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness.step_until(|harness| {
        matches!(server_match_state(harness).phase, MatchPhase::Active { .. })
            && client_barrel_health(harness, 0, 240) == Some(60)
    });

    let source_player = harness.controlled_player_id(0);
    let target_player = harness.controlled_player_id(1);
    let (barrel, barrel_position) = {
        let world = harness.server.world_mut();
        let mut objects = world.query::<(&DamageableTargetIdentity, &Position)>();
        objects
            .iter(world)
            .find(|(identity, _)| identity.placement_id() == MapPlacementId(240))
            .map(|(identity, position)| (*identity, position.0))
            .expect("Feature Yard barrel")
    };
    let (source, target_entity, target_network_id, target_health) = {
        let world = harness.server.world_mut();
        let mut fighters = world.query_filtered::<(
            Entity,
            &PlayerId,
            &NetworkEntityId,
            &TeamId,
            &Position,
            &CurrentHealth,
        ), With<Fighter>>();
        let rows = fighters
            .iter(world)
            .map(|(entity, player, network_id, team, position, health)| {
                (entity, *player, *network_id, *team, position.0, *health)
            })
            .collect::<Vec<_>>();
        let (_, _, source_network_id, source_team, source_position, _) = rows
            .iter()
            .find(|(_, player, ..)| *player == source_player)
            .copied()
            .expect("source fighter");
        let (target_entity, _, target_network_id, _, _, target_health) = rows
            .iter()
            .find(|(_, player, ..)| *player == target_player)
            .copied()
            .expect("target fighter");
        (
            AttackSource {
                kind: CombatSourceKind::PrimaryWeapon,
                attack_id: AttackId(800),
                player_id: source_player,
                owner_network_entity_id: source_network_id,
                team_id: source_team,
                recipe_fingerprint: WeaponRecipeFingerprint::default(),
                legacy_compatibility: false,
                source_preset_id: None,
                origin: WorldPoint::from(source_position),
                facing: 0.0,
            },
            target_entity,
            target_network_id,
            target_health.0,
        )
    };
    harness
        .server
        .world_mut()
        .entity_mut(target_entity)
        .insert(Position::from_xy(barrel_position.x, barrel_position.y));
    harness
        .server
        .world_mut()
        .resource_mut::<PendingWorldTargetDamages>()
        .0
        .push(PendingWorldTargetDamage {
            target: barrel,
            source,
            attack_id: source.attack_id,
            requested_damage: 60,
            delivery_index: 0,
            bundle_index: 0,
            effect_index: 0,
        });

    let expected_health = target_health.saturating_sub(35);
    harness.step_until(|harness| {
        (0..2).all(|index| {
            client_fighter_health(harness, index, target_network_id)
                == Some((expected_health, expected_health == 0))
        })
    });
    assert_eq!(
        harness.server.world().get::<CurrentHealth>(target_entity),
        Some(&CurrentHealth(expected_health))
    );
    assert_eq!(
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .applied_damage,
        35
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the scenario keeps chest destruction, pickup replication, collection, and cleanup in one lifecycle proof"
)]
fn chest_drop_and_collection_converge_for_two_clients() {
    let mut harness = Harness::new_feature_yard_match(2);
    harness.step_until(|harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.loadout_is_ready(index))
    });
    let waiting = server_match_state(&mut harness);
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 1,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness.step_until(|harness| {
        matches!(server_match_state(harness).phase, MatchPhase::Active { .. })
            && (0..2).all(|index| client_barrel_health(harness, index, 260) == Some(80))
    });
    let (target, source) = {
        let world = harness.server.world_mut();
        let target = *world
            .query::<&DamageableTargetIdentity>()
            .iter(world)
            .find(|identity| identity.placement_id() == MapPlacementId(260))
            .unwrap();
        let (player, network_id, team, position) = world
            .query_filtered::<(&PlayerId, &NetworkEntityId, &TeamId, &Position), With<Fighter>>()
            .iter(world)
            .next()
            .unwrap();
        (
            target,
            AttackSource {
                kind: CombatSourceKind::PrimaryWeapon,
                attack_id: AttackId(811),
                player_id: *player,
                owner_network_entity_id: *network_id,
                team_id: *team,
                recipe_fingerprint: WeaponRecipeFingerprint::default(),
                legacy_compatibility: false,
                source_preset_id: None,
                origin: WorldPoint::from(position.0),
                facing: 0.0,
            },
        )
    };
    harness
        .server
        .world_mut()
        .resource_mut::<PendingWorldTargetDamages>()
        .0
        .push(PendingWorldTargetDamage {
            target,
            source,
            attack_id: source.attack_id,
            requested_damage: 80,
            delivery_index: 0,
            bundle_index: 0,
            effect_index: 0,
        });
    harness.step_until(|harness| {
        (0..2).all(|index| {
            client_barrel_health(harness, index, 260).is_none()
                && client_pickup_count(harness, index) == 1
        })
    });
    let (collector, collector_position, pickup_entity) = {
        let world = harness.server.world_mut();
        let pickup_entity = world
            .query_filtered::<Entity, With<RestorationPickup>>()
            .single(world)
            .unwrap();
        let (collector, _, collector_position) = world
            .query_filtered::<
                (Entity, &NetworkEntityId, &Position),
                (With<Fighter>, With<ActiveCombatant>),
            >()
            .iter(world)
            .min_by_key(|(_, id, _)| id.0)
            .unwrap();
        (collector, collector_position.0, pickup_entity)
    };
    harness
        .server
        .world_mut()
        .entity_mut(pickup_entity)
        .insert(Position::from_xy(
            collector_position.x,
            collector_position.y,
        ));
    harness
        .server
        .world_mut()
        .entity_mut(collector)
        .insert(CurrentHealth(50));
    for _ in 0..30 {
        harness.step();
    }
    let server_pickups = {
        let world = harness.server.world_mut();
        world
            .query_filtered::<Entity, With<RestorationPickup>>()
            .iter(world)
            .count()
    };
    assert_eq!(
        server_pickups, 0,
        "the authoritative pickup was not collected"
    );
    assert!((0..2).all(|index| client_pickup_count(&mut harness, index) == 0));
    assert!(
        harness
            .server
            .world()
            .get::<CurrentHealth>(collector)
            .is_some_and(|health| health.0 > 50 && health.0 <= 100)
    );
    let world = harness.server.world_mut();
    assert_eq!(
        world
            .query::<&RestorationPickupIdentity>()
            .iter(world)
            .count(),
        0
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the scenario compares point-blank obstruction for both supported damageable object families"
)]
fn point_blank_shots_damage_the_first_chest_or_barrel_without_passing_through() {
    let mut harness = Harness::new_feature_yard_match(2);
    harness.step_until(|harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.loadout_is_ready(index))
    });
    let waiting = server_match_state(&mut harness);
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 1,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness.step_until(|harness| {
        matches!(server_match_state(harness).phase, MatchPhase::Active { .. })
            && client_barrel_health(harness, 0, 240) == Some(60)
            && client_barrel_health(harness, 0, 260) == Some(80)
    });

    let source_player = harness.controlled_player_id(0);
    let (chest_position, barrel_entity) = {
        let world = harness.server.world_mut();
        let mut objects = world.query::<(Entity, &DamageableTargetIdentity, &Position)>();
        let chest_position = objects
            .iter(world)
            .find(|(_, identity, _)| identity.placement_id() == MapPlacementId(260))
            .map(|(_, _, position)| position.0)
            .expect("Feature Yard chest");
        let barrel_entity = objects
            .iter(world)
            .find(|(_, identity, _)| identity.placement_id() == MapPlacementId(240))
            .map(|(entity, _, _)| entity)
            .expect("Feature Yard barrel");
        (chest_position, barrel_entity)
    };
    let barrel_position = chest_position + Vec2::new(64.0, 0.0);
    harness
        .server
        .world_mut()
        .entity_mut(barrel_entity)
        .insert(Position::from_xy(barrel_position.x, barrel_position.y));
    {
        let world = harness.server.world_mut();
        let mut fighters = world.query_filtered::<(
            &PlayerId,
            &mut Position,
            &mut Rotation,
            &mut brawler::builds::ResolvedMatchLoadout,
            &mut brawler::combat::ResolvedWeapon,
        ), With<Fighter>>();
        for (player, mut position, mut rotation, mut loadout, mut weapon) in
            fighters.iter_mut(world)
        {
            if *player == source_player {
                position.0 = chest_position - Vec2::new(32.0, 0.0);
                *rotation = Rotation::IDENTITY;
                let brawler::combat::PayloadEffectDefinition::Damage { amount, .. } =
                    &mut loadout.primary_weapon.recipe.payload_bundles[0].effects[0]
                else {
                    panic!("Pulse Sidearm first payload is damage");
                };
                // Keep both objects alive so this test can observe first-hit ordering twice.
                *amount = 20;
                weapon.recipe = loadout.primary_weapon.recipe.clone();
            } else {
                position.0 = Vec2::new(-700.0, -400.0);
            }
        }
    }
    for _ in 0..3 {
        harness.step();
    }

    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        client_barrel_health(harness, 0, 260).is_some_and(|health| health < 80)
    });
    harness.set_controlled_input(0, FighterInput::default());
    let chest_health_after_point_blank_hit =
        client_barrel_health(&mut harness, 0, 260).expect("damaged chest remains live");
    assert_eq!(client_barrel_health(&mut harness, 0, 240), Some(60));
    assert_eq!(harness.server_projectile_count(), 0);

    for _ in 0..30 {
        harness.step();
    }
    {
        let world = harness.server.world_mut();
        let mut fighters =
            world.query_filtered::<(&PlayerId, &mut Position, &mut Rotation), With<Fighter>>();
        for (player, mut position, mut rotation) in fighters.iter_mut(world) {
            if *player == source_player {
                position.0 = barrel_position + Vec2::new(32.0, 0.0);
                *rotation = Rotation::radians(std::f32::consts::PI);
            }
        }
    }
    for _ in 0..3 {
        harness.step();
    }
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(-Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        client_barrel_health(harness, 0, 240).is_some_and(|health| health < 60)
    });
    harness.set_controlled_input(0, FighterInput::default());

    assert_eq!(
        client_barrel_health(&mut harness, 0, 260),
        Some(chest_health_after_point_blank_hit)
    );
    assert_eq!(harness.server_projectile_count(), 0);
}
