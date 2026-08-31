use bevy::prelude::Message;

pub(crate) const MAX_AUDIO_CUE_KEY_BYTES: usize = 64;

/// Stable client-only semantic identity selected by an audio producer.
///
/// Construction is intentionally cheap and const-friendly. Registration and catalog loading own
/// validation; the playback adapter still treats an unregistered or malformed runtime key as
/// unavailable instead of panicking an update.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct AudioCueKey(&'static str);

impl AudioCueKey {
    pub(crate) const fn new(value: &'static str) -> Self {
        Self(value)
    }

    pub(crate) const fn as_str(self) -> &'static str {
        self.0
    }
}

/// A semantic one-shot request. Playback policy remains entirely catalog- and adapter-owned.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct AudioRequest {
    pub(crate) cue_key: AudioCueKey,
    pub(crate) occurrence: Option<u64>,
    pub(crate) order: AudioRequestOrder,
}

/// Deterministic local ordering and producer provenance for one audio request.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub(crate) struct AudioRequestOrder {
    pub(crate) producer_rank: u16,
    pub(crate) sequence: u64,
}

impl AudioRequestOrder {
    pub(crate) const fn new(producer_rank: u16, sequence: u64) -> Self {
        Self {
            producer_rank,
            sequence,
        }
    }
}

impl AudioRequest {
    pub(crate) const fn once(cue_key: AudioCueKey, order: AudioRequestOrder) -> Self {
        Self {
            cue_key,
            occurrence: None,
            order,
        }
    }

    pub(crate) const fn for_occurrence(
        cue_key: AudioCueKey,
        occurrence: u64,
        order: AudioRequestOrder,
    ) -> Self {
        Self {
            cue_key,
            occurrence: Some(occurrence),
            order,
        }
    }
}

pub(crate) fn validate_audio_cue_key(value: &str) -> Result<(), &'static str> {
    if value.is_empty() || value.len() > MAX_AUDIO_CUE_KEY_BYTES {
        return Err("audio cue keys must be nonempty and bounded");
    }
    if !value.bytes().all(|byte| {
        byte.is_ascii_lowercase() || byte.is_ascii_digit() || matches!(byte, b'.' | b'-')
    }) {
        return Err("audio cue keys may contain only lowercase letters, digits, dots, and hyphens");
    }
    Ok(())
}

pub(crate) mod cue_keys {
    use super::AudioCueKey;

    pub(crate) const FIRE: AudioCueKey = AudioCueKey::new("fire");
    pub(crate) const IMPACT: AudioCueKey = AudioCueKey::new("impact");
    pub(crate) const DEFEAT: AudioCueKey = AudioCueKey::new("defeat");
    pub(crate) const RESET: AudioCueKey = AudioCueKey::new("reset");
    pub(crate) const READY: AudioCueKey = AudioCueKey::new("ready");
    pub(crate) const ERROR: AudioCueKey = AudioCueKey::new("error");
    pub(crate) const DASH: AudioCueKey = AudioCueKey::new("dash");
    pub(crate) const SENTRY: AudioCueKey = AudioCueKey::new("sentry");
    pub(crate) const SENTRY_SPAWN: AudioCueKey = AudioCueKey::new("sentry-spawn");
    pub(crate) const CONCEALMENT_FIELD_SPAWN: AudioCueKey =
        AudioCueKey::new("concealment-field-spawn");
    pub(crate) const CHARGE_READY: AudioCueKey = AudioCueKey::new("charge-ready");
    pub(crate) const PASSIVE: AudioCueKey = AudioCueKey::new("passive");
    pub(crate) const OBJECTIVE_HIT: AudioCueKey = AudioCueKey::new("objective-hit");
    pub(crate) const OBJECTIVE_CRITICAL: AudioCueKey = AudioCueKey::new("objective-critical");
    pub(crate) const OBJECTIVE_DESTROYED: AudioCueKey = AudioCueKey::new("objective-destroyed");
    pub(crate) const RELOAD: AudioCueKey = AudioCueKey::new("reload");

    #[cfg(test)]
    pub(crate) const BUILTIN: [AudioCueKey; 16] = [
        FIRE,
        IMPACT,
        DEFEAT,
        RESET,
        READY,
        ERROR,
        DASH,
        SENTRY,
        SENTRY_SPAWN,
        CONCEALMENT_FIELD_SPAWN,
        CHARGE_READY,
        PASSIVE,
        OBJECTIVE_HIT,
        OBJECTIVE_CRITICAL,
        OBJECTIVE_DESTROYED,
        RELOAD,
    ];
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn keys_accept_the_bounded_stable_vocabulary_only() {
        for key in cue_keys::BUILTIN {
            validate_audio_cue_key(key.as_str()).unwrap();
        }
        for invalid in [
            "",
            "Ready",
            "objective_hit",
            "two words",
            "slash/key",
            "aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa",
        ] {
            assert!(validate_audio_cue_key(invalid).is_err(), "{invalid}");
        }
    }

    #[test]
    fn requests_carry_only_semantics_occurrence_and_deterministic_local_order() {
        let first = AudioRequestOrder::new(10, 1);
        let second = AudioRequestOrder::new(20, 0);
        assert_eq!(
            AudioRequest::once(cue_keys::READY, first),
            AudioRequest {
                cue_key: cue_keys::READY,
                occurrence: None,
                order: first,
            }
        );
        assert_eq!(
            AudioRequest::for_occurrence(cue_keys::FIRE, 42, second),
            AudioRequest {
                cue_key: cue_keys::FIRE,
                occurrence: Some(42),
                order: second,
            }
        );
        assert!(first < second);
        assert!(AudioRequestOrder::new(10, 1) < AudioRequestOrder::new(10, 2));
    }
}
