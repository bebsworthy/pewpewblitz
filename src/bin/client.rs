//! Brawler's macOS client process.

use bevy::app::AppExit;
use brawler::client::build_app_with_config;
use brawler::config::{
    ClientNetworkConfig, ScreenshotSchedule, WindowedCombatDemo, WindowedControllerDemo,
};
use core::net::SocketAddr;
use std::{env, path::PathBuf, process};

fn usage() {
    eprintln!(
        "usage: brawler-client --client-id <u64> [--server <IP:PORT>] [--build-preset <1-5> (5=custom)] [--window-size <WIDTHxHEIGHT>] [--headless --exit-after-roster <N> --move-axis <X,Y> --aim-axis <X,Y> --aim-dummy --fire --ultimate --simulation-ticks <N>] [--combat-demo | --controller-demo] [--screenshot-dir <DIR> --screenshot-first <N> --screenshot-every <N> --screenshot-count <N>]"
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
    if headless && windowed_controller_demo {
        return Err("--controller-demo requires a windowed client".to_string());
    }
    let mut config = ClientNetworkConfig::new(client_id);
    config.server_addr = server.unwrap_or(config.server_addr);
    config.headless = headless;
    config.exit_after_roster = exit_after_roster;
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
    if windowed_combat_demo {
        config.headless_aim_at_dummy = true;
        config.headless_fire = true;
    }
    config
        .validate()
        .map_err(|error| format!("invalid client configuration: {error}"))?;
    Ok(config)
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
            process::exit(2);
        }
    };
    build_app_with_config(config).run()
}
