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
fn tidal_barrier_replacement_converges_for_connected_and_late_joining_clients() {
    let catalog = brawler::map::MapContentCatalog::embedded().unwrap();
    let resolved = catalog
        .resolve_preset(brawler::map::TIDAL_GARDEN_PRESET, MapInstanceId(1))
        .unwrap();
    let placement = resolved
        .dynamic_placements
        .iter()
        .find(|placement| placement.placement_id == MapPlacementId(302))
        .unwrap();
    let center = brawler::map::placement_world_center(
        resolved.snapshot.dimensions,
        catalog.asset(placement.asset_id).unwrap(),
        placement,
    );
    let mut harness = Harness::new_tidal_garden(1);
    harness.step_until(|harness| {
        harness.client_is_active(0) && client_grid_state(harness, 0).is_some()
    });

    harness.inject_map_destruction(901, (center.x, center.y), 1.0);
    harness.step_until(|harness| {
        client_grid_state(harness, 0).is_some_and(|(_, state)| {
            state.terminal_states
                == vec![brawler::map::MapPlacementTransition {
                    placement_id: MapPlacementId(302),
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
        Some(ArenaPresetId(2))
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
