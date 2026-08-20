//! M01 process-isolation evidence with the production runtime and real child processes.
//!
//! These tests intentionally use the Bevy-free fake worker only as an IPC probe. Each child gets
//! its own manifest, Unix packet/control sockets, route, peer, and capability. The supervisor must
//! keep those identities isolated while one child stalls or exits.

use std::{
    fs,
    net::{SocketAddr, UdpSocket},
    path::PathBuf,
    time::{Duration, Instant},
};

use brawler_routing::{
    CAPABILITY_BYTES, CONTROL_VERSION_CURRENT, Capability, CapabilityBinding, CoreConfig, GameMode,
    Generation, LifecycleEvent, LobbyManifest, LogicalServerId, ManifestBody, ManifestCommon,
    MatchManifestParticipant, MatchManifestV1, PACKET_VERSION_V1, PeerId, ProcessSupervisorConfig,
    PublicEnvelope, ROUTE_VERSION_V1, RouteId, RouteRegistration, RouteSelector, RuntimeConfig,
    RuntimePollReport, SupervisorRuntime, WorkerId, WorkerKind, WorkerLaunchSpec,
    WorkerRegistration, WorkerRole,
};

const LOGICAL_SERVER_ID: u128 = 1;
const SUPERVISOR_GENERATION: u64 = 2;
const NETWORK_PROTOCOL: u64 = 3;
const CONTENT_FINGERPRINT: u64 = 4;

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

fn process_config() -> ProcessSupervisorConfig {
    let mut config = ProcessSupervisorConfig::new(
        id128(LOGICAL_SERVER_ID),
        id64(SUPERVISOR_GENERATION),
        NETWORK_PROTOCOL,
        CONTENT_FINGERPRINT,
    );
    // Keep the isolation tests bounded even for the deliberately parked worker. The values still
    // preserve the production ordering: graceful stop, then forced reap, then shutdown deadline.
    config.graceful_stop = Duration::from_millis(200);
    config.forced_reap = Duration::from_millis(200);
    config.shutdown_deadline = Duration::from_secs(1);
    config
}

fn runtime() -> SupervisorRuntime {
    let logical_server_id = id128::<LogicalServerId>(LOGICAL_SERVER_ID);
    let supervisor_generation = id64::<Generation>(SUPERVISOR_GENERATION);
    SupervisorRuntime::new(RuntimeConfig {
        public_bind: SocketAddr::from(([127, 0, 0, 1], 0)),
        core: CoreConfig::with_identity(
            logical_server_id,
            supervisor_generation,
            NETWORK_PROTOCOL,
            CONTENT_FINGERPRINT,
        ),
        process_supervisor: Some(process_config()),
        ..RuntimeConfig::default()
    })
    .unwrap()
}

fn fake_worker() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_brawler-routing-fake-worker"))
}

fn lobby_spec(mode: &str) -> WorkerLaunchSpec {
    let registration = WorkerRegistration {
        worker_id: id128(7),
        process_id: id128(17),
        generation: id64(1),
        kind: WorkerKind::Lobby,
    };
    let manifest = LobbyManifest {
        common: ManifestCommon {
            manifest_version: 1,
            role: WorkerRole::Lobby,
            logical_server_id: id128(LOGICAL_SERVER_ID),
            process_id: registration.process_id,
            worker_id: registration.worker_id,
            generation: registration.generation,
            network_protocol: NETWORK_PROTOCOL,
            protocol_registry_fingerprint: 9,
            content_fingerprint: CONTENT_FINGERPRINT,
            route_version: ROUTE_VERSION_V1,
            packet_version: PACKET_VERSION_V1,
            control_version: CONTROL_VERSION_CURRENT,
            flags: 0,
        },
        default_route_id: id128(100),
        max_authenticated_sessions: 8,
        outstanding_allocations: 2,
        active_matches: 2,
        heartbeat_ms: 100,
        raw_catalog: b"catalog".to_vec(),
        raw_catalog_fingerprint: brawler_routing::raw_catalog_fingerprint(b"catalog"),
        nonce: 6,
        digest: [0; 32],
    };
    WorkerLaunchSpec::new(
        fake_worker(),
        registration,
        ManifestBody::from_lobby(&manifest).unwrap(),
    )
    .with_environment("BRAWLER_FAKE_WORKER_MODE", mode)
}

fn match_spec(worker: u128, mode: &str) -> WorkerLaunchSpec {
    let worker_u64 = u64::try_from(worker).unwrap();
    let registration = WorkerRegistration {
        worker_id: id128(worker),
        process_id: id128(worker + 10),
        generation: id64(1),
        kind: WorkerKind::Match,
    };
    let participant = MatchManifestParticipant {
        lobby_session_id: id128(10_000 + worker),
        player_id: id64(20_000 + worker_u64),
        netcode_client_id: id64(30_000 + worker_u64),
        peer_id: id128(40_000 + worker),
        team: 0,
        display_name: brawler_routing::MatchDisplayName::new("Player").unwrap(),
        source_build_preset: Some(1),
        recipe_fingerprint: worker_u64,
        revision: 1,
        build_snapshot: brawler_routing::MatchBuildSnapshot::new(&[1]).unwrap(),
    };
    let manifest = MatchManifestV1 {
        common: ManifestCommon {
            manifest_version: 2,
            role: WorkerRole::Match,
            logical_server_id: id128(LOGICAL_SERVER_ID),
            process_id: registration.process_id,
            worker_id: registration.worker_id,
            generation: registration.generation,
            network_protocol: NETWORK_PROTOCOL,
            protocol_registry_fingerprint: 9,
            content_fingerprint: CONTENT_FINGERPRINT,
            route_version: ROUTE_VERSION_V1,
            packet_version: PACKET_VERSION_V1,
            control_version: CONTROL_VERSION_CURRENT,
            flags: 0,
        },
        request_id: brawler_routing::RequestId::new(45_000 + worker_u64).unwrap(),
        match_id: id128(50_000 + worker),
        allocation_id: id128(60_000 + worker),
        mode: GameMode::Wipeout,
        map_preset: 1,
        map_revision: 1,
        rules_profile: 1,
        objective_target: 10,
        match_duration_ticks: 10_800,
        countdown_ticks: 180,
        respawn_ticks: 180,
        reserved: 0,
        seed: worker_u64,
        participants: vec![participant],
        heartbeat_ms: 100,
        nonce: 70_000 + worker,
        digest: [0; 32],
    };
    WorkerLaunchSpec::new(
        fake_worker(),
        registration,
        ManifestBody::from_match(&manifest).unwrap(),
    )
    .with_environment("BRAWLER_FAKE_WORKER_MODE", mode)
}

fn wait_for_ready(runtime: &mut SupervisorRuntime, workers: &[WorkerId]) -> Vec<LifecycleEvent> {
    let deadline = Instant::now() + Duration::from_secs(3);
    let mut ready = Vec::new();
    let mut events = Vec::new();
    while Instant::now() < deadline && ready.len() < workers.len() {
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        for event in report.lifecycle_events {
            if let LifecycleEvent::Ready { worker_id } = event {
                if workers.contains(&worker_id) && !ready.contains(&worker_id) {
                    ready.push(worker_id);
                }
                events.push(LifecycleEvent::Ready { worker_id });
            } else {
                events.push(event);
            }
        }
    }
    assert_eq!(
        ready.len(),
        workers.len(),
        "workers did not become ready: {events:?}"
    );
    events
}

fn install_route(
    runtime: &mut SupervisorRuntime,
    worker_id: WorkerId,
    route_id: RouteId,
    peer_id: PeerId,
    capability: Capability,
    session: u128,
) {
    runtime
        .core_mut()
        .register_route(RouteRegistration {
            route_id,
            worker_id,
            peer_id,
            is_default_lobby: false,
        })
        .unwrap();
    runtime
        .core_mut()
        .bind_capability(
            capability,
            CapabilityBinding {
                logical_server_id: id128(LOGICAL_SERVER_ID),
                supervisor_generation: id64(SUPERVISOR_GENERATION),
                worker_id,
                worker_generation: id64(1),
                route_id,
                peer_id,
                lobby_session_id: id128(session),
                allocation_id: id128(route_id.get() + 10_000),
                match_id: id128(route_id.get() + 20_000),
                network_protocol: NETWORK_PROTOCOL,
                content_fingerprint: CONTENT_FINGERPRINT,
            },
            brawler_routing::MonotonicMillis(0),
        )
        .unwrap();
}

fn send_and_receive(
    runtime: &mut SupervisorRuntime,
    socket: &UdpSocket,
    capability: &Capability,
    payload: &[u8],
) -> Vec<u8> {
    let envelope = PublicEnvelope::new(
        RouteSelector::Capability(capability.clone()),
        payload.to_vec(),
    )
    .unwrap()
    .encode()
    .unwrap();
    socket
        .send_to(&envelope, runtime.public_addr().unwrap())
        .unwrap();
    let deadline = Instant::now() + Duration::from_secs(2);
    let mut bytes = [0; brawler_routing::PUBLIC_MAX_DATAGRAM_BYTES];
    while Instant::now() < deadline {
        runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        match socket.recv_from(&mut bytes) {
            Ok((length, _)) => {
                let response = PublicEnvelope::decode(&bytes[..length]).unwrap();
                return response.payload().to_vec();
            }
            Err(error) if error.kind() == std::io::ErrorKind::WouldBlock => {}
            Err(error) => panic!("unexpected UDP receive error: {error}"),
        }
    }
    panic!("worker did not echo payload within bounded deadline");
}

fn shutdown_and_assert_clean(mut runtime: SupervisorRuntime) {
    let runtime_dir = runtime.runtime_dir().unwrap().to_path_buf();
    runtime.stop_handle().request().unwrap();
    runtime.run().unwrap();
    assert_eq!(runtime.process_worker_count(), 0, "child lifecycle leaked");
    assert_eq!(runtime.core().worker_count(), 0, "worker registry leaked");
    assert_eq!(runtime.core().route_count(), 0, "route registry leaked");
    assert_eq!(runtime.core().metrics().packet_current.frames, 0);
    assert_eq!(runtime.core().metrics().packet_current.bytes, 0);
    assert_eq!(runtime.core().metrics().control_current.frames, 0);
    assert_eq!(runtime.core().metrics().control_current.bytes, 0);
    let leftovers = fs::read_dir(&runtime_dir)
        .unwrap()
        .collect::<Result<Vec<_>, _>>()
        .unwrap();
    assert!(
        leftovers.is_empty(),
        "IPC socket files leaked: {leftovers:?}"
    );
    drop(runtime);
    assert!(!runtime_dir.exists(), "private runtime directory leaked");
}

fn launch_lobby_and_matches(runtime: &mut SupervisorRuntime, first_mode: &str, second_mode: &str) {
    runtime.spawn_worker(lobby_spec("ready")).unwrap();
    runtime.spawn_worker(match_spec(8, first_mode)).unwrap();
    runtime.spawn_worker(match_spec(9, second_mode)).unwrap();
    wait_for_ready(runtime, &[id128(7), id128(8), id128(9)]);
}

#[test]
fn two_match_processes_route_opaque_packets_to_exact_worker_and_peer() {
    let mut runtime = runtime();
    launch_lobby_and_matches(&mut runtime, "packet-echo", "packet-echo");
    let first = Capability::from_bytes([1; CAPABILITY_BYTES]).unwrap();
    let second = Capability::from_bytes([2; CAPABILITY_BYTES]).unwrap();
    install_route(
        &mut runtime,
        id128(8),
        id128(1_008),
        id128(2_008),
        first.clone(),
        8_008,
    );
    install_route(
        &mut runtime,
        id128(9),
        id128(1_009),
        id128(2_009),
        second.clone(),
        8_009,
    );

    let first_client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let second_client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    first_client.set_nonblocking(true).unwrap();
    second_client.set_nonblocking(true).unwrap();
    assert_eq!(
        send_and_receive(&mut runtime, &first_client, &first, b"worker-eight"),
        b"worker-eight"
    );
    assert_eq!(
        send_and_receive(&mut runtime, &second_client, &second, b"worker-nine"),
        b"worker-nine"
    );
    assert_eq!(runtime.core().worker_count(), 3);
    assert_eq!(runtime.core().route_count(), 3);
    shutdown_and_assert_clean(runtime);
}

#[test]
fn stalled_match_does_not_stop_or_reroute_the_other_match() {
    let mut runtime = runtime();
    launch_lobby_and_matches(&mut runtime, "stall", "packet-echo");
    let stalled = Capability::from_bytes([3; CAPABILITY_BYTES]).unwrap();
    let live = Capability::from_bytes([4; CAPABILITY_BYTES]).unwrap();
    install_route(
        &mut runtime,
        id128(8),
        id128(1_018),
        id128(2_018),
        stalled.clone(),
        8_018,
    );
    install_route(
        &mut runtime,
        id128(9),
        id128(1_019),
        id128(2_019),
        live.clone(),
        8_019,
    );
    let stalled_client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    let live_client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    stalled_client.set_nonblocking(true).unwrap();
    live_client.set_nonblocking(true).unwrap();

    // The parked child accepts no packet bytes, but the supervisor's bounded queue and stream
    // remain independent. The second child must still receive and return its own route.
    let stalled_envelope = PublicEnvelope::new(
        RouteSelector::Capability(stalled),
        b"stalled-route".to_vec(),
    )
    .unwrap()
    .encode()
    .unwrap();
    stalled_client
        .send_to(&stalled_envelope, runtime.public_addr().unwrap())
        .unwrap();
    runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
    assert_eq!(runtime.core().worker_count(), 3);
    assert_eq!(runtime.core().route_count(), 3);
    assert_eq!(
        send_and_receive(&mut runtime, &live_client, &live, b"live-route"),
        b"live-route"
    );
    assert_eq!(runtime.core().worker_count(), 3);
    shutdown_and_assert_clean(runtime);
}

#[test]
fn crashing_match_cleans_only_its_routes_and_child() {
    let mut runtime = runtime();
    launch_lobby_and_matches(&mut runtime, "crash-after-ready", "packet-echo");
    let crashed = Capability::from_bytes([5; CAPABILITY_BYTES]).unwrap();
    let live = Capability::from_bytes([6; CAPABILITY_BYTES]).unwrap();
    install_route(
        &mut runtime,
        id128(8),
        id128(1_028),
        id128(2_028),
        crashed,
        8_028,
    );
    install_route(
        &mut runtime,
        id128(9),
        id128(1_029),
        id128(2_029),
        live.clone(),
        8_029,
    );

    let deadline = Instant::now() + Duration::from_secs(3);
    let mut crashed_observed = false;
    while Instant::now() < deadline {
        let report: RuntimePollReport = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        crashed_observed |= report.lifecycle_events.iter().any(|event| {
            matches!(
                event,
                LifecycleEvent::Failed {
                    worker_id,
                    category: brawler_routing::RoutingErrorCategory::WorkerCrash,
                } if worker_id.get() == 8
            )
        });
        if crashed_observed && runtime.core().route_count() == 2 {
            break;
        }
    }
    assert!(
        crashed_observed,
        "crashed match was not reconciled by OS status"
    );
    assert_eq!(runtime.core().worker_count(), 2);
    assert_eq!(
        runtime.core().route_count(),
        2,
        "crash rerouted or retained its route"
    );

    let live_client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    live_client.set_nonblocking(true).unwrap();
    assert_eq!(
        send_and_receive(&mut runtime, &live_client, &live, b"surviving-match"),
        b"surviving-match"
    );
    assert_eq!(runtime.core().worker_count(), 2);
    shutdown_and_assert_clean(runtime);
}
