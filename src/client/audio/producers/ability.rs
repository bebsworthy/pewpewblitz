use std::collections::HashMap;

use bevy::prelude::*;
use lightyear::prelude::Controlled;

use crate::{
    abilities::{Sentry, SentryIdentity},
    builds::{AbilityPhase, AbilityState, PassiveRuntimeState},
    client::audio::{
        registry::{AudioProducerRegistration, AudioProducerRegistrationAppExt},
        request::{AudioCueKey, AudioRequest, cue_keys},
    },
    combat::WeaponState,
    concealment::ConcealmentFieldState,
    protocol::Fighter,
};

use super::AudioProducerSequence;

const ABILITY_RANK: u16 = 30;
const RELOAD_RANK: u16 = 40;
const ABILITY_KEYS: &[AudioCueKey] = &[
    cue_keys::DASH,
    cue_keys::CHARGE_READY,
    cue_keys::PASSIVE,
    cue_keys::SENTRY_SPAWN,
    cue_keys::CONCEALMENT_FIELD_SPAWN,
];
const RELOAD_KEYS: &[AudioCueKey] = &[cue_keys::RELOAD];

pub(super) fn register_ability(app: &mut App) {
    app.try_register_audio_producer(AudioProducerRegistration {
        id: "ability",
        rank: ABILITY_RANK,
        cue_keys: ABILITY_KEYS,
    })
    .expect("ability audio producer registration must be valid");
}

pub(super) fn register_reload(app: &mut App) {
    app.try_register_audio_producer(AudioProducerRegistration {
        id: "reload",
        rank: RELOAD_RANK,
        cue_keys: RELOAD_KEYS,
    })
    .expect("reload audio producer registration must be valid");
}

#[allow(
    clippy::type_complexity,
    reason = "the queries declare the producer's feature-owned transition inputs"
)]
pub(super) fn produce_ability_audio_requests(
    abilities: Query<&AbilityState, (With<Fighter>, With<Controlled>, Changed<AbilityState>)>,
    passives: Query<
        &PassiveRuntimeState,
        (
            With<Fighter>,
            With<Controlled>,
            Changed<PassiveRuntimeState>,
        ),
    >,
    sentries: Query<&SentryIdentity, Added<Sentry>>,
    concealment_fields: Query<&ConcealmentFieldState, Added<ConcealmentFieldState>>,
    mut requests: MessageWriter<AudioRequest>,
    mut sequence: Local<AudioProducerSequence>,
) {
    for ability in &abilities {
        let cue_key = match ability.phase {
            AbilityPhase::Dashing { .. } => Some(cue_keys::DASH),
            AbilityPhase::Ready => Some(cue_keys::CHARGE_READY),
            AbilityPhase::Charging
            | AbilityPhase::Deployed { .. }
            | AbilityPhase::Cloaked { .. }
            | AbilityPhase::FieldActive { .. }
            | AbilityPhase::ElementalFieldActive { .. } => None,
        };
        if let Some(cue_key) = cue_key {
            requests.write(AudioRequest::once(cue_key, sequence.next(ABILITY_RANK)));
        }
    }
    if passives
        .iter()
        .any(|state| state.adrenaline_until_tick.is_some() || state.quick_cycle_primed)
    {
        requests.write(AudioRequest::once(
            cue_keys::PASSIVE,
            sequence.next(ABILITY_RANK),
        ));
    }
    for _ in &sentries {
        requests.write(AudioRequest::once(
            cue_keys::SENTRY_SPAWN,
            sequence.next(ABILITY_RANK),
        ));
    }
    for _ in &concealment_fields {
        requests.write(AudioRequest::once(
            cue_keys::CONCEALMENT_FIELD_SPAWN,
            sequence.next(ABILITY_RANK),
        ));
    }
}

#[allow(
    clippy::type_complexity,
    reason = "the query names the exact controlled-fighter ammunition state"
)]
pub(super) fn produce_reload_audio_requests(
    weapons: Query<(Entity, &WeaponState), (With<Fighter>, With<Controlled>)>,
    mut observed_ammo: Local<HashMap<Entity, u8>>,
    mut requests: MessageWriter<AudioRequest>,
    mut sequence: Local<AudioProducerSequence>,
) {
    for (entity, state) in &weapons {
        if observed_ammo
            .insert(entity, state.ammo)
            .is_some_and(|previous| state.ammo > previous)
        {
            requests.write(AudioRequest::once(
                cue_keys::RELOAD,
                sequence.next(RELOAD_RANK),
            ));
        }
    }
    observed_ammo.retain(|entity, _| weapons.get(*entity).is_ok());
}
