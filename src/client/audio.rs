//! Client-only semantic audio requests and bounded one-shot playback.

mod catalog;
mod playback;
mod producers;
mod registry;
mod request;

use bevy::prelude::*;

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
            .add_plugins(AudioProducersPlugin);
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
        assert_eq!(registry.producer_rank("hot-zone"), Some(70));
    }
}
