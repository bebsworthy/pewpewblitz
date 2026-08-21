//! Brawler's macOS client process.

use bevy::app::AppExit;
use brawler::client::build_app_with_config;
use brawler::config::{
    ClientNetworkConfig, NetworkTransport, RenderMeasurementConfig, ScreenshotSchedule,
    WindowedCombatDemo, WindowedControllerDemo,
};
use core::net::SocketAddr;
use std::{env, path::PathBuf, process, time::Duration};

fn usage() {
    eprintln!(
        "usage: brawler-client [--client-id <u64>] [--auto-connect] [--server <HOST[:PORT]>] [--local-addr <IP:PORT>] [--transport <udp|routed-udp>] [--build-preset <1-5> (5=custom)] [--window-size <WIDTHxHEIGHT>] [--headless (...)] [--product-game-type <ID>] [--combat-demo | --controller-demo] [--screenshot-dir <DIR> ...] [--render-report <FILE> --render-warmup-seconds <1-120> --render-measure-seconds <1-120>]"
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

#[allow(
    clippy::too_many_lines,
    reason = "the executable's bounded flag parser keeps its complete CLI contract visible"
)]
fn parse_args() -> Result<ClientNetworkConfig, String> {
    let mut args = env::args().skip(1);
    let mut client_id = None;
    let mut server = None;
    let mut local_addr = None;
    let mut transport = None;
    let mut headless = false;
    let mut auto_connect = false;
    let mut exit_after_roster = None;
    let mut exit_after_lobby_return = false;
    let mut exit_after_lobby_welcome = false;
    let mut product_queue_smoke = false;
    let mut product_match_smoke = false;
    let mut product_requeue_smoke = false;
    let mut product_match_players_per_team = 2;
    let mut product_match_game_type = None;
    let mut headless_move = None;
    let mut headless_aim = None;
    let mut headless_aim_at_dummy = false;
    let mut headless_fire = false;
    let mut headless_ultimate = false;
    let mut headless_simulation_ticks = None;
    let mut build_preset = None;
    let mut windowed_combat_demo = false;
    let mut windowed_controller_demo = false;
    let mut window_size = None;
    let mut screenshot_dir = None;
    let mut screenshot_first: u32 = 30;
    let mut screenshot_every: u32 = 60;
    let mut screenshot_count: u32 = 1;
    let mut render_report = None;
    let mut render_warmup_seconds: u64 = 10;
    let mut render_measure_seconds: u64 = 30;
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--client-id" => client_id = Some(parse_value(&flag, args.next())?),
            "--server" => server = Some(args.next().ok_or("--server requires a value")?),
            "--local-addr" => local_addr = Some(parse_value::<SocketAddr>(&flag, args.next())?),
            "--transport" => transport = Some(parse_transport(&flag, args.next())?),
            "--headless" => headless = true,
            "--auto-connect" => auto_connect = true,
            "--exit-after-roster" => {
                exit_after_roster = Some(parse_value(&flag, args.next())?);
            }
            "--exit-after-lobby-return" => exit_after_lobby_return = true,
            "--exit-after-lobby-welcome" => exit_after_lobby_welcome = true,
            "--product-queue-smoke" => product_queue_smoke = true,
            "--product-match-smoke" => product_match_smoke = true,
            "--product-match-smoke-1v1" => {
                product_match_smoke = true;
                product_match_players_per_team = 1;
            }
            "--product-match-smoke-3v3" => {
                product_match_smoke = true;
                product_match_players_per_team = 3;
            }
            "--product-requeue-smoke" => {
                product_match_smoke = true;
                product_requeue_smoke = true;
                product_match_players_per_team = 1;
            }
            "--product-game-type" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                product_match_game_type = Some(
                    brawler::lobby::GameTypeId::new(value)
                        .map_err(|error| format!("invalid value for {flag}: {error}"))?,
                );
            }
            "--move-axis" => headless_move = Some(parse_axis(&flag, args.next())?),
            "--aim-axis" => headless_aim = Some(parse_axis(&flag, args.next())?),
            "--aim-dummy" => headless_aim_at_dummy = true,
            "--fire" => headless_fire = true,
            "--ultimate" => headless_ultimate = true,
            "--simulation-ticks" => {
                headless_simulation_ticks = Some(parse_value(&flag, args.next())?);
            }
            "--build-preset" | "--weapon-preset" | "--weapon" => {
                build_preset = Some(parse_value(&flag, args.next())?);
            }
            "--combat-demo" => windowed_combat_demo = true,
            "--controller-demo" => windowed_controller_demo = true,
            "--window-size" => window_size = Some(parse_window_size(&flag, args.next())?),
            "--screenshot-dir" => {
                let value = args
                    .next()
                    .ok_or_else(|| format!("{flag} requires a value"))?;
                screenshot_dir = Some(PathBuf::from(value));
            }
            "--screenshot-first" => screenshot_first = parse_value(&flag, args.next())?,
            "--screenshot-every" => screenshot_every = parse_value(&flag, args.next())?,
            "--screenshot-count" => screenshot_count = parse_value(&flag, args.next())?,
            "--render-report" => {
                render_report = Some(PathBuf::from(
                    args.next()
                        .ok_or_else(|| format!("{flag} requires a value"))?,
                ));
            }
            "--render-warmup-seconds" => {
                render_warmup_seconds = parse_value(&flag, args.next())?;
            }
            "--render-measure-seconds" => {
                render_measure_seconds = parse_value(&flag, args.next())?;
            }
            "--help" | "-h" => {
                usage();
                process::exit(0);
            }
            _ => return Err(format!("unknown flag: {flag}")),
        }
    }
    let noninteractive =
        auto_connect || headless || windowed_combat_demo || windowed_controller_demo;
    let client_id = match (client_id, noninteractive) {
        (Some(_), false) => {
            return Err("--client-id is not accepted by the interactive product shell".to_string());
        }
        (Some(client_id), true) => client_id,
        (None, true) => {
            return Err("--client-id is required with auto-connect or automation".to_string());
        }
        (None, false) => random_nonzero_client_id()?,
    };
    if headless
        && exit_after_roster.is_none()
        && !exit_after_lobby_welcome
        && !product_queue_smoke
        && !product_match_smoke
    {
        return Err(
            "--headless requires a lobby, queue, product-match, or roster exit condition"
                .to_string(),
        );
    }
    if !headless && exit_after_roster.is_some() {
        return Err("--exit-after-roster requires --headless".to_string());
    }
    if headless && windowed_controller_demo {
        return Err("--controller-demo requires a windowed client".to_string());
    }
    let mut config = ClientNetworkConfig::new(client_id);
    if noninteractive {
        config.server_addr = server
            .as_deref()
            .map(str::parse::<SocketAddr>)
            .transpose()
            .map_err(|_| "--server must be a numeric socket address in automation".to_string())?
            .unwrap_or(config.server_addr);
    } else {
        config.product_server_prefill = server;
    }
    if !noninteractive && local_addr.is_some() {
        return Err("--local-addr is not accepted by the interactive product shell".to_string());
    }
    // A routed IPv6 supervisor must be reached from an IPv6 client socket. Preserve the
    // historical loopback defaults for IPv4 while deriving the local family from an explicitly
    // selected server address. `--local-addr` remains available for a concrete interface or
    // port when a caller needs one.
    config.local_addr = local_addr.unwrap_or_else(|| match config.server_addr {
        SocketAddr::V4(_) => "127.0.0.1:0"
            .parse()
            .expect("default IPv4 local address is valid"),
        SocketAddr::V6(_) => "[::]:0"
            .parse()
            .expect("default IPv6 local address is valid"),
    });
    config.transport = transport.unwrap_or(if noninteractive {
        NetworkTransport::Udp
    } else {
        NetworkTransport::RoutedUdp
    });
    if !noninteractive && config.transport != NetworkTransport::RoutedUdp {
        return Err("the interactive product shell requires --transport routed-udp".to_string());
    }
    config.headless = headless;
    config.auto_connect =
        auto_connect || headless || windowed_combat_demo || windowed_controller_demo;
    config.exit_after_roster = exit_after_roster;
    config.exit_after_lobby_return = exit_after_lobby_return;
    config.exit_after_lobby_welcome = exit_after_lobby_welcome;
    config.product_queue_smoke = product_queue_smoke;
    config.product_match_smoke = product_match_smoke;
    config.product_requeue_smoke = product_requeue_smoke;
    config.product_match_players_per_team = product_match_players_per_team;
    config.product_match_game_type = product_match_game_type;
    if product_match_smoke {
        // Six debug clients can contend during simultaneous local startup. Keep the automation
        // connection budget inside M05's loading deadline without weakening the normal product
        // shell's five-second feedback bound.
        config.connect_timeout = Duration::from_secs(10);
    }
    config.headless_move = headless_move;
    config.headless_aim = headless_aim;
    config.headless_aim_at_dummy = headless_aim_at_dummy;
    config.headless_fire = headless_fire;
    config.headless_ultimate = headless_ultimate;
    config.headless_simulation_ticks = headless_simulation_ticks;
    config.build_preset = build_preset;
    config.windowed_combat_demo = windowed_combat_demo.then_some(WindowedCombatDemo);
    config.windowed_controller_demo = windowed_controller_demo.then_some(WindowedControllerDemo);
    config.window_size = window_size;
    if let Some(dir) = screenshot_dir {
        config.screenshot_schedule = Some(ScreenshotSchedule {
            dir,
            first_update: screenshot_first,
            interval: screenshot_every,
            count: screenshot_count,
        });
    }
    if let Some(report_path) = render_report {
        config.render_measurement = Some(RenderMeasurementConfig {
            report_path,
            warmup: Duration::from_secs(render_warmup_seconds),
            measurement: Duration::from_secs(render_measure_seconds),
        });
    }
    if windowed_combat_demo {
        config.headless_aim_at_dummy = true;
        config.headless_fire = true;
    }
    config
        .validate()
        .map_err(|error| format!("invalid client configuration: {error}"))?;
    Ok(config)
}

fn random_nonzero_client_id() -> Result<u64, String> {
    for _ in 0..4 {
        let mut bytes = [0_u8; 8];
        getrandom::fill(&mut bytes)
            .map_err(|error| format!("OS entropy unavailable for client identity: {error}"))?;
        let id = u64::from_ne_bytes(bytes);
        if id != 0 {
            return Ok(id);
        }
    }
    Err("OS entropy repeatedly produced a zero client identity".to_string())
}

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
