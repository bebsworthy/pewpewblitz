use bevy::app::AppExit;
use brawler::client::{VfxLabConfig, build_vfx_lab_app};
use std::{env, process};

fn usage() {
    eprintln!(
        "Usage: brawler-vfx-lab [--effect KEY] [--reduced] [--primitive] [--window-size WIDTHxHEIGHT]\n\
         Controls: Left/Right select · Space replay · R reduced · P primitive · Esc quit"
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
