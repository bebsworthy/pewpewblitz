//! Server-authoritative mirrored Heist objective rules and replicated state.

use crate::{
    combat::TeamId,
    map::{MapDynamicGeneration, ModeAnchorId},
};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub const HEIST_RULES_REVISION: u16 = 1;
pub const HEIST_SAFE_COUNT: usize = 2;

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeistRules {
    pub safe_maximum_health: u16,
    pub critical_health_percent: u8,
}

impl Default for HeistRules {
    fn default() -> Self {
        Self {
            safe_maximum_health: 2_000,
            critical_health_percent: 25,
        }
    }
}

impl HeistRules {
    pub fn validate(self) -> Result<Self, &'static str> {
        if self.safe_maximum_health == 0 {
            return Err("Heist safe maximum health must be nonzero");
        }
        if !(1..=99).contains(&self.critical_health_percent) {
            return Err("Heist critical-health percentage must be within 1..=99");
        }
        Ok(self)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeistCompletion {
    SafeDestroyed { destroyed_teams: [bool; 2] },
    Timeout { comparison: HeistHealthComparison },
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub enum HeistHealthComparison {
    Team0Greater,
    Team1Greater,
    Equal,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeistSafeIdentity {
    pub anchor_id: ModeAnchorId,
    pub defending_team: TeamId,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeistState {
    pub match_id: super::MatchId,
    pub rules_revision: u16,
    pub generation: MapDynamicGeneration,
    pub safes: [HeistSafeIdentity; HEIST_SAFE_COUNT],
    pub completion: Option<HeistCompletion>,
}

#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeistSafe {
    pub match_id: super::MatchId,
    pub anchor_id: ModeAnchorId,
    pub defending_team: TeamId,
    pub generation: MapDynamicGeneration,
}

#[cfg(feature = "server")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PendingModeObjectiveDamage {
    pub target: crate::map::DamageableTargetIdentity,
    pub source: crate::combat::AttackSource,
    pub requested_damage: u16,
    pub delivery_index: u8,
    pub bundle_index: u8,
    pub effect_index: u8,
}

#[cfg(feature = "server")]
#[derive(Resource, Default)]
pub struct PendingModeObjectiveDamages(pub Vec<PendingModeObjectiveDamage>);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct HeistSummary {
    pub final_health: [u16; 2],
    pub maximum_health: [u16; 2],
    pub completion: Option<HeistCompletion>,
}

#[must_use]
pub fn destroyed_safe_result(health: [u16; 2]) -> Option<super::MatchResult> {
    match (health[0] == 0, health[1] == 0) {
        (false, false) => None,
        (true, false) => Some(super::MatchResult::TeamVictory { team: TeamId(1) }),
        (false, true) => Some(super::MatchResult::TeamVictory { team: TeamId(0) }),
        (true, true) => Some(super::MatchResult::Draw),
    }
}

#[must_use]
pub fn remaining_health_comparison(health: [u16; 2], maximum: [u16; 2]) -> std::cmp::Ordering {
    u64::from(health[0])
        .saturating_mul(u64::from(maximum[1]))
        .cmp(&u64::from(health[1]).saturating_mul(u64::from(maximum[0])))
}

#[must_use]
pub fn timeout_result(health: [u16; 2], maximum: [u16; 2]) -> super::MatchResult {
    match remaining_health_comparison(health, maximum) {
        std::cmp::Ordering::Greater => super::MatchResult::TeamVictory { team: TeamId(0) },
        std::cmp::Ordering::Less => super::MatchResult::TeamVictory { team: TeamId(1) },
        std::cmp::Ordering::Equal => super::MatchResult::Draw,
    }
}

#[cfg(feature = "server")]
mod authority {
    use super::*;
    use crate::{
        combat::{CombatDamageSet, CurrentHealth},
        map::{
            DamageableLifeState, DamageableMaximumHealth, DamageableTargetClass,
            DamageableTargetIdentity, HEIST_MODE_DEFINITION, MapDynamicState, MapInstanceMember,
            MapRoot, ResolvedMap,
        },
        matchplay::{
            MatchPhase, MatchRestartSet, MatchRoot, MatchSet, MatchState, ModeOutcomeCause,
            ModeRuleOutcome, PendingModeRuleOutcome,
        },
        timing::SimulationTick,
    };
    use avian2d::prelude::{Collider, Position, RigidBody, Rotation};
    use lightyear::prelude::{NetworkTarget, Replicate};

    pub struct HeistModePlugin;

    impl Plugin for HeistModePlugin {
        fn build(&self, app: &mut App) {
            let setup = app
                .world()
                .get_resource::<crate::matchplay::MatchModeSetup>()
                .copied()
                .unwrap_or_default();
            assert_eq!(
                setup.mode_definition_id, HEIST_MODE_DEFINITION,
                "HeistModePlugin requires a Heist match mode setup"
            );
            app.init_resource::<PendingModeObjectiveDamages>()
                .add_systems(
                    Startup,
                    install_heist_safes
                        .after(crate::matchplay::server::initialize_match_root)
                        .after(crate::map::MapStartupSet::Instantiate),
                )
                .add_systems(
                    FixedUpdate,
                    resolve_heist_deadline.in_set(MatchSet::DeadlineRules),
                )
                .add_systems(
                    FixedUpdate,
                    reset_heist_safes.in_set(MatchRestartSet::ModeReset),
                )
                .add_systems(
                    FixedPostUpdate,
                    apply_objective_damage.in_set(CombatDamageSet::ModeObjectives),
                )
                .add_systems(
                    FixedPostUpdate,
                    evaluate_destroyed_safes.in_set(MatchSet::ModeRules),
                );
        }
    }

    fn install_heist_safes(
        mut commands: Commands,
        rules: Res<HeistRules>,
        map: Res<ResolvedMap>,
        map_roots: Query<&MapDynamicState, With<MapRoot>>,
        roots: Query<(Entity, &MatchState), With<MatchRoot>>,
    ) {
        let Ok((root, match_state)) = roots.single() else {
            return;
        };
        let Ok(dynamic) = map_roots.single() else {
            return;
        };
        assert_eq!(match_state.mode_definition_id, HEIST_MODE_DEFINITION);
        assert_eq!(map.heist_safes.len(), HEIST_SAFE_COUNT);
        let generation = dynamic.generation_id();
        let mut identities = [HeistSafeIdentity {
            anchor_id: ModeAnchorId(0),
            defending_team: TeamId(u8::MAX),
        }; HEIST_SAFE_COUNT];
        for safe in &map.heist_safes {
            let team = usize::from(safe.defending_team.0);
            identities[team] = HeistSafeIdentity {
                anchor_id: safe.anchor_id,
                defending_team: safe.defending_team,
            };
            commands.spawn((
                HeistSafe {
                    match_id: match_state.match_id,
                    anchor_id: safe.anchor_id,
                    defending_team: safe.defending_team,
                    generation,
                },
                DamageableTargetIdentity::HeistSafe {
                    match_id: match_state.match_id,
                    anchor_id: safe.anchor_id,
                    defending_team: safe.defending_team,
                },
                DamageableTargetClass::ModeObjective,
                DamageableMaximumHealth(rules.safe_maximum_health),
                CurrentHealth(rules.safe_maximum_health),
                DamageableLifeState::Live,
                safe.defending_team,
                MapInstanceMember {
                    map_instance_id: generation.map_instance_id,
                    placement_id: safe.placement_id,
                },
                RigidBody::Static,
                Collider::rectangle(safe.half_extents.x * 2.0, safe.half_extents.y * 2.0),
                crate::movement::map_collision_layers(),
                Position(safe.center),
                Rotation::radians(f32::from(safe.quarter_turns) * std::f32::consts::FRAC_PI_2),
                Transform::from_translation(safe.center.extend(0.0)),
                Replicate::to_clients(NetworkTarget::All),
            ));
        }
        commands.entity(root).insert(HeistState {
            match_id: match_state.match_id,
            rules_revision: HEIST_RULES_REVISION,
            generation,
            safes: identities,
            completion: None,
        });
    }

    #[allow(clippy::needless_pass_by_value)]
    fn apply_objective_damage(
        mut commands: Commands,
        tick: Res<SimulationTick>,
        mut ids: ResMut<crate::combat::NextCombatIds>,
        mut facts: ResMut<crate::map::WorldTargetDamageFacts>,
        roots: Query<&MatchState, With<MatchRoot>>,
        mut pending: ResMut<PendingModeObjectiveDamages>,
        mut safes: Query<(
            Entity,
            &HeistSafe,
            &DamageableTargetIdentity,
            &mut CurrentHealth,
            &mut DamageableLifeState,
        )>,
        sources: Query<
            (
                &crate::protocol::NetworkEntityId,
                &TeamId,
                &crate::matchplay::MatchParticipant,
                Option<&crate::combat::Defeated>,
            ),
            With<crate::protocol::Fighter>,
        >,
    ) {
        let Ok(state) = roots.single() else {
            pending.0.clear();
            return;
        };
        if !matches!(state.phase, MatchPhase::Active { .. }) {
            pending.0.clear();
            return;
        }
        pending.0.sort_by_key(|request| {
            (
                request.source.attack_id.0,
                request.delivery_index,
                request.bundle_index,
                request.effect_index,
            )
        });
        for request in pending.0.drain(..).take(64) {
            if request.requested_damage == 0 {
                continue;
            }
            let valid_source = sources
                .iter()
                .any(|(network_id, team, participant, defeated)| {
                    *network_id == request.source.owner_network_entity_id
                        && *team == request.source.team_id
                        && participant.match_id == state.match_id
                        && defeated.is_none()
                });
            if !valid_source {
                continue;
            }
            let Some((entity, _, _, mut health, mut life)) =
                safes.iter_mut().find(|(_, safe, identity, _, life)| {
                    **identity == request.target
                        && safe.match_id == state.match_id
                        && request.source.team_id != safe.defending_team
                        && matches!(**life, DamageableLifeState::Live)
                })
            else {
                continue;
            };
            if facts.0.len() >= crate::map::MAX_WORLD_TARGET_FACTS {
                continue;
            }
            let Some(event_id) = ids.allocate_event() else {
                continue;
            };
            let health_before = health.0;
            health.0 = health.0.saturating_sub(request.requested_damage);
            let applied_damage = health_before - health.0;
            facts.0.push(crate::map::WorldTargetDamageFact {
                event_id,
                tick: tick.0,
                attack_id: request.source.attack_id,
                source: request.source,
                target: request.target,
                requested_damage: request.requested_damage,
                applied_damage,
                health_after: health.0,
                terminal: (health.0 == 0)
                    .then_some(crate::map::WorldTargetTerminalFact::ModeObjectiveDestroyed),
            });
            if health.0 == 0 {
                *life = DamageableLifeState::TerminalCommitted;
                commands.entity(entity).remove::<Collider>();
            }
        }
    }

    fn safe_health(
        state: &MatchState,
        safes: &Query<(&HeistSafe, &CurrentHealth, &DamageableMaximumHealth)>,
    ) -> Option<([u16; 2], [u16; 2])> {
        let mut health = [0; 2];
        let mut maximum = [0; 2];
        let mut seen = [false; 2];
        for (safe, current, max) in safes {
            if safe.match_id != state.match_id || safe.defending_team.0 > 1 {
                continue;
            }
            let index = usize::from(safe.defending_team.0);
            if seen[index] {
                return None;
            }
            seen[index] = true;
            health[index] = current.0;
            maximum[index] = max.0;
        }
        seen.into_iter()
            .all(|value| value)
            .then_some((health, maximum))
    }

    fn evaluate_destroyed_safes(
        tick: Res<SimulationTick>,
        mut roots: Query<(&MatchState, &mut HeistState), With<MatchRoot>>,
        safes: Query<(&HeistSafe, &CurrentHealth, &DamageableMaximumHealth)>,
        mut outcomes: ResMut<ModeRuleOutcome>,
        mut diagnostics: ResMut<crate::matchplay::MatchOutcomeDiagnostics>,
    ) {
        let Ok((state, mut heist)) = roots.single_mut() else {
            return;
        };
        if !matches!(state.phase, MatchPhase::Active { .. }) || heist.completion.is_some() {
            return;
        }
        let Some((health, _)) = safe_health(state, &safes) else {
            return;
        };
        let Some(result) = destroyed_safe_result(health) else {
            return;
        };
        heist.completion = Some(HeistCompletion::SafeDestroyed {
            destroyed_teams: [health[0] == 0, health[1] == 0],
        });
        crate::matchplay::offer_mode_rule_outcome(
            &mut outcomes,
            &mut diagnostics,
            PendingModeRuleOutcome {
                match_id: state.match_id,
                evaluated_tick: tick.0,
                cause: ModeOutcomeCause::Threshold,
                result,
            },
        );
    }

    fn resolve_heist_deadline(
        tick: Res<SimulationTick>,
        mut roots: Query<(&MatchState, &mut HeistState), With<MatchRoot>>,
        safes: Query<(&HeistSafe, &CurrentHealth, &DamageableMaximumHealth)>,
        mut outcomes: ResMut<ModeRuleOutcome>,
        mut diagnostics: ResMut<crate::matchplay::MatchOutcomeDiagnostics>,
    ) {
        let Ok((state, mut heist)) = roots.single_mut() else {
            return;
        };
        let MatchPhase::Active { ends_at_tick } = state.phase else {
            return;
        };
        if tick.0 < ends_at_tick || heist.completion.is_some() {
            return;
        }
        let Some((health, maximum)) = safe_health(state, &safes) else {
            return;
        };
        let ordering = remaining_health_comparison(health, maximum);
        heist.completion = Some(HeistCompletion::Timeout {
            comparison: match ordering {
                std::cmp::Ordering::Greater => HeistHealthComparison::Team0Greater,
                std::cmp::Ordering::Less => HeistHealthComparison::Team1Greater,
                std::cmp::Ordering::Equal => HeistHealthComparison::Equal,
            },
        });
        crate::matchplay::offer_mode_rule_outcome(
            &mut outcomes,
            &mut diagnostics,
            PendingModeRuleOutcome {
                match_id: state.match_id,
                evaluated_tick: tick.0,
                cause: ModeOutcomeCause::Timeout,
                result: timeout_result(health, maximum),
            },
        );
    }

    fn reset_heist_safes(
        mut commands: Commands,
        rules: Res<HeistRules>,
        restart: Option<Res<crate::matchplay::PendingMatchRestart>>,
        mut roots: Query<&mut HeistState, With<MatchRoot>>,
        mut safes: Query<(
            Entity,
            &mut HeistSafe,
            &mut DamageableTargetIdentity,
            &mut DamageableMaximumHealth,
            &mut CurrentHealth,
            &mut DamageableLifeState,
        )>,
    ) {
        let Some(slot) = restart.as_ref().and_then(|restart| restart.slot()) else {
            return;
        };
        let Ok(mut heist) = roots.single_mut() else {
            return;
        };
        heist.match_id = slot.next_id;
        heist.completion = None;
        for (entity, mut safe, mut identity, mut maximum, mut health, mut life) in &mut safes {
            safe.match_id = slot.next_id;
            *identity = DamageableTargetIdentity::HeistSafe {
                match_id: slot.next_id,
                anchor_id: safe.anchor_id,
                defending_team: safe.defending_team,
            };
            maximum.0 = rules.safe_maximum_health;
            health.0 = rules.safe_maximum_health;
            *life = DamageableLifeState::Live;
            commands
                .entity(entity)
                .insert(Collider::rectangle(96.0, 64.0));
        }
    }
}

#[cfg(feature = "server")]
pub use authority::HeistModePlugin;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rules_reject_zero_health_and_invalid_critical_thresholds() {
        assert!(HeistRules::default().validate().is_ok());
        assert!(
            HeistRules {
                safe_maximum_health: 0,
                ..HeistRules::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            HeistRules {
                critical_health_percent: 0,
                ..HeistRules::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            HeistRules {
                critical_health_percent: 100,
                ..HeistRules::default()
            }
            .validate()
            .is_err()
        );
    }

    #[test]
    fn destruction_resolves_single_safe_victory_and_same_tick_draw() {
        assert_eq!(destroyed_safe_result([1, 1]), None);
        assert_eq!(
            destroyed_safe_result([0, 1]),
            Some(super::super::MatchResult::TeamVictory { team: TeamId(1) })
        );
        assert_eq!(
            destroyed_safe_result([1, 0]),
            Some(super::super::MatchResult::TeamVictory { team: TeamId(0) })
        );
        assert_eq!(
            destroyed_safe_result([0, 0]),
            Some(super::super::MatchResult::Draw)
        );
    }

    #[test]
    fn timeout_compares_exact_remaining_health_fractions() {
        assert_eq!(
            timeout_result([1, 2], [2, 4]),
            super::super::MatchResult::Draw
        );
        assert_eq!(
            timeout_result([2, 2], [3, 4]),
            super::super::MatchResult::TeamVictory { team: TeamId(0) }
        );
        assert_eq!(
            timeout_result([1, 3], [2, 4]),
            super::super::MatchResult::TeamVictory { team: TeamId(1) }
        );
    }
}
