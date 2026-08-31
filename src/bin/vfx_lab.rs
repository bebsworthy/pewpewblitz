use bevy::app::AppExit;
use brawler::client::{VfxLabAccent, VfxLabConfig, build_vfx_lab_app};
use std::{env, process};

fn usage() {
    eprintln!(
        "Usage: brawler-vfx-lab [--effect KEY] [--fixture NAME] [--identity N] [--radius WORLD_UNITS] [--sample-ticks N] [--accent neutral|cold|fire] [--ultimate] [--reduced] [--primitive] [--paused] [--window-size WIDTHxHEIGHT] [--capture-dir PATH] [--capture-after UPDATES] [--exit-after-capture]\n\
         Controls: Left/Right select · Space restart · Enter pause · . step · N identity · C accent · U ultimate · R reduced · P primitive · I guide/details · F12 capture · Esc quit"
    );
}

fn parse_args() -> Result<VfxLabConfig, String> {
    let mut config = VfxLabConfig::default();
    let mut args = env::args().skip(1);
    while let Some(flag) = args.next() {
        match flag.as_str() {
            "--effect" => {
                config.effect_key = Some(
                    args.next()
                        .ok_or_else(|| "--effect requires a production request key".to_string())?,
                );
            }
            "--reduced" => config.reduced = true,
            "--primitive" => config.primitive = true,
            "--paused" => config.paused = true,
            "--ultimate" => config.ultimate = true,
            "--exit-after-capture" => config.exit_after_capture = true,
            "--identity" => {
                config.presentation_identity = Some(
                    args.next()
                        .ok_or_else(|| "--identity requires an integer".to_string())?
                        .parse()
                        .map_err(|_| "invalid --identity value".to_string())?,
                );
            }
            "--radius" => {
                config.radius = Some(
                    args.next()
                        .ok_or_else(|| "--radius requires world units".to_string())?
                        .parse()
                        .map_err(|_| "invalid --radius value".to_string())?,
                );
            }
            "--sample-ticks" => {
                config.sample_ticks = Some(
                    args.next()
                        .ok_or_else(|| "--sample-ticks requires an integer".to_string())?
                        .parse()
                        .map_err(|_| "invalid --sample-ticks value".to_string())?,
                );
            }
            "--accent" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--accent requires neutral, cold, or fire".to_string())?;
                config.accent = VfxLabAccent::from_key(&value)
                    .ok_or_else(|| "invalid --accent value".to_string())?;
            }
            "--fixture" => {
                config.fixture = Some(
                    args.next()
                        .ok_or_else(|| "--fixture requires a fixture name".to_string())?,
                );
            }
            "--capture-dir" => {
                config.capture_dir = Some(
                    args.next()
                        .map(Into::into)
                        .ok_or_else(|| "--capture-dir requires a path".to_string())?,
                );
            }
            "--capture-after" => {
                config.capture_after_updates = Some(
                    args.next()
                        .ok_or_else(|| "--capture-after requires an update count".to_string())?
                        .parse()
                        .map_err(|_| "invalid --capture-after update count".to_string())?,
                );
            }
            "--window-size" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--window-size requires WIDTHxHEIGHT".to_string())?;
                let (width, height) = value
                    .split_once(['x', 'X'])
                    .ok_or_else(|| "--window-size requires WIDTHxHEIGHT".to_string())?;
                config.window_size = Some((
                    width
                        .parse()
                        .map_err(|_| "invalid VfxLab window width".to_string())?,
                    height
                        .parse()
                        .map_err(|_| "invalid VfxLab window height".to_string())?,
                ));
            }
            "--help" | "-h" => {
                usage();
                process::exit(0);
            }
            _ => return Err(format!("unknown option: {flag}")),
        }
    }
    config.validate()?;
    Ok(config)
}

fn main() -> AppExit {
    let config = match parse_args() {
        Ok(config) => config,
        Err(error) => {
            eprintln!("brawler-vfx-lab: {error}");
            usage();
            process::exit(2);
        }
    };
    build_vfx_lab_app(config).run()
}
