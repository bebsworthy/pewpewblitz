//! Dedicated authoritative server networking and lifecycle systems.
#![allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    clippy::too_many_arguments
)]

use crate::{
    VERSION,
    config::{NetworkTransport, ServerNetworkConfig},
    gameplay::GameplayPlugin,
    movement::{
        AuthoritativeMovementPlugin, AvianNetworkPlugin, GreyboxArenaDefinition, InputFreshness,
        InputValidationState, MovementTuning,
    },
    protocol::{
        ClientHello, DEVELOPMENT_PRIVATE_KEY, Fighter, JoinOutcome, JoinRejection, NetworkEntityId,
        PlaceholderState, PlayerId, ProtocolFingerprint, ProtocolPlugin,
    },
};
use avian2d::prelude::{
    AngularVelocity, Collider, CollisionLayers, CustomPositionIntegration, LinearVelocity,
    Position, RigidBody, Rotation,
};
use bevy::{
    app::{ScheduleRunnerPlugin, TerminalCtrlCHandlerPlugin},
    ecs::error::{FallbackErrorHandler, error},
    log::LogPlugin,
    prelude::*,
    state::app::StatesPlugin,
};
use core::time::Duration;
use lightyear::prelude::server::ServerUdpIo;
use lightyear::prelude::server::{
    NetcodeConfig, NetcodeServer, ServerPlugins, Start, Started, Stop, Stopped,
};
use lightyear::prelude::{Connected, Disconnected, LinkOf, Linked, LocalAddr};
use lightyear::prelude::{
    ControlledBy, InterpolationTarget, Lifetime, MessageReceiver, MessageSender, NetworkTarget,
    Replicate, ReplicationMetadata, ReplicationSender,
};
use std::{env, fs, path::PathBuf};

/// Server-side session phase. Lightyear lifecycle components remain the connection truth.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub enum ServerSessionPhase {
    AwaitingHello,
    Active {
        player_id: PlayerId,
        network_entity_id: NetworkEntityId,
    },
    Rejected,
}

#[derive(Component, Debug)]
pub struct ServerSession {
    pub phase: ServerSessionPhase,
    pub deadline: Duration,
    pub last_outcome: Option<JoinOutcome>,
    pub disconnect_requested: bool,
}

#[derive(Resource, Default, Debug)]
struct ServerShutdown {
    requested_exit: Option<AppExit>,
}

#[derive(Resource, Debug)]
struct ServerStartup {
    ready_file: Option<PathBuf>,
    ready_reported: bool,
    failure_reported: bool,
}

#[derive(Resource, Debug)]
struct ProcessMovementCheck {
    enabled: bool,
    ready_file: Option<PathBuf>,
    initial_poses: Vec<(PlayerId, Vec2, f32)>,
    initial_tick: Option<u64>,
    completed: bool,
}

impl FromWorld for ProcessMovementCheck {
    fn from_world(_: &mut World) -> Self {
        Self {
            enabled: env::var("BRAWLER_NETWORK_ASSERT_MOVEMENT").as_deref() == Ok("1"),
            ready_file: env::var_os("BRAWLER_NETWORK_MOVEMENT_READY_FILE").map(PathBuf::from),
            initial_poses: Vec::new(),
            initial_tick: None,
            completed: false,
        }
    }
}

impl FromWorld for ServerStartup {
    fn from_world(_: &mut World) -> Self {
        Self {
            ready_file: env::var_os("BRAWLER_SERVER_READY_FILE").map(PathBuf::from),
            ready_reported: false,
            failure_reported: false,
        }
    }
}

#[derive(Resource, Debug, PartialEq, Eq)]
pub struct NextSessionIds {
    next_player_id: u64,
    next_network_entity_id: u64,
}

impl Default for NextSessionIds {
    fn default() -> Self {
        Self {
            next_player_id: 1,
            next_network_entity_id: 1,
        }
    }
}

impl NextSessionIds {
    pub fn allocate(&mut self) -> Option<(PlayerId, NetworkEntityId)> {
        let player_id = self.next_player_id;
        let network_entity_id = self.next_network_entity_id;
        let next_player_id = player_id.checked_add(1)?;
        let next_network_entity_id = network_entity_id.checked_add(1)?;
        self.next_player_id = next_player_id;
        self.next_network_entity_id = next_network_entity_id;
        Some((PlayerId(player_id), NetworkEntityId(network_entity_id)))
    }
}

/// Marker proving that the network endpoint belongs to the dedicated server.
#[derive(Default, Resource, Debug, PartialEq, Eq)]
pub struct DedicatedServer;

/// Adds server startup diagnostics and clean scheduled execution.
pub struct DedicatedServerPlugin;

impl Plugin for DedicatedServerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DedicatedServer>()
            .add_systems(Startup, log_server_startup);
    }
}

fn log_server_startup(config: Option<Res<ServerNetworkConfig>>) {
    let bind_addr = config.map_or_else(
        || "unknown".to_string(),
        |config| config.bind_addr.to_string(),
    );
    info!(
        mode = "dedicated-server",
        version = VERSION,
        tick_hz = crate::timing::SIMULATION_TICK_HZ,
        bind = %bind_addr,
        "brawler dedicated server started"
    );
}

/// Installs the server Lightyear group, protocol, endpoint, and authoritative session systems.
pub struct ServerNetworkPlugin;

impl Plugin for ServerNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(FallbackErrorHandler(error))
            .init_resource::<NextSessionIds>()
            .init_resource::<ServerShutdown>()
            .init_resource::<ServerStartup>()
            .init_resource::<ProcessMovementCheck>()
            .insert_resource(ReplicationMetadata::new(crate::timing::SIMULATION_TICK))
            .add_observer(configure_new_link)
            .add_systems(Startup, spawn_server_endpoint)
            .add_systems(
                Update,
                (
                    observe_server_endpoint,
                    initialize_sessions,
                    process_client_hellos,
                    enforce_session_deadlines,
                    disconnect_rejected_sessions,
                    verify_process_movement,
                )
                    .chain(),
            )
            .add_systems(
                Last,
                (forward_app_exit_to_server_stop, finish_server_shutdown).chain(),
            );
    }
}

fn verify_process_movement(
    mut check: ResMut<ProcessMovementCheck>,
    tick: Res<crate::timing::SimulationTick>,
    fighters: Query<(&PlayerId, &Position, &Rotation), With<Fighter>>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if !check.enabled || check.completed {
        return;
    }
    let mut current: Vec<_> = fighters
        .iter()
        .map(|(player, position, rotation)| (*player, position.0, rotation.as_radians()))
        .collect();
    current.sort_by_key(|(player, _, _)| player.0);
    if current.len() < 2 {
        return;
    }
    if check.initial_poses.is_empty() {
        check.initial_poses.clone_from(&current);
        check.initial_tick = Some(tick.0);
        return;
    }
    if check
        .initial_tick
        .is_none_or(|initial_tick| tick.0 < initial_tick.saturating_add(120))
    {
        return;
    }
    let moved = current.iter().any(|(player, position, _)| {
        check
            .initial_poses
            .iter()
            .find(|(initial_player, _, _)| initial_player == player)
            .is_some_and(|(_, initial_position, _)| {
                (*position - *initial_position).length() > 100.0
            })
    });
    let aimed = current.iter().any(|(player, _, facing)| {
        check
            .initial_poses
            .iter()
            .find(|(initial_player, _, _)| initial_player == player)
            .is_some_and(|(_, _, initial_facing)| (facing - initial_facing).abs() > 0.5)
    });
    if moved && aimed {
        info!(tick = tick.0, "network movement smoke assertion passed");
        if let Some(path) = check.ready_file.as_ref()
            && let Err(error) = fs::write(path, b"passed\n")
        {
            error!(
                ?path,
                ?error,
                "network movement smoke readiness signal failed"
            );
            app_exit.write(AppExit::error());
        }
        check.completed = true;
    } else {
        error!(
            tick = tick.0,
            moved, aimed, "network movement smoke assertion failed"
        );
        app_exit.write(AppExit::error());
        check.completed = true;
    }
}

fn configure_new_link(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands
        .entity(trigger.entity)
        .insert((ReplicationSender, InputValidationState::default()));
}

fn spawn_server_endpoint(mut commands: Commands, config: Res<ServerNetworkConfig>) -> Result {
    if config.transport != NetworkTransport::Udp {
        return Ok(());
    }
    let netcode_config =
        NetcodeConfig::default()
            .with_protocol_id(config.network_protocol_id)
            .with_key(DEVELOPMENT_PRIVATE_KEY)
            .with_client_timeout_secs(config.client_timeout.as_secs().try_into().map_err(
                |_| "server client timeout does not fit in Netcode's i32 seconds field",
            )?);
    let server = commands
        .spawn((
            NetcodeServer::new(netcode_config),
            LocalAddr(config.bind_addr),
            ServerUdpIo::default(),
            Name::new("Brawler UDP server"),
        ))
        .id();
    commands.trigger(Start { entity: server });
    Ok(())
}

fn observe_server_endpoint(
    mut startup: ResMut<ServerStartup>,
    ready_query: Query<
        (),
        (
            With<NetcodeServer>,
            With<ServerUdpIo>,
            With<Started>,
            With<Linked>,
        ),
    >,
    failed_query: Query<
        (),
        (
            With<NetcodeServer>,
            With<ServerUdpIo>,
            With<Started>,
            Without<Linked>,
        ),
    >,
    mut app_exit: MessageWriter<AppExit>,
) {
    if startup.ready_reported || startup.failure_reported {
        return;
    }
    if failed_query.iter().next().is_some() {
        startup.failure_reported = true;
        error!("brawler server endpoint failed to bind or link");
        app_exit.write(AppExit::error());
        return;
    }
    if ready_query.iter().next().is_none() {
        return;
    }
    if let Some(path) = startup.ready_file.clone() {
        if let Err(error) = fs::write(&path, b"ready\n") {
            startup.failure_reported = true;
            error!(path = %path.display(), ?error, "brawler server readiness signal failed");
            app_exit.write(AppExit::error());
            return;
        }
        info!(path = %path.display(), "brawler server readiness signal written");
    }
    startup.ready_reported = true;
}

fn initialize_sessions(
    mut commands: Commands,
    config: Res<ServerNetworkConfig>,
    time: Res<Time<Real>>,
    query: Query<(Entity, Has<Connected>, Option<&ServerSession>), With<LinkOf>>,
) {
    let now = time.elapsed();
    for (entity, connected, session) in query.iter() {
        if connected && session.is_none() {
            commands.entity(entity).insert(ServerSession {
                phase: ServerSessionPhase::AwaitingHello,
                deadline: now.saturating_add(config.handshake_timeout),
                last_outcome: None,
                disconnect_requested: false,
            });
        }
    }
}

fn process_client_hellos(
    mut commands: Commands,
    config: Res<ServerNetworkConfig>,
    fingerprint: Res<ProtocolFingerprint>,
    arena: Res<GreyboxArenaDefinition>,
    movement_tuning: Res<MovementTuning>,
    mut ids: ResMut<NextSessionIds>,
    mut receivers: Query<(
        Entity,
        &mut MessageReceiver<ClientHello>,
        &mut MessageSender<JoinOutcome>,
        &mut ServerSession,
    )>,
    placeholders: Query<(), With<Fighter>>,
) {
    let mut active_count = placeholders.iter().count();
    for (connection, mut receiver, mut sender, mut session) in receivers.iter_mut() {
        let messages: Vec<_> = receiver.receive().collect();
        for hello in messages {
            match &session.phase {
                ServerSessionPhase::Active {
                    player_id,
                    network_entity_id,
                } => {
                    sender.send::<crate::protocol::SessionChannel>(JoinOutcome::Accepted {
                        player_id: *player_id,
                        network_entity_id: *network_entity_id,
                    });
                }
                ServerSessionPhase::Rejected => {
                    if let Some(outcome) = &session.last_outcome {
                        sender.send::<crate::protocol::SessionChannel>(outcome.clone());
                    }
                }
                ServerSessionPhase::AwaitingHello => {
                    let outcome = if active_count >= config.max_clients {
                        JoinOutcome::Rejected {
                            reason: JoinRejection::ServerFull,
                        }
                    } else if hello.protocol_version != crate::protocol::SUPPORTED_PROTOCOL_VERSION
                    {
                        JoinOutcome::Rejected {
                            reason: JoinRejection::ProtocolVersionMismatch,
                        }
                    } else if hello.build_version != VERSION {
                        JoinOutcome::Rejected {
                            reason: JoinRejection::BuildVersionMismatch,
                        }
                    } else if hello.registry_fingerprint != fingerprint.0 {
                        JoinOutcome::Rejected {
                            reason: JoinRejection::RegistryMismatch,
                        }
                    } else {
                        match ids.allocate() {
                            Some((player_id, network_entity_id)) => {
                                let accepted = JoinOutcome::Accepted {
                                    player_id,
                                    network_entity_id,
                                };
                                let spawn_position = arena.spawn_position(player_id.0);
                                commands.spawn((
                                    Fighter,
                                    player_id,
                                    network_entity_id,
                                    PlaceholderState {
                                        spawn_slot: u64::from(GreyboxArenaDefinition::spawn_slot(
                                            player_id.0,
                                        )),
                                    },
                                    Position::from_xy(spawn_position.x, spawn_position.y),
                                    Rotation::radians(movement_tuning.spawn_facing),
                                    LinearVelocity::default(),
                                    AngularVelocity::default(),
                                    Collider::circle(movement_tuning.radius),
                                    RigidBody::Kinematic,
                                    CustomPositionIntegration,
                                    CollisionLayers::new(
                                        crate::movement::FIGHTER_LAYER,
                                        crate::movement::INDESTRUCTIBLE_TERRAIN_LAYER
                                            | crate::movement::DESTRUCTIBLE_TERRAIN_LAYER,
                                    ),
                                    InputFreshness::default(),
                                    (
                                        Replicate::to_clients(NetworkTarget::All),
                                        InterpolationTarget::to_clients(NetworkTarget::All),
                                    ),
                                    ControlledBy {
                                        owner: connection,
                                        lifetime: Lifetime::SessionBased,
                                    },
                                ));
                                session.phase = ServerSessionPhase::Active {
                                    player_id,
                                    network_entity_id,
                                };
                                active_count += 1;
                                accepted
                            }
                            None => JoinOutcome::Rejected {
                                reason: JoinRejection::IdentifierExhausted,
                            },
                        }
                    };
                    let rejected = matches!(outcome, JoinOutcome::Rejected { .. });
                    sender.send::<crate::protocol::SessionChannel>(outcome.clone());
                    session.last_outcome = Some(outcome);
                    if rejected {
                        session.phase = ServerSessionPhase::Rejected;
                    }
                }
            }
        }
    }
}

fn enforce_session_deadlines(
    time: Res<Time<Real>>,
    mut query: Query<(Entity, &mut ServerSession, Has<Disconnected>), With<LinkOf>>,
) {
    let now = time.elapsed();
    for (entity, mut session, disconnected) in query.iter_mut() {
        if !disconnected
            && matches!(session.phase, ServerSessionPhase::AwaitingHello)
            && now >= session.deadline
        {
            session.phase = ServerSessionPhase::Rejected;
            session.last_outcome = Some(JoinOutcome::Rejected {
                reason: JoinRejection::HandshakeTimeout,
            });
            warn!(?entity, "brawler server handshake timed out");
        }
    }
}

fn disconnect_rejected_sessions(
    mut commands: Commands,
    mut query: Query<(Entity, &mut ServerSession), With<LinkOf>>,
) {
    for (entity, mut session) in query.iter_mut() {
        if matches!(session.phase, ServerSessionPhase::Rejected) && !session.disconnect_requested {
            session.disconnect_requested = true;
            continue;
        }
        if matches!(session.phase, ServerSessionPhase::Rejected) {
            // Server-side `Disconnect` is a client/host trigger in Lightyear 0.29. A
            // rejected link has no authoritative entity to preserve, so mark the lifecycle
            // outcome and remove the link after its prior-frame rejection has flushed.
            commands
                .entity(entity)
                .insert(Disconnected::default())
                .despawn();
        }
    }
}

fn forward_app_exit_to_server_stop(
    mut app_exits: ResMut<Messages<AppExit>>,
    mut shutdown: ResMut<ServerShutdown>,
    mut commands: Commands,
    query: Query<(Entity, Option<&Stopped>), With<NetcodeServer>>,
) {
    if shutdown.requested_exit.is_some() {
        return;
    }
    let exits: Vec<_> = app_exits.drain().collect();
    let Some(exit) = exits
        .iter()
        .find(|exit| exit.is_error())
        .or_else(|| exits.first())
        .cloned()
    else {
        return;
    };
    shutdown.requested_exit = Some(exit);
    for (entity, stopped) in query.iter() {
        if stopped.is_none() {
            commands.trigger(Stop { entity });
        }
    }
}

fn finish_server_shutdown(
    mut app_exits: ResMut<Messages<AppExit>>,
    mut shutdown: ResMut<ServerShutdown>,
    query: Query<(), (With<NetcodeServer>, With<Stopped>)>,
) {
    if query.iter().next().is_some()
        && let Some(exit) = shutdown.requested_exit.take()
    {
        app_exits.write(exit);
    }
}

/// Build the production headless dedicated server application.
pub fn build_app_with_config(config: ServerNetworkConfig) -> App {
    let mut app = App::new();
    app.insert_resource(config)
        .add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
            crate::timing::SIMULATION_TICK,
        )))
        .add_plugins(StatesPlugin)
        .add_plugins(TerminalCtrlCHandlerPlugin)
        .add_plugins(LogPlugin::default())
        .add_plugins(ServerPlugins {
            tick_duration: crate::timing::SIMULATION_TICK,
        })
        .add_plugins((
            GameplayPlugin,
            ProtocolPlugin,
            AvianNetworkPlugin,
            AuthoritativeMovementPlugin,
            ServerNetworkPlugin,
            DedicatedServerPlugin,
        ));
    app
}

/// Build the default production server application.
pub fn build_app() -> App {
    build_app_with_config(ServerNetworkConfig::default())
}

/// Request a graceful server stop through Lightyear's lifecycle.
pub fn request_stop(world: &mut World, server: Entity) {
    world.trigger(Stop { entity: server });
}

#[cfg(feature = "network-test")]
pub fn spawn_crossbeam_server(world: &mut World, config: &ServerNetworkConfig) -> Entity {
    let timeout_secs = i32::try_from(config.client_timeout.as_secs())
        .expect("test client timeout must fit in Netcode's i32 seconds field");
    let netcode_config = NetcodeConfig::default()
        .with_protocol_id(config.network_protocol_id)
        .with_key(DEVELOPMENT_PRIVATE_KEY)
        .with_client_timeout_secs(timeout_secs);
    let netcode_config = NetcodeConfig {
        server_addr_check: false,
        ..netcode_config
    };
    let server = world.spawn(NetcodeServer::new(netcode_config)).id();
    world.flush();
    world.trigger(Start { entity: server });
    world.flush();
    server
}

#[cfg(feature = "network-test")]
pub fn spawn_crossbeam_link(
    world: &mut World,
    server: Entity,
    io: lightyear::crossbeam::CrossbeamIo,
) -> Entity {
    let link = world.spawn((LinkOf { server }, io)).id();
    world.flush();
    world.trigger(lightyear::link::LinkStart { entity: link });
    world.flush();
    link
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn checked_ids_start_at_one_and_never_wrap() {
        let mut ids = NextSessionIds::default();
        assert_eq!(ids.allocate(), Some((PlayerId(1), NetworkEntityId(1))));
        assert_eq!(ids.allocate(), Some((PlayerId(2), NetworkEntityId(2))));
        ids.next_player_id = u64::MAX;
        assert_eq!(ids.allocate(), None);
        assert_eq!(ids.next_player_id, u64::MAX);
    }

    #[test]
    fn server_config_rejects_unbounded_values() {
        let config = ServerNetworkConfig {
            max_clients: 0,
            ..default()
        };
        assert!(config.validate().is_err());
    }

    #[test]
    fn started_unlinked_udp_server_requests_error_exit() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, StatesPlugin))
            .init_resource::<lightyear::prelude::PeerMetadata>()
            .init_resource::<ServerStartup>()
            .add_systems(Update, observe_server_endpoint);
        app.world_mut().spawn((
            NetcodeServer::new(NetcodeConfig::default()),
            ServerUdpIo::default(),
            Started,
        ));

        app.update();

        assert!(app.should_exit().is_some_and(|exit| exit.is_error()));
    }

    #[test]
    fn app_exit_is_forwarded_after_update_producers_run() {
        fn request_exit(mut app_exit: MessageWriter<AppExit>) {
            app_exit.write(AppExit::Success);
        }

        let mut app = App::new();
        app.add_plugins(MinimalPlugins)
            .add_message::<AppExit>()
            .init_resource::<lightyear::prelude::PeerMetadata>()
            .init_resource::<ServerShutdown>()
            .add_systems(Update, request_exit)
            .add_systems(
                Last,
                (forward_app_exit_to_server_stop, finish_server_shutdown).chain(),
            )
            .add_observer(|trigger: On<Stop>, mut commands: Commands| {
                commands.entity(trigger.entity).insert(Stopped);
            });
        let server = app
            .world_mut()
            .spawn(NetcodeServer::new(NetcodeConfig::default()))
            .id();

        app.update();

        assert!(
            app.world()
                .resource::<ServerShutdown>()
                .requested_exit
                .is_none()
        );
        assert!(app.world().get::<Stopped>(server).is_some());
        assert!(app.should_exit().is_some_and(|exit| exit.is_success()));
    }
}
