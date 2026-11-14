//! Mobile app lifecycle: silence every playing sink while the OS suspends the app and resume
//! exactly those sinks when it returns. Desktop platforms never report `Suspended`.

use bevy::{
    audio::{AudioSink, AudioSinkPlayback, SpatialAudioSink},
    prelude::*,
    window::AppLifecycle,
};

/// Marks a sink that this module paused, so resume never restarts audio paused for other reasons.
#[derive(Component)]
pub(super) struct PausedForSuspend;

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(super) fn pause_audio_while_suspended(
    mut commands: Commands,
    mut lifecycle: MessageReader<AppLifecycle>,
    sinks: Query<(Entity, &AudioSink, Has<PausedForSuspend>)>,
    spatial_sinks: Query<(Entity, &SpatialAudioSink, Has<PausedForSuspend>)>,
) {
    let Some(latest) = lifecycle.read().last().copied() else {
        return;
    };
    match latest {
        AppLifecycle::Suspended => {
            for (entity, sink, marked) in &sinks {
                if !marked && !sink.is_paused() {
                    sink.pause();
                    commands.entity(entity).insert(PausedForSuspend);
                }
            }
            for (entity, sink, marked) in &spatial_sinks {
                if !marked && !sink.is_paused() {
                    sink.pause();
                    commands.entity(entity).insert(PausedForSuspend);
                }
            }
        }
        AppLifecycle::Running => {
            for (entity, sink, marked) in &sinks {
                if marked {
                    sink.play();
                    commands.entity(entity).remove::<PausedForSuspend>();
                }
            }
            for (entity, sink, marked) in &spatial_sinks {
                if marked {
                    sink.play();
                    commands.entity(entity).remove::<PausedForSuspend>();
                }
            }
        }
        AppLifecycle::Idle | AppLifecycle::WillSuspend | AppLifecycle::WillResume => {}
    }
}
