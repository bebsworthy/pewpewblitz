use std::{
    path::PathBuf,
    thread,
    time::{Duration, Instant},
};

use brawler_routing::{
    LifecycleEvent, LobbyManifest, ManifestBody, ManifestCommon, ProcessSupervisor,
    ProcessSupervisorConfig, RoutingErrorCategory, StopId, WorkerKind, WorkerLaunchSpec,
    WorkerRegistration, WorkerRole,
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

fn new_supervisor() -> ProcessSupervisor {
    ProcessSupervisor::new(ProcessSupervisorConfig::new(id128(1), id64(2), 3, 4)).unwrap()
}

fn launch_spec(worker_id: u128) -> WorkerLaunchSpec {
    let registration = WorkerRegistration {
        worker_id: id128(worker_id),
        process_id: id128(worker_id + 10),
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
            protocol_registry_fingerprint: 5,
            content_fingerprint: 4,
            route_version: 1,
            packet_version: 1,
            control_version: 1,
            flags: 0,
        },
        default_route_id: id128(100),
        max_authenticated_sessions: 32,
        outstanding_allocations: 2,
        active_matches: 4,
        heartbeat_ms: 1_000,
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

fn launch_spec_mode(worker_id: u128, mode: &str) -> WorkerLaunchSpec {
    launch_spec(worker_id).with_environment("BRAWLER_FAKE_WORKER_MODE", mode)
}

fn collect_until<F>(
    supervisor: &mut ProcessSupervisor,
    timeout: Duration,
    mut predicate: F,
) -> Vec<LifecycleEvent>
where
    F: FnMut(&LifecycleEvent) -> bool,
{
    let started = Instant::now();
    let mut events = Vec::new();
    while started.elapsed() < timeout {
        let polled = supervisor.poll().unwrap();
        let matched = polled.iter().any(&mut predicate);
        events.extend(polled);
        if matched {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    events
}

#[test]
fn child_manifest_ready_stop_reap_and_socket_cleanup_are_bounded() {
    let mut supervisor = new_supervisor();
    let runtime = supervisor.runtime_dir().unwrap().to_path_buf();
    let spec = launch_spec(7);
    let worker_id = spec.registration.worker_id;
    supervisor.spawn(spec).unwrap();
    assert!(runtime.exists());

    let start = Instant::now();
    let mut ready = false;
    while start.elapsed() < Duration::from_secs(2) {
        for event in supervisor.poll().unwrap() {
            if matches!(event, LifecycleEvent::Ready { .. }) {
                ready = true;
            }
        }
        if ready {
            break;
        }
        thread::sleep(Duration::from_millis(10));
    }
    assert!(ready, "worker did not acknowledge the exact Manifest");
    assert!(
        supervisor
            .stop_worker(worker_id, StopId::new(99).unwrap(), 1)
            .unwrap()
    );
    assert!(
        !supervisor
            .stop_worker(worker_id, StopId::new(99).unwrap(), 1)
            .unwrap()
    );

    let stop_start = Instant::now();
    while supervisor.worker_count() != 0 && stop_start.elapsed() < Duration::from_secs(3) {
        let _ = supervisor.poll().unwrap();
        thread::sleep(Duration::from_millis(10));
    }
    assert_eq!(supervisor.worker_count(), 0);
    drop(supervisor);
    assert!(!runtime.exists());
}

#[test]
fn manifest_identity_is_checked_before_spawn() {
    let mut supervisor = new_supervisor();
    let mut spec = launch_spec(8);
    let mut malformed = match spec.manifest.role {
        WorkerRole::Lobby => LobbyManifest::decode(&spec.manifest.manifest).unwrap(),
        WorkerRole::Match => unreachable!(),
    };
    malformed.common.content_fingerprint = 999;
    malformed.digest = [0; 32];
    spec.manifest = ManifestBody::from_lobby(&malformed).unwrap();
    assert!(matches!(
        supervisor.spawn(spec),
        Err(brawler_routing::LifecycleError::Routing(
            RoutingErrorCategory::ManifestIncompatible
        ))
    ));
}

#[test]
fn malformed_control_fails_and_reclaims_child_without_lobby_restart() {
    let mut supervisor =
        ProcessSupervisor::new(ProcessSupervisorConfig::new(id128(1), id64(2), 3, 4)).unwrap();
    let runtime = supervisor.runtime_dir().unwrap().to_path_buf();
    let spec = launch_spec_mode(9, "malformed");
    supervisor.spawn(spec).unwrap();
    let events = collect_until(&mut supervisor, Duration::from_secs(2), |event| {
        matches!(event, LifecycleEvent::Failed { .. })
    });
    assert!(events.iter().any(|event| {
        matches!(
            event,
            LifecycleEvent::Failed {
                category: RoutingErrorCategory::IpcMalformed,
                ..
            }
        )
    }));
    let _ = supervisor.begin_shutdown();
    drop(supervisor);
    assert!(!runtime.exists());
}

#[test]
fn crash_and_missing_exit_are_reconciled_against_child_status() {
    let mut supervisor = new_supervisor();
    let crash = launch_spec_mode(10, "crash");
    supervisor.spawn(crash).unwrap();
    let crash_events = collect_until(&mut supervisor, Duration::from_secs(2), |event| {
        matches!(event, LifecycleEvent::Failed { .. })
    });
    assert!(crash_events.iter().any(|event| {
        matches!(
            event,
            LifecycleEvent::Failed {
                category: RoutingErrorCategory::WorkerCrash,
                ..
            }
        )
    }));
    assert!(
        crash_events
            .iter()
            .any(|event| matches!(event, LifecycleEvent::RestartScheduled { .. }))
    );
    let restarted = collect_until(&mut supervisor, Duration::from_secs(2), |event| {
        matches!(event, LifecycleEvent::Spawned { .. })
    });
    assert!(
        restarted
            .iter()
            .any(|event| matches!(event, LifecycleEvent::Spawned { .. }))
    );
    assert_eq!(
        supervisor
            .worker_registration(id128(10))
            .expect("restarted lobby registration")
            .generation
            .get(),
        2
    );
    let _ = supervisor.begin_shutdown();
    drop(supervisor);

    let mut supervisor = new_supervisor();
    let missing_exit = launch_spec_mode(11, "no-exit");
    let worker_id = missing_exit.registration.worker_id;
    supervisor.spawn(missing_exit).unwrap();
    let ready = collect_until(&mut supervisor, Duration::from_secs(2), |event| {
        matches!(event, LifecycleEvent::Ready { .. })
    });
    assert!(
        ready
            .iter()
            .any(|event| matches!(event, LifecycleEvent::Ready { .. }))
    );
    supervisor
        .stop_worker(worker_id, StopId::new(101).unwrap(), 1)
        .unwrap();
    let events = collect_until(&mut supervisor, Duration::from_secs(2), |event| {
        matches!(
            event,
            LifecycleEvent::Failed {
                category: RoutingErrorCategory::WorkerExitMismatch,
                ..
            }
        )
    });
    assert!(events.iter().any(|event| {
        matches!(
            event,
            LifecycleEvent::Failed {
                category: RoutingErrorCategory::WorkerExitMismatch,
                ..
            }
        )
    }));
    let _ = supervisor.begin_shutdown();
}

#[test]
fn ready_timeout_and_forced_stop_are_bounded() {
    let mut config = ProcessSupervisorConfig::new(id128(1), id64(2), 3, 4);
    config.ready_timeout = Duration::from_millis(100);
    let mut timeout_supervisor = ProcessSupervisor::new(config).unwrap();
    let runtime = timeout_supervisor.runtime_dir().unwrap().to_path_buf();
    timeout_supervisor
        .spawn(launch_spec_mode(12, "hang"))
        .unwrap();
    let timeout_events = collect_until(&mut timeout_supervisor, Duration::from_secs(2), |event| {
        matches!(
            event,
            LifecycleEvent::Failed {
                category: RoutingErrorCategory::WorkerReadyTimeout,
                ..
            }
        )
    });
    assert!(timeout_events.iter().any(|event| {
        matches!(
            event,
            LifecycleEvent::Failed {
                category: RoutingErrorCategory::WorkerReadyTimeout,
                ..
            }
        )
    }));
    let _ = timeout_supervisor.begin_shutdown();
    drop(timeout_supervisor);
    assert!(!runtime.exists());

    let mut forced_supervisor = new_supervisor();
    let worker_id = launch_spec_mode(13, "forced").registration.worker_id;
    forced_supervisor
        .spawn(launch_spec_mode(13, "forced"))
        .unwrap();
    let ready = collect_until(&mut forced_supervisor, Duration::from_secs(2), |event| {
        matches!(event, LifecycleEvent::Ready { .. })
    });
    assert!(
        ready
            .iter()
            .any(|event| matches!(event, LifecycleEvent::Ready { .. }))
    );
    forced_supervisor
        .stop_worker(worker_id, StopId::new(102).unwrap(), 1)
        .unwrap();
    let forced = collect_until(&mut forced_supervisor, Duration::from_secs(4), |event| {
        matches!(event, LifecycleEvent::Stopped { forced: true, .. })
    });
    assert!(
        forced
            .iter()
            .any(|event| { matches!(event, LifecycleEvent::ForcedStop { .. }) })
    );
    assert!(
        forced
            .iter()
            .any(|event| { matches!(event, LifecycleEvent::Stopped { forced: true, .. }) })
    );
    assert_eq!(forced_supervisor.worker_count(), 0);
}
