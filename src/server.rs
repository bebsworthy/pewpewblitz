//! Dedicated authoritative server networking and lifecycle systems.
#![allow(clippy::needless_pass_by_value, clippy::type_complexity)]

use crate::{
    VERSION,
    config::{NetworkTransport, ServerNetworkConfig},
    gameplay::GameplayPlugin,
    protocol::{
        ClientHello, DEVELOPMENT_PRIVATE_KEY, JoinOutcome, JoinRejection, NetworkEntityId,
        PlaceholderPlayer, PlaceholderState, PlayerId, ProtocolPlugin,
    },
};
use bevy::{
    app::{ScheduleRunnerPlugin, TerminalCtrlCHandlerPlugin},
    log::LogPlugin,
    prelude::*,
    state::app::StatesPlugin,
};
use core::time::Duration;
use lightyear::prelude::server::ServerUdpIo;
use lightyear::prelude::server::{NetcodeConfig, NetcodeServer, ServerPlugins, Start, Stop};
use lightyear::prelude::{Connected, Disconnected, LinkOf, LocalAddr};
use lightyear::prelude::{
    ControlledBy, Lifetime, MessageReceiver, MessageSender, NetworkTarget, Replicate,
    ReplicationSender,
};

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
        app.init_resource::<NextSessionIds>()
            .add_observer(configure_new_link)
            .add_systems(Startup, spawn_server_endpoint)
            .add_systems(
                Update,
                (
                    initialize_sessions,
                    process_client_hellos,
                    enforce_session_deadlines,
                    disconnect_rejected_sessions,
                )
                    .chain(),
            );
    }
}

fn configure_new_link(trigger: On<Add, LinkOf>, mut commands: Commands) {
    commands.entity(trigger.entity).insert(ReplicationSender);
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
    mut ids: ResMut<NextSessionIds>,
    mut receivers: Query<(
        Entity,
        &mut MessageReceiver<ClientHello>,
        &mut MessageSender<JoinOutcome>,
        &mut ServerSession,
    )>,
    placeholders: Query<(), With<PlaceholderPlayer>>,
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
                    } else {
                        match ids.allocate() {
                            Some((player_id, network_entity_id)) => {
                                match u16::try_from(player_id.0) {
                                    Ok(spawn_slot) => {
                                        let accepted = JoinOutcome::Accepted {
                                            player_id,
                                            network_entity_id,
                                        };
                                        commands.spawn((
                                            PlaceholderPlayer,
                                            player_id,
                                            network_entity_id,
                                            PlaceholderState { spawn_slot },
                                            Replicate::to_clients(NetworkTarget::All),
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
                                    Err(_) => JoinOutcome::Rejected {
                                        reason: JoinRejection::IdentifierExhausted,
                                    },
                                }
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
}
