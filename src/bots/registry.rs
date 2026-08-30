//! Server-only Practice-bot behavior registration and plugin finalization.

use super::{
    behaviors::BehaviorRegistration,
    profile::{BotArbitrationPolicy, BotCatalogResource, MAX_BOT_BEHAVIOR_REGISTRATIONS},
};
use bevy::prelude::{App, Plugin, Resource};

#[derive(Resource, Default)]
struct BotBehaviorRegistryBuilder {
    registrations: Vec<BehaviorRegistration>,
}

impl BotBehaviorRegistryBuilder {
    fn register(&mut self, registration: BehaviorRegistration) -> Result<(), String> {
        if registration.id.0 == 0 {
            return Err("Practice bot behavior ID must be nonzero".into());
        }
        if self
            .registrations
            .iter()
            .any(|existing| existing.id == registration.id)
        {
            return Err(format!(
                "duplicate Practice bot behavior ID {}",
                registration.id.0
            ));
        }
        if self.registrations.len() >= MAX_BOT_BEHAVIOR_REGISTRATIONS {
            return Err("Practice bot behavior registry exceeds engine capacity".into());
        }
        self.registrations.push(registration);
        Ok(())
    }

    fn seal(mut self, policy: &BotArbitrationPolicy) -> Result<BotBehaviorRegistry, String> {
        policy.validate()?;
        self.registrations
            .sort_by_key(|registration| registration.id);
        if self.registrations.len() != policy.behaviors.len()
            || self
                .registrations
                .iter()
                .any(|registration| policy.behavior(registration.id).is_none())
            || policy.behaviors.iter().any(|behavior| {
                !self
                    .registrations
                    .iter()
                    .any(|registration| registration.id == behavior.id)
            })
        {
            return Err(
                "Practice bot handlers must exactly cover authored arbitration policy".into(),
            );
        }
        Ok(BotBehaviorRegistry {
            registrations: self.registrations,
        })
    }
}

#[derive(Resource, Clone)]
pub(super) struct BotBehaviorRegistry {
    registrations: Vec<BehaviorRegistration>,
}

impl BotBehaviorRegistry {
    pub(super) fn registrations(&self) -> &[BehaviorRegistration] {
        &self.registrations
    }
}

pub(super) trait BotBehaviorAppExt {
    fn try_register_bot_behavior(
        &mut self,
        registration: BehaviorRegistration,
    ) -> Result<&mut Self, String>;
}

impl BotBehaviorAppExt for App {
    fn try_register_bot_behavior(
        &mut self,
        registration: BehaviorRegistration,
    ) -> Result<&mut Self, String> {
        if !self
            .world()
            .contains_resource::<BotBehaviorRegistryBuilder>()
        {
            self.init_resource::<BotBehaviorRegistryBuilder>();
        }
        self.world_mut()
            .resource_mut::<BotBehaviorRegistryBuilder>()
            .register(registration)?;
        Ok(self)
    }
}

pub(super) struct BotBehaviorRegistryPlugin;

impl Plugin for BotBehaviorRegistryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BotCatalogResource>()
            .init_resource::<BotBehaviorRegistryBuilder>();
    }

    fn finish(&self, app: &mut App) {
        let builder = app
            .world_mut()
            .remove_resource::<BotBehaviorRegistryBuilder>()
            .expect("Practice bot registry builder exists until plugin finalization");
        let registry = {
            let catalog = app.world().resource::<BotCatalogResource>();
            builder
                .seal(&catalog.0.arbitration)
                .expect("Practice bot behavior registry matches authored policy")
        };
        app.insert_resource(registry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::bots::{
        behaviors::{BehaviorContext, BuiltInBotBehaviorsPlugin, CandidateBuffer},
        profile::{BotBehaviorId, BotCatalog},
    };

    fn contribute_nothing(
        _context: &BehaviorContext<'_>,
        _candidates: &mut CandidateBuffer,
        _base_score: u16,
    ) {
    }

    fn registration(id: BotBehaviorId) -> BehaviorRegistration {
        BehaviorRegistration::new(id, contribute_nothing)
    }

    fn builder_for(ids: impl IntoIterator<Item = BotBehaviorId>) -> BotBehaviorRegistryBuilder {
        let mut builder = BotBehaviorRegistryBuilder::default();
        for id in ids {
            builder.register(registration(id)).unwrap();
        }
        builder
    }

    #[test]
    fn builder_rejects_zero_duplicate_and_excess_registrations() {
        let mut builder = BotBehaviorRegistryBuilder::default();
        assert!(builder.register(registration(BotBehaviorId(0))).is_err());
        builder.register(registration(BotBehaviorId(77))).unwrap();
        assert!(builder.register(registration(BotBehaviorId(77))).is_err());
        for raw_id in 1..MAX_BOT_BEHAVIOR_REGISTRATIONS {
            builder
                .register(registration(BotBehaviorId(u16::try_from(raw_id).unwrap())))
                .unwrap();
        }
        assert!(builder.register(registration(BotBehaviorId(99))).is_err());
    }

    #[test]
    fn sealing_requires_exact_authored_handler_coverage_and_fallback() {
        let catalog = BotCatalog::embedded().unwrap();
        let registered_ids = catalog
            .arbitration
            .behaviors
            .iter()
            .map(|behavior| behavior.id)
            .collect::<Vec<_>>();

        assert!(
            builder_for(registered_ids.iter().copied().take(6))
                .seal(&catalog.arbitration)
                .is_err()
        );
        assert!(
            builder_for(
                registered_ids
                    .iter()
                    .copied()
                    .filter(|id| *id != BotBehaviorId::FALLBACK),
            )
            .seal(&catalog.arbitration)
            .is_err()
        );

        let mut with_extra = registered_ids.clone();
        with_extra.push(BotBehaviorId(77));
        assert!(builder_for(with_extra).seal(&catalog.arbitration).is_err());

        let sealed = builder_for(registered_ids.iter().copied().rev())
            .seal(&catalog.arbitration)
            .unwrap();
        assert!(
            sealed
                .registrations()
                .windows(2)
                .all(|pair| pair[0].id < pair[1].id)
        );

        let mut disabled_fallback = catalog.arbitration;
        disabled_fallback
            .behaviors
            .iter_mut()
            .find(|behavior| behavior.id == BotBehaviorId::FALLBACK)
            .unwrap()
            .enabled = false;
        assert!(
            builder_for(registered_ids)
                .seal(&disabled_fallback)
                .is_err()
        );
    }

    #[test]
    fn finalization_is_independent_of_plugin_build_order() {
        let mut app = App::new();
        app.add_plugins((BuiltInBotBehaviorsPlugin, BotBehaviorRegistryPlugin));

        crate::test_app::finalize(&mut app);

        assert_eq!(
            app.world()
                .resource::<BotBehaviorRegistry>()
                .registrations()
                .len(),
            7
        );
        assert!(
            !app.world()
                .contains_resource::<BotBehaviorRegistryBuilder>()
        );
    }
}
