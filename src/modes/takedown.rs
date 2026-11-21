//! Takedown-owned mode registration, authored-rule resolution, and server composition.

use super::{ModeRegistration, ModeTopologyPolicy};
use crate::{config::GameMode, map::TAKEDOWN_MODE_DEFINITION};
use bevy::prelude::{App, Plugin};

pub(super) const REGISTRATION: ModeRegistration = ModeRegistration {
    configured_mode: Some(GameMode::Takedown),
    key: "takedown",
    definition_id: TAKEDOWN_MODE_DEFINITION,
    topology: ModeTopologyPolicy::NoAnchors,
    #[cfg(feature = "server")]
    server: Some(super::ServerModeProjection {
        rules_revision: crate::matchplay::TAKEDOWN_RULES_REVISION,
        compatible_maps: super::CompatibleMapPolicy::ExactModeDefinition,
        default_map_preset: crate::map::FEATURE_YARD_TAKEDOWN_PRESET,
        routing_mode: brawler_routing::GameMode::Takedown,
        validate_operator_policy,
        resolve_operator_rules,
        install: install_server,
    }),
    #[cfg(feature = "client")]
    presentation: Some(super::ModePresentationProjection {
        selection_label: "Takedown",
    }),
};

pub(super) struct TakedownModeRegistrationPlugin;

impl Plugin for TakedownModeRegistrationPlugin {
    fn build(&self, app: &mut App) {
        use super::registry::ModeRegistrationAppExt as _;
        app.try_register_mode(REGISTRATION)
            .expect("Takedown mode registration is unique and bounded");
    }
}

#[cfg(feature = "server")]
fn validate_operator_policy(input: super::ModeOperatorPolicyInput) -> Result<(), &'static str> {
    crate::matchplay::TakedownRules {
        target_score: 1,
        recent_hostile_damage_credit_ticks: input.takedown_recent_hostile_damage_credit_ticks,
    }
    .validate()
    .map(|_| ())
}

#[cfg(feature = "server")]
fn resolve_operator_rules(
    input: super::ModeRuleResolveInput,
) -> Result<super::ResolvedModeRuleProjection, super::ModeRuleResolutionError> {
    if input.capture_seconds != 0
        || input.safe_health != 0
        || (input.pressure_stages != 0 || input.shards_to_win != 0)
    {
        return Err(super::ModeRuleResolutionError::MismatchedObjective);
    }
    let target_score = input.kills_to_win;
    crate::matchplay::TakedownRules {
        target_score,
        recent_hostile_damage_credit_ticks: input.takedown_recent_hostile_damage_credit_ticks,
    }
    .validate()
    .map_err(super::ModeRuleResolutionError::InvalidObjective)?;
    Ok(super::ResolvedModeRuleProjection {
        objective_target: target_score,
        rules_summary: crate::lobby::AdvertisedRulesSummary::Takedown {
            target_score,
            active_limit_ticks: input.match_duration_ticks,
        },
    })
}

#[cfg(feature = "server")]
pub(crate) fn takedown_rules_for_profile(
    profile: crate::config::MatchRulesProfile,
) -> crate::matchplay::TakedownRules {
    use crate::matchplay::TakedownRules;
    match profile {
        crate::config::MatchRulesProfile::Production => {
            panic!("production Takedown requires authored policy")
        }
        crate::config::MatchRulesProfile::ProcessVerification => TakedownRules {
            target_score: 10,
            recent_hostile_damage_credit_ticks: 300,
        },
    }
    .validate()
    .expect("configured Takedown rules profile must be valid")
}

#[cfg(feature = "server")]
fn install_server(app: &mut App, registration: &ModeRegistration, input: super::ModeInstallInput) {
    use crate::{
        map::ServerMapSelection,
        matchplay::{MatchModeSetup, TakedownModePlugin, TakedownRules},
    };
    let server = registration.server();
    app.insert_resource(MatchModeSetup {
        mode_definition_id: registration.definition_id,
        rules_revision: server.rules_revision,
    })
    .insert_resource(
        match (
            input.objective_target,
            input.takedown_recent_hostile_damage_credit_ticks,
        ) {
            (Some(target_score), Some(recent_hostile_damage_credit_ticks)) => TakedownRules {
                target_score,
                recent_hostile_damage_credit_ticks,
            }
            .validate()
            .expect("validated manifest Takedown policy"),
            (None, None)
                if input.profile == crate::config::MatchRulesProfile::ProcessVerification =>
            {
                takedown_rules_for_profile(input.profile)
            }
            _ => panic!("production Takedown requires one complete authored policy"),
        },
    )
    .insert_resource(ServerMapSelection::preset(server.default_map_preset))
    .add_plugins(TakedownModePlugin);
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    #[test]
    fn operator_rules_preserve_objective_projection() {
        let input = super::super::ModeRuleResolveInput {
            kills_to_win: 10,
            capture_seconds: 0,
            safe_health: 0,
            pressure_stages: 0,
            shards_to_win: 0,
            match_duration_ticks: 10_800,
            lifecycle: crate::matchplay::MatchLifecycleRules::default(),
            takedown_recent_hostile_damage_credit_ticks: 300,
            vault_break_critical_health_percent: 25,
            shard_rush: crate::matchplay::ShardRushPolicy::PROCESS_VERIFICATION,
        };
        assert_eq!(
            resolve_operator_rules(input).unwrap().rules_summary,
            crate::lobby::AdvertisedRulesSummary::Takedown {
                target_score: 10,
                active_limit_ticks: 10_800,
            }
        );
    }
}
