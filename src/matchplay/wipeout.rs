use super::{MatchId, MatchPhase, MatchResult};
use crate::{combat::TeamId, map::SpawnPointId, protocol::PlayerId};
use bevy::prelude::*;

pub const WIPEOUT_RULES_REVISION: u16 = 1;

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct WipeoutRules {
    pub team_count: u8,
    pub minimum_participants_per_team: u8,
    pub maximum_participants_per_team: u8,
    pub target_score: u16,
    pub countdown_ticks: u64,
    pub active_limit_ticks: u64,
    pub respawn_delay_ticks: u64,
    pub spawn_protection_ticks: u64,
    pub completed_input_lock_ticks: u64,
    pub movement_displacement_epsilon: f32,
    pub retained_match_summaries: usize,
    pub retained_match_records: usize,
}

impl Default for WipeoutRules {
    fn default() -> Self {
        Self {
            team_count: 2,
            minimum_participants_per_team: 1,
            maximum_participants_per_team: 2,
            target_score: 10,
            countdown_ticks: 180,
            active_limit_ticks: 10_800,
            respawn_delay_ticks: 180,
            spawn_protection_ticks: 90,
            completed_input_lock_ticks: 60,
            movement_displacement_epsilon: 0.25,
            retained_match_summaries: 32,
            retained_match_records: 2_048,
        }
    }
}

impl WipeoutRules {
    pub fn validate(self) -> Result<Self, &'static str> {
        if self.team_count != 2 {
            return Err("Wipeout requires exactly two teams");
        }
        if self.minimum_participants_per_team == 0
            || self.minimum_participants_per_team > self.maximum_participants_per_team
            || self.maximum_participants_per_team > 2
        {
            return Err("invalid Wipeout team capacity");
        }
        if self.target_score == 0
            || self.countdown_ticks == 0
            || self.active_limit_ticks == 0
            || self.respawn_delay_ticks == 0
            || self.spawn_protection_ticks == 0
            || self.completed_input_lock_ticks == 0
        {
            return Err("Wipeout deadlines and score target must be nonzero");
        }
        if self
            .countdown_ticks
            .checked_add(self.active_limit_ticks)
            .is_none()
            || self
                .respawn_delay_ticks
                .checked_add(self.spawn_protection_ticks)
                .is_none()
            || self
                .active_limit_ticks
                .checked_add(self.completed_input_lock_ticks)
                .is_none()
        {
            return Err("Wipeout deadline combination overflows");
        }
        if !self.movement_displacement_epsilon.is_finite()
            || self.movement_displacement_epsilon < 0.0
            || self.retained_match_summaries == 0
            || self.retained_match_records == 0
        {
            return Err("invalid Wipeout telemetry limits");
        }
        Ok(self)
    }
}

#[must_use]
pub fn assigned_team(team_counts: [u8; 2], maximum: u8) -> Option<TeamId> {
    let available = [team_counts[0] < maximum, team_counts[1] < maximum];
    match available {
        [false, false] => None,
        [true, false] => Some(TeamId(0)),
        [false, true] => Some(TeamId(1)),
        [true, true] => Some(TeamId(u8::from(team_counts[1] < team_counts[0]))),
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpawnCandidate {
    pub id: SpawnPointId,
    pub position: Vec2,
    pub facing: f32,
}

#[must_use]
pub fn select_spawn(
    mut candidates: Vec<SpawnCandidate>,
    living_fighters: &[(TeamId, Vec2)],
    team: TeamId,
    clearance: f32,
    match_id: MatchId,
    player_id: PlayerId,
    respawn_ordinal: u64,
) -> Option<SpawnCandidate> {
    candidates.retain(|candidate| candidate.position.is_finite() && candidate.facing.is_finite());
    candidates.sort_by_key(|candidate| candidate.id);
    if candidates.is_empty() {
        return None;
    }
    let clear: Vec<_> = candidates
        .iter()
        .copied()
        .filter(|candidate| {
            living_fighters.iter().all(|(_, position)| {
                candidate.position.distance_squared(*position) >= clearance.powi(2)
            })
        })
        .collect();
    let pool = if clear.is_empty() {
        &candidates
    } else {
        &clear
    };
    let hostiles: Vec<_> = living_fighters
        .iter()
        .filter(|(fighter_team, _)| *fighter_team != team)
        .map(|(_, position)| *position)
        .collect();
    if hostiles.is_empty() {
        let seed = match_id
            .0
            .wrapping_add(player_id.0)
            .wrapping_add(respawn_ordinal);
        let index = usize::try_from(seed % u64::try_from(pool.len()).ok()?).ok()?;
        return pool.get(index).copied();
    }
    pool.iter().copied().max_by(|left, right| {
        let left_distance = hostiles
            .iter()
            .map(|hostile| left.position.distance_squared(*hostile))
            .min_by(f32::total_cmp)
            .unwrap_or(f32::INFINITY);
        let right_distance = hostiles
            .iter()
            .map(|hostile| right.position.distance_squared(*hostile))
            .min_by(f32::total_cmp)
            .unwrap_or(f32::INFINITY);
        left_distance
            .total_cmp(&right_distance)
            .then_with(|| right.id.cmp(&left.id))
    })
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

#[must_use]
pub fn complete_phase(now: u64, lock_ticks: u64, result: MatchResult) -> Option<MatchPhase> {
    Some(MatchPhase::Completed {
        completed_at_tick: now,
        restart_unlocked_at_tick: now.checked_add(lock_ticks)?,
        result,
    })
}
