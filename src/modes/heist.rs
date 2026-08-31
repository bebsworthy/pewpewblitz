//! Heist-owned mode registration, authored-rule resolution, and server composition.

use super::{ModeRegistration, ModeTopologyPolicy};
use crate::{config::GameMode, map::HEIST_MODE_DEFINITION};
use bevy::prelude::{App, Plugin};

pub(super) const REGISTRATION: ModeRegistration = ModeRegistration {
    configured_mode: Some(GameMode::Heist),
    key: "heist",
    definition_id: HEIST_MODE_DEFINITION,
    topology: ModeTopologyPolicy::MirroredHeistSafes,
    #[cfg(feature = "server")]
    server: Some(super::ServerModeProjection {
        rules_revision: crate::matchplay::HEIST_RULES_REVISION,
        compatible_maps: super::CompatibleMapPolicy::ExactModeDefinition,
        default_map_preset: crate::map::FEATURE_YARD_HEIST_PRESET,
        routing_mode: brawler_routing::GameMode::Heist,
        validate_operator_policy,
        resolve_operator_rules,
        install: install_server,
    }),
    #[cfg(feature = "client")]
    presentation: Some(super::ModePresentationProjection {
        selection_label: "Heist",
    }),
};

pub(super) struct HeistModeRegistrationPlugin;

impl Plugin for HeistModeRegistrationPlugin {
    fn build(&self, app: &mut App) {
        use super::registry::ModeRegistrationAppExt as _;
        app.try_register_mode(REGISTRATION)
            .expect("Heist mode registration is unique and bounded");
    }
}

#[cfg(feature = "server")]
fn validate_operator_policy(input: super::ModeOperatorPolicyInput) -> Result<(), &'static str> {
    crate::matchplay::HeistRules {
        safe_maximum_health: 1,
        critical_health_percent: input.heist_critical_health_percent,
    }
    .validate()
    .map(|_| ())
}

#[cfg(feature = "server")]
fn resolve_operator_rules(
    input: super::ModeRuleResolveInput,
) -> Result<super::ResolvedModeRuleProjection, super::ModeRuleResolutionError> {
    if input.kills_to_win != 0 || input.capture_seconds != 0 || input.safe_health == 0 {
        return Err(super::ModeRuleResolutionError::MismatchedObjective);
    }
    let safe_maximum_health = input.safe_health;
    crate::matchplay::HeistRules {
        safe_maximum_health,
        critical_health_percent: input.heist_critical_health_percent,
    }
    .validate()
    .map_err(super::ModeRuleResolutionError::InvalidObjective)?;
    Ok(super::ResolvedModeRuleProjection {
        objective_target: safe_maximum_health,
        rules_summary: crate::lobby::AdvertisedRulesSummary::Heist {
            safe_maximum_health,
            active_limit_ticks: input.match_duration_ticks,
        },
    })
}

#[cfg(feature = "server")]
fn install_server(app: &mut App, registration: &ModeRegistration, input: super::ModeInstallInput) {
    use crate::{
        map::ServerMapSelection,
        matchplay::{HeistModePlugin, HeistRules, MatchModeSetup},
    };
    let rules = match (input.objective_target, input.heist_critical_health_percent) {
        (Some(safe_maximum_health), Some(critical_health_percent)) => HeistRules {
            safe_maximum_health,
            critical_health_percent,
        },
        (None, None) if input.profile == crate::config::MatchRulesProfile::ProcessVerification => {
            HeistRules {
                safe_maximum_health: 2_000,
                critical_health_percent: 25,
            }
        }
        _ => panic!("production Heist requires one complete authored policy"),
    };
    let rules = rules
        .validate()
        .expect("validated manifest Heist objective");
    let server = registration.server();
    app.insert_resource(MatchModeSetup {
        mode_definition_id: registration.definition_id,
        rules_revision: server.rules_revision,
    })
    .insert_resource(rules)
    .insert_resource(ServerMapSelection {
        preset_id: server.default_map_preset,
    })
    .add_plugins(HeistModePlugin);
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    fn input() -> super::super::ModeRuleResolveInput {
        super::super::ModeRuleResolveInput {
            kills_to_win: 0,
            capture_seconds: 0,
            safe_health: 2_000,
            match_duration_ticks: 10_800,
            lifecycle: crate::matchplay::MatchLifecycleRules::default(),
            wipeout_recent_hostile_damage_credit_ticks: 300,
            heist_critical_health_percent: 25,
        }
    }

    #[test]
    fn operator_rules_preserve_authored_safe_projection() {
        assert_eq!(
            resolve_operator_rules(input()).unwrap(),
            super::super::ResolvedModeRuleProjection {
                objective_target: 2_000,
                rules_summary: crate::lobby::AdvertisedRulesSummary::Heist {
                    safe_maximum_health: 2_000,
                    active_limit_ticks: 10_800,
                },
            }
        );
    }

    #[test]
    fn operator_policy_validation_is_owned_by_heist_registration() {
        assert!(
            validate_operator_policy(super::super::ModeOperatorPolicyInput {
                wipeout_recent_hostile_damage_credit_ticks: 300,
                heist_critical_health_percent: 0,
            })
            .is_err()
        );
        assert_eq!(
            resolve_operator_rules(super::super::ModeRuleResolveInput {
                safe_health: 0,
                ..input()
            }),
            Err(super::super::ModeRuleResolutionError::MismatchedObjective)
        );
    }
}
