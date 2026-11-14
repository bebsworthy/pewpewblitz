//! Android entry point for the `PewPew Blitz` player client.
//!
//! `#[bevy_main]` exports the `android_main` symbol that `GameActivity` calls and stores the
//! `AndroidApp` handle that Bevy's windowing, assets, and the client's storage paths use. The
//! application itself is the shared desktop player shell; only the configuration source differs
//! because Android has no command line.

use bevy::prelude::*;
use brawler::{client::build_app_with_config, config::ClientNetworkConfig};

/// Optional editable server prefill baked in by `just android-build server=HOST:PORT`.
const SERVER_PREFILL: Option<&str> = option_env!("BRAWLER_ANDROID_SERVER");

#[bevy_main]
fn main() {
    let server_prefill = SERVER_PREFILL
        .filter(|address| !address.trim().is_empty())
        .map(str::to_string);
    match ClientNetworkConfig::product_shell(server_prefill, None) {
        Ok(config) => {
            #[cfg(target_os = "android")]
            frame_pacing::configure_task_pools();
            let mut app = build_app_with_config(config);
            #[cfg(target_os = "android")]
            frame_pacing::install(&mut app);
            app.run();
        }
        Err(error) => error!("PewPew Blitz could not start: {error}"),
    }
}

#[cfg(target_os = "android")]
mod frame_pacing;
