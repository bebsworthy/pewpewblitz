use std::{
    net::UdpSocket,
    time::{Duration, Instant},
};

use mio::net::UnixStream;

use crate::{
    AllocateRequestBody, AllocationId, AllocationRejectedBody, ControlFrame, GameMode, Generation,
    LobbySessionId, MatchId, MonotonicMillis, PacketDirection, PacketRecord, PeerCloseBody, PeerId,
    ProcessId, PublicEnvelope, RequestId, ResultBody, RouteId, RouteRegistration, RouteSelector,
    UnixWorkerChannels, WorkerId, WorkerKind, WorkerRegistration,
};

use super::*;

fn id128(value: u128) -> WorkerId {
    WorkerId::new(value).unwrap()
}

fn registration(worker_id: u128, kind: WorkerKind) -> WorkerRegistration {
    WorkerRegistration {
        worker_id: id128(worker_id),
        process_id: ProcessId::new(worker_id + 100).unwrap(),
        generation: Generation::new(1).unwrap(),
        kind,
    }
}

fn route(route_id: u128, worker_id: WorkerId, default: bool) -> RouteRegistration {
    RouteRegistration {
        route_id: RouteId::new(route_id).unwrap(),
        worker_id,
        peer_id: PeerId::new(route_id + 1000).unwrap(),
        is_default_lobby: default,
    }
}

fn attach_worker(
    runtime: &mut SupervisorRuntime,
    worker: WorkerRegistration,
) -> UnixWorkerChannels {
    let (supervisor_packet, worker_packet) = UnixStream::pair().unwrap();
    let (supervisor_control, worker_control) = UnixStream::pair().unwrap();
    runtime
        .attach_worker_channels(
            worker.worker_id,
            UnixWorkerChannels::new(supervisor_packet, supervisor_control),
        )
        .unwrap();
    UnixWorkerChannels::new(worker_packet, worker_control)
}

fn send_worker_control(
    worker: &mut UnixWorkerChannels,
    registration: WorkerRegistration,
    sequence: u64,
    body: ControlBody,
) {
    let frame = ControlFrame::from_raw_sequence(
        sequence,
        registration.process_id,
        registration.worker_id,
        body,
    )
    .unwrap()
    .encode_framed()
    .unwrap();
    worker.enqueue_control(&frame).unwrap();
    worker.flush_control(1).unwrap();
}

#[test]
fn public_udp_routes_opaque_payload_to_packet_ipc_and_back() {
    let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let worker = registration(1, WorkerKind::Lobby);
    let default_route = route(2, worker.worker_id, true);
    runtime.register_worker(worker).unwrap();
    runtime.core_mut().register_route(default_route).unwrap();
    let (supervisor_packet, worker_packet) = UnixStream::pair().unwrap();
    let (supervisor_control, worker_control) = UnixStream::pair().unwrap();
    runtime
        .attach_worker_channels(
            worker.worker_id,
            UnixWorkerChannels::new(supervisor_packet, supervisor_control),
        )
        .unwrap();
    let mut worker_channels = UnixWorkerChannels::new(worker_packet, worker_control);
    let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    client
        .set_nonblocking(true)
        .expect("client socket should be nonblocking");
    let public = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![7, 8, 9])
        .unwrap()
        .encode()
        .unwrap();
    client
        .send_to(&public, runtime.public_addr().unwrap())
        .unwrap();
    runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
    let worker_packet = worker_channels
        .packet_read_ready(1)
        .unwrap()
        .records
        .pop()
        .expect("packet reached worker");
    let decoded =
        PacketRecord::decode(&worker_packet, PacketDirection::SupervisorToWorker).unwrap();
    assert_eq!(decoded.payload, vec![7, 8, 9]);
    let response = PacketRecord::new(
        PacketDirection::WorkerToSupervisor,
        worker.worker_id,
        decoded.route_id,
        decoded.peer_id,
        vec![4, 5],
    )
    .unwrap()
    .encode()
    .unwrap();
    worker_channels.enqueue_packet(&response).unwrap();
    worker_channels.flush_packet(1).unwrap();
    for _ in 0..3 {
        runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
    }
    let mut received = [0; crate::PUBLIC_MAX_DATAGRAM_BYTES];
    let (length, _) = client.recv_from(&mut received).unwrap();
    assert_eq!(
        PublicEnvelope::decode(&received[..length])
            .unwrap()
            .payload(),
        &[4, 5]
    );
    let routing = runtime.metrics();
    assert_eq!(routing.public_ingress.datagrams, 1);
    assert_eq!(routing.public_ingress.frames, 1);
    assert_eq!(routing.public_ingress.bytes, public.len() as u64);
    assert_eq!(routing.inner_ingress.datagrams, 1);
    assert_eq!(routing.inner_ingress.frames, 1);
    assert_eq!(routing.inner_ingress.bytes, 3);
    assert_eq!(routing.ipc_to_worker.frames, 1);
    assert_eq!(
        routing.ipc_to_worker.bytes,
        (worker_packet.len() + 4) as u64
    );
    assert_eq!(routing.ipc_from_worker.frames, 1);
    assert_eq!(routing.ipc_from_worker.bytes, (response.len() + 4) as u64);
    assert_eq!(routing.public_egress.datagrams, 1);
    assert_eq!(routing.public_egress.frames, 1);
    assert_eq!(routing.public_egress.bytes, received[..length].len() as u64);
    assert_eq!(routing.inner_egress.datagrams, 1);
    assert_eq!(routing.inner_egress.frames, 1);
    assert_eq!(routing.inner_egress.bytes, 2);
    assert_eq!(routing.public_receive_to_packet_ipc_enqueue.count(), 1);
    assert_eq!(routing.worker_packet_to_public_send.count(), 1);
}

/// BRL-q73k: when a peer's route expires, the worker may already have written packets for it
/// before it reads the `PeerClose`. Those packets are dropped; they must not fail (and, for a
/// lifecycle-owned worker, SIGKILL) the worker that still serves its other peers.
#[test]
fn worker_packets_racing_a_route_expiry_are_dropped_without_failing_the_worker() {
    let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let worker = registration(1, WorkerKind::Lobby);
    runtime.register_worker(worker).unwrap();
    runtime
        .core_mut()
        .register_route(route(2, worker.worker_id, true))
        .unwrap();
    let mut worker_channels = attach_worker(&mut runtime, worker);
    let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let public = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![7])
        .unwrap()
        .encode()
        .unwrap();
    client
        .send_to(&public, runtime.public_addr().unwrap())
        .unwrap();
    runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
    let inbound = worker_channels
        .packet_read_ready(1)
        .unwrap()
        .records
        .pop()
        .expect("packet reached worker");
    let inbound = PacketRecord::decode(&inbound, PacketDirection::SupervisorToWorker).unwrap();
    assert_eq!(
        runtime.core().route_count(),
        2,
        "the source has its own route"
    );

    // The source goes idle and its route expires; the PeerClose is queued to the worker.
    runtime.started = Instant::now()
        .checked_sub(Duration::from_millis(
            crate::PUBLIC_LOBBY_ROUTE_IDLE_MILLIS + 1,
        ))
        .expect("idle-route duration fits before the current instant");
    runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
    assert_eq!(runtime.core().route_count(), 1, "the idle route expired");

    // A packet the worker wrote before it read that PeerClose arrives afterwards.
    let late = PacketRecord::new(
        PacketDirection::WorkerToSupervisor,
        worker.worker_id,
        inbound.route_id,
        inbound.peer_id,
        vec![4],
    )
    .unwrap()
    .encode()
    .unwrap();
    worker_channels.enqueue_packet(&late).unwrap();
    worker_channels.flush_packet(1).unwrap();
    for _ in 0..3 {
        runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
    }
    assert!(
        runtime.workers.contains_key(&worker.worker_id),
        "the worker is not cleaned up for a late packet"
    );
    assert_eq!(runtime.core().worker_count(), 1);
    assert_eq!(
        runtime
            .core()
            .metrics()
            .error_counts
            .get(&RoutingErrorCategory::RouteExpired)
            .copied(),
        Some(1),
        "the late packet is counted as expired"
    );
    assert_eq!(runtime.metrics().public_egress.datagrams, 0);
}

#[test]
fn public_ipv6_udp_routes_dynamic_source_and_opaque_payload_to_packet_ipc_and_back() {
    let mut runtime =
        SupervisorRuntime::bind(SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], 0))).unwrap();
    let worker = registration(1, WorkerKind::Lobby);
    let default_route = route(2, worker.worker_id, true);
    runtime.register_worker(worker).unwrap();
    runtime.core_mut().register_route(default_route).unwrap();
    let (supervisor_packet, worker_packet) = UnixStream::pair().unwrap();
    let (supervisor_control, worker_control) = UnixStream::pair().unwrap();
    runtime
        .attach_worker_channels(
            worker.worker_id,
            UnixWorkerChannels::new(supervisor_packet, supervisor_control),
        )
        .unwrap();
    let mut worker_channels = UnixWorkerChannels::new(worker_packet, worker_control);
    let client = UdpSocket::bind(SocketAddr::from(([0, 0, 0, 0, 0, 0, 0, 1], 0))).unwrap();
    client
        .set_nonblocking(true)
        .expect("IPv6 client socket should be nonblocking");
    let public = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![7, 8, 9])
        .unwrap()
        .encode()
        .unwrap();
    client
        .send_to(&public, runtime.public_addr().unwrap())
        .unwrap();
    runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
    let worker_packet = worker_channels
        .packet_read_ready(1)
        .unwrap()
        .records
        .pop()
        .expect("IPv6 packet reached worker");
    let decoded =
        PacketRecord::decode(&worker_packet, PacketDirection::SupervisorToWorker).unwrap();
    assert_eq!(decoded.payload, vec![7, 8, 9]);
    assert_eq!(
        runtime.core().source_for_route(decoded.route_id),
        Some(client.local_addr().unwrap())
    );
    assert!(
        runtime
            .core()
            .source_for_route(decoded.route_id)
            .is_some_and(|source| source.is_ipv6())
    );
    let response = PacketRecord::new(
        PacketDirection::WorkerToSupervisor,
        worker.worker_id,
        decoded.route_id,
        decoded.peer_id,
        vec![4, 5],
    )
    .unwrap()
    .encode()
    .unwrap();
    worker_channels.enqueue_packet(&response).unwrap();
    worker_channels.flush_packet(1).unwrap();
    for _ in 0..3 {
        runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
    }
    let mut received = [0; crate::PUBLIC_MAX_DATAGRAM_BYTES];
    let (length, source) = client.recv_from(&mut received).unwrap();
    assert!(source.is_ipv6());
    assert_eq!(source, runtime.public_addr().unwrap());
    assert_eq!(
        PublicEnvelope::decode(&received[..length])
            .unwrap()
            .payload(),
        &[4, 5]
    );
    assert_eq!(
        runtime.core().source_for_route(decoded.route_id),
        Some(client.local_addr().unwrap())
    );
}

#[test]
fn public_traffic_before_lobby_ready_is_dropped_without_poll_failure() {
    let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let worker = registration(1, WorkerKind::Lobby);
    runtime.register_worker(worker).unwrap();
    let (supervisor_packet, _worker_packet) = UnixStream::pair().unwrap();
    let (supervisor_control, _worker_control) = UnixStream::pair().unwrap();
    runtime
        .attach_worker_channels(
            worker.worker_id,
            UnixWorkerChannels::new(supervisor_packet, supervisor_control),
        )
        .unwrap();
    let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let envelope = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![1, 2, 3])
        .unwrap()
        .encode()
        .unwrap();
    client
        .send_to(&envelope, runtime.public_addr().unwrap())
        .unwrap();

    let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
    assert_eq!(report.public_received, 1);
    assert_eq!(report.public_dropped, 1);
    assert_eq!(runtime.core().route_count(), 0);
    assert_eq!(runtime.core().metrics().packet_current.frames, 0);
}

#[test]
fn lobby_startup_retries_do_not_spend_preauth_budget_before_ready() {
    let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let worker = registration(1, WorkerKind::Lobby);
    runtime.register_worker(worker).unwrap();
    let (supervisor_packet, _worker_packet) = UnixStream::pair().unwrap();
    let (supervisor_control, _worker_control) = UnixStream::pair().unwrap();
    runtime
        .attach_worker_channels(
            worker.worker_id,
            UnixWorkerChannels::new(supervisor_packet, supervisor_control),
        )
        .unwrap();
    let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let envelope = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![1; 1])
        .unwrap()
        .encode()
        .unwrap();

    // The route is not published until Ready. These retries are dropped without consuming
    // the source's pre-auth handshake allowance.
    for _ in 0..crate::PUBLIC_PREAUTH_HANDSHAKE_DATAGRAMS_PER_WINDOW {
        client
            .send_to(&envelope, runtime.public_addr().unwrap())
            .unwrap();
    }
    let mut received = 0;
    let mut dropped = 0;
    for _ in 0..4 {
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        received += report.public_received;
        dropped += report.public_dropped;
        if received == crate::PUBLIC_PREAUTH_HANDSHAKE_DATAGRAMS_PER_WINDOW {
            break;
        }
    }
    assert_eq!(
        received,
        crate::PUBLIC_PREAUTH_HANDSHAKE_DATAGRAMS_PER_WINDOW
    );
    assert_eq!(
        dropped,
        crate::PUBLIC_PREAUTH_HANDSHAKE_DATAGRAMS_PER_WINDOW
    );
    assert_eq!(runtime.core().metrics().source_limited, 0);
    assert_eq!(
        runtime.metrics().public_ingress.datagrams,
        crate::PUBLIC_PREAUTH_HANDSHAKE_DATAGRAMS_PER_WINDOW as u64
    );
    assert_eq!(
        runtime.metrics().inner_ingress.datagrams,
        crate::PUBLIC_PREAUTH_HANDSHAKE_DATAGRAMS_PER_WINDOW as u64
    );
    assert!(!runtime.core().default_lobby_ready());

    // Once Ready publishes the template, the same source receives the normal bounded budget.
    runtime
        .core_mut()
        .register_route(route(2, worker.worker_id, true))
        .unwrap();
    assert!(runtime.core().default_lobby_ready());
    for _ in 0..=crate::PUBLIC_PREAUTH_HANDSHAKE_DATAGRAMS_PER_WINDOW {
        client
            .send_to(&envelope, runtime.public_addr().unwrap())
            .unwrap();
    }
    let expected = crate::PUBLIC_PREAUTH_HANDSHAKE_DATAGRAMS_PER_WINDOW + 1;
    let mut received = 0;
    let mut dropped = 0;
    for _ in 0..4 {
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        received += report.public_received;
        dropped += report.public_dropped;
        if received == expected {
            break;
        }
    }
    assert_eq!(received, expected);
    assert_eq!(dropped, 1);
    assert_eq!(runtime.core().metrics().source_limited, 1);
}

fn lobby_runtime_without_ready_route() -> (SupervisorRuntime, WorkerRegistration, UnixWorkerChannels)
{
    let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let worker = registration(1, WorkerKind::Lobby);
    runtime.register_worker(worker).unwrap();
    let channels = attach_worker(&mut runtime, worker);
    (runtime, worker, channels)
}

fn poll_until_received(runtime: &mut SupervisorRuntime, expected: usize) -> RuntimePollReport {
    let mut merged = RuntimePollReport::default();
    for _ in 0..8 {
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        merged.public_received += report.public_received;
        merged.public_dropped += report.public_dropped;
        merged
            .ingress_diagnostics
            .extend(report.ingress_diagnostics);
        if merged.public_received >= expected {
            break;
        }
    }
    assert_eq!(merged.public_received, expected);
    merged
}

#[test]
fn ingress_drops_name_their_reason_and_source_once_per_window() {
    let (mut runtime, worker, _channels) = lobby_runtime_without_ready_route();
    let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let source = client.local_addr().unwrap();
    let envelope = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![1])
        .unwrap()
        .encode()
        .unwrap();
    for _ in 0..3 {
        client
            .send_to(&envelope, runtime.public_addr().unwrap())
            .unwrap();
    }
    let report = poll_until_received(&mut runtime, 3);
    assert_eq!(
        report.ingress_diagnostics,
        vec![IngressDiagnostic::Drop {
            reason: IngressDropReason::LobbyNotReady,
            source,
            folded: 0,
        }],
        "three not-ready drops from one source make one line"
    );

    runtime
        .core_mut()
        .register_route(route(2, worker.worker_id, true))
        .unwrap();
    let datagrams = crate::PUBLIC_PREAUTH_HANDSHAKE_DATAGRAMS_PER_WINDOW + 2;
    for _ in 0..datagrams {
        client
            .send_to(&envelope, runtime.public_addr().unwrap())
            .unwrap();
    }
    let report = poll_until_received(&mut runtime, datagrams);
    assert_eq!(report.public_dropped, 2);
    assert_eq!(
        report.ingress_diagnostics,
        vec![IngressDiagnostic::Drop {
            reason: IngressDropReason::SourceLimited,
            source,
            folded: 0,
        }]
    );

    let stranger = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let capability = PublicEnvelope::new(
        RouteSelector::Capability(Capability::from_bytes([9; crate::CAPABILITY_BYTES]).unwrap()),
        vec![1],
    )
    .unwrap()
    .encode()
    .unwrap();
    stranger
        .send_to(&capability, runtime.public_addr().unwrap())
        .unwrap();
    let report = poll_until_received(&mut runtime, 1);
    assert_eq!(
        report.ingress_diagnostics,
        vec![IngressDiagnostic::Drop {
            reason: IngressDropReason::RouteRejected(RoutingErrorCategory::CapabilityUnknown),
            source: stranger.local_addr().unwrap(),
            folded: 0,
        }]
    );
}

#[test]
fn a_lobby_route_reused_after_silence_is_reported_with_its_age_and_idle_time() {
    let (mut runtime, worker, _channels) = lobby_runtime_without_ready_route();
    runtime
        .core_mut()
        .register_route(route(2, worker.worker_id, true))
        .unwrap();
    let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let source = client.local_addr().unwrap();
    let keepalive = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![0x11; 32])
        .unwrap()
        .encode()
        .unwrap();
    for _ in 0..2 {
        client
            .send_to(&keepalive, runtime.public_addr().unwrap())
            .unwrap();
    }
    let report = poll_until_received(&mut runtime, 2);
    assert!(
        report.ingress_diagnostics.is_empty(),
        "steady traffic on a fresh route is not a reuse: {:?}",
        report.ingress_diagnostics
    );
    let lobby_route = runtime.core().lobby_source_route(source).unwrap().route_id;

    std::thread::sleep(Duration::from_millis(1_100));
    let mut request = vec![0_u8; 1_078];
    request[1] = 1;
    let request = PublicEnvelope::new(RouteSelector::DefaultLobby, request)
        .unwrap()
        .encode()
        .unwrap();
    client
        .send_to(&request, runtime.public_addr().unwrap())
        .unwrap();
    let report = poll_until_received(&mut runtime, 1);
    let [
        IngressDiagnostic::LobbyRouteReuse {
            source: reported_source,
            route_id,
            route_age_millis,
            idle_millis,
            connection_request,
            folded,
        },
    ] = report.ingress_diagnostics.as_slice()
    else {
        panic!(
            "expected one reuse line, got {:?}",
            report.ingress_diagnostics
        );
    };
    assert_eq!(*reported_source, source);
    assert_eq!(*route_id, lobby_route);
    assert!(*idle_millis >= 1_000, "idle {idle_millis}");
    assert!(*route_age_millis >= *idle_millis);
    assert!(*connection_request);
    assert_eq!(*folded, 0);
}

/// Have `match_worker` send one packet back on `match_route` and wait until it is queued to the
/// public socket.
fn reply_once(
    runtime: &mut SupervisorRuntime,
    worker_io: &mut UnixWorkerChannels,
    match_worker: WorkerRegistration,
    match_route: RouteRegistration,
) {
    let reply = PacketRecord::new(
        PacketDirection::WorkerToSupervisor,
        match_worker.worker_id,
        match_route.route_id,
        match_route.peer_id,
        vec![0x12; 40],
    )
    .unwrap()
    .encode()
    .unwrap();
    worker_io.enqueue_packet(&reply).unwrap();
    worker_io.flush_packet(1).unwrap();
    let mut replied = 0;
    for _ in 0..8 {
        replied += runtime
            .poll_once(Some(Duration::from_millis(10)))
            .unwrap()
            .packets_to_public;
        if replied == 1 {
            break;
        }
    }
    assert_eq!(replied, 1);
}

/// Bind a fresh capability to `match_route` on `match_worker`, as an allocation grant does.
fn bind_match_capability(
    runtime: &mut SupervisorRuntime,
    match_worker: WorkerRegistration,
    match_route: RouteRegistration,
) -> Capability {
    let capability = Capability::from_bytes([3; crate::CAPABILITY_BYTES]).unwrap();
    let core = runtime.config.core;
    let bound_at = runtime.now();
    runtime
        .core_mut()
        .bind_capability(
            capability.clone(),
            crate::CapabilityBinding {
                logical_server_id: core.logical_server_id.unwrap(),
                supervisor_generation: core.supervisor_generation.unwrap(),
                worker_id: match_worker.worker_id,
                worker_generation: match_worker.generation,
                route_id: match_route.route_id,
                peer_id: match_route.peer_id,
                lobby_session_id: LobbySessionId::new(40).unwrap(),
                allocation_id: AllocationId::new(41).unwrap(),
                match_id: MatchId::new(42).unwrap(),
                network_protocol: core.network_protocol.unwrap(),
                content_fingerprint: core.content_fingerprint.unwrap(),
            },
            bound_at,
        )
        .unwrap();
    capability
}

#[test]
fn a_match_route_reports_its_first_datagram_per_source_and_the_handshake_window() {
    let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let match_worker = registration(2, WorkerKind::Match);
    runtime.register_worker(match_worker).unwrap();
    let match_route = route(30, match_worker.worker_id, false);
    runtime.core_mut().register_route(match_route).unwrap();
    let mut worker_io = attach_worker(&mut runtime, match_worker);
    let capability = bind_match_capability(&mut runtime, match_worker, match_route);
    let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let source = client.local_addr().unwrap();
    // A Netcode connection request: prefix byte 0 and the exact 1078-byte request size.
    let request = PublicEnvelope::new(RouteSelector::Capability(capability), vec![0_u8; 1_078])
        .unwrap()
        .encode()
        .unwrap();
    for _ in 0..3 {
        client
            .send_to(&request, runtime.public_addr().unwrap())
            .unwrap();
    }
    let report = poll_until_received(&mut runtime, 3);
    assert_eq!(report.public_dropped, 0);
    assert_eq!(
        report.ingress_diagnostics,
        vec![IngressDiagnostic::CapabilityRouteFirstSeen {
            route_id: match_route.route_id,
            worker_id: match_worker.worker_id,
            worker_kind: Some(WorkerKind::Match),
            source,
            payload_len: 1_078,
            connection_request: true,
        }],
        "three requests from one source make one first-seen line"
    );

    // The worker answers once (a Netcode challenge in production).
    let mut forwarded = 0;
    for _ in 0..8 {
        forwarded += worker_io.packet_read_ready(8).unwrap().records.len();
        if forwarded == 3 {
            break;
        }
        runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
    }
    assert_eq!(forwarded, 3, "every request reached the worker");
    reply_once(&mut runtime, &mut worker_io, match_worker, match_route);

    let mut out = Vec::new();
    let window_end = MonotonicMillis(runtime.now().0 + crate::PUBLIC_WINDOW_MILLIS);
    runtime.ingress_diagnostics.flush(window_end, &mut out);
    let [
        IngressDiagnostic::CapabilityRouteWindow {
            route_id,
            worker_id,
            new_sources,
            forwarded,
            connection_requests,
            forwarded_bytes,
            replies,
            ..
        },
        IngressDiagnostic::Summary {
            capability_first_seen,
            capability_connection_requests,
            ..
        },
    ] = out.as_slice()
    else {
        panic!("expected one route window and a summary, got {out:?}");
    };
    assert_eq!(*route_id, match_route.route_id);
    assert_eq!(*worker_id, match_worker.worker_id);
    assert_eq!(
        (
            *new_sources,
            *forwarded,
            *connection_requests,
            *forwarded_bytes,
            *replies
        ),
        (1, 3, 3, 3 * 1_078, 1)
    );
    assert_eq!(
        (*capability_first_seen, *capability_connection_requests),
        (1, 3)
    );
}

#[test]
fn queued_rejection_is_reclaimed_so_request_ids_can_restart() {
    let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let worker = registration(1, WorkerKind::Lobby);
    runtime.register_worker(worker).unwrap();
    let request = AllocateRequestBody {
        request_id: RequestId::new(77).unwrap(),
        lobby_session_id: LobbySessionId::new(88).unwrap(),
        mode: GameMode::Wipeout,
        map_preset: 1,
        map_revision: 1,
        map_recipe_fingerprint: 1,
        map_recipe: Vec::new(),
        rules_profile: 1,
        objective_target: 10,
        match_duration_ticks: 10_800,
        countdown_ticks: 180,
        respawn_ticks: 180,
        spawn_protection_ticks: 90,
        completed_input_lock_ticks: 60,
        wipeout_recent_hostile_damage_credit_ticks: 300,
        heist_critical_health_percent: 25,
        team_count: 2,
        players_per_team: 2,
        participants: Vec::new(),
        bots: Vec::new(),
    };
    runtime.allocations.insert(
        request.request_id,
        AllocationRecord {
            request: request.clone(),
            lobby_worker_id: worker.worker_id,
            allocation_id: None,
            match_id: None,
            match_worker_id: None,
            participants: Vec::new(),
            response: Some(ControlBody::AllocationRejected(AllocationRejectedBody {
                request_id: request.request_id,
                reason: ALLOCATION_REJECT_INVALID,
                retry_after_ms: 0,
            })),
            response_queued: false,
            result: None,
            ended_notified: false,
        },
    );
    runtime.queue_allocation_responses();
    assert!(runtime.allocations.is_empty());

    // A later session may safely reuse the same request ID after the prior response crossed
    // the bounded supervisor queue.
    runtime.allocations.insert(
        request.request_id,
        AllocationRecord {
            request,
            lobby_worker_id: worker.worker_id,
            allocation_id: None,
            match_id: None,
            match_worker_id: None,
            participants: Vec::new(),
            response: None,
            response_queued: false,
            result: None,
            ended_notified: false,
        },
    );
    assert_eq!(runtime.allocations.len(), 1);
}

#[test]
fn shutdown_suppresses_pending_runtime_controls_but_still_expires_routes() {
    let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let worker = registration(1, WorkerKind::Lobby);
    runtime.register_worker(worker).unwrap();
    runtime
        .core_mut()
        .register_route(route(2, worker.worker_id, true))
        .unwrap();
    let envelope = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![1, 2, 3]).unwrap();
    let source = SocketAddr::from(([127, 0, 0, 1], 4000));
    runtime
        .core_mut()
        .route_public(&envelope, source, MonotonicMillis(1))
        .unwrap();
    assert_eq!(runtime.core().route_count(), 2);

    // Leave a runtime-owned response in the bounded control queue, then enter shutdown.
    // The lifecycle Stop is the only control allowed to be appended after this point.
    let response = ControlBody::AllocationRejected(AllocationRejectedBody {
        request_id: RequestId::new(77).unwrap(),
        reason: ALLOCATION_REJECT_INVALID,
        retry_after_ms: 0,
    });
    assert!(runtime.queue_control_body(worker.worker_id, response));
    let next_before_shutdown = runtime
        .workers
        .get(&worker.worker_id)
        .expect("worker is registered")
        .next_control_sequence;
    runtime.shutting_down = true;
    runtime.started = Instant::now()
        .checked_sub(Duration::from_millis(
            crate::PUBLIC_LOBBY_ROUTE_IDLE_MILLIS + 1,
        ))
        .expect("idle-route duration fits before the current instant");

    // Expiry revokes the dynamic route and queued packets, but does not emit PeerClose after
    // the lifecycle Stop boundary or advance the runtime control cursor.
    let mut report = RuntimePollReport::default();
    runtime.expire(&mut report);
    assert_eq!(report.routes_torn_down, 1);
    assert_eq!(runtime.core().route_count(), 1);
    assert_eq!(runtime.core().metrics().control_current.frames, 1);
    assert_eq!(
        runtime
            .workers
            .get(&worker.worker_id)
            .expect("worker remains registered")
            .next_control_sequence,
        next_before_shutdown
    );

    let pending_request = AllocateRequestBody {
        request_id: RequestId::new(78).unwrap(),
        lobby_session_id: LobbySessionId::new(88).unwrap(),
        mode: GameMode::Wipeout,
        map_preset: 1,
        map_revision: 1,
        map_recipe_fingerprint: 1,
        map_recipe: Vec::new(),
        rules_profile: 1,
        objective_target: 10,
        match_duration_ticks: 10_800,
        countdown_ticks: 180,
        respawn_ticks: 180,
        spawn_protection_ticks: 90,
        completed_input_lock_ticks: 60,
        wipeout_recent_hostile_damage_credit_ticks: 300,
        heist_critical_health_percent: 25,
        team_count: 2,
        players_per_team: 2,
        participants: Vec::new(),
        bots: Vec::new(),
    };
    runtime.allocations.insert(
        pending_request.request_id,
        AllocationRecord {
            request: pending_request.clone(),
            lobby_worker_id: worker.worker_id,
            allocation_id: None,
            match_id: None,
            match_worker_id: None,
            participants: Vec::new(),
            response: Some(ControlBody::AllocationRejected(AllocationRejectedBody {
                request_id: pending_request.request_id,
                reason: ALLOCATION_REJECT_INVALID,
                retry_after_ms: 0,
            })),
            response_queued: false,
            result: None,
            ended_notified: false,
        },
    );

    // A pending allocation response is retained for bounded bookkeeping, but it cannot be
    // moved into the core queue once shutdown owns the stream ordering.
    runtime.queue_allocation_responses();
    assert_eq!(runtime.core().metrics().control_current.frames, 1);
}

#[test]
fn malformed_public_datagram_is_dropped_without_worker_work() {
    let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let worker = registration(1, WorkerKind::Lobby);
    runtime.register_worker(worker).unwrap();
    runtime
        .core_mut()
        .register_route(route(2, worker.worker_id, true))
        .unwrap();
    let (supervisor_packet, _worker_packet) = UnixStream::pair().unwrap();
    let (supervisor_control, _worker_control) = UnixStream::pair().unwrap();
    runtime
        .attach_worker_channels(
            worker.worker_id,
            UnixWorkerChannels::new(supervisor_packet, supervisor_control),
        )
        .unwrap();
    let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    client
        .send_to(
            &[0xff; crate::PUBLIC_MAX_DATAGRAM_BYTES + 1],
            runtime.public_addr().unwrap(),
        )
        .unwrap();
    let mut dropped = 0;
    for _ in 0..3 {
        dropped += runtime
            .poll_once(Some(Duration::from_millis(10)))
            .unwrap()
            .public_dropped;
        if dropped == 1 {
            break;
        }
    }
    assert_eq!(dropped, 1);
    assert_eq!(runtime.core().metrics().packet_current.frames, 0);
}

/// Read worker-bound packet records until `expected` have arrived, polling the runtime between
/// nonblocking reads so its bounded flush can make progress.
fn read_worker_records(
    runtime: &mut SupervisorRuntime,
    worker: &mut UnixWorkerChannels,
    expected: usize,
) -> Vec<PacketRecord> {
    let mut records = Vec::new();
    for _ in 0..16 {
        for record in worker.packet_read_ready(64).unwrap().records {
            records
                .push(PacketRecord::decode(&record, PacketDirection::SupervisorToWorker).unwrap());
        }
        if records.len() >= expected {
            break;
        }
        runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
    }
    assert_eq!(records.len(), expected);
    records
}

/// BRL-jx6p: on a 1.5 s round trip a Netcode client sends 15 identical connection requests at
/// 10 Hz before the lobby's challenge reaches it. The surplus requests are dropped, but the
/// challenge responses that follow the delayed reply must still reach the lobby worker.
#[test]
fn challenge_responses_after_a_delayed_lobby_reply_reach_the_worker() {
    let (mut runtime, worker, mut worker_channels) = lobby_runtime_without_ready_route();
    runtime
        .core_mut()
        .register_route(route(2, worker.worker_id, true))
        .unwrap();
    let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    client.set_nonblocking(true).unwrap();
    let request_payload = vec![0_u8; ingress_diagnostics::NETCODE_CONNECTION_REQUEST_BYTES];
    assert!(looks_like_netcode_connection_request(&request_payload));
    let request = PublicEnvelope::new(RouteSelector::DefaultLobby, request_payload)
        .unwrap()
        .encode()
        .unwrap();
    let retries = 15;
    for _ in 0..retries {
        client
            .send_to(&request, runtime.public_addr().unwrap())
            .unwrap();
    }
    let report = poll_until_received(&mut runtime, retries);
    let forwarded = crate::PUBLIC_PREAUTH_CONNECTION_REQUESTS_PER_WINDOW;
    assert_eq!(report.public_dropped, retries - forwarded);
    let requests = read_worker_records(&mut runtime, &mut worker_channels, forwarded);

    // The lobby's challenge reaches the client 1.5 s after its first request.
    runtime.started = runtime
        .started
        .checked_sub(Duration::from_millis(1_500))
        .expect("the delay fits before the runtime start");
    let challenge = PacketRecord::new(
        PacketDirection::WorkerToSupervisor,
        worker.worker_id,
        requests[0].route_id,
        requests[0].peer_id,
        vec![0x12; 200],
    )
    .unwrap()
    .encode()
    .unwrap();
    worker_channels.enqueue_packet(&challenge).unwrap();
    worker_channels.flush_packet(1).unwrap();
    let mut received = [0; crate::PUBLIC_MAX_DATAGRAM_BYTES];
    let mut delivered = None;
    for _ in 0..8 {
        runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        if let Ok((length, _)) = client.recv_from(&mut received) {
            delivered = Some(length);
            break;
        }
    }
    let length = delivered.expect("the delayed challenge reached the client");
    assert_eq!(
        PublicEnvelope::decode(&received[..length])
            .unwrap()
            .payload()
            .len(),
        200
    );

    // The client answers at 10 Hz until the lobby authenticates it; every answer is forwarded.
    let response = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![0x13; 326])
        .unwrap()
        .encode()
        .unwrap();
    let responses = 3;
    for _ in 0..responses {
        client
            .send_to(&response, runtime.public_addr().unwrap())
            .unwrap();
    }
    let report = poll_until_received(&mut runtime, responses);
    assert_eq!(report.public_dropped, 0);
    let answers = read_worker_records(&mut runtime, &mut worker_channels, responses);
    assert!(
        answers
            .iter()
            .all(|answer| answer.payload.len() == 326 && answer.route_id == requests[0].route_id)
    );
    assert_eq!(
        runtime.core().metrics().source_limited,
        (retries - forwarded) as u64
    );
}

#[test]
fn public_default_flood_is_limited_before_route_queue_and_does_not_spawn_workers() {
    let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let worker = registration(1, WorkerKind::Lobby);
    runtime.register_worker(worker).unwrap();
    runtime
        .core_mut()
        .register_route(route(2, worker.worker_id, true))
        .unwrap();
    let (supervisor_packet, _worker_packet) = UnixStream::pair().unwrap();
    let (supervisor_control, _worker_control) = UnixStream::pair().unwrap();
    runtime
        .attach_worker_channels(
            worker.worker_id,
            UnixWorkerChannels::new(supervisor_packet, supervisor_control),
        )
        .unwrap();
    let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let envelope = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![1; 1])
        .unwrap()
        .encode()
        .unwrap();
    for _ in 0..=crate::PUBLIC_PREAUTH_HANDSHAKE_DATAGRAMS_PER_WINDOW {
        client
            .send_to(&envelope, runtime.public_addr().unwrap())
            .unwrap();
    }
    let expected = crate::PUBLIC_PREAUTH_HANDSHAKE_DATAGRAMS_PER_WINDOW + 1;
    let mut received = 0;
    let mut dropped = 0;
    for _ in 0..4 {
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        received += report.public_received;
        dropped += report.public_dropped;
        if received == expected {
            break;
        }
    }
    assert_eq!(received, expected);
    assert_eq!(dropped, 1);
    assert_eq!(runtime.core().metrics().source_limited, 1);
    assert_eq!(runtime.core().route_count(), 2);
    assert_eq!(runtime.core().worker_count(), 1);
}

#[test]
fn malformed_source_is_suppressed_without_allocating_workers_or_replies() {
    let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    client.set_nonblocking(true).unwrap();
    let malformed = [0xff; crate::PUBLIC_MAX_DATAGRAM_BYTES + 1];
    for _ in 0..crate::PUBLIC_MALFORMED_PER_WINDOW {
        client
            .send_to(&malformed, runtime.public_addr().unwrap())
            .unwrap();
    }
    let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
    assert_eq!(report.public_received, crate::PUBLIC_MALFORMED_PER_WINDOW);
    assert_eq!(report.public_dropped, crate::PUBLIC_MALFORMED_PER_WINDOW);
    assert!(runtime.core().metrics().source_limited >= 1);
    assert_eq!(runtime.core().worker_count(), 0);
    let mut response = [0_u8; crate::PUBLIC_MAX_DATAGRAM_BYTES];
    assert!(client.recv_from(&mut response).is_err());
}

#[test]
fn worker_to_public_queue_drops_newest_at_global_frame_bound() {
    let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let destination = SocketAddr::from(([127, 0, 0, 1], 9));
    for _ in 0..crate::GLOBAL_PACKET_QUEUE_FRAMES {
        assert!(runtime.enqueue_public_datagram(vec![1], destination));
    }
    assert!(!runtime.enqueue_public_datagram(vec![2], destination));
    assert_eq!(runtime.outgoing.len(), crate::GLOBAL_PACKET_QUEUE_FRAMES);
    assert_eq!(
        runtime.core().metrics().error_counts[&RoutingErrorCategory::PacketQueueFull],
        1
    );
}

#[test]
fn invalid_peer_close_isolated_from_sibling_lobby() {
    let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let lobby = registration(1, WorkerKind::Lobby);
    let match_worker = registration(2, WorkerKind::Match);
    runtime.register_worker(lobby).unwrap();
    runtime.register_worker(match_worker).unwrap();
    let lobby_route = route(3, lobby.worker_id, true);
    let match_route = route(4, match_worker.worker_id, false);
    runtime.core_mut().register_route(lobby_route).unwrap();
    runtime.core_mut().register_route(match_route).unwrap();
    let lobby_io = attach_worker(&mut runtime, lobby);
    let mut match_io = attach_worker(&mut runtime, match_worker);

    send_worker_control(
        &mut match_io,
        match_worker,
        1,
        ControlBody::PeerClose(PeerCloseBody {
            route_id: match_route.route_id,
            peer_id: PeerId::new(match_route.peer_id.get() + 1).unwrap(),
            reason: 1,
        }),
    );
    let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();

    assert!(report.lifecycle_events.is_empty());
    assert_eq!(runtime.core().worker_count(), 1);
    assert_eq!(runtime.core().route_count(), 1);
    assert!(
        runtime
            .core()
            .public_selector_for_route(lobby_route.route_id)
            .is_some()
    );
    // Keep the surviving channel live so the test also proves its Mio registration was not
    // torn down with the failed match.
    drop(lobby_io);
}

#[test]
fn invalid_lobby_authentication_isolated_from_sibling_match() {
    let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let lobby = registration(1, WorkerKind::Lobby);
    let match_worker = registration(2, WorkerKind::Match);
    runtime.register_worker(lobby).unwrap();
    runtime.register_worker(match_worker).unwrap();
    let lobby_route = route(3, lobby.worker_id, true);
    let match_route = route(4, match_worker.worker_id, false);
    runtime.core_mut().register_route(lobby_route).unwrap();
    runtime.core_mut().register_route(match_route).unwrap();
    let mut lobby_io = attach_worker(&mut runtime, lobby);
    let _match_io = attach_worker(&mut runtime, match_worker);

    send_worker_control(
        &mut lobby_io,
        lobby,
        1,
        ControlBody::LobbyAuthenticated(crate::LobbyAuthenticatedBody {
            route_id: lobby_route.route_id,
            peer_id: PeerId::new(lobby_route.peer_id.get() + 1).unwrap(),
            lobby_session_id: LobbySessionId::new(7).unwrap(),
            netcode_client_id: crate::NetcodeClientId::new(8).unwrap(),
        }),
    );
    let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();

    assert!(report.lifecycle_events.is_empty());
    assert_eq!(runtime.core().worker_count(), 1);
    assert_eq!(runtime.core().route_count(), 1);
}

#[test]
fn invalid_match_result_isolated_from_sibling_lobby() {
    let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let lobby = registration(1, WorkerKind::Lobby);
    let match_worker = registration(2, WorkerKind::Match);
    runtime.register_worker(lobby).unwrap();
    runtime.register_worker(match_worker).unwrap();
    let lobby_route = route(3, lobby.worker_id, true);
    let match_route = route(4, match_worker.worker_id, false);
    runtime.core_mut().register_route(lobby_route).unwrap();
    runtime.core_mut().register_route(match_route).unwrap();
    let _lobby_io = attach_worker(&mut runtime, lobby);
    let mut match_io = attach_worker(&mut runtime, match_worker);

    send_worker_control(
        &mut match_io,
        match_worker,
        1,
        ControlBody::Result(
            ResultBody::new(
                MatchId::new(99).unwrap(),
                AllocationId::new(100).unwrap(),
                vec![1],
            )
            .unwrap(),
        ),
    );
    let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();

    assert!(report.lifecycle_events.is_empty());
    assert_eq!(runtime.core().worker_count(), 1);
    assert_eq!(runtime.core().route_count(), 1);
}

#[test]
fn lobby_capacity_is_scalar_idempotent_and_tracks_registered_match_workers() {
    let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let lobby = registration(1, WorkerKind::Lobby);
    runtime.register_worker(lobby).unwrap();
    {
        let worker = runtime.workers.get_mut(&lobby.worker_id).unwrap();
        worker.match_slot_limit = Some(3);
        worker.pending_default_route = None;
    }
    runtime.refresh_lobby_capacity();
    let first = runtime.core_mut().drain_controls(4);
    assert_eq!(first.len(), 1);
    assert!(matches!(
        ControlFrame::decode(&first[0].1).unwrap().body,
        ControlBody::LobbyCapacity(crate::LobbyCapacityBody {
            free_match_slots: 3
        })
    ));
    runtime.refresh_lobby_capacity();
    assert!(runtime.core_mut().drain_controls(4).is_empty());

    runtime
        .register_worker(registration(2, WorkerKind::Match))
        .unwrap();
    runtime.refresh_lobby_capacity();
    let second = runtime.core_mut().drain_controls(4);
    assert!(matches!(
        ControlFrame::decode(&second[0].1).unwrap().body,
        ControlBody::LobbyCapacity(crate::LobbyCapacityBody {
            free_match_slots: 2
        })
    ));
}

mod rejoin_minting {
    use super::*;
    use crate::{
        AllocateParticipant, AllocationGrantedBody, Capability, CapabilityBinding,
        MAX_REJOINS_PER_SEAT, MatchBuildSnapshot, MatchDisplayName, NetcodeClientId,
        ParticipantReboundBody, PlayerId, RejoinReason, RejoinRequestBody,
    };

    struct Fixture {
        runtime: SupervisorRuntime,
        lobby: WorkerRegistration,
        match_worker: WorkerRegistration,
        original_capability: Capability,
    }

    const REQUEST: u64 = 7;
    const ALLOCATION: u128 = 70;
    const MATCH: u128 = 71;
    const PLAYER: u64 = 5;

    fn fixture() -> Fixture {
        let mut runtime = SupervisorRuntime::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
        let lobby = registration(1, WorkerKind::Lobby);
        let match_worker = registration(2, WorkerKind::Match);
        runtime.register_worker(lobby).unwrap();
        runtime.register_worker(match_worker).unwrap();
        let original_route = route(30, match_worker.worker_id, false);
        runtime.core_mut().register_route(original_route).unwrap();
        let original_capability = Capability::from_bytes([3; 32]).unwrap();
        let core = runtime.config.core;
        runtime
            .core_mut()
            .bind_capability(
                original_capability.clone(),
                CapabilityBinding {
                    logical_server_id: core.logical_server_id.unwrap(),
                    supervisor_generation: core.supervisor_generation.unwrap(),
                    worker_id: match_worker.worker_id,
                    worker_generation: match_worker.generation,
                    route_id: original_route.route_id,
                    peer_id: original_route.peer_id,
                    lobby_session_id: LobbySessionId::new(40).unwrap(),
                    allocation_id: AllocationId::new(ALLOCATION).unwrap(),
                    match_id: MatchId::new(MATCH).unwrap(),
                    network_protocol: core.network_protocol.unwrap(),
                    content_fingerprint: core.content_fingerprint.unwrap(),
                },
                MonotonicMillis(0),
            )
            .unwrap();
        let source = AllocateParticipant {
            lobby_session_id: LobbySessionId::new(40).unwrap(),
            player_id: PlayerId::new(PLAYER).unwrap(),
            netcode_client_id: NetcodeClientId::new(41).unwrap(),
            team: 0,
            display_name: MatchDisplayName::new("Pilot").unwrap(),
            recipe_fingerprint: 1,
            build_revision: 1,
            build_snapshot: MatchBuildSnapshot::new(&[1]).unwrap(),
        };
        let request = AllocateRequestBody {
            request_id: RequestId::new(REQUEST).unwrap(),
            lobby_session_id: LobbySessionId::new(40).unwrap(),
            mode: GameMode::Wipeout,
            map_preset: 1,
            map_revision: 1,
            map_recipe_fingerprint: 1,
            map_recipe: Vec::new(),
            rules_profile: 1,
            objective_target: 10,
            match_duration_ticks: 10_800,
            countdown_ticks: 180,
            respawn_ticks: 180,
            spawn_protection_ticks: 90,
            completed_input_lock_ticks: 60,
            wipeout_recent_hostile_damage_credit_ticks: 300,
            heist_critical_health_percent: 25,
            team_count: 2,
            players_per_team: 1,
            participants: vec![source],
            bots: Vec::new(),
        };
        runtime.allocations.insert(
            request.request_id,
            AllocationRecord {
                request: request.clone(),
                lobby_worker_id: lobby.worker_id,
                allocation_id: AllocationId::new(ALLOCATION),
                match_id: MatchId::new(MATCH),
                match_worker_id: Some(match_worker.worker_id),
                participants: vec![AllocationParticipant {
                    source,
                    route_id: original_route.route_id,
                    peer_id: original_route.peer_id,
                    capability: original_capability.clone(),
                    rejoins: rejoin::SeatRejoins::default(),
                }],
                response: Some(ControlBody::AllocationGranted(AllocationGrantedBody {
                    request_id: request.request_id,
                    allocation_id: AllocationId::new(ALLOCATION).unwrap(),
                    match_id: MatchId::new(MATCH).unwrap(),
                    worker_id: match_worker.worker_id,
                    grants: Vec::new(),
                })),
                response_queued: true,
                result: None,
                ended_notified: false,
            },
        );
        Fixture {
            runtime,
            lobby,
            match_worker,
            original_capability,
        }
    }

    fn rejoin_request(rejoin_id: u64) -> RejoinRequestBody {
        RejoinRequestBody {
            rejoin_id: RequestId::new(rejoin_id).unwrap(),
            request_id: RequestId::new(REQUEST).unwrap(),
            allocation_id: AllocationId::new(ALLOCATION).unwrap(),
            match_id: MatchId::new(MATCH).unwrap(),
            player_id: PlayerId::new(PLAYER).unwrap(),
            lobby_session_id: LobbySessionId::new(50 + u128::from(rejoin_id)).unwrap(),
            netcode_client_id: NetcodeClientId::new(60 + rejoin_id).unwrap(),
        }
    }

    fn drained(runtime: &mut SupervisorRuntime) -> Vec<(WorkerId, ControlBody)> {
        runtime
            .core_mut()
            .drain_controls(16)
            .into_iter()
            .map(|(worker, record)| (worker, ControlFrame::decode(&record).unwrap().body))
            .collect()
    }

    /// Run one request through the match worker's acceptance and return the lobby's answer.
    fn rejoin(fixture: &mut Fixture, rejoin_id: u64) -> ControlBody {
        let lobby = fixture.lobby.worker_id;
        fixture
            .runtime
            .accept_rejoin_request(lobby, rejoin_request(rejoin_id));
        let mut controls = drained(&mut fixture.runtime);
        assert_eq!(controls.len(), 1, "one control answers each request");
        let (worker, body) = controls.remove(0);
        if worker == lobby {
            return body;
        }
        assert_eq!(worker, fixture.match_worker.worker_id);
        let ControlBody::ParticipantRebind(rebind) = body else {
            panic!("the match worker is asked to rebind the seat, got {body:?}");
        };
        assert_eq!(rebind.player_id.get(), PLAYER);
        assert_eq!(rebind.netcode_client_id.get(), 60 + rejoin_id);
        fixture.runtime.complete_participant_rebind(
            fixture.match_worker.worker_id,
            ParticipantReboundBody {
                rejoin_id: rebind.rejoin_id,
                player_id: rebind.player_id,
                peer_id: rebind.peer_id,
                refusal: None,
            },
        );
        let mut controls = drained(&mut fixture.runtime);
        assert_eq!(controls.len(), 1);
        let (worker, body) = controls.remove(0);
        assert_eq!(worker, lobby);
        body
    }

    #[test]
    fn a_released_seat_gets_a_fresh_route_peer_and_capability_up_to_the_limit() {
        let mut fixture = fixture();
        // The original session is still live: the seat is not released yet.
        assert!(matches!(
            rejoin(&mut fixture, 1),
            ControlBody::RejoinRejected(rejected) if rejected.reason == RejoinReason::SeatConnected
        ));
        let original = fixture.original_capability.clone();
        assert!(fixture.runtime.core_mut().revoke_capability(&original));

        let mut seen_routes = vec![RouteId::new(30).unwrap()];
        let mut seen_capabilities = vec![original.clone()];
        for rejoin_id in 2..2 + u64::from(MAX_REJOINS_PER_SEAT) {
            let ControlBody::RejoinGranted(granted) = rejoin(&mut fixture, rejoin_id) else {
                panic!("a released seat is granted");
            };
            assert_eq!(granted.player_id.get(), PLAYER);
            assert_eq!(granted.request_id.get(), REQUEST);
            assert_eq!(
                granted.grant.lobby_session_id.get(),
                50 + u128::from(rejoin_id)
            );
            assert!(!seen_routes.contains(&granted.grant.route_id));
            assert!(!seen_capabilities.contains(&granted.grant.capability));
            assert_eq!(
                fixture
                    .runtime
                    .core()
                    .capability_status(&granted.grant.capability),
                Some(crate::CapabilityStatus::Pending)
            );
            // Earlier capabilities stay revoked.
            for earlier in &seen_capabilities {
                assert_eq!(
                    fixture.runtime.core().capability_status(earlier),
                    Some(crate::CapabilityStatus::Revoked)
                );
            }
            // A live grant blocks a second one for the same seat (the limit wins once reached).
            let expected = if rejoin_id < 1 + u64::from(MAX_REJOINS_PER_SEAT) {
                RejoinReason::SeatConnected
            } else {
                RejoinReason::LimitReached
            };
            assert!(matches!(
                rejoin(&mut fixture, 100 + rejoin_id),
                ControlBody::RejoinRejected(rejected) if rejected.reason == expected
            ));
            assert!(
                fixture
                    .runtime
                    .core_mut()
                    .revoke_capability(&granted.grant.capability)
            );
            seen_routes.push(granted.grant.route_id);
            seen_capabilities.push(granted.grant.capability);
        }
        assert!(matches!(
            rejoin(&mut fixture, 99),
            ControlBody::RejoinRejected(rejected) if rejected.reason == RejoinReason::LimitReached
        ));
    }

    #[test]
    fn requests_for_another_lobby_player_or_ended_match_are_refused() {
        let mut fixture = fixture();
        let original = fixture.original_capability.clone();
        fixture.runtime.core_mut().revoke_capability(&original);
        let lobby = fixture.lobby.worker_id;

        let mut wrong_player = rejoin_request(1);
        wrong_player.player_id = PlayerId::new(PLAYER + 1).unwrap();
        fixture.runtime.accept_rejoin_request(lobby, wrong_player);
        let mut wrong_allocation = rejoin_request(2);
        wrong_allocation.allocation_id = AllocationId::new(ALLOCATION + 1).unwrap();
        fixture
            .runtime
            .accept_rejoin_request(lobby, wrong_allocation);
        // Only the lobby that owns the allocation may ask.
        fixture
            .runtime
            .accept_rejoin_request(fixture.match_worker.worker_id, rejoin_request(3));
        let answers = drained(&mut fixture.runtime);
        assert_eq!(answers.len(), 3);
        for (worker, body) in answers {
            assert!(matches!(
                body,
                ControlBody::RejoinRejected(rejected) if rejected.reason == RejoinReason::UnknownSeat
            ));
            let expected = if worker == lobby {
                lobby
            } else {
                fixture.match_worker.worker_id
            };
            assert_eq!(worker, expected);
        }

        // A refused rebind is forwarded to the lobby and does not count as a rejoin.
        fixture
            .runtime
            .accept_rejoin_request(lobby, rejoin_request(4));
        let ControlBody::ParticipantRebind(rebind) = drained(&mut fixture.runtime).remove(0).1
        else {
            panic!("rebind requested");
        };
        fixture.runtime.complete_participant_rebind(
            fixture.match_worker.worker_id,
            ParticipantReboundBody {
                rejoin_id: rebind.rejoin_id,
                player_id: rebind.player_id,
                peer_id: rebind.peer_id,
                refusal: Some(RejoinReason::SeatExpired),
            },
        );
        assert!(matches!(
            drained(&mut fixture.runtime).remove(0).1,
            ControlBody::RejoinRejected(rejected) if rejected.reason == RejoinReason::SeatExpired
        ));
        assert_eq!(
            fixture.runtime.allocations[&RequestId::new(REQUEST).unwrap()].participants[0]
                .rejoins
                .granted(),
            0
        );

        // Match end refuses an in-flight rejoin and is announced to the lobby exactly once.
        fixture
            .runtime
            .accept_rejoin_request(lobby, rejoin_request(5));
        let _rebind = drained(&mut fixture.runtime);
        fixture
            .runtime
            .notify_match_ended(RequestId::new(REQUEST).unwrap());
        fixture
            .runtime
            .notify_match_ended(RequestId::new(REQUEST).unwrap());
        let ended = drained(&mut fixture.runtime);
        assert_eq!(ended.len(), 2);
        assert!(matches!(
            ended[0].1,
            ControlBody::RejoinRejected(rejected) if rejected.reason == RejoinReason::MatchEnded
        ));
        assert!(matches!(
            ended[1].1,
            ControlBody::MatchEnded(body)
                if body.allocation_id.get() == ALLOCATION && body.match_id.get() == MATCH
        ));
    }

    /// BRL-8y1f: a participant who left on purpose releases its seat. The owning lobby is told
    /// once, and every later rejoin of that seat is refused for a terminal reason.
    #[test]
    fn a_seat_released_by_a_voluntary_leave_is_never_granted_again() {
        let mut fixture = fixture();
        let original = fixture.original_capability.clone();
        fixture.runtime.core_mut().revoke_capability(&original);
        let released = crate::SeatReleasedBody {
            request_id: RequestId::new(REQUEST).unwrap(),
            allocation_id: AllocationId::new(ALLOCATION).unwrap(),
            match_id: MatchId::new(MATCH).unwrap(),
            player_id: PlayerId::new(PLAYER).unwrap(),
        };
        // Only the allocation's own match worker may release its seats.
        assert_eq!(
            fixture
                .runtime
                .release_seat(fixture.lobby.worker_id, released),
            Err(RoutingErrorCategory::WorkerProtocolConflict)
        );
        let mut unknown = released;
        unknown.player_id = PlayerId::new(PLAYER + 1).unwrap();
        assert_eq!(
            fixture
                .runtime
                .release_seat(fixture.match_worker.worker_id, unknown),
            Err(RoutingErrorCategory::WorkerProtocolConflict)
        );
        assert!(drained(&mut fixture.runtime).is_empty());

        for _ in 0..2 {
            fixture
                .runtime
                .release_seat(fixture.match_worker.worker_id, released)
                .unwrap();
        }
        let forwarded = drained(&mut fixture.runtime);
        assert_eq!(forwarded.len(), 1, "the lobby is told exactly once");
        assert_eq!(forwarded[0].0, fixture.lobby.worker_id);
        assert_eq!(forwarded[0].1, ControlBody::SeatReleased(released));

        // Without the release this seat would now be granted (see the minting test above).
        assert!(matches!(
            rejoin(&mut fixture, 1),
            ControlBody::RejoinRejected(rejected) if rejected.reason == RejoinReason::SeatExpired
        ));
    }

    /// BRL-jxfy: a match worker that defers its Result releases the seats that cannot rejoin at
    /// completion, before its Result. The lobby is told about each seat once, and the later
    /// Result still announces the match end exactly once.
    #[test]
    fn a_seat_released_before_the_result_is_forwarded_once_and_the_end_follows() {
        let mut fixture = fixture();
        let released = crate::SeatReleasedBody {
            request_id: RequestId::new(REQUEST).unwrap(),
            allocation_id: AllocationId::new(ALLOCATION).unwrap(),
            match_id: MatchId::new(MATCH).unwrap(),
            player_id: PlayerId::new(PLAYER).unwrap(),
        };
        let worker = fixture.match_worker.worker_id;
        fixture.runtime.release_seat(worker, released).unwrap();
        let forwarded = drained(&mut fixture.runtime);
        assert_eq!(
            forwarded,
            vec![(fixture.lobby.worker_id, ControlBody::SeatReleased(released))]
        );

        fixture
            .runtime
            .complete_match_result(
                worker,
                ResultBody::new(
                    MatchId::new(MATCH).unwrap(),
                    AllocationId::new(ALLOCATION).unwrap(),
                    vec![1],
                )
                .unwrap(),
            )
            .unwrap();
        // A repeated release after the Result is accepted and not forwarded again.
        fixture.runtime.release_seat(worker, released).unwrap();
        // The allocation's reclaim announces the end only once.
        fixture
            .runtime
            .notify_match_ended(RequestId::new(REQUEST).unwrap());
        let ended = drained(&mut fixture.runtime);
        assert_eq!(ended.len(), 1, "only the match end follows: {ended:?}");
        assert!(matches!(
            ended[0],
            (lobby, ControlBody::MatchEnded(body))
                if lobby == fixture.lobby.worker_id
                    && body.allocation_id.get() == ALLOCATION
                    && body.match_id.get() == MATCH
        ));
    }
}
