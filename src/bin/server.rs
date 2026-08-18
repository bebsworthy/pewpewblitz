//! Brawler's dedicated headless-server process.

use bevy::app::AppExit;
use brawler::config::{GameMode, MatchRulesProfile, ServerNetworkConfig};
use brawler::server::build_app_with_config;
use core::{net::SocketAddr, time::Duration};
use std::{env, process};

fn usage() {
    eprintln!(
        "usage: brawler-server [--bind <IP:PORT>] [--max-clients <N>] [--handshake-timeout-ms <N>] [--mode <wipeout|hot-zone>] [--match-rules <production|verification>]"
    );
    eprintln!(
        "       brawler-server validate-closeout <DIRECTORY> <CLIENT-COUNT> <EXPECT-CHECKPOINTS>   validate finished closeout reports against schema v1 (EXPECT-CHECKPOINTS is 1 for combat-assert runs, 0 otherwise)"
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

/// Headless closeout-report gate for verification launchers: enforces the same schema-v1
/// reader the binaries' report writer uses, so the terminal check cannot drift from the
/// writer's contract. Exits 0 when every configured endpoint validated.
fn run_closeout_validation(directory: &str, client_count: &str, expect_checkpoints: &str) -> ! {
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
    match brawler::diagnostics::validate_closeout_directory(
        std::path::Path::new(directory),
        client_count,
        expect_checkpoint_evidence,
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

fn main() -> AppExit {
    let raw_args: Vec<String> = env::args().skip(1).collect();
    if raw_args
        .first()
        .is_some_and(|arg| arg == "validate-closeout")
    {
        if raw_args.len() != 4 {
            eprintln!(
                "brawler-server: validate-closeout requires <DIRECTORY> <CLIENT-COUNT> <EXPECT-CHECKPOINTS>"
            );
            usage();
            process::exit(2);
        }
        run_closeout_validation(&raw_args[1], &raw_args[2], &raw_args[3]);
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
