//! Reusable application composition for Brawler's client and dedicated server.

pub mod abilities;
pub mod builds;
pub mod combat;
pub mod config;
pub mod content;
pub mod diagnostics;
pub mod gameplay;
pub mod lobby;
pub mod map;
pub mod matchplay;
pub mod movement;
pub mod profiles;
pub mod protocol;
pub mod terrain;
pub mod timing;
pub mod weapon_parts;

#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "server")]
pub mod server;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
