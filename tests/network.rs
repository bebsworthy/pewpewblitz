use bevy::{
    app::App,
    platform::time::Instant,
    prelude::{Entity, MinimalPlugins, With},
    state::app::StatesPlugin,
    time::TimeUpdateStrategy,
};
use brawler::{
    client::{ClientJoinPhase, ClientJoinStatus, ClientNetworkPlugin, spawn_crossbeam_client},
    config::{ClientNetworkConfig, NetworkTransport, ServerNetworkConfig},
    gameplay::GameplayPlugin,
    protocol::{NetworkEntityId, PlaceholderPlayer, PlayerId, ProtocolPlugin},
    server::{
        ServerNetworkPlugin, ServerSession, ServerSessionPhase, spawn_crossbeam_link,
        spawn_crossbeam_server,
    },
    timing::SIMULATION_TICK,
};
use lightyear::prelude::client::{Client, Connected, Disconnect, Disconnected, Remote};
use lightyear::prelude::server::{NetcodeServer, ServerPlugins};
use lightyear::prelude::{AppMessageExt, LocalAddr, NetworkDirection};
use serde::{Deserialize, Serialize};

#[derive(Serialize, Deserialize, Clone, Debug)]
struct MismatchedMessage(u8);

struct MismatchedProtocolPlugin;

impl bevy::prelude::Plugin for MismatchedProtocolPlugin {
    fn build(&self, app: &mut App) {
        app.register_message::<MismatchedMessage>()
            .add_direction(NetworkDirection::Bidirectional);
    }
}

struct Harness {
    server: App,
    server_entity: Entity,
    server_links: Vec<Entity>,
    clients: Vec<App>,
    client_entities: Vec<Entity>,
    now: Instant,
}

impl Harness {
    fn new(client_count: usize) -> Self {
        Self::new_with_options(client_count, None, false)
    }

    fn new_with_protocol(client_count: usize, client_protocol_id: Option<u64>) -> Self {
        Self::new_with_options(client_count, client_protocol_id, false)
    }

    fn new_with_extra_protocol(client_count: usize) -> Self {
        Self::new_with_options(client_count, None, true)
    }

    fn new_with_options(
        client_count: usize,
        client_protocol_id: Option<u64>,
        extra_protocol: bool,
    ) -> Self {
        let server_config = ServerNetworkConfig {
            transport: NetworkTransport::Crossbeam,
            handshake_timeout: std::time::Duration::from_millis(250),
            ..Default::default()
        };

        let mut server = App::new();
        server.insert_resource(server_config.clone()).add_plugins((
            MinimalPlugins,
            StatesPlugin,
            ServerPlugins {
                tick_duration: SIMULATION_TICK,
            },
            GameplayPlugin,
            ProtocolPlugin,
            ServerNetworkPlugin,
        ));
        server.finish();
        server.cleanup();
        let server_entity = spawn_crossbeam_server(server.world_mut(), &server_config);

        let mut clients = Vec::with_capacity(client_count);
        let mut client_entities = Vec::with_capacity(client_count);
        let mut server_links = Vec::with_capacity(client_count);
        for client_id in 1..=client_count as u64 {
            let mut config = ClientNetworkConfig::new(client_id);
            config.transport = NetworkTransport::Crossbeam;
            if client_id == 1
                && let Some(protocol_id) = client_protocol_id
            {
                config.network_protocol_id = protocol_id;
            }
            let mut client = App::new();
            client.insert_resource(config).add_plugins((
                MinimalPlugins,
                StatesPlugin,
                lightyear::prelude::client::ClientPlugins {
                    tick_duration: SIMULATION_TICK,
                },
                GameplayPlugin,
                ProtocolPlugin,
            ));
            if extra_protocol {
                client.add_plugins(MismatchedProtocolPlugin);
            }
            client.add_plugins(ClientNetworkPlugin);
            client.finish();
            client.cleanup();
            let (client_io, server_io) = lightyear::crossbeam::CrossbeamIo::new_pair();
            let config = client.world().resource::<ClientNetworkConfig>().clone();
            let client_entity = spawn_crossbeam_client(client.world_mut(), config, client_io);
            let server_link = spawn_crossbeam_link(server.world_mut(), server_entity, server_io);
            clients.push(client);
            client_entities.push(client_entity);
            server_links.push(server_link);
        }

        Self {
            server,
            server_entity,
            server_links,
            clients,
            client_entities,
            now: Instant::now(),
        }
    }

    fn step(&mut self) {
        self.now += SIMULATION_TICK;
        for client in &mut self.clients {
            client.insert_resource(TimeUpdateStrategy::ManualInstant(self.now));
            client.update();
        }
        self.server
            .insert_resource(TimeUpdateStrategy::ManualInstant(self.now));
        self.server.update();
    }

    fn step_until(&mut self, mut condition: impl FnMut(&mut Self) -> bool) {
        for _ in 0..240 {
            self.step();
            if condition(self) {
                return;
            }
        }
        panic!("network harness condition did not become true");
    }

    fn server_ids(&mut self) -> Vec<(PlayerId, NetworkEntityId)> {
        let mut query = self
            .server
            .world_mut()
            .query_filtered::<(&PlayerId, &NetworkEntityId), With<PlaceholderPlayer>>();
        let mut ids: Vec<_> = query
            .iter(self.server.world())
            .map(|(player, entity)| (*player, *entity))
            .collect();
        ids.sort_by_key(|(player, entity)| (player.0, entity.0));
        ids
    }

    fn client_ids(&mut self, index: usize) -> Vec<(PlayerId, NetworkEntityId)> {
        let world = self.clients[index].world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &NetworkEntityId), With<Remote>>();
        let mut ids: Vec<_> = query
            .iter(world)
            .map(|(player, entity)| (*player, *entity))
            .collect();
        ids.sort_by_key(|(player, entity)| (player.0, entity.0));
        ids
    }

    fn client_is_active(&mut self, index: usize) -> bool {
        let world = self.clients[index].world_mut();
        let mut query = world.query::<&ClientJoinStatus>();
        query
            .iter(world)
            .any(|status| matches!(status.phase, ClientJoinPhase::Active { .. }))
    }

    fn active_server_sessions(&mut self) -> usize {
        let world = self.server.world_mut();
        let mut query = world.query::<&ServerSession>();
        query
            .iter(world)
            .filter(|session| matches!(session.phase, ServerSessionPhase::Active { .. }))
            .count()
    }
}

#[test]
fn two_clients_connect_and_receive_the_same_server_owned_roster() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
            && harness.client_ids(0).len() == 2
            && harness.client_ids(1).len() == 2
    });

    let server_ids = harness.server_ids();
    assert_eq!(harness.client_ids(0), server_ids);
    assert_eq!(harness.client_ids(1), server_ids);
    assert_eq!(harness.active_server_sessions(), 2);

    let mut query = harness.server.world_mut().query_filtered::<(
        &lightyear::prelude::Replicate,
        &lightyear::prelude::ControlledBy,
    ), With<PlaceholderPlayer>>();
    assert_eq!(query.iter(harness.server.world()).count(), 2);
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
    assert!(harness.server_ids().is_empty());
}

#[test]
#[should_panic(expected = "the message protocol doesn't match")]
fn lightyear_registry_mismatch_disconnects_before_brawler_acceptance() {
    let mut harness = Harness::new_with_extra_protocol(1);
    for _ in 0..90 {
        harness.step();
    }
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
    let mut harness = Harness::new(1);
    harness.step_until(|harness| harness.client_is_active(0) && harness.server_ids().len() == 1);
    brawler::server::request_stop(harness.server.world_mut(), harness.server_entity);
    for _ in 0..30 {
        harness.step();
    }
    assert!(harness.server_ids().is_empty());
    let client_entity = harness.client_entities[0];
    assert!(
        harness.clients[0]
            .world()
            .get::<Connected>(client_entity)
            .is_none()
    );
}

#[test]
fn disconnect_cleans_owned_placeholder_and_reconnect_allocates_fresh_ids() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| harness.client_is_active(0) && harness.server_ids().len() == 1);
    let first_ids = harness.server_ids();

    harness.clients[0].world_mut().trigger(Disconnect {
        entity: harness.client_entities[0],
    });
    harness.step_until(|harness| harness.server_ids().is_empty());

    // A fresh Bevy client world models a reconnecting process/session while reusing the
    // development Netcode ID. The old server link is gone before this new link is attached.
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
        ClientNetworkPlugin,
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
    let index = harness.clients.len() - 1;

    harness
        .step_until(|harness| harness.client_is_active(index) && harness.server_ids().len() == 1);
    let second_ids = harness.server_ids();
    assert_ne!(first_ids, second_ids);
    assert_eq!(harness.client_ids(index), second_ids);
}

#[test]
fn real_udp_loopback_connects_and_replicates_one_placeholder() {
    let server_config = ServerNetworkConfig {
        bind_addr: "127.0.0.1:0".parse().expect("loopback address"),
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
        ServerNetworkPlugin,
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
    let mut client = App::new();
    client.insert_resource(client_config).add_plugins((
        MinimalPlugins,
        StatesPlugin,
        lightyear::prelude::client::ClientPlugins {
            tick_duration: SIMULATION_TICK,
        },
        GameplayPlugin,
        ProtocolPlugin,
        ClientNetworkPlugin,
    ));
    client.finish();
    client.cleanup();

    let mut connected = false;
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
        let mut server_query = server_world.query_filtered::<Entity, With<PlaceholderPlayer>>();
        let server_spawned = server_query.iter(server_world).next().is_some();
        let mut remote_query = client.world_mut().query_filtered::<Entity, With<Remote>>();
        let client_replicated = remote_query.iter(client.world()).next().is_some();
        if client_connected && server_spawned && client_replicated {
            connected = true;
            break;
        }
    }
    assert!(
        connected,
        "real UDP client did not complete connect/hello/replication"
    );
}
