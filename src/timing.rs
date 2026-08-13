//! Simulation timing shared by every application configuration.

use bevy::prelude::*;
use core::time::Duration;

/// The authoritative simulation frequency for v1.
pub const SIMULATION_TICK_HZ: u64 = 60;

/// The one source from which application configurations derive their fixed step.
pub const SIMULATION_TICK: Duration = Duration::from_nanos(1_000_000_000 / SIMULATION_TICK_HZ);

/// Monotonic simulation tick, incremented once per `FixedUpdate` execution.
#[derive(Resource, Debug, Default, PartialEq, Eq)]
pub struct SimulationTick(pub u64);

impl SimulationTick {
    /// Return the duration configured for one simulation tick.
    #[must_use]
    pub const fn duration() -> Duration {
        SIMULATION_TICK
    }
}
