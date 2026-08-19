use std::{
    net::{SocketAddr, UdpSocket},
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

use brawler_routing::{
    AllocateParticipant, AllocateRequestBody, AllocationPolicy, CONTROL_VERSION_CURRENT,
    CoreConfig, GameMode, LifecycleEvent, LobbyManifest, ManifestBody, ManifestCommon,
    PACKET_VERSION_V1, ProcessSupervisorConfig, ROUTE_VERSION_V1, RouteSelector, RuntimeConfig,
    StopId, SupervisorRuntime, WorkerKind, WorkerLaunchSpec, WorkerRegistration, WorkerRole,
};

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

fn launch_lobby_spec() -> WorkerLaunchSpec {
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
            logical_server_id: id128(1),
            process_id: registration.process_id,
            worker_id: registration.worker_id,
            generation: registration.generation,
            network_protocol: 3,
            protocol_registry_fingerprint: 0,
            content_fingerprint: 4,
            route_version: ROUTE_VERSION_V1,
            packet_version: PACKET_VERSION_V1,
            control_version: CONTROL_VERSION_CURRENT,
            flags: 0,
        },
        default_route_id: id128(100),
        max_authenticated_sessions: 8,
        outstanding_allocations: 2,
        active_matches: 1,
        heartbeat_ms: 100,
        raw_catalog: b"catalog".to_vec(),
        raw_catalog_fingerprint: brawler_routing::raw_catalog_fingerprint(b"catalog"),
        nonce: 6,
        digest: [0; 32],
    };
    WorkerLaunchSpec::new(
        PathBuf::from(env!("CARGO_BIN_EXE_brawler-routing-fake-worker")),
        registration,
        ManifestBody::from_lobby(&manifest).unwrap(),
    )
}

fn launch_spec() -> WorkerLaunchSpec {
    launch_lobby_spec()
}

fn launch_spec_mode(mode: &str) -> WorkerLaunchSpec {
    launch_spec().with_environment("BRAWLER_FAKE_WORKER_MODE", mode)
}

fn allocation_request() -> AllocateRequestBody {
    let lobby_session_id = id128(101);
    AllocateRequestBody {
        request_id: id64(501),
        lobby_session_id,
        mode: GameMode::Wipeout,
        map_preset: 1,
        map_revision: 1,
        rules_profile: 1,
        team_count: 2,
        players_per_team: 2,
        participants: vec![
            AllocateParticipant {
                lobby_session_id,
                player_id: id64(201),
                netcode_client_id: id64(301),
                team: 0,
                source_build_preset: Some(1),
                recipe_fingerprint: 11,
                build_revision: 1,
                build_snapshot: brawler_routing::MatchBuildSnapshot::new(&[1]).unwrap(),
            },
            AllocateParticipant {
                lobby_session_id: id128(102),
                player_id: id64(202),
                netcode_client_id: id64(302),
                team: 1,
                source_build_preset: Some(2),
                recipe_fingerprint: 12,
                build_revision: 1,
                build_snapshot: brawler_routing::MatchBuildSnapshot::new(&[2]).unwrap(),
            },
            AllocateParticipant {
                lobby_session_id: id128(103),
                player_id: id64(203),
                netcode_client_id: id64(303),
                team: 0,
                source_build_preset: Some(3),
                recipe_fingerprint: 13,
                build_revision: 1,
                build_snapshot: brawler_routing::MatchBuildSnapshot::new(&[3]).unwrap(),
            },
            AllocateParticipant {
                lobby_session_id: id128(104),
                player_id: id64(204),
                netcode_client_id: id64(304),
                team: 1,
                source_build_preset: Some(4),
                recipe_fingerprint: 14,
                build_revision: 1,
                build_snapshot: brawler_routing::MatchBuildSnapshot::new(&[4]).unwrap(),
            },
        ],
    }
}

#[test]
fn runtime_owns_child_ipc_and_routes_public_udp_through_lobby_worker() {
    let logical_server_id = id128(1);
    let supervisor_generation = id64(2);
    let mut runtime = SupervisorRuntime::new(RuntimeConfig {
        public_bind: SocketAddr::from(([127, 0, 0, 1], 0)),
        core: CoreConfig::with_identity(logical_server_id, supervisor_generation, 3, 4),
        process_supervisor: Some(ProcessSupervisorConfig::new(
            logical_server_id,
            supervisor_generation,
            3,
            4,
        )),
        ..RuntimeConfig::default()
    })
    .unwrap();
    runtime.spawn_worker(launch_spec()).unwrap();

    let started = Instant::now();
    let mut ready = false;
    while started.elapsed() < Duration::from_secs(3) {
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        ready |= report.lifecycle_events.iter().any(
            |event| matches!(event, LifecycleEvent::Ready { worker_id } if worker_id.get() == 7),
        );
        if ready {
            break;
        }
        thread::sleep(Duration::from_millis(5));
    }
    assert!(
        ready,
        "child worker did not complete control-plane readiness"
    );

    let client = UdpSocket::bind(SocketAddr::from(([127, 0, 0, 1], 0))).unwrap();
    client
        .send_to(
            &brawler_routing::PublicEnvelope::new(RouteSelector::DefaultLobby, vec![1, 2, 3])
                .unwrap()
                .encode()
                .unwrap(),
            runtime.public_addr().unwrap(),
        )
        .unwrap();
    let mut routed = false;
    let deadline = Instant::now() + Duration::from_secs(1);
    while Instant::now() < deadline {
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        routed |= report.packets_to_workers > 0;
        if routed {
            break;
        }
    }
    assert!(routed, "public UDP payload did not reach worker packet IPC");

    runtime.stop_handle().request().unwrap();
    runtime.run().unwrap();
}

#[test]
fn runtime_reconciles_exit_before_immediate_child_reap() {
    let logical_server_id = id128(1);
    let supervisor_generation = id64(2);
    let mut runtime = SupervisorRuntime::new(RuntimeConfig {
        public_bind: SocketAddr::from(([127, 0, 0, 1], 0)),
        core: CoreConfig::with_identity(logical_server_id, supervisor_generation, 3, 4),
        process_supervisor: Some(ProcessSupervisorConfig::new(
            logical_server_id,
            supervisor_generation,
            3,
            4,
        )),
        ..RuntimeConfig::default()
    })
    .unwrap();
    let worker_id = id128(7);
    runtime.spawn_worker(launch_spec()).unwrap();

    let started = Instant::now();
    let mut ready = false;
    while started.elapsed() < Duration::from_secs(3) {
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        ready |= report.lifecycle_events.iter().any(
            |event| matches!(event, LifecycleEvent::Ready { worker_id: id } if *id == worker_id),
        );
        if ready {
            break;
        }
    }
    assert!(ready, "child worker did not become ready before stop");
    assert!(
        runtime
            .stop_worker(worker_id, StopId::new(91).unwrap(), 1)
            .unwrap()
    );

    let mut saw_exit = false;
    let mut saw_stopped = false;
    let mut events = Vec::new();
    let deadline = Instant::now() + Duration::from_secs(3);
    while Instant::now() < deadline {
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        saw_exit |= report.lifecycle_events.iter().any(
            |event| matches!(event, LifecycleEvent::ExitReceived { worker_id: id, .. } if *id == worker_id),
        );
        saw_stopped |= report.lifecycle_events.iter().any(
            |event| matches!(event, LifecycleEvent::Stopped { worker_id: id, forced: false } if *id == worker_id),
        );
        events.extend(report.lifecycle_events);
        if saw_stopped {
            break;
        }
    }
    assert!(saw_exit, "worker Exit body was not surfaced before reap");
    assert!(
        saw_stopped,
        "worker did not stop after its valid Exit: {events:?}"
    );
    assert!(!events.iter().any(|event| {
        matches!(
            event,
            LifecycleEvent::Failed {
                category: brawler_routing::RoutingErrorCategory::WorkerExitMismatch,
                ..
            }
        )
    }));
    runtime.stop_handle().request().unwrap();
    runtime.run().unwrap();
}

#[test]
fn runtime_restarts_crashed_lobby_after_bounded_backoff() {
    let logical_server_id = id128(1);
    let supervisor_generation = id64(2);
    let mut runtime = SupervisorRuntime::new(RuntimeConfig {
        public_bind: SocketAddr::from(([127, 0, 0, 1], 0)),
        core: CoreConfig::with_identity(logical_server_id, supervisor_generation, 3, 4),
        process_supervisor: Some(ProcessSupervisorConfig::new(
            logical_server_id,
            supervisor_generation,
            3,
            4,
        )),
        ..RuntimeConfig::default()
    })
    .unwrap();
    let worker_id = id128(7);
    let original = launch_spec_mode("crash");
    let original_registration = original.registration;
    runtime.spawn_worker(original).unwrap();

    let mut saw_restart_schedule = false;
    let mut saw_restart_spawn = false;
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(4) {
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        saw_restart_schedule |= report.lifecycle_events.iter().any(|event| {
            matches!(event, LifecycleEvent::RestartScheduled { worker_id: id, .. } if *id == worker_id)
        });
        if saw_restart_schedule {
            saw_restart_spawn |= report.lifecycle_events.iter().any(|event| {
                matches!(event, LifecycleEvent::Spawned { worker_id: id, .. } if *id == worker_id)
            });
        }
        if saw_restart_spawn {
            break;
        }
    }
    assert!(
        saw_restart_schedule,
        "crashed lobby did not schedule restart"
    );
    assert!(
        saw_restart_spawn,
        "lobby restart did not launch after backoff"
    );
    let restarted = runtime
        .worker_registration(worker_id)
        .expect("restarted lifecycle registration");
    assert_eq!(
        restarted.generation.get(),
        original_registration.generation.get() + 1
    );
    assert_ne!(restarted.process_id, original_registration.process_id);
    runtime.stop_handle().request().unwrap();
    runtime.run().unwrap();
}

#[test]
fn runtime_allocates_match_after_ready_and_deduplicates_request() {
    let logical_server_id = id128(1);
    let supervisor_generation = id64(2);
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_brawler-routing-fake-worker"));
    let mut runtime = SupervisorRuntime::new(RuntimeConfig {
        public_bind: SocketAddr::from(([127, 0, 0, 1], 0)),
        core: CoreConfig::with_identity(logical_server_id, supervisor_generation, 3, 4),
        process_supervisor: Some(ProcessSupervisorConfig::new(
            logical_server_id,
            supervisor_generation,
            3,
            4,
        )),
        allocation_policy: Some(AllocationPolicy::brawler_m01()),
        worker_executable: Some(executable),
        protocol_registry_fingerprint: Some(0),
        ..RuntimeConfig::default()
    })
    .unwrap();
    runtime.spawn_worker(launch_spec()).unwrap();

    let lobby_started = Instant::now();
    let mut lobby_ready = false;
    while lobby_started.elapsed() < Duration::from_secs(3) {
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        lobby_ready |= report.lifecycle_events.iter().any(
            |event| matches!(event, LifecycleEvent::Ready { worker_id } if worker_id.get() == 7),
        );
        if lobby_ready {
            break;
        }
    }
    assert!(lobby_ready, "lobby worker did not become ready");

    let request = allocation_request();
    runtime
        .submit_allocation_request(id128(7), request.clone())
        .unwrap();
    let allocation_started = Instant::now();
    let mut match_ready = false;
    let mut allocation_events = Vec::new();
    while allocation_started.elapsed() < Duration::from_secs(3) {
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        allocation_events.extend(report.lifecycle_events.clone());
        match_ready |= report.lifecycle_events.iter().any(
            |event| matches!(event, LifecycleEvent::Ready { worker_id } if worker_id.get() != 7),
        );
        if match_ready && runtime.core().capability_count() == 4 {
            break;
        }
    }
    assert!(
        match_ready,
        "match worker did not become ready: workers={}, routes={}, capabilities={}, events={allocation_events:?}",
        runtime.core().worker_count(),
        runtime.core().route_count(),
        runtime.core().capability_count(),
    );
    assert_eq!(runtime.core().capability_count(), 4);
    assert_eq!(runtime.core().route_count(), 5);
    assert_eq!(runtime.core().worker_count(), 2);

    let worker_count = runtime.core().worker_count();
    runtime
        .submit_allocation_request(id128(7), request)
        .unwrap();
    assert_eq!(runtime.core().worker_count(), worker_count);
    assert_eq!(runtime.core().capability_count(), 4);

    runtime.stop_handle().request().unwrap();
    runtime.run().unwrap();
}

#[test]
fn runtime_keeps_catalog_opaque_and_accepts_policy_supported_allocation_mode() {
    let logical_server_id = id128(1);
    let supervisor_generation = id64(2);
    let executable = PathBuf::from(env!("CARGO_BIN_EXE_brawler-routing-fake-worker"));
    let mut runtime = SupervisorRuntime::new(RuntimeConfig {
        public_bind: SocketAddr::from(([127, 0, 0, 1], 0)),
        core: CoreConfig::with_identity(logical_server_id, supervisor_generation, 3, 4),
        process_supervisor: Some(ProcessSupervisorConfig::new(
            logical_server_id,
            supervisor_generation,
            3,
            4,
        )),
        allocation_policy: Some(AllocationPolicy::brawler_m01()),
        worker_executable: Some(executable),
        protocol_registry_fingerprint: Some(0),
        ..RuntimeConfig::default()
    })
    .unwrap();
    runtime.spawn_worker(launch_lobby_spec()).unwrap();

    let lobby_started = Instant::now();
    let mut lobby_ready = false;
    while lobby_started.elapsed() < Duration::from_secs(3) {
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        lobby_ready |= report.lifecycle_events.iter().any(
            |event| matches!(event, LifecycleEvent::Ready { worker_id } if worker_id.get() == 7),
        );
        if lobby_ready {
            break;
        }
    }
    assert!(lobby_ready, "lobby worker did not become ready");

    let events = runtime
        .submit_allocation_request(id128(7), allocation_request())
        .expect("the supervisor admits a policy-supported request without parsing the catalog");
    assert!(
        events
            .iter()
            .any(|event| matches!(event, LifecycleEvent::Spawned { .. }))
    );
    assert_eq!(runtime.core().worker_count(), 2);
    assert_eq!(runtime.core().route_count(), 1);
    assert_eq!(runtime.core().capability_count(), 0);

    runtime.stop_handle().request().unwrap();
    runtime.run().unwrap();
}
