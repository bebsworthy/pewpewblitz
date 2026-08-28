use super::{
    Added, Authentication, ClientJoinPhase, ClientJoinStatus, ClientNetworkConfig, Commands,
    Connect, Connecting, Disconnected, Duration, Entity, Link, LinkMtu, LocalAddr, Name,
    NetcodeClient, NetcodeConfig, NetworkTransport, PeerAddr, PendingClientConnect, PingManager,
    Query, ROUTED_LINK_MTU, Real, ReplicationReceiver, Res, ResMut, Result, RoutedClientLifecycle,
    RoutedClientSession, RoutedClientSessionKind, RoutedUdpIo, RuntimeLobbyTarget, String, Time,
    UdpIo, Unlinked, VERSION, With, default, format, info,
};

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn spawn_client_connection(
    mut commands: Commands,
    config: Res<ClientNetworkConfig>,
    time: Res<Time<Real>>,
    mut routed: ResMut<RoutedClientLifecycle>,
) -> Result {
    if !config.connects_on_startup() {
        info!(
            mode = "client",
            "brawler client awaiting product-shell action"
        );
        return Ok(());
    }
    if config.transport == NetworkTransport::RoutedUdp {
        routed.start_lobby();
        let generation = routed.generation;
        spawn_client_entity(
            &mut commands,
            &config,
            time.elapsed(),
            Some((
                RoutedUdpIo::lobby(config.server_addr),
                RoutedClientSession {
                    generation,
                    kind: RoutedClientSessionKind::Lobby,
                },
            )),
        )?;
        info!(
            mode = "client",
            transport = "routed-udp",
            version = VERSION,
            tick_hz = crate::timing::SIMULATION_TICK_HZ,
            client_id = config.client_id,
            server = %config.server_addr,
            generation,
            "brawler client connecting to lobby selector"
        );
        return Ok(());
    }
    if config.transport != NetworkTransport::Udp {
        return Ok(());
    }
    spawn_client_entity(&mut commands, &config, time.elapsed(), None)?;
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

/// Spawn exactly one fresh Lightyear client entity. `routed` is either a lobby/match adapter and
/// generation marker or `None` for the unchanged direct-UDP baseline.
pub(super) fn spawn_client_entity(
    commands: &mut Commands,
    config: &ClientNetworkConfig,
    started_at: Duration,
    routed: Option<(RoutedUdpIo, RoutedClientSession)>,
) -> Result<Entity> {
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
    let netcode = NetcodeClient::new(auth, netcode_config)?;
    let status = ClientJoinStatus {
        phase: ClientJoinPhase::Connecting,
        started_at,
        disconnect_requested: false,
    };
    let entity = if let Some((io, session)) = routed {
        commands
            .spawn((
                status,
                session,
                PingManager::default(),
                ReplicationReceiver,
                Link::default()
                    .with_mtu(LinkMtu::new(ROUTED_LINK_MTU))
                    .with_conditioner(config.impairment_profile.receive_conditioner()),
                netcode,
                LocalAddr(config.local_addr),
                PeerAddr(config.server_addr),
                io,
                PendingClientConnect,
                Name::new(format!("Brawler routed client {}", config.client_id)),
            ))
            .id()
    } else {
        commands
            .spawn((
                status,
                PingManager::default(),
                ReplicationReceiver,
                Link::default().with_conditioner(config.impairment_profile.receive_conditioner()),
                netcode,
                LocalAddr(config.local_addr),
                PeerAddr(config.server_addr),
                UdpIo::default(),
                PendingClientConnect,
                Name::new(format!("Brawler client {}", config.client_id)),
            ))
            .id()
    };
    Ok(entity)
}

/// Start a client only after its deferred spawn has reached the world. This system is first in
/// the session chain, so connection observers complete before later session observation while
/// preserving the normal Lightyear receive/send schedules.
#[allow(clippy::needless_pass_by_value)]
pub(super) fn connect_spawned_clients(
    mut commands: Commands,
    clients: Query<Entity, Added<PendingClientConnect>>,
) {
    for entity in &clients {
        // NetcodeClient's required initial lifecycle markers are useful for a statically spawned
        // endpoint, but a product-shell entity lives for one deferred boundary before this
        // system runs. Clear those initial markers first so they cannot be observed as a real
        // failed attempt before `Connect` installs `Connecting` and the routed socket's `Linked`.
        commands.entity(entity).remove::<(Unlinked, Disconnected)>();
        commands.trigger(Connect { entity });
    }
}

#[allow(clippy::needless_pass_by_value)]
pub(super) fn finish_spawned_client_connect(
    mut commands: Commands,
    clients: Query<Entity, (With<PendingClientConnect>, With<Connecting>)>,
) {
    for entity in &clients {
        commands.entity(entity).remove::<PendingClientConnect>();
    }
}

pub(in crate::client) struct ProductLobbyAttempt {
    pub started_at: Duration,
    pub server_addr: std::net::SocketAddr,
    pub logical_address: String,
    pub proposed_display_name: String,
    pub netcode_timeout: Duration,
}

pub(in crate::client) fn spawn_product_lobby_connection(
    commands: &mut Commands,
    config: &ClientNetworkConfig,
    routed: &mut RoutedClientLifecycle,
    attempt: ProductLobbyAttempt,
) -> Result<Entity> {
    let mut attempt_config = config.clone();
    attempt_config.server_addr = attempt.server_addr;
    attempt_config.local_addr = match attempt.server_addr {
        std::net::SocketAddr::V4(_) => "0.0.0.0:0".parse().expect("wildcard IPv4 address is valid"),
        std::net::SocketAddr::V6(_) => "[::]:0".parse().expect("wildcard IPv6 address is valid"),
    };
    attempt_config.connect_timeout = attempt.netcode_timeout;
    routed.start_lobby();
    let generation = routed.generation;
    let entity = spawn_client_entity(
        commands,
        &attempt_config,
        attempt.started_at,
        Some((
            RoutedUdpIo::lobby(attempt.server_addr),
            RoutedClientSession {
                generation,
                kind: RoutedClientSessionKind::Lobby,
            },
        )),
    )?;
    commands.entity(entity).insert(RuntimeLobbyTarget {
        logical_address: attempt.logical_address,
        proposed_display_name: attempt.proposed_display_name,
    });
    Ok(entity)
}
