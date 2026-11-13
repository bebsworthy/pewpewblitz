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

    pub(crate) const FIRE_STRAIGHT: AudioCueKey = AudioCueKey::new("fire-straight");
    pub(crate) const FIRE_STICKY: AudioCueKey = AudioCueKey::new("fire-sticky");
    pub(crate) const FIRE_LOBBED: AudioCueKey = AudioCueKey::new("fire-lobbed");
    pub(crate) const FIRE_SPLASH: AudioCueKey = AudioCueKey::new("fire-splash");
    pub(crate) const FIRE_MELEE: AudioCueKey = AudioCueKey::new("fire-melee");
    pub(crate) const FIRE_SPRAY: AudioCueKey = AudioCueKey::new("fire-spray");
    pub(crate) const IMPACT_PROJECTILE: AudioCueKey = AudioCueKey::new("impact-projectile");
    pub(crate) const IMPACT_LOB: AudioCueKey = AudioCueKey::new("impact-lob");
    pub(crate) const IMPACT_MELEE: AudioCueKey = AudioCueKey::new("impact-melee");
    pub(crate) const IMPACT_SPRAY: AudioCueKey = AudioCueKey::new("impact-spray");
    pub(crate) const HIT_DEALT: AudioCueKey = AudioCueKey::new("hit-dealt");
    pub(crate) const HIT_TAKEN: AudioCueKey = AudioCueKey::new("hit-taken");
    pub(crate) const DEFEAT: AudioCueKey = AudioCueKey::new("defeat");
    pub(crate) const RESPAWN: AudioCueKey = AudioCueKey::new("respawn");
    pub(crate) const SENTRY_FIRE: AudioCueKey = AudioCueKey::new("sentry-fire");
    pub(crate) const SENTRY_REMOVED: AudioCueKey = AudioCueKey::new("sentry-removed");
    pub(crate) const ULTIMATE_CLOAK: AudioCueKey = AudioCueKey::new("ultimate-cloak");
    pub(crate) const ULTIMATE_UNCLOAK: AudioCueKey = AudioCueKey::new("ultimate-uncloak");
    pub(crate) const ULTIMATE_SCAN: AudioCueKey = AudioCueKey::new("ultimate-scan");
    pub(crate) const ULTIMATE_REVEALED: AudioCueKey = AudioCueKey::new("ultimate-revealed");
    pub(crate) const ULTIMATE_FIELD: AudioCueKey = AudioCueKey::new("ultimate-field");
    pub(crate) const ULTIMATE_STRIKE: AudioCueKey = AudioCueKey::new("ultimate-strike");
    pub(crate) const OBJECTIVE_HIT: AudioCueKey = AudioCueKey::new("objective-hit");
    pub(crate) const OBJECTIVE_CRITICAL: AudioCueKey = AudioCueKey::new("objective-critical");
    pub(crate) const OBJECTIVE_DESTROYED: AudioCueKey = AudioCueKey::new("objective-destroyed");
    pub(crate) const DASH: AudioCueKey = AudioCueKey::new("dash");
    pub(crate) const CHARGE_READY: AudioCueKey = AudioCueKey::new("charge-ready");
    pub(crate) const PASSIVE: AudioCueKey = AudioCueKey::new("passive");
    pub(crate) const SENTRY_SPAWN: AudioCueKey = AudioCueKey::new("sentry-spawn");
    pub(crate) const CONCEALMENT_FIELD_SPAWN: AudioCueKey =
        AudioCueKey::new("concealment-field-spawn");
    pub(crate) const RELOAD: AudioCueKey = AudioCueKey::new("reload");
    pub(crate) const PLAYABLE: AudioCueKey = AudioCueKey::new("playable");
    pub(crate) const ERROR: AudioCueKey = AudioCueKey::new("error");
    pub(crate) const MATCH_START: AudioCueKey = AudioCueKey::new("match-start");
    pub(crate) const MATCH_SCORE: AudioCueKey = AudioCueKey::new("match-score");
    pub(crate) const MATCH_VICTORY: AudioCueKey = AudioCueKey::new("match-victory");
    pub(crate) const MATCH_DEFEAT: AudioCueKey = AudioCueKey::new("match-defeat");
    pub(crate) const MATCH_DRAW: AudioCueKey = AudioCueKey::new("match-draw");
    pub(crate) const ZONE_CAPTURED: AudioCueKey = AudioCueKey::new("zone-captured");
    pub(crate) const ZONE_LOST: AudioCueKey = AudioCueKey::new("zone-lost");
    pub(crate) const ZONE_PROGRESS: AudioCueKey = AudioCueKey::new("zone-progress");
    pub(crate) const PICKUP_SPAWNED: AudioCueKey = AudioCueKey::new("pickup-spawned");
    pub(crate) const PICKUP_COLLECTED: AudioCueKey = AudioCueKey::new("pickup-collected");
    pub(crate) const UI_CLICK: AudioCueKey = AudioCueKey::new("ui-click");
    pub(crate) const UI_CONFIRM: AudioCueKey = AudioCueKey::new("ui-confirm");
    pub(crate) const UI_BACK: AudioCueKey = AudioCueKey::new("ui-back");

    #[cfg(test)]
    pub(crate) const BUILTIN: [AudioCueKey; 46] = [
        FIRE_STRAIGHT,
        FIRE_STICKY,
        FIRE_LOBBED,
        FIRE_SPLASH,
        FIRE_MELEE,
        FIRE_SPRAY,
        IMPACT_PROJECTILE,
        IMPACT_LOB,
        IMPACT_MELEE,
        IMPACT_SPRAY,
        HIT_DEALT,
        HIT_TAKEN,
        DEFEAT,
        RESPAWN,
        SENTRY_FIRE,
        SENTRY_REMOVED,
        ULTIMATE_CLOAK,
        ULTIMATE_UNCLOAK,
        ULTIMATE_SCAN,
        ULTIMATE_REVEALED,
        ULTIMATE_FIELD,
        ULTIMATE_STRIKE,
        OBJECTIVE_HIT,
        OBJECTIVE_CRITICAL,
        OBJECTIVE_DESTROYED,
        DASH,
        CHARGE_READY,
        PASSIVE,
        SENTRY_SPAWN,
        CONCEALMENT_FIELD_SPAWN,
        RELOAD,
        PLAYABLE,
        ERROR,
        MATCH_START,
        MATCH_SCORE,
        MATCH_VICTORY,
        MATCH_DEFEAT,
        MATCH_DRAW,
        ZONE_CAPTURED,
        ZONE_LOST,
        ZONE_PROGRESS,
        PICKUP_SPAWNED,
        PICKUP_COLLECTED,
        UI_CLICK,
        UI_CONFIRM,
        UI_BACK,
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
            AudioRequest::once(cue_keys::PLAYABLE, first),
            AudioRequest {
                cue_key: cue_keys::PLAYABLE,
                occurrence: None,
                order: first,
            }
        );
        assert_eq!(
            AudioRequest::for_occurrence(cue_keys::FIRE_STRAIGHT, 42, second),
            AudioRequest {
                cue_key: cue_keys::FIRE_STRAIGHT,
                occurrence: Some(42),
                order: second,
            }
        );
        assert!(first < second);
        assert!(AudioRequestOrder::new(10, 1) < AudioRequestOrder::new(10, 2));
    }
}
