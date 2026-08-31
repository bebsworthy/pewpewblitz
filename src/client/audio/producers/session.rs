use bevy::prelude::*;

use crate::{
    client::{
        ClientJoinPhase, ClientJoinStatus, ClientPlayableGate,
        audio::{
            registry::{AudioProducerRegistration, AudioProducerRegistrationAppExt},
            request::{AudioCueKey, AudioRequest, cue_keys},
        },
    },
    map::ClientMapReadiness,
};

use super::AudioProducerSequence;

const SESSION_RANK: u16 = 50;
const SESSION_KEYS: &[AudioCueKey] = &[cue_keys::READY, cue_keys::ERROR];

#[derive(Resource, Default)]
pub(in crate::client::audio::producers) struct SessionAudioMemory {
    was_playable: bool,
    was_error: bool,
}

pub(super) fn register(app: &mut App) {
    app.init_resource::<SessionAudioMemory>()
        .try_register_audio_producer(AudioProducerRegistration {
            id: "session",
            rank: SESSION_RANK,
            cue_keys: SESSION_KEYS,
        })
        .expect("session audio producer registration must be valid");
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "resources are Bevy system parameters owned by the schedule"
)]
pub(super) fn produce_session_audio_requests(
    playable: Res<ClientPlayableGate>,
    map: Res<ClientMapReadiness>,
    joins: Query<&ClientJoinStatus>,
    mut memory: ResMut<SessionAudioMemory>,
    mut requests: MessageWriter<AudioRequest>,
    mut sequence: Local<AudioProducerSequence>,
) {
    let is_error = matches!(*map, ClientMapReadiness::Invalid(_))
        || joins.iter().any(|status| {
            matches!(
                status.phase,
                ClientJoinPhase::Rejected(_) | ClientJoinPhase::Disconnected
            )
        });
    let current = SessionAudioSnapshot {
        playable: playable.0,
        is_error,
    };
    let previous = SessionAudioSnapshot {
        playable: memory.was_playable,
        is_error: memory.was_error,
    };
    let cue_key = session_audio_cue(current, previous);
    if let Some(cue_key) = cue_key {
        requests.write(AudioRequest::once(cue_key, sequence.next(SESSION_RANK)));
    }
    memory.was_playable = playable.0;
    memory.was_error = is_error;
}

#[derive(Clone, Copy)]
struct SessionAudioSnapshot {
    playable: bool,
    is_error: bool,
}

fn session_audio_cue(
    current: SessionAudioSnapshot,
    previous: SessionAudioSnapshot,
) -> Option<AudioCueKey> {
    if current.playable && !previous.playable {
        Some(cue_keys::READY)
    } else if current.is_error && !previous.is_error {
        Some(cue_keys::ERROR)
    } else {
        None
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn playable_transition_precedes_error_and_each_transition_is_edge_triggered() {
        let observation = |playable, is_error| SessionAudioSnapshot { playable, is_error };
        assert_eq!(
            session_audio_cue(observation(true, true), observation(false, false)),
            Some(cue_keys::READY)
        );
        assert_eq!(
            session_audio_cue(observation(false, true), observation(false, false)),
            Some(cue_keys::ERROR)
        );
        assert_eq!(
            session_audio_cue(observation(true, true), observation(true, true)),
            None
        );
    }
}
