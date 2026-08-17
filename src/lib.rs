//! Reusable application composition for Brawler's client and dedicated server.

pub mod abilities;
pub mod builds;
pub mod combat;
pub mod config;
pub mod content;
pub mod gameplay;
pub mod map;
pub mod matchplay;
pub mod movement;
pub mod protocol;
pub mod terrain;
pub mod timing;

#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "server")]
pub mod server;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
