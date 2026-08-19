//! Authoritative map replication, recovery, and no-client-authority scenarios.

use super::*;

fn maximum_policy_resolved_map(
    catalog: &brawler::map::MapContentCatalog,
    instance_id: MapInstanceId,
) -> ResolvedMap {
    let mut recipe = catalog.presets[0].recipe.clone();
    recipe.recipe_id = brawler::map::MapRecipeId(99);
    recipe.revision = 1;
    recipe.visuals[0].kind = VisualPlacementKind::TiledRectangle {
        half_extents: Vec2::splat(512.0),
        cell_size: Vec2::splat(32.0),
    };
    while recipe.geometry.len() < catalog.policy.max_geometry {
        let index = recipe.geometry.len();
        recipe.geometry.push(GeometryPlacement {
            placement_id: MapPlacementId(1_000 + u32::try_from(index).unwrap()),
            collision_profile_id: CollisionProfileId(1),
            presentation_profile_id: Some(MapPresentationProfileId(2)),
            position: Vec2::new(0.0, 480.0),
            rotation: 0.0,
            shape: MapShape::Circle { radius: 4.0 },
        });
    }
    while recipe.entities.len() < catalog.policy.max_entities {
        let index = recipe.entities.len();
        recipe.entities.push(MapEntityPlacement {
            placement_id: MapPlacementId(2_000 + u32::try_from(index).unwrap()),
            definition_id: EntityDefinitionId(1),
            presentation_profile_id: MapPresentationProfileId(5),
            position: Vec2::new(0.0, 400.0),
            rotation: 0.0,
        });
    }
    while recipe.regions.len() < 4 {
        let index = recipe.regions.len();
        let column = index % 2;
        let row = index / 2;
        recipe.regions.push(MapRegionPlacement {
            placement_id: MapPlacementId(3_000 + u32::try_from(index).unwrap()),
            region_id: RegionId(u16::try_from(index + 1).unwrap()),
            profile_id: RegionProfileId(1),
            presentation_profile_id: MapPresentationProfileId(3),
            position: Vec2::new(240.0 + 80.0 * column as f32, 400.0 + 80.0 * row as f32),
            rotation: 0.0,
            shape: MapShape::Circle { radius: 8.0 },
        });
    }
    for team_slot in 0..=1 {
        let x = if team_slot == 0 { -768.0 } else { 768.0 };
        let facing = if team_slot == 0 {
            0.0
        } else {
            -std::f32::consts::PI
        };
        for y in [-224.0, -32.0, 160.0, 352.0] {
            let index = recipe.spawn_points.len();
            recipe.spawn_points.push(TeamSpawnPoint {
                placement_id: MapPlacementId(4_000 + u32::try_from(index).unwrap()),
                spawn_point_id: SpawnPointId(100 + u16::try_from(index).unwrap()),
                team_slot,
                position: Vec2::new(x, y),
                facing,
            });
        }
    }
    brawler::map::resolve_map_recipe(
        &recipe,
        None,
        instance_id,
        catalog,
        &MapLayoutRequirements::wipeout(),
        brawler::map::EngineMapLimits::default(),
    )
    .unwrap()
}

fn client_snapshot(harness: &mut Harness, index: usize) -> Option<ResolvedMapSnapshot> {
    let world = harness.clients[index].world_mut();
    let mut query = world.query_filtered::<&ResolvedMapSnapshot, With<MapRoot>>();
    query.iter(world).next().cloned()
}

#[test]
fn two_clients_receive_identical_map_snapshot_without_authoritative_colliders() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && client_snapshot(harness, 0).is_some()
            && client_snapshot(harness, 1).is_some()
    });
    let first = client_snapshot(&mut harness, 0).unwrap();
    let second = client_snapshot(&mut harness, 1).unwrap();
    assert_eq!(first, second);
    assert_eq!(first.identity.instance_id, MapInstanceId(1));
    assert_eq!(first.geometry.len(), 6);
    assert_eq!(first.spawn_points.len(), 8);
    assert_eq!(first.visual_instances.len(), 28 * 18);
    for client in &mut harness.clients {
        let world = client.world_mut();
        let mut walls = world.query_filtered::<Entity, With<ArenaWall>>();
        assert_eq!(walls.iter(world).count(), 0);
    }
    let world = harness.server.world_mut();
    assert_eq!(world.resource::<ResolvedMap>().snapshot, first);
    assert_eq!(world.query::<&MapInstanceMember>().iter(world).count(), 10);
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
        .resolve_preset(
            ArenaPresetId(1),
            MapInstanceId(2),
            &MapLayoutRequirements::wipeout(),
        )
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
    query
        .single_mut(client_world)
        .unwrap()
        .playable_bounds
        .min
        .x = -4_000.0;
    for _ in 0..30 {
        harness.step();
    }
    assert_eq!(
        harness.server.world().resource::<ResolvedMap>().snapshot,
        authoritative
    );
    assert_eq!(
        harness.server.world().resource::<PlayableBounds>().0,
        authoritative.playable_bounds
    );
}

#[test]
fn maximum_policy_snapshot_converges_over_impaired_real_udp() {
    for impairment_profile in [
        NetworkImpairmentProfile::Typical,
        NetworkImpairmentProfile::Adverse,
    ] {
        let server_config = ServerNetworkConfig {
            bind_addr: "127.0.0.1:0".parse().unwrap(),
            impairment_profile,
            client_timeout: std::time::Duration::from_mins(2),
            ..Default::default()
        };
        let mut server = App::new();
        server.insert_resource(server_config).add_plugins((
            MinimalPlugins,
            StatesPlugin,
            ServerPlugins {
                tick_duration: SIMULATION_TICK,
            },
            GameplayPlugin,
            ProtocolPlugin,
            AvianNetworkPlugin,
            AuthoritativeMapPlugin,
            AuthoritativeMovementPlugin,
            ServerNetworkPlugin,
        ));
        let catalog = server.world().resource::<MapCatalogResource>().0.clone();
        let maximum = maximum_policy_resolved_map(&catalog, MapInstanceId(1));
        let maximum_snapshot = maximum.snapshot.clone();
        let encoded_size = postcard::to_allocvec(&maximum_snapshot).unwrap().len();
        assert_eq!(maximum_snapshot.geometry.len(), catalog.policy.max_geometry);
        assert_eq!(
            maximum_snapshot.visual_instances.len(),
            catalog.policy.max_visual_instances
        );
        assert_eq!(maximum_snapshot.entities.len(), catalog.policy.max_entities);
        assert!(encoded_size > 1_200 && encoded_size <= 64 * 1_024);
        println!(
            "maximum-policy map snapshot: profile={} bytes={encoded_size} geometry={} visuals={} entities={}",
            impairment_profile.name(),
            maximum_snapshot.geometry.len(),
            maximum_snapshot.visual_instances.len(),
            maximum_snapshot.entities.len(),
        );
        install_resolved_map(server.world_mut(), maximum).unwrap();
        server.finish();
        server.cleanup();
        let mut now = Instant::now();
        server.insert_resource(TimeUpdateStrategy::ManualInstant(now));
        server.update();
        now += SIMULATION_TICK;
        server.insert_resource(TimeUpdateStrategy::ManualInstant(now));
        server.update();
        let server_addr = {
            let world = server.world_mut();
            let mut endpoints = world.query_filtered::<&LocalAddr, With<NetcodeServer>>();
            endpoints.iter(world).next().unwrap().0
        };

        let mut client_config = ClientNetworkConfig::new(1);
        client_config.server_addr = server_addr;
        client_config.headless = true;
        client_config.impairment_profile = impairment_profile;
        client_config.connect_timeout = std::time::Duration::from_mins(2);
        let mut client = App::new();
        client.insert_resource(client_config).add_plugins((
            MinimalPlugins,
            StatesPlugin,
            lightyear::prelude::client::ClientPlugins {
                tick_duration: SIMULATION_TICK,
            },
            GameplayPlugin,
            ProtocolPlugin,
            AvianNetworkPlugin,
            ClientNetworkPlugin,
        ));
        client.finish();
        client.cleanup();

        let mut converged = false;
        // The impaired transport retransmits at its own cadence; the Adverse profile
        // needs roughly double the Typical bound before a full fragmented snapshot is
        // reliably delivered, so the margin keeps the test sensitive but not flaky.
        let tick_bound = match impairment_profile {
            NetworkImpairmentProfile::Local | NetworkImpairmentProfile::Typical => 3_600,
            NetworkImpairmentProfile::Adverse => 7_200,
        };
        for _ in 0..tick_bound {
            now += SIMULATION_TICK;
            client.insert_resource(TimeUpdateStrategy::ManualInstant(now));
            client.update();
            server.insert_resource(TimeUpdateStrategy::ManualInstant(now));
            server.update();
            std::thread::yield_now();
            let world = client.world_mut();
            let mut snapshots = world.query_filtered::<&ResolvedMapSnapshot, With<MapRoot>>();
            if snapshots.iter(world).next() == Some(&maximum_snapshot) {
                converged = true;
                break;
            }
        }
        assert!(
            converged,
            "{encoded_size}-byte maximum snapshot did not converge under {} impairment",
            impairment_profile.name(),
        );
    }
}
