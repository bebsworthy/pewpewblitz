use std::collections::BTreeSet;

use bevy::prelude::*;

use super::{
    catalog::AudioProfileCatalog,
    request::{AudioCueKey, AudioRequest, validate_audio_cue_key},
};

pub(crate) const MAX_AUDIO_PRODUCERS: usize = 32;
pub(crate) const MAX_AUDIO_CUE_KEYS: usize = 64;
const MAX_AUDIO_CUE_DECLARATIONS: usize = 128;

#[derive(Clone, Copy, Debug)]
pub(crate) struct AudioProducerRegistration {
    pub(crate) id: &'static str,
    pub(crate) rank: u16,
    pub(crate) cue_keys: &'static [AudioCueKey],
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) enum AudioRegistryError {
    RegistryPluginMissing,
    RegistrySealed,
    InvalidProducerId,
    InvalidProducerRank,
    EmptyCueKeys,
    InvalidCueKey(&'static str),
    DuplicateProducerId(&'static str),
    DuplicateProducerRank(u16),
    DuplicateCueKeyWithinProducer(&'static str),
    ProducerCapacityExceeded,
    CueKeyCapacityExceeded,
    CueDeclarationCapacityExceeded,
    CatalogCoverage(String),
}

#[derive(Resource, Default)]
struct AudioRegistryBuilder {
    registrations: Vec<AudioProducerRegistration>,
}

impl AudioRegistryBuilder {
    fn register(
        &mut self,
        registration: AudioProducerRegistration,
    ) -> Result<(), AudioRegistryError> {
        validate_audio_cue_key(registration.id)
            .map_err(|_| AudioRegistryError::InvalidProducerId)?;
        if registration.rank == 0 {
            return Err(AudioRegistryError::InvalidProducerRank);
        }
        if registration.cue_keys.is_empty() {
            return Err(AudioRegistryError::EmptyCueKeys);
        }
        if self
            .registrations
            .iter()
            .any(|existing| existing.id == registration.id)
        {
            return Err(AudioRegistryError::DuplicateProducerId(registration.id));
        }
        if self
            .registrations
            .iter()
            .any(|existing| existing.rank == registration.rank)
        {
            return Err(AudioRegistryError::DuplicateProducerRank(registration.rank));
        }
        if self.registrations.len() == MAX_AUDIO_PRODUCERS {
            return Err(AudioRegistryError::ProducerCapacityExceeded);
        }

        let mut registration_keys = BTreeSet::new();
        for cue_key in registration.cue_keys {
            validate_audio_cue_key(cue_key.as_str())
                .map_err(|_| AudioRegistryError::InvalidCueKey(cue_key.as_str()))?;
            if !registration_keys.insert(*cue_key) {
                return Err(AudioRegistryError::DuplicateCueKeyWithinProducer(
                    cue_key.as_str(),
                ));
            }
        }

        let cue_declarations = self
            .registrations
            .iter()
            .map(|existing| existing.cue_keys.len())
            .sum::<usize>()
            .saturating_add(registration.cue_keys.len());
        if cue_declarations > MAX_AUDIO_CUE_DECLARATIONS {
            return Err(AudioRegistryError::CueDeclarationCapacityExceeded);
        }
        let unique_cue_keys = self
            .registrations
            .iter()
            .flat_map(|existing| existing.cue_keys.iter().copied())
            .chain(registration.cue_keys.iter().copied())
            .collect::<BTreeSet<_>>();
        if unique_cue_keys.len() > MAX_AUDIO_CUE_KEYS {
            return Err(AudioRegistryError::CueKeyCapacityExceeded);
        }

        self.registrations.push(registration);
        Ok(())
    }

    fn seal(mut self, catalog: &AudioProfileCatalog) -> Result<AudioRegistry, AudioRegistryError> {
        self.registrations
            .sort_unstable_by_key(|registration| (registration.rank, registration.id));
        let cue_keys = self
            .registrations
            .iter()
            .flat_map(|registration| registration.cue_keys.iter().copied())
            .collect::<BTreeSet<_>>();
        let declared_requests = self
            .registrations
            .iter()
            .flat_map(|registration| {
                registration
                    .cue_keys
                    .iter()
                    .map(move |cue_key| (registration.rank, *cue_key))
            })
            .collect::<BTreeSet<_>>();
        catalog
            .validate_registered_keys(cue_keys.iter().map(|key| key.as_str()))
            .map_err(AudioRegistryError::CatalogCoverage)?;
        Ok(AudioRegistry {
            registrations: self.registrations,
            declared_requests,
        })
    }
}

#[derive(Resource, Clone, Debug)]
pub(crate) struct AudioRegistry {
    #[allow(
        dead_code,
        reason = "sealed deterministic producer order is retained for diagnostics and extension tests"
    )]
    registrations: Vec<AudioProducerRegistration>,
    declared_requests: BTreeSet<(u16, AudioCueKey)>,
}

impl AudioRegistry {
    #[cfg(test)]
    pub(crate) fn contains(&self, cue_key: AudioCueKey) -> bool {
        self.declared_requests
            .iter()
            .any(|(_, declared)| *declared == cue_key)
    }

    /// Fails closed unless the request's exact producer rank declared its semantic cue key.
    pub(crate) fn allows(&self, request: &AudioRequest) -> bool {
        self.declared_requests
            .contains(&(request.order.producer_rank, request.cue_key))
    }

    #[cfg(test)]
    fn registrations(&self) -> &[AudioProducerRegistration] {
        &self.registrations
    }

    #[cfg(test)]
    pub(crate) fn producer_rank(&self, producer_id: &str) -> Option<u16> {
        self.registrations
            .iter()
            .find(|registration| registration.id == producer_id)
            .map(|registration| registration.rank)
    }
}

pub(crate) trait AudioProducerRegistrationAppExt {
    fn try_register_audio_producer(
        &mut self,
        registration: AudioProducerRegistration,
    ) -> Result<&mut Self, AudioRegistryError>;
}

impl AudioProducerRegistrationAppExt for App {
    fn try_register_audio_producer(
        &mut self,
        registration: AudioProducerRegistration,
    ) -> Result<&mut Self, AudioRegistryError> {
        if self.world().contains_resource::<AudioRegistry>() {
            return Err(AudioRegistryError::RegistrySealed);
        }
        {
            let Some(mut builder) = self.world_mut().get_resource_mut::<AudioRegistryBuilder>()
            else {
                return Err(AudioRegistryError::RegistryPluginMissing);
            };
            builder.register(registration)?;
        }
        Ok(self)
    }
}

pub(crate) struct AudioRegistryPlugin;

impl Plugin for AudioRegistryPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<AudioProfileCatalog>() {
            app.insert_resource(
                AudioProfileCatalog::embedded().expect("embedded audio profile catalog is valid"),
            );
        }
        app.init_resource::<AudioRegistryBuilder>();
    }

    fn finish(&self, app: &mut App) {
        let builder = app
            .world_mut()
            .remove_resource::<AudioRegistryBuilder>()
            .expect("audio registry builder exists until plugin finalization");
        let registry = builder
            .seal(app.world().resource::<AudioProfileCatalog>())
            .expect("audio producer registrations exactly cover the validated catalog");
        app.insert_resource(registry);
    }
}

#[cfg(test)]
mod tests {
    use super::super::request::{AudioRequestOrder, cue_keys};
    use super::*;

    const CORE_KEYS: &[AudioCueKey] = &[
        cue_keys::FIRE,
        cue_keys::IMPACT,
        cue_keys::DEFEAT,
        cue_keys::RESET,
        cue_keys::READY,
        cue_keys::ERROR,
        cue_keys::DASH,
        cue_keys::SENTRY,
        cue_keys::SENTRY_SPAWN,
        cue_keys::CONCEALMENT_FIELD_SPAWN,
        cue_keys::CHARGE_READY,
        cue_keys::PASSIVE,
        cue_keys::OBJECTIVE_HIT,
        cue_keys::OBJECTIVE_CRITICAL,
        cue_keys::OBJECTIVE_DESTROYED,
        cue_keys::RELOAD,
    ];
    const READY_ONLY: &[AudioCueKey] = &[cue_keys::READY];
    const DUPLICATE_READY: &[AudioCueKey] = &[cue_keys::READY, cue_keys::READY];
    const INVALID_KEY: &[AudioCueKey] = &[AudioCueKey::new("Invalid_Key")];

    fn registration(
        id: &'static str,
        rank: u16,
        cue_keys: &'static [AudioCueKey],
    ) -> AudioProducerRegistration {
        AudioProducerRegistration { id, rank, cue_keys }
    }

    #[test]
    fn lifecycle_requires_plugin_and_rejects_registration_after_sealing() {
        let mut app = App::new();
        assert_eq!(
            app.try_register_audio_producer(registration("core", 1, CORE_KEYS))
                .err()
                .unwrap(),
            AudioRegistryError::RegistryPluginMissing
        );

        app.add_plugins(AudioRegistryPlugin);
        app.try_register_audio_producer(registration("core", 1, CORE_KEYS))
            .unwrap();
        app.finish();
        assert_eq!(
            app.try_register_audio_producer(registration("later", 1, READY_ONLY))
                .err()
                .unwrap(),
            AudioRegistryError::RegistrySealed
        );
    }

    #[test]
    fn unique_producers_may_share_a_cue_but_local_duplicates_reject() {
        let mut builder = AudioRegistryBuilder::default();
        builder
            .register(registration("session", 1, READY_ONLY))
            .unwrap();
        builder
            .register(registration("match", 2, READY_ONLY))
            .unwrap();
        assert_eq!(
            builder
                .register(registration("duplicate", 3, DUPLICATE_READY))
                .unwrap_err(),
            AudioRegistryError::DuplicateCueKeyWithinProducer("ready")
        );
    }

    #[test]
    fn duplicate_ids_ranks_and_invalid_registration_shapes_reject() {
        let mut builder = AudioRegistryBuilder::default();
        builder
            .register(registration("core", 1, CORE_KEYS))
            .unwrap();
        assert_eq!(
            builder
                .register(registration("core", 2, READY_ONLY))
                .unwrap_err(),
            AudioRegistryError::DuplicateProducerId("core")
        );
        assert_eq!(
            builder
                .register(registration("other", 1, READY_ONLY))
                .unwrap_err(),
            AudioRegistryError::DuplicateProducerRank(1)
        );
        assert_eq!(
            builder
                .register(registration("Bad_Id", 2, READY_ONLY))
                .unwrap_err(),
            AudioRegistryError::InvalidProducerId
        );
        assert_eq!(
            builder.register(registration("empty", 2, &[])).unwrap_err(),
            AudioRegistryError::EmptyCueKeys
        );
        assert_eq!(
            builder
                .register(registration("invalid-key", 2, INVALID_KEY))
                .unwrap_err(),
            AudioRegistryError::InvalidCueKey("Invalid_Key")
        );
        assert_eq!(
            builder
                .register(registration("invalid-rank", 0, READY_ONLY))
                .unwrap_err(),
            AudioRegistryError::InvalidProducerRank
        );

        let mut high_rank = AudioRegistryBuilder::default();
        high_rank
            .register(registration("high-rank", u16::MAX, READY_ONLY))
            .unwrap();
    }

    #[test]
    fn producer_capacity_is_bounded() {
        let mut builder = AudioRegistryBuilder::default();
        for rank in 1..=MAX_AUDIO_PRODUCERS {
            let id = Box::leak(format!("producer-{rank}").into_boxed_str());
            builder
                .register(registration(id, u16::try_from(rank).unwrap(), READY_ONLY))
                .unwrap();
        }
        assert_eq!(
            builder
                .register(registration(
                    "overflow",
                    u16::try_from(MAX_AUDIO_PRODUCERS + 1).unwrap(),
                    READY_ONLY,
                ))
                .unwrap_err(),
            AudioRegistryError::ProducerCapacityExceeded
        );
    }

    #[test]
    fn cue_key_and_declaration_capacities_are_bounded() {
        let too_many_unique = (0..=MAX_AUDIO_CUE_KEYS)
            .map(|index| AudioCueKey::new(Box::leak(format!("cue-{index}").into_boxed_str())))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let too_many_unique = Box::leak(too_many_unique);
        let mut builder = AudioRegistryBuilder::default();
        assert_eq!(
            builder
                .register(registration("unique-overflow", 1, too_many_unique))
                .unwrap_err(),
            AudioRegistryError::CueKeyCapacityExceeded
        );

        let shared = (0..MAX_AUDIO_CUE_KEYS)
            .map(|index| AudioCueKey::new(Box::leak(format!("shared-{index}").into_boxed_str())))
            .collect::<Vec<_>>()
            .into_boxed_slice();
        let shared = Box::leak(shared);
        let mut builder = AudioRegistryBuilder::default();
        builder.register(registration("first", 1, shared)).unwrap();
        builder.register(registration("second", 2, shared)).unwrap();
        assert_eq!(
            builder
                .register(registration("third", 3, READY_ONLY))
                .unwrap_err(),
            AudioRegistryError::CueDeclarationCapacityExceeded
        );
    }

    #[test]
    fn exact_coverage_rejects_missing_and_orphan_keys() {
        let catalog = AudioProfileCatalog::embedded().unwrap();
        let missing = AudioRegistryBuilder::default().seal(&catalog).unwrap_err();
        assert_eq!(
            missing,
            AudioRegistryError::CatalogCoverage(
                "audio catalog mapping charge-ready has no registered producer".to_string()
            )
        );

        let mut incomplete = AudioRegistryBuilder::default();
        incomplete
            .register(registration("session", 1, READY_ONLY))
            .unwrap();
        assert!(matches!(
            incomplete.seal(&catalog),
            Err(AudioRegistryError::CatalogCoverage(message))
                if message.contains("has no registered producer")
        ));
    }

    #[test]
    fn synthetic_key_seals_and_resolves_without_adapter_dispatch() {
        const SYNTHETIC: AudioCueKey = AudioCueKey::new("synthetic");
        const SYNTHETIC_KEYS: &[AudioCueKey] = &[SYNTHETIC];
        let catalog = AudioProfileCatalog::embedded()
            .unwrap()
            .with_test_mapping(SYNTHETIC, "ready")
            .unwrap();
        let mut builder = AudioRegistryBuilder::default();
        builder
            .register(registration("core", 1, CORE_KEYS))
            .unwrap();
        builder
            .register(registration("synthetic", 2, SYNTHETIC_KEYS))
            .unwrap();
        let registry = builder.seal(&catalog).unwrap();

        assert!(registry.contains(SYNTHETIC));
        assert_eq!(registry.producer_rank("synthetic"), Some(2));
        assert_eq!(
            registry
                .registrations()
                .iter()
                .map(|registration| registration.id)
                .collect::<Vec<_>>(),
            ["core", "synthetic"]
        );
        let plan = catalog.playback_plan(SYNTHETIC, |_| true).unwrap();
        assert_eq!(plan.asset_id, "audio.ready");
        assert!((plan.speed - 1.0).abs() < f32::EPSILON);
        assert!((plan.volume - 1.0).abs() < f32::EPSILON);
        assert_eq!(plan.concurrency_cap, 24);
    }

    #[test]
    fn runtime_requests_require_the_exact_registered_producer_rank_and_key() {
        const UNKNOWN: AudioCueKey = AudioCueKey::new("unknown");
        let catalog = AudioProfileCatalog::embedded().unwrap();
        let mut builder = AudioRegistryBuilder::default();
        builder
            .register(registration("core", 10, CORE_KEYS))
            .unwrap();
        builder
            .register(registration("match", 20, READY_ONLY))
            .unwrap();
        let registry = builder.seal(&catalog).unwrap();

        assert!(registry.allows(&AudioRequest::once(
            cue_keys::FIRE,
            AudioRequestOrder::new(10, 0),
        )));
        assert!(registry.allows(&AudioRequest::once(
            cue_keys::READY,
            AudioRequestOrder::new(10, 1),
        )));
        assert!(registry.allows(&AudioRequest::once(
            cue_keys::READY,
            AudioRequestOrder::new(20, 0),
        )));
        assert!(!registry.allows(&AudioRequest::once(
            cue_keys::FIRE,
            AudioRequestOrder::new(20, 0),
        )));
        assert!(!registry.allows(&AudioRequest::once(UNKNOWN, AudioRequestOrder::new(10, 0),)));
        assert!(!registry.allows(&AudioRequest::once(
            cue_keys::FIRE,
            AudioRequestOrder::new(0, 0),
        )));
    }
}
