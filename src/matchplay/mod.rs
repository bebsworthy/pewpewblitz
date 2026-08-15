//! Shared match state and server-authoritative Wipeout composition.

mod model;
#[cfg(feature = "server")]
mod server;
mod telemetry;
mod wipeout;

pub use model::*;
#[cfg(feature = "server")]
pub use server::*;
pub use telemetry::*;
pub use wipeout::*;

#[cfg(test)]
mod tests;
