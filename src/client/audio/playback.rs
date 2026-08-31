use std::collections::VecDeque;

use bevy::{
    audio::{AudioPlayer, AudioSource, PlaybackSettings, Volume},
    prelude::*,
};

use super::{
    catalog::{AudioPlaybackPlan, AudioProfileCatalog},
    registry::AudioRegistry,
    request::{AudioCueKey, AudioRequest},
};
use crate::client::assets::ClientAssetHandles;

const MAX_RECENT_AUDIO_KEYS: usize = 128;

/// Marks transient one-shot entities so the next frame's reservation baseline includes them.
#[derive(Component)]
pub(super) struct ClientAudioOneShot;

/// Shared immediate reservation count for all requests handled during one update.
#[derive(Resource, Default)]
pub(super) struct AudioFrameReservations {
    active_or_reserved: usize,
}

impl AudioFrameReservations {
    fn reset(&mut self, active: usize) {
        self.active_or_reserved = active;
    }

    fn reserve(&mut self, concurrency_cap: usize) -> bool {
        if self.active_or_reserved >= concurrency_cap {
            return false;
        }
        self.active_or_reserved += 1;
        true
    }
}

/// Explicit audio phases keep request production between reservation reset and playback.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) enum ClientAudioSet {
    ResetReservations,
    ProduceRequests,
    PlaybackRequests,
}

#[derive(Resource, Default)]
struct ClientAudioPlaybackState {
    recent_occurrences: VecDeque<(AudioCueKey, u64)>,
    suppressed: u64,
}

impl ClientAudioPlaybackState {
    fn accept(&mut self, request: AudioRequest) -> bool {
        let Some(occurrence) = request.occurrence else {
            return true;
        };
        let key = (request.cue_key, occurrence);
        if self.recent_occurrences.contains(&key) {
            return false;
        }
        remember_audio_key(&mut self.recent_occurrences, key);
        true
    }
}

/// Owns generic request consumption and Bevy one-shot materialization.
pub(super) struct ClientAudioPlaybackPlugin;

impl Plugin for ClientAudioPlaybackPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<AudioProfileCatalog>() {
            app.insert_resource(
                AudioProfileCatalog::embedded().expect("embedded audio profile catalog is valid"),
            );
        }
        app.add_message::<AudioRequest>()
            .init_resource::<ClientAudioPlaybackState>()
            .init_resource::<AudioFrameReservations>()
            .configure_sets(
                Update,
                (
                    ClientAudioSet::ResetReservations,
                    ClientAudioSet::ProduceRequests.after(ClientAudioSet::ResetReservations),
                    ClientAudioSet::PlaybackRequests.after(ClientAudioSet::ProduceRequests),
                ),
            )
            .add_systems(
                Update,
                reset_audio_reservations.in_set(ClientAudioSet::ResetReservations),
            )
            .add_systems(
                Update,
                play_audio_requests.in_set(ClientAudioSet::PlaybackRequests),
            );
    }
}

fn reset_audio_reservations(
    active: Query<(), With<ClientAudioOneShot>>,
    mut reservations: ResMut<AudioFrameReservations>,
) {
    reservations.reset(active.iter().count());
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn play_audio_requests(
    mut commands: Commands,
    handles: Option<Res<ClientAssetHandles>>,
    catalog: Res<AudioProfileCatalog>,
    registry: Option<Res<AudioRegistry>>,
    asset_server: Option<Res<AssetServer>>,
    mut requests: MessageReader<AudioRequest>,
    mut state: ResMut<ClientAudioPlaybackState>,
    mut reservations: ResMut<AudioFrameReservations>,
) {
    let mut pending = requests.read().copied().collect::<Vec<_>>();
    sort_audio_requests(&mut pending);
    for request in pending {
        // A malformed or unknown runtime key fails closed and cannot evict valid deduplication
        // history. Startup registry validation remains the normal coverage guarantee.
        if !registry
            .as_deref()
            .is_some_and(|registry| registry.allows(&request))
            || !catalog.contains_mapping(request.cue_key)
            || !state.accept(request)
        {
            continue;
        }
        if !matches!(
            try_play_audio(
                &mut commands,
                &catalog,
                handles.as_deref(),
                asset_server.as_deref(),
                request.cue_key,
                &mut reservations,
            ),
            PlaybackAttempt::Capped
        ) {
            continue;
        }

        state.suppressed = state.suppressed.saturating_add(1);
        if state.suppressed.is_power_of_two() {
            warn!(
                suppressed = state.suppressed,
                cue_key = request.cue_key.as_str(),
                live_limit = catalog
                    .playback_plan(request.cue_key, |_| true)
                    .map_or(0, |plan| plan.concurrency_cap),
                "client audio one-shot cap suppressed a cue"
            );
        }
    }
}

fn sort_audio_requests(requests: &mut [AudioRequest]) {
    requests.sort_unstable_by_key(|request| request.order);
}

fn remember_audio_key(recent: &mut VecDeque<(AudioCueKey, u64)>, key: (AudioCueKey, u64)) {
    if recent.len() == MAX_RECENT_AUDIO_KEYS {
        recent.pop_front();
    }
    recent.push_back(key);
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum PlaybackAttempt {
    Played,
    Capped,
    Unavailable,
}

fn try_play_audio(
    commands: &mut Commands,
    catalog: &AudioProfileCatalog,
    handles: Option<&ClientAssetHandles>,
    asset_server: Option<&AssetServer>,
    cue_key: AudioCueKey,
    reservations: &mut AudioFrameReservations,
) -> PlaybackAttempt {
    let Some(plan) = catalog.playback_plan(cue_key, |asset_id| {
        handles
            .and_then(|handles| handles.audio(asset_id))
            .is_some_and(|handle| asset_server.is_some_and(|server| server.is_loaded(handle)))
    }) else {
        return PlaybackAttempt::Unavailable;
    };
    let handle = handles
        .and_then(|handles| handles.audio(plan.asset_id))
        .expect("resolved audio asset ID has a retained handle")
        .clone();
    materialize_audio_plan(commands, handle, plan, reservations)
}

fn materialize_audio_plan(
    commands: &mut Commands,
    handle: Handle<AudioSource>,
    plan: AudioPlaybackPlan<'_>,
    reservations: &mut AudioFrameReservations,
) -> PlaybackAttempt {
    if !reservations.reserve(plan.concurrency_cap) {
        return PlaybackAttempt::Capped;
    }
    commands.spawn((
        ClientAudioOneShot,
        AudioPlayer::new(handle),
        PlaybackSettings {
            speed: plan.speed,
            volume: Volume::Linear(plan.volume),
            ..PlaybackSettings::DESPAWN
        },
    ));
    PlaybackAttempt::Played
}

#[cfg(test)]
mod tests {
    use bevy::audio::PlaybackMode;

    use super::super::registry::{
        AudioProducerRegistration, AudioProducerRegistrationAppExt, AudioRegistryPlugin,
    };
    use super::super::request::{AudioRequestOrder, cue_keys};
    use super::*;

    const TEST_ORDER: AudioRequestOrder = AudioRequestOrder::new(10, 0);

    #[test]
    fn audio_key_history_is_bounded_and_preserves_newest_key() {
        let mut state = ClientAudioPlaybackState::default();
        for occurrence in 0..=MAX_RECENT_AUDIO_KEYS {
            assert!(state.accept(AudioRequest::for_occurrence(
                cue_keys::IMPACT,
                occurrence as u64,
                TEST_ORDER,
            )));
        }

        assert_eq!(state.recent_occurrences.len(), MAX_RECENT_AUDIO_KEYS);
        assert_eq!(
            state.recent_occurrences.front(),
            Some(&(cue_keys::IMPACT, 1))
        );
        assert_eq!(
            state.recent_occurrences.back(),
            Some(&(cue_keys::IMPACT, MAX_RECENT_AUDIO_KEYS as u64))
        );
    }

    #[test]
    fn equal_occurrences_for_different_cue_keys_deduplicate_independently() {
        let mut state = ClientAudioPlaybackState::default();

        assert!(state.accept(AudioRequest::for_occurrence(cue_keys::FIRE, 42, TEST_ORDER,)));
        assert!(state.accept(AudioRequest::for_occurrence(
            cue_keys::IMPACT,
            42,
            TEST_ORDER,
        )));
        assert!(!state.accept(AudioRequest::for_occurrence(cue_keys::FIRE, 42, TEST_ORDER,)));
        assert!(!state.accept(AudioRequest::for_occurrence(
            cue_keys::IMPACT,
            42,
            TEST_ORDER,
        )));
    }

    #[test]
    fn once_requests_are_never_deduplicated() {
        let mut state = ClientAudioPlaybackState::default();
        let request = AudioRequest::once(cue_keys::READY, TEST_ORDER);

        assert!(state.accept(request));
        assert!(state.accept(request));
        assert!(state.recent_occurrences.is_empty());
    }

    #[test]
    fn scrambled_requests_sort_by_registered_rank_then_producer_sequence() {
        let mut requests = [
            AudioRequest::once(cue_keys::READY, AudioRequestOrder::new(50, 1)),
            AudioRequest::once(cue_keys::IMPACT, AudioRequestOrder::new(10, 2)),
            AudioRequest::once(cue_keys::FIRE, AudioRequestOrder::new(10, 1)),
            AudioRequest::once(cue_keys::RELOAD, AudioRequestOrder::new(40, 1)),
        ];

        sort_audio_requests(&mut requests);

        assert_eq!(
            requests.map(|request| request.cue_key),
            [
                cue_keys::FIRE,
                cue_keys::IMPACT,
                cue_keys::RELOAD,
                cue_keys::READY,
            ]
        );
    }

    #[test]
    fn same_frame_producers_share_one_immediate_audio_reservation_cap() {
        let mut reservations = AudioFrameReservations::default();
        reservations.reset(23);

        assert!(reservations.reserve(24));
        assert!(!reservations.reserve(24));
        assert_eq!(reservations.active_or_reserved, 24);
    }

    fn materialize_test_plan(
        mut commands: Commands,
        mut reservations: ResMut<AudioFrameReservations>,
    ) {
        let plan = AudioPlaybackPlan {
            asset_id: "test.audio",
            speed: 1.25,
            volume: 0.5,
            concurrency_cap: 2,
        };
        assert_eq!(
            materialize_audio_plan(
                &mut commands,
                Handle::<AudioSource>::default(),
                plan,
                &mut reservations,
            ),
            PlaybackAttempt::Played
        );
    }

    #[test]
    fn materialization_spawns_a_bounded_despawning_one_shot() {
        let mut app = App::new();
        app.init_resource::<AudioFrameReservations>()
            .add_systems(Update, materialize_test_plan);

        app.update();

        let world = app.world_mut();
        let mut one_shots = world.query_filtered::<&PlaybackSettings, With<ClientAudioOneShot>>();
        let settings = *one_shots.single(world).unwrap();
        assert!(matches!(settings.mode, PlaybackMode::Despawn));
        assert!((settings.speed - 1.25).abs() < f32::EPSILON);
        assert!((settings.volume.to_linear() - 0.5).abs() < f32::EPSILON);
    }

    #[derive(Resource, Default)]
    struct ScheduleTrace(Vec<&'static str>);

    fn trace_reset(mut trace: ResMut<ScheduleTrace>) {
        trace.0.push("reset");
    }

    fn trace_produce(mut trace: ResMut<ScheduleTrace>) {
        trace.0.push("produce");
    }

    fn trace_playback(mut trace: ResMut<ScheduleTrace>) {
        trace.0.push("playback");
    }

    #[test]
    fn playback_plugin_enforces_reset_produce_playback_order() {
        let mut app = App::new();
        app.add_plugins(ClientAudioPlaybackPlugin)
            .init_resource::<ScheduleTrace>()
            .add_systems(
                Update,
                trace_reset.in_set(ClientAudioSet::ResetReservations),
            )
            .add_systems(
                Update,
                trace_produce.in_set(ClientAudioSet::ProduceRequests),
            )
            .add_systems(
                Update,
                trace_playback.in_set(ClientAudioSet::PlaybackRequests),
            );
        crate::test_app::finalize(&mut app);

        app.update();

        assert_eq!(
            app.world().resource::<ScheduleTrace>().0,
            ["reset", "produce", "playback"]
        );
    }

    #[test]
    fn registered_request_without_runtime_assets_degrades_to_terminal_silence() {
        const TEST_KEYS: &[AudioCueKey] = &[cue_keys::READY];
        let mut app = App::new();
        app.add_plugins(AudioRegistryPlugin);
        app.try_register_audio_producer(AudioProducerRegistration {
            id: "test",
            rank: 10,
            cue_keys: TEST_KEYS,
        })
        .unwrap();
        // Exact catalog coverage is required at finalization, so this focused test registers the
        // remaining built-ins under a second producer while exercising READY through rank 10.
        app.try_register_audio_producer(AudioProducerRegistration {
            id: "remaining",
            rank: 20,
            cue_keys: &[
                cue_keys::FIRE,
                cue_keys::IMPACT,
                cue_keys::DEFEAT,
                cue_keys::RESET,
                cue_keys::ERROR,
                cue_keys::DASH,
                cue_keys::SENTRY,
                cue_keys::SENTRY_SPAWN,
                cue_keys::CONCEALMENT_FIELD_SPAWN,
                cue_keys::CHARGE_READY,
                cue_keys::PASSIVE,
                cue_keys::OBJECTIVE_HIT,
                cue_keys::OBJECTIVE_CRITICAL,
                cue_keys::OBJECTIVE_DESTROYED,
                cue_keys::RELOAD,
            ],
        })
        .unwrap();
        app.add_plugins(ClientAudioPlaybackPlugin);
        crate::test_app::finalize(&mut app);
        app.world_mut().write_message(AudioRequest::once(
            cue_keys::READY,
            AudioRequestOrder::new(10, 0),
        ));

        app.update();

        let world = app.world_mut();
        let mut one_shots = world.query_filtered::<Entity, With<ClientAudioOneShot>>();
        assert_eq!(one_shots.iter(world).count(), 0);
    }
}
