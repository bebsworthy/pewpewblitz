//! Client-side queue composition for Practice, matchmaking, loading, transport, and automation.

#[cfg(feature = "automation")]
mod automation;
mod loading;
mod matchmaking;
mod practice;

#[cfg(test)]
mod tests;

use bevy::prelude::*;

#[cfg(all(test, feature = "automation"))]
use automation::automation_game_type;
pub use loading::ClientMatchLoadingModel;
pub(super) use matchmaking::observe_queue_messages;
pub use matchmaking::{ClientQueueModel, PendingQueueCommand};
pub use practice::ClientPracticeModel;

pub struct ClientQueuePlugin;

impl Plugin for ClientQueuePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientQueueModel>()
            .init_resource::<ClientMatchLoadingModel>()
            .init_resource::<ClientPracticeModel>()
            .add_systems(
                Update,
                (
                    observe_queue_messages,
                    loading::observe_matchmaking_messages,
                    matchmaking::update_queue_time,
                    matchmaking::send_queue_messages,
                    loading::send_matchmaking_messages,
                    practice::send_practice_messages,
                )
                    .chain(),
            );
        // Automation-only smoke drivers mutate the queue/Practice models between observation
        // and the outbound drain, exactly where they sat inside the chain above.
        #[cfg(feature = "automation")]
        app.init_resource::<automation::HeadlessQueueSmokeStage>()
            .init_resource::<automation::HeadlessRequeueSmokeStage>()
            .add_systems(
                Update,
                (
                    automation::drive_headless_queue_smoke,
                    automation::drive_headless_requeue_smoke,
                )
                    .chain()
                    .after(matchmaking::update_queue_time)
                    .before(matchmaking::send_queue_messages),
            );
    }
}
