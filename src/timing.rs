//! Simulation timing shared by every application configuration.

use bevy::prelude::*;
use core::time::Duration;

/// The authoritative simulation frequency for v1.
pub const SIMULATION_TICK_HZ: u64 = 60;

/// Floating-point simulation frequency for presentation and authoring-unit conversion.
#[allow(
    clippy::cast_precision_loss,
    reason = "the authoritative frequency is the exact small integer 60"
)]
pub const SIMULATION_TICK_HZ_F64: f64 = SIMULATION_TICK_HZ as f64;

/// The one source from which application configurations derive their fixed step.
pub const SIMULATION_TICK: Duration = Duration::from_nanos(1_000_000_000 / SIMULATION_TICK_HZ);

/// Convert a whole-second authored duration to authoritative simulation ticks.
#[must_use]
pub const fn simulation_ticks_from_seconds(seconds: u64) -> Option<u64> {
    seconds.checked_mul(SIMULATION_TICK_HZ)
}

/// Convert authoritative ticks to completed whole seconds for compact presentation.
#[must_use]
pub const fn simulation_whole_seconds(ticks: u64) -> u64 {
    ticks / SIMULATION_TICK_HZ
}

/// Convert authoritative ticks to fractional seconds for authoring and presentation.
#[must_use]
pub fn simulation_seconds_f64(ticks: u32) -> f64 {
    f64::from(ticks) / SIMULATION_TICK_HZ_F64
}

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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shared_conversions_follow_the_authoritative_frequency() {
        assert_eq!(simulation_ticks_from_seconds(0), Some(0));
        assert_eq!(simulation_ticks_from_seconds(3), Some(180));
        assert_eq!(simulation_whole_seconds(179), 2);
        assert!((simulation_seconds_f64(1) - 1.0 / SIMULATION_TICK_HZ_F64).abs() < f64::EPSILON);
        assert_eq!(
            simulation_ticks_from_seconds(u64::MAX / SIMULATION_TICK_HZ + 1),
            None
        );
    }
}
