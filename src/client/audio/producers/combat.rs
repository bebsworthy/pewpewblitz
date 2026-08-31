use bevy::prelude::*;

use crate::{
    client::audio::{
        registry::{AudioProducerRegistration, AudioProducerRegistrationAppExt},
        request::{AudioCueKey, AudioRequest, AudioRequestOrder, cue_keys},
    },
    combat::{CombatCue, DeduplicatedCombatCue},
};

use super::AudioProducerSequence;

const COMBAT_RANK: u16 = 10;
const COMBAT_KEYS: &[AudioCueKey] = &[
    cue_keys::FIRE,
    cue_keys::IMPACT,
    cue_keys::DEFEAT,
    cue_keys::RESET,
    cue_keys::SENTRY,
    cue_keys::READY,
];

pub(super) fn register(app: &mut App) {
    app.try_register_audio_producer(AudioProducerRegistration {
        id: "combat",
        rank: COMBAT_RANK,
        cue_keys: COMBAT_KEYS,
    })
    .expect("combat audio producer registration must be valid");
}

pub(super) fn produce_combat_audio_requests(
    mut cues: MessageReader<DeduplicatedCombatCue>,
    mut requests: MessageWriter<AudioRequest>,
    mut sequence: Local<AudioProducerSequence>,
) {
    for DeduplicatedCombatCue(cue) in cues.read() {
        if let Some(request) = combat_audio_request(cue, sequence.next(COMBAT_RANK)) {
            requests.write(request);
        }
    }
}

fn combat_audio_request(cue: &CombatCue, order: AudioRequestOrder) -> Option<AudioRequest> {
    let (cue_key, occurrence) = match cue {
        CombatCue::AttackAccepted { attack_id, .. } => (cue_keys::FIRE, attack_id.0),
        CombatCue::DeliveryImpact { attack_id, .. }
        | CombatCue::LobLanded { attack_id, .. }
        | CombatCue::MeleeContact { attack_id, .. } => (cue_keys::IMPACT, attack_id.0),
        CombatCue::ConeSprayPulse { event_id, .. }
        | CombatCue::SelfCloakEnded { event_id, .. }
        | CombatCue::DemolitionStrikeActivated { event_id, .. } => (cue_keys::IMPACT, event_id.0),
        CombatCue::FighterDefeated { attack_id, .. } => (cue_keys::DEFEAT, attack_id.0),
        CombatCue::FighterReset { event_id, .. } => (cue_keys::RESET, event_id.0),
        CombatCue::SentryFired { event_id, .. } | CombatCue::DeployableRemoved { event_id, .. } => {
            (cue_keys::SENTRY, event_id.0)
        }
        CombatCue::SelfCloakActivated { event_id, .. }
        | CombatCue::RevealScanActivated { event_id, .. }
        | CombatCue::ForcedRevealApplied { event_id, .. }
        | CombatCue::ElementalFieldActivated { event_id, .. } => (cue_keys::READY, event_id.0),
        CombatCue::DamageApplied { .. }
        | CombatCue::EffectApplied { .. }
        | CombatCue::Muzzle { .. }
        | CombatCue::Impact { .. }
        | CombatCue::Damage { .. }
        | CombatCue::Defeat { .. }
        | CombatCue::Reset { .. } => return None,
    };
    Some(AudioRequest::for_occurrence(cue_key, occurrence, order))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        abilities::SentryCleanupReason,
        builds::DeployableId,
        combat::{CombatEventId, WorldPoint},
        protocol::NetworkEntityId,
    };

    #[test]
    fn sentry_fire_and_removal_keep_their_event_occurrences() {
        let sentry_fire = CombatCue::SentryFired {
            event_id: CombatEventId(92),
            tick: 100,
            owner: NetworkEntityId(7),
            deployable_id: DeployableId(3),
            target: Some(NetworkEntityId(8)),
            position: WorldPoint { x: 20.0, y: 30.0 },
        };
        let removal = CombatCue::DeployableRemoved {
            event_id: CombatEventId(93),
            tick: 101,
            owner: NetworkEntityId(7),
            deployable_id: DeployableId(3),
            position: WorldPoint { x: 20.0, y: 30.0 },
            reason: SentryCleanupReason::Destroyed,
        };

        assert_eq!(
            combat_audio_request(&sentry_fire, AudioRequestOrder::new(COMBAT_RANK, 0)),
            Some(AudioRequest::for_occurrence(
                cue_keys::SENTRY,
                92,
                AudioRequestOrder::new(COMBAT_RANK, 0),
            ))
        );
        assert_eq!(
            combat_audio_request(&removal, AudioRequestOrder::new(COMBAT_RANK, 1)),
            Some(AudioRequest::for_occurrence(
                cue_keys::SENTRY,
                93,
                AudioRequestOrder::new(COMBAT_RANK, 1),
            ))
        );
    }
}
