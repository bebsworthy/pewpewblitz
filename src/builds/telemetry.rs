use super::{PassiveDefinitionId, SelectedBuild, UltimateDefinitionId};
use crate::combat::WeaponRecipeFingerprint;
use crate::protocol::NetworkEntityId;
use bevy::prelude::Resource;
use std::collections::VecDeque;

pub const MAX_BUILD_TELEMETRY_RECORDS: usize = 1_024;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct BuildSelectionTelemetryRecord {
    pub tick: u64,
    pub request_id: u64,
    pub owner_network_id: NetworkEntityId,
    pub identity: SelectedBuild,
    pub total_points: u8,
    pub weapon_fingerprint: WeaponRecipeFingerprint,
    pub ultimate_id: UltimateDefinitionId,
    pub passive_ids: [PassiveDefinitionId; 2],
}

#[derive(Resource, Debug, Default)]
pub struct BuildTelemetry {
    pub selections: VecDeque<BuildSelectionTelemetryRecord>,
    pub dropped_records: u64,
}

impl BuildTelemetry {
    pub(crate) fn record(&mut self, record: BuildSelectionTelemetryRecord) {
        if self.selections.len() == MAX_BUILD_TELEMETRY_RECORDS {
            self.selections.pop_front();
            self.dropped_records = self.dropped_records.saturating_add(1);
        }
        self.selections.push_back(record);
    }
}
