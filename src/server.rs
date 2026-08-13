//! Dedicated headless-server composition root.

use crate::{VERSION, gameplay::GameplayPlugin, protocol::ProtocolPlugin};
use bevy::{
    app::{ScheduleRunnerPlugin, TerminalCtrlCHandlerPlugin},
    log::LogPlugin,
    prelude::*,
};

/// Server-only marker proving that no presentation plugin is required.
#[derive(Default, Resource, Debug, PartialEq, Eq)]
pub struct DedicatedServer;

/// Adds dedicated-server startup diagnostics and clean scheduled execution.
pub struct DedicatedServerPlugin;

impl Plugin for DedicatedServerPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<DedicatedServer>()
            .add_systems(Startup, log_server_startup);
    }
}

fn log_server_startup() {
    info!(
        mode = "dedicated-server",
        version = VERSION,
        tick_hz = crate::timing::SIMULATION_TICK_HZ,
        "brawler dedicated server started"
    );
}

/// Build the headless dedicated server application.
pub fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins.set(ScheduleRunnerPlugin::run_loop(
        crate::timing::SIMULATION_TICK,
    )))
    .add_plugins(TerminalCtrlCHandlerPlugin)
    .add_plugins(LogPlugin::default())
    .add_plugins((GameplayPlugin, ProtocolPlugin, DedicatedServerPlugin));
    app
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{protocol::ProtocolVersion, timing::SimulationTick};
    use lightyear::prelude::{AppMessageExt, MessageRegistry};

    #[test]
    fn server_composition_is_headless_and_reuses_shared_plugins() {
        let mut app = build_app();
        app.finish();

        assert!(app.world().contains_resource::<DedicatedServer>());
        assert!(app.world().contains_resource::<SimulationTick>());
        assert!(app.world().contains_resource::<MessageRegistry>());
        assert!(app.is_message_registered::<ProtocolVersion>());
    }
}
