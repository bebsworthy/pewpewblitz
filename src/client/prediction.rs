//! Experimental owner-only movement prediction for the M03 comparison candidate.
//!
//! This feature is NOT part of any supported build: `owner-prediction` is a non-default
//! Cargo feature enabled only by the prediction-comparison measurement. The server
//! remains the sole pose authority; the client only simulates the same deterministic
//! movement rules for immediate presentation and resynchronizes on every authoritative
//! pose. Remote fighters, combat, abilities, map mutation, match rules, and session
//! lifecycle are never predicted.
//!
//! Known candidate limitation recorded by the experiment: collision resolution covers the
//! static arena geometry from the replicated map snapshot only. Destructible map assets is
//! server-authoritative and unmodelled here, so predicted poses can cross still-solid
//! destructible cells until the next authoritative correction.

use crate::combat::{ActiveEffects, AuthoritativePose};
use crate::map::{
    MapCatalogResource, MapDynamicState, MapRoot, ResolvedMapSnapshot,
    resolve_circle_against_blocking_map,
};
use crate::movement::{InputTuning, committed_aim, decoded_move};
use crate::movement::{active_slow_multiplier, adrenaline_multiplier};
use crate::protocol::{Fighter, FighterInput};
use crate::timing::SimulationTick;
use bevy::prelude::*;
use lightyear::prelude::Controlled;
use lightyear::prelude::input::native::ActionState;
use std::collections::VecDeque;

/// How many ticks of predicted history support reconciliation and bounded replay.
pub const OWNER_PREDICTION_HISTORY: usize = 64;

/// Runtime switch for the experimental candidate (environment `BRAWLER_OWNER_PREDICTION=1`).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct OwnerPredictionSettings {
    pub enabled: bool,
}

impl Default for OwnerPredictionSettings {
    fn default() -> Self {
        Self {
            enabled: std::env::var("BRAWLER_OWNER_PREDICTION").as_deref() == Ok("1"),
        }
    }
}

/// One predicted owner pose. Presentation-only; never replicated or authoritative.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct OwnerPredictedPose {
    pub position: Vec2,
    pub facing: f32,
    pub tick: u64,
}

/// Bounded predicted-position history keyed by simulation tick for reconciliation.
#[derive(Component, Default, Debug)]
pub struct OwnerPredictionHistory {
    pub entries: VecDeque<(u64, Vec2)>,
    /// The newest authoritative tick already folded into the predicted base. Poses for
    /// this tick or older never resynchronize again, so a freshly integrated prediction
    /// for the current tick survives replication pipelining.
    pub last_reconciled_tick: u64,
}

/// Correction position error at or above this many world units counts as a correction.
pub const CORRECTION_EPSILON_UNITS: f32 = 0.25;

/// Measured facts for the comparison matrix. Bounded by construction.
#[derive(Resource, Default, Debug)]
pub struct OwnerPredictionStats {
    pub reconciliations: u64,
    pub corrections: u64,
    pub correction_samples: Vec<f32>,
    pub last_correction_error: f32,
    pub predicted_ticks: u64,
}

impl OwnerPredictionStats {
    /// p-correction magnitude over the bounded sample set (0.0 when empty).
    #[must_use]
    pub fn correction_percentile(&self, percentile: f32) -> f32 {
        if self.correction_samples.is_empty() {
            return 0.0;
        }
        let mut sorted = self.correction_samples.clone();
        sorted.sort_by(f32::total_cmp);
        #[allow(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            clippy::cast_precision_loss
        )]
        let index = ((sorted.len() as f32 - 1.0) * percentile.clamp(0.0, 1.0)).round() as usize;
        sorted[index.min(sorted.len() - 1)]
    }
}

/// Installs the experimental owner-prediction candidate on the client application.
pub struct OwnerPredictionPlugin;

impl Plugin for OwnerPredictionPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<InputTuning>()
            .init_resource::<OwnerPredictionSettings>()
            .init_resource::<OwnerPredictionStats>()
            .add_systems(
                FixedUpdate,
                (
                    ensure_owner_prediction_components,
                    reconcile_owner_prediction,
                    predict_owner_movement,
                )
                    .chain()
                    .in_set(crate::gameplay::GameplaySet::Simulation),
            );
    }
}

/// Attach prediction state to the controlled fighter from its first authoritative pose.
fn ensure_owner_prediction_components(
    settings: Res<OwnerPredictionSettings>,
    mut commands: Commands,
    owners: Query<
        (Entity, &AuthoritativePose),
        (With<Fighter>, With<Controlled>, Without<OwnerPredictedPose>),
    >,
) {
    if !settings.enabled {
        return;
    }
    for (entity, pose) in &owners {
        commands.entity(entity).insert((
            OwnerPredictedPose {
                position: Vec2::new(pose.position.x, pose.position.y),
                facing: pose.facing,
                tick: pose.tick,
            },
            OwnerPredictionHistory::default(),
        ));
    }
}

/// Resynchronize the predicted base against every newly received authoritative pose and
/// record the correction magnitude for the tick the prediction was made for.
fn reconcile_owner_prediction(
    settings: Res<OwnerPredictionSettings>,
    mut stats: ResMut<OwnerPredictionStats>,
    tick: Res<SimulationTick>,
    mut owners: Query<
        (
            &AuthoritativePose,
            &mut OwnerPredictedPose,
            &mut OwnerPredictionHistory,
        ),
        (With<Fighter>, With<Controlled>),
    >,
) {
    if !settings.enabled {
        return;
    }
    for (authoritative, mut predicted, mut history) in &mut owners {
        if authoritative.tick <= history.last_reconciled_tick {
            continue;
        }
        stats.reconciliations = stats.reconciliations.saturating_add(1);
        let predicted_at_tick = history
            .entries
            .iter()
            .rev()
            .find(|(entry_tick, _)| *entry_tick == authoritative.tick)
            .map(|(_, position)| *position);
        if let Some(predicted_position) = predicted_at_tick {
            let error = predicted_position.distance(Vec2::new(
                authoritative.position.x,
                authoritative.position.y,
            ));
            stats.last_correction_error = error;
            stats.correction_samples.push(error);
            stats.correction_samples.truncate(OWNER_PREDICTION_HISTORY);
            if error >= CORRECTION_EPSILON_UNITS {
                stats.corrections = stats.corrections.saturating_add(1);
            }
        }
        // Resynchronize the base to the authoritative pose; the prediction for the
        // current tick survives because reconciliation is keyed to last_reconciled_tick.
        predicted.position = Vec2::new(authoritative.position.x, authoritative.position.y);
        predicted.facing = authoritative.facing;
        predicted.tick = predicted.tick.max(authoritative.tick);
        history.last_reconciled_tick = authoritative.tick;
        let _ = tick.0;
    }
}

/// Advance the owner's predicted pose with the shared deterministic movement rules and
/// bounded static-arena resolution from the replicated map snapshot.
#[allow(clippy::too_many_arguments)]
fn predict_owner_movement(
    settings: Res<OwnerPredictionSettings>,
    mut stats: ResMut<OwnerPredictionStats>,
    tick: Res<SimulationTick>,
    time: Res<Time<Fixed>>,
    builds: Res<crate::builds::BuildCatalogResource>,
    input_tuning: Res<InputTuning>,
    maps: Query<(&ResolvedMapSnapshot, &MapDynamicState), With<MapRoot>>,
    catalog: Res<MapCatalogResource>,
    mut owners: Query<
        (
            Option<&ActionState<FighterInput>>,
            &crate::builds::ResolvedMatchLoadout,
            Option<&ActiveEffects>,
            Option<&crate::builds::PassiveRuntimeState>,
            &mut OwnerPredictedPose,
            &mut OwnerPredictionHistory,
        ),
        (With<Fighter>, With<Controlled>),
    >,
) {
    if !settings.enabled {
        return;
    }
    let Ok((snapshot, map_state)) = maps.single() else {
        return;
    };
    let delta = time.delta().as_secs_f32();
    for (action, loadout, effects, passive_state, mut predicted, mut history) in &mut owners {
        let input = action.map_or(FighterInput::default(), |action| action.0);
        let input = if input.is_valid() {
            input
        } else {
            FighterInput::default()
        };
        // Same shaping rules as the authoritative decoder: deadzone on the quantized axis,
        // aim committed only above the threshold.
        let movement = decoded_move(input.move_axis, *input_tuning);
        if let Some(aim) = input
            .aim_update
            .and_then(|axis| committed_aim(axis.to_vec2(), *input_tuning))
        {
            predicted.facing = aim.y.atan2(aim.x);
        }
        let speed = loadout.fighter_stats.movement_speed;
        let velocity = movement
            * speed
            * active_slow_multiplier(effects, tick.0)
            * adrenaline_multiplier(
                loadout
                    .passives
                    .iter()
                    .find(|passive| passive.kind == crate::builds::PassiveKind::AdrenalResponse)
                    .copied(),
                passive_state,
                tick.0,
            );
        let mut position = predicted.position + velocity * delta;
        position = resolve_circle_against_blocking_map(
            position,
            builds.0.fighter_body.radius,
            snapshot,
            map_state,
            &catalog.0,
        );
        predicted.position = position;
        predicted.tick = tick.0.max(history.last_reconciled_tick);
        history.entries.push_back((tick.0, position));
        while history.entries.len() > OWNER_PREDICTION_HISTORY {
            history.entries.pop_front();
        }
        stats.predicted_ticks = stats.predicted_ticks.saturating_add(1);
    }
}
