//! Client-only semantic audio requests, bounded one-shot playback, music, and volume mixing.

mod catalog;
mod lifecycle;
mod mix;
mod music;
mod playback;
mod producers;
mod registry;
mod request;

use bevy::{prelude::*, window::AppLifecycle};

use music::ClientMusicPlugin;
use playback::{ClientAudioPlaybackPlugin, ClientAudioSet};
use producers::{AudioProducerSet, AudioProducersPlugin};
use registry::AudioRegistryPlugin;

pub struct ClientAudioPlugin;

impl Plugin for ClientAudioPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(AudioRegistryPlugin)
            .add_plugins(ClientAudioPlaybackPlugin)
            .configure_sets(
                Update,
                AudioProducerSet.in_set(ClientAudioSet::ProduceRequests),
            )
            .add_plugins(AudioProducersPlugin)
            .add_plugins(ClientMusicPlugin)
            // One mix pass after every spawn and fade of the frame.
            .add_systems(
                Update,
                mix::apply_audio_mix
                    .after(ClientAudioSet::PlaybackRequests)
                    .after(music::MusicSet),
            )
            // Android suspends the app with the activity; pause and later resume exactly the
            // sinks that were playing. Idempotent registration keeps window-free tests valid.
            .add_message::<AppLifecycle>()
            .add_systems(
                Update,
                lifecycle::pause_audio_while_suspended.after(mix::apply_audio_mix),
            );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn client_audio_composition_seals_all_builtin_producers() {
        let mut app = App::new();
        app.add_plugins(ClientAudioPlugin);
        app.finish();

        let registry = app.world().resource::<registry::AudioRegistry>();
        for cue_key in request::cue_keys::BUILTIN {
            assert!(registry.contains(cue_key));
        }
        assert_eq!(registry.producer_rank("combat"), Some(10));
        assert_eq!(registry.producer_rank("turf-hold"), Some(70));
        assert_eq!(registry.producer_rank("pickup"), Some(80));
        assert_eq!(registry.producer_rank("ui"), Some(90));
    }
}
