//! Network integration scenarios extracted from the shared harness.

use super::*;

#[test]
fn protocol_version_mismatch_is_rejected_without_a_placeholder() {
    let mut harness = Harness::new(1);
    harness.clients[0]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .expected_protocol_version += 1;

    harness.step_until(|harness| {
        let world = harness.clients[0].world_mut();
        let mut query = world.query::<&ClientJoinStatus>();
        query.iter(world).any(|status| {
            matches!(
                status.phase,
                ClientJoinPhase::Rejected(
                    brawler::protocol::JoinRejection::ProtocolVersionMismatch
                )
            )
        })
    });
    assert!(harness.server_ids().is_empty());
}

#[test]
fn active_client_with_incomplete_roster_times_out() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| harness.client_is_active(0) && harness.server_ids().len() == 1);
    let client = &mut harness.clients[0];
    let mut config = client.world_mut().resource_mut::<ClientNetworkConfig>();
    config.exit_after_roster = Some(99);
    config.connect_timeout = std::time::Duration::from_millis(50);

    for _ in 0..20 {
        harness.step();
    }
    assert!(
        harness.clients[0]
            .should_exit()
            .is_some_and(|exit| exit.is_error())
    );
}

#[test]
fn incompatible_build_is_rejected_without_a_placeholder() {
    let mut harness = Harness::new(1);
    harness.clients[0]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .expected_build_version = "incompatible-build".to_string();

    harness.step_until(|harness| {
        let world = harness.clients[0].world_mut();
        let mut query = world.query::<&ClientJoinStatus>();
        query
            .iter(world)
            .any(|status| matches!(status.phase, ClientJoinPhase::Rejected(_)))
    });
    assert!(harness.server_ids().is_empty());
}

#[test]
fn netcode_protocol_id_mismatch_disconnects_before_brawler_acceptance() {
    let mut harness =
        Harness::new_with_protocol(1, Some(brawler::protocol::NETWORK_PROTOCOL_ID + 1));
    for _ in 0..300 {
        harness.step();
    }
    let client_entity = harness.client_entities[0];
    assert!(
        harness.clients[0]
            .world()
            .get::<Disconnected>(client_entity)
            .is_some()
    );
    assert!(
        harness.clients[0]
            .world()
            .get::<Connected>(client_entity)
            .is_none()
    );
    let mut status_query = harness.clients[0].world_mut().query::<&ClientJoinStatus>();
    assert!(
        status_query
            .iter(harness.clients[0].world())
            .any(|status| { matches!(status.phase, ClientJoinPhase::Disconnected) })
    );
    assert!(harness.server_ids().is_empty());
}

#[test]
fn lightyear_registry_mismatch_disconnects_before_brawler_acceptance() {
    let mut harness = Harness::new_with_extra_protocol(1);
    harness.step_until(|harness| {
        let world = harness.clients[0].world_mut();
        let mut query = world.query::<&ClientJoinStatus>();
        query.iter(world).any(|status| {
            matches!(
                status.phase,
                ClientJoinPhase::Rejected(brawler::protocol::JoinRejection::RegistryMismatch)
            )
        })
    });
    assert!(harness.server_ids().is_empty());
    assert!(
        harness.clients[0]
            .world()
            .get::<Disconnected>(harness.client_entities[0])
            .is_some()
    );
}

#[test]
fn connected_client_without_hello_times_out_without_owned_entities() {
    let mut harness = Harness::new(0);
    let mut config = ClientNetworkConfig::new(1);
    config.transport = NetworkTransport::Crossbeam;
    let mut client = App::new();
    client.insert_resource(config.clone()).add_plugins((
        MinimalPlugins,
        StatesPlugin,
        lightyear::prelude::client::ClientPlugins {
            tick_duration: SIMULATION_TICK,
        },
        GameplayPlugin,
        ProtocolPlugin,
    ));
    client.finish();
    client.cleanup();
    let (client_io, server_io) = lightyear::crossbeam::CrossbeamIo::new_pair();
    let client_entity = spawn_crossbeam_client(client.world_mut(), config, client_io);
    let server_link =
        spawn_crossbeam_link(harness.server.world_mut(), harness.server_entity, server_io);
    harness.clients.push(client);
    harness.client_entities.push(client_entity);
    harness.server_links.push(server_link);

    for _ in 0..120 {
        harness.step();
    }
    assert!(harness.server_ids().is_empty());
    assert!(harness.server.world().get_entity(server_link).is_err());
}

#[test]
fn graceful_server_stop_removes_sessions_and_owned_placeholders() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
    });
    brawler::server::request_stop(harness.server.world_mut(), harness.server_entity);
    for _ in 0..30 {
        harness.step();
    }
    assert!(harness.server_ids().is_empty());
    assert!(
        harness
            .server
            .world()
            .get::<Stopped>(harness.server_entity)
            .is_some()
    );
    for link in &harness.server_links {
        assert!(harness.server.world().get_entity(*link).is_err());
    }
    for (client, entity) in harness.clients.iter().zip(&harness.client_entities) {
        assert!(client.world().get::<Connected>(*entity).is_none());
        assert!(client.world().get::<Disconnected>(*entity).is_some());
    }
}

#[test]
fn disconnect_cleans_owned_placeholder_and_reconnect_allocates_fresh_ids() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
    });
    let first_ids = harness.server_ids();
    let static_count = harness.server_static_arena_count();

    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| harness.server_projectile_count() > 0);

    harness.clients[0].world_mut().trigger(Disconnect {
        entity: harness.client_entities[0],
    });
    for _ in 0..240 {
        harness.step();
        if harness.server_ids().len() == 1 {
            break;
        }
    }
    assert_eq!(harness.server_ids().len(), 1);
    assert_eq!(harness.server_projectile_count(), 0);
    let remaining_ids = harness.server_ids();
    for _ in 0..240 {
        harness.step();
        if harness.client_ids(1) == remaining_ids {
            break;
        }
    }
    assert_eq!(harness.client_ids(1), remaining_ids);
    harness.clients[0].world_mut().trigger(Disconnect {
        entity: harness.client_entities[0],
    });
    for _ in 0..10 {
        harness.step();
    }
    assert_eq!(harness.server_ids(), remaining_ids);

    // A fresh Bevy client world models a reconnecting process/session while reusing the
    // development Netcode ID. The old server link is gone before this new link is attached.
    harness.add_client(1);
    let index = harness.clients.len() - 1;

    harness.step_until(|harness| {
        harness.client_is_active(index)
            && harness.server_ids().len() == 2
            && harness.client_ids(index).len() == 2
    });
    let second_ids = harness.server_ids();
    assert_eq!(second_ids.len(), 2);
    assert_ne!(first_ids[0], second_ids[1]);
    assert_eq!(harness.client_ids(index), second_ids);
    assert_eq!(harness.server_static_arena_count(), static_count);
    let server_snapshot = harness
        .server
        .world()
        .resource::<ResolvedMap>()
        .snapshot
        .clone();
    let client_world = harness.clients[index].world_mut();
    let mut maps = client_world.query_filtered::<&ResolvedMapSnapshot, With<MapRoot>>();
    assert_eq!(maps.iter(client_world).next(), Some(&server_snapshot));
}

#[test]
fn disconnect_before_fixed_sweep_removes_near_impact_projectile_without_damage() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| harness.server_projectile_count() == 1);

    let (projectile, health_before) = {
        let world = harness.server.world_mut();
        let mut dummy_query =
            world.query_filtered::<(&Position, &CurrentHealth), With<TestDummy>>();
        let (dummy_position, health) = dummy_query
            .single(world)
            .map(|(position, health)| (*position, *health))
            .expect("dummy");
        let mut projectile_query = world.query_filtered::<Entity, With<Projectile>>();
        let projectile = projectile_query.single(world).expect("projectile");
        world
            .get_mut::<Position>(projectile)
            .expect("projectile position")
            .0 = dummy_position.0 - Vec2::new(20.0, 0.0);
        world
            .get_mut::<ComposedProjectileRuntime>(projectile)
            .expect("projectile runtime")
            .velocity = Vec2::new(900.0, 0.0);
        (projectile, health)
    };
    let server_link = harness.server_links[0];
    harness
        .server
        .world_mut()
        .entity_mut(server_link)
        .insert(Disconnected::default());
    harness.step();

    assert!(harness.server.world().get_entity(projectile).is_err());
    let health_after = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
        query.single(world).expect("dummy").0
    };
    assert_eq!(health_after, health_before.0);
}

#[test]
fn fabricated_orphan_projectile_is_rejected_before_collision() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });
    let (projectile, health_before) = {
        let world = harness.server.world_mut();
        let mut dummy_query =
            world.query_filtered::<(&Position, &CurrentHealth, &ResolvedWeapon), With<TestDummy>>();
        let (dummy_position, health, resolved) = dummy_query
            .single(world)
            .map(|(position, health, resolved)| (*position, *health, resolved.clone()))
            .expect("dummy");
        let source = AttackSource {
            kind: CombatSourceKind::PrimaryWeapon,
            attack_id: AttackId(99_999),
            player_id: PlayerId(99),
            owner_network_entity_id: NetworkEntityId(99_999),
            team_id: TeamId(0),
            recipe_fingerprint: resolved.recipe_fingerprint,
            presentation_profile_id: resolved.presentation_profile_id,
            legacy_compatibility: false,
            source_preset_id: resolved.source_preset_id,
            origin: WorldPoint::from(dummy_position.0 - Vec2::new(20.0, 0.0)),
            facing: 0.0,
        };
        let projectile = world
            .spawn((
                Projectile,
                AttackDelivery {
                    attack_id: AttackId(99_999),
                    delivery_index: 0,
                },
                ReplicatedAttackSource { attack: source },
                ComposedProjectileRuntime {
                    owner_entity: Entity::PLACEHOLDER,
                    source_entity: Entity::PLACEHOLDER,
                    source,
                    delivery_index: 0,
                    velocity: Vec2::new(900.0, 0.0),
                    travelled: 0.0,
                    expires_at_tick: u64::MAX,
                    maximum_range: 1_000.0,
                    radius: 6.0,
                    landing: None,
                    recipe: resolved.recipe,
                },
                Position(dummy_position.0 - Vec2::new(20.0, 0.0)),
                Rotation::IDENTITY,
                Collider::circle(6.0),
                CollisionLayers::new(
                    brawler::movement::PROJECTILE_LAYER,
                    brawler::movement::FIGHTER_LAYER
                        | brawler::movement::INDESTRUCTIBLE_TERRAIN_LAYER
                        | brawler::movement::DESTRUCTIBLE_TERRAIN_LAYER,
                ),
            ))
            .id();
        (projectile, health)
    };
    harness.step();

    assert!(harness.server.world().get_entity(projectile).is_err());
    let health_after = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
        query.single(world).expect("dummy").0
    };
    assert_eq!(health_after, health_before.0);
}

#[test]
#[allow(clippy::too_many_lines)]
fn real_udp_loopback_moves_and_replicates_authoritative_pose() {
    let server_config = ServerNetworkConfig {
        bind_addr: "127.0.0.1:0".parse().expect("loopback address"),
        ..Default::default()
    };

    let mut server = App::new();
    server
        .insert_resource(server_config)
        .insert_resource(brawler::matchplay::MatchLifecycleRules::default())
        .insert_resource(brawler::matchplay::WipeoutRules::default())
        .add_plugins((
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
            brawler::matchplay::AuthoritativeMatchPlugin,
            brawler::matchplay::WipeoutModePlugin,
            brawler::terrain::AuthoritativeTerrainPlugin,
        ));
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
        let mut query = world.query_filtered::<&LocalAddr, With<NetcodeServer>>();
        query
            .iter(world)
            .next()
            .expect("UDP server endpoint should be spawned")
            .0
    };
    assert_ne!(
        server_addr.port(),
        0,
        "UDP server should bind an OS-assigned port"
    );

    let mut client_config = ClientNetworkConfig::new(1);
    client_config.server_addr = server_addr;
    client_config.headless = true;
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

    let mut connected = false;
    let mut final_state = (false, false, false);
    for _ in 0..240 {
        now += SIMULATION_TICK;
        client.insert_resource(TimeUpdateStrategy::ManualInstant(now));
        client.update();
        server.insert_resource(TimeUpdateStrategy::ManualInstant(now));
        server.update();
        std::thread::yield_now();

        let client_world = client.world_mut();
        let mut client_query =
            client_world.query_filtered::<Entity, (With<Client>, With<Connected>)>();
        let client_connected = client_query.iter(client_world).next().is_some();
        let server_world = server.world_mut();
        let mut server_query =
            server_world.query_filtered::<Entity, (With<PlaceholderPlayer>, Without<TestDummy>)>();
        let server_spawned = server_query.iter(server_world).next().is_some();
        let mut remote_query = client.world_mut().query_filtered::<Entity, With<Remote>>();
        let client_replicated = remote_query.iter(client.world()).next().is_some();
        final_state = (client_connected, server_spawned, client_replicated);
        if client_connected && server_spawned && client_replicated {
            connected = true;
            break;
        }
    }
    assert!(
        connected,
        "real UDP client did not complete connect/hello/replication: {final_state:?}"
    );
    {
        let world = server.world_mut();
        let entities: Vec<_> = world
            .query_filtered::<Entity, (With<Fighter>, Without<TestDummy>)>()
            .iter(world)
            .collect();
        for entity in entities {
            world.entity_mut(entity).insert(ActiveCombatant);
        }
    }

    let initial_x = {
        let world = server.world_mut();
        let mut query = world.query_filtered::<&Position, (With<Fighter>, Without<TestDummy>)>();
        query
            .iter(world)
            .next()
            .expect("UDP server should have one fighter")
            .0
            .x
    };
    {
        let mut pending = client.world_mut().resource_mut::<PendingLocalActions>();
        pending.move_axis = Vec2::X;
        pending.aim_axis = Some(Vec2::Y);
    }
    for _ in 0..120 {
        now += SIMULATION_TICK;
        client.insert_resource(TimeUpdateStrategy::ManualInstant(now));
        client.update();
        server.insert_resource(TimeUpdateStrategy::ManualInstant(now));
        server.update();
        std::thread::yield_now();
    }
    let (final_x, final_facing) = {
        let world = server.world_mut();
        let mut query =
            world.query_filtered::<(&Position, &Rotation), (With<Fighter>, Without<TestDummy>)>();
        let (position, rotation) = query
            .iter(world)
            .next()
            .expect("UDP server should retain one fighter");
        (position.0.x, rotation.as_radians())
    };
    assert!(final_x > initial_x + 100.0);
    assert!((final_facing - std::f32::consts::FRAC_PI_2).abs() < 0.05);
}
