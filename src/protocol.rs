//! Stable protocol registration shared by the client and dedicated server.

use bevy::prelude::*;
use lightyear::prelude::{
    AppChannelExt, AppComponentExt, AppMessageExt, ChannelMode, ChannelSettings, NetworkDirection,
    ReliableSettings,
};
use serde::{Deserialize, Serialize};

/// Netcode protocol ID. Bump this for incompatible wire-level changes.
pub const NETWORK_PROTOCOL_ID: u64 = 0x4252_4157_4c45_5232;

/// Brawler-level compatibility version exchanged after Netcode connects.
pub const SUPPORTED_PROTOCOL_VERSION: u16 = 1;

/// Development-only key for local loopback sessions. This is not authentication.
pub const DEVELOPMENT_PRIVATE_KEY: [u8; 32] = [0x42; 32];

/// Ordered reliable channel for the compatibility handshake and join outcome.
pub struct SessionChannel;

/// Stable server-assigned player identity.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlayerId(pub u64);

/// Stable server-assigned network entity identity.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct NetworkEntityId(pub u64);

/// Marker for the placeholder replicated by this milestone.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct PlaceholderPlayer;

/// Small replicated state proving that the server owns the placeholder data.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PlaceholderState {
    pub spawn_slot: u16,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct ClientHello {
    pub protocol_version: u16,
    pub build_version: String,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum JoinOutcome {
    Accepted {
        player_id: PlayerId,
        network_entity_id: NetworkEntityId,
    },
    Rejected {
        reason: JoinRejection,
    },
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub enum JoinRejection {
    ProtocolVersionMismatch,
    BuildVersionMismatch,
    HandshakeTimeout,
    ServerFull,
    IdentifierExhausted,
}

/// Registers every message, channel, and replicated component used by the sandbox.
pub struct ProtocolPlugin;

impl Plugin for ProtocolPlugin {
    fn build(&self, app: &mut App) {
        app.register_message::<ClientHello>()
            .add_direction(NetworkDirection::ClientToServer);
        app.register_message::<JoinOutcome>()
            .add_direction(NetworkDirection::ServerToClient);
        app.add_channel::<SessionChannel>(ChannelSettings {
            mode: ChannelMode::OrderedReliable(ReliableSettings::default()),
            ..default()
        })
        .add_direction(NetworkDirection::Bidirectional);

        app.component::<PlaceholderPlayer>().replicate_once();
        app.component::<PlayerId>().replicate_once();
        app.component::<NetworkEntityId>().replicate_once();
        app.component::<PlaceholderState>().replicate();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use lightyear::prelude::{AppMessageExt, ComponentRegistry, MessageRegistry};

    #[test]
    fn protocol_plugin_registers_messages_channel_and_components() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, bevy::state::app::StatesPlugin));
        #[cfg(feature = "client")]
        app.add_plugins(lightyear::prelude::client::ClientPlugins {
            tick_duration: crate::timing::SIMULATION_TICK,
        });
        #[cfg(all(not(feature = "client"), feature = "server"))]
        app.add_plugins(lightyear::prelude::server::ServerPlugins {
            tick_duration: crate::timing::SIMULATION_TICK,
        });
        app.add_plugins(ProtocolPlugin);

        assert!(app.is_message_registered::<ClientHello>());
        assert!(app.is_message_registered::<JoinOutcome>());
        assert!(app.world().contains_resource::<MessageRegistry>());
        assert!(
            app.world()
                .contains_resource::<lightyear::prelude::ChannelRegistry>()
        );
        let channels = app
            .world()
            .resource::<lightyear::prelude::ChannelRegistry>();
        assert!((0..32).any(|id| {
            channels.get_name_from_net_id(id) == core::any::type_name::<SessionChannel>()
        }));
        let components = app.world().resource::<ComponentRegistry>();
        assert!(components.is_registered::<PlaceholderPlayer>());
        assert!(components.is_registered::<PlayerId>());
        assert!(components.is_registered::<NetworkEntityId>());
        assert!(components.is_registered::<PlaceholderState>());
    }

    #[test]
    fn session_messages_round_trip_with_serde() {
        let message = JoinOutcome::Accepted {
            player_id: PlayerId(4),
            network_entity_id: NetworkEntityId(9),
        };
        let bytes = postcard::to_allocvec(&message).expect("message serializes");
        let decoded: JoinOutcome = postcard::from_bytes(&bytes).expect("message deserializes");
        assert_eq!(decoded, message);
    }
}
