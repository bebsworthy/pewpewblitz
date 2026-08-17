//! Stable terrain grid, chunk, brush, and wire shapes shared by every role.
//!
//! Every value here is integer-quantized or a stable ID. No image, collider, mesh, or
//! process-local entity identity belongs in this module or on the wire.

use crate::combat::AttackId;
use crate::map::MapInstanceId;
use crate::matchplay::MatchId;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Version of the quantized terrain representation itself. Bumping it invalidates every
/// terrain fingerprint and requires a specification-review recovery-contract change.
pub const TERRAIN_FORMAT_VERSION: u16 = 1;
/// Gameplay solidity resolution: one cell is one 8x8 world-unit square.
pub const TERRAIN_CELL_SIZE_WORLD: f32 = 8.0;
/// Brush centers and radii quantize to half-cell (4 world unit) coordinates.
pub const TERRAIN_SUBCELL_SIZE_WORLD: f32 = 4.0;
/// One chunk covers 32x32 cells.
pub const TERRAIN_CHUNK_SIDE_CELLS: u32 = 32;
/// One chunk covers 256x256 world units.
pub const TERRAIN_CHUNK_SIDE_WORLD: f32 = 256.0;
/// Occupancy is 32*32 bits = 16 `u64` words per chunk.
pub const TERRAIN_WORDS_PER_CHUNK: usize = 16;
/// Maximum global chunks a legal map's destructible reservations may allocate.
pub const MAX_TERRAIN_CHUNKS: usize = 221;
/// Maximum initially occupied cells across every destructible reservation.
pub const MAX_TERRAIN_CELLS: usize = 196_608;
/// Maximum authored terrain brush radius in world units.
pub const MAX_TERRAIN_BRUSH_RADIUS_WORLD: f32 = 64.0;
/// A brush smaller than one chunk diameter touches at most four chunks.
pub const MAX_TERRAIN_BRUSH_CHUNKS: usize = 4;
/// Terrain concurrency ceiling independent of team topology.
pub const MAX_TERRAIN_ACTIVE_FIGHTERS: usize = 24;
/// Distinct allocated chunks whose colliders may rebuild in one fixed tick.
pub const MAX_TERRAIN_COLLIDER_REBUILDS_PER_TICK: usize = 221;
/// Brushes admitted in one fixed tick.
pub const MAX_TERRAIN_BRUSHES_PER_TICK: usize = 24;
/// Bounded deferred-brush queue before newest excess facts are rejected.
pub const MAX_PENDING_TERRAIN_BRUSHES: usize = 64;
/// Bounded client live-event buffer while recovery is outstanding.
pub const MAX_BUFFERED_TERRAIN_EVENTS: usize = 64;
/// Serialized recovery-snapshot ceiling.
pub const MAX_TERRAIN_RECOVERY_BYTES: usize = 48 * 1024;
/// Serialized live-event ceiling.
pub const MAX_TERRAIN_EVENT_BYTES: usize = 96;
/// One accepted link may receive at most one recovery response per this many ticks.
pub const TERRAIN_RECOVERY_COOLDOWN_TICKS: u64 = 30;
/// Live cosmetic debris entity ceiling on windowed clients.
pub const MAX_TERRAIN_DEBRIS_EFFECTS: usize = 64;
/// Bounded terrain telemetry records.
pub const MAX_TERRAIN_TELEMETRY_RECORDS: usize = 2_048;

/// Global chunk coordinate. Meaningful only together with the owning map instance and
/// terrain fingerprint.
#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Ord, PartialOrd,
)]
pub struct TerrainChunkId {
    pub x: i16,
    pub y: i16,
}

/// Row-major 32x32 occupancy bits. Bit index is `local_y * 32 + local_x`.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TerrainBits(pub [u64; TERRAIN_WORDS_PER_CHUNK]);

/// Integer terrain brush. One coordinate unit is one half cell (4 world units).
#[derive(
    Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Ord, PartialOrd,
)]
pub struct TerrainBrush {
    pub center_half_cells_x: i16,
    pub center_half_cells_y: i16,
    pub radius_half_cells: u16,
}

/// Exact terrain generation identity. Live events, resets, and recovery are valid only for
/// one matching generation.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct TerrainGeneration {
    pub map_instance_id: MapInstanceId,
    pub match_id: MatchId,
    pub terrain_fingerprint: u64,
}

/// One applied authoritative brush at one revision.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TerrainDestructionEvent {
    pub generation: TerrainGeneration,
    pub revision: u64,
    pub source_attack_id: AttackId,
    pub source_delivery_index: u8,
    pub brush: TerrainBrush,
    /// Sorted unique affected chunks, at most four.
    pub affected_chunks: Vec<TerrainChunkId>,
    pub erased_cells: u16,
}

/// Request for one generation's complete current occupancy.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainRecoveryRequest {
    pub generation: TerrainGeneration,
}

/// One chunk's complete current occupancy.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainChunkSnapshot {
    pub chunk_id: TerrainChunkId,
    pub occupancy: TerrainBits,
}

/// Complete current terrain state at one revision.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct TerrainRecoverySnapshot {
    pub generation: TerrainGeneration,
    pub revision: u64,
    /// The exact sorted allocated chunk set, at most 221 entries.
    pub chunks: Vec<TerrainChunkSnapshot>,
}

/// Announces the exact generation transition on match restart.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainResetEvent {
    pub previous_generation: TerrainGeneration,
    pub next_generation: TerrainGeneration,
}
