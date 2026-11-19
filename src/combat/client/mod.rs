//! Client-only combat presentation geometry.

use super::{
    CombatCue, CombatEventId, ConeSprayState, DamageSource, DeliveryMethod, Fighter, FiringPattern,
    MAX_COMBAT_EVIDENCE_EVENTS, NetworkEntityId, PersistentAreaShape, ProjectileBody,
    ResolvedWeapon, TargetSelection, WeaponDefinitionId, delivery, spread_angles,
};
use bevy::prelude::*;
use std::collections::VecDeque;

mod cues;
mod preview;
#[cfg(test)]
mod tests;

#[cfg(feature = "network-test")]
pub use cues::CaptureCombatCues;
pub(crate) use cues::RecentCombatEvents;
pub use preview::{
    AimMark, AimTone, AimTraceBlockerClass, AimTraceBlockerIndex, AimTraceDynamicBlocker,
    MAX_CONE_SPRAY_SEGMENTS, PreviewGeometry, PreviewPrimitive, aim_lane_marks,
    cone_spray_primitives, straight_map_travel, ultimate_area_marks,
};

pub struct ClientCombatPlugin;

/// A combat cue that passed the bounded client deduplication gate.
#[derive(Message, Clone, Debug)]
pub struct DeduplicatedCombatCue(pub CombatCue);

/// Named client combat update phases. The registration below preserves the exact
/// phase order the milestone locked before the split; the names document the
/// demonstrated data/message-flow dependencies (cue ingestion before visual sync, command
/// application before HUD/effect readers). No edge is relaxed
/// without a measured, schedule-tested demonstration of independence.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CombatClientSet {
    /// Receive and deduplicate authoritative cues.
    Ingest,
    /// Spawn presentation entities for replicated combat state.
    Ensure,
    /// Sync world-space visuals with the ingested state.
    Sync,
    /// Health bars, combat HUD, and durable status markers read synced state.
    HudAndStatus,
    /// Bounded transient visual effects.
    Effects,
}

#[cfg(feature = "client")]
impl Plugin for ClientCombatPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<RecentCombatEvents>()
            .add_message::<DeduplicatedCombatCue>()
            .configure_sets(
                Update,
                (
                    CombatClientSet::Ingest,
                    CombatClientSet::Ensure,
                    CombatClientSet::Sync,
                    CombatClientSet::HudAndStatus,
                    CombatClientSet::Effects,
                )
                    .chain(),
            )
            .add_systems(
                Update,
                (
                    cues::receive_combat_cues.in_set(CombatClientSet::Ingest),
                    cues::rumble_spray_feedback.in_set(CombatClientSet::Effects),
                )
                    .chain(),
            );
    }
}
