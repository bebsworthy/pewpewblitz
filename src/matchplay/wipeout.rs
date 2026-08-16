//! Server-authoritative Wipeout scoring rules, state, and mode plugin.

use super::MatchResult;
use crate::combat::TeamId;
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Composed revision of the validated common lifecycle plus Wipeout rules for this state shape.
pub const WIPEOUT_RULES_REVISION: u16 = 2;

/// Wipeout-specific scoring rules layered on the common `MatchLifecycleRules`.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct WipeoutRules {
    pub target_score: u16,
}

impl Default for WipeoutRules {
    fn default() -> Self {
        Self { target_score: 10 }
    }
}

impl WipeoutRules {
    pub fn validate(self) -> Result<Self, &'static str> {
        if self.target_score == 0 {
            return Err("Wipeout score target must be nonzero");
        }
        Ok(self)
    }
}

/// Durable replicated Wipeout scoring state, present on the match root only for Wipeout.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct WipeoutState {
    pub team_scores: [u16; 2],
    pub target_score: u16,
}

#[must_use]
pub fn score_result(scores: [u16; 2], target: u16) -> Option<MatchResult> {
    if scores[0] < target && scores[1] < target {
        return None;
    }
    Some(match scores[0].cmp(&scores[1]) {
        std::cmp::Ordering::Greater => MatchResult::TeamVictory { team: TeamId(0) },
        std::cmp::Ordering::Less => MatchResult::TeamVictory { team: TeamId(1) },
        std::cmp::Ordering::Equal => MatchResult::Draw,
    })
}

#[must_use]
pub fn timeout_result(scores: [u16; 2]) -> MatchResult {
    match scores[0].cmp(&scores[1]) {
        std::cmp::Ordering::Greater => MatchResult::TeamVictory { team: TeamId(0) },
        std::cmp::Ordering::Less => MatchResult::TeamVictory { team: TeamId(1) },
        std::cmp::Ordering::Equal => MatchResult::Draw,
    }
}

#[cfg(feature = "server")]
mod rules {
    #![allow(clippy::wildcard_imports)]
    use super::*;
    use crate::matchplay::{
        MatchOutcomeDiagnostics, MatchPhase, MatchRestartSet, MatchRoot, MatchSet, MatchState,
        ModeOutcomeCause, ModeRuleOutcome, PendingModeRuleOutcome, offer_mode_rule_outcome,
        prepare_mode_rule_facts,
    };
    use crate::{
        combat::{CombatOutcomeFacts, CombatOutcomeKind},
        map::WIPEOUT_MODE_DEFINITION,
        matchplay::MatchModeSetup,
        protocol::{Fighter, NetworkEntityId},
        timing::SimulationTick,
    };
    use std::collections::BTreeSet;

    /// Bounded per-match scored-event identity set so duplicate combat facts cannot score twice.
    #[derive(Resource, Default, Debug)]
    pub(crate) struct ScoredCombatEvents {
        pub(crate) match_id: Option<crate::matchplay::MatchId>,
        pub(crate) ids: BTreeSet<u64>,
    }

    pub struct WipeoutModePlugin;

    impl Plugin for WipeoutModePlugin {
        fn build(&self, app: &mut App) {
            let setup = app
                .world()
                .get_resource::<MatchModeSetup>()
                .copied()
                .unwrap_or_default();
            assert_eq!(
                setup.mode_definition_id, WIPEOUT_MODE_DEFINITION,
                "WipeoutModePlugin requires a Wipeout match mode setup"
            );
            app.init_resource::<ScoredCombatEvents>()
                .add_systems(
                    Startup,
                    initialize_wipeout_state.after(super::super::server::initialize_match_root),
                )
                .add_systems(
                    FixedUpdate,
                    resolve_wipeout_deadline.in_set(MatchSet::DeadlineRules),
                )
                .add_systems(
                    FixedUpdate,
                    reset_wipeout_state_on_restart.in_set(MatchRestartSet::ModeReset),
                )
                .add_systems(
                    FixedPostUpdate,
                    resolve_wipeout_scoring
                        .in_set(MatchSet::ModeRules)
                        .after(prepare_mode_rule_facts),
                );
        }
    }

    #[allow(clippy::needless_pass_by_value)]
    fn initialize_wipeout_state(
        mut commands: Commands,
        rules: Res<WipeoutRules>,
        roots: Query<Entity, With<MatchRoot>>,
    ) {
        let Ok(root) = roots.single() else {
            return;
        };
        commands.entity(root).insert(WipeoutState {
            team_scores: [0, 0],
            target_score: rules.target_score,
        });
    }

    /// Deadline rule: at or after `ends_at_tick`, recognize an already-present threshold state
    /// (recovered or injected) and otherwise resolve the timeout comparison.
    #[allow(clippy::needless_pass_by_value)]
    fn resolve_wipeout_deadline(
        tick: Res<SimulationTick>,
        roots: Query<(&MatchState, &WipeoutState), With<MatchRoot>>,
        mut outcomes: ResMut<ModeRuleOutcome>,
        mut diagnostics: ResMut<MatchOutcomeDiagnostics>,
    ) {
        let Ok((state, wipeout)) = roots.single() else {
            return;
        };
        let MatchPhase::Active { ends_at_tick } = state.phase else {
            return;
        };
        if tick.0 < ends_at_tick {
            return;
        }
        let (cause, result) = match score_result(wipeout.team_scores, wipeout.target_score) {
            Some(result) => (ModeOutcomeCause::Threshold, result),
            None => (
                ModeOutcomeCause::Timeout,
                timeout_result(wipeout.team_scores),
            ),
        };
        offer_mode_rule_outcome(
            &mut outcomes,
            &mut diagnostics,
            PendingModeRuleOutcome {
                match_id: state.match_id,
                evaluated_tick: tick.0,
                cause,
                result,
            },
        );
    }

    fn reset_wipeout_state_on_restart(
        restart: Option<Res<crate::matchplay::server::PendingMatchRestart>>,
        mut roots: Query<&mut WipeoutState, With<MatchRoot>>,
        mut scored_events: ResMut<ScoredCombatEvents>,
    ) {
        if restart.is_none_or(|restart| restart.slot().is_none()) {
            return;
        }
        let Ok(mut wipeout) = roots.single_mut() else {
            return;
        };
        wipeout.team_scores = [0, 0];
        scored_events.ids.clear();
        scored_events.match_id = None;
    }

    /// Post-damage defeat scoring. Reads the current-tick fact buffer without draining it;
    /// the common finalizer clears the buffer after every registered reader has run.
    #[allow(
        clippy::needless_pass_by_value,
        clippy::too_many_arguments,
        clippy::type_complexity
    )]
    fn resolve_wipeout_scoring(
        tick: Res<SimulationTick>,
        mut roots: Query<(&MatchState, &mut WipeoutState), With<MatchRoot>>,
        facts: Res<CombatOutcomeFacts>,
        mut scored_events: ResMut<ScoredCombatEvents>,
        mut diagnostics: ResMut<MatchOutcomeDiagnostics>,
        mut outcomes: ResMut<ModeRuleOutcome>,
        participants: Query<(&NetworkEntityId, &TeamId, &MatchParticipantView), With<Fighter>>,
    ) {
        let Ok((state, mut wipeout)) = roots.single_mut() else {
            return;
        };
        if !matches!(state.phase, MatchPhase::Active { .. }) {
            return;
        }
        if scored_events.match_id != Some(state.match_id) {
            scored_events.match_id = Some(state.match_id);
            scored_events.ids.clear();
        }
        for fact in &facts.0 {
            if fact.tick != tick.0 {
                diagnostics.stale_tick = diagnostics.stale_tick.saturating_add(1);
                continue;
            }
            if !matches!(fact.kind, CombatOutcomeKind::Defeat) {
                continue;
            }
            if !scored_events.ids.insert(fact.event_id.0) {
                diagnostics.duplicate_event = diagnostics.duplicate_event.saturating_add(1);
                continue;
            }
            let Some((_, target_team, _)) =
                participants.iter().find(|(network_id, _, participant)| {
                    **network_id == fact.target_network_id && participant.match_id == state.match_id
                })
            else {
                diagnostics.unknown_or_wrong_match_target =
                    diagnostics.unknown_or_wrong_match_target.saturating_add(1);
                continue;
            };
            if *target_team != fact.target_team {
                diagnostics.unknown_or_wrong_match_target =
                    diagnostics.unknown_or_wrong_match_target.saturating_add(1);
                continue;
            }
            if let Some(source_team) = credited_defeat_team(fact, *target_team) {
                let index = usize::from(source_team.0);
                wipeout.team_scores[index] = wipeout.team_scores[index].saturating_add(1);
            } else if fact
                .source_network_id
                .is_some_and(|source| source != fact.target_network_id)
                && fact.source_team == Some(*target_team)
            {
                diagnostics.friendly_invalid_defeat =
                    diagnostics.friendly_invalid_defeat.saturating_add(1);
                warn!(
                    event_id = fact.event_id.0,
                    target = fact.target_network_id.0,
                    team = target_team.0,
                    "ignored same-team defeat outcome"
                );
            }
        }
        if let Some(result) = score_result(wipeout.team_scores, wipeout.target_score) {
            offer_mode_rule_outcome(
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
    }

    pub(crate) fn credited_defeat_team(
        fact: &crate::combat::CombatOutcomeFact,
        target_team: TeamId,
    ) -> Option<TeamId> {
        let source_team = fact.source_team?;
        (source_team.0 <= 1
            && source_team != target_team
            && fact.source_network_id != Some(fact.target_network_id))
        .then_some(source_team)
    }

    /// View alias avoiding a second import of the shared participant component.
    type MatchParticipantView = crate::matchplay::MatchParticipant;
}

#[cfg(feature = "server")]
pub use rules::WipeoutModePlugin;
#[cfg(all(feature = "server", test))]
pub(crate) use rules::credited_defeat_team;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn wipeout_rules_require_a_nonzero_target() {
        assert!(WipeoutRules::default().validate().is_ok());
        assert!(
            WipeoutRules { target_score: 0 }
                .validate()
                .is_err_and(|error| error.contains("nonzero"))
        );
    }
}
