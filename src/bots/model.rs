use crate::{
    combat::{TeamId, WeaponPhase},
    map::{DamageableTargetIdentity, MapDynamicGeneration, MapInstanceId},
    matchplay::{HotZoneStatus, MatchId},
    protocol::NetworkEntityId,
};
use bevy::prelude::*;
use std::collections::VecDeque;

pub(super) const MAX_OBSERVATION_HISTORY: usize = 16;
pub(super) const MAX_CONTACTS: usize = brawler_routing::MAX_PARTICIPANTS;

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BotFighterView {
    pub network_id: NetworkEntityId,
    pub team: TeamId,
    pub position: Vec2,
    pub velocity: Vec2,
    pub current_health: u16,
    pub maximum_health: u16,
    pub active: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BotObjectKind {
    OilBarrel,
    TreasureChest,
    HeistSafe { defending_team: TeamId },
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BotObjectView {
    pub identity: DamageableTargetIdentity,
    pub kind: BotObjectKind,
    pub position: Vec2,
    pub current_health: u16,
    pub maximum_health: u16,
    pub live: bool,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BotPickupView {
    pub position: Vec2,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum BotModeView {
    Wipeout {
        scores: [u16; 2],
    },
    HotZone {
        center: Vec2,
        radius: f32,
        status: HotZoneStatus,
        progress: [u16; 2],
    },
    Heist,
}

#[derive(Clone, Debug, PartialEq)]
pub(super) struct BotObservation {
    pub tick: u64,
    pub match_id: MatchId,
    pub map_instance_id: MapInstanceId,
    pub map_generation: MapDynamicGeneration,
    pub map_revision: u64,
    pub match_active: bool,
    pub self_view: BotFighterView,
    pub allies: Vec<BotFighterView>,
    pub visible_enemies: Vec<BotFighterView>,
    pub objects: Vec<BotObjectView>,
    pub pickups: Vec<BotPickupView>,
    pub mode: BotModeView,
    pub weapon_phase: WeaponPhase,
    pub weapon_ammo: u8,
    pub ability_ready: bool,
    pub ultimate_kind: crate::builds::UltimateKind,
    pub ultimate_range: f32,
    pub weapon_range: f32,
    pub projectile_speed: f32,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct BotContact {
    pub network_id: NetworkEntityId,
    pub position: Vec2,
    pub velocity: Vec2,
    pub observed_at_tick: u64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum BotTactic {
    Pressure,
    Retreat,
    CollectPickup,
    Contest,
    DefendSafe,
    AttackSafe,
    BreakObject,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) enum BotRole {
    #[default]
    Pressure,
    Objective,
    Defender,
}

#[derive(Clone, Debug)]
pub(super) struct BotState {
    pub contacts: Vec<BotContact>,
    pub tactic: BotTactic,
    pub tactic_until_tick: u64,
    pub aim_error_radians: f32,
    pub aim_error_until_tick: u64,
    pub route: Vec<Vec2>,
    pub route_cursor: usize,
    pub route_goal: Option<Vec2>,
    pub route_search: Option<super::navigation::BotRouteSearch>,
    pub route_retry_at_tick: u64,
    pub last_position: Option<Vec2>,
    pub stationary_ticks: u64,
    pub perimeter_recovery: bool,
    pub stuck_escape_until_tick: u64,
    pub stuck_escape_axis: Vec2,
    pub last_ultimate_tick: Option<u64>,
}

impl Default for BotState {
    fn default() -> Self {
        Self {
            contacts: Vec::new(),
            tactic: BotTactic::Pressure,
            tactic_until_tick: 0,
            aim_error_radians: 0.0,
            aim_error_until_tick: 0,
            route: Vec::new(),
            route_cursor: 0,
            route_goal: None,
            route_search: None,
            route_retry_at_tick: 0,
            last_position: None,
            stationary_ticks: 0,
            perimeter_recovery: false,
            stuck_escape_until_tick: 0,
            stuck_escape_axis: Vec2::ZERO,
            last_ultimate_tick: None,
        }
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub(super) struct BotIntent {
    pub move_goal: Option<Vec2>,
    pub aim_target: Option<(Vec2, Vec2)>,
    pub fire: bool,
    pub dash: bool,
}

#[derive(Component, Clone, Debug)]
pub(crate) struct PracticeBotController {
    pub(super) seed: u64,
    pub(super) life_generation: u64,
    pub(super) was_active: bool,
    pub(super) history: VecDeque<BotObservation>,
    pub(super) state: BotState,
    pub(super) last_decision_tick: Option<u64>,
}

impl PracticeBotController {
    pub(crate) fn new(seed: u64) -> Self {
        Self {
            seed,
            life_generation: 0,
            was_active: false,
            history: VecDeque::with_capacity(MAX_OBSERVATION_HISTORY),
            state: BotState::default(),
            last_decision_tick: None,
        }
    }

    pub(super) fn push_observation(&mut self, observation: BotObservation) {
        if self.history.len() == MAX_OBSERVATION_HISTORY {
            self.history.pop_front();
        }
        self.history.push_back(observation);
    }

    pub(super) fn delayed_observation(&self, tick: u64, delay: u64) -> Option<&BotObservation> {
        let required = tick.checked_sub(delay)?;
        self.history
            .iter()
            .rev()
            .find(|entry| entry.tick <= required)
    }

    pub(super) fn reset_life(&mut self) {
        self.life_generation = self.life_generation.saturating_add(1);
        self.reset_context();
    }

    pub(super) fn reset_context(&mut self) {
        self.history.clear();
        self.state = BotState::default();
        self.last_decision_tick = None;
    }
}
