#[cfg(feature = "server")]
use bevy::prelude::{FromWorld, Resource, World};
use serde::{Deserialize, Serialize};
use std::f32::consts::PI;

pub(crate) const BOT_CATALOG_SCHEMA_VERSION: u16 = 1;
const BOT_FINGERPRINT_FORMAT_VERSION: u16 = 1;
const MAX_BOT_CATALOG_BYTES: usize = 4 * 1024;
const MAX_REACTION_TICKS: u64 = 15;
const MAX_SEARCH_EXPANSIONS: u32 =
    crate::map::MAX_MAP_DIMENSION_CELLS as u32 * crate::map::MAX_MAP_DIMENSION_CELLS as u32;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct BotProfile {
    pub reaction_ticks: u64,
    pub contact_memory_ticks: u64,
    pub tactic_cadence_ticks: u64,
    pub tactic_commitment_ticks: u64,
    pub aim_hold_ticks: u64,
    pub maximum_aim_error_radians: f32,
    pub preferred_range_fraction: f32,
    pub retreat_health_fraction: f32,
    pub stuck_ticks: u64,
    pub total_search_expansions_per_tick: u32,
    pub maximum_search_expansions: u32,
    pub maximum_route_points: u16,
    pub restoration_health_fraction: f32,
    pub ultimate_retrigger_ticks: u64,
    pub retreat_step_min_world: f32,
    pub retreat_step_max_world: f32,
    pub retreat_dash_range_fraction: f32,
    pub hot_zone_dash_radius_fraction: f32,
    pub hot_zone_hold_radius_fraction: f32,
    pub standoff_arrival_fraction: f32,
    pub attack_safe_dash_range_fraction: f32,
    pub defend_anchor_range_fraction: f32,
    pub defend_anchor_max_distance: f32,
    pub pressure_far_range_fraction: f32,
    pub pressure_near_range_fraction: f32,
    pub pressure_retreat_fraction: f32,
    pub pressure_strafe_distance: f32,
    pub pressure_dash_range_fraction: f32,
    pub perimeter_recovery_trigger_cells: f32,
    pub perimeter_recovery_release_cells: f32,
    pub waypoint_reach_distance: f32,
    pub ally_separation_distance: f32,
    pub ally_separation_weight: f32,
    pub route_goal_change_distance: f32,
    pub damage_tile_cost_milli: u16,
}

impl BotProfile {
    #[cfg(feature = "server")]
    pub(crate) fn embedded() -> Result<Self, String> {
        Ok(BotCatalog::embedded()?.practice)
    }

    pub(super) fn validate(self) -> Result<(), String> {
        let finite_positive = [
            self.retreat_step_min_world,
            self.retreat_step_max_world,
            self.defend_anchor_max_distance,
            self.pressure_strafe_distance,
            self.perimeter_recovery_trigger_cells,
            self.perimeter_recovery_release_cells,
            self.waypoint_reach_distance,
            self.ally_separation_distance,
            self.route_goal_change_distance,
        ]
        .into_iter()
        .all(|value| value.is_finite() && value > 0.0);
        let finite_fractions = [
            self.maximum_aim_error_radians,
            self.preferred_range_fraction,
            self.retreat_health_fraction,
            self.restoration_health_fraction,
            self.retreat_dash_range_fraction,
            self.hot_zone_dash_radius_fraction,
            self.hot_zone_hold_radius_fraction,
            self.standoff_arrival_fraction,
            self.attack_safe_dash_range_fraction,
            self.defend_anchor_range_fraction,
            self.pressure_far_range_fraction,
            self.pressure_near_range_fraction,
            self.pressure_retreat_fraction,
            self.pressure_dash_range_fraction,
            self.ally_separation_weight,
        ]
        .into_iter()
        .all(f32::is_finite);
        if self.reaction_ticks == 0
            || self.reaction_ticks > MAX_REACTION_TICKS
            || self.contact_memory_ticks < self.reaction_ticks
            || self.tactic_cadence_ticks == 0
            || self.tactic_commitment_ticks < self.tactic_cadence_ticks
            || self.aim_hold_ticks == 0
            || self.ultimate_retrigger_ticks == 0
            || self.stuck_ticks == 0
            || !finite_fractions
            || !(0.0..=PI / 4.0).contains(&self.maximum_aim_error_radians)
            || !(0.1..1.0).contains(&self.preferred_range_fraction)
            || !(0.0..1.0).contains(&self.retreat_health_fraction)
            || !(0.0..=1.0).contains(&self.restoration_health_fraction)
            || !(0.0..=1.0).contains(&self.retreat_dash_range_fraction)
            || self.hot_zone_dash_radius_fraction < 1.0
            || !(0.0..=1.0).contains(&self.hot_zone_hold_radius_fraction)
            || self.standoff_arrival_fraction < 1.0
            || self.attack_safe_dash_range_fraction < 1.0
            || !(0.0..=1.0).contains(&self.defend_anchor_range_fraction)
            || self.pressure_far_range_fraction < 1.0
            || !(0.0..1.0).contains(&self.pressure_near_range_fraction)
            || !(0.0..=1.0).contains(&self.pressure_retreat_fraction)
            || self.pressure_dash_range_fraction < 1.0
            || !(0.0..=1.0).contains(&self.ally_separation_weight)
            || !finite_positive
            || self.retreat_step_min_world > self.retreat_step_max_world
            || self.perimeter_recovery_trigger_cells >= self.perimeter_recovery_release_cells
            || self.total_search_expansions_per_tick == 0
            || self.total_search_expansions_per_tick > 65_536
            || self.maximum_search_expansions == 0
            || self.maximum_search_expansions > MAX_SEARCH_EXPANSIONS
            || self.maximum_route_points < 2
            || u32::from(self.maximum_route_points) > self.maximum_search_expansions
            || !(1_000..=10_000).contains(&self.damage_tile_cost_milli)
        {
            return Err("Practice bot profile exceeds engine bounds".into());
        }
        Ok(())
    }

    #[cfg(feature = "server")]
    pub(super) fn search_budget_per_bot(self, active_bots: usize) -> usize {
        usize::try_from(self.total_search_expansions_per_tick)
            .unwrap_or(1)
            .checked_div(active_bots.max(1))
            .unwrap_or(1)
            .max(1)
    }

    #[cfg(feature = "server")]
    pub(super) fn maximum_search_expansions(self) -> usize {
        usize::try_from(self.maximum_search_expansions).unwrap_or(1)
    }

    #[cfg(feature = "server")]
    pub(super) fn maximum_route_points(self) -> usize {
        usize::from(self.maximum_route_points)
    }
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
#[serde(deny_unknown_fields)]
pub(crate) struct BotCatalog {
    pub schema_version: u16,
    pub practice: BotProfile,
}

impl BotCatalog {
    pub(crate) fn embedded() -> Result<Self, String> {
        let catalog: Self = ron::from_str(include_str!("../../content/catalogs/bots.ron"))
            .map_err(|error| format!("embedded bot catalog parse failed: {error}"))?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub(crate) fn validate(&self) -> Result<(), String> {
        if self.schema_version != BOT_CATALOG_SCHEMA_VERSION {
            return Err("unsupported bot catalog schema".into());
        }
        self.practice.validate()?;
        if postcard::to_allocvec(self).map_or(true, |bytes| bytes.len() > MAX_BOT_CATALOG_BYTES) {
            return Err("bot catalog exceeds engine size ceiling".into());
        }
        Ok(())
    }

    pub(crate) fn canonical_fingerprint_material(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        postcard::to_allocvec(&(BOT_FINGERPRINT_FORMAT_VERSION, self))
            .map_err(|error| format!("bot fingerprint serialization failed: {error}"))
    }
}

#[cfg(feature = "server")]
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(super) struct BotProfileResource(pub BotProfile);

#[cfg(feature = "server")]
impl FromWorld for BotProfileResource {
    fn from_world(_: &mut World) -> Self {
        Self(BotProfile::embedded().expect("embedded Practice bot catalog is valid"))
    }
}
