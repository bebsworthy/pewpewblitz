use avian2d::prelude::{Collider, CollisionLayers, Position, Rotation};
use bevy::{
    app::App,
    platform::time::Instant,
    prelude::{
        Entity, IntoScheduleConfigs, MinimalPlugins, PreUpdate, Query, ResMut, Resource, Vec2,
        With, Without,
    },
    state::app::StatesPlugin,
    time::TimeUpdateStrategy,
};
use brawler::{
    client::{
        ClientJoinPhase, ClientJoinStatus, ClientNetworkPlugin, PendingLocalActions,
        spawn_crossbeam_client,
    },
    combat::{
        ActiveEffects, AttackDelivery, AttackId, AttackSource, CaptureCombatCues, CombatCue,
        CombatEventId, CombatLogRecord, CombatTelemetry, ComposedProjectileRuntime, CurrentHealth,
        DUMMY_NETWORK_ENTITY, Defeated, FighterDefinitions, Projectile, ProjectileDeadline,
        ReplicatedAttackSource, ResolvedWeapon, SelectedBuild, SelectingWeapon, SpawnState, TeamId,
        TestDummy, WeaponPhase, WeaponPresetId, WeaponRecipeFingerprint, WeaponState,
        WeaponTelemetry, WorldPoint,
    },
    config::{ClientNetworkConfig, NetworkTransport, ServerNetworkConfig},
    gameplay::GameplayPlugin,
    movement::{
        ArenaWall, AuthoritativeMovementPlugin, AvianNetworkPlugin, GreyboxArenaDefinition,
        InputTuning, InputValidationState, MovementTuning, SpawnMarker,
    },
    protocol::{
        Fighter, FighterInput, NetworkEntityId, PlaceholderPlayer, PlayerId, ProtocolPlugin,
        SessionChannel, TestNativeInputMessage, WeaponSelectionDecision, WeaponSelectionRequest,
        send_forged_native_input_for_test,
    },
    server::{
        ServerNetworkPlugin, ServerSession, ServerSessionPhase, spawn_crossbeam_link,
        spawn_crossbeam_server,
    },
    timing::{SIMULATION_TICK, SimulationTick},
};
use lightyear::prelude::client::{Client, Connected, Disconnect, Disconnected, Remote};
use lightyear::prelude::server::{NetcodeServer, ServerPlugins, Stopped};
use lightyear::prelude::{
    AppMessageExt, ConfirmedHistory, Controlled, Interpolated, InterpolationTimeline, Link,
    LinkSystems, LocalAddr, MessageSender, MessageSystems, NetworkDirection, NetworkTimeline,
    Predicted,
};
use lightyear::transport::plugin::TransportSystems;
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

#[test]
fn milestone_five_selection_resolves_distinct_presets_and_spawns_spread_deliveries() {
    let mut harness = Harness::new(2);
    harness.clients[0]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(2);
    harness.clients[1]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(4);

    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
    });
    for _ in 0..60 {
        harness.step();
    }
    harness.step_until(|harness| {
        (0..2).all(|index| {
            let world = harness.clients[index].world_mut();
            let mut query = world
                .query_filtered::<(), (With<Fighter>, With<Controlled>, With<SelectingWeapon>)>();
            query.iter(world).next().is_none()
        })
    });

    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<(&PlayerId, &SelectedBuild, &ResolvedWeapon, &WeaponState), With<Fighter>>();
    let mut selections: Vec<_> = query
        .iter(world)
        .filter(|(player, _, _, _)| player.0 != 0)
        .map(|(player, build, resolved, state)| (player.0, build, resolved, state))
        .collect();
    selections.sort_by_key(|(_, build, _, _)| build.source_preset_id);
    assert_eq!(selections.len(), 2);
    assert_eq!(selections[0].1.source_preset_id, Some(WeaponPresetId(2)));
    assert_eq!(selections[1].1.source_preset_id, Some(WeaponPresetId(4)));
    assert_eq!(selections[0].2.recipe.economy.capacity(), 4);
    assert_eq!(selections[1].2.recipe.economy.capacity(), 3);
    assert_eq!(selections[0].3.ammo, 4);
    assert_eq!(selections[1].3.ammo, 3);

    for index in 0..2 {
        harness.set_controlled_input(
            index,
            FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
        );
    }
    for _ in 0..3 {
        harness.step();
    }
    let world = harness.server.world_mut();
    let mut deliveries = world.query_filtered::<&AttackDelivery, With<Projectile>>();
    assert_eq!(deliveries.iter(world).count(), 7);
}

#[test]
fn selection_channel_is_connection_scoped_idempotent_and_strictly_ordered() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| harness.client_is_active(0) && harness.selection_is_complete(0));
    let accepted = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_response)
        .filter(|outcome| outcome.decision == WeaponSelectionDecision::Accepted)
        .expect("automatic accepted selection outcome");
    let accepted_request = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_request)
        .expect("accepted selection request");
    harness.send_weapon_selection(0, accepted_request);
    harness.send_weapon_selection(0, accepted_request);
    harness.step();
    let selected = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&SelectedBuild, &ResolvedWeapon), With<Fighter>>();
        query
            .iter(world)
            .find(|(build, _)| build.source_preset_id == accepted.accepted_preset_id)
            .map(|(build, resolved)| (*build, resolved.recipe_fingerprint))
            .expect("accepted selection")
    };
    let duplicate = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_response)
        .expect("duplicate outcome");
    assert_eq!(duplicate, accepted);
    assert_eq!(selected.0.source_preset_id, accepted.accepted_preset_id);
    assert_eq!(
        harness
            .server
            .world()
            .resource::<WeaponTelemetry>()
            .selection_records
            .len(),
        1
    );

    harness.send_weapon_selection(
        0,
        WeaponSelectionRequest {
            request_id: accepted.request_id.saturating_sub(1),
            preset_id: WeaponPresetId(4),
        },
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .get::<ServerSession>(harness.server_links[0])
            .and_then(|session| session.last_selection_response)
            .is_some_and(|outcome| outcome.decision == WeaponSelectionDecision::StaleRequest)
    });
    let stale = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_response)
        .expect("stale outcome");
    assert_eq!(stale.decision, WeaponSelectionDecision::StaleRequest);

    harness.send_weapon_selection(
        0,
        WeaponSelectionRequest {
            request_id: accepted.request_id.saturating_add(1),
            preset_id: WeaponPresetId(4),
        },
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .get::<ServerSession>(harness.server_links[0])
            .and_then(|session| session.last_selection_response)
            .is_some_and(|outcome| outcome.decision == WeaponSelectionDecision::NotSelecting)
    });
    let not_selecting = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_response)
        .expect("not-selecting outcome");
    assert_eq!(
        not_selecting.decision,
        WeaponSelectionDecision::NotSelecting
    );
    let final_build = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&SelectedBuild, With<Fighter>>();
        query
            .iter(world)
            .find(|build| build.source_preset_id == accepted.accepted_preset_id)
            .copied()
            .expect("selection cannot be switched")
    };
    assert_eq!(final_build, selected.0);

    // Re-entering selection here isolates the registered request path from the automatic
    // first-selection helper and proves that an unknown preset cannot mutate the accepted build.
    let fighter_entity = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<Entity, (With<Fighter>, Without<TestDummy>)>();
        query.iter(world).next().expect("player fighter")
    };
    harness
        .server
        .world_mut()
        .entity_mut(fighter_entity)
        .insert(SelectingWeapon);
    harness.send_weapon_selection(
        0,
        WeaponSelectionRequest {
            request_id: accepted.request_id.saturating_add(2),
            preset_id: WeaponPresetId(999),
        },
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .get::<ServerSession>(harness.server_links[0])
            .and_then(|session| session.last_selection_response)
            .is_some_and(|outcome| outcome.decision == WeaponSelectionDecision::UnknownPreset)
    });
    let unknown = harness
        .server
        .world()
        .get::<ServerSession>(harness.server_links[0])
        .and_then(|session| session.last_selection_response)
        .expect("unknown-preset outcome");
    assert_eq!(unknown.decision, WeaponSelectionDecision::UnknownPreset);
    let unchanged_build = harness
        .server
        .world()
        .get::<SelectedBuild>(fighter_entity)
        .copied()
        .expect("accepted build remains authoritative");
    assert_eq!(unchanged_build, final_build);
}

#[test]
fn launcher_replication_preserves_flight_deadline_and_durable_slow_state() {
    let mut harness = Harness::new(1);
    harness.clients[0]
        .world_mut()
        .resource_mut::<ClientNetworkConfig>()
        .weapon_preset = Some(3);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.selection_is_complete(0)
    });
    harness.set_controlled_input(0, FighterInput::default());
    harness.step();
    let aim = harness.aim_at_dummy(0);
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(aim), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| harness.server_projectile_count() > 0);
    let (server_deadline, server_flight) = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&ProjectileDeadline, &brawler::combat::LobbedFlight), With<Projectile>>();
        let (deadline, flight) = query.iter(world).next().expect("server lobbed delivery");
        (*deadline, *flight)
    };
    assert_eq!(server_deadline.expires_at_tick, server_flight.lands_at_tick);
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&ActiveEffects, With<TestDummy>>();
        query.iter(world).any(|effects| effects.slow.is_some())
    });
    let telemetry = harness.server.world().resource::<WeaponTelemetry>();
    assert!(
        telemetry
            .hostile_damage_events
            .get(&WeaponPresetId(3))
            .copied()
            .unwrap_or(0)
            > 0
    );
    harness.step_until(|harness| {
        let world = harness.clients[0].world_mut();
        let mut query = world.query_filtered::<(&NetworkEntityId, &ActiveEffects), With<Fighter>>();
        query.iter(world).any(|(network_id, effects)| {
            *network_id == DUMMY_NETWORK_ENTITY && effects.slow.is_some()
        })
    });
}

#[derive(Resource, Debug, Default)]
struct CuePacketImpairment {
    armed: bool,
    injected: bool,
    duplicated_packets: u32,
    reordered_batches: u32,
    held_packet: Option<lightyear::link::RecvPayload>,
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

struct Harness {
    server: App,
    server_entity: Entity,
    server_links: Vec<Entity>,
    clients: Vec<App>,
    client_entities: Vec<Entity>,
    client_cues: Vec<Vec<CombatCue>>,
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

    fn add_client(&mut self, client_id: u64) {
        self.add_client_with_options(client_id, None, false);
    }

    fn add_client_with_options(
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

    fn step(&mut self) {
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

    fn drain_client_cues(&mut self, index: usize) {
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

    fn client_cues(&self, index: usize) -> &[CombatCue] {
        &self.client_cues[index]
    }

    fn arm_cue_packet_impairment(&mut self, index: usize) {
        self.clients[index]
            .world_mut()
            .resource_mut::<CuePacketImpairment>()
            .armed = true;
    }

    fn cue_packet_impairment(&self, index: usize) -> CuePacketImpairment {
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
    fn step_server_only(&mut self) {
        self.now += SIMULATION_TICK;
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
            .query_filtered::<(&PlayerId, &NetworkEntityId), (With<PlaceholderPlayer>, Without<TestDummy>)>();
        let mut ids: Vec<_> = query
            .iter(self.server.world())
            .filter(|(player, _)| player.0 != 0)
            .map(|(player, entity)| (*player, *entity))
            .collect();
        ids.sort_by_key(|(player, entity)| (player.0, entity.0));
        ids
    }

    fn client_ids(&mut self, index: usize) -> Vec<(PlayerId, NetworkEntityId)> {
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

    fn client_is_active(&mut self, index: usize) -> bool {
        let world = self.clients[index].world_mut();
        let mut query = world.query::<&ClientJoinStatus>();
        query
            .iter(world)
            .any(|status| matches!(status.phase, ClientJoinPhase::Active { .. }))
    }

    fn selection_is_complete(&mut self, index: usize) -> bool {
        let world = self.clients[index].world_mut();
        let mut query =
            world.query_filtered::<(), (With<Fighter>, With<Controlled>, With<SelectingWeapon>)>();
        query.iter(world).next().is_none()
    }

    fn active_server_sessions(&mut self) -> usize {
        let world = self.server.world_mut();
        let mut query = world.query::<&ServerSession>();
        query
            .iter(world)
            .filter(|session| matches!(session.phase, ServerSessionPhase::Active { .. }))
            .count()
    }

    fn set_controlled_input(&mut self, index: usize, input: FighterInput) {
        let mut pending = self.clients[index]
            .world_mut()
            .resource_mut::<PendingLocalActions>();
        pending.move_axis = input.move_axis.to_vec2();
        pending.aim_axis = input.aim_update.map(|axis| axis.to_vec2());
        pending.held_buttons = input.gameplay_buttons;
    }

    fn controlled_player_id(&mut self, index: usize) -> PlayerId {
        let world = self.clients[index].world_mut();
        let mut query = world.query_filtered::<&PlayerId, (With<Fighter>, With<Controlled>)>();
        *query
            .iter(world)
            .next()
            .expect("active client should own a fighter")
    }

    fn controlled_entity(&mut self, index: usize) -> Entity {
        let world = self.clients[index].world_mut();
        let mut query = world.query_filtered::<Entity, (With<Fighter>, With<Controlled>)>();
        query
            .iter(world)
            .next()
            .expect("active client should own a fighter")
    }

    fn aim_at_dummy(&mut self, index: usize) -> Vec2 {
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

    fn remote_entity_for_player(&mut self, index: usize, player_id: PlayerId) -> Entity {
        let world = self.clients[index].world_mut();
        let mut query =
            world.query_filtered::<(Entity, &PlayerId), (With<Fighter>, With<Remote>)>();
        query
            .iter(world)
            .find(|(_, id)| **id == player_id)
            .map(|(entity, _)| entity)
            .expect("client should have the requested remote fighter")
    }

    fn server_tick(&mut self) -> u32 {
        self.server
            .world()
            .resource::<lightyear::prelude::LocalTimeline>()
            .tick()
            .0
    }

    fn server_simulation_tick(&mut self) -> u64 {
        self.server.world().resource::<SimulationTick>().0
    }

    fn send_forged_input(
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

    fn send_weapon_selection(&mut self, index: usize, request: WeaponSelectionRequest) {
        let client_entity = self.client_entities[index];
        let world = self.clients[index].world_mut();
        let mut sender = world
            .get_mut::<MessageSender<WeaponSelectionRequest>>(client_entity)
            .expect("client selection sender");
        sender.send::<SessionChannel>(request);
    }

    fn server_positions(&mut self) -> Vec<(PlayerId, Position)> {
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

    fn server_static_arena_count(&mut self) -> usize {
        let world = self.server.world_mut();
        let mut walls = world.query_filtered::<Entity, With<ArenaWall>>();
        let mut markers = world.query_filtered::<Entity, With<SpawnMarker>>();
        walls.iter(world).count() + markers.iter(world).count()
    }

    fn server_projectile_count(&mut self) -> usize {
        let world = self.server.world_mut();
        let mut query = world.query_filtered::<Entity, With<Projectile>>();
        query.iter(world).count()
    }

    fn client_projectile_count(&mut self, index: usize) -> usize {
        let world = self.clients[index].world_mut();
        let mut query = world.query_filtered::<Entity, With<Projectile>>();
        query.iter(world).count()
    }

    fn server_poses(&mut self) -> Vec<(PlayerId, Position, Rotation)> {
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

    fn client_positions(&mut self, index: usize) -> Vec<(PlayerId, Position)> {
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

    fn client_fighter_combat_state(
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
    fn sample_client_at_newest_position_history(&mut self, index: usize) {
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

    fn client_interpolated_fighters(&mut self, index: usize) -> usize {
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

#[test]
fn two_clients_connect_and_receive_the_same_server_owned_roster() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });
    harness.add_client(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
            && harness.client_ids(0).len() == 2
            && harness.client_ids(1).len() == 2
            && harness.selection_is_complete(0)
            && harness.selection_is_complete(1)
    });

    let server_ids = harness.server_ids();
    assert_eq!(harness.client_ids(0), server_ids);
    assert_eq!(harness.client_ids(1), server_ids);
    assert_eq!(harness.active_server_sessions(), 2);

    let mut query = harness.server.world_mut().query_filtered::<(
        &lightyear::prelude::Replicate,
        &lightyear::prelude::ControlledBy,
    ), (With<PlaceholderPlayer>, Without<TestDummy>)>(
    );
    assert_eq!(query.iter(harness.server.world()).count(), 2);
}

#[test]
fn lost_input_repeats_briefly_then_neutralizes_without_server_pause() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
            && harness.selection_is_complete(0)
    });

    harness.set_controlled_input(0, FighterInput::from_axes(Vec2::X, None, 0));
    for _ in 0..36 {
        harness.step();
    }
    let moving_position = harness.server_positions()[0].1.0;

    // Native input redundancy can leave a few already-received states in the
    // authoritative buffer; the server must drain those before neutralizing.
    for _ in 0..24 {
        harness.step_server_only();
    }
    let neutralized_position = harness.server_positions()[0].1.0;
    for _ in 0..4 {
        harness.step_server_only();
    }
    let settled_position = harness.server_positions()[0].1.0;

    assert!(
        neutralized_position.x > moving_position.x,
        "lost input did not advance before neutralization: moving={moving_position:?} neutralized={neutralized_position:?}"
    );
    assert!(
        settled_position.distance(neutralized_position) < 0.001,
        "server kept moving after neutralization: neutralized={neutralized_position:?} settled={settled_position:?}"
    );
}

#[test]
fn authoritative_pulse_hits_dummy_and_sandbox_reset_restores_durable_state() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
            && harness.client_ids(0).len() == 2
            && harness.client_ids(1).len() == 2
            && harness.selection_is_complete(0)
            && harness.selection_is_complete(1)
    });
    let dummy_aim = harness.aim_at_dummy(0);
    let (dummy_entity, dummy_spawn, dummy_initial_rotation, dummy_initial_layers) = {
        let world = harness.server.world_mut();
        let mut query = world
            .query_filtered::<(Entity, &SpawnState, &Rotation, &CollisionLayers), With<TestDummy>>(
            );
        let (entity, spawn, rotation, layers) = query.single(world).expect("dummy spawn state");
        (entity, *spawn, *rotation, *layers)
    };

    // Cross the server's strictly-newer-input activation barrier before the first fire intent.
    harness.set_controlled_input(0, FighterInput::default());
    harness.step();
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(dummy_aim), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .accepted_shots
            >= 1
            && harness.server_projectile_count() > 0
    });
    let first_tick_projectile_travelled = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&ComposedProjectileRuntime, With<Projectile>>();
        query
            .iter(world)
            .next()
            .expect("first-tick projectile")
            .travelled
    };
    assert!(first_tick_projectile_travelled > 0.0);
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&Defeated, With<TestDummy>>();
        query.iter(world).next().is_some()
    });

    harness.step_until(|harness| {
        [0, 1].into_iter().all(|index| {
            let (health, _, defeated) =
                harness.client_fighter_combat_state(index, DUMMY_NETWORK_ENTITY);
            health.0 == 0 && defeated
        })
    });

    let defeated_health = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
        query.single(world).expect("dummy health").0
    };
    assert_eq!(defeated_health, 0);
    let (accepted_shots, applied_damage, defeats) = {
        let telemetry = harness.server.world().resource::<CombatTelemetry>();
        (
            telemetry.accepted_shots,
            telemetry.applied_damage,
            telemetry.defeats,
        )
    };
    assert!(accepted_shots >= 4);
    assert_eq!(applied_damage, 100);
    assert_eq!(defeats, 1);

    let reset_at_tick = {
        let world = harness.server.world_mut();
        let mut defeated = world.query_filtered::<&Defeated, With<TestDummy>>();
        defeated.single(world).expect("dummy defeat").reset_at_tick
    };
    // Disturb every durable pose/state field after defeat so reset verification cannot pass by
    // observing the unchanged spawn state. The authored SpawnState and original collision layers
    // remain untouched and are the expected restoration values.
    {
        let world = harness.server.world_mut();
        world.entity_mut(dummy_entity).insert((
            Position::from_xy(
                dummy_spawn.position.x + 137.0,
                dummy_spawn.position.y + 71.0,
            ),
            Rotation::radians(dummy_spawn.facing + 0.75),
            CurrentHealth(1),
            WeaponState {
                ammo: 0,
                phase: WeaponPhase::Reloading {
                    ready_at_tick: reset_at_tick.saturating_add(10),
                },
            },
        ));
    }
    while harness.server_simulation_tick() < reset_at_tick {
        harness.step();
    }
    assert_eq!(
        harness.server_simulation_tick(),
        reset_at_tick,
        "reset deadline must be evaluated in the authoritative SimulationTick"
    );
    let still_defeated = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<Entity, With<TestDummy>>();
        query
            .iter(world)
            .all(|entity| world.get::<Defeated>(entity).is_some())
    };
    assert!(still_defeated);
    harness.step();
    let (reset_tick, reset_position) = {
        let world = harness.server.world_mut();
        let telemetry = world.resource::<CombatTelemetry>();
        telemetry
            .records
            .iter()
            .rev()
            .find_map(|record| match record {
                CombatLogRecord::Reset {
                    tick,
                    target,
                    position,
                    ..
                } if *target == DUMMY_NETWORK_ENTITY => Some((*tick, *position)),
                _ => None,
            })
            .expect("authoritative reset record")
    };
    assert_eq!(reset_tick, reset_at_tick);
    assert_eq!(reset_position, WorldPoint::from(dummy_spawn.position));
    let reset_state = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(
            &Position,
            &Rotation,
            &CurrentHealth,
            &WeaponState,
            &CollisionLayers,
            Option<&Defeated>,
        ), With<TestDummy>>();
        let (position, rotation, health, weapon, layers, defeated) =
            query.single(world).expect("reset dummy state");
        (
            *position,
            *rotation,
            *health,
            *weapon,
            *layers,
            defeated.is_some(),
        )
    };
    assert_eq!(
        reset_state.0,
        Position::from_xy(dummy_spawn.position.x, dummy_spawn.position.y)
    );
    assert!((reset_state.1.as_radians() - dummy_initial_rotation.as_radians()).abs() < 0.001);
    assert_eq!(reset_state.2, CurrentHealth(100));
    assert_eq!(reset_state.3.ammo, 6);
    assert!(matches!(reset_state.3.phase, WeaponPhase::Ready));
    assert_eq!(reset_state.4, dummy_initial_layers);
    assert!(!reset_state.5);
    harness.step_until(|harness| {
        [0, 1].into_iter().all(|index| {
            let (health, weapon, defeated) =
                harness.client_fighter_combat_state(index, DUMMY_NETWORK_ENTITY);
            health.0 == 100
                && weapon.ammo == 6
                && matches!(weapon.phase, WeaponPhase::Ready)
                && !defeated
        })
    });
    let reset_health = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
        query.single(world).expect("reset dummy health").0
    };
    assert_eq!(reset_health, 100);
    assert!(
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .records
            .iter()
            .any(|record| matches!(record, CombatLogRecord::Reset { .. }))
    );
    let expected_cue_stream = harness
        .server
        .world()
        .resource::<CombatTelemetry>()
        .cues
        .clone();
    let expected_muzzle_count = expected_cue_stream
        .iter()
        .filter(|cue| matches!(cue, CombatCue::Muzzle { .. }))
        .count() as u64;
    let current_accepted_shots = harness
        .server
        .world()
        .resource::<CombatTelemetry>()
        .accepted_shots;
    assert_eq!(expected_muzzle_count, current_accepted_shots);
    assert_eq!(harness.client_cues(0), expected_cue_stream.as_slice());
    assert_eq!(harness.client_cues(1), expected_cue_stream.as_slice());

    harness.set_controlled_input(0, FighterInput::default());
    for _ in 0..20 {
        harness.step();
    }
    let defeats_before_repeats = harness.server.world().resource::<CombatTelemetry>().defeats;
    for repeat in 0..2 {
        harness.set_controlled_input(
            0,
            FighterInput::from_axes(Vec2::ZERO, Some(dummy_aim), FighterInput::PRIMARY_FIRE),
        );
        let mut saw_defeat = false;
        for _ in 0..240 {
            harness.step();
            let world = harness.server.world_mut();
            let mut query = world.query_filtered::<Entity, With<Defeated>>();
            if query
                .iter(world)
                .any(|entity| world.get::<TestDummy>(entity).is_some())
            {
                saw_defeat = true;
                break;
            }
        }
        assert!(
            saw_defeat,
            "repeat {repeat} did not defeat the dummy; telemetry={:?}",
            harness.server.world().resource::<CombatTelemetry>()
        );
        harness.step_until(|harness| {
            let world = harness.server.world_mut();
            let mut query = world.query_filtered::<Entity, With<TestDummy>>();
            let dummy = query.single(world).expect("dummy");
            world.get::<Defeated>(dummy).is_none()
        });
        harness.set_controlled_input(0, FighterInput::default());
        harness.step_until(|harness| {
            [0, 1].into_iter().all(|index| {
                let (health, weapon, defeated) =
                    harness.client_fighter_combat_state(index, DUMMY_NETWORK_ENTITY);
                health.0 == 100
                    && weapon.ammo == 6
                    && matches!(weapon.phase, WeaponPhase::Ready)
                    && !defeated
            })
        });
    }
    assert!(
        harness.server.world().resource::<CombatTelemetry>().defeats >= defeats_before_repeats + 2
    );
}

#[test]
fn newly_spawned_projectile_can_hit_the_target_in_its_first_fixed_tick() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });

    let (source_entity, dummy_entity) = {
        let world = harness.server.world_mut();
        let mut source_query =
            world.query_filtered::<Entity, (With<Fighter>, Without<TestDummy>)>();
        let source_entity = source_query.iter(world).next().expect("source fighter");
        let mut dummy_query = world.query_filtered::<Entity, With<TestDummy>>();
        let dummy_entity = dummy_query.iter(world).next().expect("dummy fighter");
        (source_entity, dummy_entity)
    };
    {
        let world = harness.server.world_mut();
        // The muzzle starts at x=-66 and advances 15 units during the fixed sweep. Placing the
        // target at x=-35 leaves it outside the initial overlap but inside that first sweep.
        world
            .entity_mut(source_entity)
            .insert((Position::from_xy(-100.0, -300.0), Rotation::IDENTITY));
        world
            .entity_mut(dummy_entity)
            .insert(Position::from_xy(-35.0, -300.0));
    }
    let records_before = harness
        .server
        .world()
        .resource::<CombatTelemetry>()
        .records
        .len();
    let dummy_aim = harness.aim_at_dummy(0);
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(dummy_aim), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .records
            .iter()
            .skip(records_before)
            .any(|record| {
                matches!(
                    record,
                    CombatLogRecord::Damage {
                        target: DUMMY_NETWORK_ENTITY,
                        applied: 25,
                        ..
                    }
                )
            })
    });

    let records = &harness.server.world().resource::<CombatTelemetry>().records;
    let shot_tick = records
        .iter()
        .skip(records_before)
        .find_map(|record| match record {
            CombatLogRecord::Shot { tick, .. } => Some(*tick),
            _ => None,
        });
    let impact_tick = records
        .iter()
        .skip(records_before)
        .find_map(|record| match record {
            CombatLogRecord::Hit {
                tick,
                target: Some(target),
                ..
            } if *target == DUMMY_NETWORK_ENTITY => Some(*tick),
            _ => None,
        });
    let damage_tick = records
        .iter()
        .skip(records_before)
        .find_map(|record| match record {
            CombatLogRecord::Damage {
                tick,
                target: DUMMY_NETWORK_ENTITY,
                ..
            } => Some(*tick),
            _ => None,
        });
    let (shot_tick, impact_tick, damage_tick) = (
        shot_tick.expect("first-tick shot record"),
        impact_tick.expect("first-tick impact record"),
        damage_tick.expect("first-tick damage record"),
    );
    assert_eq!(shot_tick, impact_tick);
    assert_eq!(impact_tick, damage_tick);
    assert_eq!(
        harness.server_simulation_tick(),
        damage_tick.saturating_add(1),
        "the damage was emitted before the fixed tick advanced"
    );
    assert_eq!(harness.server_projectile_count(), 0);
    let dummy_health = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
        query.single(world).expect("dummy health").0
    };
    assert_eq!(dummy_health, 75);
}

#[test]
fn fixed_schedule_reload_completion_refills_and_fires_on_the_ready_tick() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });
    let player_id = harness.controlled_player_id(0);
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &WeaponState), With<Fighter>>();
        query.iter(world).any(|(candidate, state)| {
            *candidate == player_id
                && state.ammo == 0
                && matches!(state.phase, WeaponPhase::Reloading { .. })
        })
    });
    let reload_at_tick = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &WeaponState), With<Fighter>>();
        query
            .iter(world)
            .find(|(candidate, _)| **candidate == player_id)
            .and_then(|(_, state)| match state.phase {
                WeaponPhase::Reloading { ready_at_tick } => Some(ready_at_tick),
                _ => None,
            })
            .expect("reload deadline")
    };
    let shot_count_before_reload = harness
        .server
        .world()
        .resource::<CombatTelemetry>()
        .records
        .iter()
        .filter(|record| matches!(record, CombatLogRecord::Shot { .. }))
        .count();
    let shots_before_reload = harness
        .server
        .world()
        .resource::<CombatTelemetry>()
        .accepted_shots;
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .accepted_shots
            > shots_before_reload
    });
    let (shots_after_reload, state_after_reload, reload_shot_tick) = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &WeaponState), With<Fighter>>();
        let state = query
            .iter(world)
            .find(|(candidate, _)| **candidate == player_id)
            .map(|(_, state)| *state)
            .expect("fighter weapon");
        let reload_shot_tick = world
            .resource::<CombatTelemetry>()
            .records
            .iter()
            .filter_map(|record| match record {
                CombatLogRecord::Shot { tick, .. } => Some(*tick),
                _ => None,
            })
            .nth(shot_count_before_reload)
            .expect("shot record after reload");
        (
            world.resource::<CombatTelemetry>().accepted_shots,
            state,
            reload_shot_tick,
        )
    };
    assert_eq!(shots_after_reload, shots_before_reload + 1);
    assert_eq!(reload_shot_tick, reload_at_tick);
    assert_eq!(
        harness.server_simulation_tick(),
        reload_shot_tick.saturating_add(1)
    );
    assert!(matches!(
        state_after_reload.phase,
        WeaponPhase::Cooldown { .. } | WeaponPhase::Reloading { .. }
    ));
}

#[test]
fn reciprocal_lethal_hits_defeat_both_fighters_with_stable_attribution() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
            && harness.client_ids(0).len() == 2
            && harness.client_ids(1).len() == 2
    });

    {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(
            &PlayerId,
            &mut Position,
            &mut Rotation,
            &mut CurrentHealth,
        ), With<Fighter>>();
        for (player, mut position, mut rotation, mut health) in query.iter_mut(world) {
            if player.0 == 0 {
                continue;
            }
            let right_side = player.0 % 2 == 0;
            position.0 = Vec2::new(if right_side { 100.0 } else { -100.0 }, 0.0);
            *rotation = Rotation::radians(if right_side {
                std::f32::consts::PI
            } else {
                0.0
            });
            health.0 = 25;
        }
    }

    for index in 0..2 {
        let right_side = harness.controlled_player_id(index).0 % 2 == 0;
        harness.set_controlled_input(
            index,
            FighterInput::from_axes(
                Vec2::ZERO,
                Some(if right_side { -Vec2::X } else { Vec2::X }),
                FighterInput::PRIMARY_FIRE,
            ),
        );
    }
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&NetworkEntityId, (With<Fighter>, With<Defeated>)>();
        query.iter(world).count() == 2
    });

    let telemetry = harness.server.world().resource::<CombatTelemetry>();
    assert_eq!(telemetry.defeats, 2);
    let cue_event_ids: Vec<_> = telemetry
        .cues
        .iter()
        .map(|cue| match cue {
            CombatCue::AttackAccepted { event_id, .. }
            | CombatCue::DeliveryImpact { event_id, .. }
            | CombatCue::LobLanded { event_id, .. }
            | CombatCue::MeleeContact { event_id, .. }
            | CombatCue::DamageApplied { event_id, .. }
            | CombatCue::EffectApplied { event_id, .. }
            | CombatCue::FighterDefeated { event_id, .. }
            | CombatCue::FighterReset { event_id, .. }
            | CombatCue::Muzzle { event_id, .. }
            | CombatCue::Impact { event_id, .. }
            | CombatCue::Damage { event_id, .. }
            | CombatCue::Defeat { event_id, .. }
            | CombatCue::Reset { event_id, .. } => event_id.0,
        })
        .collect();
    assert!(
        cue_event_ids.windows(2).all(|window| window[0] < window[1]),
        "combat cue event IDs must be globally increasing: {cue_event_ids:?}"
    );
    let mut defeated_targets: Vec<_> = telemetry
        .records
        .iter()
        .filter_map(|record| match record {
            CombatLogRecord::Defeat {
                source: Some(brawler::combat::DamageSource::PlayerWeapon { shot_id, .. }),
                target,
                ..
            } => Some((*target, *shot_id)),
            _ => None,
        })
        .collect();
    defeated_targets.sort_by_key(|(target, shot_id)| (target.0, shot_id.0));
    assert_eq!(defeated_targets.len(), 2);
    assert_ne!(defeated_targets[0].0, defeated_targets[1].0);
    assert_ne!(defeated_targets[0].1, defeated_targets[1].1);
}

#[test]
fn projectile_filters_allied_fighters_and_consumes_on_terrain() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
    });

    {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(
            &PlayerId,
            &mut Position,
            &mut Rotation,
            &mut TeamId,
        ), With<Fighter>>();
        let mut player_one_team = None;
        for (player, mut position, mut rotation, mut team) in query.iter_mut(world) {
            if player.0 == 0 {
                continue;
            }
            let right_side = player.0 % 2 == 0;
            position.0 = Vec2::new(if right_side { 100.0 } else { -100.0 }, 0.0);
            *rotation = Rotation::radians(if right_side {
                std::f32::consts::PI
            } else {
                0.0
            });
            if player_one_team.is_none() {
                player_one_team = Some(*team);
            } else {
                *team = player_one_team.expect("first player team");
            }
        }
    }
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    for _ in 0..70 {
        harness.step();
    }
    let server_health: Vec<_> = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &CurrentHealth), With<Fighter>>();
        query
            .iter(world)
            .filter(|(player, _)| player.0 != 0)
            .map(|(player, health)| (*player, *health))
            .collect()
    };
    assert!(server_health.iter().all(|(_, health)| health.0 == 100));
    assert_eq!(
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .hostile_fighter_hits,
        0
    );
    harness.set_controlled_input(0, FighterInput::default());
    for _ in 0..70 {
        harness.step();
    }
    assert_eq!(harness.server_projectile_count(), 0);

    {
        let world = harness.server.world_mut();
        let mut query =
            world.query_filtered::<(&PlayerId, &mut Position, &mut Rotation), With<Fighter>>();
        for (player, mut position, mut rotation) in query.iter_mut(world) {
            if player.0 == 1 {
                position.0 = Vec2::new(700.0, 0.0);
                *rotation = Rotation::IDENTITY;
            }
        }
    }
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .records
            .iter()
            .any(|record| matches!(record, CombatLogRecord::Hit { target: None, .. }))
    });
    assert_eq!(harness.server_projectile_count(), 0);
}

#[test]
fn projectile_hits_the_closest_valid_target_and_does_not_pass_through_it() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
    });
    let source_player = harness.controlled_player_id(0);
    let target_player = harness
        .server_ids()
        .into_iter()
        .find(|(player, _)| *player != source_player)
        .map(|(player, _)| player)
        .expect("second player");
    let target_network_id = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &NetworkEntityId), With<Fighter>>();
        *query
            .iter(world)
            .find(|(player, _)| **player == target_player)
            .expect("target fighter")
            .1
    };
    {
        let world = harness.server.world_mut();
        let mut fighters =
            world.query_filtered::<(&PlayerId, &mut Position, &mut Rotation), With<Fighter>>();
        for (player, mut position, mut rotation) in fighters.iter_mut(world) {
            if player.0 == 0 {
                position.0 = Vec2::new(300.0, -300.0);
            } else if *player == source_player {
                position.0 = Vec2::new(-300.0, -300.0);
                *rotation = Rotation::IDENTITY;
            } else {
                position.0 = Vec2::new(-100.0, -300.0);
            }
        }
    }
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .hostile_fighter_hits
            > 0
    });

    let (target_health, dummy_health) = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&PlayerId, &CurrentHealth), With<Fighter>>();
        let mut target_health = None;
        let mut dummy_health = None;
        for (player, health) in query.iter(world) {
            if *player == target_player {
                target_health = Some(health.0);
            } else if player.0 == 0 {
                dummy_health = Some(health.0);
            }
        }
        (
            target_health.expect("target health"),
            dummy_health.expect("dummy health"),
        )
    };
    assert!(target_health < 100);
    assert_eq!(dummy_health, 100);
    assert!(
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .records
            .iter()
            .any(|record| matches!(
                record,
                CombatLogRecord::Hit {
                    target: Some(target), ..
                } if *target == target_network_id
            ))
    );
}

#[test]
fn projectile_stops_at_thin_cover_before_the_target() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });
    {
        let world = harness.server.world_mut();
        let mut source =
            world.query_filtered::<(&PlayerId, &mut Position, &mut Rotation), With<Fighter>>();
        for (player, mut position, mut rotation) in source.iter_mut(world) {
            if player.0 == 0 {
                position.0 = Vec2::new(300.0, -220.0);
            } else {
                position.0 = Vec2::new(-300.0, -220.0);
                *rotation = Rotation::IDENTITY;
            }
        }
    }
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .records
            .iter()
            .any(|record| matches!(record, CombatLogRecord::Hit { target: None, .. }))
    });
    let dummy_health = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&CurrentHealth, With<TestDummy>>();
        query.single(world).expect("dummy health").0
    };
    assert_eq!(dummy_health, 100);
    assert_eq!(harness.server_projectile_count(), 0);
}

#[test]
fn posthumous_projectile_retains_original_source_attribution() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
    });
    {
        let world = harness.server.world_mut();
        let mut query =
            world.query_filtered::<(&PlayerId, &mut Position, &mut Rotation), With<Fighter>>();
        for (player, mut position, mut rotation) in query.iter_mut(world) {
            if player.0 == 0 {
                continue;
            }
            let right_side = player.0 % 2 == 0;
            position.0 = Vec2::new(if right_side { 100.0 } else { -100.0 }, 0.0);
            *rotation = Rotation::radians(if right_side {
                std::f32::consts::PI
            } else {
                0.0
            });
        }
    }
    let source_player = harness.controlled_player_id(0);
    let source_aim = if source_player.0 == 1 {
        Vec2::X
    } else {
        -Vec2::X
    };
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(source_aim), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| harness.server_projectile_count() > 0);
    let owner = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(Entity, &PlayerId), With<Fighter>>();
        query
            .iter(world)
            .find(|(_, player)| **player == source_player)
            .map(|(entity, _)| entity)
            .expect("owner fighter")
    };
    harness.server.world_mut().entity_mut(owner).insert((
        CurrentHealth(0),
        Defeated {
            event_id: CombatEventId(10_000),
            reset_at_tick: u64::MAX,
        },
    ));

    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .hostile_fighter_hits
            > 0
    });
    assert!(
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .records
            .iter()
            .any(|record| {
                matches!(
                    record,
                    CombatLogRecord::Damage {
                        source: brawler::combat::DamageSource::PlayerWeapon { player_id, .. },
                        ..
                    } if *player_id == source_player
                )
            })
    );
}

#[test]
fn late_join_recovers_active_projectile_and_defeated_durable_state() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
            && harness.selection_is_complete(0)
    });
    harness
        .server
        .world_mut()
        .resource_mut::<FighterDefinitions>()
        .entries[0]
        .defeat_reset_delay_ticks = 600;

    harness.set_controlled_input(0, FighterInput::default());
    harness.step();
    let dummy_aim = harness.aim_at_dummy(0);
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(dummy_aim), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| harness.server_projectile_count() > 0);

    harness.add_client(2);
    harness.step_until(|harness| {
        harness.client_is_active(1)
            && harness.server_ids().len() == 2
            && harness.client_ids(1).len() == 2
            && harness.client_projectile_count(1) > 0
    });
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<Entity, With<TestDummy>>();
        let dummy = query.single(world).expect("dummy");
        world.get::<Defeated>(dummy).is_some()
    });
    harness.step_until(|harness| {
        let (health, _, defeated) = harness.client_fighter_combat_state(1, DUMMY_NETWORK_ENTITY);
        health.0 == 0 && defeated
    });
}

#[test]
fn late_join_recovers_in_progress_reload_state() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });
    {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&mut Position, With<TestDummy>>();
        query.single_mut(world).expect("dummy").0.y = 300.0;
    }
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE),
    );
    let player_network_id = harness.server_ids()[0].1;
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(&NetworkEntityId, &WeaponState), With<Fighter>>();
        query.iter(world).any(|(network_id, weapon)| {
            *network_id == player_network_id
                && matches!(weapon.phase, WeaponPhase::Reloading { .. })
                && weapon.ammo == 0
        })
    });

    harness.add_client(2);
    harness.step_until(|harness| {
        harness.client_is_active(1)
            && harness.client_ids(1).len() == 2
            && matches!(
                harness
                    .client_fighter_combat_state(1, player_network_id)
                    .1
                    .phase,
                WeaponPhase::Reloading { .. }
            )
    });
}

#[test]
fn duplicate_and_reordered_fire_inputs_do_not_bypass_server_cadence() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });
    let target = harness.controlled_entity(0);
    let first_tick = harness.server_tick().saturating_add(1);
    let fire = FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE);
    for _ in 0..4 {
        harness.send_forged_input(
            0,
            lightyear::input::input_message::InputTarget::Entity(target),
            first_tick,
            fire,
        );
    }
    harness.step();
    let first_shot_count = harness
        .server
        .world()
        .resource::<CombatTelemetry>()
        .accepted_shots;
    assert_eq!(first_shot_count, 1);

    for stale_tick in [first_tick, first_tick.saturating_sub(1)] {
        for _ in 0..3 {
            harness.send_forged_input(
                0,
                lightyear::input::input_message::InputTarget::Entity(target),
                stale_tick,
                fire,
            );
        }
    }
    for _ in 0..4 {
        harness.step();
    }
    assert_eq!(
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .accepted_shots,
        first_shot_count
    );
    let diagnostics = harness
        .server
        .world_mut()
        .query::<&InputValidationState>()
        .iter(harness.server.world())
        .copied()
        .collect::<Vec<_>>();
    assert!(
        diagnostics
            .iter()
            .any(|state| state.stale_or_reordered_rejections > 0),
        "duplicate/stale inputs should be diagnosed: {diagnostics:?}"
    );

    let drop_tick = harness.server_tick().saturating_add(1);
    harness.send_forged_input(
        0,
        lightyear::input::input_message::InputTarget::Entity(target),
        drop_tick,
        FighterInput::default(),
    );
    for _ in 0..14 {
        harness.step();
    }
    let after_drop_count = harness
        .server
        .world()
        .resource::<CombatTelemetry>()
        .accepted_shots;
    assert_eq!(after_drop_count, first_shot_count);

    let release_tick = harness.server_tick().saturating_add(1);
    harness.send_forged_input(
        0,
        lightyear::input::input_message::InputTarget::Entity(target),
        release_tick,
        fire,
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .accepted_shots
            > after_drop_count
    });
}

#[test]
fn duplicate_and_reordered_cue_packets_converge_to_one_full_payload_stream() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
            && harness.client_ids(0).len() == 2
            && harness.client_ids(1).len() == 2
    });
    let dummy_aim = if harness.controlled_player_id(0).0 == 1 {
        Vec2::X
    } else {
        -Vec2::X
    };
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::ZERO, Some(dummy_aim), FighterInput::PRIMARY_FIRE),
    );
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .accepted_shots
            >= 1
    });
    harness.arm_cue_packet_impairment(0);
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .accepted_shots
            >= 4
    });
    harness.set_controlled_input(0, FighterInput::default());
    harness.step_until(|harness| {
        let expected_len = harness
            .server
            .world()
            .resource::<CombatTelemetry>()
            .cues
            .len();
        harness.cue_packet_impairment(0).injected
            && harness.client_cues(0).len() == expected_len
            && harness.client_cues(1).len() == expected_len
    });

    let expected = harness
        .server
        .world()
        .resource::<CombatTelemetry>()
        .cues
        .clone();
    let impairment = harness.cue_packet_impairment(0);
    assert!(impairment.duplicated_packets > 0);
    assert!(impairment.reordered_batches > 0);
    assert_eq!(harness.client_cues(0), expected.as_slice());
    assert_eq!(harness.client_cues(1), expected.as_slice());
}

#[test]
fn native_input_moves_the_server_owned_fighter_and_replicates_position() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });

    let initial = harness.server_positions();
    assert_eq!(initial.len(), 1);
    harness.set_controlled_input(0, FighterInput::from_axes(Vec2::X, Some(Vec2::X), 0));

    let mut previous = initial[0].1;
    for _ in 0..36 {
        harness.step();
        let current = harness.server_positions()[0].1;
        assert!(
            (current.0 - previous.0).length() <= MovementTuning::default().speed / 60.0 + 0.1,
            "one tick displacement exceeded the authoritative speed limit: {previous:?} -> {current:?}"
        );
        previous = current;
    }

    let final_positions = harness.server_positions();
    assert_eq!(final_positions.len(), 1);
    assert!(final_positions[0].1.0.x > initial[0].1.0.x + 1.0);
    assert!(final_positions[0].1.0.x <= 800.0 - 24.0 + f32::EPSILON);
}

#[test]
fn two_clients_move_simultaneously_and_observe_the_same_authoritative_poses() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
            && harness.client_ids(0).len() == 2
            && harness.client_ids(1).len() == 2
    });

    let initial = harness.server_positions();
    for index in 0..2 {
        let direction = if harness.controlled_player_id(index).0 == 1 {
            Vec2::X
        } else {
            -Vec2::X
        };
        harness.set_controlled_input(
            index,
            FighterInput::from_axes(direction, Some(direction), 0),
        );
    }
    for _ in 0..36 {
        harness.step();
    }

    let server = harness.server_positions();
    assert!(
        server[0].1.0.x > initial[0].1.0.x + 1.0,
        "initial={initial:?} server={server:?}"
    );
    assert!(
        server[1].1.0.x < initial[1].1.0.x - 1.0,
        "initial={initial:?} server={server:?}"
    );
    let poses = harness.server_poses();
    assert!(poses[0].2.as_radians().abs() < 0.01);
    assert!((poses[1].2.as_radians() - std::f32::consts::PI).abs() < 0.02);

    harness.set_controlled_input(0, FighterInput::default());
    harness.set_controlled_input(1, FighterInput::default());
    for _ in 0..8 {
        harness.step();
    }
    harness.sample_client_at_newest_position_history(0);
    harness.sample_client_at_newest_position_history(1);
    let client_zero = harness.client_positions(0);
    let client_one = harness.client_positions(1);
    assert_eq!(client_zero.len(), 2);
    assert_eq!(client_one.len(), 2);
    for (((player_zero, position_zero), (player_one, position_one)), (_, server_position)) in
        client_zero.iter().zip(&client_one).zip(&server)
    {
        assert_eq!(player_zero, player_one);
        assert!((position_zero.0 - position_one.0).length() < 0.5);
        assert!(
            (position_zero.0 - server_position.0).length() < 64.0,
            "client pose did not converge toward the authoritative pose: client={position_zero:?} server={server_position:?}"
        );
    }
    assert_eq!(harness.client_interpolated_fighters(0), 2);
    assert_eq!(harness.client_interpolated_fighters(1), 2);
}

#[test]
fn owner_view_records_authoritative_interpolation_baseline_without_prediction() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });

    let world = harness.clients[0].world_mut();
    let mut controlled = world.query_filtered::<Entity, (With<Fighter>, With<Controlled>)>();
    assert_eq!(controlled.iter(world).count(), 1);
    let mut predicted = world.query_filtered::<Entity, With<Predicted>>();
    assert_eq!(predicted.iter(world).count(), 0);
}

#[test]
fn changed_authoritative_position_reaches_an_existing_client() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });
    let expected = {
        let world = harness.server.world_mut();
        let mut query =
            world.query_filtered::<&mut Position, (With<Fighter>, Without<TestDummy>)>();
        let mut position = query.single_mut(world).expect("one server fighter");
        position.0.x += 100.0;
        *position
    };
    for _ in 0..12 {
        harness.step();
    }
    harness.sample_client_at_newest_position_history(0);
    let actual = harness.client_positions(0)[0].1;
    let history = {
        let world = harness.clients[0].world_mut();
        let mut query = world.query_filtered::<
            (&PlayerId, Option<&ConfirmedHistory<Position>>),
            (With<Fighter>, With<Remote>, Without<TestDummy>),
        >();
        query
            .iter(world)
            .find(|(player, _)| player.0 != 0)
            .and_then(|(_, history)| {
                history.and_then(|history| {
                    history.newest_present().map(|(tick, value)| (tick, *value))
                })
            })
    };
    let timeline = {
        let timeline = harness.clients[0]
            .world()
            .resource::<InterpolationTimeline>();
        (timeline.is_synced(), timeline.now())
    };
    assert!(
        (actual.0 - expected.0).length() < 64.0,
        "changed authoritative pose did not reach the client: actual={actual:?} expected={expected:?} history={history:?} timeline={timeline:?}"
    );
}

#[test]
fn late_join_receives_current_poses_without_duplicating_static_arena() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });
    let static_count = harness.server_static_arena_count();
    let initial = harness.server_positions()[0].1;
    harness.set_controlled_input(0, FighterInput::from_axes(Vec2::X, Some(Vec2::Y), 0));
    for _ in 0..24 {
        harness.step();
    }
    let before_join = harness.server_positions()[0].1;
    assert!((before_join.0 - initial.0).length() > 1.0);
    harness.set_controlled_input(0, FighterInput::default());
    for _ in 0..60 {
        harness.step();
    }

    harness.add_client(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
            && harness.client_ids(1).len() == 2
    });
    for _ in 0..60 {
        harness.step();
    }
    let server = harness.server_positions();
    let late_view = harness.client_positions(1);
    assert_eq!(server.len(), 2);
    assert_eq!(late_view.len(), 2);
    for ((server_player, server_position), (client_player, client_position)) in
        server.iter().zip(&late_view)
    {
        assert_eq!(server_player, client_player);
        assert!(
            (server_position.0 - client_position.0).length() < 1.0,
            "player={server_player:?} server={server_position:?} late={client_position:?}"
        );
    }
    assert_eq!(harness.server_static_arena_count(), static_count);
}

#[test]
fn hostile_input_and_client_pose_attempts_are_rejected_and_counted() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
            && harness.client_ids(0).len() == 2
    });
    harness.set_controlled_input(0, FighterInput::default());
    harness.set_controlled_input(1, FighterInput::default());
    for _ in 0..60 {
        harness.step();
    }

    let own_player = harness.controlled_player_id(0);
    let target_player = [PlayerId(1), PlayerId(2)]
        .into_iter()
        .find(|player| *player != own_player)
        .expect("two-client harness should have another player");
    let target_client_entity = harness.remote_entity_for_player(0, target_player);
    let initial_target = harness
        .server_positions()
        .into_iter()
        .find(|(player, _)| *player == target_player)
        .expect("target fighter should exist")
        .1;
    let spoof_target = harness.remote_entity_for_player(0, target_player);
    let spoof_tick = harness.server_tick().saturating_add(1);
    harness.send_forged_input(
        0,
        lightyear::input::input_message::InputTarget::Entity(spoof_target),
        spoof_tick,
        FighterInput::from_axes(Vec2::X, None, 0),
    );
    {
        let client = harness.clients[0].world_mut();
        client
            .entity_mut(target_client_entity)
            .insert(Position::from_xy(9_999.0, 9_999.0));
    }
    for _ in 0..4 {
        harness.step();
    }
    let target_after = harness
        .server_positions()
        .into_iter()
        .find(|(player, _)| *player == target_player)
        .expect("target fighter should remain")
        .1;
    assert!(
        (target_after.0 - initial_target.0).length() < 0.01,
        "spoofed target moved: initial={initial_target:?} after={target_after:?}"
    );
    let diagnostics = harness
        .server
        .world_mut()
        .query::<&InputValidationState>()
        .iter(harness.server.world())
        .copied()
        .collect::<Vec<_>>();
    assert!(
        diagnostics
            .iter()
            .any(|state| state.ownership_rejections > 0),
        "spoofed target should increment ownership diagnostics: {diagnostics:?}"
    );

    let own_target = harness.controlled_entity(0);
    let valid_tick = harness.server_tick().saturating_add(1);
    harness.send_forged_input(
        0,
        lightyear::input::input_message::InputTarget::Entity(own_target),
        valid_tick,
        FighterInput::from_axes(Vec2::X, None, 0),
    );
    harness.step();
    harness.send_forged_input(
        0,
        lightyear::input::input_message::InputTarget::Entity(own_target),
        valid_tick,
        FighterInput::default(),
    );
    let future_tick = harness.server_tick().saturating_add(100);
    harness.send_forged_input(
        0,
        lightyear::input::input_message::InputTarget::Entity(own_target),
        future_tick,
        FighterInput::default(),
    );
    let mut malformed = FighterInput::default();
    malformed.gameplay_buttons = 0x80;
    let malformed_tick = harness.server_tick().saturating_add(1);
    harness.send_forged_input(
        0,
        lightyear::input::input_message::InputTarget::Entity(own_target),
        malformed_tick,
        malformed,
    );
    harness.step();
    for link in &harness.server_links {
        harness
            .server
            .world_mut()
            .get_mut::<InputValidationState>(*link)
            .expect("validation state")
            .tokens = 0.0;
    }
    harness
        .server
        .world_mut()
        .resource_mut::<InputTuning>()
        .input_rate = 0.0;
    let rate_limited_tick = harness.server_tick().saturating_add(1);
    harness.send_forged_input(
        0,
        lightyear::input::input_message::InputTarget::Entity(own_target),
        rate_limited_tick,
        FighterInput::default(),
    );
    for _ in 0..4 {
        harness.step();
    }
    let diagnostics = harness
        .server
        .world_mut()
        .query::<&InputValidationState>()
        .iter(harness.server.world())
        .copied()
        .collect::<Vec<_>>();
    assert!(
        diagnostics
            .iter()
            .any(|state| state.stale_or_reordered_rejections > 0)
    );
    assert!(
        diagnostics
            .iter()
            .any(|state| state.old_or_future_rejections > 0)
    );
    assert!(
        diagnostics
            .iter()
            .any(|state| state.malformed_rejections > 0),
        "expected malformed diagnostic, got {diagnostics:?}"
    );
    assert!(diagnostics.iter().any(|state| state.rate_rejections > 0));
}

#[test]
fn client_owned_component_writes_cannot_mutate_authoritative_build_weapon_or_pose() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
            && harness.selection_is_complete(0)
    });
    let player_id = harness.controlled_player_id(0);
    let (server_build, server_fingerprint, server_ammo, server_position) = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(
            &PlayerId,
            &SelectedBuild,
            &ResolvedWeapon,
            &WeaponState,
            &Position,
        ), With<Fighter>>();
        let (_, build, resolved, weapon, position) = query
            .iter(world)
            .find(|(player, _, _, _, _)| **player == player_id)
            .expect("server fighter");
        (*build, resolved.recipe_fingerprint, *weapon, *position)
    };
    let client_entity = harness.controlled_entity(0);
    {
        let world = harness.clients[0].world_mut();
        let mut forged_build = server_build;
        forged_build.source_preset_id = Some(WeaponPresetId(4));
        forged_build.recipe_fingerprint = Some(WeaponRecipeFingerprint(0xdead_beef));
        let mut forged_resolved = world
            .get::<ResolvedWeapon>(client_entity)
            .expect("client resolved weapon")
            .clone();
        forged_resolved.recipe_fingerprint = WeaponRecipeFingerprint(0xdead_beef);
        world.entity_mut(client_entity).insert((
            forged_build,
            forged_resolved,
            WeaponState {
                ammo: 0,
                phase: WeaponPhase::Reloading { ready_at_tick: 1 },
            },
            Position::from_xy(9_000.0, 9_000.0),
        ));
    }
    for _ in 0..12 {
        harness.step();
    }
    let (actual_build, actual_fingerprint, actual_ammo, actual_position) = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<(
            &PlayerId,
            &SelectedBuild,
            &ResolvedWeapon,
            &WeaponState,
            &Position,
        ), With<Fighter>>();
        let (_, build, resolved, weapon, position) = query
            .iter(world)
            .find(|(player, _, _, _, _)| **player == player_id)
            .expect("server fighter");
        (*build, resolved.recipe_fingerprint, *weapon, *position)
    };
    assert_eq!(actual_build, server_build);
    assert_eq!(actual_fingerprint, server_fingerprint);
    assert_eq!(actual_ammo, server_ammo);
    assert!((actual_position.0 - server_position.0).length() < 0.01);
}

#[test]
fn authoritative_fighters_stop_at_walls_slide_tangentially_and_overlap() {
    let mut harness = Harness::new(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
    });

    let wall_client = (0..2)
        .find(|&index| harness.controlled_player_id(index).0 == 1)
        .expect("player one client");
    harness.set_controlled_input(wall_client, FighterInput::from_axes(Vec2::X, None, 0));
    for _ in 0..300 {
        harness.step();
    }
    let wall_pose = harness.server_poses()[0];
    assert!(
        (774.5..=776.0).contains(&wall_pose.1.0.x),
        "wall_pose={wall_pose:?}"
    );

    harness.set_controlled_input(
        wall_client,
        FighterInput::from_axes(Vec2::new(1.0, 1.0), None, 0),
    );
    let before_slide = harness.server_poses()[0].1.0;
    for _ in 0..60 {
        harness.step();
    }
    let after_slide = harness.server_poses()[0].1.0;
    assert!((774.5..=776.0).contains(&after_slide.x));
    assert!(after_slide.y > before_slide.y + 100.0);
    for _ in 0..240 {
        harness.step();
    }
    let corner_pose = harness.server_poses()[0].1.0;
    assert!((774.5..=776.0).contains(&corner_pose.x));
    assert!((474.5..=476.0).contains(&corner_pose.y));

    let mut overlap = Harness::new(2);
    overlap.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
    });
    let second_entity = {
        let world = overlap.server.world_mut();
        let mut query = world.query_filtered::<(Entity, &PlayerId), With<Fighter>>();
        query
            .iter(world)
            .find(|(_, player)| player.0 == 2)
            .map(|(entity, _)| entity)
            .expect("second fighter should exist")
    };
    overlap
        .server
        .world_mut()
        .entity_mut(second_entity)
        .insert(Position::from_xy(620.0, -300.0));
    for index in 0..2 {
        let direction = if overlap.controlled_player_id(index).0 == 1 {
            Vec2::X
        } else {
            -Vec2::X
        };
        overlap.set_controlled_input(index, FighterInput::from_axes(direction, None, 0));
    }
    for _ in 0..140 {
        overlap.step();
    }
    let overlap_poses = overlap.server_poses();
    assert!(
        (overlap_poses[0].1.0 - overlap_poses[1].1.0).length() < 48.0,
        "overlap_poses={overlap_poses:?}"
    );
}

#[test]
fn configured_arena_and_movement_resources_drive_spawn_and_collider_tuning() {
    let mut harness = Harness::new(1);
    {
        let mut arena = harness
            .server
            .world_mut()
            .resource_mut::<GreyboxArenaDefinition>();
        arena.spawn_x = [-700.0, 700.0];
    }
    harness
        .server
        .world_mut()
        .resource_mut::<MovementTuning>()
        .spawn_facing = 1.25;
    harness.step_until(|harness| harness.client_is_active(0) && harness.server_ids().len() == 1);
    let pose = harness.server_poses()[0];
    assert!((pose.1.0.x + 700.0).abs() < 0.5);
    assert!((pose.2.as_radians() - 1.25).abs() < 0.01);
}

#[test]
fn authoritative_move_and_slide_depenetrates_a_spawned_inside_cover_fighter() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| harness.client_is_active(0) && harness.server_ids().len() == 1);
    let fighter = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<Entity, (With<Fighter>, Without<TestDummy>)>();
        query
            .iter(world)
            .next()
            .expect("server should have one fighter")
    };
    harness
        .server
        .world_mut()
        .entity_mut(fighter)
        .insert(Position::from_xy(0.0, -220.0));
    harness.set_controlled_input(0, FighterInput::default());
    for _ in 0..4 {
        harness.step();
    }
    let pose = harness.server_poses()[0].1.0;
    assert!(pose.x.abs() >= 114.0 || (pose.y + 220.0).abs() >= 84.0);
}

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
fn real_udp_loopback_moves_and_replicates_authoritative_pose() {
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
        AvianNetworkPlugin,
        AuthoritativeMovementPlugin,
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
        AvianNetworkPlugin,
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
        let mut server_query =
            server_world.query_filtered::<Entity, (With<PlaceholderPlayer>, Without<TestDummy>)>();
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
