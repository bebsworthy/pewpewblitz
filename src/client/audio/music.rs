//! Menu and match music: one looping track selected from the client flow, crossfaded on change.
//!
//! Selection is semantic (`menu`/`match`); the authored catalog owns the asset, volume and loop
//! length. Missing or still-loading assets degrade to silence and start once loaded; nothing in
//! navigation, readiness or gameplay waits on music.

use core::time::Duration;

use bevy::{
    audio::{AudioPlayer, PlaybackSettings, Volume},
    prelude::*,
};

use super::{catalog::AudioProfileCatalog, mix::AudioMix};
use crate::client::{ClientFlow, assets::ClientAssetHandles};

pub(super) const MENU_MUSIC: &str = "menu";
pub(super) const MATCH_MUSIC: &str = "match";
pub(super) const MUSIC_KEYS: [&str; 2] = [MENU_MUSIC, MATCH_MUSIC];

/// Seconds for a track to fade in, and for the replaced track to fade out.
const MUSIC_FADE_SECONDS: f32 = 0.8;
/// Fading-out tracks beyond this bound stop immediately, so rapid flow changes stay bounded.
const MAX_FADING_MUSIC: usize = 2;

/// One live music entity.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub(super) struct ClientMusic {
    key: &'static str,
    track_volume: f32,
    /// 0 → 1 while fading in or playing; counts down while fading out.
    fade: f32,
    fading_out: bool,
}

/// Selection, start/replace, and fade of music, before the frame's volume mix.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub(super) struct MusicSet;

/// The music key the client wants now; `None` keeps silence.
#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct MusicSelection(pub(super) Option<&'static str>);

pub(super) struct ClientMusicPlugin;

impl Plugin for ClientMusicPlugin {
    fn build(&self, app: &mut App) {
        if !app.world().contains_resource::<AudioProfileCatalog>() {
            app.insert_resource(
                AudioProfileCatalog::embedded().expect("embedded audio profile catalog is valid"),
            );
        }
        app.world()
            .resource::<AudioProfileCatalog>()
            .validate_music_keys(MUSIC_KEYS)
            .expect("music selection keys exactly cover the authored music tracks");
        app.init_resource::<MusicSelection>().add_systems(
            Update,
            (select_music, play_selected_music, fade_music)
                .chain()
                .in_set(MusicSet),
        );
    }
}

/// Match music during a match; the loading screen keeps whatever is already playing so a short
/// load does not bounce between tracks; every other screen plays the menu track.
fn music_for_flow(flow: ClientFlow, current: Option<&'static str>) -> Option<&'static str> {
    match flow {
        ClientFlow::Match => Some(MATCH_MUSIC),
        ClientFlow::MatchLoading => current.or(Some(MENU_MUSIC)),
        _ => Some(MENU_MUSIC),
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "the flow state is a read-only Bevy system resource"
)]
fn select_music(flow: Option<Res<State<ClientFlow>>>, mut selection: ResMut<MusicSelection>) {
    let next = flow.and_then(|flow| music_for_flow(*flow.get(), selection.0));
    if selection.0 != next {
        selection.0 = next;
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn play_selected_music(
    mut commands: Commands,
    selection: Res<MusicSelection>,
    catalog: Res<AudioProfileCatalog>,
    handles: Option<Res<ClientAssetHandles>>,
    asset_server: Option<Res<AssetServer>>,
    mut music: Query<(Entity, &mut ClientMusic)>,
) {
    let playing = music
        .iter()
        .find(|(_, track)| !track.fading_out)
        .map(|(_, track)| track.key);
    if playing == selection.0 {
        return;
    }
    // Start the selected track first; while its asset is still loading the current track keeps
    // playing rather than dropping to silence.
    let next = selection.0.and_then(|key| {
        let plan = catalog.music_plan(key)?;
        let handle = handles.as_deref()?.audio(plan.asset_id)?;
        asset_server
            .as_deref()
            .is_some_and(|server| server.is_loaded(handle))
            .then(|| (key, plan, handle.clone()))
    });
    if selection.0.is_some() && next.is_none() {
        return;
    }
    for (_, mut track) in &mut music {
        track.fading_out = true;
    }
    let mut fading = music
        .iter()
        .filter(|(_, track)| track.fading_out)
        .map(|(entity, track)| (track.fade, entity))
        .collect::<Vec<_>>();
    // Keep the loudest fading tracks; stop the rest now.
    fading.sort_by(|left, right| right.0.total_cmp(&left.0));
    for (_, entity) in fading.into_iter().skip(MAX_FADING_MUSIC) {
        commands.entity(entity).despawn();
    }
    let Some((key, plan, handle)) = next else {
        return;
    };
    commands.spawn((
        ClientMusic {
            key,
            track_volume: plan.volume,
            fade: 0.0,
            fading_out: false,
        },
        // Starts silent; `fade_music` raises the base and the mix applies the Music category.
        AudioMix::music(0.0),
        AudioPlayer::new(handle),
        PlaybackSettings {
            volume: Volume::Linear(0.0),
            // Trimming to the authored loop keeps encoder padding out of the seam.
            duration: Some(Duration::from_secs_f32(plan.loop_seconds)),
            ..PlaybackSettings::LOOP
        },
    ));
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn fade_music(
    mut commands: Commands,
    time: Res<Time<Real>>,
    mut music: Query<(Entity, &mut ClientMusic, &mut AudioMix)>,
) {
    let step = time.delta_secs() / MUSIC_FADE_SECONDS;
    for (entity, mut track, mut mix) in &mut music {
        if let Some(fade) = advance_fade(*track, step) {
            track.fade = fade;
            mix.base = track.track_volume * fade;
        } else {
            commands.entity(entity).despawn();
        }
    }
}

/// The next fade level, or `None` once a fading-out track is silent.
fn advance_fade(track: ClientMusic, step: f32) -> Option<f32> {
    if track.fading_out {
        let fade = track.fade - step;
        (fade > 0.0).then_some(fade)
    } else {
        Some((track.fade + step).min(1.0))
    }
}

#[cfg(test)]
mod tests {
    use bevy::ecs::system::RunSystemOnce;

    use super::*;

    #[test]
    fn match_flow_plays_match_music_and_loading_keeps_the_current_track() {
        assert_eq!(music_for_flow(ClientFlow::Match, None), Some(MATCH_MUSIC));
        assert_eq!(
            music_for_flow(ClientFlow::Dashboard, Some(MATCH_MUSIC)),
            Some(MENU_MUSIC)
        );
        assert_eq!(
            music_for_flow(ClientFlow::Results, Some(MATCH_MUSIC)),
            Some(MENU_MUSIC)
        );
        assert_eq!(
            music_for_flow(ClientFlow::MatchLoading, Some(MATCH_MUSIC)),
            Some(MATCH_MUSIC)
        );
        assert_eq!(
            music_for_flow(ClientFlow::MatchLoading, None),
            Some(MENU_MUSIC)
        );
    }

    #[test]
    fn fades_rise_to_full_and_fall_to_removal() {
        let mut track = ClientMusic {
            key: MENU_MUSIC,
            track_volume: 0.8,
            fade: 0.0,
            fading_out: false,
        };
        track.fade = advance_fade(track, 0.6).unwrap();
        track.fade = advance_fade(track, 0.6).unwrap();
        assert!((track.fade - 1.0).abs() < f32::EPSILON);
        track.fading_out = true;
        track.fade = advance_fade(track, 0.6).unwrap();
        assert!(advance_fade(track, 0.6).is_none());
    }

    #[test]
    fn music_waits_silently_for_assets_and_replaces_tracks_with_a_bounded_crossfade() {
        let mut app = App::new();
        app.add_plugins(ClientMusicPlugin)
            .init_resource::<Time<Real>>();
        crate::test_app::finalize(&mut app);

        // No flow state and no asset handles: silence, no entity.
        app.update();
        assert_eq!(app.world().resource::<MusicSelection>().0, None);
        let mut query = app.world_mut().query::<&ClientMusic>();
        assert_eq!(query.iter(app.world()).count(), 0);

        // A selection whose asset cannot load yet still spawns nothing.
        app.world_mut().resource_mut::<MusicSelection>().0 = Some(MATCH_MUSIC);
        app.world_mut()
            .run_system_once(play_selected_music)
            .unwrap();
        assert_eq!(query.iter(app.world()).count(), 0);

        // Existing tracks fade out (bounded) when the selection changes to silence.
        for key in [MENU_MUSIC, MATCH_MUSIC, MENU_MUSIC] {
            app.world_mut().spawn((
                ClientMusic {
                    key,
                    track_volume: 1.0,
                    fade: 1.0,
                    fading_out: false,
                },
                AudioMix::music(1.0),
            ));
        }
        app.world_mut().resource_mut::<MusicSelection>().0 = None;
        app.world_mut()
            .run_system_once(play_selected_music)
            .unwrap();
        app.world_mut().flush();
        let tracks = query.iter(app.world()).copied().collect::<Vec<_>>();
        assert_eq!(tracks.len(), MAX_FADING_MUSIC);
        assert!(tracks.iter().all(|track| track.fading_out));
    }
}
