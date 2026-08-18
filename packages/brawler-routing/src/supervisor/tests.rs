use std::net::{IpAddr, Ipv4Addr, SocketAddr};

use crate::*;

use super::*;

fn id128<T: TryFrom<u128>>(value: u128) -> T
where
    T::Error: std::fmt::Debug,
{
    T::try_from(value).unwrap()
}

fn id64<T: TryFrom<u64>>(value: u64) -> T
where
    T::Error: std::fmt::Debug,
{
    T::try_from(value).unwrap()
}

fn worker(value: u128, kind: WorkerKind) -> WorkerRegistration {
    WorkerRegistration {
        worker_id: id128(value),
        process_id: id128(value + 100),
        generation: id64(1),
        kind,
    }
}

fn route(value: u128, worker_id: WorkerId, lobby: bool) -> RouteRegistration {
    RouteRegistration {
        route_id: id128(value),
        worker_id,
        peer_id: id128(value + 1000),
        is_default_lobby: lobby,
    }
}

fn capability(value: u8) -> Capability {
    Capability::from_bytes([value; CAPABILITY_BYTES]).unwrap()
}

fn binding(route: RouteRegistration) -> CapabilityBinding {
    CapabilityBinding {
        logical_server_id: id128(80),
        supervisor_generation: id64(1),
        worker_id: route.worker_id,
        worker_generation: id64(1),
        route_id: route.route_id,
        peer_id: route.peer_id,
        lobby_session_id: id128(81),
        allocation_id: id128(82),
        match_id: id128(83),
        network_protocol: 10,
        content_fingerprint: 11,
    }
}

fn source(last: u8) -> SocketAddr {
    SocketAddr::new(IpAddr::V4(Ipv4Addr::new(127, 0, 0, last)), 5000)
}

fn configured_core() -> (SupervisorCore, RouteRegistration) {
    let mut core = SupervisorCore::default();
    let worker = worker(1, WorkerKind::Match);
    core.register_worker(worker).unwrap();
    let registered_route = route(10, worker.worker_id, false);
    core.register_route(registered_route).unwrap();
    (core, registered_route)
}

#[test]
fn registries_enforce_bounds_identity_and_one_default_lobby() {
    let config = CoreConfig {
        max_workers: 1,
        max_routes: 1,
        ..CoreConfig::default()
    };
    let mut core = SupervisorCore::new(config);
    let lobby = worker(1, WorkerKind::Lobby);
    core.register_worker(lobby).unwrap();
    assert_eq!(
        core.register_worker(worker(3, WorkerKind::Lobby)),
        Err(RoutingErrorCategory::WorkerProtocolConflict)
    );
    assert_eq!(
        core.register_worker(worker(2, WorkerKind::Match)),
        Err(RoutingErrorCategory::AllocationCapacity)
    );
    assert_eq!(
        core.register_worker(lobby),
        Err(RoutingErrorCategory::WorkerProtocolConflict)
    );
    let lobby_route = route(10, lobby.worker_id, true);
    core.register_route(lobby_route).unwrap();
    assert_eq!(
        core.register_route(route(11, lobby.worker_id, true)),
        Err(RoutingErrorCategory::AllocationCapacity)
    );
    assert_eq!(
        core.register_route(route(12, id128(999), false)),
        Err(RoutingErrorCategory::ManifestIdentity)
    );
    assert_eq!(core.worker_count(), 1);
    assert_eq!(core.route_count(), 1);
}

#[test]
fn capability_registry_counts_negative_records_toward_its_bound() {
    let config = CoreConfig {
        max_capabilities: 1,
        ..CoreConfig::default()
    };
    let mut core = SupervisorCore::new(config);
    let worker = worker(1, WorkerKind::Match);
    core.register_worker(worker).unwrap();
    let primary_route = route(10, worker.worker_id, false);
    let replacement_route = route(11, worker.worker_id, false);
    core.register_route(primary_route).unwrap();
    core.register_route(replacement_route).unwrap();
    let first = capability(20);
    core.bind_capability(first.clone(), binding(primary_route), MonotonicMillis(0))
        .unwrap();
    assert!(core.revoke_capability(&first));
    assert_eq!(core.capability_count(), 1);
    assert_eq!(
        core.bind_capability(
            capability(21),
            binding(replacement_route),
            MonotonicMillis(1),
        ),
        Err(RoutingErrorCategory::AllocationCapacity)
    );
    core.expire(MonotonicMillis(CAPABILITY_HARD_LIFETIME_MILLIS));
    assert_eq!(core.capability_count(), 0);
    core.bind_capability(
        capability(21),
        binding(replacement_route),
        MonotonicMillis(CAPABILITY_HARD_LIFETIME_MILLIS + 1),
    )
    .unwrap();
}

#[test]
fn capability_activates_repeats_and_only_accepts_newest_source() {
    let (mut core, route) = configured_core();
    let token = capability(1);
    core.bind_capability(token.clone(), binding(route), MonotonicMillis(0))
        .unwrap();
    assert_eq!(
        core.capability_status(&token),
        Some(CapabilityStatus::Pending)
    );
    let first = core
        .authorize(&token, source(1), MonotonicMillis(1))
        .unwrap();
    assert!(first.activated);
    assert!(!first.rebound);
    let repeat = core
        .authorize(&token, source(1), MonotonicMillis(2))
        .unwrap();
    assert!(!repeat.activated && !repeat.rebound);
    assert!(
        core.authorize(&token, source(2), MonotonicMillis(3))
            .unwrap()
            .rebound
    );
    assert_eq!(
        core.authorize(&token, source(1), MonotonicMillis(4)),
        Err(RoutingErrorCategory::Binding)
    );
    assert!(
        core.authorize(&token, source(3), MonotonicMillis(5))
            .unwrap()
            .rebound
    );
    assert_eq!(
        core.authorize(&token, source(4), MonotonicMillis(6)),
        Err(RoutingErrorCategory::RebindLimited)
    );
    assert_eq!(core.metrics().capabilities_activated, 1);
    assert_eq!(core.metrics().capability_rebinds, 2);
}

#[test]
fn capability_rejects_monotonic_time_regression_without_mutating_state() {
    let (mut core, route) = configured_core();
    let token = capability(22);
    core.bind_capability(token.clone(), binding(route), MonotonicMillis(100))
        .unwrap();
    assert_eq!(
        core.authorize(&token, source(1), MonotonicMillis(99)),
        Err(RoutingErrorCategory::SupervisorInternal)
    );
    core.authorize(&token, source(1), MonotonicMillis(101))
        .unwrap();
    assert_eq!(
        core.authorize(&token, source(1), MonotonicMillis(100)),
        Err(RoutingErrorCategory::SupervisorInternal)
    );
    assert_eq!(
        core.capability_status(&token),
        Some(CapabilityStatus::Active)
    );
}

#[test]
fn rebind_window_rolls_forward_but_retired_sources_stay_invalid() {
    let (mut core, route) = configured_core();
    let token = capability(2);
    core.bind_capability(token.clone(), binding(route), MonotonicMillis(0))
        .unwrap();
    core.authorize(&token, source(1), MonotonicMillis(1))
        .unwrap();
    core.authorize(&token, source(2), MonotonicMillis(2))
        .unwrap();
    core.authorize(&token, source(3), MonotonicMillis(3))
        .unwrap();
    core.authorize(&token, source(3), MonotonicMillis(9_000))
        .unwrap();
    assert!(
        core.authorize(&token, source(4), MonotonicMillis(10_003))
            .unwrap()
            .rebound
    );
    assert_eq!(
        core.authorize(&token, source(2), MonotonicMillis(10_004)),
        Err(RoutingErrorCategory::Binding)
    );
}

#[test]
fn pending_idle_and_hard_expiry_are_monotonic_and_revoke_once() {
    let (mut core, primary_route) = configured_core();
    let pending = capability(3);
    core.bind_capability(pending.clone(), binding(primary_route), MonotonicMillis(10))
        .unwrap();
    assert_eq!(
        core.authorize(
            &pending,
            source(1),
            MonotonicMillis(10 + CAPABILITY_PENDING_MILLIS)
        ),
        Err(RoutingErrorCategory::PendingExpired)
    );
    assert_eq!(
        core.capability_status(&pending),
        Some(CapabilityStatus::Revoked)
    );
    assert_eq!(core.metrics().capabilities_revoked, 1);
    assert_eq!(core.route_count(), 0);
    assert_eq!(core.source_for_route(primary_route.route_id), None);

    let idle_route = route(11, primary_route.worker_id, false);
    core.register_route(idle_route).unwrap();
    let idle = capability(4);
    core.bind_capability(idle.clone(), binding(idle_route), MonotonicMillis(0))
        .unwrap();
    core.authorize(&idle, source(1), MonotonicMillis(1))
        .unwrap();
    assert_eq!(
        core.authorize(
            &idle,
            source(1),
            MonotonicMillis(1 + CAPABILITY_IDLE_MILLIS)
        ),
        Err(RoutingErrorCategory::RouteExpired)
    );
    assert_eq!(core.metrics().capabilities_revoked, 2);

    let hard_route = route(12, primary_route.worker_id, false);
    core.register_route(hard_route).unwrap();
    let hard = capability(5);
    core.bind_capability(hard.clone(), binding(hard_route), MonotonicMillis(0))
        .unwrap();
    core.authorize(&hard, source(1), MonotonicMillis(1))
        .unwrap();
    for now in (9_000..CAPABILITY_HARD_LIFETIME_MILLIS).step_by(9_000) {
        core.authorize(&hard, source(1), MonotonicMillis(now))
            .unwrap();
    }
    assert_eq!(
        core.authorize(
            &hard,
            source(1),
            MonotonicMillis(CAPABILITY_HARD_LIFETIME_MILLIS)
        ),
        Err(RoutingErrorCategory::RouteExpired)
    );
    assert!(!core.revoke_capability(&hard));
}

#[test]
fn worker_to_public_requires_the_registered_route_peer() {
    let (mut core, route) = configured_core();
    let capability = capability(23);
    core.bind_capability(capability.clone(), binding(route), MonotonicMillis(0))
        .unwrap();
    let envelope = PublicEnvelope::new(RouteSelector::Capability(capability), vec![1]).unwrap();
    let source = source(1);
    core.route_public(&envelope, source, MonotonicMillis(1))
        .unwrap();

    let wrong_peer = PeerId::new(route.peer_id.get() + 1).unwrap();
    let packet = PacketRecord::new(
        PacketDirection::WorkerToSupervisor,
        route.worker_id,
        route.route_id,
        wrong_peer,
        vec![2],
    )
    .unwrap();
    assert_eq!(
        core.accept_worker_packet(&packet),
        Err(RoutingErrorCategory::Binding)
    );
}

#[test]
fn lobby_authentication_fact_promotes_only_the_exact_authenticated_route_source() {
    let mut core = SupervisorCore::default();
    let lobby = worker(1, WorkerKind::Lobby);
    let match_worker = worker(2, WorkerKind::Match);
    core.register_worker(lobby).unwrap();
    core.register_worker(match_worker).unwrap();
    let template = route(10, lobby.worker_id, true);
    core.register_route(template).unwrap();
    let source = source(7);
    let dynamic = core
        .route_public(
            &PublicEnvelope::new(RouteSelector::DefaultLobby, vec![1]).unwrap(),
            source,
            MonotonicMillis(1),
        )
        .unwrap();
    let fact = LobbyAuthenticatedBody {
        route_id: dynamic.route_id,
        peer_id: dynamic.peer_id,
        lobby_session_id: id128(80),
        netcode_client_id: id64(81),
    };
    assert_eq!(
        core.authenticated_lobby_source(lobby.worker_id, fact),
        Ok(source)
    );
    assert_eq!(
        core.authenticated_lobby_source(match_worker.worker_id, fact),
        Err(RoutingErrorCategory::Binding)
    );
    assert_eq!(
        core.authenticated_lobby_source(
            lobby.worker_id,
            LobbyAuthenticatedBody {
                peer_id: id128(dynamic.peer_id.get() + 1),
                ..fact
            }
        ),
        Err(RoutingErrorCategory::Binding)
    );
    assert_eq!(
        core.authenticated_lobby_source(
            lobby.worker_id,
            LobbyAuthenticatedBody {
                route_id: template.route_id,
                peer_id: template.peer_id,
                ..fact
            }
        ),
        Err(RoutingErrorCategory::Binding)
    );
    core.expire(MonotonicMillis(PUBLIC_LOBBY_ROUTE_IDLE_MILLIS + 2));
    assert_eq!(
        core.authenticated_lobby_source(lobby.worker_id, fact),
        Err(RoutingErrorCategory::Binding)
    );
}

#[test]
fn lobby_netcode_authentication_fact_promotes_without_a_brawler_session() {
    let mut core = SupervisorCore::default();
    let lobby = worker(1, WorkerKind::Lobby);
    core.register_worker(lobby).unwrap();
    let template = route(10, lobby.worker_id, true);
    core.register_route(template).unwrap();
    let source = source(8);
    let dynamic = core
        .route_public(
            &PublicEnvelope::new(RouteSelector::DefaultLobby, vec![1]).unwrap(),
            source,
            MonotonicMillis(1),
        )
        .unwrap();
    let fact = LobbyNetcodeAuthenticatedBody {
        route_id: dynamic.route_id,
        peer_id: dynamic.peer_id,
        netcode_client_id: NetcodeClientId::new(81).unwrap(),
    };
    assert_eq!(
        core.authenticated_lobby_netcode_source(lobby.worker_id, fact),
        Ok(source)
    );
    assert_eq!(
        core.authenticated_lobby_netcode_source(
            lobby.worker_id,
            LobbyNetcodeAuthenticatedBody {
                peer_id: PeerId::new(dynamic.peer_id.get() + 1).unwrap(),
                ..fact
            }
        ),
        Err(RoutingErrorCategory::Binding)
    );
    core.expire(MonotonicMillis(PUBLIC_LOBBY_ROUTE_IDLE_MILLIS + 2));
    assert_eq!(
        core.authenticated_lobby_netcode_source(lobby.worker_id, fact),
        Err(RoutingErrorCategory::Binding)
    );
}

#[test]
fn idle_default_lobby_routes_reclaim_capacity_without_expiring_active_traffic() {
    let mut core = SupervisorCore::default();
    let lobby = worker(1, WorkerKind::Lobby);
    core.register_worker(lobby).unwrap();
    let template = route(10, lobby.worker_id, true);
    core.register_route(template).unwrap();
    let envelope = PublicEnvelope::new(RouteSelector::DefaultLobby, vec![1]).unwrap();

    // The template consumes one route slot, so 63 distinct sources exhaust the default route
    // capacity.  Their random route/peer IDs must remain distinct even though all use one lobby.
    let mut routes = Vec::new();
    for source_id in 1..=63 {
        routes.push(
            core.route_public(&envelope, source(source_id), MonotonicMillis(0))
                .unwrap(),
        );
    }
    assert_eq!(core.route_count(), MAX_ACTIVE_ROUTES);
    assert_eq!(
        core.route_public(&envelope, source(100), MonotonicMillis(0)),
        Err(RoutingErrorCategory::AllocationCapacity)
    );
    assert_eq!(
        routes
            .iter()
            .map(|route| route.route_id)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        63
    );
    assert_eq!(
        routes
            .iter()
            .map(|route| route.peer_id)
            .collect::<std::collections::HashSet<_>>()
            .len(),
        63
    );

    // A valid packet from source 1 refreshes only its dynamic route.  At the exact idle boundary
    // all other sources are reclaimed, while the active source remains usable.
    let active = routes[0];
    core.route_public(
        &envelope,
        source(1),
        MonotonicMillis(PUBLIC_LOBBY_ROUTE_IDLE_MILLIS - 1),
    )
    .unwrap();
    let teardowns = core.expire(MonotonicMillis(PUBLIC_LOBBY_ROUTE_IDLE_MILLIS));
    assert_eq!(teardowns.len(), 62);
    assert!(
        teardowns
            .iter()
            .all(|teardown| teardown.route_id != active.route_id)
    );
    assert_eq!(core.route_count(), 2); // template plus active source 1
    assert_eq!(core.source_for_route(active.route_id), Some(source(1)));

    let recovered = core
        .route_public(
            &envelope,
            source(100),
            MonotonicMillis(PUBLIC_LOBBY_ROUTE_IDLE_MILLIS),
        )
        .unwrap();
    assert_ne!(recovered.route_id, active.route_id);
    assert_ne!(recovered.peer_id, active.peer_id);
    assert_eq!(core.route_count(), 3);
}

#[test]
fn expiry_eagerly_removes_route_source_and_packet_queue() {
    let (mut core, route) = configured_core();
    let capability = capability(24);
    core.bind_capability(capability.clone(), binding(route), MonotonicMillis(0))
        .unwrap();
    let envelope = PublicEnvelope::new(RouteSelector::Capability(capability), vec![1]).unwrap();
    let source = source(1);
    core.route_public(&envelope, source, MonotonicMillis(1))
        .unwrap();
    core.enqueue_packet(route.route_id, vec![2]).unwrap();

    let teardowns = core.expire(MonotonicMillis(CAPABILITY_IDLE_MILLIS + 1));
    assert_eq!(
        teardowns,
        vec![RouteTeardown {
            route_id: route.route_id,
            worker_id: route.worker_id,
            peer_id: route.peer_id,
            reason: RoutingErrorCategory::RouteExpired,
        }]
    );
    assert_eq!(core.route_count(), 0);
    assert_eq!(core.source_for_route(route.route_id), None);
    assert!(core.drain_packets(8).is_empty());
    assert_eq!(core.metrics().routes_cleaned, 1);
    assert_eq!(
        core.expire(MonotonicMillis(CAPABILITY_IDLE_MILLIS + 2)),
        Vec::new()
    );
}

#[test]
fn authorization_expiry_returns_the_same_teardown_that_it_applies() {
    let (mut core, route) = configured_core();
    let capability = capability(25);
    core.bind_capability(capability.clone(), binding(route), MonotonicMillis(0))
        .unwrap();
    let envelope =
        PublicEnvelope::new(RouteSelector::Capability(capability.clone()), vec![1]).unwrap();
    core.route_public(&envelope, source(1), MonotonicMillis(1))
        .unwrap();
    core.enqueue_packet(route.route_id, vec![2]).unwrap();

    let (result, teardown) = core.authorize_with_teardown(
        &capability,
        source(1),
        MonotonicMillis(CAPABILITY_IDLE_MILLIS + 1),
    );
    assert_eq!(result, Err(RoutingErrorCategory::RouteExpired));
    assert_eq!(
        teardown,
        Some(RouteTeardown {
            route_id: route.route_id,
            worker_id: route.worker_id,
            peer_id: route.peer_id,
            reason: RoutingErrorCategory::RouteExpired,
        })
    );
    assert_eq!(core.route_count(), 0);
    assert_eq!(core.source_for_route(route.route_id), None);
    assert!(core.drain_packets(8).is_empty());
}

#[test]
fn explicit_expiry_counts_errors_and_purges_negative_records_at_hard_limit() {
    let (mut core, route) = configured_core();
    let pending = capability(6);
    core.bind_capability(pending.clone(), binding(route), MonotonicMillis(0))
        .unwrap();
    core.expire(MonotonicMillis(CAPABILITY_PENDING_MILLIS));
    assert_eq!(
        core.capability_status(&pending),
        Some(CapabilityStatus::Revoked)
    );
    assert_eq!(core.metrics().capabilities_revoked, 1);
    assert_eq!(
        core.metrics().error_counts[&RoutingErrorCategory::PendingExpired],
        1
    );
    core.expire(MonotonicMillis(CAPABILITY_HARD_LIFETIME_MILLIS));
    assert_eq!(core.capability_status(&pending), None);
}

#[test]
fn route_and_worker_packet_bounds_drop_newest_without_disturbing_existing_order() {
    let config = CoreConfig {
        route_packet_frames: 2,
        route_packet_bytes: 10_000,
        worker_packet_frames: 3,
        worker_packet_bytes: 10_000,
        ..CoreConfig::default()
    };
    let mut core = SupervisorCore::new(config);
    let worker = worker(1, WorkerKind::Match);
    core.register_worker(worker).unwrap();
    let a = route(10, worker.worker_id, false);
    let b = route(11, worker.worker_id, false);
    core.register_route(a).unwrap();
    core.register_route(b).unwrap();
    core.enqueue_packet(a.route_id, vec![1]).unwrap();
    core.enqueue_packet(a.route_id, vec![2]).unwrap();
    assert_eq!(
        core.enqueue_packet(a.route_id, vec![99]),
        Err(RoutingErrorCategory::PacketQueueFull)
    );
    core.enqueue_packet(b.route_id, vec![3]).unwrap();
    assert_eq!(
        core.enqueue_packet(b.route_id, vec![4]),
        Err(RoutingErrorCategory::PacketQueueFull)
    );
    let drained = core.drain_packets(10);
    assert_eq!(
        drained
            .iter()
            .map(|packet| packet.payload[0])
            .collect::<Vec<_>>(),
        vec![1, 3, 2]
    );
    assert_eq!(core.metrics().packet_dropped_newest, 2);
    assert_eq!(core.metrics().packet_high_water.frames, 3);
    assert_eq!(core.metrics().packet_current.frames, 0);
}

#[test]
fn byte_bounds_include_packet_record_header() {
    let config = CoreConfig {
        route_packet_frames: 10,
        route_packet_bytes: PACKET_HEADER_BYTES + 2,
        worker_packet_frames: 10,
        worker_packet_bytes: PACKET_HEADER_BYTES + 2,
        ..CoreConfig::default()
    };
    let (mut core, route) = {
        let mut core = SupervisorCore::new(config);
        let worker = worker(1, WorkerKind::Match);
        core.register_worker(worker).unwrap();
        let route = route(10, worker.worker_id, false);
        core.register_route(route).unwrap();
        (core, route)
    };
    core.enqueue_packet(route.route_id, vec![1, 2]).unwrap();
    assert_eq!(
        core.enqueue_packet(route.route_id, vec![3]),
        Err(RoutingErrorCategory::PacketQueueFull)
    );
    assert_eq!(
        core.metrics().packet_high_water.bytes,
        PACKET_HEADER_BYTES + 2
    );
}

#[test]
fn lobby_has_reserved_service_and_match_workers_rotate_deterministically() {
    let mut core = SupervisorCore::default();
    let lobby = worker(1, WorkerKind::Lobby);
    let match_a = worker(2, WorkerKind::Match);
    let match_b = worker(3, WorkerKind::Match);
    for worker in [lobby, match_a, match_b] {
        core.register_worker(worker).unwrap();
    }
    let lobby_route = route(10, lobby.worker_id, true);
    let route_a = route(20, match_a.worker_id, false);
    let route_b = route(30, match_b.worker_id, false);
    for route in [lobby_route, route_a, route_b] {
        core.register_route(route).unwrap();
    }
    for value in 0..3 {
        core.enqueue_packet(lobby_route.route_id, vec![10 + value])
            .unwrap();
        core.enqueue_packet(route_a.route_id, vec![20 + value])
            .unwrap();
        core.enqueue_packet(route_b.route_id, vec![30 + value])
            .unwrap();
    }
    let workers: Vec<_> = core
        .drain_packets(9)
        .into_iter()
        .map(|packet| packet.worker_id)
        .collect();
    assert_eq!(
        workers,
        vec![
            lobby.worker_id,
            match_a.worker_id,
            lobby.worker_id,
            match_b.worker_id,
            lobby.worker_id,
            match_a.worker_id,
            match_b.worker_id,
            match_a.worker_id,
            match_b.worker_id
        ]
    );
}

#[test]
fn full_stalled_route_does_not_block_sibling_route_or_worker() {
    let config = CoreConfig {
        route_packet_frames: 1,
        route_packet_bytes: 10_000,
        ..CoreConfig::default()
    };
    let mut core = SupervisorCore::new(config);
    let a = worker(1, WorkerKind::Match);
    let b = worker(2, WorkerKind::Match);
    core.register_worker(a).unwrap();
    core.register_worker(b).unwrap();
    let stalled = route(10, a.worker_id, false);
    let sibling = route(11, a.worker_id, false);
    let independent = route(20, b.worker_id, false);
    for route in [stalled, sibling, independent] {
        core.register_route(route).unwrap();
    }
    core.enqueue_packet(stalled.route_id, vec![1]).unwrap();
    assert_eq!(
        core.enqueue_packet(stalled.route_id, vec![9]),
        Err(RoutingErrorCategory::PacketQueueFull)
    );
    core.enqueue_packet(sibling.route_id, vec![2]).unwrap();
    core.enqueue_packet(independent.route_id, vec![3]).unwrap();
    let routes: Vec<_> = core
        .drain_packets(3)
        .into_iter()
        .map(|packet| packet.route_id)
        .collect();
    assert!(routes.contains(&stalled.route_id));
    assert!(routes.contains(&sibling.route_id));
    assert!(routes.contains(&independent.route_id));
}

#[test]
fn control_queues_are_bounded_per_worker_and_round_robin() {
    let config = CoreConfig {
        worker_control_frames: 2,
        worker_control_bytes: 4,
        ..CoreConfig::default()
    };
    let mut core = SupervisorCore::new(config);
    let a = worker(1, WorkerKind::Match);
    let b = worker(2, WorkerKind::Match);
    core.register_worker(a).unwrap();
    core.register_worker(b).unwrap();
    core.enqueue_control(a.worker_id, vec![1, 1]).unwrap();
    core.enqueue_control(a.worker_id, vec![2, 2]).unwrap();
    assert_eq!(
        core.enqueue_control(a.worker_id, vec![3]),
        Err(RoutingErrorCategory::ControlQueueFull)
    );
    core.enqueue_control(b.worker_id, vec![4]).unwrap();
    let drained = core.drain_controls(3);
    assert_eq!(
        drained
            .iter()
            .map(|(worker, _)| *worker)
            .collect::<Vec<_>>(),
        vec![a.worker_id, b.worker_id, a.worker_id]
    );
    assert_eq!(core.metrics().control_rejected, 1);
    assert_eq!(core.metrics().control_high_water.frames, 3);
}

#[test]
fn worker_cleanup_is_exactly_once_and_revokes_owned_capabilities() {
    let (mut core, route) = configured_core();
    let token = capability(7);
    core.bind_capability(token.clone(), binding(route), MonotonicMillis(0))
        .unwrap();
    core.enqueue_packet(route.route_id, vec![1]).unwrap();
    core.enqueue_control(route.worker_id, vec![2]).unwrap();
    let report = core.cleanup_worker(route.worker_id).unwrap();
    assert_eq!(
        report,
        CleanupReport {
            routes_removed: 1,
            capabilities_revoked: 1,
            packet_frames_removed: 1,
            control_frames_removed: 1
        }
    );
    assert_eq!(core.cleanup_worker(route.worker_id), None);
    assert_eq!(
        core.capability_status(&token),
        Some(CapabilityStatus::Revoked)
    );
    assert_eq!(
        core.authorize(&token, source(1), MonotonicMillis(1)),
        Err(RoutingErrorCategory::Revoked)
    );
    assert_eq!(core.metrics().workers_cleaned, 1);
    assert_eq!(core.metrics().routes_cleaned, 1);
    assert_eq!(core.metrics().capabilities_revoked, 1);
    assert_eq!(core.metrics().packet_current, QueueHighWater::default());
    assert_eq!(core.metrics().control_current, QueueHighWater::default());
}

#[test]
fn worker_peer_close_removes_only_matching_route_and_is_idempotent() {
    let (mut core, registered_route) = configured_core();
    let token = capability(77);
    core.bind_capability(token.clone(), binding(registered_route), MonotonicMillis(0))
        .unwrap();
    assert_eq!(
        core.close_route_from_worker(
            registered_route.worker_id,
            registered_route.route_id,
            registered_route.peer_id,
        )
        .unwrap()
        .map(|teardown| teardown.route_id),
        Some(registered_route.route_id)
    );
    assert_eq!(core.route_count(), 0);
    assert_eq!(
        core.capability_status(&token),
        Some(CapabilityStatus::Revoked)
    );
    assert_eq!(
        core.close_route_from_worker(
            registered_route.worker_id,
            registered_route.route_id,
            registered_route.peer_id,
        ),
        Ok(None)
    );

    let other_worker = worker(2, WorkerKind::Match);
    core.register_worker(other_worker).unwrap();
    let other_route = route(11, other_worker.worker_id, false);
    core.register_route(other_route).unwrap();
    assert_eq!(
        core.close_route_from_worker(
            other_worker.worker_id,
            other_route.route_id,
            registered_route.peer_id,
        ),
        Err(RoutingErrorCategory::Binding)
    );
    assert_eq!(core.route_count(), 1);
}
