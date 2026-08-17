//! Client terrain convergence recovery: derive the expected generation from replicated
//! map and match state, request and apply recovery, and process live terrain traffic.
//!
//! This module owns no gameplay collision. Occupancy committed here is presentation
//! input for the windowed composition.

use crate::map::{MapRoot, ResolvedMapSnapshot};
use crate::matchplay::{MatchRoot as MatchRootMarker, MatchState};
use crate::protocol::TerrainChannel;
use crate::terrain::model::{
    TerrainDestructionEvent, TerrainGeneration, TerrainRecoveryRequest, TerrainRecoverySnapshot,
    TerrainResetEvent,
};
use crate::terrain::network::{
    ClientTerrainConvergence, TerrainConvergenceAction, TerrainConvergencePhase,
};
use crate::terrain::telemetry::{
    TerrainTelemetry, TerrainTelemetryOutcome, TerrainTelemetryRecord,
};
use crate::timing::SimulationTick;
use bevy::prelude::*;
use lightyear::prelude::client::Client;
use lightyear::prelude::{MessageReceiver, MessageSender};
use std::collections::BTreeMap;

use super::{ClientTerrainReadiness, ExpectedClientTerrain, ExpectedClientTerrainSlot};

/// Silent ticks after one recovery request before the client re-arms it. Doubles the
/// server's per-link cooldown so a served exchange is never double-counted.
const RECOVERY_REQUEST_RETRY_TICKS: u64 = 60;

/// Observe the replicated map snapshot and match state, re-derive the expected terrain
/// generation when either changes, and request recovery through the pure machine.
pub(super) fn derive_expected_client_terrain(
    mut convergence: ResMut<ClientTerrainConvergence>,
    mut expected: ResMut<ExpectedClientTerrainSlot>,
    snapshots: Query<&ResolvedMapSnapshot, With<MapRoot>>,
    matches: Query<&MatchState, With<MatchRootMarker>>,
) {
    let derived = match (snapshots.single(), matches.single()) {
        (Ok(snapshot), Ok(match_state)) => {
            let pair = (snapshot.identity.instance_id, match_state.match_id);
            if let ExpectedClientTerrainSlot::Derived(current) = &*expected
                && current.derived_from == pair
            {
                return;
            }
            let layout = crate::map::resolve_initial_terrain(
                snapshot.playable_bounds,
                &snapshot.geometry,
                &snapshot.regions,
                &snapshot.spawn_points,
                &snapshot.mode_anchors,
                crate::map::EngineMapLimits::default(),
            );
            Some((pair, layout))
        }
        _ => None,
    };
    let derived = match derived {
        Some((pair, Ok(layout))) => ExpectedClientTerrain {
            generation: TerrainGeneration {
                map_instance_id: pair.0,
                match_id: pair.1,
                terrain_fingerprint: layout.terrain_fingerprint,
            },
            layout,
            derived_from: pair,
        },
        Some((_, Err(_))) => {
            if *expected != ExpectedClientTerrainSlot::Waiting {
                convergence.clear();
            }
            *expected = ExpectedClientTerrainSlot::Failed(
                "replicated map snapshot failed terrain layout validation".to_string(),
            );
            return;
        }
        None => {
            if *expected != ExpectedClientTerrainSlot::Waiting {
                convergence.clear();
                *expected = ExpectedClientTerrainSlot::Waiting;
            }
            return;
        }
    };
    let _ = convergence.observe_generation(derived.generation, &derived.layout.chunks);
    *expected = ExpectedClientTerrainSlot::Derived(derived);
}

/// One client-local convergence telemetry record: only the tick, generation identity,
/// and revision it observed are meaningful.
fn client_convergence_record(
    tick: u64,
    generation: TerrainGeneration,
    revision: u64,
    outcome: TerrainTelemetryOutcome,
) -> TerrainTelemetryRecord {
    TerrainTelemetryRecord {
        tick,
        map_instance_id: generation.map_instance_id,
        revision,
        source_attack_id: None,
        delivery_index: None,
        brush: None,
        affected_chunks: Vec::new(),
        erased_cells: 0,
        rebuilt_colliders: 0,
        serialized_event_bytes: None,
        outcome,
    }
}

/// Record the convergence facts the pure machine signals only through its action: a
/// duplicate revision and a revision gap observed from an already-committed state.
pub(crate) fn classify_client_event(
    convergence: &ClientTerrainConvergence,
    event: &TerrainDestructionEvent,
    tick: u64,
    telemetry: &mut TerrainTelemetry,
) {
    let TerrainConvergencePhase::Ready { generation } = convergence.phase else {
        return;
    };
    if event.generation != generation {
        return;
    }
    let committed = convergence.revision();
    let outcome = if event.revision <= committed {
        TerrainTelemetryOutcome::ClientDuplicateIgnored
    } else if event.revision > committed.saturating_add(1) {
        TerrainTelemetryOutcome::ClientGapObserved
    } else {
        return;
    };
    telemetry.record(client_convergence_record(
        tick,
        generation,
        event.revision,
        outcome,
    ));
}

/// Record one applied recovery snapshot against the convergence machine's committed
/// generation. Called only after `apply_snapshot` committed new authoritative state.
pub(crate) fn record_snapshot_application(
    convergence: &ClientTerrainConvergence,
    snapshot_revision: u64,
    tick: u64,
    telemetry: &mut TerrainTelemetry,
) {
    if let TerrainConvergencePhase::Ready { generation } = convergence.phase {
        telemetry.record(client_convergence_record(
            tick,
            generation,
            snapshot_revision,
            TerrainTelemetryOutcome::ClientSnapshotApplied,
        ));
    }
}

/// Clear the convergence telemetry whenever the machine's generation changes. The
/// generation switches only when the derived map/match observation changes (caught at
/// this system's start) or when an applied reset commits the next match generation
/// (checked immediately after `apply_reset`); gaps and duplicates of a discarded
/// generation are not facts of the new one.
pub(crate) fn clear_telemetry_on_generation_change(
    convergence: &ClientTerrainConvergence,
    telemetry: &mut TerrainTelemetry,
    telemetry_generation: &mut Option<TerrainGeneration>,
) {
    let current = convergence.phase_generation();
    if *telemetry_generation != current {
        *telemetry = TerrainTelemetry::default();
        *telemetry_generation = current;
    }
}

/// Receive terrain traffic, drive the pure convergence machine, and send at most one
/// outstanding recovery request for the awaited generation.
#[allow(clippy::too_many_arguments)]
pub(super) fn drive_terrain_wire_convergence(
    tick: Option<Res<SimulationTick>>,
    mut last_request_tick: Local<Option<u64>>,
    mut telemetry_generation: Local<Option<TerrainGeneration>>,
    expected: Res<ExpectedClientTerrainSlot>,
    mut convergence: ResMut<ClientTerrainConvergence>,
    mut telemetry: ResMut<TerrainTelemetry>,
    mut readiness: ResMut<ClientTerrainReadiness>,
    mut requests: Query<&mut MessageSender<TerrainRecoveryRequest>, With<Client>>,
    mut snapshots: Query<Option<&mut MessageReceiver<TerrainRecoverySnapshot>>, With<Client>>,
    mut resets: Query<Option<&mut MessageReceiver<TerrainResetEvent>>, With<Client>>,
    mut events: Query<Option<&mut MessageReceiver<TerrainDestructionEvent>>, With<Client>>,
) {
    let observed = match &*expected {
        ExpectedClientTerrainSlot::Derived(current) => Some(current.generation),
        _ => None,
    };
    let empty = BTreeMap::new();
    let initial_chunks = match &*expected {
        ExpectedClientTerrainSlot::Derived(current) => &current.layout.chunks,
        _ => &empty,
    };
    let tick = tick.map_or(0, |tick| tick.0);
    clear_telemetry_on_generation_change(&convergence, &mut telemetry, &mut telemetry_generation);
    for receiver in &mut snapshots {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for snapshot in receiver.receive() {
            let action = convergence.apply_snapshot(&snapshot, initial_chunks);
            if action == TerrainConvergenceAction::Applied {
                record_snapshot_application(&convergence, snapshot.revision, tick, &mut telemetry);
            }
            report_invalid(action);
        }
    }
    for receiver in &mut resets {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for reset in receiver.receive() {
            report_invalid(convergence.apply_reset(reset, observed, initial_chunks));
            clear_telemetry_on_generation_change(
                &convergence,
                &mut telemetry,
                &mut telemetry_generation,
            );
        }
    }
    for receiver in &mut events {
        let Some(mut receiver) = receiver else {
            continue;
        };
        for event in receiver.receive() {
            classify_client_event(&convergence, &event, tick, &mut telemetry);
            report_invalid(convergence.apply_event(event));
        }
    }
    // One outstanding request, re-armed after a bounded silent window so a lost request
    // or response on an unreliable transport cannot wedge convergence forever.
    let tick = SimulationTick(tick);
    let mut resend = false;
    match convergence.phase {
        TerrainConvergencePhase::AwaitingRecovery {
            generation,
            request_pending: false,
            ..
        } => {
            for mut sender in &mut requests {
                sender.send::<TerrainChannel>(TerrainRecoveryRequest { generation });
            }
            convergence.mark_request_sent();
            *last_request_tick = Some(tick.0);
        }
        TerrainConvergencePhase::AwaitingRecovery {
            request_pending: true,
            ..
        } => {
            resend = last_request_tick
                .is_some_and(|sent| tick.0.saturating_sub(sent) >= RECOVERY_REQUEST_RETRY_TICKS);
        }
        _ => {}
    }
    if resend
        && let TerrainConvergencePhase::AwaitingRecovery { generation, .. } = convergence.phase
    {
        for mut sender in &mut requests {
            sender.send::<TerrainChannel>(TerrainRecoveryRequest { generation });
        }
        *last_request_tick = Some(tick.0);
    }
    refresh_terrain_readiness(&mut readiness, &convergence, &expected);
}

/// Derive the user-facing readiness observation from the committed convergence phase.
/// Runs after the Update-stage readiness writers so the clamp is authoritative for the
/// next sampled frame.
pub(super) fn refresh_terrain_readiness(
    readiness: &mut ClientTerrainReadiness,
    convergence: &ClientTerrainConvergence,
    expected: &ExpectedClientTerrainSlot,
) {
    let was_ready = matches!(*readiness, ClientTerrainReadiness::Ready);
    *readiness = match &convergence.phase {
        TerrainConvergencePhase::WaitingForMap => match expected {
            ExpectedClientTerrainSlot::Failed(reason) => {
                ClientTerrainReadiness::Invalid(reason.clone())
            }
            _ => ClientTerrainReadiness::WaitingForMap,
        },
        TerrainConvergencePhase::AwaitingRecovery {
            request_pending, ..
        } => {
            if *request_pending {
                ClientTerrainReadiness::RecoveringTerrain
            } else {
                ClientTerrainReadiness::SyncingTerrain
            }
        }
        TerrainConvergencePhase::Ready { generation } => {
            if !was_ready {
                info!(
                    map_instance = generation.map_instance_id.0,
                    revision = convergence.revision(),
                    "client terrain converged to authoritative state"
                );
            }
            ClientTerrainReadiness::Ready
        }
        TerrainConvergencePhase::Invalid { reason, .. } => {
            ClientTerrainReadiness::Invalid(reason.clone())
        }
    };
}

pub(super) fn report_invalid(action: TerrainConvergenceAction) {
    if let TerrainConvergenceAction::Invalidated(reason) = action {
        warn!(
            reason,
            "client terrain convergence entered an invalid state"
        );
    }
}

/// Disconnect clears every generation-scoped convergence state, including `Invalid`.
pub(super) fn clear_terrain_convergence_on_disconnect(
    clients: Query<
        (),
        (
            With<Client>,
            Without<lightyear::prelude::client::Disconnected>,
        ),
    >,
    mut convergence: ResMut<ClientTerrainConvergence>,
) {
    if !clients.is_empty() {
        return;
    }
    if convergence.phase != TerrainConvergencePhase::WaitingForMap {
        convergence.clear();
    }
}
