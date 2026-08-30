//! Process-local game-mode descriptors.
//!
//! Stable wire enums and IDs remain in their owning modules. This registry only connects those
//! identities to local composition policy and optional consumers such as UI and Practice bots.

#[cfg(any(feature = "server", test))]
use crate::config::GameMode;
#[cfg(feature = "server")]
use crate::map::{
    FEATURE_YARD_HEIST_PRESET, FEATURE_YARD_HOT_ZONE_PRESET, FEATURE_YARD_WIPEOUT_PRESET,
    MapPresetId,
};
use crate::map::{
    HEIST_MODE_DEFINITION, HOT_ZONE_MODE_DEFINITION, ModeDefinitionId, WIPEOUT_MODE_DEFINITION,
};

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
    pub(crate) fn accepts(self, descriptor: &ModeDescriptor, map_mode: ModeDefinitionId) -> bool {
        match self {
            Self::ExactModeDefinition => descriptor.definition_id == map_mode,
        }
    }
}

#[cfg(feature = "client")]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ModePresentationProjection {
    pub(crate) selection_label: &'static str,
}

#[cfg(feature = "server")]
type ServerModeInstaller = fn(
    &mut bevy::prelude::App,
    &'static ModeDescriptor,
    crate::config::MatchRulesProfile,
    Option<u16>,
    Option<u64>,
    Option<u8>,
);

#[derive(Clone, Copy)]
pub(crate) struct ModeDescriptor {
    #[cfg(any(feature = "server", test))]
    pub(crate) mode: GameMode,
    #[cfg(any(feature = "server", test))]
    pub(crate) key: &'static str,
    pub(crate) definition_id: ModeDefinitionId,
    pub(crate) topology: ModeTopologyPolicy,
    #[cfg(feature = "server")]
    pub(crate) rules_revision: u16,
    #[cfg(feature = "server")]
    pub(crate) compatible_maps: CompatibleMapPolicy,
    #[cfg(feature = "server")]
    pub(crate) default_map_preset: MapPresetId,
    #[cfg(feature = "server")]
    pub(crate) routing_mode: brawler_routing::GameMode,
    #[cfg(feature = "client")]
    pub(crate) presentation: Option<ModePresentationProjection>,
    #[cfg(feature = "server")]
    install_server: ServerModeInstaller,
}

impl ModeDescriptor {
    pub(crate) const fn topology(self) -> ModeTopologyPolicy {
        self.topology
    }

    #[cfg(feature = "server")]
    pub(crate) fn accepts_map(self, map_mode: ModeDefinitionId) -> bool {
        self.compatible_maps.accepts(&self, map_mode)
    }

    #[cfg(feature = "server")]
    pub(crate) fn install_server(
        &'static self,
        app: &mut bevy::prelude::App,
        profile: crate::config::MatchRulesProfile,
        objective_target: Option<u16>,
        wipeout_recent_hostile_damage_credit_ticks: Option<u64>,
        heist_critical_health_percent: Option<u8>,
    ) {
        (self.install_server)(
            app,
            self,
            profile,
            objective_target,
            wipeout_recent_hostile_damage_credit_ticks,
            heist_critical_health_percent,
        );
    }
}

pub(crate) static MODE_DESCRIPTORS: [ModeDescriptor; 3] = [
    ModeDescriptor {
        #[cfg(any(feature = "server", test))]
        mode: GameMode::Wipeout,
        #[cfg(any(feature = "server", test))]
        key: "wipeout",
        definition_id: WIPEOUT_MODE_DEFINITION,
        topology: ModeTopologyPolicy::NoAnchors,
        #[cfg(feature = "server")]
        rules_revision: crate::matchplay::WIPEOUT_RULES_REVISION,
        #[cfg(feature = "server")]
        compatible_maps: CompatibleMapPolicy::ExactModeDefinition,
        #[cfg(feature = "server")]
        default_map_preset: FEATURE_YARD_WIPEOUT_PRESET,
        #[cfg(feature = "server")]
        routing_mode: brawler_routing::GameMode::Wipeout,
        #[cfg(feature = "client")]
        presentation: Some(ModePresentationProjection {
            selection_label: "Wipeout",
        }),
        #[cfg(feature = "server")]
        install_server: install_wipeout,
    },
    ModeDescriptor {
        #[cfg(any(feature = "server", test))]
        mode: GameMode::HotZone,
        #[cfg(any(feature = "server", test))]
        key: "hot-zone",
        definition_id: HOT_ZONE_MODE_DEFINITION,
        topology: ModeTopologyPolicy::HotZoneCircle,
        #[cfg(feature = "server")]
        rules_revision: crate::matchplay::HOT_ZONE_RULES_REVISION,
        #[cfg(feature = "server")]
        compatible_maps: CompatibleMapPolicy::ExactModeDefinition,
        #[cfg(feature = "server")]
        default_map_preset: FEATURE_YARD_HOT_ZONE_PRESET,
        #[cfg(feature = "server")]
        routing_mode: brawler_routing::GameMode::HotZone,
        #[cfg(feature = "client")]
        presentation: Some(ModePresentationProjection {
            selection_label: "Hot Zone",
        }),
        #[cfg(feature = "server")]
        install_server: install_hot_zone,
    },
    ModeDescriptor {
        #[cfg(any(feature = "server", test))]
        mode: GameMode::Heist,
        #[cfg(any(feature = "server", test))]
        key: "heist",
        definition_id: HEIST_MODE_DEFINITION,
        topology: ModeTopologyPolicy::MirroredHeistSafes,
        #[cfg(feature = "server")]
        rules_revision: crate::matchplay::HEIST_RULES_REVISION,
        #[cfg(feature = "server")]
        compatible_maps: CompatibleMapPolicy::ExactModeDefinition,
        #[cfg(feature = "server")]
        default_map_preset: FEATURE_YARD_HEIST_PRESET,
        #[cfg(feature = "server")]
        routing_mode: brawler_routing::GameMode::Heist,
        #[cfg(feature = "client")]
        presentation: Some(ModePresentationProjection {
            selection_label: "Heist",
        }),
        #[cfg(feature = "server")]
        install_server: install_heist,
    },
];

#[cfg(feature = "server")]
pub(crate) fn descriptor_for_mode(mode: GameMode) -> Option<&'static ModeDescriptor> {
    MODE_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.mode == mode)
}

#[cfg(feature = "server")]
pub(crate) fn descriptor_for_key(key: &str) -> Option<&'static ModeDescriptor> {
    MODE_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.key == key)
}

pub(crate) fn descriptor_for_definition(
    definition_id: ModeDefinitionId,
) -> Option<&'static ModeDescriptor> {
    MODE_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.definition_id == definition_id)
}

#[cfg(feature = "server")]
pub(crate) fn descriptor_for_routing_mode(
    routing_mode: brawler_routing::GameMode,
) -> Option<&'static ModeDescriptor> {
    MODE_DESCRIPTORS
        .iter()
        .find(|descriptor| descriptor.routing_mode == routing_mode)
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
fn install_wipeout(
    app: &mut bevy::prelude::App,
    descriptor: &'static ModeDescriptor,
    profile: crate::config::MatchRulesProfile,
    objective_target: Option<u16>,
    wipeout_recent_hostile_damage_credit_ticks: Option<u64>,
    _heist_critical_health_percent: Option<u8>,
) {
    use crate::{
        map::ServerMapSelection,
        matchplay::{MatchModeSetup, WipeoutModePlugin, WipeoutRules},
    };
    app.insert_resource(MatchModeSetup {
        mode_definition_id: descriptor.definition_id,
        rules_revision: descriptor.rules_revision,
    })
    .insert_resource(
        match (objective_target, wipeout_recent_hostile_damage_credit_ticks) {
            (Some(target_score), Some(recent_hostile_damage_credit_ticks)) => WipeoutRules {
                target_score,
                recent_hostile_damage_credit_ticks,
            }
            .validate()
            .expect("validated manifest Wipeout policy"),
            (None, None) if profile == crate::config::MatchRulesProfile::ProcessVerification => {
                wipeout_rules_for_profile(profile)
            }
            _ => panic!("production Wipeout requires one complete authored policy"),
        },
    )
    .insert_resource(ServerMapSelection {
        preset_id: descriptor.default_map_preset,
    })
    .add_plugins(WipeoutModePlugin);
}

#[cfg(feature = "server")]
fn install_hot_zone(
    app: &mut bevy::prelude::App,
    descriptor: &'static ModeDescriptor,
    profile: crate::config::MatchRulesProfile,
    objective_target: Option<u16>,
    _wipeout_recent_hostile_damage_credit_ticks: Option<u64>,
    _heist_critical_health_percent: Option<u8>,
) {
    use crate::{
        map::ServerMapSelection,
        matchplay::{
            HotZoneModePlugin, HotZoneRules, MatchLifecycleRules, MatchModeSetup,
            hot_zone_rules_for_profile,
        },
    };
    let rules = objective_target.map_or_else(
        || match profile {
            crate::config::MatchRulesProfile::ProcessVerification => {
                hot_zone_rules_for_profile(profile)
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
    app.insert_resource(MatchModeSetup {
        mode_definition_id: descriptor.definition_id,
        rules_revision: descriptor.rules_revision,
    })
    .insert_resource(rules)
    .insert_resource(ServerMapSelection {
        preset_id: descriptor.default_map_preset,
    })
    .add_plugins(HotZoneModePlugin);
}

#[cfg(feature = "server")]
fn install_heist(
    app: &mut bevy::prelude::App,
    descriptor: &'static ModeDescriptor,
    profile: crate::config::MatchRulesProfile,
    objective_target: Option<u16>,
    _wipeout_recent_hostile_damage_credit_ticks: Option<u64>,
    heist_critical_health_percent: Option<u8>,
) {
    use crate::{
        map::ServerMapSelection,
        matchplay::{HeistModePlugin, HeistRules, MatchModeSetup},
    };
    let rules = match (objective_target, heist_critical_health_percent) {
        (Some(safe_maximum_health), Some(critical_health_percent)) => HeistRules {
            safe_maximum_health,
            critical_health_percent,
        },
        (None, None) if profile == crate::config::MatchRulesProfile::ProcessVerification => {
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
    app.insert_resource(MatchModeSetup {
        mode_definition_id: descriptor.definition_id,
        rules_revision: descriptor.rules_revision,
    })
    .insert_resource(rules)
    .insert_resource(ServerMapSelection {
        preset_id: descriptor.default_map_preset,
    })
    .add_plugins(HeistModePlugin);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[derive(Clone, Copy, Debug, PartialEq, Eq)]
    enum RegistryError {
        DuplicateMode(GameMode),
        DuplicateKey(&'static str),
        DuplicateDefinition(ModeDefinitionId),
        #[cfg(feature = "server")]
        DuplicateRouting(brawler_routing::GameMode),
        MissingMode(GameMode),
    }

    fn validate_registry(descriptors: &[ModeDescriptor]) -> Result<(), RegistryError> {
        let mut modes = Vec::new();
        let mut keys = Vec::new();
        let mut definitions = Vec::new();
        #[cfg(feature = "server")]
        let mut routing_modes = Vec::new();
        for descriptor in descriptors {
            if modes.contains(&descriptor.mode) {
                return Err(RegistryError::DuplicateMode(descriptor.mode));
            }
            modes.push(descriptor.mode);
            if keys.contains(&descriptor.key) {
                return Err(RegistryError::DuplicateKey(descriptor.key));
            }
            keys.push(descriptor.key);
            if definitions.contains(&descriptor.definition_id) {
                return Err(RegistryError::DuplicateDefinition(descriptor.definition_id));
            }
            definitions.push(descriptor.definition_id);
            #[cfg(feature = "server")]
            if routing_modes.contains(&descriptor.routing_mode) {
                return Err(RegistryError::DuplicateRouting(descriptor.routing_mode));
            }
            #[cfg(feature = "server")]
            routing_modes.push(descriptor.routing_mode);
        }
        for mode in GameMode::ALL {
            if !modes.contains(&mode) {
                return Err(RegistryError::MissingMode(mode));
            }
        }
        Ok(())
    }

    #[test]
    fn registry_covers_each_stable_mode_and_projection() {
        assert_eq!(validate_registry(&MODE_DESCRIPTORS), Ok(()));
        for mode in GameMode::ALL {
            let descriptor = MODE_DESCRIPTORS
                .iter()
                .find(|descriptor| descriptor.mode == mode)
                .unwrap();
            assert_eq!(descriptor.key, mode.name());
            #[cfg(feature = "server")]
            assert_eq!(
                descriptor_for_key(descriptor.key).map(|entry| entry.mode),
                Some(mode)
            );
            #[cfg(feature = "server")]
            assert_eq!(
                descriptor_for_routing_mode(descriptor.routing_mode).map(|entry| entry.mode),
                Some(mode)
            );
            #[cfg(feature = "server")]
            assert!(descriptor.accepts_map(descriptor.definition_id));
            #[cfg(feature = "client")]
            assert!(descriptor.presentation.is_some());
            assert_eq!(
                descriptor_for_definition(descriptor.definition_id).map(|entry| entry.mode),
                Some(mode)
            );
        }
    }

    #[test]
    fn registry_rejects_duplicate_mode_key_and_definition() {
        let mut duplicate = MODE_DESCRIPTORS;
        duplicate[1].mode = duplicate[0].mode;
        assert!(matches!(
            validate_registry(&duplicate),
            Err(RegistryError::DuplicateMode(GameMode::Wipeout))
        ));

        duplicate = MODE_DESCRIPTORS;
        duplicate[1].key = duplicate[0].key;
        assert!(matches!(
            validate_registry(&duplicate),
            Err(RegistryError::DuplicateKey("wipeout"))
        ));

        duplicate = MODE_DESCRIPTORS;
        duplicate[1].definition_id = duplicate[0].definition_id;
        assert!(matches!(
            validate_registry(&duplicate),
            Err(RegistryError::DuplicateDefinition(WIPEOUT_MODE_DEFINITION))
        ));

        #[cfg(feature = "server")]
        {
            duplicate = MODE_DESCRIPTORS;
            duplicate[1].routing_mode = duplicate[0].routing_mode;
            assert!(matches!(
                validate_registry(&duplicate),
                Err(RegistryError::DuplicateRouting(
                    brawler_routing::GameMode::Wipeout
                ))
            ));
        }
    }

    #[test]
    fn registry_rejects_missing_supported_mode() {
        assert!(matches!(
            validate_registry(&MODE_DESCRIPTORS[..2]),
            Err(RegistryError::MissingMode(GameMode::Heist))
        ));
    }

    #[test]
    fn a_synthetic_descriptor_can_reuse_an_existing_topology_policy() {
        let mut synthetic = MODE_DESCRIPTORS[0];
        synthetic.definition_id = ModeDefinitionId(99);
        synthetic.key = "synthetic-anchorless";

        assert_eq!(synthetic.topology(), ModeTopologyPolicy::NoAnchors);
    }
}
