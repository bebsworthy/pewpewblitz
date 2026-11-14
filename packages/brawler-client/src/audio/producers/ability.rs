use std::collections::HashMap;

use bevy::prelude::*;
use lightyear::prelude::Controlled;

use crate::{
    audio::{
        registry::{AudioProducerRegistration, AudioProducerRegistrationAppExt},
        request::{AudioCueKey, AudioRequest, cue_keys},
    },
    local_attack::{LocalAttackKind, LocalAttackLedger, LocalAttackPresented},
};
use brawler::{
    abilities::Sentry,
    builds::{AbilityPhase, AbilityState, PassiveRuntimeState, UltimateKind},
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

/// Activation-sound occurrence of a locally presented ultimate cast (shared with the combat
/// producer's disjoint local range).
const LOCAL_ULTIMATE_AUDIO_OCCURRENCE: u64 = super::combat::LOCAL_ULTIMATE_AUDIO_OCCURRENCE;

/// The ability producer's spawn sound of a locally cast ultimate, when it is a launch.
const fn local_ultimate_spawn_cue(kind: UltimateKind) -> Option<AudioCueKey> {
    if !crate::local_attack::presents_activation_sound_locally(kind) {
        return None;
    }
    match kind {
        UltimateKind::Sentry => Some(cue_keys::SENTRY_SPAWN),
        UltimateKind::ConcealmentField => Some(cue_keys::CONCEALMENT_FIELD_SPAWN),
        _ => None,
    }
}

/// Ability transitions plus the local player's Sentry and Concealment Field spawn sounds on the
/// input frame; a replicated spawn that echoes a locally presented cast stays silent.
#[allow(
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "the queries declare the producer's feature-owned transition inputs"
)]
pub(super) fn produce_ability_audio_requests(
    mut local_attacks: MessageReader<LocalAttackPresented>,
    ledger: Option<Res<LocalAttackLedger>>,
    abilities: Query<&AbilityState, (With<Fighter>, With<Controlled>, Changed<AbilityState>)>,
    passives: Query<
        &PassiveRuntimeState,
        (
            With<Fighter>,
            With<Controlled>,
            Changed<PassiveRuntimeState>,
        ),
    >,
    sentries: Query<Entity, Added<Sentry>>,
    concealment_fields: Query<Entity, Added<ConcealmentFieldState>>,
    mut requests: MessageWriter<AudioRequest>,
    mut sequence: Local<AudioProducerSequence>,
) {
    for local in local_attacks.read() {
        if let LocalAttackKind::Ultimate { kind, cast } = local.kind
            && let Some(cue_key) = local_ultimate_spawn_cue(kind)
        {
            requests.write(AudioRequest::for_occurrence(
                cue_key,
                LOCAL_ULTIMATE_AUDIO_OCCURRENCE | cast.0,
                sequence.next(ABILITY_RANK),
            ));
        }
    }
    let echoed = |entity: Entity| {
        ledger
            .as_deref()
            .is_some_and(|ledger| ledger.is_activation_echo_entity(entity))
    };
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
    for _ in sentries.iter().filter(|entity| !echoed(*entity)) {
        requests.write(AudioRequest::once(
            cue_keys::SENTRY_SPAWN,
            sequence.next(ABILITY_RANK),
        ));
    }
    for _ in concealment_fields.iter().filter(|entity| !echoed(*entity)) {
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
