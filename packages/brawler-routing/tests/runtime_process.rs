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
            manifest_version: 2,
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
        profile_database_path: "profiles.sqlite3".to_string(),
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
        mode: GameMode::Takedown,
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
        takedown_recent_hostile_damage_credit_ticks: 300,
        vault_break_critical_health_percent: 25,
        shard_rush: brawler_routing::ShardRushPolicy {
            hold_ticks: 720,
            spawn_interval_ticks: 360,
            live_shard_ceiling: 12,
        },
        team_count: 2,
        players_per_team: 2,
        participants: vec![
            AllocateParticipant {
                lobby_session_id,
                player_id: id64(201),
                netcode_client_id: id64(301),
                team: 0,
                display_name: brawler_routing::MatchDisplayName::new("Player 1").unwrap(),
                recipe_fingerprint: 11,
                build_revision: 1,
                build_snapshot: brawler_routing::MatchBuildSnapshot::new(&[1]).unwrap(),
            },
            AllocateParticipant {
                lobby_session_id: id128(102),
                player_id: id64(202),
                netcode_client_id: id64(302),
                team: 1,
                display_name: brawler_routing::MatchDisplayName::new("Player 2").unwrap(),
                recipe_fingerprint: 12,
                build_revision: 1,
                build_snapshot: brawler_routing::MatchBuildSnapshot::new(&[2]).unwrap(),
            },
            AllocateParticipant {
                lobby_session_id: id128(103),
                player_id: id64(203),
                netcode_client_id: id64(303),
                team: 0,
                display_name: brawler_routing::MatchDisplayName::new("Player 3").unwrap(),
                recipe_fingerprint: 13,
                build_revision: 1,
                build_snapshot: brawler_routing::MatchBuildSnapshot::new(&[3]).unwrap(),
            },
            AllocateParticipant {
                lobby_session_id: id128(104),
                player_id: id64(204),
                netcode_client_id: id64(304),
                team: 1,
                display_name: brawler_routing::MatchDisplayName::new("Player 4").unwrap(),
                recipe_fingerprint: 14,
                build_revision: 1,
                build_snapshot: brawler_routing::MatchBuildSnapshot::new(&[4]).unwrap(),
            },
        ],
        bots: Vec::new(),
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

/// BRL-q73k: a worker the runtime cleans up from its packet path is killed by the lifecycle
/// owner. That kill must publish `Failed` before the child's reap, so a `success=false` reap is
/// never left unexplained in the supervisor log.
#[test]
fn runtime_cleanup_kill_publishes_failed_before_the_reap() {
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
    runtime
        .spawn_worker(launch_spec_mode("malformed-packet"))
        .unwrap();

    let mut events = Vec::new();
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(4) {
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        events.extend(report.lifecycle_events);
        if events.iter().any(|event| {
            matches!(event, LifecycleEvent::ChildReaped { worker_id: id, .. } if *id == worker_id)
        }) {
            break;
        }
    }
    let failed = events.iter().position(|event| {
        matches!(
            event,
            LifecycleEvent::Failed {
                worker_id: id,
                category: brawler_routing::RoutingErrorCategory::IpcPacketClosed,
            } if *id == worker_id
        )
    });
    let reaped = events.iter().position(|event| {
        matches!(event, LifecycleEvent::ChildReaped { worker_id: id, status }
            if *id == worker_id && status.signal == Some(9))
    });
    assert!(
        failed.is_some(),
        "the cleanup kill published no Failed: {events:?}"
    );
    assert!(
        reaped.is_some(),
        "the killed worker was not reaped by SIGKILL: {events:?}"
    );
    assert!(failed < reaped, "Failed precedes the reap: {events:?}");
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

/// Allocate one match whose fake worker runs `mode`, then poll until that match is reaped.
fn allocate_match_and_collect_until_reaped(mode: &str) -> (SupervisorRuntime, Vec<LifecycleEvent>) {
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
    runtime.spawn_worker(launch_spec_mode("ready")).unwrap();
    let lobby_started = Instant::now();
    let mut lobby_ready = false;
    while lobby_started.elapsed() < Duration::from_secs(3) && !lobby_ready {
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        lobby_ready = report.lifecycle_events.iter().any(
            |event| matches!(event, LifecycleEvent::Ready { worker_id } if worker_id.get() == 7),
        );
    }
    assert!(lobby_ready, "lobby worker did not become ready");

    let mut request = allocation_request();
    request.map_preset = 0;
    request.map_recipe = format!("fake-worker-mode={mode}").into_bytes();
    let mut events = runtime
        .submit_allocation_request(id128(7), request)
        .unwrap();
    let started = Instant::now();
    while started.elapsed() < Duration::from_secs(5) {
        let report = runtime.poll_once(Some(Duration::from_millis(10))).unwrap();
        let reaped = report.lifecycle_events.iter().any(|event| {
            matches!(event, LifecycleEvent::ChildReaped { worker_id, .. } if worker_id.get() != 7)
        });
        events.extend(report.lifecycle_events);
        if reaped {
            break;
        }
    }
    // One more owner turn applies the terminal reconciliation that follows the reap.
    let report = runtime.poll_once(Some(Duration::from_millis(150))).unwrap();
    events.extend(report.lifecycle_events);
    (runtime, events)
}

fn lobby_was_disturbed(events: &[LifecycleEvent]) -> bool {
    events.iter().any(|event| match event {
        LifecycleEvent::Failed { worker_id, .. }
        | LifecycleEvent::RestartScheduled { worker_id, .. }
        | LifecycleEvent::Stopped { worker_id, .. } => worker_id.get() == 7,
        _ => false,
    })
}

/// BRL-xndc/BRL-5yss: a match worker whose formation failed reports `CancelActivation`, then
/// retires through an unsolicited Exit and a successful exit. The supervisor records an ordered
/// stop rather than `WorkerExitMismatch`, retires the match routes, capabilities, and allocation,
/// and leaves the lobby running.
#[test]
fn runtime_retires_a_formation_failed_match_through_exit_without_failure() {
    let (mut runtime, events) = allocate_match_and_collect_until_reaped("formation-failure");
    let match_worker = events
        .iter()
        .find_map(|event| match event {
            LifecycleEvent::Spawned { worker_id, .. } if worker_id.get() != 7 => Some(*worker_id),
            _ => None,
        })
        .expect("match worker spawned");
    let exit_at = events.iter().position(|event| {
        matches!(event, LifecycleEvent::ExitReceived { worker_id, exit }
            if *worker_id == match_worker && exit.exit_category == 0)
    });
    let stopped_at = events.iter().position(|event| {
        matches!(event, LifecycleEvent::Stopped { worker_id, forced: false }
            if *worker_id == match_worker)
    });
    assert!(exit_at.is_some(), "match Exit was not observed: {events:?}");
    assert!(
        stopped_at.is_some(),
        "match was not stopped gracefully: {events:?}"
    );
    assert!(exit_at < stopped_at, "Exit precedes the stop: {events:?}");
    assert!(
        !events
            .iter()
            .any(|event| matches!(event, LifecycleEvent::Failed { .. })),
        "a formation-failure retirement is not a worker failure: {events:?}"
    );
    assert!(!lobby_was_disturbed(&events), "{events:?}");
    assert_eq!(runtime.core().worker_count(), 1, "only the lobby remains");
    assert_eq!(
        runtime.core().route_count(),
        1,
        "only the lobby default route remains"
    );
    assert_eq!(runtime.core().live_capability_count(), 0);
    // The allocation record is retired, so the same request is admitted again rather than
    // deduplicated against a stale record.
    let resubmitted = runtime
        .submit_allocation_request(id128(7), allocation_request())
        .unwrap();
    assert!(
        resubmitted
            .iter()
            .any(|event| matches!(event, LifecycleEvent::Spawned { .. })),
        "{resubmitted:?}"
    );
    runtime.stop_handle().request().unwrap();
    runtime.run().unwrap();
}

/// A match that crashes after Ready, without Exit, remains a genuine worker failure.
#[test]
fn runtime_still_classifies_a_crashed_match_as_a_failure() {
    let (mut runtime, events) = allocate_match_and_collect_until_reaped("crash-after-ready");
    assert!(
        events.iter().any(|event| matches!(
            event,
            LifecycleEvent::Failed {
                worker_id,
                category: brawler_routing::RoutingErrorCategory::WorkerCrash,
            } if worker_id.get() != 7
        )),
        "{events:?}"
    );
    assert!(!lobby_was_disturbed(&events), "{events:?}");
    assert_eq!(runtime.core().worker_count(), 1);
    assert_eq!(runtime.core().live_capability_count(), 0);
    runtime.stop_handle().request().unwrap();
    runtime.run().unwrap();
}
