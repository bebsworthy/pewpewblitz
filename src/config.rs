//! Validated process configuration shared by the client and dedicated server.

use bevy::prelude::Resource;
use core::{net::SocketAddr, time::Duration};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum NetworkTransport {
    Udp,
    #[cfg(feature = "network-test")]
    Crossbeam,
}

/// Runtime configuration for the dedicated server.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct ServerNetworkConfig {
    pub bind_addr: SocketAddr,
    pub transport: NetworkTransport,
    pub network_protocol_id: u64,
    pub max_clients: usize,
    pub handshake_timeout: Duration,
    pub client_timeout: Duration,
}

impl Default for ServerNetworkConfig {
    fn default() -> Self {
        Self {
            bind_addr: "127.0.0.1:5000"
                .parse()
                .expect("default server address is valid"),
            transport: NetworkTransport::Udp,
            network_protocol_id: crate::protocol::NETWORK_PROTOCOL_ID,
            max_clients: 8,
            handshake_timeout: Duration::from_secs(3),
            client_timeout: Duration::from_secs(3),
        }
    }
}

impl ServerNetworkConfig {
    /// Validate values that would otherwise make a process run indefinitely or wrap IDs.
    pub fn validate(&self) -> Result<(), String> {
        if self.max_clients == 0 {
            return Err("--max-clients must be greater than zero".to_string());
        }
        if self.handshake_timeout.is_zero() {
            return Err("--handshake-timeout-ms must be greater than zero".to_string());
        }
        if self.client_timeout.is_zero() {
            return Err("client timeout must be greater than zero".to_string());
        }
        Ok(())
    }
}

/// Runtime configuration for one client process.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct ClientNetworkConfig {
    pub server_addr: SocketAddr,
    pub local_addr: SocketAddr,
    pub transport: NetworkTransport,
    pub network_protocol_id: u64,
    pub client_id: u64,
    pub expected_protocol_version: u16,
    pub expected_build_version: String,
    pub connect_timeout: Duration,
    pub headless: bool,
    pub exit_after_roster: Option<usize>,
}

impl ClientNetworkConfig {
    #[must_use]
    pub fn new(client_id: u64) -> Self {
        Self {
            server_addr: "127.0.0.1:5000"
                .parse()
                .expect("default server address is valid"),
            local_addr: "127.0.0.1:0"
                .parse()
                .expect("default client address is valid"),
            transport: NetworkTransport::Udp,
            network_protocol_id: crate::protocol::NETWORK_PROTOCOL_ID,
            client_id,
            expected_protocol_version: crate::protocol::SUPPORTED_PROTOCOL_VERSION,
            expected_build_version: crate::VERSION.to_string(),
            connect_timeout: Duration::from_secs(5),
            headless: false,
            exit_after_roster: None,
        }
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.exit_after_roster.is_some_and(|count| count == 0) {
            return Err("--exit-after-roster must be greater than zero".to_string());
        }
        if self.connect_timeout.is_zero() {
            return Err("client connect timeout must be greater than zero".to_string());
        }
        if self.expected_build_version.is_empty() {
            return Err("expected build version must not be empty".to_string());
        }
        Ok(())
    }
}
