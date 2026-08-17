//! Bounded server-side terrain telemetry records and aggregates.
#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::wildcard_imports,
    reason = "bounded counter aggregation over checked sizes and the shared model mirror"
)]

use super::model::*;
use crate::combat::AttackId;
use bevy::prelude::Resource;
use std::collections::{BTreeSet, VecDeque};

/// The exact rejection reason for one recovery request.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainRecoveryRejection {
    UnknownLink,
    WrongGeneration,
    OversizedRequest,
    CooldownActive,
    ResponseAlreadyStaged,
}

/// One bounded terrain telemetry record.
#[derive(Clone, Debug, PartialEq)]
pub struct TerrainTelemetryRecord {
    pub tick: u64,
    pub map_instance_id: crate::map::MapInstanceId,
    pub revision: u64,
    pub source_attack_id: Option<AttackId>,
    pub delivery_index: Option<u8>,
    pub brush: Option<TerrainBrush>,
    pub affected_chunks: Vec<TerrainChunkId>,
    pub erased_cells: u16,
    pub rebuilt_colliders: usize,
    pub serialized_event_bytes: Option<usize>,
    pub outcome: TerrainTelemetryOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TerrainTelemetryOutcome {
    Applied,
    NoOccupiedCell,
    DeferredRebuildBudget,
    RejectedQueueFull,
    Reset,
    RecoverySent { bytes: usize, chunks: usize },
    RecoveryRejected { reason: TerrainRecoveryRejection },
    ClientGapObserved,
    ClientDuplicateIgnored,
    ClientSnapshotApplied,
    DefensiveRepair,
}

/// Match-scoped aggregate counters over terrain behavior.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct TerrainTelemetryAggregates {
    pub requested_brushes: u64,
    pub applied_brushes: u64,
    pub no_op_brushes: u64,
    pub deferred_brushes: u64,
    pub rejected_brushes: u64,
    pub cells_erased: u64,
    pub occupancy_dirty_chunks: BTreeSet<TerrainChunkId>,
    pub collision_rebuilt_chunks: BTreeSet<TerrainChunkId>,
    pub visual_dirty_chunks: BTreeSet<TerrainChunkId>,
    pub max_brushes_in_one_tick: u64,
    pub max_collider_rebuilds_in_one_tick: u64,
    pub collider_voxels_before: u64,
    pub collider_voxels_after: u64,
    pub empty_chunks: usize,
    pub events_sent: u64,
    pub event_min_bytes: Option<usize>,
    pub event_max_bytes: Option<usize>,
    pub event_total_bytes: u64,
    pub recovery_requests: u64,
    pub recovery_responses: u64,
    pub recovery_rejections: u64,
    pub recovery_snapshot_chunks: u64,
    pub recovery_snapshot_bytes: u64,
    pub client_gaps: u64,
    pub client_duplicates: u64,
    pub client_snapshots_applied: u64,
    pub defensive_repairs: u64,
    pub dropped_records: u64,
}

/// Bounded terrain telemetry ownership.
#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct TerrainTelemetry {
    pub records: VecDeque<TerrainTelemetryRecord>,
    pub aggregates: TerrainTelemetryAggregates,
}

impl TerrainTelemetry {
    /// Record one event, dropping the oldest record when the bound is hit.
    pub fn record(&mut self, record: TerrainTelemetryRecord) {
        if self.records.len() >= MAX_TERRAIN_TELEMETRY_RECORDS {
            self.records.pop_front();
            self.aggregates.dropped_records = self.aggregates.dropped_records.saturating_add(1);
        }
        let aggregates = &mut self.aggregates;
        match record.outcome {
            TerrainTelemetryOutcome::Applied => {
                aggregates.applied_brushes = aggregates.applied_brushes.saturating_add(1);
                aggregates.cells_erased = aggregates
                    .cells_erased
                    .saturating_add(u64::from(record.erased_cells));
                aggregates
                    .occupancy_dirty_chunks
                    .extend(record.affected_chunks.iter().copied());
                aggregates
                    .visual_dirty_chunks
                    .extend(record.affected_chunks.iter().copied());
            }
            TerrainTelemetryOutcome::NoOccupiedCell => {
                aggregates.no_op_brushes = aggregates.no_op_brushes.saturating_add(1);
            }
            TerrainTelemetryOutcome::DeferredRebuildBudget => {
                aggregates.deferred_brushes = aggregates.deferred_brushes.saturating_add(1);
            }
            TerrainTelemetryOutcome::RejectedQueueFull => {
                aggregates.rejected_brushes = aggregates.rejected_brushes.saturating_add(1);
            }
            TerrainTelemetryOutcome::Reset => {}
            TerrainTelemetryOutcome::RecoverySent { bytes, chunks } => {
                aggregates.recovery_responses = aggregates.recovery_responses.saturating_add(1);
                aggregates.recovery_snapshot_chunks = aggregates
                    .recovery_snapshot_chunks
                    .saturating_add(chunks as u64);
                aggregates.recovery_snapshot_bytes = aggregates
                    .recovery_snapshot_bytes
                    .saturating_add(bytes as u64);
            }
            TerrainTelemetryOutcome::RecoveryRejected { .. } => {
                aggregates.recovery_rejections = aggregates.recovery_rejections.saturating_add(1);
            }
            TerrainTelemetryOutcome::ClientGapObserved => {
                aggregates.client_gaps = aggregates.client_gaps.saturating_add(1);
            }
            TerrainTelemetryOutcome::ClientDuplicateIgnored => {
                aggregates.client_duplicates = aggregates.client_duplicates.saturating_add(1);
            }
            TerrainTelemetryOutcome::ClientSnapshotApplied => {
                aggregates.client_snapshots_applied =
                    aggregates.client_snapshots_applied.saturating_add(1);
            }
            TerrainTelemetryOutcome::DefensiveRepair => {
                aggregates.defensive_repairs = aggregates.defensive_repairs.saturating_add(1);
            }
        }
        if let Some(bytes) = record.serialized_event_bytes {
            aggregates.events_sent = aggregates.events_sent.saturating_add(1);
            aggregates.event_total_bytes =
                aggregates.event_total_bytes.saturating_add(bytes as u64);
            aggregates.event_min_bytes = Some(match aggregates.event_min_bytes {
                Some(current) => current.min(bytes),
                None => bytes,
            });
            aggregates.event_max_bytes = Some(match aggregates.event_max_bytes {
                Some(current) => current.max(bytes),
                None => bytes,
            });
        }
        self.records.push_back(record);
    }

    /// Observe one brush request before admission.
    pub fn record_request(&mut self) {
        self.aggregates.requested_brushes = self.aggregates.requested_brushes.saturating_add(1);
    }

    /// Observe the per-tick maxima of admitted brushes and collider rebuilds.
    pub fn record_tick_maxima(&mut self, brushes: u64, rebuilds: u64) {
        self.aggregates.max_brushes_in_one_tick =
            self.aggregates.max_brushes_in_one_tick.max(brushes);
        self.aggregates.max_collider_rebuilds_in_one_tick = self
            .aggregates
            .max_collider_rebuilds_in_one_tick
            .max(rebuilds);
    }

    /// Observe collider voxel totals after one transaction and the current empty count.
    pub fn record_collider_state(&mut self, before: u64, after: u64, empty_chunks: usize) {
        self.aggregates.collider_voxels_before = before;
        self.aggregates.collider_voxels_after = after;
        self.aggregates.empty_chunks = empty_chunks;
    }

    /// Observe one recovery request arrival.
    pub fn record_recovery_request(&mut self) {
        self.aggregates.recovery_requests = self.aggregates.recovery_requests.saturating_add(1);
    }
}
