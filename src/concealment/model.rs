use bevy::prelude::*;
use serde::{Deserialize, Serialize};

pub const ATTACK_REVEAL_TICKS: u64 = 90;
pub const DAMAGE_REVEAL_TICKS: u64 = 120;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct TerrainConcealmentMembership {
    pub map_instance_id: crate::map::MapInstanceId,
    pub placement_id: crate::map::MapPlacementId,
}

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ConcealmentRevealDeadlines {
    pub attack_until_tick: u64,
    pub damage_until_tick: u64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct TeamRevealDeadline {
    pub team: crate::combat::TeamId,
    pub expires_at_tick: u64,
}

#[derive(Component, Serialize, Deserialize, Clone, Debug, Default, PartialEq, Eq)]
pub struct ConcealmentPresentationState {
    pub inside_concealing_terrain: bool,
    pub self_cloaked_until_tick: u64,
    pub revealed_until_tick: u64,
    pub forced_reveals: Vec<TeamRevealDeadline>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct ForcedRevealSource {
    pub revealing_team: crate::combat::TeamId,
    pub source_network_id: crate::protocol::NetworkEntityId,
    pub source_generation: u64,
    pub applied_at_tick: u64,
    pub expires_at_tick: u64,
}

#[derive(Component, Clone, Debug, Default, PartialEq, Eq)]
pub struct ForcedRevealSources(pub Vec<ForcedRevealSource>);

pub const MAX_FORCED_REVEAL_SOURCES: usize = 32;

impl ForcedRevealSources {
    pub fn apply(&mut self, source: ForcedRevealSource) -> bool {
        if let Some(existing) = self.0.iter_mut().find(|existing| {
            existing.revealing_team == source.revealing_team
                && existing.source_network_id == source.source_network_id
                && existing.source_generation == source.source_generation
        }) {
            let refreshed = source.expires_at_tick > existing.expires_at_tick;
            existing.expires_at_tick = existing.expires_at_tick.max(source.expires_at_tick);
            return refreshed;
        }
        if self.0.len() >= MAX_FORCED_REVEAL_SOURCES {
            return false;
        }
        self.0.push(source);
        self.0.sort();
        true
    }

    #[must_use]
    pub fn active_for_team(&self, team: crate::combat::TeamId, tick: u64) -> bool {
        self.0
            .iter()
            .any(|source| source.revealing_team == team && tick < source.expires_at_tick)
    }
}

#[must_use]
pub fn reveal_lock_active(tick: u64, deadlines: ConcealmentRevealDeadlines) -> bool {
    tick < deadlines.attack_until_tick || tick < deadlines.damage_until_tick
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ObserverRelation {
    SelfOrAlly,
    Enemy,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ConcealmentSources {
    None,
    Terrain,
    SelfCloak,
    TerrainAndSelfCloak,
}

impl ConcealmentSources {
    const fn self_cloak(self) -> bool {
        matches!(self, Self::SelfCloak | Self::TerrainAndSelfCloak)
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ObserverVisibilityInput {
    pub relation: ObserverRelation,
    pub observer_alive: bool,
    pub concealment: ConcealmentSources,
    pub forced_revealed: bool,
    pub subject_reveal_locked: bool,
    pub distance_squared: f32,
    pub reveal_radius: f32,
}

#[must_use]
pub fn observer_can_see(input: ObserverVisibilityInput) -> bool {
    input.relation == ObserverRelation::SelfOrAlly
        || input.concealment == ConcealmentSources::None
        || (input.observer_alive && input.forced_revealed)
        || (input.observer_alive && input.subject_reveal_locked)
        || (!input.concealment.self_cloak()
            && input.observer_alive
            && input.distance_squared.is_finite()
            && input.reveal_radius.is_finite()
            && input.distance_squared <= input.reveal_radius * input.reveal_radius)
}
