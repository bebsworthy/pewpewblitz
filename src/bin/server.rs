//! Brawler's dedicated headless-server process.

use bevy::app::AppExit;
use brawler::config::ServerNetworkConfig;
use brawler::server::build_app_with_config;
use core::{net::SocketAddr, time::Duration};
use std::{env, process};

fn usage() {
    eprintln!(
        "usage: brawler-server [--bind <IP:PORT>] [--max-clients <N>] [--handshake-timeout-ms <N>]"
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
    let mut args = env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--bind" => config.bind_addr = parse_value::<SocketAddr>(&flag, args.next())?,
            "--max-clients" => config.max_clients = parse_value(&flag, args.next())?,
            "--handshake-timeout-ms" => {
                let millis: u64 = parse_value(&flag, args.next())?;
                config.handshake_timeout = Duration::from_millis(millis);
            }
            "--help" | "-h" => {
                usage();
                process::exit(0);
            }
            _ => return Err(format!("unknown flag: {flag}")),
        }
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
            process::exit(2);
        }
    };
    build_app_with_config(config).run()
}
