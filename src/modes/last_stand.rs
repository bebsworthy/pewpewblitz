//! Last Stand-owned registration, operator-rule resolution, and server composition.

use super::{ModeRegistration, ModeTopologyPolicy};
use crate::{config::GameMode, map::LAST_STAND_MODE_DEFINITION};
use bevy::prelude::{App, Plugin};

pub(super) const REGISTRATION: ModeRegistration = ModeRegistration {
    configured_mode: Some(GameMode::LastStand),
    key: "last-stand",
    definition_id: LAST_STAND_MODE_DEFINITION,
    topology: ModeTopologyPolicy::SoloSpawns,
    #[cfg(feature = "server")]
    server: Some(super::ServerModeProjection {
        rules_revision: crate::matchplay::LAST_STAND_RULES_REVISION,
        compatible_maps: super::CompatibleMapPolicy::ExactModeDefinition,
        default_map_preset: crate::map::SUNDER_RING_PRESET,
        routing_mode: brawler_routing::GameMode::LastStand,
        default_topology: super::ModeTeamTopology {
            team_count: crate::matchplay::LAST_STAND_FIGHTERS,
            maximum_participants_per_team: 1,
        },
        validate_operator_policy,
        resolve_operator_rules,
        install: install_server,
    }),
    #[cfg(feature = "client")]
    presentation: Some(super::ModePresentationProjection {
        selection_label: "Last Stand · Solo",
    }),
};

pub(super) struct LastStandModeRegistrationPlugin;

impl Plugin for LastStandModeRegistrationPlugin {
    fn build(&self, app: &mut App) {
        use super::registry::ModeRegistrationAppExt as _;
        app.try_register_mode(REGISTRATION)
            .expect("Last Stand mode registration is unique and bounded");
    }
}

#[cfg(feature = "server")]
#[allow(clippy::unnecessary_wraps)]
fn validate_operator_policy(_input: super::ModeOperatorPolicyInput) -> Result<(), &'static str> {
    Ok(())
}

/// Last Stand has no visible operator objective: the round ends on the last fighter standing
/// or the deadline, and the boundary schedule is code-owned (`LastStandRules::default`).
#[cfg(feature = "server")]
fn resolve_operator_rules(
    input: super::ModeRuleResolveInput,
) -> Result<super::ResolvedModeRuleProjection, super::ModeRuleResolutionError> {
    if input.kills_to_win != 0
        || input.capture_seconds != 0
        || input.safe_health != 0
        || input.pressure_stages != 0
        || input.shards_to_win != 0
    {
        return Err(super::ModeRuleResolutionError::MismatchedObjective);
    }
    Ok(super::ResolvedModeRuleProjection {
        objective_target: u16::from(crate::matchplay::LAST_STAND_FIGHTERS),
        rules_summary: crate::lobby::AdvertisedRulesSummary::LastStand {
            fighters: crate::matchplay::LAST_STAND_FIGHTERS,
            active_limit_ticks: input.match_duration_ticks,
        },
    })
}

#[cfg(feature = "server")]
fn install_server(app: &mut App, registration: &ModeRegistration, input: super::ModeInstallInput) {
    use crate::{
        map::ServerMapSelection,
        matchplay::{LastStandModePlugin, MatchModeSetup, last_stand_rules_for_profile},
    };
    let server = registration.server();
    app.insert_resource(MatchModeSetup {
        mode_definition_id: registration.definition_id,
        rules_revision: server.rules_revision,
    })
    .insert_resource(
        last_stand_rules_for_profile(input.profile)
            .validate()
            .expect("code-owned Last Stand rules are valid"),
    )
    .insert_resource(ServerMapSelection::preset(server.default_map_preset))
    .add_plugins(LastStandModePlugin);
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
            shards_to_win: 0,
            match_duration_ticks: 10_800,
            lifecycle: crate::matchplay::MatchLifecycleRules::default(),
            takedown_recent_hostile_damage_credit_ticks: 300,
            vault_break_critical_health_percent: 25,
            shard_rush: crate::matchplay::ShardRushPolicy::PRODUCTION,
        }
    }

    #[test]
    fn operator_rules_project_fighters_and_duration() {
        assert_eq!(
            resolve_operator_rules(input()).unwrap(),
            super::super::ResolvedModeRuleProjection {
                objective_target: 6,
                rules_summary: crate::lobby::AdvertisedRulesSummary::LastStand {
                    fighters: 6,
                    active_limit_ticks: 10_800,
                },
            }
        );
    }

    #[test]
    fn operator_rules_reject_visible_objectives() {
        for mismatched in [
            super::super::ModeRuleResolveInput {
                kills_to_win: 10,
                ..input()
            },
            super::super::ModeRuleResolveInput {
                shards_to_win: 8,
                ..input()
            },
        ] {
            assert_eq!(
                resolve_operator_rules(mismatched),
                Err(super::super::ModeRuleResolutionError::MismatchedObjective)
            );
        }
    }
}
