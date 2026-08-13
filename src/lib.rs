//! Reusable application composition for Brawler's client and dedicated server.

pub mod gameplay;
pub mod protocol;
pub mod timing;

#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "server")]
pub mod server;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
