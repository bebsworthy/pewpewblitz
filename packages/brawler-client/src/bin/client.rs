//! Brawler's macOS client process.
//!
//! The player build (`--features client`) accepts only the product CLI contract. Smoke, headless,
//! demo, capture, and render-report flags exist only with the `automation` feature.

#[cfg(feature = "automation")]
#[path = "client/automation.rs"]
mod automation;

use bevy::app::AppExit;
#[cfg(not(feature = "automation"))]
use brawler::config::ClientNetworkConfig;
use brawler::config::NetworkTransport;
#[cfg(feature = "automation")]
use brawler::config::random_nonzero_client_id;
use brawler_client::build_app_with_config;
#[cfg(feature = "automation")]
use core::net::SocketAddr;
use std::{env, process};

#[cfg(not(feature = "automation"))]
const USAGE: &str = "usage: brawler-client [--server <HOST[:PORT]>] [--transport routed-udp] [--window-size <WIDTHxHEIGHT>]";
#[cfg(feature = "automation")]
use automation::USAGE;

fn usage() {
    eprintln!("{USAGE}");
}

fn parse_transport(flag: &str, value: Option<String>) -> Result<NetworkTransport, String> {
    match value
        .ok_or_else(|| format!("{flag} requires udp or routed-udp"))?
        .as_str()
    {
        "udp" | "direct-udp" => Ok(NetworkTransport::Udp),
        "routed" | "routed-udp" => Ok(NetworkTransport::RoutedUdp),
        value => Err(format!("invalid value for {flag}: {value}")),
    }
}

/// A routed IPv6 supervisor must be reached from an IPv6 client socket. Preserve the historical
/// loopback defaults for IPv4 while deriving the local family from the selected server address.
#[cfg(feature = "automation")]
fn default_local_addr(server_addr: SocketAddr) -> SocketAddr {
    match server_addr {
        SocketAddr::V4(_) => "127.0.0.1:0"
            .parse()
            .expect("default IPv4 local address is valid"),
        SocketAddr::V6(_) => "[::]:0"
            .parse()
            .expect("default IPv6 local address is valid"),
    }
}

/// The interactive product shell: routed UDP, a random client identity, and an editable server
/// prefill. This is the complete player-build contract.
#[cfg(not(feature = "automation"))]
fn parse_args() -> Result<ClientNetworkConfig, String> {
    let mut args = env::args().skip(1);
    let mut server = None;
    let mut transport = None;
    let mut window_size = None;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--server" => server = Some(args.next().ok_or("--server requires a value")?),
            "--transport" => transport = Some(parse_transport(&flag, args.next())?),
            "--window-size" => window_size = Some(parse_window_size(&flag, args.next())?),
            "--help" | "-h" => {
                usage();
                process::exit(0);
            }
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    if transport.is_some_and(|transport| transport != NetworkTransport::RoutedUdp) {
        return Err("the interactive product shell requires --transport routed-udp".to_string());
    }
    ClientNetworkConfig::product_shell(server, window_size)
}

#[cfg(feature = "automation")]
use automation::parse_args;

fn parse_window_size(flag: &str, value: Option<String>) -> Result<(u16, u16), String> {
    let value = value.ok_or_else(|| format!("{flag} requires WIDTHxHEIGHT"))?;
    let (width, height) = value
        .split_once(['x', 'X'])
        .ok_or_else(|| format!("{flag} requires WIDTHxHEIGHT"))?;
    Ok((
        width
            .parse()
            .map_err(|_| format!("invalid width for {flag}"))?,
        height
            .parse()
            .map_err(|_| format!("invalid height for {flag}"))?,
    ))
}

fn main() -> AppExit {
    let config = match parse_args() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("brawler-client: {error}");
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
