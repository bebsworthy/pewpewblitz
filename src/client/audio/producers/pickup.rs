use bevy::prelude::*;

use crate::{
    client::audio::{
        registry::{AudioProducerRegistration, AudioProducerRegistrationAppExt},
        request::{AudioCueKey, AudioRequest, AudioRequestOrder, cue_keys},
    },
    map::{MapDynamicState, MapRoot, PickupCue, ReceivedPickupCues},
};

use super::AudioProducerSequence;

const PICKUP_RANK: u16 = 80;
const PICKUP_KEYS: &[AudioCueKey] = &[cue_keys::PICKUP_SPAWNED, cue_keys::PICKUP_COLLECTED];

pub(super) fn register(app: &mut App) {
    app.try_register_audio_producer(AudioProducerRegistration {
        id: "pickup",
        rank: PICKUP_RANK,
        cue_keys: PICKUP_KEYS,
    })
    .expect("pickup audio producer registration must be valid");
}

/// Restoration drops and collections from this frame's pickup inbox (shared with pickup VFX).
#[allow(
    clippy::needless_pass_by_value,
    reason = "the inbox is a read-only Bevy system resource"
)]
pub(super) fn produce_pickup_audio_requests(
    received: Option<Res<ReceivedPickupCues>>,
    map_states: Query<&MapDynamicState, With<MapRoot>>,
    mut requests: MessageWriter<AudioRequest>,
    mut sequence: Local<AudioProducerSequence>,
) {
    let Some(received) = received else { return };
    let Ok(map_state) = map_states.single() else {
        return;
    };
    let generation = map_state.generation_id();
    for cue in &received.0 {
        if let Some(request) = pickup_audio_request(*cue, generation, sequence.next(PICKUP_RANK)) {
            requests.write(request);
        }
    }
}

fn pickup_audio_request(
    cue: PickupCue,
    active_generation: crate::map::MapDynamicGeneration,
    order: AudioRequestOrder,
) -> Option<AudioRequest> {
    let (identity, cue_key) = match cue {
        PickupCue::Spawned { identity, .. } => (identity, cue_keys::PICKUP_SPAWNED),
        PickupCue::Collected { identity, .. } => (identity, cue_keys::PICKUP_COLLECTED),
        PickupCue::Expired { .. } => return None,
    };
    (identity.generation == active_generation)
        .then(|| AudioRequest::for_occurrence(cue_key, cue.event_id().0, order))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{
        combat::{CombatEventId, WorldPoint},
        map::{
            MapDynamicGeneration, MapInstanceId, MapPlacementId, RestorationPickupDefinitionId,
            RestorationPickupIdentity,
        },
    };

    const ORDER: AudioRequestOrder = AudioRequestOrder::new(PICKUP_RANK, 0);

    fn generation(value: u64) -> MapDynamicGeneration {
        MapDynamicGeneration {
            map_instance_id: MapInstanceId(7),
            generation: value,
        }
    }

    fn identity(value: u64) -> RestorationPickupIdentity {
        RestorationPickupIdentity {
            generation: generation(value),
            source_placement_id: MapPlacementId(11),
        }
    }

    #[test]
    fn current_generation_drops_and_collections_sound_and_expiry_is_silent() {
        let spawned = PickupCue::Spawned {
            event_id: CombatEventId(9),
            tick: 10,
            identity: identity(2),
            definition_id: RestorationPickupDefinitionId(12),
            position: WorldPoint { x: 2.0, y: 3.0 },
        };
        let collected = PickupCue::Collected {
            event_id: CombatEventId(10),
            tick: 20,
            identity: identity(2),
            position: WorldPoint { x: 2.0, y: 3.0 },
            collector: None,
            applied_restoration: 5,
        };
        let expired = PickupCue::Expired {
            event_id: CombatEventId(11),
            tick: 30,
            identity: identity(2),
            position: WorldPoint { x: 2.0, y: 3.0 },
        };
        assert_eq!(
            pickup_audio_request(spawned, generation(2), ORDER),
            Some(AudioRequest::for_occurrence(
                cue_keys::PICKUP_SPAWNED,
                9,
                ORDER
            ))
        );
        assert_eq!(
            pickup_audio_request(collected, generation(2), ORDER),
            Some(AudioRequest::for_occurrence(
                cue_keys::PICKUP_COLLECTED,
                10,
                ORDER
            ))
        );
        assert_eq!(pickup_audio_request(expired, generation(2), ORDER), None);
        assert_eq!(pickup_audio_request(spawned, generation(1), ORDER), None);
    }
}
