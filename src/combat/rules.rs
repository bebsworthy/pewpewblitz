//! Authored global combat-condition lifecycle rules.

use crate::content::{GameplayContentFingerprint, fnv1a64};
use bevy::prelude::{FromWorld, Resource};
use serde::{Deserialize, Serialize};

pub const COMBAT_CONDITION_RULES_SCHEMA_VERSION: u16 = 1;
pub const MAX_COLD_RULE_TICKS: u64 = 3_600;
pub const MAX_COLD_DECAY_PER_TICK: u16 = 1_000;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombatConditionRules {
    pub schema_version: u16,
    pub cold_decay_delay_ticks: u64,
    pub cold_decay_per_tick: u16,
    pub freeze_duration_ticks: u64,
    pub thaw_immunity_ticks: u64,
}

impl CombatConditionRules {
    pub fn embedded() -> Result<Self, String> {
        let rules: Self = ron::from_str(include_str!(
            "../../content/catalogs/combat_conditions.ron"
        ))
        .map_err(|error| format!("embedded combat-condition rules parse failed: {error}"))?;
        rules.validate()?;
        Ok(rules)
    }

    pub fn validate(self) -> Result<(), String> {
        if self.schema_version != COMBAT_CONDITION_RULES_SCHEMA_VERSION {
            return Err("unsupported combat-condition rules schema".into());
        }
        if self.cold_decay_delay_ticks > MAX_COLD_RULE_TICKS
            || self.cold_decay_per_tick == 0
            || self.cold_decay_per_tick > MAX_COLD_DECAY_PER_TICK
            || self.freeze_duration_ticks == 0
            || self.freeze_duration_ticks > MAX_COLD_RULE_TICKS
            || self.thaw_immunity_ticks > MAX_COLD_RULE_TICKS
        {
            return Err("invalid authored Cold/Freeze lifecycle rules".into());
        }
        Ok(())
    }

    pub fn canonical_fingerprint_material(self) -> Result<Vec<u8>, String> {
        self.validate()?;
        postcard::to_allocvec(&self).map_err(|error| error.to_string())
    }

    pub fn fingerprint(self) -> Result<GameplayContentFingerprint, String> {
        Ok(GameplayContentFingerprint(fnv1a64(
            &self.canonical_fingerprint_material()?,
        )))
    }
}

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombatConditionRulesResource(pub CombatConditionRules);

impl FromWorld for CombatConditionRulesResource {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self(CombatConditionRules::embedded().expect("embedded combat-condition rules are valid"))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_rules_are_valid_and_fingerprinted() {
        let rules = CombatConditionRules::embedded().unwrap();
        assert_eq!(rules.cold_decay_delay_ticks, 90);
        assert_eq!(rules.cold_decay_per_tick, 10);
        assert_eq!(rules.freeze_duration_ticks, 60);
        assert_eq!(rules.thaw_immunity_ticks, 90);
        assert_ne!(rules.fingerprint().unwrap().0, 0);
    }

    #[test]
    fn rules_reject_zero_or_out_of_bounds_lifecycle_values() {
        let baseline = CombatConditionRules::embedded().unwrap();
        assert!(
            CombatConditionRules {
                freeze_duration_ticks: 0,
                ..baseline
            }
            .validate()
            .is_err()
        );
        assert!(
            CombatConditionRules {
                cold_decay_per_tick: MAX_COLD_DECAY_PER_TICK + 1,
                ..baseline
            }
            .validate()
            .is_err()
        );
    }
}
