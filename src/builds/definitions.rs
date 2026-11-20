use super::{
    BuildRecipeFingerprint, BuildRevision, ElementalFieldEffect, FighterBody, LoadoutRecipe,
    PASSIVE_ADRENAL_DURATION_TICKS_BOUNDS, PASSIVE_ADRENAL_MOVEMENT_BONUS_BASIS_POINTS_BOUNDS,
    PASSIVE_ADRENAL_REARM_TICKS_BOUNDS, PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS,
    PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS,
    PASSIVE_ELEMENTAL_RESISTANCE_BASIS_POINTS_BOUNDS, PASSIVE_FRAME_HEALTH_BASIS_POINTS_BOUNDS,
    PASSIVE_FRAME_SPEED_BASIS_POINTS_BOUNDS, PASSIVE_QUICK_CYCLE_REFILL_BASIS_POINTS_BOUNDS,
    PASSIVE_TENACITY_SLOW_BASIS_POINTS_BOUNDS, PassiveDefinitionId, PassiveKind, PassiveParameters,
    ResolvedFighterStats, ResolvedMatchLoadout, ResolvedPassive, ResolvedUltimate,
    RevealProximityModifier, SelectedBuild, ULTIMATE_PARAMETER_BOUNDS, UltimateChargePolicy,
    UltimateDefinitionId, UltimateKind, UltimateParameters,
};
use crate::combat::{DamageOverTimeKind, WeaponCatalog};
use crate::content::{GameplayContentFingerprint, fnv1a64};
use bevy::prelude::{FromWorld, Plugin, Resource};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const BUILD_CATALOG_SCHEMA_VERSION: u16 = 19;
pub const BUILD_FINGERPRINT_FORMAT_VERSION: u16 = 17;
const BUILD_RECIPE_SCHEMA_VERSION: u16 = 17;
pub const MAX_RESOLVED_LOADOUT_BYTES: usize = 4096;
pub(crate) const MAX_DIRECT_DIAGNOSTIC_PICKS: usize = 8;
pub(crate) const MAX_ULTIMATE_DEFINITIONS: usize = 32;
pub(crate) const MAX_PASSIVE_DEFINITIONS: usize = 32;
pub const MAX_FIGHTER_MOVEMENT_SPEED: f32 = 1_200.0;
pub const MIN_FIGHTER_BODY_RADIUS: f32 = 1.0;
/// Largest body supported by the one-cell map navigation and spawn-clearance contract.
pub const MAX_FIGHTER_BODY_RADIUS: f32 = 14.0;
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
    pub fighter_body: FighterBody,
    pub ultimate_charge: UltimateChargePolicy,
    pub(crate) direct_diagnostic: DirectDiagnosticLoadoutPolicy,
    pub ultimates: Vec<UltimateDefinition>,
    pub passives: Vec<PassiveDefinition>,
}

/// The ordered character picks a direct (non-routed) diagnostic session rotates through by
/// player ID. The character catalog validates and resolves them.
#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub(crate) struct DirectDiagnosticLoadoutPolicy {
    pub(crate) picks: Vec<crate::characters::CharacterChoice>,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct UltimateDefinition {
    pub id: UltimateDefinitionId,
    pub key: String,
    pub display_name: String,
    pub kind: UltimateKind,
    pub parameters: UltimateParameters,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PassiveDefinition {
    pub id: PassiveDefinitionId,
    pub key: String,
    pub display_name: String,
    pub kind: PassiveKind,
    pub parameters: PassiveParameters,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum BuildResolutionError {
    UnknownId,
    InvalidCombination,
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
        if self.ultimates.is_empty()
            || self.ultimates.len() > MAX_ULTIMATE_DEFINITIONS
            || self.passives.len() < 2
            || self.passives.len() > MAX_PASSIVE_DEFINITIONS
        {
            return Err("the build catalog requires non-empty definition inventories".into());
        }
        validate_metadata(&self.ultimates, |d| d.id.0, |d| &d.key, |d| &d.display_name)?;
        validate_metadata(&self.passives, |d| d.id.0, |d| &d.key, |d| &d.display_name)?;
        validate_ultimate_definitions(&self.ultimates)?;
        validate_passive_definitions(&self.passives)?;
        self.validate_direct_diagnostic_policy()?;
        if postcard::to_allocvec(self).map_or(true, |bytes| bytes.len() > 16 * 1024) {
            return Err("build catalog exceeds engine size ceiling".into());
        }
        Ok(())
    }

    /// Validate references that cross the build and weapon catalog boundary.
    pub(crate) fn validate_weapon_references(&self, weapons: &WeaponCatalog) -> Result<(), String> {
        self.validate()?;
        weapons.validate()?;
        for preset in &weapons.presets {
            weapons.resolve_preset(preset.id, self.fighter_body)?;
        }
        Ok(())
    }

    /// The bounded, authored direct-diagnostic picks; the character catalog resolves them.
    #[must_use]
    pub(crate) fn direct_diagnostic_picks(&self) -> &[crate::characters::CharacterChoice] {
        &self.direct_diagnostic.picks
    }

    fn validate_direct_diagnostic_policy(&self) -> Result<(), String> {
        let picks = &self.direct_diagnostic.picks;
        if picks.is_empty()
            || picks.len() > MAX_DIRECT_DIAGNOSTIC_PICKS
            || picks.iter().any(|pick| {
                pick.character_id.0 == 0
                    || self.ultimate(pick.ultimate_id).is_none()
                    || self.passive(pick.passive_id).is_none()
            })
        {
            return Err("direct diagnostic picks are invalid".into());
        }
        Ok(())
    }

    fn validate_tuning(&self) -> Result<(), String> {
        if !self.fighter_body.radius.is_finite()
            || !(MIN_FIGHTER_BODY_RADIUS..=MAX_FIGHTER_BODY_RADIUS)
                .contains(&self.fighter_body.radius)
        {
            return Err("invalid authored fighter body radius".into());
        }
        if self.ultimate_charge.maximum == 0
            || self.ultimate_charge.maximum > 10_000
            || self.ultimate_charge.dealt_damage_multiplier == 0
            || self.ultimate_charge.dealt_damage_multiplier > 100
            || self.ultimate_charge.received_damage_multiplier == 0
            || self.ultimate_charge.received_damage_multiplier > 100
        {
            return Err("invalid ultimate charge policy".into());
        }
        Ok(())
    }

    #[must_use]
    pub fn ultimate(&self, id: UltimateDefinitionId) -> Option<&UltimateDefinition> {
        self.ultimates.iter().find(|definition| definition.id == id)
    }

    #[must_use]
    pub fn passive(&self, id: PassiveDefinitionId) -> Option<&PassiveDefinition> {
        self.passives.iter().find(|definition| definition.id == id)
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
    reason = "one exhaustive kind/parameter matrix keeps all ultimate bounds and invariants reviewable"
)]
fn validate_ultimate_definitions(definitions: &[UltimateDefinition]) -> Result<(), String> {
    let bounds = ULTIMATE_PARAMETER_BOUNDS;
    let valid =
        definitions.iter().all(
            |definition| match (definition.kind, definition.parameters) {
                (
                    UltimateKind::Dash,
                    UltimateParameters::Dash {
                        maximum_distance_milliunits,
                        duration_ticks,
                        damage,
                        knockback_speed_milliunits,
                        knockback_duration_ticks,
                        maximum_targets,
                    },
                ) => {
                    bounds
                        .world_distance_milliunits
                        .contains(&maximum_distance_milliunits)
                        && bounds.short_ticks.contains(&duration_ticks)
                        && bounds.damage.contains(&damage)
                        && bounds
                            .world_distance_milliunits
                            .contains(&knockback_speed_milliunits)
                        && bounds.short_ticks.contains(&knockback_duration_ticks)
                        && bounds.target_count.contains(&maximum_targets)
                }
                (
                    UltimateKind::Sentry,
                    UltimateParameters::Sentry {
                        placement_offsets_milliunits,
                        body_radius_milliunits,
                        acquisition_range_milliunits,
                        acquisition_interval_ticks,
                        fire_interval_ticks,
                        lifetime_ticks,
                        maximum_health,
                        projectile_speed_milliunits,
                        projectile_radius_milliunits,
                        projectile_range_milliunits,
                        projectile_lifetime_ticks,
                        projectile_damage,
                    },
                ) => {
                    placement_offsets_milliunits
                        .iter()
                        .all(|offset| bounds.sentry_placement_offset_milliunits.contains(offset))
                        && placement_offsets_milliunits
                            .windows(2)
                            .all(|pair| pair[0] > pair[1])
                        && bounds
                            .compact_radius_milliunits
                            .contains(&body_radius_milliunits)
                        && bounds
                            .world_distance_milliunits
                            .contains(&acquisition_range_milliunits)
                        && bounds.short_ticks.contains(&acquisition_interval_ticks)
                        && bounds.duration_ticks.contains(&fire_interval_ticks)
                        && bounds.long_lifetime_ticks.contains(&lifetime_ticks)
                        && bounds.health.contains(&maximum_health)
                        && bounds
                            .world_distance_milliunits
                            .contains(&projectile_speed_milliunits)
                        && bounds
                            .compact_radius_milliunits
                            .contains(&projectile_radius_milliunits)
                        && bounds
                            .world_distance_milliunits
                            .contains(&projectile_range_milliunits)
                        && bounds.short_ticks.contains(&projectile_lifetime_ticks)
                        && bounds.damage.contains(&projectile_damage)
                        && projectile_radius_milliunits <= body_radius_milliunits
                        && projectile_range_milliunits <= acquisition_range_milliunits
                }
                (UltimateKind::SelfCloak, UltimateParameters::SelfCloak { duration_ticks }) => {
                    bounds.duration_ticks.contains(&duration_ticks)
                }
                (
                    UltimateKind::RevealScan,
                    UltimateParameters::RevealScan {
                        maximum_range_milliunits,
                        radius_milliunits,
                        reveal_ticks,
                    },
                ) => {
                    bounds
                        .world_distance_milliunits
                        .contains(&maximum_range_milliunits)
                        && bounds.field_radius_milliunits.contains(&radius_milliunits)
                        && bounds.duration_ticks.contains(&reveal_ticks)
                }
                (
                    UltimateKind::ConcealmentField,
                    UltimateParameters::ConcealmentField {
                        maximum_range_milliunits,
                        radius_milliunits,
                        duration_ticks,
                    },
                ) => {
                    bounds
                        .world_distance_milliunits
                        .contains(&maximum_range_milliunits)
                        && bounds.field_radius_milliunits.contains(&radius_milliunits)
                        && bounds.duration_ticks.contains(&duration_ticks)
                }
                (
                    UltimateKind::DemolitionStrike,
                    UltimateParameters::DemolitionStrike {
                        maximum_range_milliunits,
                        radius_milliunits,
                    },
                ) => {
                    bounds
                        .world_distance_milliunits
                        .contains(&maximum_range_milliunits)
                        && bounds
                            .demolition_radius_milliunits
                            .contains(&radius_milliunits)
                        && radius_milliunits.is_multiple_of(4_000)
                }
                (
                    kind @ (UltimateKind::CryogenicField
                    | UltimateKind::FireField
                    | UltimateKind::PoisonField
                    | UltimateKind::RestorationField),
                    UltimateParameters::ElementalField {
                        maximum_range_milliunits,
                        radius_milliunits,
                        duration_ticks,
                        pulse_interval_ticks,
                        effect,
                    },
                ) => {
                    bounds
                        .world_distance_milliunits
                        .contains(&maximum_range_milliunits)
                        && bounds.field_radius_milliunits.contains(&radius_milliunits)
                        && bounds.duration_ticks.contains(&duration_ticks)
                        && bounds.duration_ticks.contains(&pulse_interval_ticks)
                        && valid_elemental_ultimate_effect(kind, effect)
                }
                (
                    UltimateKind::BigBlob,
                    UltimateParameters::BigBlob {
                        maximum_range_milliunits,
                        flight_ticks,
                        visual_arc_height_milliunits,
                        landing_clearance_milliunits,
                        child_speed_milliunits,
                        child_radius_milliunits,
                        child_range_milliunits,
                        child_lifetime_ticks,
                        child_fuse_ticks,
                        child_explosion_radius_milliunits,
                        child_damage,
                        max_active_per_owner,
                    },
                ) => {
                    bounds
                        .world_distance_milliunits
                        .contains(&maximum_range_milliunits)
                        && bounds.short_ticks.contains(&flight_ticks)
                        && bounds
                            .field_radius_milliunits
                            .contains(&visual_arc_height_milliunits)
                        && bounds
                            .compact_radius_milliunits
                            .contains(&landing_clearance_milliunits)
                        && bounds
                            .world_distance_milliunits
                            .contains(&child_speed_milliunits)
                        && bounds
                            .compact_radius_milliunits
                            .contains(&child_radius_milliunits)
                        && bounds
                            .world_distance_milliunits
                            .contains(&child_range_milliunits)
                        && bounds.short_ticks.contains(&child_lifetime_ticks)
                        && bounds.duration_ticks.contains(&child_fuse_ticks)
                        && bounds
                            .compact_radius_milliunits
                            .contains(&child_explosion_radius_milliunits)
                        && bounds.damage.contains(&child_damage)
                        && bounds.active_count.contains(&max_active_per_owner)
                }
                _ => false,
            },
        );
    if !valid {
        return Err("ultimate kind and parameters do not match engine bounds".into());
    }
    Ok(())
}

fn validate_passive_definitions(definitions: &[PassiveDefinition]) -> Result<(), String> {
    let valid =
        definitions.iter().all(
            |definition| match (definition.kind, definition.parameters) {
                (
                    PassiveKind::LightweightFrame,
                    PassiveParameters::LightweightFrame {
                        maximum_health_basis_points,
                        movement_speed_basis_points,
                    },
                )
                | (
                    PassiveKind::ReinforcedFrame,
                    PassiveParameters::ReinforcedFrame {
                        maximum_health_basis_points,
                        movement_speed_basis_points,
                    },
                ) => {
                    PASSIVE_FRAME_HEALTH_BASIS_POINTS_BOUNDS.contains(&maximum_health_basis_points)
                        && PASSIVE_FRAME_SPEED_BASIS_POINTS_BOUNDS
                            .contains(&movement_speed_basis_points)
                }
                (
                    PassiveKind::AdrenalResponse,
                    PassiveParameters::AdrenalResponse {
                        duration_ticks,
                        rearm_ticks,
                        movement_bonus_basis_points,
                    },
                ) => {
                    PASSIVE_ADRENAL_DURATION_TICKS_BOUNDS.contains(&duration_ticks)
                        && PASSIVE_ADRENAL_REARM_TICKS_BOUNDS.contains(&rearm_ticks)
                        && PASSIVE_ADRENAL_MOVEMENT_BONUS_BASIS_POINTS_BOUNDS
                            .contains(&movement_bonus_basis_points)
                        && rearm_ticks >= duration_ticks
                }
                (
                    PassiveKind::CloseQuarters,
                    PassiveParameters::CloseQuarters {
                        near_distance_milliunits,
                        far_distance_milliunits,
                        near_damage_basis_points,
                        far_damage_basis_points,
                    },
                ) => {
                    PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS
                        .contains(&near_distance_milliunits)
                        && PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS
                            .contains(&far_distance_milliunits)
                        && PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS
                            .contains(&near_damage_basis_points)
                        && PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS
                            .contains(&far_damage_basis_points)
                        && near_distance_milliunits < far_distance_milliunits
                        && near_damage_basis_points > far_damage_basis_points
                }
                (
                    PassiveKind::QuickCycle,
                    PassiveParameters::QuickCycle {
                        refill_duration_basis_points,
                    },
                ) => PASSIVE_QUICK_CYCLE_REFILL_BASIS_POINTS_BOUNDS
                    .contains(&refill_duration_basis_points),
                (
                    PassiveKind::Tenacity,
                    PassiveParameters::Tenacity {
                        slow_duration_basis_points,
                    },
                ) => {
                    PASSIVE_TENACITY_SLOW_BASIS_POINTS_BOUNDS.contains(&slow_duration_basis_points)
                }
                (
                    PassiveKind::CryogenicInsulation,
                    PassiveParameters::CryogenicInsulation {
                        resistance_basis_points,
                    },
                )
                | (
                    PassiveKind::FilteredCirculation,
                    PassiveParameters::FilteredCirculation {
                        resistance_basis_points,
                    },
                )
                | (
                    PassiveKind::HeatShielding,
                    PassiveParameters::HeatShielding {
                        resistance_basis_points,
                    },
                ) => PASSIVE_ELEMENTAL_RESISTANCE_BASIS_POINTS_BOUNDS
                    .contains(&resistance_basis_points),
                _ => false,
            },
        );
    valid
        .then_some(())
        .ok_or_else(|| "passive kind and parameters do not match engine bounds".into())
}

fn valid_elemental_ultimate_effect(kind: UltimateKind, effect: ElementalFieldEffect) -> bool {
    let bounds = ULTIMATE_PARAMETER_BOUNDS;
    match (kind, effect) {
        (UltimateKind::CryogenicField, ElementalFieldEffect::Cold { amount })
        | (UltimateKind::RestorationField, ElementalFieldEffect::Heal { amount }) => {
            bounds.effect_amount.contains(&amount)
        }
        (
            UltimateKind::FireField,
            ElementalFieldEffect::DamageOverTime {
                kind: DamageOverTimeKind::Fire,
                damage_per_tick,
                tick_interval,
                duration_ticks,
            },
        )
        | (
            UltimateKind::PoisonField,
            ElementalFieldEffect::DamageOverTime {
                kind: DamageOverTimeKind::Poison,
                damage_per_tick,
                tick_interval,
                duration_ticks,
            },
        ) => {
            bounds.effect_amount.contains(&damage_per_tick)
                && bounds.duration_ticks.contains(&tick_interval)
                && bounds.duration_ticks.contains(&duration_ticks)
        }
        _ => false,
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

/// Engine bounds for one fighter's resolved stats, shared by every authored stat owner.
pub(crate) fn validate_fighter_stats(stats: &ResolvedFighterStats) -> Result<(), &'static str> {
    if stats.maximum_health == 0
        || !stats.movement_speed.is_finite()
        || stats.movement_speed <= 0.0
        || stats.movement_speed > MAX_FIGHTER_MOVEMENT_SPEED
        || stats.health_recovery_rate == 0
        || stats.idle_attack_delay_ticks == 0
        || stats.cold_capacity == 0
        || stats.cold_capacity > MAX_COLD_CAPACITY
        || stats.cold_resistance_basis_points > 6_000
        || stats.poison_resistance_basis_points > 6_000
        || stats.fire_resistance_basis_points > 6_000
        || !stats.reveal_proximity_radius.is_finite()
        || !(MIN_REVEAL_PROXIMITY_RADIUS..=MAX_REVEAL_PROXIMITY_RADIUS)
            .contains(&stats.reveal_proximity_radius)
    {
        return Err(
            "fighter stats: health, movement speed, resistances, or reveal proximity is outside engine bounds",
        );
    }
    Ok(())
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

/// Resolve one character recipe into the immutable match loadout.
///
/// This is the single authority path: character picks, Practice bots, diagnostics, and Balance Lab
/// re-resolution all reach combat through it. The fighter stats come from the character; the one
/// passive may scale or extend them.
pub fn resolve_loadout(
    catalog: &BuildCatalog,
    weapons: &WeaponCatalog,
    recipe: LoadoutRecipe,
) -> Result<ResolvedMatchLoadout, BuildResolutionError> {
    validate_fighter_stats(&recipe.fighter_stats)
        .map_err(|_| BuildResolutionError::InvalidCombination)?;
    let passive = catalog
        .passive(recipe.passive)
        .map(|definition| ResolvedPassive {
            id: definition.id,
            kind: definition.kind,
            parameters: definition.parameters,
        })
        .ok_or(BuildResolutionError::UnknownId)?;
    let ultimate_definition = catalog
        .ultimate(recipe.ultimate)
        .ok_or(BuildResolutionError::UnknownId)?;
    let ultimate = ResolvedUltimate {
        id: ultimate_definition.id,
        kind: ultimate_definition.kind,
        parameters: ultimate_definition.parameters,
        charge_policy: catalog.ultimate_charge,
    };
    let primary_weapon = crate::weapon_parts::resolve_weapon_parts(
        weapons,
        catalog.fighter_body,
        recipe.weapon_base,
        recipe.weapon_modifiers,
    )
    .map_err(|_| BuildResolutionError::InvalidCombination)?;
    let fighter_stats = apply_passive_to_stats(recipe.fighter_stats, &passive)?;
    // The identity names the pick, not its tuned values: stat and parameter changes are caught
    // by the gameplay content fingerprint, as before characters owned stats.
    let fingerprint_bytes = postcard::to_allocvec(&(
        BUILD_FINGERPRINT_FORMAT_VERSION,
        BUILD_RECIPE_SCHEMA_VERSION,
        catalog.balance_revision,
        recipe.character_id,
        recipe.weapon_base,
        recipe.weapon_modifiers,
        recipe.ultimate,
        recipe.passive,
    ))
    .map_err(|_| BuildResolutionError::ResolutionFailed)?;
    let resolved = ResolvedMatchLoadout {
        identity: SelectedBuild {
            recipe_fingerprint: BuildRecipeFingerprint(fnv1a64(&fingerprint_bytes)),
            revision: catalog.balance_revision,
        },
        character_id: recipe.character_id,
        fighter_stats,
        primary_weapon,
        ultimate,
        passives: [passive],
    };
    if postcard::to_allocvec(&resolved)
        .map_or(true, |bytes| bytes.len() > MAX_RESOLVED_LOADOUT_BYTES)
    {
        return Err(BuildResolutionError::ResolutionFailed);
    }
    Ok(resolved)
}

/// The passive's effect on the character's own stats: frames scale health and speed, resistance
/// passives add their resistance. Other passives act at runtime and leave the stats unchanged.
fn apply_passive_to_stats(
    mut stats: ResolvedFighterStats,
    passive: &ResolvedPassive,
) -> Result<ResolvedFighterStats, BuildResolutionError> {
    match passive.parameters {
        PassiveParameters::LightweightFrame {
            maximum_health_basis_points,
            movement_speed_basis_points,
        }
        | PassiveParameters::ReinforcedFrame {
            maximum_health_basis_points,
            movement_speed_basis_points,
        } => {
            let health =
                u32::from(stats.maximum_health) * u32::from(maximum_health_basis_points) / 10_000;
            stats.maximum_health = u16::try_from(health)
                .ok()
                .filter(|health| *health > 0)
                .ok_or(BuildResolutionError::ResolutionFailed)?;
            stats.movement_speed =
                stats.movement_speed * f32::from(movement_speed_basis_points) / 10_000.0;
        }
        PassiveParameters::CryogenicInsulation {
            resistance_basis_points,
        } => {
            stats.cold_resistance_basis_points = stats
                .cold_resistance_basis_points
                .saturating_add(resistance_basis_points)
                .min(6_000);
        }
        PassiveParameters::FilteredCirculation {
            resistance_basis_points,
        } => {
            stats.poison_resistance_basis_points = stats
                .poison_resistance_basis_points
                .saturating_add(resistance_basis_points)
                .min(6_000);
        }
        PassiveParameters::HeatShielding {
            resistance_basis_points,
        } => {
            stats.fire_resistance_basis_points = stats
                .fire_resistance_basis_points
                .saturating_add(resistance_basis_points)
                .min(6_000);
        }
        PassiveParameters::AdrenalResponse { .. }
        | PassiveParameters::CloseQuarters { .. }
        | PassiveParameters::QuickCycle { .. }
        | PassiveParameters::Tenacity { .. } => {}
    }
    validate_fighter_stats(&stats).map_err(|_| BuildResolutionError::ResolutionFailed)?;
    Ok(stats)
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub struct BuildCatalogResource(pub BuildCatalog);

impl FromWorld for BuildCatalogResource {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self(BuildCatalog::embedded().expect("embedded build catalog is valid"))
    }
}

pub struct BuildContentPlugin;

const FINGERPRINT_DOMAIN_SCHEMA_VERSION: u16 = 1;

impl Plugin for BuildContentPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.init_resource::<BuildCatalogResource>();
        crate::content::register_gameplay_fingerprint_contributor(
            app,
            crate::content::BUILDS_FINGERPRINT_DOMAIN,
            FINGERPRINT_DOMAIN_SCHEMA_VERSION,
            build_fingerprint_material,
        );
    }
}

fn build_fingerprint_material(world: &bevy::prelude::World) -> Result<Vec<u8>, String> {
    let builds = world
        .get_resource::<BuildCatalogResource>()
        .ok_or_else(|| "build catalog resource is not installed".to_owned())?;
    let weapons = world
        .get_resource::<crate::combat::WeaponCatalogResource>()
        .ok_or_else(|| "weapon catalog resource is not installed".to_owned())?;
    builds.0.validate_weapon_references(&weapons.0)?;
    builds.0.canonical_fingerprint_material()
}
