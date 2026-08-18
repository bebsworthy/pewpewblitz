//! Routed public-UDP IO for the client-side Lightyear link.
//!
//! This is deliberately a thin datagram adapter.  Routing metadata is decoded at the public
//! socket boundary and the selected envelope payload is handed to Lightyear unchanged.  Netcode,
//! packetization, reliability, ordering, and authentication remain above this module.

use std::{
    collections::VecDeque,
    fmt,
    io::ErrorKind,
    net::{SocketAddr, UdpSocket},
};

use bevy::{
    app::{App, Plugin, PostUpdate, PreUpdate},
    ecs::prelude::*,
    log::warn,
};
use brawler_routing::{PUBLIC_MAX_DATAGRAM_BYTES, PublicEnvelope, ROUTED_LINK_MTU, RouteSelector};
use lightyear::prelude::LocalAddr;
use lightyear::{
    core::time::Instant,
    link::{
        Link, LinkMtu, LinkPlugin, LinkReceiveSystems, LinkStart, LinkSystems, Linked, Linking,
        RecvPayload, SendPayload, Unlink, UnlinkReason, Unlinked,
    },
};

const RECEIVE_BUFFER_BYTES: usize = PUBLIC_MAX_DATAGRAM_BYTES + 1;
const MAX_RECEIVED_DATAGRAMS_PER_UPDATE: usize = 64;
/// Maximum number of already-Netcode-wrapped payloads retained by one routed client adapter.
///
/// `RoutedUdpIo::send` runs after Lightyear's Netcode send stage. Payloads must therefore leave
/// `Link::send` in the same frame they are produced; returning a would-block payload to that
/// queue would make Netcode encrypt it a second time on the next frame. The adapter-owned queue
/// is bounded and drops newest payloads on overflow, preserving FIFO order for the payloads that
/// remain.
const MAX_PENDING_SEND_PAYLOADS: usize = 128;

fn routed_link() -> Link {
    Link::default().with_mtu(LinkMtu::new(ROUTED_LINK_MTU))
}

/// Client-side public UDP transport that wraps each Lightyear datagram in a route envelope.
///
/// The selector is intentionally retained as a redacted [`RouteSelector`].  `Capability`'s
/// `Debug` and `Display` implementations never expose its bytes, so logging this component is
/// safe.  A lobby link uses [`RoutedUdpIo::lobby`]; a match link uses
/// [`RoutedUdpIo::with_match_capability`].
#[derive(Component)]
#[require(Link = routed_link())]
pub struct RoutedUdpIo {
    socket: Option<UdpSocket>,
    public_peer: SocketAddr,
    selector: RouteSelector,
    pending_sends: VecDeque<SendPayload>,
}

impl fmt::Debug for RoutedUdpIo {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("RoutedUdpIo")
            .field("public_peer", &self.public_peer)
            .field("selector", &self.selector)
            .field("bound", &self.socket.is_some())
            .field("pending_sends", &self.pending_sends.len())
            .finish()
    }
}

impl RoutedUdpIo {
    /// Construct a routed client using the all-zero lobby selector.
    #[must_use]
    pub fn lobby(public_peer: SocketAddr) -> Self {
        Self::new(public_peer, RouteSelector::DefaultLobby)
    }

    /// Construct a routed client using an installed match capability.
    #[must_use]
    pub fn with_match_capability(
        public_peer: SocketAddr,
        capability: brawler_routing::Capability,
    ) -> Self {
        Self::new(public_peer, RouteSelector::Capability(capability))
    }

    /// Construct a routed client with an explicit selector.
    #[must_use]
    pub fn new(public_peer: SocketAddr, selector: RouteSelector) -> Self {
        Self {
            socket: None,
            public_peer,
            selector,
            pending_sends: VecDeque::new(),
        }
    }

    /// Install the capability received from the authenticated lobby allocation grant.
    pub fn install_match_capability(&mut self, capability: brawler_routing::Capability) {
        self.selector = RouteSelector::Capability(capability);
    }

    /// Returns the redacted route selector.
    #[must_use]
    pub const fn selector(&self) -> &RouteSelector {
        &self.selector
    }

    /// Returns the public supervisor address.
    #[must_use]
    pub const fn public_peer(&self) -> SocketAddr {
        self.public_peer
    }

    /// Returns the bound local address after [`LinkStart`] has succeeded.
    #[must_use]
    pub fn local_addr(&self) -> Option<SocketAddr> {
        self.socket
            .as_ref()
            .and_then(|socket| socket.local_addr().ok())
    }

    #[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
    fn bind(
        trigger: On<LinkStart>,
        mut query: Query<
            (&mut RoutedUdpIo, &mut Link, Option<&LocalAddr>),
            (Without<Linking>, Without<Linked>),
        >,
        mut commands: Commands,
    ) {
        let Ok((mut routed, mut link, local_addr)) = query.get_mut(trigger.entity) else {
            return;
        };
        let Some(local_addr) = local_addr.map(|address| address.0) else {
            commands.trigger(Unlink {
                entity: trigger.entity,
                reason: UnlinkReason::TransportError(
                    "routed UDP local address is missing".to_owned(),
                ),
            });
            return;
        };
        let Ok(socket) = UdpSocket::bind(local_addr)
            .and_then(|socket| socket.set_nonblocking(true).map(|()| socket))
        else {
            commands.trigger(Unlink {
                entity: trigger.entity,
                reason: UnlinkReason::TransportError("routed UDP bind failed".to_owned()),
            });
            return;
        };

        // NetcodeClient may have supplied a default Link before this transport was inserted.
        // LinkStart is the transport boundary, so normalize it here while no session is active.
        *link = routed_link();
        routed.socket = Some(socket);
        commands.entity(trigger.entity).insert(Linked);
    }

    #[allow(clippy::needless_pass_by_value)]
    fn unlink(trigger: On<Unlink>, mut query: Query<&mut RoutedUdpIo, Without<Unlinked>>) {
        if let Ok(mut routed) = query.get_mut(trigger.entity) {
            routed.socket = None;
            routed.pending_sends.clear();
        }
    }

    /// Move every post-Netcode payload out of Lightyear's link queue before attempting IO.
    ///
    /// This is intentionally separate from the socket loop so the ownership and overflow rule
    /// remain testable without depending on OS UDP-buffer behaviour. The existing adapter FIFO
    /// wins over newly produced payloads; once full, newest payloads are dropped after they have
    /// been removed from `Link::send`.
    fn absorb_post_netcode_payloads(
        link: &mut Link,
        pending_sends: &mut VecDeque<SendPayload>,
    ) -> usize {
        let mut dropped = 0;
        for payload in link.send.drain() {
            if pending_sends.len() < MAX_PENDING_SEND_PAYLOADS {
                pending_sends.push_back(payload);
            } else {
                dropped += 1;
            }
        }
        dropped
    }

    fn receive(
        mut query: Query<(Entity, &mut Link, &mut RoutedUdpIo), With<Linked>>,
        mut commands: Commands,
    ) {
        for (entity, mut link, mut routed) in &mut query {
            let public_peer = routed.public_peer;
            let selector = routed.selector.clone();
            let Some(socket) = routed.socket.take() else {
                continue;
            };
            let mut buffer = [0_u8; RECEIVE_BUFFER_BYTES];
            for _ in 0..MAX_RECEIVED_DATAGRAMS_PER_UPDATE {
                match socket.recv_from(&mut buffer) {
                    Ok((length, source)) => {
                        if length > PUBLIC_MAX_DATAGRAM_BYTES || source != public_peer {
                            continue;
                        }
                        let Ok(envelope) = PublicEnvelope::decode(&buffer[..length]) else {
                            continue;
                        };
                        if envelope.selector() != &selector {
                            continue;
                        }
                        // BufferToLink runs before LinkPlugin's conditioner stage.  Pushing
                        // through `push` preserves configured delay/loss semantics.
                        link.recv
                            .push(RecvPayload::from(envelope.payload()), Instant::now());
                    }
                    Err(error) if error.kind() == ErrorKind::WouldBlock => break,
                    Err(error) => {
                        warn!("routed UDP receive failed: {error}");
                        commands.trigger(Unlink {
                            entity,
                            reason: UnlinkReason::TransportError(
                                "routed UDP receive failed".to_owned(),
                            ),
                        });
                        break;
                    }
                }
            }
            routed.socket = Some(socket);
        }
    }

    fn send(
        mut query: Query<(Entity, &mut Link, &mut RoutedUdpIo), With<Linked>>,
        mut commands: Commands,
    ) {
        for (entity, mut link, mut routed) in &mut query {
            let public_peer = routed.public_peer;
            let selector = routed.selector.clone();
            let Some(socket) = routed.socket.take() else {
                continue;
            };
            let dropped = Self::absorb_post_netcode_payloads(&mut link, &mut routed.pending_sends);
            if dropped > 0 {
                warn!(
                    dropped,
                    limit = MAX_PENDING_SEND_PAYLOADS,
                    "routed UDP post-Netcode send queue dropped newest payloads"
                );
            }
            while let Some(payload) = routed.pending_sends.pop_front() {
                let Ok(envelope) = PublicEnvelope::new(selector.clone(), payload.to_vec()) else {
                    continue;
                };
                let Ok(datagram) = envelope.encode() else {
                    continue;
                };
                match socket.send_to(&datagram, public_peer) {
                    Err(error) if error.kind() == ErrorKind::WouldBlock => {
                        // UDP datagrams are atomic: retain the inner datagram in the adapter FIFO
                        // and stop flushing this link so FIFO order and the next frame's
                        // backpressure survive. It must never return to Link::send: this payload
                        // has already passed through Netcode.
                        routed.pending_sends.push_front(payload);
                        break;
                    }
                    Ok(_) => {}
                    Err(_) => {
                        commands.trigger(Unlink {
                            entity,
                            reason: UnlinkReason::TransportError(
                                "routed UDP send failed".to_owned(),
                            ),
                        });
                        break;
                    }
                }
            }
            routed.socket = Some(socket);
        }
    }
}

/// Bevy plugin integrating [`RoutedUdpIo`] at Lightyear's raw link IO seam.
pub struct RoutedUdpPlugin;

impl Plugin for RoutedUdpPlugin {
    fn build(&self, app: &mut App) {
        if !app.is_plugin_added::<LinkPlugin>() {
            app.add_plugins(LinkPlugin);
        }
        app.add_observer(RoutedUdpIo::bind);
        app.add_observer(RoutedUdpIo::unlink);
        app.add_systems(
            PreUpdate,
            RoutedUdpIo::receive.in_set(LinkReceiveSystems::BufferToLink),
        );
        app.add_systems(PostUpdate, RoutedUdpIo::send.in_set(LinkSystems::Send));
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::app::App;
    use lightyear::link::{LinkStart, UnlinkReason};
    use lightyear::prelude::PeerAddr;
    use std::time::Duration;

    fn app_with_link(peer: SocketAddr, selector: RouteSelector) -> (App, Entity) {
        let local = match peer {
            SocketAddr::V4(_) => "127.0.0.1:0".parse().unwrap(),
            SocketAddr::V6(_) => "[::1]:0".parse().unwrap(),
        };
        app_with_link_at(local, peer, selector)
    }

    fn app_with_link_at(
        local: SocketAddr,
        peer: SocketAddr,
        selector: RouteSelector,
    ) -> (App, Entity) {
        let mut app = App::new();
        app.add_plugins(RoutedUdpPlugin);
        let entity = app
            .world_mut()
            .spawn((
                LocalAddr(local),
                PeerAddr(peer),
                RoutedUdpIo::new(peer, selector),
            ))
            .id();
        app.world_mut().trigger(LinkStart { entity });
        app.update();
        (app, entity)
    }

    fn bound_addr(app: &App, entity: Entity) -> SocketAddr {
        app.world()
            .get::<RoutedUdpIo>(entity)
            .and_then(RoutedUdpIo::local_addr)
            .unwrap()
    }

    #[test]
    fn link_start_installs_exact_routed_mtu() {
        let peer = "127.0.0.1:9".parse().unwrap();
        let (app, entity) = app_with_link(peer, RouteSelector::DefaultLobby);
        assert!(app.world().get::<Linked>(entity).is_some());
        assert_eq!(
            app.world().get::<Link>(entity).unwrap().mtu(),
            ROUTED_LINK_MTU
        );
    }

    #[test]
    fn malformed_and_wrong_selector_datagrams_are_dropped() {
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        sender
            .set_read_timeout(Some(Duration::from_millis(100)))
            .unwrap();
        let peer = sender.local_addr().unwrap();
        let (mut app, entity) = app_with_link(peer, RouteSelector::DefaultLobby);
        let local = bound_addr(&app, entity);

        sender.send_to(b"not-an-envelope", local).unwrap();
        let capability = brawler_routing::Capability::from_bytes([7; 32]).unwrap();
        let wrong = PublicEnvelope::new(RouteSelector::Capability(capability), vec![1]).unwrap();
        sender.send_to(&wrong.encode().unwrap(), local).unwrap();
        sender
            .send_to(&vec![0; PUBLIC_MAX_DATAGRAM_BYTES + 1], local)
            .unwrap();
        app.update();
        assert_eq!(app.world().get::<Link>(entity).unwrap().recv.len(), 0);
    }

    #[test]
    fn loopback_receive_and_send_preserve_inner_datagrams() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server.set_nonblocking(true).unwrap();
        let peer = server.local_addr().unwrap();
        let (mut app, entity) = app_with_link(peer, RouteSelector::DefaultLobby);
        let local = bound_addr(&app, entity);
        let inbound = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![1, 2, 3])
            .unwrap()
            .encode()
            .unwrap();
        server.send_to(&inbound, local).unwrap();
        let received = (0..4).find_map(|_| {
            app.update();
            app.world_mut()
                .get_mut::<Link>(entity)
                .and_then(|mut link| link.recv.pop())
        });
        let received = received.expect("loopback payload should arrive within bounded updates");
        assert_eq!(received.as_ref(), &[1, 2, 3]);

        app.world_mut()
            .get_mut::<Link>(entity)
            .unwrap()
            .send
            .push(SendPayload::from(&[4, 5, 6][..]));
        let mut buffer = [0; PUBLIC_MAX_DATAGRAM_BYTES];
        let mut datagram = None;
        for _ in 0..4 {
            app.update();
            if let Ok((length, source)) = server.recv_from(&mut buffer) {
                datagram = Some((length, source));
                break;
            }
        }
        let (length, source) =
            datagram.expect("loopback envelope should arrive within bounded updates");
        assert_eq!(source, local);
        let envelope = PublicEnvelope::decode(&buffer[..length]).unwrap();
        assert_eq!(envelope.payload(), &[4, 5, 6]);
    }

    #[test]
    fn ipv6_loopback_receive_and_send_preserve_inner_datagrams() {
        let server = UdpSocket::bind("[::1]:0").unwrap();
        server.set_nonblocking(true).unwrap();
        let peer = server.local_addr().unwrap();
        assert!(peer.is_ipv6());
        let (mut app, entity) = app_with_link(peer, RouteSelector::DefaultLobby);
        let local = bound_addr(&app, entity);
        assert!(local.is_ipv6());
        let inbound = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![1, 2, 3])
            .unwrap()
            .encode()
            .unwrap();
        server.send_to(&inbound, local).unwrap();
        let received = (0..4).find_map(|_| {
            app.update();
            app.world_mut()
                .get_mut::<Link>(entity)
                .and_then(|mut link| link.recv.pop())
        });
        let received =
            received.expect("IPv6 loopback payload should arrive within bounded updates");
        assert_eq!(received.as_ref(), &[1, 2, 3]);

        app.world_mut()
            .get_mut::<Link>(entity)
            .unwrap()
            .send
            .push(SendPayload::from(&[4, 5, 6][..]));
        let mut buffer = [0; PUBLIC_MAX_DATAGRAM_BYTES];
        let mut datagram = None;
        for _ in 0..4 {
            app.update();
            if let Ok((length, source)) = server.recv_from(&mut buffer) {
                datagram = Some((length, source));
                break;
            }
        }
        let (length, source) =
            datagram.expect("IPv6 loopback envelope should arrive within bounded updates");
        assert_eq!(source, local);
        assert!(source.is_ipv6());
        let envelope = PublicEnvelope::decode(&buffer[..length]).unwrap();
        assert_eq!(envelope.payload(), &[4, 5, 6]);
    }

    #[test]
    fn routed_link_mtu_payload_stays_within_public_datagram_limit() {
        let server = UdpSocket::bind("127.0.0.1:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let peer = server.local_addr().unwrap();
        let (mut app, entity) = app_with_link(peer, RouteSelector::DefaultLobby);
        let payload = vec![0xa5; ROUTED_LINK_MTU];
        let expected = payload.clone();
        app.world_mut()
            .get_mut::<Link>(entity)
            .unwrap()
            .send
            .push(SendPayload::from(payload));
        app.update();

        let mut buffer = [0; PUBLIC_MAX_DATAGRAM_BYTES];
        let (length, _) = server.recv_from(&mut buffer).unwrap();
        assert_eq!(
            length,
            brawler_routing::PUBLIC_HEADER_BYTES + ROUTED_LINK_MTU
        );
        assert!(length <= PUBLIC_MAX_DATAGRAM_BYTES);
        assert_eq!(
            PublicEnvelope::decode(&buffer[..length]).unwrap().payload(),
            expected
        );
    }

    #[test]
    fn ipv6_routed_link_mtu_payload_stays_within_public_datagram_limit() {
        let server = UdpSocket::bind("[::1]:0").unwrap();
        server
            .set_read_timeout(Some(Duration::from_secs(1)))
            .unwrap();
        let peer = server.local_addr().unwrap();
        let (mut app, entity) = app_with_link(peer, RouteSelector::DefaultLobby);
        let payload = vec![0xa5; ROUTED_LINK_MTU];
        let expected = payload.clone();
        app.world_mut()
            .get_mut::<Link>(entity)
            .unwrap()
            .send
            .push(SendPayload::from(payload));
        app.update();

        let mut buffer = [0; PUBLIC_MAX_DATAGRAM_BYTES];
        let (length, source) = server.recv_from(&mut buffer).unwrap();
        assert!(source.is_ipv6());
        assert_eq!(
            length,
            brawler_routing::PUBLIC_HEADER_BYTES + ROUTED_LINK_MTU
        );
        assert!(length <= PUBLIC_MAX_DATAGRAM_BYTES);
        assert_eq!(
            PublicEnvelope::decode(&buffer[..length]).unwrap().payload(),
            expected
        );
    }

    #[test]
    fn post_netcode_send_fifo_drains_link_and_drops_newest_on_overflow() {
        let mut link = Link::default();
        let mut pending = VecDeque::from([SendPayload::from(1_u16.to_be_bytes().to_vec())]);
        let max_pending = u16::try_from(MAX_PENDING_SEND_PAYLOADS).unwrap();
        for value in 2..=(max_pending + 1) {
            link.send
                .push(SendPayload::from(value.to_be_bytes().to_vec()));
        }

        let dropped = RoutedUdpIo::absorb_post_netcode_payloads(&mut link, &mut pending);

        assert_eq!(dropped, 1);
        assert_eq!(link.send.len(), 0);
        assert_eq!(pending.len(), MAX_PENDING_SEND_PAYLOADS);
        assert_eq!(pending.front().unwrap().as_ref(), &[0, 1]);
        assert_eq!(pending.back().unwrap().as_ref(), &max_pending.to_be_bytes());
    }

    #[test]
    fn capability_selector_is_redacted_and_can_be_installed() {
        let capability = brawler_routing::Capability::from_bytes([0x5a; 32]).unwrap();
        let mut io = RoutedUdpIo::lobby("127.0.0.1:9".parse().unwrap());
        io.install_match_capability(capability);
        let debug = format!("{io:?}");
        assert!(debug.contains("Capability([REDACTED])"));
        assert!(!debug.contains("5a"));
        assert!(matches!(io.selector(), RouteSelector::Capability(_)));
    }

    #[test]
    fn unlink_closes_socket_and_marks_link_unlinked() {
        let peer = "127.0.0.1:9".parse().unwrap();
        let (mut app, entity) = app_with_link(peer, RouteSelector::DefaultLobby);
        app.world_mut().trigger(Unlink {
            entity,
            reason: UnlinkReason::UserRequested(None),
        });
        app.update();
        assert!(app.world().get::<Unlinked>(entity).is_some());
        assert!(
            app.world()
                .get::<RoutedUdpIo>(entity)
                .unwrap()
                .local_addr()
                .is_none()
        );
    }

    #[test]
    fn link_start_without_local_addr_reports_transport_error() {
        let mut app = App::new();
        app.add_plugins(RoutedUdpPlugin);
        let entity = app
            .world_mut()
            .spawn((
                PeerAddr("127.0.0.1:9".parse().unwrap()),
                RoutedUdpIo::lobby("127.0.0.1:9".parse().unwrap()),
            ))
            .id();
        app.world_mut().trigger(LinkStart { entity });
        app.update();

        let unlinked = app.world().get::<Unlinked>(entity).unwrap();
        assert!(matches!(
            unlinked.reason,
            UnlinkReason::TransportError(ref reason)
                if reason.contains("local address is missing")
        ));
    }

    #[test]
    fn link_start_bind_failure_reports_transport_error() {
        let blocker = UdpSocket::bind("127.0.0.1:0").unwrap();
        let occupied = blocker.local_addr().unwrap();
        let mut app = App::new();
        app.add_plugins(RoutedUdpPlugin);
        let entity = app
            .world_mut()
            .spawn((
                LocalAddr(occupied),
                PeerAddr("127.0.0.1:9".parse().unwrap()),
                RoutedUdpIo::lobby("127.0.0.1:9".parse().unwrap()),
            ))
            .id();
        app.world_mut().trigger(LinkStart { entity });
        app.update();

        let unlinked = app.world().get::<Unlinked>(entity).unwrap();
        assert!(matches!(
            unlinked.reason,
            UnlinkReason::TransportError(ref reason) if reason.contains("bind failed")
        ));
    }

    #[test]
    fn receive_is_bounded_to_sixty_four_datagrams_per_update() {
        let sender = UdpSocket::bind("127.0.0.1:0").unwrap();
        let peer = sender.local_addr().unwrap();
        let (mut app, entity) = app_with_link(peer, RouteSelector::DefaultLobby);
        let local = bound_addr(&app, entity);
        let datagram = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![1])
            .unwrap()
            .encode()
            .unwrap();
        for _ in 0..=MAX_RECEIVED_DATAGRAMS_PER_UPDATE {
            sender.send_to(&datagram, local).unwrap();
        }

        app.update();
        assert_eq!(
            app.world().get::<Link>(entity).unwrap().recv.len(),
            MAX_RECEIVED_DATAGRAMS_PER_UPDATE
        );
        app.update();
        assert_eq!(app.world().get::<Link>(entity).unwrap().recv.len(), 65);
    }
}
