//! Atomic terminal objective values; presentation never reconstructs a result from live entities.
use super::{MatchId, MatchResult};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub enum ObjectiveTotals {
    TurfHold { progress: [u16; 2], target: u16 },
    VaultBreak { health: [u16; 2], maximum: [u16; 2] },
    ShardRush { shards: [u8; 2], target: u8 },
}

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct ObjectiveResult {
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
            Option<&super::TurfHoldState>,
            Option<&super::VaultBreakState>,
            Option<&super::ShardRushState>,
            Option<&ObjectiveResult>,
        ),
        With<super::MatchRoot>,
    >,
    safes: Query<(
        &super::VaultBreakSafe,
        &crate::combat::CurrentHealth,
        &crate::map::DamageableMaximumHealth,
    )>,
) {
    for (entity, state, zone, vault_break, shard_rush, previous) in &roots {
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
            ObjectiveTotals::TurfHold {
                progress: zone.progress_ticks,
                target: zone.target_progress_ticks,
            }
        } else if let Some(shards) = shard_rush.filter(|s| s.match_id == state.match_id) {
            ObjectiveTotals::ShardRush {
                shards: shards.team_shards,
                target: shards.target_shards,
            }
        } else if let Some(vault_break) = vault_break.filter(|h| h.match_id == state.match_id) {
            let mut health = [0; 2];
            let mut maximum = [0; 2];
            let mut seen = [false; 2];
            for (safe, current, max) in &safes {
                if safe.match_id != state.match_id
                    || safe.generation != vault_break.generation
                    || safe.defending_team.0 > 1
                {
                    continue;
                }
                if !vault_break.safes.iter().any(|s| {
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
            ObjectiveTotals::VaultBreak { health, maximum }
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
    use crate::matchplay::{MatchPhase, MatchRoot, MatchState, TurfHoldState, TurfHoldStatus};
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
                    mode_definition_id: crate::map::TURF_HOLD_MODE_DEFINITION,
                    phase: MatchPhase::Completed {
                        completed_at_tick: 10,
                        restart_unlocked_at_tick: 20,
                        result: MatchResult::Draw,
                    },
                    rules_revision: 2,
                },
                TurfHoldState {
                    match_id: MatchId(8),
                    zone_anchor_id: crate::map::ModeAnchorId(1),
                    occupants: [0; 2],
                    status: TurfHoldStatus::Empty,
                    progress_ticks: [27, 27],
                    target_progress_ticks: 100,
                    next_evaluation_tick: 11,
                },
            ))
            .id();
        app.update();
        assert!(app.world().get::<ObjectiveResult>(root).is_none());
        app.world_mut()
            .get_mut::<TurfHoldState>(root)
            .unwrap()
            .match_id = MatchId(7);
        app.update();
        let final_value = *app.world().get::<ObjectiveResult>(root).unwrap();
        assert_eq!(
            final_value.totals,
            ObjectiveTotals::TurfHold {
                progress: [27, 27],
                target: 100
            }
        );
        app.world_mut()
            .get_mut::<TurfHoldState>(root)
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
    fn vault_break_snapshot_requires_both_matching_idols() {
        use crate::{
            combat::{CurrentHealth, TeamId},
            map::{DamageableMaximumHealth, MapDynamicGeneration, MapInstanceId, ModeAnchorId},
            matchplay::{VaultBreakSafe, VaultBreakSafeIdentity, VaultBreakState},
        };
        let mut app = App::new();
        app.add_systems(Update, publish);
        let generation = MapDynamicGeneration {
            map_instance_id: MapInstanceId(9),
            generation: 1,
        };
        let identities = [0, 1].map(|t| VaultBreakSafeIdentity {
            anchor_id: ModeAnchorId(t + 1),
            defending_team: TeamId(u8::try_from(t).unwrap()),
        });
        let root = app
            .world_mut()
            .spawn((
                MatchRoot,
                MatchState {
                    match_id: MatchId(7),
                    mode_definition_id: crate::map::VAULT_BREAK_MODE_DEFINITION,
                    phase: MatchPhase::Completed {
                        completed_at_tick: 10,
                        restart_unlocked_at_tick: 20,
                        result: MatchResult::Draw,
                    },
                    rules_revision: 2,
                },
                VaultBreakState {
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
                VaultBreakSafe {
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
            ObjectiveTotals::VaultBreak {
                health: [731, 0],
                maximum: [2000; 2]
            }
        );
    }
}
