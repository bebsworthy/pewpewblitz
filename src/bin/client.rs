//! Brawler's macOS client process.

use bevy::app::AppExit;
use brawler::client::build_app_with_config;
use brawler::config::ClientNetworkConfig;
use core::net::SocketAddr;
use std::{env, process};

fn usage() {
    eprintln!(
        "usage: brawler-client --client-id <u64> [--server <IP:PORT>] [--headless --exit-after-roster <N> --move-axis <X,Y> --aim-axis <X,Y> --simulation-ticks <N>]"
    );
}

fn parse_value<T: core::str::FromStr>(flag: &str, value: Option<String>) -> Result<T, String> {
    value
        .ok_or_else(|| format!("{flag} requires a value"))?
        .parse()
        .map_err(|_| format!("invalid value for {flag}"))
}

fn parse_axis(flag: &str, value: Option<String>) -> Result<(i8, i8), String> {
    let value = value.ok_or_else(|| format!("{flag} requires a value"))?;
    let mut parts = value.split(',');
    let x = parts
        .next()
        .ok_or_else(|| format!("invalid value for {flag}; expected X,Y"))?
        .parse()
        .map_err(|_| format!("invalid value for {flag}; expected signed X,Y"))?;
    let y = parts
        .next()
        .ok_or_else(|| format!("invalid value for {flag}; expected X,Y"))?
        .parse()
        .map_err(|_| format!("invalid value for {flag}; expected signed X,Y"))?;
    if parts.next().is_some() {
        return Err(format!("invalid value for {flag}; expected X,Y"));
    }
    Ok((x, y))
}

fn parse_args() -> Result<ClientNetworkConfig, String> {
    let mut args = env::args().skip(1);
    let mut client_id = None;
    let mut server = None;
    let mut headless = false;
    let mut exit_after_roster = None;
    let mut headless_move = None;
    let mut headless_aim = None;
    let mut headless_simulation_ticks = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--client-id" => client_id = Some(parse_value(&flag, args.next())?),
            "--server" => server = Some(parse_value::<SocketAddr>(&flag, args.next())?),
            "--headless" => headless = true,
            "--exit-after-roster" => {
                exit_after_roster = Some(parse_value(&flag, args.next())?);
            }
            "--move-axis" => headless_move = Some(parse_axis(&flag, args.next())?),
            "--aim-axis" => headless_aim = Some(parse_axis(&flag, args.next())?),
            "--simulation-ticks" => {
                headless_simulation_ticks = Some(parse_value(&flag, args.next())?);
            }
            "--help" | "-h" => {
                usage();
                process::exit(0);
            }
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    let client_id = client_id.ok_or_else(|| "--client-id is required".to_string())?;
    if headless && exit_after_roster.is_none() {
        return Err("--headless requires --exit-after-roster".to_string());
    }
    if !headless && exit_after_roster.is_some() {
        return Err("--exit-after-roster requires --headless".to_string());
    }
    let mut config = ClientNetworkConfig::new(client_id);
    config.server_addr = server.unwrap_or(config.server_addr);
    config.headless = headless;
    config.exit_after_roster = exit_after_roster;
    config.headless_move = headless_move;
    config.headless_aim = headless_aim;
    config.headless_simulation_ticks = headless_simulation_ticks;
    config
        .validate()
        .map_err(|error| format!("invalid client configuration: {error}"))?;
    Ok(config)
}

fn main() -> AppExit {
    let config = match parse_args() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("brawler-client: {error}");
            usage();
            process::exit(2);
        }
    };
    build_app_with_config(config).run()
}
