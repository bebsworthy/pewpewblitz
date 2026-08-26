use std::f32::consts::PI;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BotProfile {
    pub reaction_ticks: u64,
    pub contact_memory_ticks: u64,
    pub tactic_cadence_ticks: u64,
    pub tactic_commitment_ticks: u64,
    pub aim_hold_ticks: u64,
    pub maximum_aim_error_radians: f32,
    pub preferred_range_fraction: f32,
    pub retreat_health_fraction: f32,
    pub stuck_ticks: u64,
    pub total_search_expansions_per_tick: usize,
    pub maximum_search_expansions: usize,
    pub maximum_route_points: usize,
}

impl Default for BotProfile {
    fn default() -> Self {
        Self {
            reaction_ticks: 9,
            contact_memory_ticks: 120,
            tactic_cadence_ticks: 6,
            tactic_commitment_ticks: 18,
            aim_hold_ticks: 24,
            maximum_aim_error_radians: 5.0 * PI / 180.0,
            preferred_range_fraction: 0.70,
            retreat_health_fraction: 0.35,
            stuck_ticks: 30,
            total_search_expansions_per_tick: 512,
            maximum_search_expansions: 16_384,
            maximum_route_points: 1_024,
        }
    }
}

impl BotProfile {
    pub(super) fn validate(self) -> bool {
        self.reaction_ticks > 0
            && self.contact_memory_ticks >= self.reaction_ticks
            && self.tactic_cadence_ticks > 0
            && self.tactic_commitment_ticks >= self.tactic_cadence_ticks
            && self.aim_hold_ticks > 0
            && self.maximum_aim_error_radians.is_finite()
            && (0.0..=PI / 4.0).contains(&self.maximum_aim_error_radians)
            && (0.1..1.0).contains(&self.preferred_range_fraction)
            && (0.0..1.0).contains(&self.retreat_health_fraction)
            && self.stuck_ticks > 0
            && self.total_search_expansions_per_tick > 0
            && self.maximum_search_expansions > 0
            && self.maximum_route_points > 1
    }

    pub(super) fn search_budget_per_bot(self, active_bots: usize) -> usize {
        self.total_search_expansions_per_tick
            .checked_div(active_bots.max(1))
            .unwrap_or(1)
            .max(1)
    }
}
