//! Minimal production supervisor entry point.
//!
//! This binary owns one public UDP endpoint, a Mio owner loop, and one validated lobby-worker
//! bootstrap through the control-plane manifest contract.

use std::{error::Error, fs, io::Read as _, net::SocketAddr, path::PathBuf};

use brawler_routing::{
    AllocationPolicy, CONTROL_VERSION_V1, CoreConfig, GameMode, Generation, LobbyManifest,
    LogicalServerId, MAX_LOBBY_CATALOG_BYTES, ManifestBody, ManifestCommon, PACKET_VERSION_V1,
    ProcessId, ProcessSupervisorConfig, ROUTE_VERSION_V1, RuntimeConfig, SupervisorRuntime,
    WorkerKind, WorkerLaunchSpec, WorkerRegistration, WorkerRole, raw_catalog_fingerprint,
};

fn main() -> Result<(), Box<dyn Error>> {
    let args = parse_args(std::env::args().skip(1))?;
    let network_protocol = args
        .network_protocol
        .ok_or("--network-protocol is required")?;
    let content_fingerprint = args
        .content_fingerprint
        .ok_or("--content-fingerprint is required")?;
    let protocol_registry_fingerprint = args
        .protocol_registry_fingerprint
        .ok_or("--protocol-registry-fingerprint is required")?;
    let worker_executable = args
        .worker_executable
        .ok_or("--worker-executable is required")?;
    let catalog_path = args.game_types.ok_or("--game-types is required")?;
    let raw_catalog = read_catalog_file(&catalog_path)?;
    let logical_server_id = LogicalServerId::new(random_u128()?).ok_or("zero logical server ID")?;
    let generation = Generation::new(random_u64()?).ok_or("zero supervisor generation")?;
    let mut runtime = SupervisorRuntime::new(RuntimeConfig {
        public_bind: args.bind,
        core: CoreConfig::with_identity(
            logical_server_id,
            generation,
            network_protocol,
            content_fingerprint,
        ),
        process_supervisor: Some(ProcessSupervisorConfig::new(
            logical_server_id,
            generation,
            network_protocol,
            content_fingerprint,
        )),
        allocation_policy: Some(AllocationPolicy::brawler_m01_with_rules_profile(
            args.rules_profile,
        )),
        worker_executable: Some(worker_executable.clone()),
        protocol_registry_fingerprint: Some(protocol_registry_fingerprint),
        ..RuntimeConfig::default()
    })?;
    let worker_id = brawler_routing::WorkerId::new(random_u128()?).ok_or("zero worker ID")?;
    let process_id = ProcessId::new(random_u128()?).ok_or("zero process ID")?;
    let worker_generation = Generation::new(1).expect("constant generation is nonzero");
    let default_route_id =
        brawler_routing::RouteId::new(random_u128()?).ok_or("zero lobby route ID")?;
    let manifest = LobbyManifest {
        common: ManifestCommon {
            manifest_version: 1,
            role: WorkerRole::Lobby,
            logical_server_id,
            process_id,
            worker_id,
            generation: worker_generation,
            network_protocol,
            protocol_registry_fingerprint,
            content_fingerprint,
            route_version: ROUTE_VERSION_V1,
            packet_version: PACKET_VERSION_V1,
            control_version: CONTROL_VERSION_V1,
            flags: 0,
        },
        default_route_id,
        max_authenticated_sessions: 32,
        outstanding_allocations: 2,
        active_matches: 4,
        heartbeat_ms: 1_000,
        raw_catalog_fingerprint: raw_catalog_fingerprint(&raw_catalog),
        raw_catalog,
        nonce: random_u128()?,
        digest: [0; 32],
    };
    let mut lobby_spec = WorkerLaunchSpec::new(
        worker_executable,
        WorkerRegistration {
            worker_id,
            process_id,
            generation: worker_generation,
            kind: WorkerKind::Lobby,
        },
        ManifestBody::from_lobby(&manifest)?,
    );
    if args.automatic_transition_driver {
        let transition_mode = match args.mode {
            GameMode::Wipeout => "wipeout",
            GameMode::HotZone => "hot-zone",
        };
        lobby_spec = lobby_spec
            .with_environment("BRAWLER_LOBBY_TRANSITION_DRIVER", "1")
            .with_environment("BRAWLER_LOBBY_TRANSITION_MODE", transition_mode);
    }
    runtime.spawn_worker(lobby_spec)?;
    let stop = runtime.stop_handle();
    ctrlc::set_handler(move || {
        let _ = stop.request();
    })?;
    eprintln!("brawler-supervisor listening on {}", runtime.public_addr()?);
    runtime.run()?;
    if let Some(path) = args.metrics_file {
        write_metrics(&runtime, &path)?;
    }
    Ok(())
}

fn read_catalog_file(path: &PathBuf) -> Result<Vec<u8>, Box<dyn Error>> {
    let file = fs::File::open(path)?;
    let mut bytes = Vec::new();
    file.take(u64::try_from(MAX_LOBBY_CATALOG_BYTES + 1)?)
        .read_to_end(&mut bytes)?;
    if bytes.is_empty() || bytes.len() > MAX_LOBBY_CATALOG_BYTES {
        return Err("game-type catalog must contain 1..=16384 bytes".into());
    }
    Ok(bytes)
}

/// Write a deliberately small, stable, machine-readable snapshot for local evidence runs.
///
/// This is emitted only after the bounded runtime shutdown has completed. It reports route,
/// queue, drop, lifecycle, directional-byte, and owner-boundary latency counters owned by the
/// supervisor; it does not infer CPU, paired bandwidth, or packet-capture results that this
/// process does not measure.
fn write_metrics(runtime: &SupervisorRuntime, path: &PathBuf) -> Result<(), Box<dyn Error>> {
    let metrics = runtime.core().metrics();
    let routing = runtime.metrics();
    let errors = metrics
        .error_counts
        .iter()
        .map(|(category, count)| format!("\"{category:?}\":{count}"))
        .collect::<Vec<_>>()
        .join(",");
    let runtime_dir_entries = runtime
        .runtime_dir()
        .map(fs::read_dir)
        .transpose()?
        .map_or(0, Iterator::count);
    let json = format!(
        concat!(
            "{{",
            "\"schema\":\"brawler-routed-metrics-v1\",",
            "\"workers\":{},",
            "\"routes\":{},",
            "\"capabilities\":{},",
            "\"live_capabilities\":{},",
            "\"process_workers\":{},",
            "\"packet_current_frames\":{},",
            "\"packet_current_bytes\":{},",
            "\"packet_high_water_frames\":{},",
            "\"packet_high_water_bytes\":{},",
            "\"control_current_frames\":{},",
            "\"control_current_bytes\":{},",
            "\"control_high_water_frames\":{},",
            "\"control_high_water_bytes\":{},",
            "\"packet_dropped_newest\":{},",
            "\"control_rejected\":{},",
            "\"source_limited\":{},",
            "\"capabilities_activated\":{},",
            "\"capability_rebinds\":{},",
            "\"capabilities_revoked\":{},",
            "\"workers_cleaned\":{},",
            "\"routes_cleaned\":{},",
            "\"runtime_dir_entries\":{},",
            "\"traffic\":{{",
            "\"public_ingress\":{},",
            "\"public_egress\":{},",
            "\"inner_ingress\":{},",
            "\"inner_egress\":{},",
            "\"match_inner_ingress\":{},",
            "\"match_inner_egress\":{},",
            "\"ipc_to_worker\":{},",
            "\"ipc_from_worker\":{}",
            "}},",
            "\"latency\":{{",
            "\"public_receive_to_packet_ipc_enqueue\":{},",
            "\"worker_packet_to_public_send\":{}",
            "}},",
            "\"errors\":{{{}}}",
            "}}\n"
        ),
        runtime.core().worker_count(),
        runtime.core().route_count(),
        runtime.core().capability_count(),
        runtime.core().live_capability_count(),
        runtime.process_worker_count(),
        metrics.packet_current.frames,
        metrics.packet_current.bytes,
        metrics.packet_high_water.frames,
        metrics.packet_high_water.bytes,
        metrics.control_current.frames,
        metrics.control_current.bytes,
        metrics.control_high_water.frames,
        metrics.control_high_water.bytes,
        metrics.packet_dropped_newest,
        metrics.control_rejected,
        metrics.source_limited,
        metrics.capabilities_activated,
        metrics.capability_rebinds,
        metrics.capabilities_revoked,
        metrics.workers_cleaned,
        metrics.routes_cleaned,
        runtime_dir_entries,
        traffic_json(routing.public_ingress),
        traffic_json(routing.public_egress),
        traffic_json(routing.inner_ingress),
        traffic_json(routing.inner_egress),
        traffic_json(routing.match_inner_ingress),
        traffic_json(routing.match_inner_egress),
        traffic_json(routing.ipc_to_worker),
        traffic_json(routing.ipc_from_worker),
        latency_json(&routing.public_receive_to_packet_ipc_enqueue),
        latency_json(&routing.worker_packet_to_public_send),
        errors,
    );
    if let Some(parent) = path
        .parent()
        .filter(|parent| !parent.as_os_str().is_empty())
    {
        fs::create_dir_all(parent)?;
    }
    fs::write(path, json)?;
    Ok(())
}

fn traffic_json(counters: brawler_routing::TrafficCounters) -> String {
    format!(
        "{{\"datagrams\":{},\"frames\":{},\"bytes\":{}}}",
        counters.datagrams, counters.frames, counters.bytes
    )
}

fn latency_json(histogram: &brawler_routing::LatencyHistogram) -> String {
    let buckets = histogram
        .bucket_counts()
        .iter()
        .map(u64::to_string)
        .collect::<Vec<_>>()
        .join(",");
    format!(
        concat!(
            "{{\"samples\":{},\"sum_nanos\":{},",
            "\"min_nanos\":{},\"max_nanos\":{},",
            "\"p50_us\":{},\"p95_us\":{},\"p99_us\":{},",
            "\"bucket_upper_us\":[1,2,4,8,16,32,64,128,256,512,1024,2048,4096,8192,16384,32768,",
            "65536,131072,262144,524288,1048576,2097152,4194304,8388608,16777216,33554432,",
            "67108864,134217728,268435456,536870912,1073741824,2147483648,18446744073709551615],",
            "\"bucket_counts\":[{}]}}"
        ),
        histogram.count(),
        histogram.sum_nanos(),
        histogram.min_nanos().unwrap_or(0),
        histogram.max_nanos().unwrap_or(0),
        histogram
            .p50_us()
            .map_or_else(|| "null".to_string(), |value| value.to_string()),
        histogram
            .p95_us()
            .map_or_else(|| "null".to_string(), |value| value.to_string()),
        histogram
            .p99_us()
            .map_or_else(|| "null".to_string(), |value| value.to_string()),
        buckets,
    )
}

fn random_u128() -> Result<u128, Box<dyn Error>> {
    let mut bytes = [0_u8; 16];
    getrandom::fill(&mut bytes).map_err(|error| format!("OS entropy unavailable: {error}"))?;
    Ok(u128::from_be_bytes(bytes).max(1))
}

fn random_u64() -> Result<u64, Box<dyn Error>> {
    let mut bytes = [0_u8; 8];
    getrandom::fill(&mut bytes).map_err(|error| format!("OS entropy unavailable: {error}"))?;
    Ok(u64::from_be_bytes(bytes).max(1))
}

#[derive(Debug)]
struct Arguments {
    bind: SocketAddr,
    network_protocol: Option<u64>,
    protocol_registry_fingerprint: Option<u64>,
    content_fingerprint: Option<u64>,
    worker_executable: Option<PathBuf>,
    metrics_file: Option<PathBuf>,
    game_types: Option<PathBuf>,
    rules_profile: u8,
    mode: GameMode,
    automatic_transition_driver: bool,
}

fn parse_args(mut args: impl Iterator<Item = String>) -> Result<Arguments, Box<dyn Error>> {
    let mut parsed = Arguments {
        bind: SocketAddr::from(([127, 0, 0, 1], 5000)),
        network_protocol: None,
        protocol_registry_fingerprint: None,
        content_fingerprint: None,
        worker_executable: None,
        metrics_file: None,
        game_types: None,
        rules_profile: 1,
        mode: GameMode::Wipeout,
        automatic_transition_driver: false,
    };
    while let Some(argument) = args.next() {
        if argument == "--bind" {
            let value = args.next().ok_or("--bind requires an address")?;
            parsed.bind = value.parse()?;
        } else if argument == "--network-protocol" {
            parsed.network_protocol = Some(
                args.next()
                    .ok_or("--network-protocol requires a value")?
                    .parse()?,
            );
        } else if argument == "--content-fingerprint" {
            parsed.content_fingerprint = Some(
                args.next()
                    .ok_or("--content-fingerprint requires a value")?
                    .parse()?,
            );
        } else if argument == "--protocol-registry-fingerprint" {
            parsed.protocol_registry_fingerprint = Some(
                args.next()
                    .ok_or("--protocol-registry-fingerprint requires a value")?
                    .parse()?,
            );
        } else if argument == "--worker-executable" {
            parsed.worker_executable = Some(PathBuf::from(
                args.next().ok_or("--worker-executable requires a path")?,
            ));
        } else if argument == "--metrics-file" {
            parsed.metrics_file = Some(PathBuf::from(
                args.next().ok_or("--metrics-file requires a path")?,
            ));
        } else if argument == "--game-types" {
            parsed.game_types = Some(PathBuf::from(
                args.next().ok_or("--game-types requires a path")?,
            ));
        } else if argument == "--match-rules" {
            parsed.rules_profile = match args
                .next()
                .ok_or("--match-rules requires production or verification")?
                .as_str()
            {
                "production" => 1,
                "verification" => 2,
                _ => return Err("--match-rules requires production or verification".into()),
            };
        } else if argument == "--mode" {
            parsed.mode = match args
                .next()
                .ok_or("--mode requires wipeout or hot-zone")?
                .as_str()
            {
                "wipeout" => GameMode::Wipeout,
                "hot-zone" => GameMode::HotZone,
                _ => return Err("--mode requires wipeout or hot-zone".into()),
            };
        } else if argument == "--automatic-transition-driver" {
            parsed.automatic_transition_driver = true;
        } else if argument == "--help" || argument == "-h" {
            println!(
                "Usage: brawler-supervisor --network-protocol N --protocol-registry-fingerprint N --content-fingerprint N --worker-executable PATH --game-types PATH [--automatic-transition-driver] [--bind IP:PORT] [--mode <wipeout|hot-zone>] [--match-rules <production|verification>] [--metrics-file PATH]"
            );
            std::process::exit(0);
        } else {
            return Err(format!("unknown argument: {argument}").into());
        }
    }
    Ok(parsed)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parser_keeps_protocol_registry_fingerprint_explicit() {
        let args = parse_args(
            [
                "--network-protocol",
                "3",
                "--protocol-registry-fingerprint",
                "9",
                "--content-fingerprint",
                "4",
                "--worker-executable",
                "/tmp/worker",
            ]
            .into_iter()
            .map(String::from),
        )
        .unwrap();
        assert_eq!(args.protocol_registry_fingerprint, Some(9));
    }

    #[test]
    fn parser_rejects_missing_fingerprint_value() {
        let error = parse_args(
            ["--protocol-registry-fingerprint"]
                .into_iter()
                .map(String::from),
        )
        .unwrap_err();
        assert!(
            error
                .to_string()
                .contains("--protocol-registry-fingerprint requires a value")
        );
    }

    #[test]
    fn parser_accepts_optional_metrics_file() {
        let args = parse_args(
            ["--metrics-file", "/tmp/routed-metrics.json"]
                .into_iter()
                .map(String::from),
        )
        .unwrap();
        assert_eq!(
            args.metrics_file,
            Some(PathBuf::from("/tmp/routed-metrics.json"))
        );
    }
}
