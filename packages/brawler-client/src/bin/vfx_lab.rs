use bevy::app::AppExit;
use brawler_client::{VfxLabAccent, VfxLabConfig, VfxLabRelationship, build_vfx_lab_app};
use std::{env, process, str::FromStr};

fn usage() {
    eprintln!(
        "Usage: brawler-vfx-lab [--effect KEY] [--fixture NAME] [--identity N] [--radius WORLD_UNITS] [--theme N] [--sample-ticks N] [--accent neutral|cold|fire|poison|restoration] [--relationship neutral|ally|enemy] [--lobbed-impact] [--ultimate] [--reduced] [--primitive] [--paused] [--window-size WIDTHxHEIGHT] [--capture-dir PATH] [--capture-after UPDATES] [--exit-after-capture]\n\
         Controls: Left/Right select · Space restart · Enter pause · . step · N identity · C accent · O relationship · U ultimate · R reduced · P primitive · I guide/details · F12 capture · Esc quit"
    );
}

/// The next argument parsed as `T`, with the flag's own missing/invalid messages.
fn parse_next<T: FromStr>(
    args: &mut impl Iterator<Item = String>,
    missing: &str,
    invalid: &str,
) -> Result<T, String> {
    args.next()
        .ok_or_else(|| missing.to_string())?
        .parse()
        .map_err(|_| invalid.to_string())
}

fn parse_window_size(value: &str) -> Result<(u16, u16), String> {
    let (width, height) = value
        .split_once(['x', 'X'])
        .ok_or_else(|| "--window-size requires WIDTHxHEIGHT".to_string())?;
    Ok((
        width
            .parse()
            .map_err(|_| "invalid VfxLab window width".to_string())?,
        height
            .parse()
            .map_err(|_| "invalid VfxLab window height".to_string())?,
    ))
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
            "--lobbed-impact" => config.lobbed_impact = true,
            "--exit-after-capture" => config.exit_after_capture = true,
            "--identity" => {
                config.presentation_identity = Some(parse_next(
                    &mut args,
                    "--identity requires an integer",
                    "invalid --identity value",
                )?);
            }
            "--radius" => {
                config.radius = Some(parse_next(
                    &mut args,
                    "--radius requires world units",
                    "invalid --radius value",
                )?);
            }
            "--theme" => {
                config.theme = Some(parse_next(
                    &mut args,
                    "--theme requires a map theme ID",
                    "invalid --theme value",
                )?);
            }
            "--sample-ticks" => {
                config.sample_ticks = Some(parse_next(
                    &mut args,
                    "--sample-ticks requires an integer",
                    "invalid --sample-ticks value",
                )?);
            }
            "--accent" => {
                let value = args.next().ok_or_else(|| {
                    "--accent requires neutral, cold, fire, poison, or restoration".to_string()
                })?;
                config.accent = VfxLabAccent::from_key(&value)
                    .ok_or_else(|| "invalid --accent value".to_string())?;
            }
            "--relationship" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--relationship requires neutral, ally, or enemy".to_string())?;
                config.relationship = VfxLabRelationship::from_key(&value)
                    .ok_or_else(|| "invalid --relationship value".to_string())?;
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
                config.capture_after_updates = Some(parse_next(
                    &mut args,
                    "--capture-after requires an update count",
                    "invalid --capture-after update count",
                )?);
            }
            "--window-size" => {
                let value = args
                    .next()
                    .ok_or_else(|| "--window-size requires WIDTHxHEIGHT".to_string())?;
                config.window_size = Some(parse_window_size(&value)?);
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
