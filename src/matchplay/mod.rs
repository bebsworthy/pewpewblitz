//! Shared match state and server-authoritative Wipeout composition.

#[cfg(feature = "server")]
mod lifecycle;
mod model;
#[cfg(feature = "server")]
mod server;
mod telemetry;
mod wipeout;

#[cfg(feature = "server")]
pub use lifecycle::*;
pub use model::*;
#[cfg(feature = "server")]
pub use server::*;
pub use telemetry::*;
pub use wipeout::*;

#[cfg(test)]
mod tests;
