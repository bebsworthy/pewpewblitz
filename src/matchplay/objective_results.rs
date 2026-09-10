//! Atomic terminal objective values; presentation never reconstructs a result from live entities.
use super::{MatchId, MatchResult};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) enum ObjectiveTotals {
    HotZone { progress: [u16; 2], target: u16 },
    Heist { health: [u16; 2], maximum: [u16; 2] },
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub(crate) struct ObjectiveResult {
    pub match_id: MatchId,
    pub result: MatchResult,
    pub totals: ObjectiveTotals,
}

#[cfg(feature = "server")]
#[allow(
    clippy::type_complexity,
    reason = "Authoritative terminal snapshot queries"
)]
pub(super) fn publish(
    mut commands: Commands,
    roots: Query<
        (
            Entity,
            &super::MatchState,
            Option<&super::HotZoneState>,
            Option<&super::HeistState>,
            Option<&ObjectiveResult>,
        ),
        With<super::MatchRoot>,
    >,
    safes: Query<(
        &super::HeistSafe,
        &crate::combat::CurrentHealth,
        &crate::map::DamageableMaximumHealth,
    )>,
) {
    for (entity, state, zone, heist, previous) in &roots {
        let super::MatchPhase::Completed { result, .. } = state.phase else {
            if previous.is_some() {
                commands.entity(entity).remove::<ObjectiveResult>();
            }
            continue;
        };
        if previous.is_some_and(|p| p.match_id == state.match_id && p.result == result) {
            continue;
        }
        let totals = if let Some(zone) = zone.filter(|z| z.match_id == state.match_id) {
            ObjectiveTotals::HotZone {
                progress: zone.progress_ticks,
                target: zone.target_progress_ticks,
            }
        } else if let Some(heist) = heist.filter(|h| h.match_id == state.match_id) {
            let mut health = [0; 2];
            let mut maximum = [0; 2];
            let mut seen = [false; 2];
            for (safe, current, max) in &safes {
                if safe.match_id != state.match_id
                    || safe.generation != heist.generation
                    || safe.defending_team.0 > 1
                {
                    continue;
                }
                if !heist.safes.iter().any(|s| {
                    s.anchor_id == safe.anchor_id && s.defending_team == safe.defending_team
                }) {
                    continue;
                }
                let i = usize::from(safe.defending_team.0);
                if seen[i] {
                    maximum[i] = 0;
                    break;
                }
                seen[i] = true;
                health[i] = current.0;
                maximum[i] = max.0;
            }
            if !seen.into_iter().all(|s| s) || maximum.contains(&0) {
                continue;
            }
            ObjectiveTotals::Heist { health, maximum }
        } else {
            continue;
        };
        commands.entity(entity).insert(ObjectiveResult {
            match_id: state.match_id,
            result,
            totals,
        });
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;
    use crate::matchplay::{HotZoneState, HotZoneStatus, MatchPhase, MatchRoot, MatchState};
    #[test]
    fn captures_final_objective_once_and_clears_it_on_restart() {
        let mut app = App::new();
        app.add_systems(Update, publish);
        let root = app
            .world_mut()
            .spawn((
                MatchRoot,
                MatchState {
                    match_id: MatchId(7),
                    mode_definition_id: crate::map::HOT_ZONE_MODE_DEFINITION,
                    phase: MatchPhase::Completed {
                        completed_at_tick: 10,
                        restart_unlocked_at_tick: 20,
                        result: MatchResult::Draw,
                    },
                    rules_revision: 2,
                },
                HotZoneState {
                    match_id: MatchId(8),
                    zone_anchor_id: crate::map::ModeAnchorId(1),
                    occupants: [0; 2],
                    status: HotZoneStatus::Empty,
                    progress_ticks: [27, 27],
                    target_progress_ticks: 100,
                    next_evaluation_tick: 11,
                },
            ))
            .id();
        app.update();
        assert!(app.world().get::<ObjectiveResult>(root).is_none());
        app.world_mut()
            .get_mut::<HotZoneState>(root)
            .unwrap()
            .match_id = MatchId(7);
        app.update();
        let final_value = *app.world().get::<ObjectiveResult>(root).unwrap();
        assert_eq!(
            final_value.totals,
            ObjectiveTotals::HotZone {
                progress: [27, 27],
                target: 100
            }
        );
        app.world_mut()
            .get_mut::<HotZoneState>(root)
            .unwrap()
            .progress_ticks = [0; 2];
        app.update();
        assert_eq!(
            *app.world().get::<ObjectiveResult>(root).unwrap(),
            final_value
        );
        app.world_mut().get_mut::<MatchState>(root).unwrap().phase =
            MatchPhase::Active { ends_at_tick: 100 };
        app.update();
        assert!(app.world().get::<ObjectiveResult>(root).is_none());
    }
    #[test]
    fn heist_snapshot_requires_both_matching_idols() {
        use crate::{
            combat::{CurrentHealth, TeamId},
            map::{DamageableMaximumHealth, MapDynamicGeneration, MapInstanceId, ModeAnchorId},
            matchplay::{HeistSafe, HeistSafeIdentity, HeistState},
        };
        let mut app = App::new();
        app.add_systems(Update, publish);
        let generation = MapDynamicGeneration {
            map_instance_id: MapInstanceId(9),
            generation: 1,
        };
        let identities = [0, 1].map(|t| HeistSafeIdentity {
            anchor_id: ModeAnchorId(t + 1),
            defending_team: TeamId(u8::try_from(t).unwrap()),
        });
        let root = app
            .world_mut()
            .spawn((
                MatchRoot,
                MatchState {
                    match_id: MatchId(7),
                    mode_definition_id: crate::map::HEIST_MODE_DEFINITION,
                    phase: MatchPhase::Completed {
                        completed_at_tick: 10,
                        restart_unlocked_at_tick: 20,
                        result: MatchResult::Draw,
                    },
                    rules_revision: 2,
                },
                HeistState {
                    match_id: MatchId(7),
                    rules_revision: 2,
                    generation,
                    critical_health_percent: 25,
                    safes: identities,
                    completion: None,
                },
            ))
            .id();
        for (i, identity) in identities.into_iter().enumerate() {
            app.update();
            assert!(app.world().get::<ObjectiveResult>(root).is_none());
            app.world_mut().spawn((
                HeistSafe {
                    match_id: MatchId(7),
                    anchor_id: identity.anchor_id,
                    defending_team: identity.defending_team,
                    generation,
                },
                CurrentHealth(if i == 0 { 731 } else { 0 }),
                DamageableMaximumHealth(2000),
            ));
        }
        app.update();
        assert_eq!(
            app.world().get::<ObjectiveResult>(root).unwrap().totals,
            ObjectiveTotals::Heist {
                health: [731, 0],
                maximum: [2000; 2]
            }
        );
    }
}
