//! Client networking, compatibility status, and lightweight roster presentation.
#![allow(clippy::needless_pass_by_value, clippy::type_complexity)]

use crate::{
    VERSION,
    config::{ClientNetworkConfig, NetworkTransport},
    gameplay::GameplayPlugin,
    protocol::{
        ClientHello, JoinOutcome, JoinRejection, NetworkEntityId, PlayerId, ProtocolPlugin,
        SessionChannel,
    },
};
use bevy::{
    app::ScheduleRunnerPlugin, log::LogPlugin, prelude::*, state::app::StatesPlugin,
    window::WindowCloseRequested,
};
use core::time::Duration;
use lightyear::prelude::client::{Client, Connected, Connecting, Disconnected, Remote};
use lightyear::prelude::client::{
    ClientPlugins, Connect, Disconnect, NetcodeClient, NetcodeConfig,
};
use lightyear::prelude::{Authentication, LocalAddr, PeerAddr};
use lightyear::prelude::{MessageReceiver, MessageSender, PingManager, ReplicationReceiver, UdpIo};

/// User-visible client connection state. Lightyear lifecycle components remain the truth.
#[derive(Component, Clone, Debug, PartialEq, Eq)]
pub enum ClientJoinPhase {
    Connecting,
    AwaitingOutcome,
    Active {
        player_id: PlayerId,
        network_entity_id: NetworkEntityId,
    },
    Rejected(JoinRejection),
    Disconnected,
}

#[derive(Component, Debug)]
pub struct ClientJoinStatus {
    pub phase: ClientJoinPhase,
    pub started_at: Duration,
    pub disconnect_requested: bool,
}

#[derive(Resource, Default, Debug, PartialEq, Eq)]
struct RosterLogState(Vec<(PlayerId, NetworkEntityId)>);

/// Marker proving that the windowed presentation composition is installed.
#[derive(Default, Resource, Debug, PartialEq, Eq)]
pub struct ClientPresentation;

/// Adds client-only window behavior and startup diagnostics.
pub struct ClientPresentationPlugin;

impl Plugin for ClientPresentationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientPresentation>()
            .add_systems(Update, exit_on_close_requested);
    }
}

fn exit_on_close_requested(
    mut close_requests: MessageReader<WindowCloseRequested>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if close_requests.read().next().is_some() {
        app_exit.write(AppExit::Success);
    }
}

/// Installs the client Lightyear group, protocol, connection, and status systems.
pub struct ClientNetworkPlugin;

impl Plugin for ClientNetworkPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RosterLogState>()
            .add_systems(Startup, spawn_client_connection)
            .add_systems(
                Update,
                (
                    send_client_hello,
                    process_join_outcome,
                    disconnect_rejected_client,
                    observe_client_lifecycle,
                    log_replicated_roster,
                    enforce_client_timeout,
                )
                    .chain(),
            );
    }
}

fn spawn_client_connection(
    mut commands: Commands,
    config: Res<ClientNetworkConfig>,
    time: Res<Time<Real>>,
) -> Result {
    if config.transport != NetworkTransport::Udp {
        return Ok(());
    }
    let auth = Authentication::Manual {
        server_addr: config.server_addr,
        client_id: config.client_id,
        private_key: crate::protocol::DEVELOPMENT_PRIVATE_KEY,
        protocol_id: config.network_protocol_id,
    };
    let netcode_config =
        NetcodeConfig {
            client_timeout_secs: config.connect_timeout.as_secs().try_into().map_err(
                |_| "client connect timeout does not fit in Netcode's i32 seconds field",
            )?,
            token_expire_secs: -1,
            ..default()
        };
    let entity = commands
        .spawn((
            ClientJoinStatus {
                phase: ClientJoinPhase::Connecting,
                started_at: time.elapsed(),
                disconnect_requested: false,
            },
            PingManager::default(),
            ReplicationReceiver,
            NetcodeClient::new(auth, netcode_config)?,
            LocalAddr(config.local_addr),
            PeerAddr(config.server_addr),
            UdpIo::default(),
            Name::new(format!("Brawler client {}", config.client_id)),
        ))
        .id();
    commands.trigger(Connect { entity });
    info!(
        mode = "client",
        version = VERSION,
        tick_hz = crate::timing::SIMULATION_TICK_HZ,
        client_id = config.client_id,
        server = %config.server_addr,
        "brawler client connecting"
    );
    Ok(())
}

fn send_client_hello(
    config: Res<ClientNetworkConfig>,
    time: Res<Time<Real>>,
    mut query: Query<
        (&mut ClientJoinStatus, &mut MessageSender<ClientHello>),
        (With<Client>, With<Connected>),
    >,
) {
    for (mut status, mut sender) in query.iter_mut() {
        if matches!(status.phase, ClientJoinPhase::Connecting) {
            sender.send::<SessionChannel>(ClientHello {
                protocol_version: config.expected_protocol_version,
                build_version: config.expected_build_version.clone(),
            });
            status.phase = ClientJoinPhase::AwaitingOutcome;
            status.started_at = time.elapsed();
            info!("brawler client connected; awaiting compatibility outcome");
        }
    }
}

fn process_join_outcome(
    mut query: Query<
        (
            &mut ClientJoinStatus,
            Option<&mut MessageReceiver<JoinOutcome>>,
        ),
        With<Client>,
    >,
) {
    for (mut status, receiver) in query.iter_mut() {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for outcome in receiver.receive() {
            match outcome {
                JoinOutcome::Accepted {
                    player_id,
                    network_entity_id,
                } => {
                    info!(
                        player_id = player_id.0,
                        network_entity_id = network_entity_id.0,
                        "brawler client accepted"
                    );
                    status.phase = ClientJoinPhase::Active {
                        player_id,
                        network_entity_id,
                    };
                }
                JoinOutcome::Rejected { reason } => {
                    warn!(?reason, "brawler client rejected");
                    status.phase = ClientJoinPhase::Rejected(reason);
                }
            }
        }
    }
}

fn disconnect_rejected_client(
    mut commands: Commands,
    mut query: Query<(Entity, &mut ClientJoinStatus), With<Client>>,
) {
    for (entity, mut status) in query.iter_mut() {
        if matches!(status.phase, ClientJoinPhase::Rejected(_)) && !status.disconnect_requested {
            status.disconnect_requested = true;
            commands.trigger(Disconnect { entity });
        }
    }
}

fn observe_client_lifecycle(
    mut query: Query<
        (
            &mut ClientJoinStatus,
            Option<&Disconnected>,
            Has<Connecting>,
        ),
        With<Client>,
    >,
) {
    for (mut status, disconnected, connecting) in query.iter_mut() {
        if disconnected.is_some()
            && !connecting
            && !matches!(
                status.phase,
                ClientJoinPhase::Connecting
                    | ClientJoinPhase::Rejected(_)
                    | ClientJoinPhase::Disconnected
            )
        {
            let reason = disconnected.map(|disconnected| disconnected.reason.to_string());
            warn!(?reason, "brawler client disconnected");
            status.phase = ClientJoinPhase::Disconnected;
        }
    }
}

fn log_replicated_roster(
    config: Res<ClientNetworkConfig>,
    mut roster_state: ResMut<RosterLogState>,
    status_query: Query<&ClientJoinStatus, With<Client>>,
    roster: Query<
        (&PlayerId, &NetworkEntityId),
        (With<Remote>, With<crate::protocol::PlaceholderPlayer>),
    >,
    mut app_exit: MessageWriter<AppExit>,
) {
    let mut current: Vec<_> = roster
        .iter()
        .map(|(player, entity)| (*player, *entity))
        .collect();
    current.sort_by_key(|(player, entity)| (player.0, entity.0));
    if current != roster_state.0 {
        info!(
            roster = ?current.iter().map(|(player, entity)| (player.0, entity.0)).collect::<Vec<_>>(),
            "brawler replicated roster changed"
        );
        roster_state.0.clone_from(&current);
    }
    if let Some(target) = config.exit_after_roster
        && status_query
            .iter()
            .any(|status| matches!(status.phase, ClientJoinPhase::Active { .. }))
        && current.len() >= target
    {
        app_exit.write(AppExit::Success);
    }
}

fn enforce_client_timeout(
    config: Res<ClientNetworkConfig>,
    time: Res<Time<Real>>,
    status_query: Query<&ClientJoinStatus, With<Client>>,
    mut app_exit: MessageWriter<AppExit>,
) {
    let now = time.elapsed();
    for status in status_query.iter() {
        if matches!(
            status.phase,
            ClientJoinPhase::Connecting | ClientJoinPhase::AwaitingOutcome
        ) && now >= status.started_at.saturating_add(config.connect_timeout)
        {
            error!("brawler client connection timed out");
            app_exit.write(AppExit::error());
        }
    }
}

/// Build the windowed or headless client application.
pub fn build_app_with_config(config: ClientNetworkConfig) -> App {
    let headless = config.headless;
    let client_id = config.client_id;
    let mut app = App::new();
    app.insert_resource(config);
    if headless {
        app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
            crate::timing::SIMULATION_TICK,
        )))
        .add_plugins(StatesPlugin)
        .add_plugins(LogPlugin::default());
    } else {
        app.add_plugins(DefaultPlugins.set(WindowPlugin {
            primary_window: Some(Window {
                title: format!("Brawler Client {client_id}"),
                ..default()
            }),
            ..default()
        }));
    }
    app.add_plugins(ClientPlugins {
        tick_duration: crate::timing::SIMULATION_TICK,
    })
    .add_plugins((GameplayPlugin, ProtocolPlugin, ClientNetworkPlugin));
    if !headless {
        app.add_plugins(ClientPresentationPlugin);
    }
    app
}

/// Build the default client application.
pub fn build_app() -> App {
    build_app_with_config(ClientNetworkConfig::new(1))
}

#[cfg(feature = "network-test")]
pub fn spawn_crossbeam_client(
    world: &mut World,
    config: ClientNetworkConfig,
    io: lightyear::crossbeam::CrossbeamIo,
) -> Entity {
    let auth = Authentication::Manual {
        server_addr: config.server_addr,
        client_id: config.client_id,
        private_key: crate::protocol::DEVELOPMENT_PRIVATE_KEY,
        protocol_id: config.network_protocol_id,
    };
    let entity = world
        .spawn((
            ClientJoinStatus {
                phase: ClientJoinPhase::Connecting,
                started_at: Duration::ZERO,
                disconnect_requested: false,
            },
            PingManager::default(),
            ReplicationReceiver,
            NetcodeClient::new(auth, NetcodeConfig::default()).expect("test netcode client"),
            io,
        ))
        .id();
    world.flush();
    world.trigger(Connect { entity });
    entity
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_config_defaults_to_loopback_and_validates_roster_target() {
        let mut config = ClientNetworkConfig::new(1);
        assert!(config.validate().is_ok());
        config.exit_after_roster = Some(0);
        assert!(config.validate().is_err());
    }
}
