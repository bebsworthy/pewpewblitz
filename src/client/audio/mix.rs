//! Volume categories: every client audio entity is mixed as master × category × its own base.

use bevy::{
    audio::{AudioSink, AudioSinkPlayback, SpatialAudioSink, Volume},
    prelude::*,
    window::PrimaryWindow,
};

use crate::client::ClientShellSettings;

/// The persisted volume category an audio entity belongs to.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum AudioCategory {
    /// One-shot gameplay, match, and UI sounds.
    Effects,
    /// Looping menu and match music.
    Music,
}

/// Category and authored base volume (profile volume, music track volume × fade) of one entity.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(super) struct AudioMix {
    pub(super) category: AudioCategory,
    pub(super) base: f32,
}

impl AudioMix {
    pub(super) const fn effects(base: f32) -> Self {
        Self {
            category: AudioCategory::Effects,
            base,
        }
    }

    pub(super) const fn music(base: f32) -> Self {
        Self {
            category: AudioCategory::Music,
            base,
        }
    }
}

/// The category's share of the master volume, before the entity's own base volume.
pub(super) fn category_volume(settings: &ClientShellSettings, category: AudioCategory) -> f32 {
    let category = match category {
        AudioCategory::Effects => settings.effects_volume,
        AudioCategory::Music => settings.music_volume,
    };
    f32::from(settings.master_volume) / 100.0 * f32::from(category) / 100.0
}

/// Applies the persisted categories and focus-loss mute to every live sink each frame.
#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn apply_audio_mix(
    settings: Option<Res<ClientShellSettings>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut sinks: Query<(&mut AudioSink, Option<&AudioMix>)>,
    mut spatial_sinks: Query<(&mut SpatialAudioSink, Option<&AudioMix>)>,
) {
    let settings = settings.as_deref().copied().unwrap_or_default();
    let master = f32::from(settings.master_volume) / 100.0;
    // `GlobalVolume` stays at its default of 1: entities spawn with their complete mixed volume
    // (`category_volume × base`), so the first frame already matches this steady-state mix.
    let muted =
        settings.mute_when_unfocused && windows.iter().next().is_some_and(|window| !window.focused);
    let volume = |mix: Option<&AudioMix>| {
        Volume::Linear(mix.map_or(master, |mix| {
            category_volume(&settings, mix.category) * mix.base
        }))
    };
    for (mut sink, mix) in &mut sinks {
        sink.set_volume(volume(mix));
        if muted {
            sink.mute();
        } else {
            sink.unmute();
        }
    }
    for (mut sink, mix) in &mut spatial_sinks {
        sink.set_volume(volume(mix));
        if muted {
            sink.mute();
        } else {
            sink.unmute();
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn categories_scale_the_master_volume_independently() {
        let settings = ClientShellSettings {
            master_volume: 50,
            effects_volume: 80,
            music_volume: 20,
            ..ClientShellSettings::default()
        };
        assert!((category_volume(&settings, AudioCategory::Effects) - 0.4).abs() < 1e-6);
        assert!((category_volume(&settings, AudioCategory::Music) - 0.1).abs() < 1e-6);
        let silent_music = ClientShellSettings {
            music_volume: 0,
            ..ClientShellSettings::default()
        };
        assert!(category_volume(&silent_music, AudioCategory::Music).abs() < f32::EPSILON);
        assert!((category_volume(&silent_music, AudioCategory::Effects) - 1.0).abs() < 1e-6);
    }
}
