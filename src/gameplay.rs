//! Minimal authoritative gameplay composition.

use crate::timing::{SIMULATION_TICK, SimulationTick};
use bevy::prelude::*;

/// Ordering contract for gameplay systems that will be added in later milestones.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameplaySet {
    Input,
    Simulation,
    Presentation,
}

/// Installs the non-presentation gameplay and fixed-tick foundation.
pub struct GameplayPlugin;

impl Plugin for GameplayPlugin {
    fn build(&self, app: &mut App) {
        app.insert_resource(Time::<Fixed>::from_duration(SIMULATION_TICK))
            .init_resource::<SimulationTick>()
            .configure_sets(
                FixedUpdate,
                (
                    GameplaySet::Input,
                    GameplaySet::Simulation,
                    GameplaySet::Presentation,
                )
                    .chain(),
            )
            .add_systems(
                FixedUpdate,
                advance_simulation_tick.in_set(GameplaySet::Simulation),
            );
    }
}

fn advance_simulation_tick(mut tick: ResMut<SimulationTick>) {
    tick.0 = tick.0.saturating_add(1);
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn fixed_tick_advances_in_declared_schedule() {
        let mut app = App::new();
        app.add_plugins(GameplayPlugin);
        app.update();
        app.world_mut().run_schedule(FixedUpdate);

        assert_eq!(app.world().resource::<SimulationTick>().0, 1);
        assert_eq!(SIMULATION_TICK, SimulationTick::duration());
    }
}
