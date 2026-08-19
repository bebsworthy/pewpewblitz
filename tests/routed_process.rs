//! Bounded M01 process evidence using the production `brawler-server` worker entry points.
//!
//! The Bevy-free routing unit tests use a fake worker to isolate codec and queue behavior. This
//! integration test intentionally launches the real lobby and match-worker roles, lets the
//! supervisor allocate two independent matches, then kills one child. It proves that the second
//! Bevy process, its route/peer registrations, and its capabilities remain usable in the
//! supervisor registry until the bounded final shutdown.

use std::{
    collections::{HashMap, HashSet},
    path::PathBuf,
    process::Command,
    time::{Duration, Instant},
};

use brawler::server::{default_build_identity, routing_identity};
use brawler_routing::{
    AllocateParticipant, AllocateRequestBody, AllocationPolicy, CONTROL_VERSION_V1, CoreConfig,
    GameMode, Generation, LifecycleEvent, LobbyManifest, LobbySessionId, LogicalServerId,
    ManifestBody, ManifestCommon, NetcodeClientId, PACKET_VERSION_V1, PlayerId, ProcessId,
    ProcessSupervisorConfig, ROUTE_VERSION_V1, RequestId, RoutingErrorCategory, RuntimeConfig,
    StderrPolicy, SupervisorRuntime, WorkerId, WorkerKind, WorkerLaunchSpec, WorkerRegistration,
    WorkerRole,
};

const LOGICAL_SERVER_ID: u128 = 0x4d01;
const SUPERVISOR_GENERATION: u64 = 0x51;
const LOBBY_WORKER_ID: u128 = 0x100;
const LOBBY_PROCESS_ID: u128 = 0x1000;

fn real_worker_binary() -> PathBuf {
    PathBuf::from(env!("CARGO_BIN_EXE_brawler-server"))
}

fn runtime() -> SupervisorRuntime {
    let identity = routing_identity().expect("production routing identity is computable");
    let logical_server_id = LogicalServerId::new(LOGICAL_SERVER_ID).unwrap();
    let supervisor_generation = Generation::new(SUPERVISOR_GENERATION).unwrap();
    let mut process = ProcessSupervisorConfig::new(
        logical_server_id,
        supervisor_generation,
        identity.network_protocol,
        identity.content_fingerprint,
    );
    process.ready_timeout = Duration::from_secs(10);
    process.graceful_stop = Duration::from_millis(500);
    process.forced_reap = Duration::from_millis(500);
    process.shutdown_deadline = Duration::from_secs(2);
    process.stderr = StderrPolicy::Null;
    SupervisorRuntime::new(RuntimeConfig {
        core: CoreConfig::with_identity(
            logical_server_id,
            supervisor_generation,
            identity.network_protocol,
            identity.content_fingerprint,
        ),
        process_supervisor: Some(process),
        allocation_policy: Some(AllocationPolicy::brawler_m01()),
        worker_executable: Some(real_worker_binary()),
        protocol_registry_fingerprint: Some(identity.protocol_registry_fingerprint),
        ..RuntimeConfig::default()
    })
    .expect("production process runtime starts")
}

fn quiet(mut spec: WorkerLaunchSpec) -> WorkerLaunchSpec {
    spec.stderr = Some(StderrPolicy::Null);
    spec
}

fn lobby_spec() -> WorkerLaunchSpec {
    let identity = routing_identity().expect("production routing identity is computable");
    let worker_id = WorkerId::new(LOBBY_WORKER_ID).unwrap();
    let process_id = ProcessId::new(LOBBY_PROCESS_ID).unwrap();
    let manifest = LobbyManifest {
        common: ManifestCommon {
            manifest_version: 1,
            role: WorkerRole::Lobby,
            logical_server_id: LogicalServerId::new(LOGICAL_SERVER_ID).unwrap(),
            process_id,
            worker_id,
            generation: Generation::new(1).unwrap(),
            network_protocol: identity.network_protocol,
            protocol_registry_fingerprint: identity.protocol_registry_fingerprint,
            content_fingerprint: identity.content_fingerprint,
            route_version: ROUTE_VERSION_V1,
            packet_version: PACKET_VERSION_V1,
            control_version: CONTROL_VERSION_V1,
            flags: 0,
        },
        default_route_id: brawler_routing::RouteId::new(0x200).unwrap(),
        max_authenticated_sessions: 32,
        outstanding_allocations: 2,
        active_matches: 4,
        heartbeat_ms: 1_000,
        raw_catalog: include_bytes!("../config/server/game-types.ron").to_vec(),
        raw_catalog_fingerprint: brawler_routing::raw_catalog_fingerprint(include_bytes!(
            "../config/server/game-types.ron"
        )),
        nonce: 0x1234,
        digest: [0; 32],
    };
    quiet(WorkerLaunchSpec::new(
        real_worker_binary(),
        WorkerRegistration {
            worker_id,
            process_id,
            generation: Generation::new(1).unwrap(),
            kind: WorkerKind::Lobby,
        },
        ManifestBody::from_lobby(&manifest).expect("lobby manifest encodes"),
    ))
}

fn allocation_request(
    request_id: u64,
    base: u128,
    build: brawler::server::LobbyBuildIdentity,
) -> AllocateRequestBody {
    let lobby_session_id = LobbySessionId::new(base).unwrap();
    let participants = [
        AllocateParticipant {
            lobby_session_id,
            player_id: PlayerId::new(u64::try_from(base + 1).unwrap()).unwrap(),
            netcode_client_id: NetcodeClientId::new(u64::try_from(base + 101).unwrap()).unwrap(),
            team: 0,
            source_build_preset: build.source_build_preset,
            recipe_fingerprint: build.recipe_fingerprint,
            build_revision: build.build_revision,
        },
        AllocateParticipant {
            lobby_session_id: LobbySessionId::new(base + 1).unwrap(),
            player_id: PlayerId::new(u64::try_from(base + 2).unwrap()).unwrap(),
            netcode_client_id: NetcodeClientId::new(u64::try_from(base + 102).unwrap()).unwrap(),
            team: 1,
            source_build_preset: build.source_build_preset,
            recipe_fingerprint: build.recipe_fingerprint,
            build_revision: build.build_revision,
        },
    ];
    AllocateRequestBody {
        request_id: RequestId::new(request_id).unwrap(),
        lobby_session_id,
        mode: GameMode::Wipeout,
        participants: participants.to_vec(),
    }
}

fn worker_ids(events: &[LifecycleEvent]) -> Vec<WorkerId> {
    events
        .iter()
        .filter_map(|event| match event {
            LifecycleEvent::Spawned { worker_id, .. }
                if *worker_id != WorkerId::new(LOBBY_WORKER_ID).unwrap() =>
            {
                Some(*worker_id)
            }
            _ => None,
        })
        .collect()
}

fn wait_for_ready(
    runtime: &mut SupervisorRuntime,
    target: &[WorkerId],
    mut events: Vec<LifecycleEvent>,
) -> Vec<LifecycleEvent> {
    let deadline = Instant::now() + Duration::from_secs(12);
    while Instant::now() < deadline {
        let report = runtime
            .poll_once(Some(Duration::from_millis(10)))
            .expect("supervisor poll succeeds");
        events.extend(report.lifecycle_events);
        if target.iter().all(|worker_id| {
            events.iter().any(|event| {
                matches!(event, LifecycleEvent::Ready { worker_id: ready } if ready == worker_id)
            })
        }) {
            return events;
        }
        if let Some(failure) = events.iter().find(|event| {
            matches!(event, LifecycleEvent::Failed { worker_id, .. } if target.contains(worker_id))
        }) {
            panic!("production worker failed before Ready: {failure:?}");
        }
    }
    panic!("production workers did not become ready: {events:?}");
}

fn shutdown_and_assert_clean(mut runtime: SupervisorRuntime) {
    let runtime_dir = runtime.runtime_dir().unwrap().to_path_buf();
    runtime.stop_handle().request().unwrap();
    runtime.run().expect("bounded supervisor shutdown succeeds");
    assert_eq!(runtime.process_worker_count(), 0, "child workers leaked");
    assert_eq!(runtime.core().worker_count(), 0, "worker registry leaked");
    assert_eq!(runtime.core().route_count(), 0, "route registry leaked");
    assert_eq!(
        runtime.core().live_capability_count(),
        0,
        "live capability registry leaked"
    );
    assert_eq!(runtime.core().metrics().packet_current.frames, 0);
    assert_eq!(runtime.core().metrics().packet_current.bytes, 0);
    assert_eq!(runtime.core().metrics().control_current.frames, 0);
    assert_eq!(runtime.core().metrics().control_current.bytes, 0);
    assert!(
        std::fs::read_dir(&runtime_dir)
            .expect("private runtime directory remains readable before drop")
            .next()
            .is_none(),
        "private socket files leaked"
    );
    drop(runtime);
    assert!(!runtime_dir.exists(), "private runtime directory leaked");
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "this integration test keeps the full bounded process-isolation evidence sequence together"
)]
fn real_bevy_workers_isolate_match_crash_and_cleanup_routes_peers() {
    let build = default_build_identity().expect("default build identity is available");
    let mut runtime = runtime();
    let mut events = runtime
        .spawn_worker(lobby_spec())
        .expect("real lobby worker spawns");
    events = wait_for_ready(
        &mut runtime,
        &[WorkerId::new(LOBBY_WORKER_ID).unwrap()],
        events,
    );
    assert_eq!(
        runtime.core().route_count(),
        1,
        "lobby route was not activated"
    );

    events.extend(
        runtime
            .submit_allocation_request(
                WorkerId::new(LOBBY_WORKER_ID).unwrap(),
                allocation_request(1, 0x10000, build),
            )
            .expect("first production allocation spawns"),
    );
    events.extend(
        runtime
            .submit_allocation_request(
                WorkerId::new(LOBBY_WORKER_ID).unwrap(),
                allocation_request(2, 0x20000, build),
            )
            .expect("second production allocation spawns"),
    );
    let matches = worker_ids(&events);
    assert_eq!(
        matches.len(),
        2,
        "two match workers were not spawned: {events:?}"
    );
    assert_ne!(
        matches[0], matches[1],
        "match workers share a stable identity"
    );
    events = wait_for_ready(&mut runtime, &matches, events);

    let spawned_pids = events
        .iter()
        .filter_map(|event| match event {
            LifecycleEvent::Spawned { worker_id, pid } => Some((*worker_id, *pid)),
            _ => None,
        })
        .collect::<HashMap<_, _>>();
    assert_eq!(
        spawned_pids.len(),
        3,
        "lobby and both matches have child PIDs"
    );
    let distinct_pids = spawned_pids.values().copied().collect::<HashSet<_>>();
    assert_eq!(
        distinct_pids.len(),
        3,
        "workers do not have distinct child PIDs"
    );
    assert!(spawned_pids.values().all(|pid| *pid > 0));
    assert_eq!(runtime.core().worker_count(), 3);
    assert_eq!(
        runtime.core().route_count(),
        5,
        "lobby plus two route/peer pairs"
    );
    assert_eq!(
        runtime.core().capability_count(),
        4,
        "two capabilities per match"
    );
    for worker_id in &matches {
        let routes = runtime.core().routes_for_worker(*worker_id);
        assert_eq!(
            routes.len(),
            2,
            "allocation did not install two peer routes"
        );
        assert!(routes.iter().all(|route| !route.is_default_lobby));
    }
    let first_routes = runtime.core().routes_for_worker(matches[0]);
    let second_routes = runtime.core().routes_for_worker(matches[1]);
    assert!(
        first_routes.iter().all(|first| second_routes
            .iter()
            .all(|second| first.peer_id != second.peer_id)),
        "match route peers were not isolated"
    );

    let crashed = matches[0];
    let survivor = matches[1];
    let survivor_process_id = runtime.worker_registration(survivor).unwrap().process_id;
    let kill_status = Command::new("kill")
        .args(["-KILL", &spawned_pids[&crashed].to_string()])
        .status()
        .expect("OS can terminate the selected production worker");
    assert!(
        kill_status.success(),
        "selected production worker was not killed"
    );

    let deadline = Instant::now() + Duration::from_secs(8);
    let mut saw_crash = false;
    while Instant::now() < deadline {
        let report = runtime
            .poll_once(Some(Duration::from_millis(10)))
            .expect("supervisor poll after worker crash succeeds");
        saw_crash |= report.lifecycle_events.iter().any(|event| {
            matches!(
                event,
                LifecycleEvent::Failed {
                    worker_id,
                    category: RoutingErrorCategory::WorkerCrash,
                } if *worker_id == crashed
            )
        });
        if saw_crash && runtime.core().worker_count() == 2 {
            break;
        }
    }
    assert!(
        saw_crash,
        "supervisor did not classify the killed Bevy worker as a crash"
    );
    assert_eq!(
        runtime.core().worker_count(),
        2,
        "survivor was torn down with sibling"
    );
    assert_eq!(runtime.process_worker_count(), 2);
    assert_eq!(
        runtime.core().route_count(),
        3,
        "crashed worker routes were not revoked"
    );
    assert_eq!(runtime.core().live_capability_count(), 2);
    assert_eq!(runtime.core().routes_for_worker(crashed).len(), 0);
    assert_eq!(runtime.core().routes_for_worker(survivor).len(), 2);
    assert_eq!(
        runtime.worker_registration(survivor).unwrap().kind,
        WorkerKind::Match
    );
    assert_eq!(
        runtime.worker_registration(survivor).unwrap().process_id,
        survivor_process_id
    );

    shutdown_and_assert_clean(runtime);
}

#[test]
fn real_bevy_lobby_restarts_after_crash_and_cleans_exactly() {
    let mut runtime = runtime();
    let events = runtime
        .spawn_worker(lobby_spec())
        .expect("real lobby worker spawns");
    let events = wait_for_ready(
        &mut runtime,
        &[WorkerId::new(LOBBY_WORKER_ID).unwrap()],
        events,
    );
    let worker_id = WorkerId::new(LOBBY_WORKER_ID).unwrap();
    let original_generation = runtime.worker_registration(worker_id).unwrap().generation;
    let original_pid = events
        .iter()
        .find_map(|event| match event {
            LifecycleEvent::Spawned { worker_id: id, pid } if *id == worker_id => Some(*pid),
            _ => None,
        })
        .expect("lobby spawn PID is reported");
    assert!(
        Command::new("kill")
            .args(["-KILL", &original_pid.to_string()])
            .status()
            .expect("OS can terminate the selected production lobby")
            .success()
    );

    let deadline = Instant::now() + Duration::from_secs(12);
    let mut saw_restart = false;
    let mut saw_restarted_ready = false;
    let mut restarted_pid = None;
    while Instant::now() < deadline {
        let report = runtime
            .poll_once(Some(Duration::from_millis(10)))
            .expect("supervisor poll during production restart succeeds");
        for event in &report.lifecycle_events {
            saw_restart |= matches!(
                event,
                LifecycleEvent::RestartScheduled { worker_id: id, .. } if *id == worker_id
            );
            if saw_restart {
                if let LifecycleEvent::Spawned { worker_id: id, pid } = event {
                    if *id == worker_id && *pid != original_pid {
                        restarted_pid = Some(*pid);
                    }
                }
                saw_restarted_ready |= matches!(
                    event,
                    LifecycleEvent::Ready { worker_id: id } if *id == worker_id
                );
            }
        }
        if saw_restarted_ready {
            break;
        }
    }
    assert!(saw_restart, "production lobby restart was not scheduled");
    assert!(
        saw_restarted_ready,
        "production lobby did not become Ready after restart"
    );
    assert_ne!(restarted_pid, Some(original_pid));
    assert_eq!(
        runtime
            .worker_registration(worker_id)
            .unwrap()
            .generation
            .get(),
        original_generation.get() + 1
    );
    assert_eq!(runtime.core().worker_count(), 1);
    assert_eq!(runtime.core().route_count(), 1);
    shutdown_and_assert_clean(runtime);
}
