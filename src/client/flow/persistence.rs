//! Local connection-flow persistence ownership.

use super::{
    ClientFlow, ClientNetworkConfig, FlowError, FlowErrorAction, FlowErrorKind, ServerSelectModel,
};
use crate::client::connection_persistence::{
    ClientConnectionsPath, ConnectionsFileV1, load_connections,
};
use bevy::prelude::{Commands, Res, ResMut, Resource};

#[derive(Resource, Clone, Debug)]
pub(in crate::client) struct ConnectionPersistence {
    pub(in crate::client) state: ConnectionsFileV1,
    pub(in crate::client) dirty_error: Option<String>,
}

#[derive(Resource, Default, Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::client) struct ClientLocalLoadFailures {
    pub(in crate::client) settings_failed: bool,
    pub(in crate::client) connections_failed: bool,
    pub(in crate::client) build_failed: bool,
}

pub(in crate::client) fn local_load_error(failures: ClientLocalLoadFailures) -> Option<FlowError> {
    let mut sources = Vec::new();
    if failures.settings_failed {
        sources.push("Settings");
    }
    if failures.connections_failed {
        sources.push("connection data");
    }
    if failures.build_failed {
        sources.push("saved build");
    }
    if sources.is_empty() {
        return None;
    }
    let message = format!(
        "{} could not be loaded; safe defaults are active",
        match sources.as_slice() {
            [one] => (*one).to_string(),
            [first, second] => format!("{first} and {second}"),
            [first, second, third] => format!("{first}, {second}, and {third}"),
            _ => unreachable!("three closed persistence sources"),
        }
    );
    Some(FlowError {
        kind: FlowErrorKind::Persistence,
        message,
        return_flow: ClientFlow::ServerSelect,
        actions: [Some(FlowErrorAction::ContinueWithDefaults), None],
    })
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(super) fn load_connection_state(
    mut commands: Commands,
    path: Res<ClientConnectionsPath>,
    config: Res<ClientNetworkConfig>,
    mut failures: ResMut<ClientLocalLoadFailures>,
) {
    let state = match load_connections(&path.0) {
        Ok(Some(state)) => state,
        Ok(None) => ConnectionsFileV1::empty(),
        Err(_) => {
            failures.connections_failed = true;
            ConnectionsFileV1::empty()
        }
    };
    let name = state
        .preferred_display_name
        .clone()
        .unwrap_or_else(|| crate::lobby::generated_display_name(config.client_id));
    let address = startup_server_address(&config, &state);
    commands.insert_resource(ServerSelectModel {
        address,
        committed_name: name.clone(),
        name,
        editing: None,
        caret: 0,
        inline_error: None,
    });
    commands.insert_resource(ConnectionPersistence {
        state,
        dirty_error: None,
    });
}

pub(super) fn startup_server_address(
    config: &ClientNetworkConfig,
    state: &ConnectionsFileV1,
) -> String {
    config.product_server_prefill.clone().unwrap_or_else(|| {
        state.recents.first().map_or_else(
            || "127.0.0.1:5000".to_string(),
            |recent| recent.address.clone(),
        )
    })
}
