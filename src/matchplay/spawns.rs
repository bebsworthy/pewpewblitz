//! Mode-neutral team assignment and deterministic spawn selection shared by both roles.

use super::MatchId;
use crate::{combat::TeamId, map::SpawnPointId, protocol::PlayerId};
use bevy::prelude::Vec2;

/// Most teams one match composition may declare: Last Stand's six solo fighters (BRL-5ss9).
pub const MAX_MATCH_TEAMS: usize = 6;

/// Per-team participant counts indexed by `TeamId`, bounded by [`MAX_MATCH_TEAMS`].
pub type TeamCounts = [u8; MAX_MATCH_TEAMS];

/// Deterministically assign one joining fighter to the least-filled of the first `team_count`
/// teams with free capacity; ties go to the lowest team index.
#[must_use]
pub fn assigned_team(team_counts: TeamCounts, team_count: u8, maximum: u8) -> Option<TeamId> {
    team_counts
        .iter()
        .take(usize::from(team_count))
        .enumerate()
        .filter(|(_, count)| **count < maximum)
        .min_by_key(|(index, count)| (**count, *index))
        .and_then(|(index, _)| u8::try_from(index).ok())
        .map(TeamId)
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct SpawnCandidate {
    pub id: SpawnPointId,
    pub position: Vec2,
    pub facing: f32,
}

/// Reduce a routed match identity to the u64 seed accepted by the spawn-selection algorithm.
///
/// The common case remains the exact low 64 bits. Routed IDs may use the full u128 space, so the
/// checked conversion explicitly falls back to a deterministic fold instead of silently
/// truncating or rejecting an otherwise valid match worker identity.
fn match_seed_component(match_id: MatchId) -> u64 {
    if let Ok(value) = u64::try_from(match_id.0) {
        value
    } else {
        let low =
            u64::try_from(match_id.0 & u128::from(u64::MAX)).expect("masked match ID must fit u64");
        let high = u64::try_from(match_id.0 >> 64).expect("shifted match ID must fit u64");
        low ^ high.rotate_left(32)
    }
}

/// Select a deterministic, clearance-aware spawn for one activation or respawn.
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
        let seed = match_seed_component(match_id)
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
