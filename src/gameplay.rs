//! Minimal authoritative gameplay composition.

use crate::timing::{SIMULATION_TICK, SimulationTick};
use bevy::prelude::*;

/// Ordering contract for gameplay systems that will be added in later milestones.
#[derive(SystemSet, Debug, Clone, PartialEq, Eq, Hash)]
pub enum GameplaySet {
    Lifecycle,
    Input,
    Simulation,
    Fire,
    Finalize,
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
                    GameplaySet::Lifecycle,
                    GameplaySet::Input,
                    GameplaySet::Simulation,
                    GameplaySet::Fire,
                    GameplaySet::Finalize,
                )
                    .chain(),
            );
        app.configure_sets(
            FixedPostUpdate,
            (
                crate::combat::CombatSet::ProjectileSweep,
                crate::combat::CombatSet::Damage,
                crate::combat::CombatSet::Lifecycle,
                crate::combat::CombatSet::TelemetryAndCues,
                crate::combat::CombatSet::Finalize,
            )
                .chain(),
        )
        .add_systems(
            FixedUpdate,
            ApplyDeferred
                .after(GameplaySet::Lifecycle)
                .before(GameplaySet::Input),
        )
        .add_systems(
            FixedPostUpdate,
            advance_simulation_tick.in_set(crate::combat::CombatSet::Finalize),
        );
    }
}

pub(crate) fn advance_simulation_tick(mut tick: ResMut<SimulationTick>) {
    tick.0 = tick.0.saturating_add(1);
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::time::TimeUpdateStrategy;

    #[derive(Resource, Default)]
    struct SetTrace(Vec<GameplaySet>);

    fn record_input_set(mut trace: ResMut<SetTrace>) {
        trace.0.push(GameplaySet::Input);
    }

    fn record_simulation_set(mut trace: ResMut<SetTrace>) {
        trace.0.push(GameplaySet::Simulation);
    }

    fn record_finalize_set(mut trace: ResMut<SetTrace>) {
        trace.0.push(GameplaySet::Finalize);
    }

    #[test]
    fn fixed_tick_advances_through_bevy_fixed_loop_and_set_chain() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, GameplayPlugin))
            .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
            .init_resource::<SetTrace>()
            .add_systems(
                FixedUpdate,
                (
                    record_input_set.in_set(GameplaySet::Input),
                    record_simulation_set.in_set(GameplaySet::Simulation),
                    record_finalize_set.in_set(GameplaySet::Finalize),
                ),
            );
        app.update();
        // Bevy's real clock records its first instant before producing a delta.
        app.update();

        assert_eq!(app.world().resource::<SimulationTick>().0, 1);
        assert_eq!(
            app.world().resource::<SetTrace>().0,
            vec![
                GameplaySet::Input,
                GameplaySet::Simulation,
                GameplaySet::Finalize,
            ]
        );
        assert_eq!(
            app.world().resource::<Time<Fixed>>().elapsed(),
            SIMULATION_TICK
        );
        assert_eq!(SIMULATION_TICK, SimulationTick::duration());
    }
}
