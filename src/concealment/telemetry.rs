use crate::{combat::TeamId, protocol::NetworkEntityId};
use bevy::prelude::*;

const MAX_CONCEALMENT_TRANSITIONS: usize = 2_048;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum VisibilityTransitionReason {
    SelfOrAlly,
    PublicOrOutsideTerrain,
    RevealLock,
    Proximity,
    TerrainConcealment,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct VisibilityTransitionRecord {
    pub tick: u64,
    pub observer_team: TeamId,
    pub subject: NetworkEntityId,
    pub visible: bool,
    pub reason: VisibilityTransitionReason,
}

#[derive(Resource, Default, Debug)]
pub(super) struct ConcealmentTelemetry {
    pub transitions: Vec<VisibilityTransitionRecord>,
    pub dropped_transitions: u64,
}

impl ConcealmentTelemetry {
    pub(super) fn record(&mut self, transition: VisibilityTransitionRecord) {
        if self.transitions.len() < MAX_CONCEALMENT_TRANSITIONS {
            self.transitions.push(transition);
        } else {
            self.dropped_transitions = self.dropped_transitions.saturating_add(1);
        }
    }
}
