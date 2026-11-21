//! Server-authoritative Turf Hold objective rules, state, and mode plugin.

use super::MatchId;
use crate::combat::TeamId;
use crate::map::ModeAnchorId;
#[cfg(feature = "server")]
use crate::map::{NormalizedArea, TURF_HOLD_MODE_DEFINITION};
use bevy::prelude::*;
use serde::{Deserialize, Serialize};

/// Composed revision of the validated common lifecycle plus Turf Hold rules.
pub const TURF_HOLD_RULES_REVISION: u16 = 2;

/// World-unit outward expansion defining near-zone combat for telemetry.
pub const TURF_HOLD_NEAR_COMBAT_EXPANSION: f32 = 240.0;

/// Turf Hold-specific objective rules layered on the common `MatchLifecycleRules`.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TurfHoldRules {
    pub target_progress_ticks: u16,
}

impl Default for TurfHoldRules {
    fn default() -> Self {
        // 1,800 ticks at the fixed 60 Hz rate is exactly 30 seconds of uncontested control.
        Self {
            target_progress_ticks: 1_800,
        }
    }
}

#[cfg(feature = "server")]
impl TurfHoldRules {
    pub fn validate_with(
        self,
        lifecycle: &super::MatchLifecycleRules,
    ) -> Result<Self, &'static str> {
        let ceiling = u64::from(u16::MAX).min(lifecycle.active_limit_ticks);
        if u64::from(self.target_progress_ticks) < 2
            || u64::from(self.target_progress_ticks) > ceiling
        {
            return Err("Turf Hold target progress must be between 2 and the active limit ceiling");
        }
        Ok(self)
    }
}

/// Current occupancy status of the zone, derived once per evaluated tick.
#[derive(Serialize, Deserialize, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum TurfHoldStatus {
    #[default]
    Empty,
    Controlled {
        team: TeamId,
    },
    Contested,
}

/// Durable replicated Turf Hold objective state, present on the match root only for Turf Hold.
#[derive(Component, Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TurfHoldState {
    pub match_id: MatchId,
    pub zone_anchor_id: ModeAnchorId,
    pub occupants: [u8; 2],
    pub status: TurfHoldStatus,
    pub progress_ticks: [u16; 2],
    pub target_progress_ticks: u16,
    pub next_evaluation_tick: u64,
}

impl TurfHoldState {
    /// A zero evaluation tick marks an uninitialized activation; the first eligible active
    /// tick initializes it before evaluating.
    pub const UNINITIALIZED_EVALUATION_TICK: u64 = 0;
}

/// Server-cached normalized objective area resolved from the installed map instance. The
/// replicated map snapshot remains the client presentation source.
#[cfg(feature = "server")]
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(crate) struct ResolvedObjectiveZone {
    pub(crate) anchor_id: ModeAnchorId,
    pub(crate) area: NormalizedArea,
}

/// Bounded saturating counters for objective evaluation faults. No allocation or raw record
/// is appended per fault; sampled evidence uses the common bounded match record deque.
#[cfg(feature = "server")]
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TurfHoldDiagnostics {
    pub(crate) duplicate_evaluations: u64,
    pub(crate) skipped_evaluation_ticks: u64,
    pub(crate) skipped_evaluation_distance: u64,
    pub(crate) ineligible_fighters: u64,
    pub(crate) occupant_count_saturations: u64,
}

/// Terminal Turf Hold telemetry snapshot attached to one common match summary.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct TurfHoldSummary {
    pub final_progress_ticks: [u16; 2],
    pub target_progress_ticks: u16,
    pub first_entry_tick_by_team: [Option<u64>; 2],
    pub first_progress_tick_by_team: [Option<u64>; 2],
    pub controlled_ticks_by_team: [u64; 2],
    pub occupant_fighter_ticks_by_team: [u64; 2],
    pub empty_ticks: u64,
    pub contested_ticks: u64,
    pub control_gained_transitions_by_team: [u32; 2],
    pub longest_consecutive_control_ticks_by_team: [u64; 2],
    pub near_zone_damage_suffered_by_team: [u64; 2],
    pub near_zone_defeats_suffered_by_team: [u32; 2],
}

/// Live Turf Hold telemetry accumulator, reset by the mode restart reset.
#[cfg(feature = "server")]
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq)]
pub(crate) struct TurfHoldTelemetry {
    pub(crate) match_id: Option<MatchId>,
    first_entry_tick_by_team: [Option<u64>; 2],
    first_progress_tick_by_team: [Option<u64>; 2],
    controlled_ticks_by_team: [u64; 2],
    occupant_fighter_ticks_by_team: [u64; 2],
    empty_ticks: u64,
    contested_ticks: u64,
    control_gained_transitions_by_team: [u32; 2],
    longest_consecutive_control_ticks_by_team: [u64; 2],
    current_consecutive_control_ticks: [u64; 2],
    previous_status: TurfHoldStatus,
    near_zone_damage_suffered_by_team: [u64; 2],
    near_zone_defeats_suffered_by_team: [u32; 2],
}

#[cfg(feature = "server")]
impl TurfHoldTelemetry {
    pub(crate) fn reset_for(&mut self, match_id: MatchId) {
        *self = Self {
            match_id: Some(match_id),
            ..Self::default()
        };
    }

    fn begin_match(&mut self, match_id: MatchId) {
        if self.match_id != Some(match_id) {
            self.reset_for(match_id);
        }
    }

    /// Record one evaluated tick from the completed occupancy snapshot.
    pub(crate) fn record_evaluation(
        &mut self,
        tick: u64,
        occupants: [u8; 2],
        status: TurfHoldStatus,
    ) {
        for team in [0_usize, 1] {
            if occupants[team] > 0 {
                self.first_entry_tick_by_team[team].get_or_insert(tick);
                self.occupant_fighter_ticks_by_team[team] = self.occupant_fighter_ticks_by_team
                    [team]
                    .saturating_add(u64::from(occupants[team]));
            }
            let controlling = controlled_by(status, team);
            if controlling {
                self.controlled_ticks_by_team[team] =
                    self.controlled_ticks_by_team[team].saturating_add(1);
                self.first_progress_tick_by_team[team].get_or_insert(tick);
                self.current_consecutive_control_ticks[team] =
                    self.current_consecutive_control_ticks[team].saturating_add(1);
                self.extend_longest_consecutive_control(team);
            } else {
                self.current_consecutive_control_ticks[team] = 0;
            }
            if controlling && !controlled_by(self.previous_status, team) {
                self.control_gained_transitions_by_team[team] =
                    self.control_gained_transitions_by_team[team].saturating_add(1);
            }
        }
        match status {
            TurfHoldStatus::Empty => self.empty_ticks = self.empty_ticks.saturating_add(1),
            TurfHoldStatus::Contested => {
                self.contested_ticks = self.contested_ticks.saturating_add(1);
            }
            TurfHoldStatus::Controlled { .. } => {}
        }
        self.previous_status = status;
    }

    fn extend_longest_consecutive_control(&mut self, team: usize) {
        self.longest_consecutive_control_ticks_by_team[team] = self
            .longest_consecutive_control_ticks_by_team[team]
            .max(self.current_consecutive_control_ticks[team]);
    }

    /// Record one applied hostile combat fact against a fighter target near the zone.
    pub(crate) fn record_near_zone_combat(
        &mut self,
        team: TeamId,
        damage: Option<u16>,
        inside_near_area: bool,
    ) {
        if team.0 > 1 || !inside_near_area {
            return;
        }
        let index = usize::from(team.0);
        if let Some(damage) = damage {
            self.near_zone_damage_suffered_by_team[index] =
                self.near_zone_damage_suffered_by_team[index].saturating_add(u64::from(damage));
        } else {
            self.near_zone_defeats_suffered_by_team[index] =
                self.near_zone_defeats_suffered_by_team[index].saturating_add(1);
        }
    }

    pub(crate) fn summary(&self, state: &TurfHoldState) -> TurfHoldSummary {
        TurfHoldSummary {
            final_progress_ticks: state.progress_ticks,
            target_progress_ticks: state.target_progress_ticks,
            first_entry_tick_by_team: self.first_entry_tick_by_team,
            first_progress_tick_by_team: self.first_progress_tick_by_team,
            controlled_ticks_by_team: self.controlled_ticks_by_team,
            occupant_fighter_ticks_by_team: self.occupant_fighter_ticks_by_team,
            empty_ticks: self.empty_ticks,
            contested_ticks: self.contested_ticks,
            control_gained_transitions_by_team: self.control_gained_transitions_by_team,
            longest_consecutive_control_ticks_by_team: self
                .longest_consecutive_control_ticks_by_team,
            near_zone_damage_suffered_by_team: self.near_zone_damage_suffered_by_team,
            near_zone_defeats_suffered_by_team: self.near_zone_defeats_suffered_by_team,
        }
    }
}

#[cfg(feature = "server")]
fn controlled_by(status: TurfHoldStatus, team: usize) -> bool {
    matches!(status, TurfHoldStatus::Controlled { team: held } if usize::from(held.0) == team)
}

#[cfg(feature = "server")]
mod rules {
    use super::{
        IntoScheduleConfigs, NormalizedArea, ResolvedObjectiveZone,
        TURF_HOLD_NEAR_COMBAT_EXPANSION, TeamId, TurfHoldDiagnostics, TurfHoldRules, TurfHoldState,
        TurfHoldStatus, TurfHoldTelemetry,
    };
    use crate::combat::CurrentHealth;
    use crate::map::TURF_HOLD_MODE_DEFINITION;
    use crate::matchplay::{
        ActiveCombatant, ConnectedMatchRoster, MatchOutcomeDiagnostics, MatchParticipant,
        MatchPhase, MatchRestartSet, MatchRoot, MatchSet, MatchState, ModeOutcomeCause,
        ModeRuleOutcome, PendingModeRuleOutcome, clear_combat_facts, offer_mode_rule_outcome,
        prepare_mode_rule_facts, record_match_telemetry,
    };
    use crate::protocol::{Fighter, NetworkEntityId};
    use crate::timing::SimulationTick;
    use avian2d::prelude::Position;
    use bevy::prelude::*;

    pub struct TurfHoldModePlugin;

    impl Plugin for TurfHoldModePlugin {
        fn build(&self, app: &mut App) {
            let setup = app
                .world()
                .get_resource::<crate::matchplay::MatchModeSetup>()
                .copied()
                .unwrap_or_default();
            assert_eq!(
                setup.mode_definition_id, TURF_HOLD_MODE_DEFINITION,
                "TurfHoldModePlugin requires a Turf Hold match mode setup"
            );
            app.init_resource::<TurfHoldTelemetry>()
                .init_resource::<TurfHoldDiagnostics>()
                .add_systems(
                    Startup,
                    initialize_turf_hold_state.after(super::super::server::initialize_match_root),
                )
                .add_systems(
                    FixedUpdate,
                    resolve_turf_hold_deadline.in_set(MatchSet::DeadlineRules),
                )
                .add_systems(
                    FixedUpdate,
                    reset_turf_hold_state_on_restart.in_set(MatchRestartSet::ModeReset),
                )
                .add_systems(
                    FixedPostUpdate,
                    evaluate_turf_hold_objective
                        .in_set(MatchSet::ModeRules)
                        .after(prepare_mode_rule_facts),
                )
                .add_systems(
                    FixedPostUpdate,
                    record_turf_hold_near_combat
                        .in_set(MatchSet::Outcomes)
                        .after(record_match_telemetry)
                        .before(clear_combat_facts),
                );
        }
    }

    /// Resolve the single objective anchor from the installed map and install the durable
    /// mode state on the match root. Startup fails on absent, duplicate, wrong-shaped, or
    /// mode-mismatched anchors.
    #[allow(clippy::needless_pass_by_value)]
    fn initialize_turf_hold_state(
        mut commands: Commands,
        rules: Res<TurfHoldRules>,
        zone: Res<ResolvedObjectiveZone>,
        roots: Query<(Entity, &MatchState), With<MatchRoot>>,
    ) {
        let Ok((root, state)) = roots.single() else {
            return;
        };
        assert_eq!(
            state.mode_definition_id, TURF_HOLD_MODE_DEFINITION,
            "Turf Hold mode requires a Turf Hold map"
        );
        let zone = *zone;
        let crate::map::MapShape::Circle { radius } = zone.area.shape else {
            panic!("validated Turf Hold objective must be circular");
        };
        commands.entity(root).insert((
            TurfHoldState {
                match_id: state.match_id,
                zone_anchor_id: zone.anchor_id,
                occupants: [0, 0],
                status: TurfHoldStatus::Empty,
                progress_ticks: [0, 0],
                target_progress_ticks: rules.target_progress_ticks,
                next_evaluation_tick: TurfHoldState::UNINITIALIZED_EVALUATION_TICK,
            },
            crate::matchplay::BotObjectiveView::ControlArea {
                center: zone.area.center,
                radius,
            },
        ));
        commands.insert_resource(ResolvedObjectiveZone {
            anchor_id: zone.anchor_id,
            area: zone.area,
        });
    }

    /// Deadline rule: at or after `ends_at_tick`, recognize an already-present threshold
    /// state (recovered or injected) and otherwise resolve the timeout comparison.
    #[allow(clippy::needless_pass_by_value)]
    fn resolve_turf_hold_deadline(
        tick: Res<SimulationTick>,
        roots: Query<(&MatchState, &TurfHoldState), With<MatchRoot>>,
        mut outcomes: ResMut<ModeRuleOutcome>,
        mut diagnostics: ResMut<MatchOutcomeDiagnostics>,
    ) {
        let Ok((state, turf_hold)) = roots.single() else {
            return;
        };
        let MatchPhase::Active { ends_at_tick } = state.phase else {
            return;
        };
        if tick.0 < ends_at_tick {
            return;
        }
        let (cause, result) =
            match threshold_result(turf_hold.progress_ticks, turf_hold.target_progress_ticks) {
                Some(result) => (ModeOutcomeCause::Threshold, result),
                None => (
                    ModeOutcomeCause::Timeout,
                    progress_comparison(turf_hold.progress_ticks),
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

    #[allow(clippy::needless_pass_by_value)]
    fn reset_turf_hold_state_on_restart(
        restart: Option<Res<crate::matchplay::server::PendingMatchRestart>>,
        mut roots: Query<&mut TurfHoldState, With<MatchRoot>>,
        mut telemetry: ResMut<TurfHoldTelemetry>,
    ) {
        let Some(slot) = restart.as_ref().and_then(|restart| restart.slot()) else {
            return;
        };
        let Ok(mut turf_hold) = roots.single_mut() else {
            return;
        };
        turf_hold.match_id = slot.next_id;
        turf_hold.occupants = [0, 0];
        turf_hold.status = TurfHoldStatus::Empty;
        turf_hold.progress_ticks = [0, 0];
        turf_hold.next_evaluation_tick = TurfHoldState::UNINITIALIZED_EVALUATION_TICK;
        telemetry.reset_for(slot.next_id);
    }

    /// One authoritative objective evaluation per eligible half-open active tick, after
    /// movement, physics, and same-tick damage are visible.
    #[allow(
        clippy::needless_pass_by_value,
        clippy::too_many_arguments,
        clippy::type_complexity
    )]
    fn evaluate_turf_hold_objective(
        tick: Res<SimulationTick>,
        zone: Option<Res<ResolvedObjectiveZone>>,
        roster: Res<ConnectedMatchRoster>,
        mut roots: Query<(&MatchState, &mut TurfHoldState), With<MatchRoot>>,
        mut telemetry: ResMut<TurfHoldTelemetry>,
        mut diagnostics: ResMut<TurfHoldDiagnostics>,
        mut outcomes: ResMut<ModeRuleOutcome>,
        mut outcome_diagnostics: ResMut<MatchOutcomeDiagnostics>,
        fighters: Query<
            (
                &NetworkEntityId,
                &MatchParticipant,
                &TeamId,
                &CurrentHealth,
                &Position,
            ),
            (
                With<Fighter>,
                With<ActiveCombatant>,
                Without<crate::combat::Defeated>,
            ),
        >,
    ) {
        let Some(zone) = zone else { return };
        let Ok((state, mut turf_hold)) = roots.single_mut() else {
            return;
        };
        if !matches!(state.phase, MatchPhase::Active { .. }) || state.match_id != turf_hold.match_id
        {
            return;
        }
        if turf_hold.next_evaluation_tick == TurfHoldState::UNINITIALIZED_EVALUATION_TICK {
            turf_hold.next_evaluation_tick = tick.0;
        }
        if tick.0 < turf_hold.next_evaluation_tick {
            diagnostics.duplicate_evaluations = diagnostics.duplicate_evaluations.saturating_add(1);
            return;
        }
        if tick.0 > turf_hold.next_evaluation_tick {
            diagnostics.skipped_evaluation_ticks =
                diagnostics.skipped_evaluation_ticks.saturating_add(1);
            diagnostics.skipped_evaluation_distance = diagnostics
                .skipped_evaluation_distance
                .saturating_add(tick.0 - turf_hold.next_evaluation_tick);
        }
        if roster.match_id != Some(state.match_id) {
            return;
        }
        telemetry.begin_match(state.match_id);

        let mut occupants = [0_u8; 2];
        for (network_id, participant, team, health, position) in &fighters {
            if participant.match_id != state.match_id
                || team.0 > 1
                || health.0 == 0
                || !position.0.is_finite()
                || !roster.connected_network_ids.contains(&network_id.0)
            {
                diagnostics.ineligible_fighters = diagnostics.ineligible_fighters.saturating_add(1);
                continue;
            }
            if zone.area.contains_point(position.0) {
                let index = usize::from(team.0);
                let (count, saturated) = occupants[index].overflowing_add(1);
                occupants[index] = count;
                if saturated {
                    diagnostics.occupant_count_saturations =
                        diagnostics.occupant_count_saturations.saturating_add(1);
                }
            }
        }
        let status = zone_status(occupants);
        if let TurfHoldStatus::Controlled { team } = status {
            let index = usize::from(team.0);
            turf_hold.progress_ticks[index] = turf_hold.progress_ticks[index]
                .saturating_add(1)
                .min(turf_hold.target_progress_ticks);
        }
        turf_hold.occupants = occupants;
        turf_hold.status = status;
        turf_hold.next_evaluation_tick = tick.0.saturating_add(1);
        telemetry.record_evaluation(tick.0, occupants, status);

        if let Some(result) =
            threshold_result(turf_hold.progress_ticks, turf_hold.target_progress_ticks)
        {
            offer_mode_rule_outcome(
                &mut outcomes,
                &mut outcome_diagnostics,
                PendingModeRuleOutcome {
                    match_id: state.match_id,
                    evaluated_tick: tick.0,
                    cause: ModeOutcomeCause::Threshold,
                    result,
                },
            );
        }
    }

    /// Near-zone combat telemetry: applied hostile damage/defeat facts whose fighter target
    /// stands inside the objective shape expanded outward, attributed to the suffering team.
    #[allow(clippy::needless_pass_by_value, clippy::type_complexity)]
    fn record_turf_hold_near_combat(
        tick: Res<SimulationTick>,
        zone: Option<Res<ResolvedObjectiveZone>>,
        roots: Query<(&MatchState, &TurfHoldState), With<MatchRoot>>,
        mut telemetry: ResMut<TurfHoldTelemetry>,
        facts: Res<crate::combat::CombatOutcomeFacts>,
        fighters: Query<(&NetworkEntityId, &MatchParticipant, &TeamId, &Position), With<Fighter>>,
    ) {
        let Some(zone) = zone else { return };
        let Ok((state, _)) = roots.single() else {
            return;
        };
        if !matches!(state.phase, MatchPhase::Active { .. }) {
            return;
        }
        telemetry.begin_match(state.match_id);
        let near_area = expand_area(zone.area, TURF_HOLD_NEAR_COMBAT_EXPANSION);
        for fact in &facts.0 {
            if fact.tick != tick.0 {
                continue;
            }
            let (damage, hostile) = match fact.kind {
                crate::combat::CombatOutcomeKind::Damage { amount } => (
                    Some(amount),
                    fact.source_team
                        .is_some_and(|source| source != fact.target_team),
                ),
                crate::combat::CombatOutcomeKind::Defeat => (
                    None,
                    fact.source_team
                        .is_some_and(|source| source != fact.target_team),
                ),
                crate::combat::CombatOutcomeKind::ProtectedContact
                | crate::combat::CombatOutcomeKind::Healing { .. }
                | crate::combat::CombatOutcomeKind::DeployableDestroyed => continue,
            };
            if !hostile
                || fact.target_kind != crate::combat::CombatTargetKind::Fighter
                || fact.source_network_id == Some(fact.target_network_id)
            {
                continue;
            }
            let Some((_, _, team, position)) =
                fighters.iter().find(|(network_id, participant, _, _)| {
                    **network_id == fact.target_network_id && participant.match_id == state.match_id
                })
            else {
                continue;
            };
            telemetry.record_near_zone_combat(*team, damage, near_area.contains_point(position.0));
        }
    }

    fn expand_area(area: NormalizedArea, expansion: f32) -> NormalizedArea {
        let shape = match area.shape {
            crate::map::MapShape::Circle { radius } => crate::map::MapShape::Circle {
                radius: radius + expansion,
            },
            crate::map::MapShape::Rectangle { half_extents } => crate::map::MapShape::Rectangle {
                half_extents: half_extents + Vec2::splat(expansion),
            },
        };
        NormalizedArea {
            center: area.center,
            shape,
        }
    }

    #[must_use]
    pub(crate) fn zone_status(occupants: [u8; 2]) -> TurfHoldStatus {
        match (occupants[0] > 0, occupants[1] > 0) {
            (false, false) => TurfHoldStatus::Empty,
            (true, false) => TurfHoldStatus::Controlled { team: TeamId(0) },
            (false, true) => TurfHoldStatus::Controlled { team: TeamId(1) },
            (true, true) => TurfHoldStatus::Contested,
        }
    }

    #[must_use]
    pub(crate) fn threshold_result(
        progress: [u16; 2],
        target: u16,
    ) -> Option<crate::matchplay::MatchResult> {
        if progress[0] < target && progress[1] < target {
            return None;
        }
        Some(progress_comparison(progress))
    }

    #[must_use]
    pub(crate) fn progress_comparison(progress: [u16; 2]) -> crate::matchplay::MatchResult {
        match progress[0].cmp(&progress[1]) {
            std::cmp::Ordering::Greater => {
                crate::matchplay::MatchResult::TeamVictory { team: TeamId(0) }
            }
            std::cmp::Ordering::Less => {
                crate::matchplay::MatchResult::TeamVictory { team: TeamId(1) }
            }
            std::cmp::Ordering::Equal => crate::matchplay::MatchResult::Draw,
        }
    }
}

#[cfg(feature = "server")]
pub use rules::TurfHoldModePlugin;

/// Match mode setup for the dedicated-server composition table.
#[must_use]
#[cfg(feature = "server")]
pub fn turf_hold_setup_for_composition() -> super::MatchModeSetup {
    super::MatchModeSetup {
        mode_definition_id: TURF_HOLD_MODE_DEFINITION,
        rules_revision: TURF_HOLD_RULES_REVISION,
    }
}

/// Turf Hold rules for one rules profile. Verification shortens deadlines and uses a 30-tick
/// progress target without changing semantics.
#[must_use]
pub fn turf_hold_rules_for_profile(profile: crate::config::MatchRulesProfile) -> TurfHoldRules {
    match profile {
        crate::config::MatchRulesProfile::Production => TurfHoldRules::default(),
        crate::config::MatchRulesProfile::ProcessVerification => TurfHoldRules {
            target_progress_ticks: 30,
        },
    }
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::rules::{progress_comparison, threshold_result, zone_status};
    use super::*;
    use crate::combat::TeamId;
    use crate::map::{MapShape, NormalizedArea};
    use bevy::prelude::Vec2;

    fn circle() -> NormalizedArea {
        NormalizedArea {
            center: Vec2::ZERO,
            shape: MapShape::Circle { radius: 160.0 },
        }
    }

    fn rectangle() -> NormalizedArea {
        NormalizedArea {
            center: Vec2::new(-100.0, 50.0),
            shape: MapShape::Rectangle {
                half_extents: Vec2::new(120.0, 80.0),
            },
        }
    }

    #[test]
    fn circle_containment_is_inclusive_on_exactly_representable_boundary_points() {
        let zone = circle();
        assert!(zone.contains_point(Vec2::ZERO));
        assert!(zone.contains_point(Vec2::new(120.0, 0.0)));
        assert!(zone.contains_point(Vec2::new(-160.0, 0.0)));
        assert!(zone.contains_point(Vec2::new(160.0, 0.0)));
        assert!(zone.contains_point(Vec2::new(0.0, 160.0)));
        assert!(zone.contains_point(Vec2::new(0.0, -160.0)));
        assert!(!zone.contains_point(Vec2::new(160.1, 0.0)));
        assert!(!zone.contains_point(Vec2::new(0.0, 160.1)));
        assert!(!zone.contains_point(Vec2::new(-200.0, 0.0)));
        assert!(!zone.contains_point(Vec2::new(f32::NAN, 0.0)));
        assert!(!zone.contains_point(Vec2::new(f32::INFINITY, 0.0)));
    }

    #[test]
    fn rectangle_containment_is_inclusive_and_translated() {
        let zone = rectangle();
        assert!(zone.contains_point(Vec2::new(-100.0, 50.0)));
        assert!(zone.contains_point(Vec2::new(20.0, 50.0)));
        assert!(zone.contains_point(Vec2::new(-220.0, 130.0)));
        assert!(zone.contains_point(Vec2::new(20.0, -30.0)));
        assert!(!zone.contains_point(Vec2::new(20.1, 50.0)));
        assert!(!zone.contains_point(Vec2::new(-100.0, 130.1)));
        assert!(!zone.contains_point(Vec2::new(50.0, 50.0)));
    }

    #[test]
    fn zone_status_covers_empty_controlled_and_contested() {
        assert_eq!(zone_status([0, 0]), TurfHoldStatus::Empty);
        assert_eq!(
            zone_status([2, 0]),
            TurfHoldStatus::Controlled { team: TeamId(0) }
        );
        assert_eq!(
            zone_status([0, 1]),
            TurfHoldStatus::Controlled { team: TeamId(1) }
        );
        assert_eq!(zone_status([1, 3]), TurfHoldStatus::Contested);
    }

    #[test]
    fn threshold_and_timeout_use_progress_comparison() {
        assert_eq!(threshold_result([29, 29], 30), None);
        assert_eq!(
            threshold_result([30, 29], 30),
            Some(crate::matchplay::MatchResult::TeamVictory { team: TeamId(0) })
        );
        assert_eq!(
            threshold_result([29, 30], 30),
            Some(crate::matchplay::MatchResult::TeamVictory { team: TeamId(1) })
        );
        assert_eq!(
            threshold_result([30, 30], 30),
            Some(crate::matchplay::MatchResult::Draw)
        );
        assert_eq!(
            progress_comparison([7, 9]),
            crate::matchplay::MatchResult::TeamVictory { team: TeamId(1) }
        );
        assert_eq!(
            progress_comparison([9, 9]),
            crate::matchplay::MatchResult::Draw
        );
    }

    #[test]
    fn telemetry_counts_ticks_transitions_and_near_zone_combat() {
        let mut telemetry = TurfHoldTelemetry::default();
        telemetry.begin_match(MatchId(1));
        telemetry.record_evaluation(10, [1, 0], TurfHoldStatus::Controlled { team: TeamId(0) });
        telemetry.record_evaluation(11, [2, 0], TurfHoldStatus::Controlled { team: TeamId(0) });
        telemetry.record_evaluation(12, [2, 2], TurfHoldStatus::Contested);
        telemetry.record_evaluation(13, [0, 0], TurfHoldStatus::Empty);
        telemetry.record_evaluation(14, [0, 1], TurfHoldStatus::Controlled { team: TeamId(1) });
        telemetry.record_near_zone_combat(TeamId(0), Some(25), true);
        telemetry.record_near_zone_combat(TeamId(0), None, true);
        telemetry.record_near_zone_combat(TeamId(0), Some(25), false);
        telemetry.record_near_zone_combat(TeamId(2), Some(25), true);
        let state = TurfHoldState {
            match_id: MatchId(1),
            zone_anchor_id: ModeAnchorId(1),
            occupants: [0, 1],
            status: TurfHoldStatus::Controlled { team: TeamId(1) },
            progress_ticks: [2, 1],
            target_progress_ticks: 30,
            next_evaluation_tick: 15,
        };
        let summary = telemetry.summary(&state);
        assert_eq!(summary.final_progress_ticks, [2, 1]);
        assert_eq!(summary.first_entry_tick_by_team, [Some(10), Some(12)]);
        assert_eq!(summary.first_progress_tick_by_team, [Some(10), Some(14)]);
        assert_eq!(summary.controlled_ticks_by_team, [2, 1]);
        assert_eq!(summary.occupant_fighter_ticks_by_team, [5, 3]);
        assert_eq!(summary.empty_ticks, 1);
        assert_eq!(summary.contested_ticks, 1);
        assert_eq!(summary.control_gained_transitions_by_team, [1, 1]);
        assert_eq!(summary.longest_consecutive_control_ticks_by_team, [2, 1]);
        assert_eq!(summary.near_zone_damage_suffered_by_team, [25, 0]);
        assert_eq!(summary.near_zone_defeats_suffered_by_team, [1, 0]);

        telemetry.reset_for(MatchId(2));
        assert_eq!(telemetry.summary(&state).controlled_ticks_by_team, [0, 0]);
    }
}
