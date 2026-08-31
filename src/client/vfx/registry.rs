//! Plugin-populated VFX request registration and immutable catalog lookup.

use bevy::prelude::{App, Plugin, Resource};
use std::collections::BTreeSet;

use super::{VfxLifetime, VfxProfile, VfxRequest, VfxRequestKey, catalog::VfxCatalog};

pub(crate) const MAX_VFX_REQUEST_REGISTRATIONS: usize = 32;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct VfxRequestCapabilities {
    pub(crate) authoritative_radius: bool,
    pub(crate) authoritative_deadline: bool,
}

impl VfxRequestCapabilities {
    pub(crate) const NONE: Self = Self {
        authoritative_radius: false,
        authoritative_deadline: false,
    };
    pub(crate) const RADIUS: Self = Self {
        authoritative_radius: true,
        authoritative_deadline: false,
    };
    pub(crate) const RADIUS_AND_DEADLINE: Self = Self {
        authoritative_radius: true,
        authoritative_deadline: true,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct VfxRequestRegistration {
    pub(crate) key: VfxRequestKey,
    pub(crate) producer_rank: u16,
    pub(crate) capabilities: VfxRequestCapabilities,
}

impl VfxRequestRegistration {
    pub(crate) const fn new(
        key: VfxRequestKey,
        producer_rank: u16,
        capabilities: VfxRequestCapabilities,
    ) -> Self {
        Self {
            key,
            producer_rank,
            capabilities,
        }
    }
}

#[derive(Resource, Default)]
struct VfxRegistryBuilder {
    registrations: Vec<VfxRequestRegistration>,
}

impl VfxRegistryBuilder {
    fn register(&mut self, registration: VfxRequestRegistration) -> Result<(), String> {
        if !registration.key.is_valid() {
            return Err(format!(
                "invalid VFX request registration key: {}",
                registration.key.as_str()
            ));
        }
        if registration.producer_rank == 0 {
            return Err("VFX request producer rank must be nonzero".into());
        }
        if self
            .registrations
            .iter()
            .any(|existing| existing.key == registration.key)
        {
            return Err(format!(
                "duplicate VFX request registration: {}",
                registration.key.as_str()
            ));
        }
        if self.registrations.len() >= MAX_VFX_REQUEST_REGISTRATIONS {
            return Err("VFX request registry exceeds engine capacity".into());
        }
        self.registrations.push(registration);
        Ok(())
    }

    fn seal(mut self, catalog: VfxCatalog) -> Result<VfxRegistry, String> {
        self.registrations
            .sort_unstable_by_key(|registration| (registration.producer_rank, registration.key));
        let registered_keys = self
            .registrations
            .iter()
            .map(|registration| registration.key.as_str())
            .collect::<BTreeSet<_>>();
        let mapped_keys = catalog.mapping_keys().collect::<BTreeSet<_>>();
        if let Some(missing) = registered_keys.difference(&mapped_keys).next() {
            return Err(format!(
                "registered VFX request lacks an authored mapping: {missing}"
            ));
        }
        if let Some(extra) = mapped_keys.difference(&registered_keys).next() {
            return Err(format!(
                "authored VFX request mapping lacks a registered producer: {extra}"
            ));
        }
        for registration in &self.registrations {
            let requirements = catalog
                .requirements(registration.key.as_str())
                .expect("exact request-key coverage was validated");
            if requirements.authoritative_radius && !registration.capabilities.authoritative_radius
            {
                return Err(format!(
                    "VFX request {} maps to a profile requiring authoritative radius",
                    registration.key.as_str()
                ));
            }
            if requirements.authoritative_deadline
                && !registration.capabilities.authoritative_deadline
            {
                return Err(format!(
                    "VFX request {} maps to a profile requiring authoritative deadline",
                    registration.key.as_str()
                ));
            }
        }
        Ok(VfxRegistry {
            registrations: self.registrations,
            catalog,
        })
    }
}

#[derive(Resource, Clone, Debug)]
pub(crate) struct VfxRegistry {
    registrations: Vec<VfxRequestRegistration>,
    catalog: VfxCatalog,
}

impl VfxRegistry {
    pub(crate) fn resolve(&self, request: &VfxRequest, reduced: bool) -> Option<&VfxProfile> {
        request.validate().ok()?;
        let registration = self.registration(request.key)?;
        if request.order.producer_rank != registration.producer_rank
            || (request.authoritative_radius.is_some()
                && !registration.capabilities.authoritative_radius)
            || (request.deadline.is_some() && !registration.capabilities.authoritative_deadline)
        {
            return None;
        }
        let profile =
            self.catalog
                .resolve(request.key.as_str(), reduced, request.deadline.is_some())?;
        if (profile.scale.requires_authoritative_radius()
            || profile.anchor.requires_authoritative_radius())
            && request.authoritative_radius.is_none()
        {
            return None;
        }
        if matches!(profile.lifetime, VfxLifetime::AuthoritativeDeadline)
            && request.deadline.is_none()
        {
            return None;
        }
        Some(profile)
    }

    #[cfg(test)]
    pub(crate) fn producer_rank(&self, key: VfxRequestKey) -> Option<u16> {
        self.registration(key)
            .map(|registration| registration.producer_rank)
    }

    fn registration(&self, key: VfxRequestKey) -> Option<&VfxRequestRegistration> {
        self.registrations
            .iter()
            .find(|registration| registration.key == key)
    }

    #[cfg(test)]
    pub(crate) fn with_test_mapping(
        self,
        registration: VfxRequestRegistration,
        profile_id: &str,
    ) -> Result<Self, String> {
        let Self {
            registrations,
            mut catalog,
        } = self;
        catalog.insert_test_mapping(registration.key.as_str(), profile_id)?;
        let mut builder = VfxRegistryBuilder { registrations };
        builder.register(registration)?;
        builder.seal(catalog)
    }

    #[cfg(test)]
    fn registrations(&self) -> &[VfxRequestRegistration] {
        &self.registrations
    }
}

pub(crate) trait VfxAppExt {
    fn try_register_vfx_request(
        &mut self,
        registration: VfxRequestRegistration,
    ) -> Result<&mut Self, String>;
}

impl VfxAppExt for App {
    fn try_register_vfx_request(
        &mut self,
        registration: VfxRequestRegistration,
    ) -> Result<&mut Self, String> {
        let Some(mut builder) = self.world_mut().get_resource_mut::<VfxRegistryBuilder>() else {
            return Err(if self.world().contains_resource::<VfxRegistry>() {
                "VFX request registry is already sealed".into()
            } else {
                "VfxRegistryPlugin must be installed before VFX request producers".into()
            });
        };
        builder.register(registration)?;
        Ok(self)
    }
}

pub(crate) struct VfxRegistryPlugin;

impl Plugin for VfxRegistryPlugin {
    fn build(&self, app: &mut App) {
        app.add_message::<VfxRequest>()
            .init_resource::<VfxRegistryBuilder>();
    }

    fn finish(&self, app: &mut App) {
        let builder = app
            .world_mut()
            .remove_resource::<VfxRegistryBuilder>()
            .expect("VFX registry builder exists until plugin finalization");
        let catalog = VfxCatalog::embedded().expect("embedded client VFX catalog must be valid");
        let registry = builder
            .seal(catalog)
            .expect("installed VFX producers and authored mappings must agree");
        app.insert_resource(registry);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::client::vfx::{COMBAT_MUZZLE_VFX, COMBAT_VFX_PRODUCER_RANK, REVEAL_SCAN_VFX};

    fn registration(
        key: VfxRequestKey,
        rank: u16,
        capabilities: VfxRequestCapabilities,
    ) -> VfxRequestRegistration {
        VfxRequestRegistration::new(key, rank, capabilities)
    }

    #[test]
    fn registration_rejects_duplicates_invalid_rank_and_capacity() {
        let mut builder = VfxRegistryBuilder::default();
        builder
            .register(registration(
                COMBAT_MUZZLE_VFX,
                COMBAT_VFX_PRODUCER_RANK,
                VfxRequestCapabilities::NONE,
            ))
            .unwrap();
        assert!(
            builder
                .register(registration(
                    COMBAT_MUZZLE_VFX,
                    COMBAT_VFX_PRODUCER_RANK,
                    VfxRequestCapabilities::NONE,
                ))
                .is_err()
        );
        assert!(
            VfxRegistryBuilder::default()
                .register(registration(
                    VfxRequestKey::new("test.zero-rank"),
                    0,
                    VfxRequestCapabilities::NONE,
                ))
                .is_err()
        );

        let mut capacity = VfxRegistryBuilder::default();
        for index in 0..MAX_VFX_REQUEST_REGISTRATIONS {
            let key = Box::leak(format!("test.request-{index}").into_boxed_str());
            capacity
                .register(registration(
                    VfxRequestKey::new(key),
                    1,
                    VfxRequestCapabilities::NONE,
                ))
                .unwrap();
        }
        assert!(
            capacity
                .register(registration(
                    VfxRequestKey::new("test.over-capacity"),
                    1,
                    VfxRequestCapabilities::NONE,
                ))
                .is_err()
        );
    }

    #[test]
    fn coverage_and_profile_capabilities_are_fail_closed() {
        let catalog = VfxCatalog::embedded().unwrap();
        let mut missing = VfxRegistryBuilder::default();
        missing
            .register(registration(
                COMBAT_MUZZLE_VFX,
                COMBAT_VFX_PRODUCER_RANK,
                VfxRequestCapabilities::NONE,
            ))
            .unwrap();
        assert!(missing.seal(catalog).is_err());

        let catalog = VfxCatalog::embedded().unwrap();
        let mut wrong_capability = builder_for_embedded();
        wrong_capability
            .registrations
            .iter_mut()
            .find(|registration| registration.key == REVEAL_SCAN_VFX)
            .unwrap()
            .capabilities = VfxRequestCapabilities::NONE;
        assert!(wrong_capability.seal(catalog).is_err());

        let source = super::super::catalog::VFX_CATALOG.replacen(
            "scale: AuthoritativeRadius(1.0), anchor: GroundOffset(2.5), lifetime: AuthoritativeDeadline, concurrency_cap: 96, fallback_profile: \"impact\"",
            "scale: FixedWorld(10.0), anchor: GroundOffset(2.5), lifetime: AuthoritativeDeadline, concurrency_cap: 96, fallback_profile: \"elemental_field\"",
            1,
        );
        let catalog = VfxCatalog::from_ron(&source).unwrap();
        let mut missing_fallback_radius = builder_for_embedded();
        missing_fallback_radius
            .registrations
            .iter_mut()
            .find(|registration| registration.key == REVEAL_SCAN_VFX)
            .unwrap()
            .capabilities = VfxRequestCapabilities {
            authoritative_radius: false,
            authoritative_deadline: true,
        };
        assert!(missing_fallback_radius.seal(catalog).is_err());
    }

    #[test]
    fn sealing_sorts_registrations_and_runtime_resolution_fails_closed() {
        let mut builder = builder_for_embedded();
        builder.registrations.reverse();
        let registry = builder.seal(VfxCatalog::embedded().unwrap()).unwrap();
        assert!(registry.registrations().windows(2).all(|pair| (
            pair[0].producer_rank,
            pair[0].key
        ) <= (
            pair[1].producer_rank,
            pair[1].key
        )));

        let valid = VfxRequest::try_new(
            COMBAT_MUZZLE_VFX,
            super::super::VfxRequestOrder::new(COMBAT_VFX_PRODUCER_RANK, 7),
            bevy::prelude::Vec2::ZERO,
            None,
            None,
            "test muzzle",
        )
        .unwrap();
        assert_eq!(registry.resolve(&valid, false).unwrap().id, "muzzle");

        let mut wrong_rank = valid;
        wrong_rank.order.producer_rank += 1;
        assert!(registry.resolve(&wrong_rank, false).is_none());
    }

    #[test]
    fn synthetic_key_registers_and_resolves_without_a_central_family_branch() {
        const SYNTHETIC: VfxRequestKey = VfxRequestKey::new("synthetic.spark");
        const SYNTHETIC_RANK: u16 = 900;
        let source = super::super::catalog::VFX_CATALOG.replacen(
            "mappings: [",
            "mappings: [(key: \"synthetic.spark\", profile: \"impact\"),",
            1,
        );
        let catalog = VfxCatalog::from_ron(&source).unwrap();
        let mut builder = builder_for_embedded();
        builder
            .register(VfxRequestRegistration::new(
                SYNTHETIC,
                SYNTHETIC_RANK,
                VfxRequestCapabilities::NONE,
            ))
            .unwrap();
        let registry = builder.seal(catalog).unwrap();
        let request = VfxRequest::try_new(
            SYNTHETIC,
            super::super::VfxRequestOrder::new(SYNTHETIC_RANK, 99),
            bevy::prelude::Vec2::new(1.0, 2.0),
            None,
            None,
            "synthetic extension",
        )
        .unwrap();

        assert_eq!(registry.resolve(&request, false).unwrap().id, "impact");
    }

    #[test]
    fn app_registration_requires_builder_and_rejects_after_seal() {
        let mut missing = App::new();
        assert!(
            missing
                .try_register_vfx_request(registration(
                    COMBAT_MUZZLE_VFX,
                    COMBAT_VFX_PRODUCER_RANK,
                    VfxRequestCapabilities::NONE,
                ))
                .is_err()
        );

        let mut app = App::new();
        app.add_plugins(VfxRegistryPlugin);
        for registration in builder_for_embedded().registrations {
            app.try_register_vfx_request(registration).unwrap();
        }
        app.finish();
        assert!(app.world().contains_resource::<VfxRegistry>());
        assert!(
            app.try_register_vfx_request(registration(
                VfxRequestKey::new("test.after-seal"),
                900,
                VfxRequestCapabilities::NONE,
            ))
            .is_err()
        );
    }

    fn builder_for_embedded() -> VfxRegistryBuilder {
        use crate::client::vfx::*;
        let mut builder = VfxRegistryBuilder::default();
        for (key, rank, capabilities) in [
            (
                COMBAT_MUZZLE_VFX,
                COMBAT_VFX_PRODUCER_RANK,
                VfxRequestCapabilities::NONE,
            ),
            (
                COMBAT_IMPACT_VFX,
                COMBAT_VFX_PRODUCER_RANK,
                VfxRequestCapabilities::NONE,
            ),
            (
                COMBAT_DAMAGE_VFX,
                COMBAT_VFX_PRODUCER_RANK,
                VfxRequestCapabilities::NONE,
            ),
            (
                COMBAT_RESET_VFX,
                COMBAT_VFX_PRODUCER_RANK,
                VfxRequestCapabilities::NONE,
            ),
            (
                REVEAL_SCAN_VFX,
                COMBAT_VFX_PRODUCER_RANK,
                VfxRequestCapabilities::RADIUS_AND_DEADLINE,
            ),
            (
                ELEMENTAL_FIELD_VFX,
                COMBAT_VFX_PRODUCER_RANK,
                VfxRequestCapabilities::RADIUS,
            ),
            (
                DEMOLITION_STRIKE_VFX,
                COMBAT_VFX_PRODUCER_RANK,
                VfxRequestCapabilities::RADIUS,
            ),
            (
                WORLD_OBJECT_DAMAGED_VFX,
                WORLD_OBJECT_VFX_PRODUCER_RANK,
                VfxRequestCapabilities::NONE,
            ),
            (
                WORLD_OBJECT_EXPLOSION_VFX,
                WORLD_OBJECT_VFX_PRODUCER_RANK,
                VfxRequestCapabilities::RADIUS,
            ),
            (
                PICKUP_SPAWNED_VFX,
                PICKUP_VFX_PRODUCER_RANK,
                VfxRequestCapabilities::NONE,
            ),
            (
                PICKUP_COLLECTED_VFX,
                PICKUP_VFX_PRODUCER_RANK,
                VfxRequestCapabilities::NONE,
            ),
            (
                PICKUP_EXPIRED_VFX,
                PICKUP_VFX_PRODUCER_RANK,
                VfxRequestCapabilities::NONE,
            ),
            (
                HEIST_DAMAGED_VFX,
                HEIST_VFX_PRODUCER_RANK,
                VfxRequestCapabilities::NONE,
            ),
            (
                HEIST_CRITICAL_VFX,
                HEIST_VFX_PRODUCER_RANK,
                VfxRequestCapabilities::NONE,
            ),
            (
                HEIST_DESTROYED_VFX,
                HEIST_VFX_PRODUCER_RANK,
                VfxRequestCapabilities::NONE,
            ),
        ] {
            builder
                .register(VfxRequestRegistration::new(key, rank, capabilities))
                .unwrap();
        }
        builder
    }
}
