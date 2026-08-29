use super::{
    BrawlerBuildRecipe, BuildRecipeFingerprint, BuildRevision, ElementalFieldEffect,
    PassiveDefinitionId, PassiveKind, PassiveParameters, PulseMagazine, PulsePower, PulseReach,
    ResolvedFighterStats, ResolvedMatchLoadout, ResolvedPassive, ResolvedUltimate,
    RevealProximityModifier, SelectedBuild, UltimateChargePolicy, UltimateDefinitionId,
    UltimateKind, UltimateParameters, WeaponChoice,
};
use crate::combat::{
    DamageOverTimeKind, DeliveryMethod, FighterDefinition, PayloadEffectDefinition, WeaponCatalog,
    WeaponConfiguration, WeaponEconomy, WeaponPresetId, resolve_configuration,
};
use crate::content::{GameplayContentFingerprint, fnv1a64};
use bevy::prelude::{FromWorld, Plugin, Resource};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const BUILD_CATALOG_SCHEMA_VERSION: u16 = 14;
pub const BUILD_FINGERPRINT_FORMAT_VERSION: u16 = 14;
pub const MAX_BUILD_CANDIDATE_BYTES: usize = 128;
pub const MAX_RESOLVED_LOADOUT_BYTES: usize = 4096;
pub(crate) const MAX_ULTIMATE_DEFINITIONS: usize = 32;
pub(crate) const MAX_PASSIVE_DEFINITIONS: usize = 32;
pub const BUILD_POINT_BUDGET: u8 = 12;
pub const MAX_FIGHTER_MOVEMENT_SPEED: f32 = 1_200.0;
pub const MAX_COLD_CAPACITY: u16 = 10_000;
pub const MIN_REVEAL_PROXIMITY_RADIUS: f32 = 32.0;
pub const MAX_REVEAL_PROXIMITY_RADIUS: f32 = 1_024.0;
pub const MAX_REVEAL_PROXIMITY_FLAT_MILLIUNITS: i32 = 512_000;
pub const MIN_REVEAL_PROXIMITY_PERCENT_BASIS_POINTS: i16 = -9_000;
pub const MAX_REVEAL_PROXIMITY_PERCENT_BASIS_POINTS: i16 = 20_000;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BuildCatalog {
    pub schema_version: u16,
    pub balance_revision: BuildRevision,
    pub fighter_profiles: FighterStatProfiles,
    pub custom_pulse: CustomPulseTuning,
    pub ultimate_charge: UltimateChargePolicy,
    pub weapon_costs: Vec<WeaponPointCost>,
    pub ultimates: Vec<UltimateDefinition>,
    pub passives: Vec<PassiveDefinition>,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct FighterStatProfiles {
    pub default: ResolvedFighterStats,
    pub lightweight: ResolvedFighterStats,
    pub reinforced: ResolvedFighterStats,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PulsePowerTuning {
    pub damage: u16,
    pub fire_cooldown_ticks: u64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct PulseReachTuning {
    pub speed: f32,
    pub range: f32,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct PulseMagazineTuning {
    pub capacity: u8,
    pub refill_ticks: u64,
}

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct CustomPulseTuning {
    pub light: PulsePowerTuning,
    pub balanced: PulsePowerTuning,
    pub heavy: PulsePowerTuning,
    pub compact: PulseReachTuning,
    pub standard: PulseReachTuning,
    pub long: PulseReachTuning,
    pub quick: PulseMagazineTuning,
    pub standard_magazine: PulseMagazineTuning,
    pub expanded: PulseMagazineTuning,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WeaponPointCost {
    pub weapon_id: WeaponPresetId,
    pub point_cost: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct UltimateDefinition {
    pub id: UltimateDefinitionId,
    pub key: String,
    pub display_name: String,
    pub kind: UltimateKind,
    pub point_cost: u8,
    pub parameters: UltimateParameters,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PassiveDefinition {
    pub id: PassiveDefinitionId,
    pub key: String,
    pub display_name: String,
    pub kind: PassiveKind,
    pub point_cost: u8,
    pub parameters: PassiveParameters,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildResolutionError {
    UnknownId,
    InvalidCombination,
    OverBudget,
    CandidateTooLarge,
    ResolutionFailed,
}

impl BuildCatalog {
    pub fn embedded() -> Result<Self, String> {
        let catalog: Self = ron::from_str(include_str!("../../content/catalogs/builds.ron"))
            .map_err(|error| format!("embedded build catalog parse failed: {error}"))?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != BUILD_CATALOG_SCHEMA_VERSION || self.balance_revision.0 == 0 {
            return Err("unsupported build catalog schema/revision".into());
        }
        self.validate_tuning()?;
        if self.weapon_costs.is_empty()
            || self.weapon_costs.len() > crate::combat::MAX_WEAPON_PRESETS
            || self.ultimates.is_empty()
            || self.ultimates.len() > MAX_ULTIMATE_DEFINITIONS
            || self.passives.len() < 2
            || self.passives.len() > MAX_PASSIVE_DEFINITIONS
        {
            return Err("the build catalog requires non-empty definition inventories".into());
        }
        validate_metadata(&self.ultimates, |d| d.id.0, |d| &d.key, |d| &d.display_name)?;
        validate_metadata(&self.passives, |d| d.id.0, |d| &d.key, |d| &d.display_name)?;
        if self
            .weapon_costs
            .windows(2)
            .any(|pair| pair[0].weapon_id >= pair[1].weapon_id)
            || self
                .weapon_costs
                .iter()
                .any(|definition| definition.weapon_id.0 == 0)
        {
            return Err("weapon point costs must have unique ascending non-zero IDs".into());
        }
        validate_ultimate_definitions(&self.ultimates)?;
        validate_passive_definitions(&self.passives)?;
        if self
            .ultimates
            .iter()
            .any(|d| d.point_cost == 0 || d.point_cost > BUILD_POINT_BUDGET)
            || self
                .passives
                .iter()
                .any(|d| d.point_cost == 0 || d.point_cost > BUILD_POINT_BUDGET)
            || self
                .weapon_costs
                .iter()
                .any(|d| d.point_cost == 0 || d.point_cost > BUILD_POINT_BUDGET)
        {
            return Err("invalid authored point cost".into());
        }
        if postcard::to_allocvec(self).map_or(true, |bytes| bytes.len() > 16 * 1024) {
            return Err("build catalog exceeds engine size ceiling".into());
        }
        Ok(())
    }

    /// Validate references that cross the build and weapon catalog boundary.
    pub(crate) fn validate_weapon_references(&self, weapons: &WeaponCatalog) -> Result<(), String> {
        self.validate()?;
        weapons.validate()?;
        let cost_ids: HashSet<_> = self
            .weapon_costs
            .iter()
            .map(|definition| definition.weapon_id)
            .collect();
        let preset_ids: HashSet<_> = weapons.presets.iter().map(|preset| preset.id).collect();
        if cost_ids != preset_ids {
            return Err("weapon point costs must exactly cover the weapon catalog".into());
        }
        Ok(())
    }

    fn validate_tuning(&self) -> Result<(), String> {
        if self.ultimate_charge.maximum == 0
            || self.ultimate_charge.maximum > 10_000
            || self.ultimate_charge.dealt_damage_multiplier == 0
            || self.ultimate_charge.dealt_damage_multiplier > 100
            || self.ultimate_charge.received_damage_multiplier == 0
            || self.ultimate_charge.received_damage_multiplier > 100
        {
            return Err("invalid ultimate charge policy".into());
        }
        for (name, profile) in [
            ("default", self.fighter_profiles.default),
            ("lightweight", self.fighter_profiles.lightweight),
            ("reinforced", self.fighter_profiles.reinforced),
        ] {
            if profile.maximum_health == 0
                || !profile.movement_speed.is_finite()
                || profile.movement_speed <= 0.0
                || profile.movement_speed > MAX_FIGHTER_MOVEMENT_SPEED
                || profile.health_recovery_rate == 0
                || profile.idle_attack_delay_ticks == 0
                || profile.cold_capacity == 0
                || profile.cold_capacity > MAX_COLD_CAPACITY
                || profile.cold_resistance_basis_points > 6_000
                || profile.poison_resistance_basis_points > 6_000
                || profile.fire_resistance_basis_points > 6_000
                || !profile.reveal_proximity_radius.is_finite()
                || !(MIN_REVEAL_PROXIMITY_RADIUS..=MAX_REVEAL_PROXIMITY_RADIUS)
                    .contains(&profile.reveal_proximity_radius)
            {
                return Err(format!(
                    "invalid {name} fighter stat profile: health, movement speed, or reveal proximity is outside engine bounds"
                ));
            }
        }
        for power in [
            self.custom_pulse.light,
            self.custom_pulse.balanced,
            self.custom_pulse.heavy,
        ] {
            if power.damage == 0
                || power.damage > 1_000
                || power.fire_cooldown_ticks == 0
                || power.fire_cooldown_ticks > 3_600
            {
                return Err("invalid custom Pulse power tuning".into());
            }
        }
        for reach in [
            self.custom_pulse.compact,
            self.custom_pulse.standard,
            self.custom_pulse.long,
        ] {
            if !reach.speed.is_finite()
                || !reach.range.is_finite()
                || !(1.0..=4_096.0).contains(&reach.speed)
                || !(1.0..=4_096.0).contains(&reach.range)
            {
                return Err("invalid custom Pulse reach tuning".into());
            }
        }
        for magazine in [
            self.custom_pulse.quick,
            self.custom_pulse.standard_magazine,
            self.custom_pulse.expanded,
        ] {
            if magazine.capacity == 0 || magazine.capacity > 32 || magazine.refill_ticks == 0 {
                return Err("invalid custom Pulse magazine tuning".into());
            }
        }
        Ok(())
    }

    #[must_use]
    pub fn ultimate(&self, id: UltimateDefinitionId) -> Option<&UltimateDefinition> {
        self.ultimates.iter().find(|definition| definition.id == id)
    }

    pub fn canonical_fingerprint_material(&self) -> Result<Vec<u8>, String> {
        self.validate()?;
        postcard::to_allocvec(&(BUILD_FINGERPRINT_FORMAT_VERSION, self))
            .map_err(|error| format!("build fingerprint serialization failed: {error}"))
    }

    pub fn fingerprint(&self) -> Result<GameplayContentFingerprint, String> {
        Ok(GameplayContentFingerprint(fnv1a64(
            &self.canonical_fingerprint_material()?,
        )))
    }
}

#[allow(
    clippy::too_many_lines,
    reason = "one exhaustive kind/parameter validation matrix keeps authored ultimate bounds reviewable together"
)]
fn validate_ultimate_definitions(definitions: &[UltimateDefinition]) -> Result<(), String> {
    if definitions.iter().any(|definition| {
        !matches!(
            (definition.kind, definition.parameters),
            (
                UltimateKind::Dash,
                UltimateParameters::Dash {
                    maximum_distance_milliunits: 1..=4_096_000,
                    duration_ticks: 1..=600,
                    damage: 1..=1_000,
                    knockback_speed_milliunits: 1..=4_096_000,
                    knockback_duration_ticks: 1..=600,
                    maximum_targets: 1..=32,
                }
            )
                | (
                    UltimateKind::Sentry,
                    UltimateParameters::Sentry {
                        placement_offsets_milliunits: _,
                        body_radius_milliunits: 1..=512_000,
                        acquisition_range_milliunits: 1..=4_096_000,
                        acquisition_interval_ticks: 1..=600,
                        fire_interval_ticks: 1..=3_600,
                        lifetime_ticks: 1..=36_000,
                        maximum_health: 1..=10_000,
                        projectile_speed_milliunits: 1..=4_096_000,
                        projectile_radius_milliunits: 1..=512_000,
                        projectile_range_milliunits: 1..=4_096_000,
                        projectile_lifetime_ticks: 1..=600,
                        projectile_damage: 1..=1_000,
                        presentation_profile_id: 1..,
                    }
                )
                | (
                    UltimateKind::SelfCloak,
                    UltimateParameters::SelfCloak {
                        duration_ticks: 1..=3_600
                    }
                )
                | (
                    UltimateKind::RevealScan,
                    UltimateParameters::RevealScan {
                        maximum_range_milliunits: 1..=4_096_000,
                        radius_milliunits: 1..=2_048_000,
                        reveal_ticks: 1..=3_600,
                    }
                )
                | (
                    UltimateKind::ConcealmentField,
                    UltimateParameters::ConcealmentField {
                        maximum_range_milliunits: 1..=4_096_000,
                        radius_milliunits: 1..=2_048_000,
                        duration_ticks: 1..=3_600,
                    }
                )
                | (
                    UltimateKind::DemolitionStrike,
                    UltimateParameters::DemolitionStrike {
                        maximum_range_milliunits: 1..=4_096_000,
                        radius_milliunits: 8_000..=64_000,
                    }
                )
                | (
                    UltimateKind::BigBlob,
                    UltimateParameters::BigBlob {
                        maximum_range_milliunits: 1..=4_096_000,
                        flight_ticks: 1..=600,
                        visual_arc_height_milliunits: 1..=2_048_000,
                        landing_clearance_milliunits: 1..=512_000,
                        child_speed_milliunits: 1..=4_096_000,
                        child_radius_milliunits: 1..=512_000,
                        child_range_milliunits: 1..=4_096_000,
                        child_lifetime_ticks: 1..=600,
                        child_fuse_ticks: 1..=3_600,
                        child_explosion_radius_milliunits: 1..=512_000,
                        child_damage: 1..=1_000,
                        max_active_per_owner: 1..=16,
                    }
                )
                | (
                    UltimateKind::CryogenicField
                        | UltimateKind::FireField
                        | UltimateKind::PoisonField
                        | UltimateKind::RestorationField,
                    UltimateParameters::ElementalField {
                        maximum_range_milliunits: 1..=4_096_000,
                        radius_milliunits: 1..=2_048_000,
                        duration_ticks: 1..=3_600,
                        pulse_interval_ticks: 1..=3_600,
                        ..
                    }
                )
        ) || matches!(
            definition.parameters,
            UltimateParameters::Sentry {
                placement_offsets_milliunits,
                body_radius_milliunits,
                projectile_radius_milliunits,
                projectile_range_milliunits,
                acquisition_range_milliunits,
                ..
            } if placement_offsets_milliunits.iter().any(|offset| *offset == 0 || *offset > 1_024_000)
                || placement_offsets_milliunits.windows(2).any(|pair| pair[0] <= pair[1])
                || projectile_radius_milliunits > body_radius_milliunits
                || projectile_range_milliunits > acquisition_range_milliunits
        ) || matches!(
            definition.parameters,
            UltimateParameters::DemolitionStrike {
                radius_milliunits,
                ..
            } if !radius_milliunits.is_multiple_of(4_000)
        ) || !valid_elemental_ultimate_effect(definition.kind, definition.parameters)
    }) {
        return Err("ultimate kind and parameters do not match engine bounds".into());
    }
    Ok(())
}

fn validate_passive_definitions(definitions: &[PassiveDefinition]) -> Result<(), String> {
    let valid = definitions.iter().all(|definition| {
        matches!(
            (definition.kind, definition.parameters),
            (
                PassiveKind::LightweightFrame,
                PassiveParameters::LightweightFrame
            ) | (
                PassiveKind::ReinforcedFrame,
                PassiveParameters::ReinforcedFrame
            ) | (
                PassiveKind::AdrenalResponse,
                PassiveParameters::AdrenalResponse {
                    duration_ticks: 1..=3_600,
                    rearm_ticks: 1..=36_000,
                    movement_bonus_basis_points: 1..=10_000,
                }
            ) | (
                PassiveKind::CloseQuarters,
                PassiveParameters::CloseQuarters {
                    near_distance_milliunits: 1..=4_096_000,
                    far_distance_milliunits: 1..=4_096_000,
                    near_damage_basis_points: 1..=30_000,
                    far_damage_basis_points: 1..=30_000,
                }
            ) | (
                PassiveKind::QuickCycle,
                PassiveParameters::QuickCycle {
                    refill_duration_basis_points: 1..=10_000,
                }
            ) | (
                PassiveKind::Tenacity,
                PassiveParameters::Tenacity {
                    slow_duration_basis_points: 1..=10_000,
                }
            ) | (
                PassiveKind::CryogenicInsulation,
                PassiveParameters::CryogenicInsulation {
                    resistance_basis_points: 1..=6_000,
                }
            ) | (
                PassiveKind::FilteredCirculation,
                PassiveParameters::FilteredCirculation {
                    resistance_basis_points: 1..=6_000,
                }
            ) | (
                PassiveKind::HeatShielding,
                PassiveParameters::HeatShielding {
                    resistance_basis_points: 1..=6_000,
                }
            )
        ) && !matches!(
            definition.parameters,
            PassiveParameters::AdrenalResponse {
                duration_ticks,
                rearm_ticks,
                ..
            } if rearm_ticks < duration_ticks
        ) && !matches!(
            definition.parameters,
            PassiveParameters::CloseQuarters {
                near_distance_milliunits,
                far_distance_milliunits,
                near_damage_basis_points,
                far_damage_basis_points,
            } if near_distance_milliunits >= far_distance_milliunits
                || near_damage_basis_points <= far_damage_basis_points
        )
    });
    valid
        .then_some(())
        .ok_or_else(|| "passive kind and parameters do not match engine bounds".into())
}

fn valid_elemental_ultimate_effect(kind: UltimateKind, parameters: UltimateParameters) -> bool {
    match (kind, parameters) {
        (
            UltimateKind::CryogenicField,
            UltimateParameters::ElementalField {
                effect: ElementalFieldEffect::Cold { amount: 1.. },
                ..
            },
        )
        | (
            UltimateKind::FireField,
            UltimateParameters::ElementalField {
                effect:
                    ElementalFieldEffect::DamageOverTime {
                        kind: DamageOverTimeKind::Fire,
                        damage_per_tick: 1..,
                        tick_interval: 1..=3_600,
                        duration_ticks: 1..=3_600,
                    },
                ..
            },
        )
        | (
            UltimateKind::PoisonField,
            UltimateParameters::ElementalField {
                effect:
                    ElementalFieldEffect::DamageOverTime {
                        kind: DamageOverTimeKind::Poison,
                        damage_per_tick: 1..,
                        tick_interval: 1..=3_600,
                        duration_ticks: 1..=3_600,
                    },
                ..
            },
        )
        | (
            UltimateKind::RestorationField,
            UltimateParameters::ElementalField {
                effect: ElementalFieldEffect::Heal { amount: 1.. },
                ..
            },
        ) => true,
        (
            UltimateKind::CryogenicField
            | UltimateKind::FireField
            | UltimateKind::PoisonField
            | UltimateKind::RestorationField,
            _,
        ) => false,
        _ => true,
    }
}

/// Resolve one reveal radius from bounded, deterministic authored modifier units.
///
/// Percentage applies to the base first, the flat term applies second, and the result is clamped
/// before being rounded once to thousandths of a world unit.
pub fn resolve_reveal_proximity_radius(
    base_radius: f32,
    modifier: RevealProximityModifier,
) -> Result<f32, String> {
    if !base_radius.is_finite() || base_radius <= 0.0 {
        return Err("reveal proximity base radius must be finite and positive".to_string());
    }
    if modifier.flat_milliunits.unsigned_abs() > MAX_REVEAL_PROXIMITY_FLAT_MILLIUNITS.unsigned_abs()
        || !(MIN_REVEAL_PROXIMITY_PERCENT_BASIS_POINTS..=MAX_REVEAL_PROXIMITY_PERCENT_BASIS_POINTS)
            .contains(&modifier.percent_basis_points)
    {
        return Err("reveal proximity modifier exceeds engine bounds".to_string());
    }
    let percentage = 1.0 + f64::from(modifier.percent_basis_points) / 10_000.0;
    let flat = f64::from(modifier.flat_milliunits) / 1_000.0;
    let resolved = (f64::from(base_radius) * percentage + flat).clamp(
        f64::from(MIN_REVEAL_PROXIMITY_RADIUS),
        f64::from(MAX_REVEAL_PROXIMITY_RADIUS),
    );
    let rounded = (resolved * 1_000.0).round() / 1_000.0;
    if !rounded.is_finite() || rounded <= 0.0 {
        return Err("resolved reveal proximity radius is invalid".to_string());
    }
    #[allow(
        clippy::cast_possible_truncation,
        reason = "the value is clamped to 32..=1024 and rounded to authored f32 precision"
    )]
    let rounded = rounded as f32;
    Ok(rounded)
}

fn validate_metadata<T>(
    values: &[T],
    id: impl Fn(&T) -> u16,
    key: impl Fn(&T) -> &String,
    name: impl Fn(&T) -> &String,
) -> Result<(), String> {
    let mut ids = HashSet::new();
    let mut keys = HashSet::new();
    let mut prior = 0;
    for value in values {
        let current = id(value);
        let key = key(value);
        let name = name(value);
        if current == 0
            || current <= prior
            || !ids.insert(current)
            || !keys.insert(key)
            || key.is_empty()
            || key.len() > 48
            || !key
                .bytes()
                .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'-')
            || name.trim().is_empty()
            || name.len() > 64
        {
            return Err("invalid or duplicate build catalog metadata".into());
        }
        prior = current;
    }
    Ok(())
}

#[allow(clippy::too_many_lines)]
pub fn resolve_build_recipe(
    catalog: &BuildCatalog,
    weapons: &WeaponCatalog,
    fighter: &FighterDefinition,
    recipe: BrawlerBuildRecipe,
) -> Result<ResolvedMatchLoadout, BuildResolutionError> {
    resolve_build_recipe_inner(catalog, weapons, fighter, recipe, None)
}

/// Resolve V7's authored saved-brawler recipe without the retired point-budget or frame-passive
/// stat inference. The fighter profile is an explicit immutable creation choice.
pub fn resolve_saved_brawler_recipe(
    catalog: &BuildCatalog,
    weapons: &WeaponCatalog,
    fighter: &FighterDefinition,
    fighter_profile_id: crate::profiles::FighterProfileId,
    weapon_base_id: crate::profiles::WeaponBaseId,
    ultimate: UltimateDefinitionId,
    passives: [PassiveDefinitionId; 2],
) -> Result<ResolvedMatchLoadout, BuildResolutionError> {
    let fighter_stats = match fighter_profile_id.0 {
        1 => catalog.fighter_profiles.default,
        2 => catalog.fighter_profiles.lightweight,
        3 => catalog.fighter_profiles.reinforced,
        _ => return Err(BuildResolutionError::UnknownId),
    };
    if passives.iter().any(|id| {
        catalog
            .passives
            .iter()
            .find(|definition| definition.id == *id)
            .is_none_or(|definition| {
                matches!(
                    definition.kind,
                    PassiveKind::LightweightFrame | PassiveKind::ReinforcedFrame
                )
            })
    }) {
        return Err(BuildResolutionError::UnknownId);
    }
    resolve_build_recipe_inner(
        catalog,
        weapons,
        fighter,
        BrawlerBuildRecipe {
            weapon: WeaponChoice::Preset(WeaponPresetId(weapon_base_id.0)),
            ultimate,
            passives,
        },
        Some((fighter_profile_id.0, fighter_stats)),
    )
}

#[allow(clippy::too_many_lines)]
fn resolve_build_recipe_inner(
    catalog: &BuildCatalog,
    weapons: &WeaponCatalog,
    fighter: &FighterDefinition,
    recipe: BrawlerBuildRecipe,
    explicit_fighter_profile: Option<(u16, ResolvedFighterStats)>,
) -> Result<ResolvedMatchLoadout, BuildResolutionError> {
    if postcard::to_allocvec(&recipe).map_or(true, |bytes| bytes.len() > MAX_BUILD_CANDIDATE_BYTES)
    {
        return Err(BuildResolutionError::CandidateTooLarge);
    }
    if recipe.passives[0] == recipe.passives[1] {
        return Err(BuildResolutionError::InvalidCombination);
    }
    let passives = recipe.passives.map(|id| {
        catalog
            .passives
            .iter()
            .find(|definition| definition.id == id)
            .map(|definition| ResolvedPassive {
                id,
                kind: definition.kind,
                point_cost: definition.point_cost,
                parameters: definition.parameters,
            })
            .ok_or(BuildResolutionError::UnknownId)
    });
    let mut passives = [passives[0].clone()?, passives[1].clone()?];
    passives.sort_by_key(|passive| passive.id);
    let has_lightweight = passives
        .iter()
        .any(|p| p.kind == PassiveKind::LightweightFrame);
    let has_reinforced = passives
        .iter()
        .any(|p| p.kind == PassiveKind::ReinforcedFrame);
    if has_lightweight && has_reinforced {
        return Err(BuildResolutionError::InvalidCombination);
    }
    let resistance_passive_count = passives
        .iter()
        .filter(|passive| {
            matches!(
                passive.kind,
                PassiveKind::CryogenicInsulation
                    | PassiveKind::FilteredCirculation
                    | PassiveKind::HeatShielding
            )
        })
        .count();
    if resistance_passive_count > 1 {
        return Err(BuildResolutionError::InvalidCombination);
    }
    let ultimate_definition = catalog
        .ultimates
        .iter()
        .find(|definition| definition.id == recipe.ultimate)
        .ok_or(BuildResolutionError::UnknownId)?;
    let ultimate = ResolvedUltimate {
        id: ultimate_definition.id,
        kind: ultimate_definition.kind,
        point_cost: ultimate_definition.point_cost,
        parameters: ultimate_definition.parameters,
        charge_policy: catalog.ultimate_charge,
    };
    let (primary_weapon, _weapon_points) =
        resolve_weapon_choice(catalog, weapons, fighter, recipe.weapon)?;
    let total_points = build_point_total(catalog, recipe)?;
    if explicit_fighter_profile.is_none() && total_points > BUILD_POINT_BUDGET {
        return Err(BuildResolutionError::OverBudget);
    }
    let mut fighter_stats = if let Some((_, stats)) = explicit_fighter_profile {
        stats
    } else if has_lightweight {
        catalog.fighter_profiles.lightweight
    } else if has_reinforced {
        catalog.fighter_profiles.reinforced
    } else {
        catalog.fighter_profiles.default
    };
    for passive in &passives {
        match passive.kind {
            PassiveKind::CryogenicInsulation => {
                let PassiveParameters::CryogenicInsulation {
                    resistance_basis_points,
                } = passive.parameters
                else {
                    return Err(BuildResolutionError::ResolutionFailed);
                };
                fighter_stats.cold_resistance_basis_points = fighter_stats
                    .cold_resistance_basis_points
                    .saturating_add(resistance_basis_points)
                    .min(6_000);
            }
            PassiveKind::FilteredCirculation => {
                let PassiveParameters::FilteredCirculation {
                    resistance_basis_points,
                } = passive.parameters
                else {
                    return Err(BuildResolutionError::ResolutionFailed);
                };
                fighter_stats.poison_resistance_basis_points = fighter_stats
                    .poison_resistance_basis_points
                    .saturating_add(resistance_basis_points)
                    .min(6_000);
            }
            PassiveKind::HeatShielding => {
                let PassiveParameters::HeatShielding {
                    resistance_basis_points,
                } = passive.parameters
                else {
                    return Err(BuildResolutionError::ResolutionFailed);
                };
                fighter_stats.fire_resistance_basis_points = fighter_stats
                    .fire_resistance_basis_points
                    .saturating_add(resistance_basis_points)
                    .min(6_000);
            }
            PassiveKind::LightweightFrame
            | PassiveKind::ReinforcedFrame
            | PassiveKind::AdrenalResponse
            | PassiveKind::CloseQuarters
            | PassiveKind::QuickCycle
            | PassiveKind::Tenacity => {}
        }
    }
    let mut canonical_passives = recipe.passives;
    canonical_passives.sort();
    let fingerprint_bytes = if let Some((fighter_profile_id, _)) = explicit_fighter_profile {
        postcard::to_allocvec(&(
            BUILD_FINGERPRINT_FORMAT_VERSION,
            catalog.schema_version,
            catalog.balance_revision,
            fighter_profile_id,
            recipe.weapon,
            recipe.ultimate,
            canonical_passives,
        ))
    } else {
        postcard::to_allocvec(&(
            BUILD_FINGERPRINT_FORMAT_VERSION,
            catalog.schema_version,
            catalog.balance_revision,
            recipe.weapon,
            recipe.ultimate,
            canonical_passives,
        ))
    }
    .map_err(|_| BuildResolutionError::ResolutionFailed)?;
    let identity = SelectedBuild {
        recipe_fingerprint: BuildRecipeFingerprint(fnv1a64(&fingerprint_bytes)),
        revision: catalog.balance_revision,
    };
    let resolved = ResolvedMatchLoadout {
        identity,
        total_points: if explicit_fighter_profile.is_some() {
            0
        } else {
            total_points
        },
        fighter_stats,
        primary_weapon,
        ultimate,
        passives,
    };
    if postcard::to_allocvec(&resolved)
        .map_or(true, |bytes| bytes.len() > MAX_RESOLVED_LOADOUT_BYTES)
    {
        return Err(BuildResolutionError::ResolutionFailed);
    }
    Ok(resolved)
}

/// Return the exact authored point total without requiring a complete runtime loadout resolution.
/// This keeps editor budget feedback and authoritative rejection copy on the same pure rule.
pub fn build_point_total(
    catalog: &BuildCatalog,
    recipe: BrawlerBuildRecipe,
) -> Result<u8, BuildResolutionError> {
    let weapon = match recipe.weapon {
        WeaponChoice::Preset(id) => weapon_point_cost(catalog, id)?,
        WeaponChoice::CustomPulse {
            power,
            reach,
            magazine,
        } => weapon_point_cost(catalog, WeaponPresetId(1))?
            .checked_add(u8::from(matches!(power, PulsePower::Heavy)))
            .and_then(|points| points.checked_add(u8::from(matches!(reach, PulseReach::Long))))
            .and_then(|points| {
                points.checked_add(u8::from(matches!(magazine, PulseMagazine::Expanded)))
            })
            .ok_or(BuildResolutionError::OverBudget)?,
    };
    let ultimate = catalog
        .ultimates
        .iter()
        .find(|definition| definition.id == recipe.ultimate)
        .map(|definition| definition.point_cost)
        .ok_or(BuildResolutionError::UnknownId)?;
    recipe.passives.iter().try_fold(
        weapon
            .checked_add(ultimate)
            .ok_or(BuildResolutionError::OverBudget)?,
        |total, passive| {
            let cost = catalog
                .passives
                .iter()
                .find(|definition| definition.id == *passive)
                .map(|definition| definition.point_cost)
                .ok_or(BuildResolutionError::UnknownId)?;
            total
                .checked_add(cost)
                .ok_or(BuildResolutionError::OverBudget)
        },
    )
}

fn resolve_weapon_choice(
    catalog: &BuildCatalog,
    weapons: &WeaponCatalog,
    fighter: &FighterDefinition,
    choice: WeaponChoice,
) -> Result<(crate::combat::ResolvedWeapon, u8), BuildResolutionError> {
    match choice {
        WeaponChoice::Preset(id) => {
            let cost = weapon_point_cost(catalog, id)?;
            weapons
                .resolve_preset(id, fighter)
                .map(|weapon| (weapon, cost))
                .map_err(|_| BuildResolutionError::ResolutionFailed)
        }
        WeaponChoice::CustomPulse {
            power,
            reach,
            magazine,
        } => {
            let mut configuration: WeaponConfiguration = weapons
                .preset(WeaponPresetId(1))
                .ok_or(BuildResolutionError::ResolutionFailed)?
                .configuration
                .clone();
            let (power_tuning, power_cost) = match power {
                PulsePower::Light => (catalog.custom_pulse.light, 0),
                PulsePower::Balanced => (catalog.custom_pulse.balanced, 0),
                PulsePower::Heavy => (catalog.custom_pulse.heavy, 1),
            };
            let (reach_tuning, reach_cost) = match reach {
                PulseReach::Compact => (catalog.custom_pulse.compact, 0),
                PulseReach::Standard => (catalog.custom_pulse.standard, 0),
                PulseReach::Long => (catalog.custom_pulse.long, 1),
            };
            let lifetime = projectile_lifetime_ticks(reach_tuning.range, reach_tuning.speed)?;
            let (magazine_tuning, magazine_cost) = match magazine {
                PulseMagazine::Quick => (catalog.custom_pulse.quick, 0),
                PulseMagazine::Standard => (catalog.custom_pulse.standard_magazine, 0),
                PulseMagazine::Expanded => (catalog.custom_pulse.expanded, 1),
            };
            configuration.recipe.fire_cooldown_ticks = power_tuning.fire_cooldown_ticks;
            configuration.recipe.economy = WeaponEconomy::Magazine {
                capacity: magazine_tuning.capacity,
                refill_ticks: magazine_tuning.refill_ticks,
            };
            if let DeliveryMethod::Straight {
                speed: value_speed,
                range: value_range,
                lifetime_ticks: value_lifetime,
                ..
            } = &mut configuration.recipe.delivery
            {
                *value_speed = reach_tuning.speed;
                *value_range = reach_tuning.range;
                *value_lifetime = lifetime;
            }
            for bundle in &mut configuration.recipe.payload_bundles {
                for effect in &mut bundle.effects {
                    if let PayloadEffectDefinition::Damage { amount, .. } = effect {
                        *amount = power_tuning.damage;
                    }
                }
            }
            let points = weapon_point_cost(catalog, WeaponPresetId(1))?
                .checked_add(power_cost)
                .and_then(|points| points.checked_add(reach_cost))
                .and_then(|points| points.checked_add(magazine_cost))
                .ok_or(BuildResolutionError::ResolutionFailed)?;
            resolve_configuration(None, configuration, fighter)
                .map(|weapon| (weapon, points))
                .map_err(|_| BuildResolutionError::ResolutionFailed)
        }
    }
}

fn weapon_point_cost(
    catalog: &BuildCatalog,
    id: WeaponPresetId,
) -> Result<u8, BuildResolutionError> {
    catalog
        .weapon_costs
        .iter()
        .find(|definition| definition.weapon_id == id)
        .map(|definition| definition.point_cost)
        .ok_or(BuildResolutionError::UnknownId)
}

#[allow(
    clippy::cast_possible_truncation,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss
)]
fn projectile_lifetime_ticks(range: f32, speed: f32) -> Result<u64, BuildResolutionError> {
    let ticks = (range * crate::timing::SIMULATION_TICK_HZ as f32 / speed).ceil();
    if !ticks.is_finite() || ticks <= 0.0 || ticks > u64::MAX as f32 {
        return Err(BuildResolutionError::ResolutionFailed);
    }
    Ok(ticks as u64)
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct BuildCatalogResource(pub BuildCatalog);

impl FromWorld for BuildCatalogResource {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self(BuildCatalog::embedded().expect("embedded build catalog is valid"))
    }
}

pub struct BuildContentPlugin;

impl Plugin for BuildContentPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.init_resource::<BuildCatalogResource>();
    }
}
