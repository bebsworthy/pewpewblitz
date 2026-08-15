use super::{
    BrawlerBuildRecipe, BuildPresetId, BuildRecipeFingerprint, BuildRevision, PassiveDefinitionId,
    PassiveKind, PulseMagazine, PulsePower, PulseReach, ResolvedFighterStats, ResolvedMatchLoadout,
    ResolvedPassive, ResolvedUltimate, SelectedBuild, UltimateDefinitionId, UltimateKind,
    WeaponChoice,
};
use crate::combat::{
    DeliveryMethod, FighterDefinition, PayloadEffectDefinition, WeaponCatalog, WeaponConfiguration,
    WeaponEconomy, WeaponPresetId, resolve_configuration,
};
use crate::content::{GameplayContentFingerprint, fnv1a64};
use bevy::prelude::{FromWorld, Plugin, Resource};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

pub const BUILD_CATALOG_SCHEMA_VERSION: u16 = 1;
pub const BUILD_FINGERPRINT_FORMAT_VERSION: u16 = 1;
pub const MAX_BUILD_CANDIDATE_BYTES: usize = 128;
pub const MAX_RESOLVED_LOADOUT_BYTES: usize = 4096;
pub const BUILD_POINT_BUDGET: u8 = 12;

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
pub struct BuildCatalog {
    pub schema_version: u16,
    pub balance_revision: BuildRevision,
    pub weapon_costs: Vec<WeaponPointCost>,
    pub ultimates: Vec<UltimateDefinition>,
    pub passives: Vec<PassiveDefinition>,
    pub presets: Vec<BuildPresetDefinition>,
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
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct PassiveDefinition {
    pub id: PassiveDefinitionId,
    pub key: String,
    pub display_name: String,
    pub kind: PassiveKind,
    pub point_cost: u8,
}

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct BuildPresetDefinition {
    pub id: BuildPresetId,
    pub key: String,
    pub display_name: String,
    pub recipe: BrawlerBuildRecipe,
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
        let catalog: Self = ron::from_str(include_str!("../../content/v1/builds.ron"))
            .map_err(|error| format!("embedded build catalog parse failed: {error}"))?;
        catalog.validate()?;
        Ok(catalog)
    }

    pub fn validate(&self) -> Result<(), String> {
        if self.schema_version != BUILD_CATALOG_SCHEMA_VERSION || self.balance_revision.0 == 0 {
            return Err("unsupported build catalog schema/revision".into());
        }
        if self.weapon_costs.len() != 4
            || self.ultimates.len() != 2
            || self.passives.len() != 6
            || self.presets.len() != 4
        {
            return Err(
                "M08 requires four weapon costs, two ultimates, six passives, and four presets"
                    .into(),
            );
        }
        validate_metadata(&self.ultimates, |d| d.id.0, |d| &d.key, |d| &d.display_name)?;
        validate_metadata(&self.passives, |d| d.id.0, |d| &d.key, |d| &d.display_name)?;
        validate_metadata(&self.presets, |d| d.id.0, |d| &d.key, |d| &d.display_name)?;
        let expected_weapon_costs = [
            (WeaponPresetId(1), 4),
            (WeaponPresetId(2), 5),
            (WeaponPresetId(3), 5),
            (WeaponPresetId(4), 4),
        ];
        if !self
            .weapon_costs
            .iter()
            .zip(expected_weapon_costs)
            .all(|(definition, expected)| (definition.weapon_id, definition.point_cost) == expected)
        {
            return Err("weapon point costs do not match the M08 content contract".into());
        }
        let expected_ultimates = [
            (UltimateDefinitionId(1), UltimateKind::Dash, 3),
            (UltimateDefinitionId(2), UltimateKind::Sentry, 4),
        ];
        if !self
            .ultimates
            .iter()
            .zip(expected_ultimates)
            .all(|(definition, expected)| {
                (definition.id, definition.kind, definition.point_cost) == expected
            })
        {
            return Err("ultimate inventory does not match the M08 engine contract".into());
        }
        let expected_passives = [
            (PassiveDefinitionId(1), PassiveKind::LightweightFrame, 2),
            (PassiveDefinitionId(2), PassiveKind::ReinforcedFrame, 2),
            (PassiveDefinitionId(3), PassiveKind::AdrenalResponse, 2),
            (PassiveDefinitionId(4), PassiveKind::CloseQuarters, 2),
            (PassiveDefinitionId(5), PassiveKind::QuickCycle, 2),
            (PassiveDefinitionId(6), PassiveKind::Tenacity, 1),
        ];
        if !self
            .passives
            .iter()
            .zip(expected_passives)
            .all(|(definition, expected)| {
                (definition.id, definition.kind, definition.point_cost) == expected
            })
            || !self.presets.iter().enumerate().all(|(index, preset)| {
                preset.id.0 == u16::try_from(index + 1).expect("four presets fit u16")
            })
        {
            return Err(
                "passive or preset inventory does not match the M08 engine contract".into(),
            );
        }
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
        let weapons = WeaponCatalog::embedded()?;
        let fighter = crate::combat::FighterDefinitions::default().entries[0];
        for preset in &self.presets {
            resolve_build_recipe(self, &weapons, &fighter, preset.recipe, Some(preset.id))
                .map_err(|error| format!("illegal build preset {}: {error:?}", preset.key))?;
        }
        if postcard::to_allocvec(self).map_or(true, |bytes| bytes.len() > 16 * 1024) {
            return Err("build catalog exceeds engine size ceiling".into());
        }
        Ok(())
    }

    #[must_use]
    pub fn preset(&self, id: BuildPresetId) -> Option<&BuildPresetDefinition> {
        self.presets.iter().find(|definition| definition.id == id)
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
    source_preset_id: Option<BuildPresetId>,
) -> Result<ResolvedMatchLoadout, BuildResolutionError> {
    if let Some(preset_id) = source_preset_id {
        let preset = catalog
            .preset(preset_id)
            .ok_or(BuildResolutionError::UnknownId)?;
        if preset.recipe != recipe {
            return Err(BuildResolutionError::InvalidCombination);
        }
    }
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
    let ultimate_definition = catalog
        .ultimates
        .iter()
        .find(|definition| definition.id == recipe.ultimate)
        .ok_or(BuildResolutionError::UnknownId)?;
    let ultimate = ResolvedUltimate {
        id: ultimate_definition.id,
        kind: ultimate_definition.kind,
        point_cost: ultimate_definition.point_cost,
    };
    let (primary_weapon, weapon_points) =
        resolve_weapon_choice(catalog, weapons, fighter, recipe.weapon)?;
    let total_points = weapon_points
        .checked_add(ultimate.point_cost)
        .and_then(|points| points.checked_add(passives[0].point_cost))
        .and_then(|points| points.checked_add(passives[1].point_cost))
        .ok_or(BuildResolutionError::OverBudget)?;
    if total_points > BUILD_POINT_BUDGET {
        return Err(BuildResolutionError::OverBudget);
    }
    let fighter_stats = if has_lightweight {
        ResolvedFighterStats {
            maximum_health: 85,
            movement_speed: 360.0,
        }
    } else if has_reinforced {
        ResolvedFighterStats {
            maximum_health: 120,
            movement_speed: 288.0,
        }
    } else {
        ResolvedFighterStats {
            maximum_health: 100,
            movement_speed: 320.0,
        }
    };
    let mut canonical_passives = recipe.passives;
    canonical_passives.sort();
    let fingerprint_bytes = postcard::to_allocvec(&(
        BUILD_FINGERPRINT_FORMAT_VERSION,
        catalog.schema_version,
        catalog.balance_revision,
        recipe.weapon,
        recipe.ultimate,
        canonical_passives,
    ))
    .map_err(|_| BuildResolutionError::ResolutionFailed)?;
    let identity = SelectedBuild {
        source_build_preset_id: source_preset_id,
        recipe_fingerprint: BuildRecipeFingerprint(fnv1a64(&fingerprint_bytes)),
        revision: catalog.balance_revision,
    };
    let resolved = ResolvedMatchLoadout {
        identity,
        total_points,
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
            let (damage, cooldown, power_cost) = match power {
                PulsePower::Light => (20, 9, 0),
                PulsePower::Balanced => (25, 12, 0),
                PulsePower::Heavy => (30, 15, 1),
            };
            let (speed, range, reach_cost) = match reach {
                PulseReach::Compact => (1020.0, 750.0, 0),
                PulseReach::Standard => (900.0, 900.0, 0),
                PulseReach::Long => (780.0, 1050.0, 1),
            };
            let lifetime = projectile_lifetime_ticks(range, speed)?;
            let (capacity, refill_ticks, magazine_cost) = match magazine {
                PulseMagazine::Quick => (4, 42, 0),
                PulseMagazine::Standard => (6, 60, 0),
                PulseMagazine::Expanded => (8, 78, 1),
            };
            configuration.recipe.fire_cooldown_ticks = cooldown;
            configuration.recipe.economy = WeaponEconomy::Magazine {
                capacity,
                refill_ticks,
            };
            if let DeliveryMethod::Straight {
                speed: value_speed,
                range: value_range,
                lifetime_ticks: value_lifetime,
                ..
            } = &mut configuration.recipe.delivery
            {
                *value_speed = speed;
                *value_range = range;
                *value_lifetime = lifetime;
            }
            for bundle in &mut configuration.recipe.payload_bundles {
                for effect in &mut bundle.effects {
                    if let PayloadEffectDefinition::Damage { amount, .. } = effect {
                        *amount = damage;
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
