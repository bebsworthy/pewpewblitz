//! Zombie-owned registration, operator-rule resolution, and server composition.

use super::{ModeRegistration, ModeTopologyPolicy};
use crate::{config::GameMode, map::ZOMBIE_MODE_DEFINITION};
use bevy::prelude::{App, Plugin};

pub(super) const REGISTRATION: ModeRegistration = ModeRegistration {
    configured_mode: Some(GameMode::Zombie),
    key: "zombie",
    definition_id: ZOMBIE_MODE_DEFINITION,
    topology: ModeTopologyPolicy::ZombieIngress,
    #[cfg(feature = "server")]
    server: Some(super::ServerModeProjection {
        rules_revision: crate::matchplay::ZOMBIE_RULES_REVISION,
        compatible_maps: super::CompatibleMapPolicy::ExactModeDefinition,
        default_map_preset: crate::map::ZOMBIE_YARD_PRESET,
        routing_mode: brawler_routing::GameMode::Zombie,
        validate_operator_policy,
        resolve_operator_rules,
        install: install_server,
    }),
    #[cfg(feature = "client")]
    presentation: Some(super::ModePresentationProjection {
        selection_label: "Zombie · Co-op PvE",
    }),
};

pub(super) struct ZombieModeRegistrationPlugin;

impl Plugin for ZombieModeRegistrationPlugin {
    fn build(&self, app: &mut App) {
        use super::registry::ModeRegistrationAppExt as _;
        app.try_register_mode(REGISTRATION)
            .expect("Zombie mode registration is unique and bounded");
    }
}

#[cfg(feature = "server")]
#[allow(clippy::unnecessary_wraps)]
fn validate_operator_policy(_input: super::ModeOperatorPolicyInput) -> Result<(), &'static str> {
    Ok(())
}

#[cfg(feature = "server")]
fn resolve_operator_rules(
    input: super::ModeRuleResolveInput,
) -> Result<super::ResolvedModeRuleProjection, super::ModeRuleResolutionError> {
    if input.pressure_stages == 0
        || input.kills_to_win != 0
        || input.capture_seconds != 0
        || input.safe_health != 0
    {
        return Err(super::ModeRuleResolutionError::MismatchedObjective);
    }
    Ok(super::ResolvedModeRuleProjection {
        objective_target: u16::from(input.pressure_stages),
        rules_summary: crate::lobby::AdvertisedRulesSummary::Zombie {
            pressure_stage_count: input.pressure_stages,
            active_limit_ticks: input.match_duration_ticks,
        },
    })
}

#[cfg(feature = "server")]
fn install_server(app: &mut App, registration: &ModeRegistration, input: super::ModeInstallInput) {
    use crate::{
        map::ServerMapSelection,
        matchplay::{MatchModeSetup, ZombieModePlugin, ZombieRules},
    };
    let stage_count = input
        .objective_target
        .and_then(|value| u8::try_from(value).ok())
        .unwrap_or(4);
    let server = registration.server();
    app.insert_resource(MatchModeSetup {
        mode_definition_id: registration.definition_id,
        rules_revision: server.rules_revision,
    })
    .insert_resource(
        ZombieRules {
            pressure_stage_count: stage_count,
            ..ZombieRules::default()
        }
        .validate()
        .expect("validated Zombie rules"),
    )
    .insert_resource(ServerMapSelection {
        preset_id: server.default_map_preset,
    })
    .add_plugins(ZombieModePlugin);
}

#[cfg(all(test, feature = "server"))]
mod tests {
    use super::*;

    #[test]
    fn operator_rules_project_hidden_pressure_stages_and_duration() {
        let projection = resolve_operator_rules(super::super::ModeRuleResolveInput {
            kills_to_win: 0,
            capture_seconds: 0,
            safe_health: 0,
            pressure_stages: 6,
            match_duration_ticks: 14_400,
            lifecycle: crate::matchplay::MatchLifecycleRules::default(),
            wipeout_recent_hostile_damage_credit_ticks: 300,
            heist_critical_health_percent: 25,
        })
        .unwrap();

        assert_eq!(projection.objective_target, 6);
        assert_eq!(
            projection.rules_summary,
            crate::lobby::AdvertisedRulesSummary::Zombie {
                pressure_stage_count: 6,
                active_limit_ticks: 14_400,
            }
        );
    }

    #[test]
    fn operator_rules_reject_visible_mode_objectives() {
        let error = resolve_operator_rules(super::super::ModeRuleResolveInput {
            kills_to_win: 1,
            capture_seconds: 0,
            safe_health: 0,
            pressure_stages: 4,
            match_duration_ticks: 10_800,
            lifecycle: crate::matchplay::MatchLifecycleRules::default(),
            wipeout_recent_hostile_damage_credit_ticks: 300,
            heist_critical_health_percent: 25,
        })
        .unwrap_err();

        assert_eq!(
            error,
            super::super::ModeRuleResolutionError::MismatchedObjective
        );
    }
}
