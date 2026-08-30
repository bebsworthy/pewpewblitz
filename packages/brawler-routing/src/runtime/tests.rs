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
    // the source's 8-datagram/9-KiB pre-auth allowance.
    for _ in 0..crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW {
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
        if received == crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW {
            break;
        }
    }
    assert_eq!(received, crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW);
    assert_eq!(dropped, crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW);
    assert_eq!(runtime.core().metrics().source_limited, 0);
    assert_eq!(
        runtime.metrics().public_ingress.datagrams,
        crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW as u64
    );
    assert_eq!(
        runtime.metrics().inner_ingress.datagrams,
        crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW as u64
    );
    assert!(!runtime.core().default_lobby_ready());

    // Once Ready publishes the template, the same source receives the normal bounded budget.
    runtime
        .core_mut()
        .register_route(route(2, worker.worker_id, true))
        .unwrap();
    assert!(runtime.core().default_lobby_ready());
    for _ in 0..=crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW {
        client
            .send_to(&envelope, runtime.public_addr().unwrap())
            .unwrap();
    }
    let expected = crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW + 1;
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
    for _ in 0..=crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW {
        client
            .send_to(&envelope, runtime.public_addr().unwrap())
            .unwrap();
    }
    let expected = crate::PUBLIC_PREAUTH_DATAGRAMS_PER_WINDOW + 1;
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
