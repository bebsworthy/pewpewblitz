//! Stable protocol registration shared by the client and dedicated server.

use bevy::prelude::*;
use lightyear::prelude::{AppMessageExt, NetworkDirection};
use serde::{Deserialize, Serialize};

/// Protocol/build identity reserved for the connection milestone.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct ProtocolVersion {
    pub major: u16,
    pub minor: u16,
}

/// Stable protocol registration for both application configurations.
pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        app.register_message::<ProtocolVersion>()
            .add_direction(NetworkDirection::Bidirectional);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lightyear::prelude::MessageRegistry;

    #[test]
    fn protocol_plugin_registers_stable_message_without_connections() {
        let mut app = App::new();
        app.add_plugins(ProtocolPlugin);

        assert!(app.is_message_registered::<ProtocolVersion>());
        assert!(app.world().contains_resource::<MessageRegistry>());
    }
}
