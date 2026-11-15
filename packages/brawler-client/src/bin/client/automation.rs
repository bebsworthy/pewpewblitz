//! Automation-only client CLI: smoke, headless, demo, scheduled-capture, and render-report
//! flags. Compiled only with the `automation` feature; the player build parses the small
//! product contract in `client.rs` instead.

use super::{default_local_addr, parse_transport, parse_window_size, random_nonzero_client_id};
use brawler::config::{ClientNetworkConfig, NetworkTransport};
use brawler_client::{
    ClientAutomationConfig, RenderMeasurementConfig, ScreenshotSchedule, WindowedCombatDemo,
    WindowedControllerDemo,
};
use core::net::SocketAddr;
use std::{env, path::PathBuf, process, time::Duration};

pub(super) const USAGE: &str = "usage: brawler-client [--client-id <u64>] [--auto-connect] [--server <HOST[:PORT]>] [--local-addr <IP:PORT>] [--transport <udp|routed-udp>] [--weapon-preset <BUILD-CATALOG-WEAPON-ID>] [--window-size <WIDTHxHEIGHT>] [--headless (...)] [--product-game-type <ID>] [--product-practice-smoke | --product-network-change-smoke | --product-rejoin-smoke] [--combat-demo | --controller-demo] [--screenshot-dir <DIR> ...] [--render-report <FILE> --render-warmup-seconds <1-120> --render-measure-seconds <1-120> [--render-reduced-effects]]";

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

#[allow(
    clippy::too_many_lines,
    reason = "the executable's bounded flag parser keeps its complete CLI contract visible"
)]
pub(super) fn parse_args() -> Result<(ClientNetworkConfig, ClientAutomationConfig), String> {
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
    let mut product_practice_smoke = false;
    let mut product_network_change_smoke = false;
    let mut product_rejoin_smoke = false;
    let mut product_requeue_smoke = false;
    let mut product_match_players_per_team = 2;
    let mut product_match_game_type = None;
    let mut headless_move = None;
    let mut headless_aim = None;
    let mut headless_aim_at_dummy = false;
    let mut headless_fire = false;
    let mut headless_ultimate = false;
    let mut headless_simulation_ticks = None;
    let mut weapon_preset = None;
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
    let mut render_reduced_effects = false;
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
            "--product-practice-smoke" => {
                product_match_smoke = true;
                product_practice_smoke = true;
            }
            "--product-network-change-smoke" => {
                product_match_smoke = true;
                product_practice_smoke = true;
                product_network_change_smoke = true;
            }
            "--product-rejoin-smoke" => {
                product_match_smoke = true;
                product_practice_smoke = true;
                product_rejoin_smoke = true;
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
            "--weapon-preset" | "--weapon" => {
                weapon_preset = Some(parse_value(&flag, args.next())?);
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
            "--render-reduced-effects" => render_reduced_effects = true,
            "--help" | "-h" => {
                super::usage();
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
    config.local_addr = local_addr.unwrap_or_else(|| default_local_addr(config.server_addr));
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
    if product_match_smoke {
        // Six debug clients can contend during simultaneous local startup. Keep the automation
        // connection budget inside M05's loading deadline without weakening the normal product
        // shell's five-second feedback bound.
        config.connect_timeout = Duration::from_secs(10);
    }
    config.window_size = window_size;
    if render_report.is_none() && render_reduced_effects {
        return Err("--render-reduced-effects requires --render-report".to_string());
    }
    let automation = ClientAutomationConfig {
        exit_after_roster,
        exit_after_lobby_return,
        exit_after_lobby_welcome,
        product_queue_smoke,
        product_match_smoke,
        product_practice_smoke,
        product_requeue_smoke,
        product_network_change_smoke,
        product_rejoin_smoke,
        product_match_players_per_team,
        product_match_game_type,
        headless_move,
        headless_aim,
        // The windowed combat demo aims at the dummy and fires through the scripted input path.
        headless_aim_at_dummy: headless_aim_at_dummy || windowed_combat_demo,
        headless_fire: headless_fire || windowed_combat_demo,
        headless_ultimate,
        headless_simulation_ticks,
        weapon_preset,
        windowed_combat_demo: windowed_combat_demo.then_some(WindowedCombatDemo),
        windowed_controller_demo: windowed_controller_demo.then_some(WindowedControllerDemo),
        screenshot_schedule: screenshot_dir.map(|dir| ScreenshotSchedule {
            dir,
            first_update: screenshot_first,
            interval: screenshot_every,
            count: screenshot_count,
        }),
        render_measurement: render_report.map(|report_path| RenderMeasurementConfig {
            report_path,
            warmup: Duration::from_secs(render_warmup_seconds),
            measurement: Duration::from_secs(render_measure_seconds),
            reduced_effects: render_reduced_effects,
        }),
    };
    config
        .validate()
        .and_then(|()| automation.validate(&config))
        .map_err(|error| format!("invalid client configuration: {error}"))?;
    Ok((config, automation))
}
