//! Shard Rush-owned mode registration, authored-rule resolution, and server composition.

use super::{ModeRegistration, ModeTopologyPolicy};
use crate::{config::GameMode, map::SHARD_RUSH_MODE_DEFINITION};
use bevy::prelude::{App, Plugin};

pub(super) const REGISTRATION: ModeRegistration = ModeRegistration {
    configured_mode: Some(GameMode::ShardRush),
    key: "shard-rush",
    definition_id: SHARD_RUSH_MODE_DEFINITION,
    topology: ModeTopologyPolicy::ShardRushSource,
    #[cfg(feature = "server")]
    server: Some(super::ServerModeProjection {
        rules_revision: crate::matchplay::SHARD_RUSH_RULES_REVISION,
        compatible_maps: super::CompatibleMapPolicy::ExactModeDefinition,
        default_map_preset: crate::map::PRISM_QUARRY_PRESET,
        routing_mode: brawler_routing::GameMode::ShardRush,
        validate_operator_policy,
        resolve_operator_rules,
        install: install_server,
    }),
    #[cfg(feature = "client")]
    presentation: Some(super::ModePresentationProjection {
        selection_label: "Shard Rush",
    }),
};

pub(super) struct ShardRushModeRegistrationPlugin;

impl Plugin for ShardRushModeRegistrationPlugin {
    fn build(&self, app: &mut App) {
        use super::registry::ModeRegistrationAppExt as _;
        app.try_register_mode(REGISTRATION)
            .expect("Shard Rush mode registration is unique and bounded");
    }
}

#[cfg(feature = "server")]
fn validate_operator_policy(input: super::ModeOperatorPolicyInput) -> Result<(), &'static str> {
    input.shard_rush.validate().map(|_| ())
}

#[cfg(feature = "server")]
fn resolve_operator_rules(
    input: super::ModeRuleResolveInput,
) -> Result<super::ResolvedModeRuleProjection, super::ModeRuleResolutionError> {
    if input.kills_to_win != 0
        || input.capture_seconds != 0
        || input.safe_health != 0
        || input.pressure_stages != 0
        || input.shards_to_win == 0
    {
        return Err(super::ModeRuleResolutionError::MismatchedObjective);
    }
    let rules = crate::matchplay::ShardRushRules {
        target_shards: input.shards_to_win,
        policy: input.shard_rush,
    }
    .validate()
    .map_err(super::ModeRuleResolutionError::InvalidObjective)?;
    if u64::from(rules.policy.hold_ticks) >= input.match_duration_ticks {
        return Err(super::ModeRuleResolutionError::InvalidObjective(
            "Shard Rush hold countdown must be shorter than the match",
        ));
    }
    Ok(super::ResolvedModeRuleProjection {
        objective_target: u16::from(rules.target_shards),
        rules_summary: crate::lobby::AdvertisedRulesSummary::ShardRush {
            target_shards: rules.target_shards,
            hold_ticks: rules.policy.hold_ticks,
            active_limit_ticks: input.match_duration_ticks,
        },
    })
}

#[cfg(feature = "server")]
fn install_server(app: &mut App, registration: &ModeRegistration, input: super::ModeInstallInput) {
    use crate::{
        map::ServerMapSelection,
        matchplay::{
            MatchModeSetup, ShardRushModePlugin, ShardRushRules, shard_rush_rules_for_profile,
        },
    };
    let rules = match (input.objective_target, input.shard_rush) {
        (Some(target), Some(policy)) => ShardRushRules {
            target_shards: u8::try_from(target)
                .expect("validated manifest Shard Rush target fits the shard ceiling"),
            policy,
        },
        (None, None) if input.profile == crate::config::MatchRulesProfile::ProcessVerification => {
            shard_rush_rules_for_profile(input.profile)
        }
        _ => panic!("production Shard Rush requires one complete authored policy"),
    };
    let rules = rules
        .validate()
        .expect("validated manifest Shard Rush objective");
    let server = registration.server();
    app.insert_resource(MatchModeSetup {
        mode_definition_id: registration.definition_id,
        rules_revision: server.rules_revision,
    })
    .insert_resource(rules)
    .insert_resource(ServerMapSelection::preset(server.default_map_preset))
    .add_plugins(ShardRushModePlugin);
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    fn input() -> super::super::ModeRuleResolveInput {
        super::super::ModeRuleResolveInput {
            kills_to_win: 0,
            capture_seconds: 0,
            safe_health: 0,
            pressure_stages: 0,
            shards_to_win: 8,
            match_duration_ticks: 10_800,
            lifecycle: crate::matchplay::MatchLifecycleRules::default(),
            takedown_recent_hostile_damage_credit_ticks: 300,
            vault_break_critical_health_percent: 25,
            shard_rush: crate::matchplay::ShardRushPolicy {
                hold_ticks: 720,
                spawn_interval_ticks: 360,
                live_shard_ceiling: 12,
            },
        }
    }

    #[test]
    fn operator_rules_project_target_and_hold() {
        assert_eq!(
            resolve_operator_rules(input()).unwrap(),
            super::super::ResolvedModeRuleProjection {
                objective_target: 8,
                rules_summary: crate::lobby::AdvertisedRulesSummary::ShardRush {
                    target_shards: 8,
                    hold_ticks: 720,
                    active_limit_ticks: 10_800,
                },
            }
        );
    }

    #[test]
    fn operator_rules_reject_other_objectives_and_invalid_policy() {
        for mismatched in [
            super::super::ModeRuleResolveInput {
                shards_to_win: 0,
                ..input()
            },
            super::super::ModeRuleResolveInput {
                kills_to_win: 10,
                ..input()
            },
        ] {
            assert_eq!(
                resolve_operator_rules(mismatched),
                Err(super::super::ModeRuleResolutionError::MismatchedObjective)
            );
        }
        assert!(matches!(
            resolve_operator_rules(super::super::ModeRuleResolveInput {
                shards_to_win: 40,
                ..input()
            }),
            Err(super::super::ModeRuleResolutionError::InvalidObjective(_))
        ));
        assert!(matches!(
            resolve_operator_rules(super::super::ModeRuleResolveInput {
                match_duration_ticks: 720,
                ..input()
            }),
            Err(super::super::ModeRuleResolutionError::InvalidObjective(_))
        ));
        assert!(
            validate_operator_policy(super::super::ModeOperatorPolicyInput {
                takedown_recent_hostile_damage_credit_ticks: 300,
                vault_break_critical_health_percent: 25,
                shard_rush: crate::matchplay::ShardRushPolicy {
                    live_shard_ceiling: 0,
                    ..input().shard_rush
                },
            })
            .is_err()
        );
    }
}
