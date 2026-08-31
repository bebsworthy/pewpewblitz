//! Bounded process-local mode registration and immutable lookup surfaces.

use super::ModeRegistration;
use crate::{config::GameMode, map::ModeDefinitionId};
use bevy::prelude::{App, Plugin, Resource};
use std::{fmt, sync::OnceLock};

pub(crate) const MAX_MODE_REGISTRATIONS: usize = 16;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum ModeRegistryError {
    RegistryPluginMissing,
    RegistrySealed,
    CapacityExceeded,
    DuplicateDefinition(ModeDefinitionId),
    DuplicateKey(&'static str),
    DuplicateConfiguredMode(GameMode),
    #[cfg(feature = "server")]
    DuplicateRoutingMode(brawler_routing::GameMode),
    MissingBuiltInMode(GameMode),
    #[cfg(feature = "server")]
    MissingServerProjection(GameMode),
    #[cfg(feature = "client")]
    MissingClientProjection(GameMode),
}

impl fmt::Display for ModeRegistryError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "{self:?}")
    }
}

#[derive(Resource, Default)]
struct ModeRegistryBuilder {
    registrations: Vec<ModeRegistration>,
}

impl ModeRegistryBuilder {
    fn register(&mut self, registration: ModeRegistration) -> Result<(), ModeRegistryError> {
        if self.registrations.len() >= MAX_MODE_REGISTRATIONS {
            return Err(ModeRegistryError::CapacityExceeded);
        }
        if self
            .registrations
            .iter()
            .any(|existing| existing.definition_id == registration.definition_id)
        {
            return Err(ModeRegistryError::DuplicateDefinition(
                registration.definition_id,
            ));
        }
        if self
            .registrations
            .iter()
            .any(|existing| existing.key == registration.key)
        {
            return Err(ModeRegistryError::DuplicateKey(registration.key));
        }
        if let Some(mode) = registration.configured_mode
            && self
                .registrations
                .iter()
                .any(|existing| existing.configured_mode == Some(mode))
        {
            return Err(ModeRegistryError::DuplicateConfiguredMode(mode));
        }
        #[cfg(feature = "server")]
        if let Some(routing_mode) = registration
            .server
            .map(|projection| projection.routing_mode)
            && self.registrations.iter().any(|existing| {
                existing.server.map(|projection| projection.routing_mode) == Some(routing_mode)
            })
        {
            return Err(ModeRegistryError::DuplicateRoutingMode(routing_mode));
        }
        self.registrations.push(registration);
        Ok(())
    }

    fn seal(mut self) -> Result<ModeCatalog, ModeRegistryError> {
        for mode in GameMode::ALL {
            let registration = self
                .registrations
                .iter()
                .find(|registration| registration.configured_mode == Some(mode))
                .ok_or(ModeRegistryError::MissingBuiltInMode(mode))?;
            #[cfg(feature = "server")]
            if registration.server.is_none() {
                return Err(ModeRegistryError::MissingServerProjection(mode));
            }
            #[cfg(feature = "client")]
            if registration.presentation.is_none() {
                return Err(ModeRegistryError::MissingClientProjection(mode));
            }
        }
        self.registrations.sort_by(|left, right| {
            left.definition_id
                .cmp(&right.definition_id)
                .then_with(|| left.key.cmp(right.key))
        });
        Ok(ModeCatalog {
            registrations: self.registrations.into_boxed_slice(),
        })
    }
}

#[derive(Clone)]
pub(crate) struct ModeCatalog {
    registrations: Box<[ModeRegistration]>,
}

impl ModeCatalog {
    #[cfg(any(feature = "server", test))]
    pub(crate) fn registrations(&self) -> &[ModeRegistration] {
        &self.registrations
    }

    #[cfg(any(feature = "server", test))]
    pub(crate) fn descriptor_for_mode(&self, mode: GameMode) -> Option<&ModeRegistration> {
        self.registrations
            .iter()
            .find(|registration| registration.configured_mode == Some(mode))
    }

    #[cfg(feature = "server")]
    pub(crate) fn descriptor_for_key(&self, key: &str) -> Option<&ModeRegistration> {
        self.registrations
            .iter()
            .find(|registration| registration.key == key)
    }

    pub(crate) fn descriptor_for_definition(
        &self,
        definition_id: ModeDefinitionId,
    ) -> Option<&ModeRegistration> {
        self.registrations
            .iter()
            .find(|registration| registration.definition_id == definition_id)
    }

    #[cfg(feature = "server")]
    pub(crate) fn descriptor_for_routing_mode(
        &self,
        routing_mode: brawler_routing::GameMode,
    ) -> Option<&ModeRegistration> {
        self.registrations.iter().find(|registration| {
            registration
                .server
                .map(|projection| projection.routing_mode)
                == Some(routing_mode)
        })
    }
}

#[derive(Resource, Clone)]
#[cfg_attr(
    not(feature = "client"),
    allow(
        dead_code,
        reason = "all roles seal the same registry; server validates composition without runtime lookup"
    )
)]
pub(crate) struct ModeRegistry(ModeCatalog);

impl ModeRegistry {
    #[cfg(test)]
    pub(crate) fn registrations(&self) -> &[ModeRegistration] {
        self.0.registrations()
    }

    #[cfg(any(feature = "client", test))]
    pub(crate) fn descriptor_for_definition(
        &self,
        definition_id: ModeDefinitionId,
    ) -> Option<&ModeRegistration> {
        self.0.descriptor_for_definition(definition_id)
    }
}

pub(crate) trait ModeRegistrationAppExt {
    fn try_register_mode(
        &mut self,
        registration: ModeRegistration,
    ) -> Result<&mut Self, ModeRegistryError>;
}

impl ModeRegistrationAppExt for App {
    fn try_register_mode(
        &mut self,
        registration: ModeRegistration,
    ) -> Result<&mut Self, ModeRegistryError> {
        let sealed = self.world().contains_resource::<ModeRegistry>();
        let Some(mut builder) = self.world_mut().get_resource_mut::<ModeRegistryBuilder>() else {
            return Err(if sealed {
                ModeRegistryError::RegistrySealed
            } else {
                ModeRegistryError::RegistryPluginMissing
            });
        };
        builder.register(registration)?;
        Ok(self)
    }
}

pub(crate) struct ModeRegistryPlugin;

impl Plugin for ModeRegistryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ModeRegistryBuilder>();
    }

    fn finish(&self, app: &mut App) {
        let builder = app
            .world_mut()
            .remove_resource::<ModeRegistryBuilder>()
            .expect("mode registry builder exists until plugin finalization");
        let catalog = builder
            .seal()
            .expect("mode registrations form one complete bounded catalog");
        app.insert_resource(ModeRegistry(catalog));
    }
}

pub(super) fn builtin_catalog(
    registrations: impl IntoIterator<Item = ModeRegistration>,
) -> &'static ModeCatalog {
    static CATALOG: OnceLock<ModeCatalog> = OnceLock::new();
    CATALOG.get_or_init(|| {
        let mut builder = ModeRegistryBuilder::default();
        for registration in registrations {
            builder
                .register(registration)
                .expect("built-in mode registrations are unique and bounded");
        }
        builder
            .seal()
            .expect("built-in mode registrations cover every configured mode")
    })
}

#[cfg(feature = "server")]
pub(crate) fn install_configured_server_mode(
    app: &mut App,
    mode: GameMode,
    input: super::ModeInstallInput,
) {
    let registration = app
        .world()
        .resource::<ModeRegistryBuilder>()
        .registrations
        .iter()
        .find(|registration| registration.configured_mode == Some(mode))
        .copied()
        .expect("every configured game mode is registered before server composition");
    registration.install_server(app, input);
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::modes::{
        BuiltInModeRegistrationsPlugin, ModeTopologyPolicy, heist, hot_zone, wipeout,
    };

    #[test]
    fn finalization_sorts_independently_of_registration_order() {
        let mut normal = App::new();
        normal.add_plugins((ModeRegistryPlugin, BuiltInModeRegistrationsPlugin));
        normal.finish();

        let mut reversed = App::new();
        reversed.add_plugins((
            ModeRegistryPlugin,
            heist::HeistModeRegistrationPlugin,
            hot_zone::HotZoneModeRegistrationPlugin,
            wipeout::WipeoutModeRegistrationPlugin,
        ));
        reversed.finish();

        let ids = |app: &App| {
            app.world()
                .resource::<ModeRegistry>()
                .registrations()
                .iter()
                .map(|registration| registration.definition_id)
                .collect::<Vec<_>>()
        };
        assert_eq!(ids(&normal), ids(&reversed));
    }

    const SYNTHETIC: ModeRegistration = ModeRegistration {
        configured_mode: None,
        key: "synthetic-anchorless",
        definition_id: ModeDefinitionId(99),
        topology: ModeTopologyPolicy::NoAnchors,
        #[cfg(feature = "server")]
        server: None,
        #[cfg(feature = "client")]
        presentation: None,
    };

    struct SyntheticModePlugin;

    impl Plugin for SyntheticModePlugin {
        fn build(&self, app: &mut App) {
            app.try_register_mode(SYNTHETIC)
                .expect("synthetic mode registration is valid");
        }
    }

    #[test]
    fn local_only_plugin_extends_registry_without_routed_identity() {
        let mut app = App::new();
        app.add_plugins((
            ModeRegistryPlugin,
            BuiltInModeRegistrationsPlugin,
            SyntheticModePlugin,
        ));
        app.finish();

        let registry = app.world().resource::<ModeRegistry>();
        assert_eq!(
            registry
                .descriptor_for_definition(ModeDefinitionId(99))
                .map(|registration| registration.topology()),
            Some(ModeTopologyPolicy::NoAnchors)
        );
        assert_eq!(
            app.try_register_mode(SYNTHETIC).map(|_| ()),
            Err(ModeRegistryError::RegistrySealed)
        );
    }

    #[test]
    fn registration_requires_registry_plugin() {
        let mut app = App::new();
        assert_eq!(
            app.try_register_mode(SYNTHETIC).map(|_| ()),
            Err(ModeRegistryError::RegistryPluginMissing)
        );
    }

    #[test]
    fn builder_rejects_duplicates_capacity_and_missing_builtins() {
        let mut duplicate = ModeRegistryBuilder::default();
        duplicate.register(wipeout::REGISTRATION).unwrap();
        assert!(matches!(
            duplicate.register(wipeout::REGISTRATION),
            Err(ModeRegistryError::DuplicateDefinition(_))
        ));

        let mut duplicate_key = ModeRegistryBuilder::default();
        duplicate_key.register(wipeout::REGISTRATION).unwrap();
        let mut duplicate_key_registration = hot_zone::REGISTRATION;
        duplicate_key_registration.key = wipeout::REGISTRATION.key;
        assert_eq!(
            duplicate_key.register(duplicate_key_registration),
            Err(ModeRegistryError::DuplicateKey("wipeout"))
        );

        let mut duplicate_mode = ModeRegistryBuilder::default();
        duplicate_mode.register(wipeout::REGISTRATION).unwrap();
        let mut duplicate_mode_registration = hot_zone::REGISTRATION;
        duplicate_mode_registration.configured_mode = Some(GameMode::Wipeout);
        assert_eq!(
            duplicate_mode.register(duplicate_mode_registration),
            Err(ModeRegistryError::DuplicateConfiguredMode(
                GameMode::Wipeout
            ))
        );

        #[cfg(feature = "server")]
        {
            let mut duplicate_routing = ModeRegistryBuilder::default();
            duplicate_routing.register(wipeout::REGISTRATION).unwrap();
            let mut duplicate_routing_registration = hot_zone::REGISTRATION;
            duplicate_routing_registration
                .server
                .as_mut()
                .unwrap()
                .routing_mode = brawler_routing::GameMode::Wipeout;
            assert_eq!(
                duplicate_routing.register(duplicate_routing_registration),
                Err(ModeRegistryError::DuplicateRoutingMode(
                    brawler_routing::GameMode::Wipeout
                ))
            );
        }

        let mut capacity = ModeRegistryBuilder::default();
        for raw_id in 1..=MAX_MODE_REGISTRATIONS {
            capacity
                .register(ModeRegistration {
                    configured_mode: None,
                    key: Box::leak(format!("synthetic-{raw_id}").into_boxed_str()),
                    definition_id: ModeDefinitionId(u16::try_from(raw_id + 100).unwrap()),
                    topology: ModeTopologyPolicy::NoAnchors,
                    #[cfg(feature = "server")]
                    server: None,
                    #[cfg(feature = "client")]
                    presentation: None,
                })
                .unwrap();
        }
        assert_eq!(
            capacity.register(SYNTHETIC),
            Err(ModeRegistryError::CapacityExceeded)
        );

        let mut missing = ModeRegistryBuilder::default();
        missing.register(wipeout::REGISTRATION).unwrap();
        assert!(matches!(
            missing.seal(),
            Err(ModeRegistryError::MissingBuiltInMode(GameMode::HotZone))
        ));
    }

    #[cfg(feature = "server")]
    #[test]
    fn sealing_rejects_a_configured_mode_without_server_projection() {
        let mut builder = ModeRegistryBuilder::default();
        let mut wipeout = wipeout::REGISTRATION;
        wipeout.server = None;
        for registration in [wipeout, hot_zone::REGISTRATION, heist::REGISTRATION] {
            builder.register(registration).unwrap();
        }
        assert!(matches!(
            builder.seal(),
            Err(ModeRegistryError::MissingServerProjection(
                GameMode::Wipeout
            ))
        ));
    }

    #[cfg(feature = "client")]
    #[test]
    fn sealing_rejects_a_configured_mode_without_client_projection() {
        let mut builder = ModeRegistryBuilder::default();
        let mut wipeout = wipeout::REGISTRATION;
        wipeout.presentation = None;
        for registration in [wipeout, hot_zone::REGISTRATION, heist::REGISTRATION] {
            builder.register(registration).unwrap();
        }
        assert!(matches!(
            builder.seal(),
            Err(ModeRegistryError::MissingClientProjection(
                GameMode::Wipeout
            ))
        ));
    }

    #[cfg(feature = "server")]
    #[test]
    fn selected_server_installation_uses_one_matching_plugin_before_seal() {
        use crate::{
            config::MatchRulesProfile,
            map::ServerMapSelection,
            matchplay::{
                HeistModePlugin, HeistRules, HotZoneModePlugin, HotZoneRules, MatchLifecycleRules,
                MatchModeSetup, WipeoutModePlugin, WipeoutRules,
            },
        };

        let cases = [
            (GameMode::Wipeout, 10, Some(300), None),
            (GameMode::HotZone, 1_800, None, None),
            (GameMode::Heist, 2_000, None, Some(25)),
        ];
        for (mode, objective_target, wipeout_credit, heist_critical) in cases {
            let mut app = App::new();
            app.insert_resource(MatchLifecycleRules::default())
                .add_plugins((ModeRegistryPlugin, BuiltInModeRegistrationsPlugin));
            install_configured_server_mode(
                &mut app,
                mode,
                super::super::ModeInstallInput {
                    profile: MatchRulesProfile::Production,
                    objective_target: Some(objective_target),
                    wipeout_recent_hostile_damage_credit_ticks: wipeout_credit,
                    heist_critical_health_percent: heist_critical,
                },
            );

            let registration = super::super::builtin_mode_catalog()
                .descriptor_for_mode(mode)
                .unwrap();
            let setup = app.world().resource::<MatchModeSetup>();
            assert_eq!(setup.mode_definition_id, registration.definition_id);
            assert_eq!(setup.rules_revision, registration.server().rules_revision);
            assert_eq!(
                app.world().resource::<ServerMapSelection>().preset_id,
                registration.server().default_map_preset
            );
            assert_eq!(
                app.is_plugin_added::<WipeoutModePlugin>(),
                mode == GameMode::Wipeout
            );
            assert_eq!(
                app.is_plugin_added::<HotZoneModePlugin>(),
                mode == GameMode::HotZone
            );
            assert_eq!(
                app.is_plugin_added::<HeistModePlugin>(),
                mode == GameMode::Heist
            );
            match mode {
                GameMode::Wipeout => assert_eq!(
                    *app.world().resource::<WipeoutRules>(),
                    WipeoutRules {
                        target_score: objective_target,
                        recent_hostile_damage_credit_ticks: wipeout_credit.unwrap(),
                    }
                ),
                GameMode::HotZone => assert_eq!(
                    app.world().resource::<HotZoneRules>().target_progress_ticks,
                    objective_target
                ),
                GameMode::Heist => assert_eq!(
                    *app.world().resource::<HeistRules>(),
                    HeistRules {
                        safe_maximum_health: objective_target,
                        critical_health_percent: heist_critical.unwrap(),
                    }
                ),
            }

            app.finish();
            assert!(!app.world().contains_resource::<ModeRegistryBuilder>());
            assert!(app.world().contains_resource::<ModeRegistry>());
        }
    }
}
