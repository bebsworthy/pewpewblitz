//! Client-only combat presentation geometry.

#[allow(
    clippy::wildcard_imports,
    reason = "the child module consumes its parent module's composition surface"
)]
use super::*;
use bevy::prelude::*;
use std::collections::{BTreeMap, VecDeque};
use std::env;
use std::path::PathBuf;
use std::time::Instant;

mod cues;
mod preview;
#[cfg(test)]
mod tests;

#[cfg(feature = "network-test")]
pub use cues::CaptureCombatCues;
pub use cues::ClientCombatEvidenceStatus;
pub use cues::ClientCombatObservation;
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
/// application before HUD/effect readers, evidence capture last). No edge is relaxed
/// without a measured, schedule-tested demonstration of independence.
#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum CombatClientSet {
    /// Receive and deduplicate authoritative cues and evidence checkpoints.
    Ingest,
    /// Spawn presentation entities for replicated combat state.
    Ensure,
    /// Sync world-space visuals with the ingested state.
    Sync,
    /// Health bars, combat HUD, and durable status markers read synced state.
    HudAndStatus,
    /// Bounded transient visual effects.
    Effects,
    /// Combat evidence capture and headless observation run last.
    Evidence,
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
                    CombatClientSet::Evidence,
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

/// Legacy direct-UDP client combat evidence (`BRAWLER_NETWORK_COMBAT_CLIENT_READY_FILE`): checkpoint
/// reproduction, the ready-file report, and the closeout evidence it feeds. Only `brawler-client`'s
/// `automation` feature installs it, after [`ClientCombatPlugin`]; the player client never does.
/// It reads the automation-inserted [`crate::config::HeadlessAutomation`].
pub struct ClientCombatEvidencePlugin;

#[cfg(feature = "client")]
impl Plugin for ClientCombatEvidencePlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<ClientCombatObservation>()
            .init_resource::<ClientCombatEvidenceStatus>()
            .add_systems(
                Update,
                (
                    // Former chain positions: checkpoint ingestion right after cue ingestion,
                    // checkpoint capture then the ready report last. None defers commands.
                    receive_combat_evidence_checkpoints
                        .in_set(CombatClientSet::Ingest)
                        .after(cues::receive_combat_cues),
                    (
                        capture_client_combat_checkpoints,
                        record_headless_combat_observation,
                    )
                        .chain()
                        .in_set(CombatClientSet::Evidence),
                ),
            );
    }
}
