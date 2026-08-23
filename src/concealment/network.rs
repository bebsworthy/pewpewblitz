use bevy::prelude::*;
use lightyear::prelude::VisibilityExt;
use std::collections::HashMap;

#[derive(Resource, Default)]
pub(crate) struct ObserverVisibilityCache(pub(super) HashMap<(Entity, Entity), bool>);

impl ObserverVisibilityCache {
    #[must_use]
    pub(crate) fn permits(&self, connection: Entity, subject: Entity) -> bool {
        self.0.get(&(connection, subject)).copied().unwrap_or(false)
    }
}

#[derive(Resource, Default)]
pub(super) struct QueuedVisibilityTransitions(pub(super) Vec<(Entity, Entity, bool)>);

pub(super) fn apply_queued_observer_visibility(
    mut commands: Commands,
    mut queued: ResMut<QueuedVisibilityTransitions>,
) {
    for (subject, connection, visible) in queued.0.drain(..) {
        if visible {
            commands.gain_visibility(subject, connection);
        } else {
            commands.lose_visibility(subject, connection);
        }
    }
}
