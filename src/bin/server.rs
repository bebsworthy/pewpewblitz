//! Brawler's dedicated headless-server process.

use bevy::app::AppExit;
use brawler::config::{GameMode, MatchRulesProfile, ServerNetworkConfig};
use brawler::server::{WorkerBootstrap, build_app_with_config, parse_worker_arguments};
use core::{net::SocketAddr, time::Duration};
use std::{env, process};

fn usage() {
    eprintln!(
        "usage: brawler-server [--bind <IP:PORT>] [--max-clients <N>] [--handshake-timeout-ms <N>] [--mode <wipeout|hot-zone>] [--match-rules <production|verification>]"
    );
    eprintln!(
        "       brawler-server lobby-worker --role lobby --logical-server-id <U128> --supervisor-generation <U64> --worker-id <U128> --process-id <U128> --worker-generation <U64> --packet-socket <PATH> --control-socket <PATH>"
    );
    eprintln!(
        "       brawler-server match-worker --role match --logical-server-id <U128> --supervisor-generation <U64> --worker-id <U128> --process-id <U128> --worker-generation <U64> --packet-socket <PATH> --control-socket <PATH>"
    );
    eprintln!(
        "       brawler-server validate-closeout <DIRECTORY> <CLIENT-COUNT> <EXPECT-CHECKPOINTS> [WEAPON-PRESET]   validate finished closeout reports against the current report schema (EXPECT-CHECKPOINTS is 1 for combat-assert runs, 0 otherwise; WEAPON-PRESET re-derives the declared checkpoint requirement from the asserted preset)"
    );
    eprintln!(
        "       brawler-server required-checkpoint-count <WEAPON-PRESET>   print the asserted preset's required process-checkpoint count"
    );
    eprintln!(
        "       brawler-server routing-identity   print routed network and content identity as key=value lines"
    );
    eprintln!(
        "note: --wipeout-rules <production|verification> is a deprecated alias for --match-rules"
    );
}

fn parse_value<T: core::str::FromStr>(flag: &str, value: Option<String>) -> Result<T, String> {
    value
        .ok_or_else(|| format!("{flag} requires a value"))?
        .parse()
        .map_err(|_| format!("invalid value for {flag}"))
}

fn parse_args() -> Result<ServerNetworkConfig, String> {
    let mut config = ServerNetworkConfig::default();
    let mut saw_match_rules = false;
    let mut saw_legacy_rules = false;
    let mut args = env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--bind" => config.bind_addr = parse_value::<SocketAddr>(&flag, args.next())?,
            "--max-clients" => config.max_clients = parse_value(&flag, args.next())?,
            "--handshake-timeout-ms" => {
                let millis: u64 = parse_value(&flag, args.next())?;
                config.handshake_timeout = Duration::from_millis(millis);
            }
            "--mode" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                config.game_mode =
                    GameMode::parse(&value).ok_or_else(|| format!("invalid value for {flag}"))?;
            }
            "--match-rules" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                config.match_rules_profile = MatchRulesProfile::parse(&value)
                    .ok_or_else(|| format!("invalid value for {flag}"))?;
                saw_match_rules = true;
            }
            "--wipeout-rules" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                config.match_rules_profile = MatchRulesProfile::parse(&value)
                    .ok_or_else(|| format!("invalid value for {flag}"))?;
                saw_legacy_rules = true;
            }
            "--help" | "-h" => {
                usage();
                process::exit(0);
            }
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    if saw_match_rules && saw_legacy_rules {
        return Err(
            "--wipeout-rules conflicts with --match-rules; use --match-rules only".to_string(),
        );
    }
    config
        .validate()
        .map_err(|error| format!("invalid server configuration: {error}"))?;
    Ok(config)
}

/// Headless closeout-report gate for verification launchers: enforces the same report
/// reader the binaries' report writer uses, so the terminal check cannot drift from the
/// writer's contract. Exits 0 when every configured endpoint validated.
fn run_closeout_validation(
    directory: &str,
    client_count: &str,
    expect_checkpoints: &str,
    weapon_preset: Option<&String>,
) -> ! {
    let Ok(client_count) = client_count.parse::<u32>() else {
        eprintln!("brawler-server: validate-closeout requires a numeric client count");
        process::exit(2);
    };
    let expect_checkpoint_evidence = match expect_checkpoints {
        "0" => false,
        "1" => true,
        _ => {
            eprintln!("brawler-server: validate-closeout requires EXPECT-CHECKPOINTS of 0 or 1");
            process::exit(2);
        }
    };
    // With the asserted weapon preset, the gate re-derives the required checkpoint set
    // from the same mapping the combat assertion uses, so the launcher's declared
    // scenario counts are checked against the preset instead of trusting the launcher.
    let declared_checkpoint_requirement = weapon_preset.map(|preset| {
        let Ok(preset_id) = preset.parse::<u16>() else {
            eprintln!("brawler-server: validate-closeout WEAPON-PRESET must be numeric");
            process::exit(2);
        };
        let required = brawler::server::required_process_checkpoints(
            brawler::combat::WeaponPresetId(preset_id),
        );
        u32::try_from(required.len()).unwrap_or(u32::MAX)
    });
    match brawler::diagnostics::validate_closeout_directory(
        std::path::Path::new(directory),
        client_count,
        expect_checkpoint_evidence,
        declared_checkpoint_requirement,
    ) {
        Ok(count) => {
            println!("brawler-server: validated {count} closeout reports in {directory}");
            process::exit(0);
        }
        Err(error) => {
            eprintln!("brawler-server: closeout validation failed: {error}");
            process::exit(2);
        }
    }
}

fn run_required_checkpoint_count(weapon_preset: &str) -> ! {
    let Ok(preset_id) = weapon_preset.parse::<u16>() else {
        eprintln!("brawler-server: required-checkpoint-count WEAPON-PRESET must be numeric");
        process::exit(2);
    };
    let required =
        brawler::server::required_process_checkpoints(brawler::combat::WeaponPresetId(preset_id));
    println!("{}", required.len());
    process::exit(0);
}

fn run_routing_identity() -> ! {
    let identity = match brawler::server::routing_identity() {
        Ok(identity) => identity,
        Err(error) => {
            eprintln!("brawler-server: cannot compute routing identity: {error:?}");
            process::exit(2);
        }
    };
    println!("network_protocol={}", identity.network_protocol);
    println!(
        "protocol_registry_fingerprint={}",
        identity.protocol_registry_fingerprint
    );
    println!("content_fingerprint={}", identity.content_fingerprint);
    process::exit(0);
}

fn main() -> AppExit {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    let explicit_worker_role = match raw_args.first().map(String::as_str) {
        Some("lobby-worker") => Some("lobby"),
        Some("match-worker") => Some("match"),
        _ => None,
    };
    let argv_worker = raw_args.first().is_some_and(|arg| arg == "--role");
    if let Some(expected_role) = explicit_worker_role.or(argv_worker.then_some("")) {
        let worker_args = if explicit_worker_role.is_some() {
            raw_args.iter().skip(1).cloned().collect::<Vec<_>>()
        } else {
            raw_args.clone()
        };
        let parsed = match parse_worker_arguments(worker_args) {
            Ok(args) => args,
            Err(error) => {
                eprintln!("brawler-server: {error}");
                usage();
                process::exit(2);
            }
        };
        if !expected_role.is_empty()
            && ((expected_role == "lobby"
                && parsed.role != brawler::server::WorkerEntrypointRole::Lobby)
                || (expected_role == "match"
                    && parsed.role != brawler::server::WorkerEntrypointRole::Match))
        {
            eprintln!("brawler-server: worker mode and --role disagree");
            process::exit(2);
        }
        let mut app = match WorkerBootstrap::connect(parsed).and_then(WorkerBootstrap::start) {
            Ok(app) => app,
            Err(error) => {
                eprintln!("brawler-server: {error}");
                process::exit(2);
            }
        };
        return app.run();
    }
    if raw_args
        .first()
        .is_some_and(|arg| arg == "required-checkpoint-count")
    {
        if raw_args.len() != 2 {
            eprintln!("brawler-server: required-checkpoint-count requires <WEAPON-PRESET>");
            usage();
            process::exit(2);
        }
        run_required_checkpoint_count(&raw_args[1]);
    }
    if raw_args
        .first()
        .is_some_and(|arg| arg == "routing-identity")
    {
        if raw_args.len() != 1 {
            eprintln!("brawler-server: routing-identity takes no arguments");
            usage();
            process::exit(2);
        }
        run_routing_identity();
    }
    if raw_args
        .first()
        .is_some_and(|arg| arg == "validate-closeout")
    {
        if raw_args.len() != 4 && raw_args.len() != 5 {
            eprintln!(
                "brawler-server: validate-closeout requires <DIRECTORY> <CLIENT-COUNT> <EXPECT-CHECKPOINTS> [WEAPON-PRESET]"
            );
            usage();
            process::exit(2);
        }
        run_closeout_validation(&raw_args[1], &raw_args[2], &raw_args[3], raw_args.get(4));
    }
    let config = match parse_args() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("brawler-server: {error}");
            usage();
            if let Some(path) = env::var_os("BRAWLER_FAILURE_REPORT") {
                brawler::diagnostics::write_failure_record(
                    std::path::Path::new(&path),
                    &brawler::diagnostics::ProcessFailureRecordV1::new(
                        brawler::diagnostics::FailureCategory::Configuration,
                        format!("configuration rejected: {error}"),
                    ),
                );
            }
            process::exit(2);
        }
    };
    build_app_with_config(config).run()
}
