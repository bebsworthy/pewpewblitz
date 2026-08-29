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

/// Cross-domain ordering contract for one authoritative fixed-post transaction.
///
/// Domain plugins retain their focused internal sets and place those sets into these neutral
/// phases. This keeps extension plugins independent of concrete combat, map, match, and
/// presentation implementations while leaving their transaction order visible in one place.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub(crate) enum AuthoritativePhase {
    Delivery,
    Effects,
    Environment,
    Objectives,
    Visibility,
    Publication,
    Finalization,
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
                AuthoritativePhase::Delivery,
                AuthoritativePhase::Effects,
                AuthoritativePhase::Environment,
                AuthoritativePhase::Objectives,
                AuthoritativePhase::Visibility,
                AuthoritativePhase::Publication,
                AuthoritativePhase::Finalization,
            )
                .chain(),
        )
        .configure_sets(
            FixedPostUpdate,
            (
                crate::combat::CombatSet::ProjectileSweep.in_set(AuthoritativePhase::Delivery),
                crate::combat::CombatSet::Damage.in_set(AuthoritativePhase::Effects),
                crate::combat::CombatSet::Lifecycle.in_set(AuthoritativePhase::Objectives),
                crate::combat::CombatSet::TelemetryAndCues.in_set(AuthoritativePhase::Publication),
                crate::combat::CombatSet::Finalize.in_set(AuthoritativePhase::Finalization),
            ),
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

    #[derive(Resource, Default)]
    struct PhaseTrace(Vec<AuthoritativePhase>);

    fn record_input_set(mut trace: ResMut<SetTrace>) {
        trace.0.push(GameplaySet::Input);
    }

    fn record_simulation_set(mut trace: ResMut<SetTrace>) {
        trace.0.push(GameplaySet::Simulation);
    }

    fn record_finalize_set(mut trace: ResMut<SetTrace>) {
        trace.0.push(GameplaySet::Finalize);
    }

    fn record_phase(phase: AuthoritativePhase) -> impl Fn(ResMut<PhaseTrace>) {
        move |mut trace| trace.0.push(phase)
    }

    #[test]
    fn fixed_tick_advances_through_bevy_fixed_loop_and_set_chain() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, GameplayPlugin))
            .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
            .init_resource::<SetTrace>()
            .init_resource::<PhaseTrace>()
            .add_systems(
                FixedUpdate,
                (
                    record_input_set.in_set(GameplaySet::Input),
                    record_simulation_set.in_set(GameplaySet::Simulation),
                    record_finalize_set.in_set(GameplaySet::Finalize),
                ),
            )
            .add_systems(
                FixedPostUpdate,
                (
                    record_phase(AuthoritativePhase::Delivery).in_set(AuthoritativePhase::Delivery),
                    record_phase(AuthoritativePhase::Effects).in_set(AuthoritativePhase::Effects),
                    record_phase(AuthoritativePhase::Environment)
                        .in_set(AuthoritativePhase::Environment),
                    record_phase(AuthoritativePhase::Objectives)
                        .in_set(AuthoritativePhase::Objectives),
                    record_phase(AuthoritativePhase::Visibility)
                        .in_set(AuthoritativePhase::Visibility),
                    record_phase(AuthoritativePhase::Publication)
                        .in_set(AuthoritativePhase::Publication),
                    record_phase(AuthoritativePhase::Finalization)
                        .in_set(AuthoritativePhase::Finalization),
                ),
            );
        crate::test_app::reject_owned_schedule_ambiguities(&mut app, FixedUpdate);
        crate::test_app::reject_owned_schedule_ambiguities(&mut app, FixedPostUpdate);
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
        assert_eq!(
            app.world().resource::<PhaseTrace>().0,
            vec![
                AuthoritativePhase::Delivery,
                AuthoritativePhase::Effects,
                AuthoritativePhase::Environment,
                AuthoritativePhase::Objectives,
                AuthoritativePhase::Visibility,
                AuthoritativePhase::Publication,
                AuthoritativePhase::Finalization,
            ]
        );
    }
}
