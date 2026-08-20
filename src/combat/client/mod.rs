//! Client-only combat presentation geometry.
#![allow(clippy::wildcard_imports)]

use super::*;
use bevy::prelude::*;
use std::collections::{BTreeMap, HashMap, HashSet, VecDeque};
use std::env;
use std::path::PathBuf;
use std::time::Instant;

mod cues;
mod effects;
mod hud;
mod preview;
#[cfg(test)]
mod tests;
mod world;

pub use cues::CaptureCombatCues;
pub use cues::ClientCombatEvidenceStatus;
pub use cues::ClientCombatObservation;
pub(crate) use cues::RecentCombatEvents;
pub use hud::{BuildSelectionText, CombatAbilityHudText, CombatHudText};
pub use preview::MAX_PREVIEW_SEGMENTS;

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
        app.init_resource::<FighterDefinitions>()
            .init_resource::<WeaponDefinitions>()
            .init_resource::<RecentCombatEvents>()
            .init_resource::<ClientCombatObservation>()
            .init_resource::<ClientCombatEvidenceStatus>()
            .add_message::<DeduplicatedCombatCue>()
            .add_systems(Startup, validate_definitions)
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
                    receive_combat_evidence_checkpoints.in_set(CombatClientSet::Ingest),
                    world::ensure_sentry_visuals.in_set(CombatClientSet::Ensure),
                    world::ensure_dash_trails.in_set(CombatClientSet::Ensure),
                    world::sync_sentry_visuals.in_set(CombatClientSet::Sync),
                    world::sync_dash_trails.in_set(CombatClientSet::Sync),
                    preview::update_weapon_preview.in_set(CombatClientSet::Sync),
                    hud::update_health_bars.in_set(CombatClientSet::HudAndStatus),
                    effects::update_durable_effect_markers.in_set(CombatClientSet::HudAndStatus),
                    hud::update_combat_hud.in_set(CombatClientSet::HudAndStatus),
                    effects::update_combat_effects.in_set(CombatClientSet::Effects),
                    capture_client_combat_checkpoints.in_set(CombatClientSet::Evidence),
                    record_headless_combat_observation.in_set(CombatClientSet::Evidence),
                )
                    // The chain retains every implicit deferred boundary the milestone
                    // locked; no demonstrated-independent edge is relaxed.
                    .chain(),
            );
    }
}
