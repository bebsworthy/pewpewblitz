//! Terrain wire API: the pure client convergence machine and the server-gated recovery
//! request validation and snapshot/event publication live in sibling submodules. This
//! module owns only the shared composition surface and registration-facing re-exports.

mod convergence;

#[cfg(feature = "server")]
mod server;
#[cfg(feature = "server")]
pub(crate) use server::register_terrain_network;
#[cfg(feature = "server")]
pub use server::{MAX_TERRAIN_REQUEST_BYTES, TerrainRecoveryCooldown};

pub use convergence::{
    ClientTerrainConvergence, TerrainConvergenceAction, TerrainConvergencePhase,
};
