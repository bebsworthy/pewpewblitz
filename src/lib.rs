//! Shared gameplay model, protocol, and dedicated-server composition for Brawler.
//!
//! The player client application is the `brawler-client` package, which builds on this crate's
//! `client` feature (BRL-0170). This crate keeps the name `brawler` so replicated type paths, and
//! therefore the protocol registry fingerprint, stay stable.

pub mod abilities;
mod bots;
pub mod builds;
pub mod characters;
pub mod combat;
pub mod concealment;
pub mod config;
pub mod content;
pub mod diagnostics;
pub mod gameplay;
pub mod lobby;
#[cfg(any(feature = "client", feature = "server"))]
#[doc(hidden)]
pub mod logging;
pub mod map;
pub mod matchplay;
#[doc(hidden)]
pub mod modes;
pub mod movement;
pub mod profiles;
pub mod protocol;
pub mod timing;
pub mod weapon_parts;

#[cfg(feature = "network-test")]
pub mod testing;

#[cfg(any(test, feature = "test-support"))]
#[doc(hidden)]
pub mod test_app;

#[cfg(feature = "server")]
pub mod server;

pub const VERSION: &str = env!("CARGO_PKG_VERSION");
