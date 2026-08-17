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

/// Deterministic receive-delay line for impairment profiles: every inbound packet is
/// held for `delay` client ticks before delivery, modelling one-way latency without
/// wall-clock sleeps.
#[derive(Resource, Debug)]
pub(super) struct ReplicationDelayLine {
    pub(super) delay: usize,
    queue: std::collections::VecDeque<Vec<lightyear::link::RecvPayload>>,
}

impl Default for ReplicationDelayLine {
    fn default() -> Self {
        Self {
            delay: 0,
            queue: std::collections::VecDeque::new(),
        }
    }
}

fn delay_packets(
    mut delay_line: ResMut<ReplicationDelayLine>,
    mut links: Query<&mut Link, With<Client>>,
) {
    if delay_line.delay == 0 {
        return;
    }
    for mut link in &mut links {
        let inbound: Vec<_> = link.recv.drain().collect();
        if !inbound.is_empty() {
            delay_line.queue.push_back(inbound);
        }
    }
    if delay_line.queue.len() > delay_line.delay {
        if let Some(outbound) = delay_line.queue.pop_front() {
            if let Some(mut link) = links.iter_mut().next() {
                for packet in outbound {
                    link.recv.push_raw(packet);
                }
            }
        }
    }
}

#[derive(Resource, Debug, Default)]
pub(super) struct DeterministicPacketImpairment {
    pub(super) armed: bool,
    pub(super) injected: bool,
    pub(super) duplicated_packets: u32,
    pub(super) dropped_packets: u32,
    pub(super) delayed_packets: u32,
    pub(super) reordered_batches: u32,
    pub(super) held_packet: Option<lightyear::link::RecvPayload>,
}

fn impair_packets(
    mut impairment: ResMut<DeterministicPacketImpairment>,
    mut links: Query<&mut Link, With<Client>>,
) {
    if !impairment.armed || impairment.injected {
        return;
    }
    for mut link in &mut links {
        let mut packets: Vec<_> = link.recv.drain().collect();
        if impairment.held_packet.is_none() {
            impairment.held_packet = packets.pop();
            for packet in packets {
                link.recv.push_raw(packet);
            }
            continue;
        }
        if let Some(held_packet) = impairment.held_packet.take() {
            packets.push(held_packet);
            impairment.delayed_packets = impairment.delayed_packets.saturating_add(1);
        }
        if packets.len() < 2 {
            impairment.held_packet = packets.pop();
            for packet in packets {
                link.recv.push_raw(packet);
            }
            continue;
        }
        packets.reverse();
        packets.remove(0);
        impairment.dropped_packets = impairment.dropped_packets.saturating_add(1);
        let duplicate = packets[0].clone();
        packets.insert(1, duplicate);
        for packet in packets {
            link.recv.push_raw(packet);
        }
        impairment.injected = true;
        impairment.duplicated_packets = impairment.duplicated_packets.saturating_add(1);
        impairment.reordered_batches = impairment.reordered_batches.saturating_add(1);
        break;
    }
}

#[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
fn activate_legacy_test_fighters(
    mut commands: bevy::prelude::Commands,
    spawn_points: Res<SpawnPointCatalog>,
    fighters: Query<
        (Entity, &PlayerId, &TeamId, &SpawnAssignment),
        (With<Fighter>, Without<ActiveCombatant>),
    >,
) {
    for (entity, player, team, assignment) in &fighters {
        let ordinal = player.0.saturating_sub(1) / 2;
        let Some(spawn) = spawn_points.deterministic_point(team.0, ordinal) else {
            continue;
        };
        commands.entity(entity).insert((
            ActiveCombatant,
            SpawnState {
                position: spawn.position,
                facing: spawn.facing,
            },
            Position::from_xy(spawn.position.x, spawn.position.y),
            Rotation::radians(spawn.facing),
            SpawnAssignment {
                spawn_point_id: spawn.spawn_point_id,
                ..*assignment
            },
        ));
    }
}

#[derive(Resource)]
struct LegacySandboxActivation;

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

    pub(super) fn new_match(client_count: usize) -> Self {
        let mut harness = Self::new_mode_with_options(client_count, None, false, false);
        harness
            .server
            .world_mut()
            .remove_resource::<LegacySandboxActivation>();
        harness
    }

    pub(super) fn new_hot_zone_match(client_count: usize) -> Self {
        let mut harness = Self::new_mode_with_options(client_count, None, false, true);
        harness
            .server
            .world_mut()
            .remove_resource::<LegacySandboxActivation>();
        harness
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
        Self::new_mode_with_options(client_count, client_protocol_id, extra_protocol, false)
    }

    fn new_mode_with_options(
        client_count: usize,
        client_protocol_id: Option<u64>,
        extra_protocol: bool,
        hot_zone: bool,
    ) -> Self {
        let server_config = ServerNetworkConfig {
            transport: NetworkTransport::Crossbeam,
            handshake_timeout: std::time::Duration::from_millis(250),
            ..Default::default()
        };

        let mut server = App::new();
        server.insert_resource(TestDummyFixture {
            position: Vec2::new(0.0, -320.0),
            facing: 0.0,
        });
        server.insert_resource(LegacySandboxActivation);
        let lifecycle = if hot_zone {
            // Shortened verification deadlines with a 1v1 capacity so deterministic
            // two-client scenarios can activate without changing rule semantics.
            brawler::matchplay::MatchLifecycleRules {
                minimum_participants_per_team: 1,
                ..brawler::server::match_lifecycle_rules_for_profile(
                    brawler::config::MatchRulesProfile::ProcessVerification,
                )
            }
        } else {
            brawler::matchplay::MatchLifecycleRules::default()
        };
        server
            .insert_resource(server_config.clone())
            .insert_resource(lifecycle);
        if hot_zone {
            server
                .insert_resource(brawler::matchplay::hot_zone_setup_for_composition())
                .insert_resource(brawler::matchplay::hot_zone_rules_for_profile(
                    brawler::config::MatchRulesProfile::ProcessVerification,
                ))
                .insert_resource(brawler::map::ServerMapSelection {
                    preset_id: brawler::map::HOT_ZONE_MAP_PRESET,
                });
        } else {
            server.insert_resource(brawler::matchplay::WipeoutRules::default());
        }
        server.add_plugins((
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
            brawler::terrain::AuthoritativeTerrainPlugin,
        ));
        if hot_zone {
            server.add_plugins(brawler::matchplay::HotZoneModePlugin);
        } else {
            server.add_plugins(brawler::matchplay::WipeoutModePlugin);
        }
        server.add_systems(
            bevy::prelude::Update,
            activate_legacy_test_fighters
                .run_if(bevy::prelude::resource_exists::<LegacySandboxActivation>),
        );
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
        config.headless = true;
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
        #[cfg(feature = "owner-prediction")]
        client.add_plugins(brawler::client::prediction::OwnerPredictionPlugin);
        client
            .insert_resource(DeterministicPacketImpairment::default())
            .insert_resource(ReplicationDelayLine::default())
            .add_systems(
                PreUpdate,
                (
                    delay_packets
                        .after(LinkSystems::Receive)
                        .before(TransportSystems::Receive)
                        .before(MessageSystems::Receive),
                    impair_packets
                        .after(LinkSystems::Receive)
                        .before(TransportSystems::Receive)
                        .before(MessageSystems::Receive),
                ),
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

    pub(super) fn set_replication_delay(&mut self, index: usize, delay: usize) {
        self.clients[index]
            .world_mut()
            .resource_mut::<ReplicationDelayLine>()
            .delay = delay;
    }

    pub(super) fn arm_packet_impairment(&mut self, index: usize) {
        self.clients[index]
            .world_mut()
            .resource_mut::<DeterministicPacketImpairment>()
            .armed = true;
    }

    pub(super) fn packet_impairment(&self, index: usize) -> DeterministicPacketImpairment {
        let impairment = self.clients[index]
            .world()
            .resource::<DeterministicPacketImpairment>();
        DeterministicPacketImpairment {
            armed: impairment.armed,
            injected: impairment.injected,
            duplicated_packets: impairment.duplicated_packets,
            dropped_packets: impairment.dropped_packets,
            delayed_packets: impairment.delayed_packets,
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
            world.query_filtered::<(), (With<Fighter>, With<Controlled>, With<SelectingBuild>)>();
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
        pending.aim_axis = input
            .aim_update
            .map(brawler::protocol::QuantizedAxis2::to_vec2);
        pending.aim_distance = input
            .aim_distance
            .map(brawler::protocol::QuantizedAimDistance::to_world_units);
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

    pub(super) fn send_build_selection(&mut self, index: usize, request: BuildSelectionRequest) {
        let client_entity = self.client_entities[index];
        let world = self.clients[index].world_mut();
        let mut sender = world
            .get_mut::<MessageSender<BuildSelectionRequest>>(client_entity)
            .expect("client selection sender");
        sender.send::<SessionChannel>(request);
    }

    pub(super) fn send_match_command(&mut self, index: usize, request: MatchCommandRequest) {
        let client_entity = self.client_entities[index];
        let world = self.clients[index].world_mut();
        let mut sender = world
            .get_mut::<MessageSender<MatchCommandRequest>>(client_entity)
            .expect("client match command sender");
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
        walls.iter(world).count()
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

impl Harness {
    /// Inject one authoritative `DestroyTerrain` world-effect fact on the server.
    pub(super) fn inject_terrain_brush(&mut self, attack: u64, position: (f32, f32), radius: f32) {
        let world = self.server.world_mut();
        world
            .resource_mut::<brawler::combat::CombatWorldEffectFacts>()
            .0
            .push(terrain_brush_fact(attack, position, radius));
    }

    /// The authoritative root revision, or `None` before terrain installed.
    pub(super) fn server_terrain_revision(&mut self) -> Option<u64> {
        let world = self.server.world_mut();
        world
            .query::<&brawler::terrain::TerrainRoot>()
            .iter(world)
            .next()
            .map(|root| root.revision)
    }

    /// Digest of the authoritative current occupancy across allocated chunks.
    pub(super) fn server_terrain_digest(&mut self) -> u64 {
        let world = self.server.world_mut();
        let index = world.resource::<brawler::terrain::TerrainChunkIndex>();
        let mut chunks = std::collections::BTreeMap::new();
        for (chunk, entity) in &index.0 {
            if let Some(state) = world.get::<brawler::terrain::TerrainChunkState>(*entity) {
                chunks.insert(*chunk, state.current);
            }
        }
        brawler::terrain::grid::occupancy_digest(&chunks)
    }

    /// Digest of one client's converged occupancy plus its readiness/revision.
    pub(super) fn client_terrain(
        &mut self,
        index: usize,
    ) -> (brawler::terrain::ClientTerrainReadiness, u64, u64) {
        let world = self.clients[index].world();
        let convergence = world.resource::<brawler::terrain::ClientTerrainConvergence>();
        let readiness = world
            .resource::<brawler::terrain::ClientTerrainReadiness>()
            .clone();
        (
            readiness,
            convergence.revision(),
            brawler::terrain::grid::occupancy_digest(convergence.chunks()),
        )
    }

    /// Send a forged recovery request that bypasses the client state machine.
    pub(super) fn send_forged_terrain_request(
        &mut self,
        index: usize,
        generation: brawler::terrain::TerrainGeneration,
    ) {
        let client_entity = self.client_entities[index];
        let world = self.clients[index].world_mut();
        let mut sender = world
            .get_mut::<MessageSender<brawler::terrain::TerrainRecoveryRequest>>(client_entity)
            .expect("client link should have a terrain recovery sender");
        sender.send::<brawler::protocol::TerrainChannel>(
            brawler::terrain::TerrainRecoveryRequest { generation },
        );
    }

    /// Current authoritative terrain telemetry aggregates for forgery assertions.
    pub(super) fn server_terrain_aggregates(
        &mut self,
    ) -> brawler::terrain::telemetry::TerrainTelemetryAggregates {
        self.server
            .world()
            .resource::<brawler::terrain::telemetry::TerrainTelemetry>()
            .aggregates
            .clone()
    }
}

/// One deterministic Arc-landing-like world fact for terrain injection.
pub(super) fn terrain_brush_fact(
    attack: u64,
    position: (f32, f32),
    radius: f32,
) -> brawler::combat::CombatWorldEffectFact {
    brawler::combat::CombatWorldEffectFact {
        tick: 0,
        source: AttackSource {
            kind: CombatSourceKind::PrimaryWeapon,
            attack_id: AttackId(attack),
            player_id: PlayerId(1),
            owner_network_entity_id: NetworkEntityId(1),
            team_id: TeamId(0),
            recipe_fingerprint: WeaponRecipeFingerprint::default(),
            presentation_profile_id: brawler::combat::WeaponPresentationProfileId(3),
            legacy_compatibility: false,
            source_preset_id: None,
            origin: WorldPoint { x: 0.0, y: 0.0 },
            facing: 0.0,
        },
        delivery_index: 0,
        effect_index: 0,
        position: WorldPoint {
            x: position.0,
            y: position.1,
        },
        effect: brawler::combat::WorldEffectDefinition::DestroyTerrain { radius },
    }
}
