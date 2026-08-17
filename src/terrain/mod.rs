//! Server-authoritative quantized destructible terrain.
//!
//! Gameplay solidity is an 8-world-unit occupancy grid divided into sparse 32x32-cell
//! chunks. This module owns the shared grid model and pure logic; server-gated submodules
//! own authoritative mutation, Avian collision, and recovery, while the client-gated
//! submodule owns presentation and convergence without gameplay collision.

use bevy::prelude::{App, Plugin};

#[cfg(feature = "server")]
pub mod authority;
#[cfg(feature = "client")]
pub mod client;
#[cfg(feature = "server")]
pub mod collider;
pub mod grid;
#[cfg(feature = "server")]
pub mod lifecycle;
pub mod model;
pub mod network;
pub mod telemetry;
#[cfg(test)]
mod tests;

#[cfg(feature = "server")]
pub use authority::{
    AuthoritativeTerrainPlugin, PendingTerrainBrush, PendingTerrainBrushes, TerrainBrushBatch,
    TerrainBrushEpoch, TerrainChunk, TerrainChunkCollision, TerrainChunkIndex, TerrainChunkState,
    TerrainOutbox, TerrainRecoveryCache, TerrainRoot, TerrainTransaction,
};
#[cfg(feature = "client")]
pub use client::{ClientTerrainPlugin, ClientTerrainReadiness, paint_chunk_pixels};
#[cfg(all(test, feature = "server"))]
pub(crate) use lifecycle::reset_terrain_on_match_restart;
#[cfg(feature = "server")]
pub use lifecycle::teardown_authoritative_terrain;
pub use model::*;
pub use network::{ClientTerrainConvergence, TerrainConvergenceAction, TerrainConvergencePhase};

/// Shared terrain composition for both roles: the pure convergence state and its bounds.
/// No role mutation, presentation, or wire registration happens here.
pub struct TerrainCorePlugin;

impl Plugin for TerrainCorePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientTerrainConvergence>();
    }
}

/// Shared terrain schedule sets for the authoritative fixed-post chain. Role plugins
/// configure the exact ordering; these sets never mutate state by themselves.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, bevy::prelude::SystemSet)]
pub enum TerrainSet {
    CollectBrushes,
    ApplyBrushes,
    RebuildCollision,
    ValidateFighters,
    Publish,
}
