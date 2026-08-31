//! Wipeout-owned mode registration, authored-rule resolution, and server composition.

use super::{ModeRegistration, ModeTopologyPolicy};
use crate::{config::GameMode, map::WIPEOUT_MODE_DEFINITION};
use bevy::prelude::{App, Plugin};

pub(super) const REGISTRATION: ModeRegistration = ModeRegistration {
    configured_mode: Some(GameMode::Wipeout),
    key: "wipeout",
    definition_id: WIPEOUT_MODE_DEFINITION,
    topology: ModeTopologyPolicy::NoAnchors,
    #[cfg(feature = "server")]
    server: Some(super::ServerModeProjection {
        rules_revision: crate::matchplay::WIPEOUT_RULES_REVISION,
        compatible_maps: super::CompatibleMapPolicy::ExactModeDefinition,
        default_map_preset: crate::map::FEATURE_YARD_WIPEOUT_PRESET,
        routing_mode: brawler_routing::GameMode::Wipeout,
        validate_operator_policy,
        resolve_operator_rules,
        install: install_server,
    }),
    #[cfg(feature = "client")]
    presentation: Some(super::ModePresentationProjection {
        selection_label: "Wipeout",
    }),
};

pub(super) struct WipeoutModeRegistrationPlugin;

impl Plugin for WipeoutModeRegistrationPlugin {
    fn build(&self, app: &mut App) {
        use super::registry::ModeRegistrationAppExt as _;
        app.try_register_mode(REGISTRATION)
            .expect("Wipeout mode registration is unique and bounded");
    }
}

#[cfg(feature = "server")]
fn validate_operator_policy(input: super::ModeOperatorPolicyInput) -> Result<(), &'static str> {
    crate::matchplay::WipeoutRules {
        target_score: 1,
        recent_hostile_damage_credit_ticks: input.wipeout_recent_hostile_damage_credit_ticks,
    }
    .validate()
    .map(|_| ())
}

#[cfg(feature = "server")]
fn resolve_operator_rules(
    input: super::ModeRuleResolveInput,
) -> Result<super::ResolvedModeRuleProjection, super::ModeRuleResolutionError> {
    if input.capture_seconds != 0 || input.safe_health != 0 {
        return Err(super::ModeRuleResolutionError::MismatchedObjective);
    }
    let target_score = input.kills_to_win;
    crate::matchplay::WipeoutRules {
        target_score,
        recent_hostile_damage_credit_ticks: input.wipeout_recent_hostile_damage_credit_ticks,
    }
    .validate()
    .map_err(super::ModeRuleResolutionError::InvalidObjective)?;
    Ok(super::ResolvedModeRuleProjection {
        objective_target: target_score,
        rules_summary: crate::lobby::AdvertisedRulesSummary::Wipeout {
            target_score,
            active_limit_ticks: input.match_duration_ticks,
        },
    })
}

#[cfg(feature = "server")]
pub(crate) fn wipeout_rules_for_profile(
    profile: crate::config::MatchRulesProfile,
) -> crate::matchplay::WipeoutRules {
    use crate::matchplay::WipeoutRules;
    match profile {
        crate::config::MatchRulesProfile::Production => {
            panic!("production Wipeout requires authored policy")
        }
        crate::config::MatchRulesProfile::ProcessVerification => WipeoutRules {
            target_score: 10,
            recent_hostile_damage_credit_ticks: 300,
        },
    }
    .validate()
    .expect("configured Wipeout rules profile must be valid")
}

#[cfg(feature = "server")]
fn install_server(app: &mut App, registration: &ModeRegistration, input: super::ModeInstallInput) {
    use crate::{
        map::ServerMapSelection,
        matchplay::{MatchModeSetup, WipeoutModePlugin, WipeoutRules},
    };
    let server = registration.server();
    app.insert_resource(MatchModeSetup {
        mode_definition_id: registration.definition_id,
        rules_revision: server.rules_revision,
    })
    .insert_resource(
        match (
            input.objective_target,
            input.wipeout_recent_hostile_damage_credit_ticks,
        ) {
            (Some(target_score), Some(recent_hostile_damage_credit_ticks)) => WipeoutRules {
                target_score,
                recent_hostile_damage_credit_ticks,
            }
            .validate()
            .expect("validated manifest Wipeout policy"),
            (None, None)
                if input.profile == crate::config::MatchRulesProfile::ProcessVerification =>
            {
                wipeout_rules_for_profile(input.profile)
            }
            _ => panic!("production Wipeout requires one complete authored policy"),
        },
    )
    .insert_resource(ServerMapSelection {
        preset_id: server.default_map_preset,
    })
    .add_plugins(WipeoutModePlugin);
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
            match_duration_ticks: 10_800,
            lifecycle: crate::matchplay::MatchLifecycleRules::default(),
            wipeout_recent_hostile_damage_credit_ticks: 300,
            heist_critical_health_percent: 25,
        };
        assert_eq!(
            resolve_operator_rules(input).unwrap().rules_summary,
            crate::lobby::AdvertisedRulesSummary::Wipeout {
                target_score: 10,
                active_limit_ticks: 10_800,
            }
        );
    }
}
