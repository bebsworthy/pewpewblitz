//! Plugin-populated world-object terminal reaction registration.

use std::collections::{BTreeSet, VecDeque};

use bevy::prelude::{App, Plugin, Resource, World};

use super::object_authority::WorldObjectTerminalPlan;

pub(super) const MAX_TERMINAL_REACTION_REGISTRATIONS: usize = 16;

pub(crate) type TerminalReactionHandler =
    for<'world, 'queue> fn(&WorldObjectTerminalPlan, &mut TerminalReactionContext<'world, 'queue>);

#[derive(Clone, Copy)]
pub(crate) struct TerminalReactionRegistration {
    id: crate::map::TerminalReactionId,
    semantics: TerminalReactionSemantics,
    handler: TerminalReactionHandler,
}

impl TerminalReactionRegistration {
    pub(crate) const fn new(
        id: crate::map::TerminalReactionId,
        semantics: TerminalReactionSemantics,
        handler: TerminalReactionHandler,
    ) -> Self {
        Self {
            id,
            semantics,
            handler,
        }
    }
}

/// Process-local capabilities projected by a terminal-reaction plugin onto each authored
/// damageable object that uses the reaction. Consumers observe these semantics instead of
/// recognizing concrete map assets.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct TerminalReactionSemantics {
    pub(crate) hazardous: bool,
    pub(crate) valuable: bool,
}

impl TerminalReactionSemantics {
    pub(crate) const HAZARDOUS: Self = Self {
        hazardous: true,
        valuable: false,
    };
    pub(crate) const VALUABLE: Self = Self {
        hazardous: false,
        valuable: true,
    };
}

#[derive(Resource, Default)]
struct TerminalReactionRegistryBuilder {
    registrations: Vec<TerminalReactionRegistration>,
}

impl TerminalReactionRegistryBuilder {
    fn register(&mut self, registration: TerminalReactionRegistration) -> Result<(), String> {
        if self
            .registrations
            .iter()
            .any(|existing| existing.id == registration.id)
        {
            return Err(format!(
                "duplicate world-object terminal reaction {}",
                registration.id.get()
            ));
        }
        if self.registrations.len() >= MAX_TERMINAL_REACTION_REGISTRATIONS {
            return Err("world-object terminal reaction registry exceeds engine capacity".into());
        }
        self.registrations.push(registration);
        Ok(())
    }

    fn seal(
        mut self,
        catalog: &crate::map::MapContentCatalog,
    ) -> Result<TerminalReactionRegistry, String> {
        self.registrations
            .sort_by_key(|registration| registration.id);
        let authored_ids: BTreeSet<_> = catalog
            .damage_profiles
            .iter()
            .map(|profile| profile.terminal.reaction_id())
            .collect();
        if authored_ids.iter().any(|id| {
            !self
                .registrations
                .iter()
                .any(|registration| registration.id == *id)
        }) {
            return Err(
                "authored world-object terminal reaction lacks a registered handler".into(),
            );
        }
        Ok(TerminalReactionRegistry {
            registrations: self.registrations,
        })
    }
}

#[derive(Resource)]
pub(crate) struct TerminalReactionRegistry {
    registrations: Vec<TerminalReactionRegistration>,
}

impl TerminalReactionRegistry {
    pub(super) fn handler(
        &self,
        id: crate::map::TerminalReactionId,
    ) -> Option<TerminalReactionHandler> {
        self.registrations
            .binary_search_by_key(&id, |registration| registration.id)
            .ok()
            .map(|index| self.registrations[index].handler)
    }

    pub(super) fn semantics(
        &self,
        id: crate::map::TerminalReactionId,
    ) -> Option<TerminalReactionSemantics> {
        self.registrations
            .binary_search_by_key(&id, |registration| registration.id)
            .ok()
            .map(|index| self.registrations[index].semantics)
    }

    #[cfg(test)]
    fn registered_ids(&self) -> Vec<crate::map::TerminalReactionId> {
        self.registrations
            .iter()
            .map(|registration| registration.id)
            .collect()
    }
}

pub(crate) trait TerminalReactionAppExt {
    fn try_register_terminal_reaction(
        &mut self,
        registration: TerminalReactionRegistration,
    ) -> Result<&mut Self, String>;
}

impl TerminalReactionAppExt for App {
    fn try_register_terminal_reaction(
        &mut self,
        registration: TerminalReactionRegistration,
    ) -> Result<&mut Self, String> {
        if !self
            .world()
            .contains_resource::<TerminalReactionRegistryBuilder>()
        {
            self.init_resource::<TerminalReactionRegistryBuilder>();
        }
        self.world_mut()
            .resource_mut::<TerminalReactionRegistryBuilder>()
            .register(registration)?;
        Ok(self)
    }
}

pub(super) struct TerminalReactionRegistryPlugin;

impl Plugin for TerminalReactionRegistryPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<crate::map::MapCatalogResource>()
            .init_resource::<TerminalReactionRegistryBuilder>();
    }

    fn finish(&self, app: &mut App) {
        let builder = app
            .world_mut()
            .remove_resource::<TerminalReactionRegistryBuilder>()
            .expect("terminal reaction registry builder exists until plugin finalization");
        let registry = {
            let catalog = app.world().resource::<crate::map::MapCatalogResource>();
            builder
                .seal(&catalog.0)
                .expect("terminal reactions cover authored world-object behavior")
        };
        app.insert_resource(registry);
    }
}

pub(crate) struct TerminalReactionContext<'world, 'queue> {
    world: &'world mut World,
    queue: &'queue mut VecDeque<crate::map::PendingWorldTargetDamage>,
    secondary_count: &'queue mut usize,
}

impl<'world, 'queue> TerminalReactionContext<'world, 'queue> {
    pub(crate) fn new(
        world: &'world mut World,
        queue: &'queue mut VecDeque<crate::map::PendingWorldTargetDamage>,
        secondary_count: &'queue mut usize,
    ) -> Self {
        Self {
            world,
            queue,
            secondary_count,
        }
    }

    pub(crate) fn commit_restoration_pickup(
        &mut self,
        plan: &WorldObjectTerminalPlan,
        pickup_definition_id: crate::map::RestorationPickupDefinitionId,
    ) {
        super::object_authority::commit_restoration_pickup_plan(
            self.world,
            plan,
            pickup_definition_id,
        );
    }

    pub(crate) fn commit_explosion(
        &mut self,
        plan: &WorldObjectTerminalPlan,
        explosion_profile_id: crate::map::EnvironmentExplosionProfileId,
    ) {
        super::object_authority::commit_explosion_plan(
            self.world,
            plan,
            self.queue,
            self.secondary_count,
            explosion_profile_id,
        );
    }
}

#[cfg(test)]
mod tests {
    use std::sync::atomic::{AtomicUsize, Ordering};

    use super::*;

    const TEST_REACTION: crate::map::TerminalReactionId =
        crate::map::TerminalReactionId::new(99).unwrap();
    static SYNTHETIC_CALLS: AtomicUsize = AtomicUsize::new(0);

    fn synthetic_handler(
        _plan: &WorldObjectTerminalPlan,
        _context: &mut TerminalReactionContext<'_, '_>,
    ) {
        SYNTHETIC_CALLS.fetch_add(1, Ordering::Relaxed);
    }

    fn registration(id: crate::map::TerminalReactionId) -> TerminalReactionRegistration {
        TerminalReactionRegistration::new(
            id,
            TerminalReactionSemantics::default(),
            synthetic_handler,
        )
    }

    struct SyntheticReactionPlugin;

    impl Plugin for SyntheticReactionPlugin {
        fn build(&self, app: &mut App) {
            app.try_register_terminal_reaction(registration(TEST_REACTION))
                .unwrap();
        }
    }

    fn test_plan(world: &mut World) -> WorldObjectTerminalPlan {
        let entity = world.spawn_empty().id();
        let generation = crate::map::MapDynamicGeneration {
            map_instance_id: crate::map::MapInstanceId(4),
            generation: 2,
        };
        let source = crate::combat::AttackSource {
            kind: crate::combat::CombatSourceKind::PrimaryWeapon,
            attack_id: crate::combat::AttackId(7),
            player_id: crate::protocol::PlayerId(1),
            owner_network_entity_id: crate::protocol::NetworkEntityId(2),
            team_id: crate::combat::TeamId(0),
            recipe_fingerprint: crate::combat::WeaponRecipeFingerprint(3),
            legacy_compatibility: false,
            source_preset_id: None,
            origin: crate::combat::WorldPoint { x: 0.0, y: 0.0 },
            facing: 0.0,
        };
        WorldObjectTerminalPlan {
            reaction_id: TEST_REACTION,
            tick: 8,
            entity,
            position: bevy::prelude::Vec2::ZERO,
            request: crate::map::PendingWorldTargetDamage {
                target: crate::map::DamageableTargetIdentity::MapObject {
                    generation,
                    placement_id: crate::map::MapPlacementId(5),
                },
                source,
                attack_id: source.attack_id,
                requested_damage: 1,
                delivery_index: 0,
                bundle_index: 0,
                effect_index: 0,
            },
            reaction_event_id: crate::combat::CombatEventId(9),
            behavior: crate::map::MapObjectTerminalBehavior::Explode {
                explosion_profile_id: crate::map::EnvironmentExplosionProfileId(1),
                outcome: crate::map::MapPlacementOutcome::Removed,
            },
            outcome: crate::map::MapPlacementOutcome::Removed,
        }
    }

    #[test]
    fn builder_rejects_duplicate_and_excess_registrations() {
        assert!(crate::map::TerminalReactionId::new(0).is_none());
        let mut builder = TerminalReactionRegistryBuilder::default();
        builder.register(registration(TEST_REACTION)).unwrap();
        assert_eq!(
            builder.register(registration(TEST_REACTION)),
            Err("duplicate world-object terminal reaction 99".to_string())
        );
        for raw_id in 1..MAX_TERMINAL_REACTION_REGISTRATIONS {
            builder
                .register(registration(
                    crate::map::TerminalReactionId::new(u16::try_from(raw_id).unwrap()).unwrap(),
                ))
                .unwrap();
        }
        assert!(
            builder
                .register(registration(
                    crate::map::TerminalReactionId::new(200).unwrap(),
                ))
                .is_err()
        );
    }

    #[test]
    fn sealing_requires_authored_coverage_and_orders_additive_registrations() {
        let catalog = crate::map::MapContentCatalog::embedded().unwrap();
        let mut missing = TerminalReactionRegistryBuilder::default();
        missing
            .register(registration(crate::map::TerminalReactionId::EXPLOSION))
            .unwrap();
        assert!(missing.seal(&catalog).is_err());

        let mut complete = TerminalReactionRegistryBuilder::default();
        for id in [
            TEST_REACTION,
            crate::map::TerminalReactionId::RESTORATION_PICKUP,
            crate::map::TerminalReactionId::EXPLOSION,
        ] {
            complete.register(registration(id)).unwrap();
        }
        let registry = complete.seal(&catalog).unwrap();
        assert_eq!(
            registry.registered_ids(),
            vec![
                crate::map::TerminalReactionId::EXPLOSION,
                crate::map::TerminalReactionId::RESTORATION_PICKUP,
                TEST_REACTION,
            ]
        );
        assert!(registry.handler(TEST_REACTION).is_some());
    }

    #[test]
    fn plugin_finalization_is_order_independent_and_synthetic_handler_executes() {
        let mut app = App::new();
        app.add_plugins((
            SyntheticReactionPlugin,
            super::super::object_authority::RestorationPickupTerminalReactionPlugin,
            TerminalReactionRegistryPlugin,
            super::super::object_authority::ExplosionTerminalReactionPlugin,
        ));
        crate::test_app::finalize(&mut app);

        let handler = app
            .world()
            .resource::<TerminalReactionRegistry>()
            .handler(TEST_REACTION)
            .unwrap();
        let plan = test_plan(app.world_mut());
        let mut queue = VecDeque::new();
        let mut secondary_count = 0;
        SYNTHETIC_CALLS.store(0, Ordering::Relaxed);
        handler(
            &plan,
            &mut TerminalReactionContext::new(app.world_mut(), &mut queue, &mut secondary_count),
        );

        assert_eq!(SYNTHETIC_CALLS.load(Ordering::Relaxed), 1);
        assert!(queue.is_empty());
        assert_eq!(secondary_count, 0);
        assert!(
            !app.world()
                .contains_resource::<TerminalReactionRegistryBuilder>()
        );
    }
}
