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
pub use hud::{BuildSelectionText, CombatHudText};
pub use preview::MAX_PREVIEW_SEGMENTS;

pub struct ClientCombatPlugin;

/// A combat cue that passed the bounded client deduplication gate.
#[derive(Message, Clone, Debug)]
pub struct DeduplicatedCombatCue(pub CombatCue);

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
            .add_systems(
                Update,
                (
                    cues::receive_combat_cues,
                    receive_combat_evidence_checkpoints,
                    world::ensure_projectile_visuals,
                    world::ensure_sentry_visuals,
                    world::ensure_dash_trails,
                    world::sync_projectile_visuals,
                    world::sync_sentry_visuals,
                    world::sync_dash_trails,
                    preview::update_weapon_preview,
                    hud::update_health_bars,
                    effects::update_durable_effect_markers,
                    hud::update_combat_hud,
                    effects::update_combat_effects,
                    capture_client_combat_checkpoints,
                    record_headless_combat_observation,
                )
                    .chain(),
            );
    }
}
