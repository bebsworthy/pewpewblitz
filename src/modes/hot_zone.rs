//! Hot Zone-owned mode registration, authored-rule resolution, and server composition.

use super::{ModeRegistration, ModeTopologyPolicy};
use crate::{config::GameMode, map::HOT_ZONE_MODE_DEFINITION};
use bevy::prelude::{App, Plugin};

pub(super) const REGISTRATION: ModeRegistration = ModeRegistration {
    configured_mode: Some(GameMode::HotZone),
    key: "hot-zone",
    definition_id: HOT_ZONE_MODE_DEFINITION,
    topology: ModeTopologyPolicy::HotZoneCircle,
    #[cfg(feature = "server")]
    server: Some(super::ServerModeProjection {
        rules_revision: crate::matchplay::HOT_ZONE_RULES_REVISION,
        compatible_maps: super::CompatibleMapPolicy::ExactModeDefinition,
        default_map_preset: crate::map::FEATURE_YARD_HOT_ZONE_PRESET,
        routing_mode: brawler_routing::GameMode::HotZone,
        validate_operator_policy,
        resolve_operator_rules,
        install: install_server,
    }),
    #[cfg(feature = "client")]
    presentation: Some(super::ModePresentationProjection {
        selection_label: "Hot Zone",
    }),
};

pub(super) struct HotZoneModeRegistrationPlugin;

impl Plugin for HotZoneModeRegistrationPlugin {
    fn build(&self, app: &mut App) {
        use super::registry::ModeRegistrationAppExt as _;
        app.try_register_mode(REGISTRATION)
            .expect("Hot Zone mode registration is unique and bounded");
    }
}

#[cfg(feature = "server")]
#[allow(
    clippy::unnecessary_wraps,
    reason = "all mode-owned policy validators share one fail-closed callback signature"
)]
fn validate_operator_policy(_input: super::ModeOperatorPolicyInput) -> Result<(), &'static str> {
    Ok(())
}

#[cfg(feature = "server")]
fn resolve_operator_rules(
    input: super::ModeRuleResolveInput,
) -> Result<super::ResolvedModeRuleProjection, super::ModeRuleResolutionError> {
    if input.kills_to_win != 0 || input.safe_health != 0 {
        return Err(super::ModeRuleResolutionError::MismatchedObjective);
    }
    let target_progress_ticks =
        crate::timing::simulation_ticks_from_seconds(u64::from(input.capture_seconds))
            .filter(|ticks| *ticks > 0)
            .ok_or(super::ModeRuleResolutionError::InvalidTiming)
            .and_then(|ticks| {
                u16::try_from(ticks)
                    .map_err(|_| super::ModeRuleResolutionError::CaptureDurationTooLong)
            })?;
    crate::matchplay::HotZoneRules {
        target_progress_ticks,
    }
    .validate_with(&input.lifecycle)
    .map_err(super::ModeRuleResolutionError::InvalidObjective)?;
    Ok(super::ResolvedModeRuleProjection {
        objective_target: target_progress_ticks,
        rules_summary: crate::lobby::AdvertisedRulesSummary::HotZone {
            target_progress_ticks,
            active_limit_ticks: input.match_duration_ticks,
        },
    })
}

#[cfg(feature = "server")]
fn install_server(app: &mut App, registration: &ModeRegistration, input: super::ModeInstallInput) {
    use crate::{
        map::ServerMapSelection,
        matchplay::{
            HotZoneModePlugin, HotZoneRules, MatchLifecycleRules, MatchModeSetup,
            hot_zone_rules_for_profile,
        },
    };
    let rules = input.objective_target.map_or_else(
        || match input.profile {
            crate::config::MatchRulesProfile::ProcessVerification => {
                hot_zone_rules_for_profile(input.profile)
            }
            crate::config::MatchRulesProfile::Production => {
                panic!("production Hot Zone requires an authored objective target")
            }
        },
        |target_progress_ticks| HotZoneRules {
            target_progress_ticks,
        },
    );
    let rules = rules
        .validate_with(app.world().resource::<MatchLifecycleRules>())
        .expect("validated manifest Hot Zone objective");
    let server = registration.server();
    app.insert_resource(MatchModeSetup {
        mode_definition_id: registration.definition_id,
        rules_revision: server.rules_revision,
    })
    .insert_resource(rules)
    .insert_resource(ServerMapSelection {
        preset_id: server.default_map_preset,
    })
    .add_plugins(HotZoneModePlugin);
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    fn input() -> super::super::ModeRuleResolveInput {
        super::super::ModeRuleResolveInput {
            kills_to_win: 0,
            capture_seconds: 30,
            safe_health: 0,
            match_duration_ticks: 10_800,
            lifecycle: crate::matchplay::MatchLifecycleRules::default(),
            wipeout_recent_hostile_damage_credit_ticks: 300,
            heist_critical_health_percent: 25,
        }
    }

    #[test]
    fn operator_rules_convert_authored_capture_seconds() {
        assert_eq!(
            resolve_operator_rules(input()).unwrap(),
            super::super::ResolvedModeRuleProjection {
                objective_target: 1_800,
                rules_summary: crate::lobby::AdvertisedRulesSummary::HotZone {
                    target_progress_ticks: 1_800,
                    active_limit_ticks: 10_800,
                },
            }
        );
    }

    #[test]
    fn operator_rules_distinguish_shape_timing_and_capacity_errors() {
        assert_eq!(
            resolve_operator_rules(super::super::ModeRuleResolveInput {
                kills_to_win: 1,
                ..input()
            }),
            Err(super::super::ModeRuleResolutionError::MismatchedObjective)
        );
        assert_eq!(
            resolve_operator_rules(super::super::ModeRuleResolveInput {
                capture_seconds: 0,
                ..input()
            }),
            Err(super::super::ModeRuleResolutionError::InvalidTiming)
        );
        assert_eq!(
            resolve_operator_rules(super::super::ModeRuleResolveInput {
                capture_seconds: u16::MAX,
                ..input()
            }),
            Err(super::super::ModeRuleResolutionError::CaptureDurationTooLong)
        );
    }
}
