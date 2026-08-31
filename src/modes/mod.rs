//! Process-local game-mode registration and composition policy.
//!
//! Stable wire enums and IDs remain in their owning modules. Runtime applications seal mode
//! registrations during plugin finalization, while pure pre-`App` consumers use an immutable
//! catalog assembled from the exact same mode-owned registration constants.

use crate::{config::GameMode, map::ModeDefinitionId};
use bevy::prelude::{App, Plugin};

mod heist;
mod hot_zone;
mod registry;
mod wipeout;

#[cfg(feature = "client")]
pub(crate) use registry::ModeRegistry;
#[cfg(feature = "server")]
pub(crate) use registry::install_configured_server_mode;
pub(crate) use registry::{ModeCatalog, ModeRegistryPlugin};

#[cfg(feature = "server")]
use crate::{config::MatchRulesProfile, map::MapPresetId, matchplay::MatchLifecycleRules};

#[cfg(feature = "server")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum CompatibleMapPolicy {
    ExactModeDefinition,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModeTopologyPolicy {
    NoAnchors,
    HotZoneCircle,
    MirroredHeistSafes,
}

#[cfg(feature = "server")]
impl CompatibleMapPolicy {
    fn accepts(self, registration: &ModeRegistration, map_mode: ModeDefinitionId) -> bool {
        match self {
            Self::ExactModeDefinition => registration.definition_id == map_mode,
        }
    }
}

#[cfg(feature = "client")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ModePresentationProjection {
    pub(crate) selection_label: &'static str,
}

#[cfg(feature = "server")]
pub(crate) type ServerModeInstaller = fn(&mut App, &ModeRegistration, ModeInstallInput);

#[cfg(feature = "server")]
pub(crate) type OperatorRuleResolver =
    fn(ModeRuleResolveInput) -> Result<ResolvedModeRuleProjection, ModeRuleResolutionError>;

#[cfg(feature = "server")]
pub(crate) type OperatorPolicyValidator = fn(ModeOperatorPolicyInput) -> Result<(), &'static str>;

#[cfg(feature = "server")]
#[derive(Clone, Copy)]
pub(crate) struct ServerModeProjection {
    pub(crate) rules_revision: u16,
    pub(crate) compatible_maps: CompatibleMapPolicy,
    pub(crate) default_map_preset: MapPresetId,
    pub(crate) routing_mode: brawler_routing::GameMode,
    pub(crate) validate_operator_policy: OperatorPolicyValidator,
    pub(crate) resolve_operator_rules: OperatorRuleResolver,
    install: ServerModeInstaller,
}

#[derive(Clone, Copy)]
pub(crate) struct ModeRegistration {
    pub(crate) configured_mode: Option<GameMode>,
    pub(crate) key: &'static str,
    pub(crate) definition_id: ModeDefinitionId,
    pub(crate) topology: ModeTopologyPolicy,
    #[cfg(feature = "server")]
    pub(crate) server: Option<ServerModeProjection>,
    #[cfg(feature = "client")]
    pub(crate) presentation: Option<ModePresentationProjection>,
}

impl ModeRegistration {
    pub(crate) const fn topology(self) -> ModeTopologyPolicy {
        self.topology
    }

    #[cfg(feature = "server")]
    pub(crate) fn accepts_map(self, map_mode: ModeDefinitionId) -> bool {
        self.server
            .expect("routed modes have a server projection")
            .compatible_maps
            .accepts(&self, map_mode)
    }

    #[cfg(feature = "server")]
    pub(crate) fn server(self) -> ServerModeProjection {
        self.server.expect("routed modes have a server projection")
    }

    #[cfg(feature = "server")]
    pub(crate) fn resolve_operator_rules(
        self,
        input: ModeRuleResolveInput,
    ) -> Result<ResolvedModeRuleProjection, ModeRuleResolutionError> {
        (self.server().resolve_operator_rules)(input)
    }

    #[cfg(feature = "server")]
    pub(crate) fn validate_operator_policy(
        self,
        input: ModeOperatorPolicyInput,
    ) -> Result<(), &'static str> {
        (self.server().validate_operator_policy)(input)
    }

    #[cfg(feature = "server")]
    pub(crate) fn install_server(self, app: &mut App, input: ModeInstallInput) {
        (self.server().install)(app, &self, input);
    }
}

#[cfg(feature = "server")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ModeInstallInput {
    pub(crate) profile: MatchRulesProfile,
    pub(crate) objective_target: Option<u16>,
    pub(crate) wipeout_recent_hostile_damage_credit_ticks: Option<u64>,
    pub(crate) heist_critical_health_percent: Option<u8>,
}

#[cfg(feature = "server")]
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct ModeRuleResolveInput {
    pub(crate) kills_to_win: u16,
    pub(crate) capture_seconds: u16,
    pub(crate) safe_health: u16,
    pub(crate) match_duration_ticks: u64,
    pub(crate) lifecycle: MatchLifecycleRules,
    pub(crate) wipeout_recent_hostile_damage_credit_ticks: u64,
    pub(crate) heist_critical_health_percent: u8,
}

#[cfg(feature = "server")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ModeOperatorPolicyInput {
    pub(crate) wipeout_recent_hostile_damage_credit_ticks: u64,
    pub(crate) heist_critical_health_percent: u8,
}

#[cfg(feature = "server")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ResolvedModeRuleProjection {
    pub(crate) objective_target: u16,
    pub(crate) rules_summary: crate::lobby::AdvertisedRulesSummary,
}

#[cfg(feature = "server")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ModeRuleResolutionError {
    MismatchedObjective,
    InvalidTiming,
    CaptureDurationTooLong,
    InvalidObjective(&'static str),
}

pub(crate) struct BuiltInModeRegistrationsPlugin;

impl Plugin for BuiltInModeRegistrationsPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins((
            wipeout::WipeoutModeRegistrationPlugin,
            hot_zone::HotZoneModeRegistrationPlugin,
            heist::HeistModeRegistrationPlugin,
        ));
    }
}

pub(crate) fn builtin_mode_catalog() -> &'static ModeCatalog {
    registry::builtin_catalog([
        wipeout::REGISTRATION,
        hot_zone::REGISTRATION,
        heist::REGISTRATION,
    ])
}

#[cfg(all(test, feature = "server"))]
pub(crate) use wipeout::wipeout_rules_for_profile;

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn built_in_catalog_covers_each_stable_mode_and_projection() {
        let catalog = builtin_mode_catalog();
        assert_eq!(catalog.registrations().len(), GameMode::ALL.len());
        for mode in GameMode::ALL {
            let registration = catalog.descriptor_for_mode(mode).unwrap();
            assert_eq!(registration.key, mode.name());
            assert_eq!(
                catalog
                    .descriptor_for_definition(registration.definition_id)
                    .and_then(|entry| entry.configured_mode),
                Some(mode)
            );
            #[cfg(feature = "server")]
            {
                let server = registration.server();
                assert_eq!(
                    catalog
                        .descriptor_for_routing_mode(server.routing_mode)
                        .and_then(|entry| entry.configured_mode),
                    Some(mode)
                );
                assert!(registration.accepts_map(registration.definition_id));
            }
            #[cfg(feature = "client")]
            assert!(registration.presentation.is_some());
        }
    }
}
