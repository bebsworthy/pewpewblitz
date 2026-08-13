//! Windowed client composition root.

use crate::{VERSION, gameplay::GameplayPlugin, protocol::ProtocolPlugin};
use bevy::{prelude::*, window::WindowCloseRequested};

/// Client-only marker proving presentation composition is installed.
#[derive(Default, Resource, Debug, PartialEq, Eq)]
pub struct ClientPresentation;

/// Adds client-only presentation and startup diagnostics.
pub struct ClientPresentationPlugin;

impl Plugin for ClientPresentationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientPresentation>()
            .add_systems(Startup, log_client_startup)
            .add_systems(Update, exit_on_close_requested);
    }
}

fn log_client_startup() {
    info!(
        mode = "client",
        version = VERSION,
        tick_hz = crate::timing::SIMULATION_TICK_HZ,
        "brawler client started"
    );
}

// Request application exit as soon as the user closes the client window. This
// keeps the process lifecycle deterministic for the one-command launcher,
// rather than relying only on the window plugin's deferred all-windows check.
fn exit_on_close_requested(
    mut close_requests: MessageReader<WindowCloseRequested>,
    mut app_exit: MessageWriter<AppExit>,
) {
    if close_requests.read().next().is_some() {
        app_exit.write(AppExit::Success);
    }
}

/// Build the blank client application.
pub fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(DefaultPlugins);
    install_client_plugins(&mut app);
    app
}

fn install_client_plugins(app: &mut App) {
    app.add_plugins((GameplayPlugin, ProtocolPlugin, ClientPresentationPlugin));
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{protocol::ProtocolVersion, timing::SimulationTick};
    use lightyear::prelude::{AppMessageExt, MessageRegistry};

    #[test]
    fn client_composition_installs_expected_responsibilities() {
        // Bevy's macOS event loop is main-thread-only. The process smoke test
        // covers DefaultPlugins; this test inspects the reusable composition
        // without constructing the platform event loop on Cargo's test thread.
        let mut app = App::new();
        install_client_plugins(&mut app);

        assert!(app.world().contains_resource::<ClientPresentation>());
        assert!(app.world().contains_resource::<SimulationTick>());
        assert!(app.world().contains_resource::<MessageRegistry>());
        assert!(app.is_message_registered::<ProtocolVersion>());
        assert!(app.world().contains_resource::<Time<Fixed>>());
    }
}
