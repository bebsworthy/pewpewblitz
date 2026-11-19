//! Client-side queue composition for Practice, matchmaking, loading, transport, and automation.

#[cfg(feature = "automation")]
mod automation;
mod loading;
mod matchmaking;
mod practice;
mod publish;

#[cfg(test)]
mod tests;

use bevy::prelude::*;

#[cfg(all(test, feature = "automation"))]
use automation::automation_game_type;
pub use loading::ClientMatchLoadingModel;
pub(super) use matchmaking::observe_queue_messages;
pub use matchmaking::{ClientQueueModel, PendingQueueCommand};
pub use practice::ClientPracticeModel;
pub use publish::{ClientMapPublishModel, map_publish_decision_text};

pub struct ClientQueuePlugin;

impl Plugin for ClientQueuePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientQueueModel>()
            .init_resource::<ClientMatchLoadingModel>()
            .init_resource::<ClientPracticeModel>()
            .init_resource::<ClientMapPublishModel>()
            .add_systems(
                Update,
                (
                    observe_queue_messages,
                    loading::observe_matchmaking_messages,
                    matchmaking::update_queue_time,
                    matchmaking::send_queue_messages,
                    loading::send_matchmaking_messages,
                    practice::send_practice_messages,
                    publish::exchange_map_publish_messages,
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
                    automation::drive_headless_publish_smoke,
                    automation::report_published_map_credit,
                )
                    .chain()
                    .after(matchmaking::update_queue_time)
                    .before(matchmaking::send_queue_messages),
            );
    }
}
