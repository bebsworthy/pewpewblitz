//! Authored global concealment reveal-lock rules.

use crate::content::{GameplayContentFingerprint, fnv1a64};
use bevy::prelude::{FromWorld, Plugin, Resource};
use serde::{Deserialize, Serialize};

pub const CONCEALMENT_RULES_SCHEMA_VERSION: u16 = 1;
pub const MAX_REVEAL_LOCK_TICKS: u64 = 3_600;

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq, Eq)]
#[serde(deny_unknown_fields)]
pub struct ConcealmentRules {
    pub schema_version: u16,
    pub attack_reveal_ticks: u64,
    pub damage_reveal_ticks: u64,
}

impl ConcealmentRules {
    pub fn embedded() -> Result<Self, String> {
        let rules: Self = ron::from_str(include_str!("../../content/catalogs/concealment.ron"))
            .map_err(|error| format!("embedded concealment rules parse failed: {error}"))?;
        rules.validate()?;
        Ok(rules)
    }

    pub fn validate(self) -> Result<(), String> {
        if self.schema_version != CONCEALMENT_RULES_SCHEMA_VERSION {
            return Err("unsupported concealment rules schema".into());
        }
        if self.attack_reveal_ticks == 0
            || self.attack_reveal_ticks > MAX_REVEAL_LOCK_TICKS
            || self.damage_reveal_ticks == 0
            || self.damage_reveal_ticks > MAX_REVEAL_LOCK_TICKS
        {
            return Err("invalid authored concealment reveal-lock duration".into());
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
pub struct ConcealmentRulesResource(pub ConcealmentRules);

impl FromWorld for ConcealmentRulesResource {
    fn from_world(_: &mut bevy::prelude::World) -> Self {
        Self(ConcealmentRules::embedded().expect("embedded concealment rules are valid"))
    }
}

pub struct ConcealmentContentPlugin;

const FINGERPRINT_DOMAIN_SCHEMA_VERSION: u16 = 1;

impl Plugin for ConcealmentContentPlugin {
    fn build(&self, app: &mut bevy::prelude::App) {
        app.init_resource::<ConcealmentRulesResource>();
        crate::content::register_gameplay_fingerprint_contributor(
            app,
            crate::content::CONCEALMENT_FINGERPRINT_DOMAIN,
            FINGERPRINT_DOMAIN_SCHEMA_VERSION,
            concealment_fingerprint_material,
        );
    }
}

fn concealment_fingerprint_material(world: &bevy::prelude::World) -> Result<Vec<u8>, String> {
    world
        .get_resource::<ConcealmentRulesResource>()
        .ok_or_else(|| "concealment rules resource is not installed".to_owned())?
        .0
        .canonical_fingerprint_material()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn embedded_rules_preserve_the_accepted_reveal_windows() {
        let rules = ConcealmentRules::embedded().unwrap();
        assert_eq!(rules.attack_reveal_ticks, 90);
        assert_eq!(rules.damage_reveal_ticks, 120);
        assert_ne!(rules.fingerprint().unwrap().0, 0);
    }

    #[test]
    fn rules_reject_wrong_schema_zero_and_excessive_durations() {
        let baseline = ConcealmentRules::embedded().unwrap();
        for invalid in [
            ConcealmentRules {
                schema_version: CONCEALMENT_RULES_SCHEMA_VERSION + 1,
                ..baseline
            },
            ConcealmentRules {
                attack_reveal_ticks: 0,
                ..baseline
            },
            ConcealmentRules {
                damage_reveal_ticks: 0,
                ..baseline
            },
            ConcealmentRules {
                attack_reveal_ticks: MAX_REVEAL_LOCK_TICKS + 1,
                ..baseline
            },
            ConcealmentRules {
                damage_reveal_ticks: MAX_REVEAL_LOCK_TICKS + 1,
                ..baseline
            },
        ] {
            assert!(invalid.validate().is_err());
        }
    }

    #[test]
    fn either_authored_duration_changes_the_rules_fingerprint() {
        let baseline = ConcealmentRules::embedded().unwrap();
        let baseline_fingerprint = baseline.fingerprint().unwrap();
        assert_ne!(
            ConcealmentRules {
                attack_reveal_ticks: baseline.attack_reveal_ticks + 1,
                ..baseline
            }
            .fingerprint()
            .unwrap(),
            baseline_fingerprint
        );
        assert_ne!(
            ConcealmentRules {
                damage_reveal_ticks: baseline.damage_reveal_ticks + 1,
                ..baseline
            }
            .fingerprint()
            .unwrap(),
            baseline_fingerprint
        );
    }
}
