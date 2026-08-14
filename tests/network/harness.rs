//! Shared deterministic Crossbeam client/server integration harness.

use super::*;

#[derive(Serialize, Deserialize, Clone, Debug)]
struct MismatchedMessage(u8);

struct MismatchedProtocolPlugin;

impl bevy::prelude::Plugin for MismatchedProtocolPlugin {
    fn build(&self, app: &mut App) {
        app.register_message::<MismatchedMessage>()
            .add_direction(NetworkDirection::Bidirectional);
    }
}

#[derive(Resource, Debug, Default)]
pub(super) struct CuePacketImpairment {
    pub(super) armed: bool,
    pub(super) injected: bool,
    pub(super) duplicated_packets: u32,
    pub(super) reordered_batches: u32,
    pub(super) held_packet: Option<lightyear::link::RecvPayload>,
}

fn impair_cue_packets(
    mut impairment: ResMut<CuePacketImpairment>,
    mut links: Query<&mut Link, With<Client>>,
) {
    if !impairment.armed || impairment.injected {
        return;
    }
    for mut link in &mut links {
        let mut packets: Vec<_> = link.recv.drain().collect();
        if let Some(held_packet) = impairment.held_packet.take() {
            packets.push(held_packet);
        }
        if packets.len() < 2 {
            impairment.held_packet = packets.pop();
            for packet in packets {
                link.recv.push_raw(packet);
            }
            continue;
        }
        packets.reverse();
        let duplicate = packets[1].clone();
        packets.insert(2, duplicate);
        for packet in packets {
            link.recv.push_raw(packet);
        }
        impairment.injected = true;
        impairment.duplicated_packets = impairment.duplicated_packets.saturating_add(1);
        impairment.reordered_batches = impairment.reordered_batches.saturating_add(1);
        break;
    }
}

pub(super) struct Harness {
    pub(super) server: App,
    pub(super) server_entity: Entity,
    pub(super) server_links: Vec<Entity>,
    pub(super) clients: Vec<App>,
    pub(super) client_entities: Vec<Entity>,
    pub(super) client_cues: Vec<Vec<CombatCue>>,
    pub(super) now: Instant,
}

impl Harness {
    pub(super) fn new(client_count: usize) -> Self {
        Self::new_with_options(client_count, None, false)
    }

    pub(super) fn new_with_protocol(client_count: usize, client_protocol_id: Option<u64>) -> Self {
        Self::new_with_options(client_count, client_protocol_id, false)
    }

    pub(super) fn new_with_extra_protocol(client_count: usize) -> Self {
        Self::new_with_options(client_count, None, true)
    }

    pub(super) fn new_with_options(
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
            AvianNetworkPlugin,
            AuthoritativeMovementPlugin,
            ServerNetworkPlugin,
        ));
        server.finish();
        server.cleanup();
        let server_entity = spawn_crossbeam_server(server.world_mut(), &server_config);

        let mut harness = Self {
            server,
            server_entity,
            server_links: Vec::with_capacity(client_count),
            clients: Vec::with_capacity(client_count),
            client_entities: Vec::with_capacity(client_count),
            client_cues: Vec::with_capacity(client_count),
            now: Instant::now(),
        };
        for client_id in 1..=client_count as u64 {
            harness.add_client_with_options(
                client_id,
                if client_id == 1 {
                    client_protocol_id
                } else {
                    None
                },
                extra_protocol,
            );
        }
        harness
    }

    pub(super) fn add_client(&mut self, client_id: u64) {
        self.add_client_with_options(client_id, None, false);
    }

    pub(super) fn add_client_with_options(
        &mut self,
        client_id: u64,
        client_protocol_id: Option<u64>,
        extra_protocol: bool,
    ) {
        let mut config = ClientNetworkConfig::new(client_id);
        config.transport = NetworkTransport::Crossbeam;
        if let Some(protocol_id) = client_protocol_id {
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
            AvianNetworkPlugin,
        ));
        if extra_protocol {
            client.add_plugins(MismatchedProtocolPlugin);
        }
        client
            .insert_resource(CuePacketImpairment::default())
            .add_systems(
                PreUpdate,
                impair_cue_packets
                    .after(LinkSystems::Receive)
                    .before(TransportSystems::Receive)
                    .before(MessageSystems::Receive),
            );
        client.insert_resource(CaptureCombatCues::default());
        client.add_plugins(ClientNetworkPlugin);
        client.finish();
        client.cleanup();
        let (client_transport, server_transport) = lightyear::crossbeam::CrossbeamIo::new_pair();
        let config = client.world().resource::<ClientNetworkConfig>().clone();
        let client_entity = spawn_crossbeam_client(client.world_mut(), config, client_transport);
        let server_link = spawn_crossbeam_link(
            self.server.world_mut(),
            self.server_entity,
            server_transport,
        );
        self.clients.push(client);
        self.client_entities.push(client_entity);
        self.server_links.push(server_link);
        self.client_cues.push(Vec::new());
    }

    pub(super) fn step(&mut self) {
        self.now += SIMULATION_TICK;
        for index in 0..self.clients.len() {
            let client = &mut self.clients[index];
            client.insert_resource(TimeUpdateStrategy::ManualInstant(self.now));
            client.update();
            self.drain_client_cues(index);
        }
        self.server
            .insert_resource(TimeUpdateStrategy::ManualInstant(self.now));
        self.server.update();
    }

    pub(super) fn drain_client_cues(&mut self, index: usize) {
        let cues = {
            let world = self.clients[index].world_mut();
            let Some(mut capture) = world.get_resource_mut::<CaptureCombatCues>() else {
                return;
            };
            std::mem::take(&mut capture.cues)
        };
        if index >= self.client_cues.len() {
            self.client_cues.resize_with(index + 1, Vec::new);
        }
        self.client_cues[index].extend(cues);
    }

    pub(super) fn client_cues(&self, index: usize) -> &[CombatCue] {
        &self.client_cues[index]
    }

    pub(super) fn arm_cue_packet_impairment(&mut self, index: usize) {
        self.clients[index]
            .world_mut()
            .resource_mut::<CuePacketImpairment>()
            .armed = true;
    }

    pub(super) fn cue_packet_impairment(&self, index: usize) -> CuePacketImpairment {
        let impairment = self.clients[index]
            .world()
            .resource::<CuePacketImpairment>();
        CuePacketImpairment {
            armed: impairment.armed,
            injected: impairment.injected,
            duplicated_packets: impairment.duplicated_packets,
            reordered_batches: impairment.reordered_batches,
            held_packet: None,
        }
    }

    /// Advance only the authoritative server after a client stops producing input.
    /// This models a lost-input interval while keeping the fixed simulation running.
    pub(super) fn step_server_only(&mut self) {
        self.now += SIMULATION_TICK;
        self.server
            .insert_resource(TimeUpdateStrategy::ManualInstant(self.now));
        self.server.update();
    }

    pub(super) fn step_until(&mut self, mut condition: impl FnMut(&mut Self) -> bool) {
        for _ in 0..240 {
            self.step();
            if condition(self) {
                return;
            }
        }
        panic!("network harness condition did not become true");
    }

    pub(super) fn server_ids(&mut self) -> Vec<(PlayerId, NetworkEntityId)> {
        let mut query = self
            .server
            .world_mut()
            .query_filtered::<(&PlayerId, &NetworkEntityId), (With<PlaceholderPlayer>, Without<TestDummy>)>();
        let mut ids: Vec<_> = query
            .iter(self.server.world())
            .filter(|(player, _)| player.0 != 0)
            .map(|(player, entity)| (*player, *entity))
            .collect();
        ids.sort_by_key(|(player, entity)| (player.0, entity.0));
        ids
    }

    pub(super) fn client_ids(&mut self, index: usize) -> Vec<(PlayerId, NetworkEntityId)> {
        let world = self.clients[index].world_mut();
        let mut query = world
            .query_filtered::<(&PlayerId, &NetworkEntityId), (With<Remote>, Without<TestDummy>)>();
        let mut ids: Vec<_> = query
            .iter(world)
            .filter(|(player, _)| player.0 != 0)
            .map(|(player, entity)| (*player, *entity))
            .collect();
        ids.sort_by_key(|(player, entity)| (player.0, entity.0));
        ids
    }

    pub(super) fn client_is_active(&mut self, index: usize) -> bool {
        let world = self.clients[index].world_mut();
        let mut query = world.query::<&ClientJoinStatus>();
        query
            .iter(world)
            .any(|status| matches!(status.phase, ClientJoinPhase::Active { .. }))
    }

    pub(super) fn selection_is_complete(&mut self, index: usize) -> bool {
        let world = self.clients[index].world_mut();
        let mut query =
            world.query_filtered::<(), (With<Fighter>, With<Controlled>, With<SelectingWeapon>)>();
        query.iter(world).next().is_none()
    }

    pub(super) fn active_server_sessions(&mut self) -> usize {
        let world = self.server.world_mut();
        let mut query = world.query::<&ServerSession>();
        query
            .iter(world)
            .filter(|session| matches!(session.phase, ServerSessionPhase::Active { .. }))
            .count()
    }

    pub(super) fn set_controlled_input(&mut self, index: usize, input: FighterInput) {
        let mut pending = self.clients[index]
            .world_mut()
            .resource_mut::<PendingLocalActions>();
        pending.move_axis = input.move_axis.to_vec2();
        pending.aim_axis = input.aim_update.map(|axis| axis.to_vec2());
        pending.held_buttons = input.gameplay_buttons;
    }

    pub(super) fn controlled_player_id(&mut self, index: usize) -> PlayerId {
        let world = self.clients[index].world_mut();
        let mut query = world.query_filtered::<&PlayerId, (With<Fighter>, With<Controlled>)>();
        *query
            .iter(world)
            .next()
            .expect("active client should own a fighter")
    }

    pub(super) fn controlled_entity(&mut self, index: usize) -> Entity {
        let world = self.clients[index].world_mut();
        let mut query = world.query_filtered::<Entity, (With<Fighter>, With<Controlled>)>();
        query
            .iter(world)
            .next()
            .expect("active client should own a fighter")
    }

    pub(super) fn aim_at_dummy(&mut self, index: usize) -> Vec2 {
        let player = self.controlled_player_id(index);
        let world = self.server.world_mut();
        let mut fighters = world.query_filtered::<(&PlayerId, &Position), With<Fighter>>();
        let owner_position = fighters
            .iter(world)
            .find(|(candidate, _)| **candidate == player)
            .map(|(_, position)| position.0)
            .expect("controlled fighter position");
        let mut dummies = world.query_filtered::<&Position, With<TestDummy>>();
        let dummy_position = dummies.single(world).expect("dummy position").0;
        (dummy_position - owner_position).normalize_or_zero()
    }

    pub(super) fn remote_entity_for_player(&mut self, index: usize, player_id: PlayerId) -> Entity {
        let world = self.clients[index].world_mut();
        let mut query =
            world.query_filtered::<(Entity, &PlayerId), (With<Fighter>, With<Remote>)>();
        query
            .iter(world)
            .find(|(_, id)| **id == player_id)
            .map(|(entity, _)| entity)
            .expect("client should have the requested remote fighter")
    }

    pub(super) fn server_tick(&mut self) -> u32 {
        self.server
            .world()
            .resource::<lightyear::prelude::LocalTimeline>()
            .tick()
            .0
    }

    pub(super) fn server_simulation_tick(&mut self) -> u64 {
        self.server.world().resource::<SimulationTick>().0
    }

    pub(super) fn send_forged_input(
        &mut self,
        index: usize,
        target: lightyear::input::input_message::InputTarget,
        end_tick: u32,
        input: FighterInput,
    ) {
        let client_entity = self.client_entities[index];
        let world = self.clients[index].world_mut();
        let mut sender = world
            .get_mut::<MessageSender<TestNativeInputMessage>>(client_entity)
            .expect("client link should have native input sender");
        send_forged_native_input_for_test(&mut sender, target, end_tick, input);
    }

    pub(super) fn send_weapon_selection(&mut self, index: usize, request: WeaponSelectionRequest) {
        let client_entity = self.client_entities[index];
        let world = self.clients[index].world_mut();
        let mut sender = world
            .get_mut::<MessageSender<WeaponSelectionRequest>>(client_entity)
            .expect("client selection sender");
        sender.send::<SessionChannel>(request);
    }

    pub(super) fn server_positions(&mut self) -> Vec<(PlayerId, Position)> {
        let world = self.server.world_mut();
        let mut query =
            world.query_filtered::<(&PlayerId, &Position), (With<Fighter>, Without<TestDummy>)>();
        let mut positions: Vec<_> = query
            .iter(world)
            .filter(|(player, _)| player.0 != 0)
            .map(|(player, position)| (*player, *position))
            .collect();
        positions.sort_by_key(|(player, _)| player.0);
        positions
    }

    pub(super) fn server_static_arena_count(&mut self) -> usize {
        let world = self.server.world_mut();
        let mut walls = world.query_filtered::<Entity, With<ArenaWall>>();
        let mut markers = world.query_filtered::<Entity, With<SpawnMarker>>();
        walls.iter(world).count() + markers.iter(world).count()
    }

    pub(super) fn server_projectile_count(&mut self) -> usize {
        let world = self.server.world_mut();
        let mut query = world.query_filtered::<Entity, With<Projectile>>();
        query.iter(world).count()
    }

    pub(super) fn client_projectile_count(&mut self, index: usize) -> usize {
        let world = self.clients[index].world_mut();
        let mut query = world.query_filtered::<Entity, With<Projectile>>();
        query.iter(world).count()
    }

    pub(super) fn server_poses(&mut self) -> Vec<(PlayerId, Position, Rotation)> {
        let world = self.server.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &Position, &Rotation), (With<Fighter>, Without<TestDummy>)>();
        let mut poses: Vec<_> = query
            .iter(world)
            .filter(|(player, _, _)| player.0 != 0)
            .map(|(player, position, rotation)| (*player, *position, *rotation))
            .collect();
        poses.sort_by_key(|(player, _, _)| player.0);
        poses
    }

    pub(super) fn client_positions(&mut self, index: usize) -> Vec<(PlayerId, Position)> {
        let world = self.clients[index].world_mut();
        let mut query =
            world.query_filtered::<(&PlayerId, &Position), (With<Fighter>, With<Remote>)>();
        let mut positions: Vec<_> = query
            .iter(world)
            .filter(|(player, _)| player.0 != 0)
            .map(|(player, position)| (*player, *position))
            .collect();
        positions.sort_by_key(|(player, _)| player.0);
        positions
    }

    pub(super) fn client_fighter_combat_state(
        &mut self,
        index: usize,
        network_id: NetworkEntityId,
    ) -> (CurrentHealth, WeaponState, bool) {
        let world = self.clients[index].world_mut();
        let mut query = world.query_filtered::<(
            &NetworkEntityId,
            &CurrentHealth,
            &WeaponState,
            Option<&Defeated>,
        ), With<Fighter>>();
        query
            .iter(world)
            .find(|(candidate, _, _, _)| **candidate == network_id)
            .map(|(_, health, weapon, defeated)| (*health, *weapon, defeated.is_some()))
            .expect("client should have the requested combat fighter")
    }

    /// The in-memory harness advances Bevy time without wall-clock delay, so Lightyear's ping
    /// sampler does not synchronize its presentation timeline as it does in a live process.
    /// Move that timeline to the newest received pose sample before asserting interpolation.
    pub(super) fn sample_client_at_newest_position_history(&mut self, index: usize) {
        let newest_tick = {
            let world = self.clients[index].world_mut();
            let mut query = world.query_filtered::<(&PlayerId, &ConfirmedHistory<Position>), (With<Fighter>, Without<TestDummy>)>();
            query
                .iter(world)
                .filter(|(player, _)| player.0 != 0)
                .filter_map(|(_, history)| history.newest_present())
                .map(|(tick, _)| tick)
                .max()
                .expect("client fighter should have replicated position history")
        };
        let current_tick = self.clients[index]
            .world()
            .resource::<InterpolationTimeline>()
            .tick();
        let delta = newest_tick.0.saturating_sub(current_tick.0);
        self.clients[index]
            .world_mut()
            .resource_mut::<InterpolationTimeline>()
            .apply_duration(SIMULATION_TICK * delta, SIMULATION_TICK);
        self.clients[index].update();
    }

    pub(super) fn client_interpolated_fighters(&mut self, index: usize) -> usize {
        let world = self.clients[index].world_mut();
        let mut query = world.query_filtered::<(&PlayerId, Entity), (
            With<Fighter>,
            With<Remote>,
            With<Interpolated>,
            Without<TestDummy>,
        )>();
        query
            .iter(world)
            .filter(|(player, _)| player.0 != 0)
            .count()
    }
}
