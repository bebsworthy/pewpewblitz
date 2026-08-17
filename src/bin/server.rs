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

fn main() -> AppExit {
    let config = match parse_args() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("brawler-server: {error}");
            usage();
            if let Some(path) = env::var_os("BRAWLER_FAILURE_REPORT") {
                brawler::diagnostics::write_failure_record(
                    std::path::Path::new(&path),
                    &brawler::diagnostics::ProcessFailureRecordV1::new(
                        brawler::diagnostics::FailureCategory::VerificationFailed,
                        format!("configuration rejected: {error}"),
                    ),
                );
            }
            process::exit(2);
        }
    };
    build_app_with_config(config).run()
}
