//! Bounded per-weapon telemetry aggregation.

use super::{
    ActiveEffects, AttackId, CombatEventId, DistanceBand, ExternalMotion, NetworkEntityId,
    PayloadEffectDefinition, WeaponPresetId, WeaponRecipeFingerprint, WorldPoint,
};
use bevy::prelude::Resource;
use std::collections::{BTreeMap, BTreeSet};

#[derive(Resource, Clone, Debug, Default, PartialEq)]
pub struct WeaponTelemetry {
    pub selections: BTreeMap<WeaponPresetId, u64>,
    pub selections_by_recipe: BTreeMap<WeaponRecipeFingerprint, u64>,
    pub accepted_attacks: BTreeMap<WeaponPresetId, u64>,
    pub emitted_deliveries: BTreeMap<WeaponPresetId, u64>,
    pub hostile_delivery_contacts: BTreeMap<WeaponPresetId, u64>,
    pub attacks_with_hostile_contact: BTreeMap<WeaponPresetId, u64>,
    pub hostile_damage: BTreeMap<WeaponPresetId, u64>,
    pub self_damage: BTreeMap<WeaponPresetId, u64>,
    pub defeats: BTreeMap<WeaponPresetId, u64>,
    pub close_hits: BTreeMap<WeaponPresetId, u64>,
    pub mid_hits: BTreeMap<WeaponPresetId, u64>,
    pub long_hits: BTreeMap<WeaponPresetId, u64>,
    pub close_damage: BTreeMap<WeaponPresetId, u64>,
    pub mid_damage: BTreeMap<WeaponPresetId, u64>,
    pub long_damage: BTreeMap<WeaponPresetId, u64>,
    pub contacted_attacks: BTreeSet<AttackId>,
    pub tracker_drops: u64,
    pub event_reservation_drops: u64,
    pub dropped_aggregate_entries: u64,
    pub dropped_records: u64,
    pub contact_evictions: u64,
    /// Bounded aggregates keyed by both the selected preset and the resolved recipe fingerprint.
    /// The preset is an attribution label; the fingerprint proves which immutable rules produced
    /// the outcome when content revisions reuse a preset ID.
    pub source_aggregates: BTreeMap<WeaponTelemetryKey, WeaponTelemetryAggregate>,
    pub bounded_records: Vec<WeaponTelemetryRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct WeaponTelemetryKey {
    pub preset_id: WeaponPresetId,
    pub recipe_fingerprint: WeaponRecipeFingerprint,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct WeaponTelemetryAggregate {
    pub selections: u64,
    pub accepted_attacks: u64,
    pub emitted_deliveries: u64,
    pub hostile_delivery_contacts: u64,
    pub attacks_with_hostile_contact: u64,
    pub hostile_damage: u64,
    pub self_damage: u64,
    pub defeats: u64,
    pub close_hits: u64,
    pub mid_hits: u64,
    pub long_hits: u64,
    pub close_damage: u64,
    pub mid_damage: u64,
    pub long_damage: u64,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WeaponTelemetryRecord {
    pub tick: u64,
    pub event_id: CombatEventId,
    pub attack_id: AttackId,
    pub preset_id: WeaponPresetId,
    pub recipe_fingerprint: WeaponRecipeFingerprint,
    pub delivery_index: Option<u8>,
    pub source: NetworkEntityId,
    pub target: Option<NetworkEntityId>,
    pub position: WorldPoint,
    pub requested_value: u16,
    pub applied_value: u16,
    pub engagement_distance: f32,
    pub delivery_travel: f32,
    pub hostile_contact: bool,
    pub effect: Option<PayloadEffectDefinition>,
    pub resulting_health: Option<u16>,
    pub resulting_effects: Option<ActiveEffects>,
    pub resulting_motion: Option<ExternalMotion>,
}

const MAX_TELEMETRY_RECORDS: usize = 512;
const MAX_TRACKED_CONTACTS: usize = 512;
const MAX_RECIPE_AGGREGATES: usize = 64;

impl WeaponTelemetry {
    pub fn increment(map: &mut BTreeMap<WeaponPresetId, u64>, preset_id: WeaponPresetId) {
        *map.entry(preset_id).or_default() =
            map.get(&preset_id).copied().unwrap_or(0).saturating_add(1);
    }
    fn with_source_aggregate(
        &mut self,
        preset_id: WeaponPresetId,
        recipe_fingerprint: WeaponRecipeFingerprint,
        update: impl FnOnce(&mut WeaponTelemetryAggregate),
    ) {
        let key = WeaponTelemetryKey {
            preset_id,
            recipe_fingerprint,
        };
        if !self.source_aggregates.contains_key(&key)
            && self.source_aggregates.len() >= MAX_RECIPE_AGGREGATES
        {
            self.dropped_aggregate_entries = self.dropped_aggregate_entries.saturating_add(1);
            return;
        }
        update(self.source_aggregates.entry(key).or_default());
    }

    pub fn record_selection(
        &mut self,
        preset_id: WeaponPresetId,
        recipe_fingerprint: WeaponRecipeFingerprint,
    ) {
        Self::increment(&mut self.selections, preset_id);
        if !self.selections_by_recipe.contains_key(&recipe_fingerprint)
            && self.selections_by_recipe.len() >= MAX_RECIPE_AGGREGATES
        {
            self.dropped_aggregate_entries = self.dropped_aggregate_entries.saturating_add(1);
        } else {
            let count = self
                .selections_by_recipe
                .entry(recipe_fingerprint)
                .or_default();
            *count = count.saturating_add(1);
        }
        self.with_source_aggregate(preset_id, recipe_fingerprint, |aggregate| {
            aggregate.selections = aggregate.selections.saturating_add(1);
        });
    }

    pub fn record_accepted_attack(
        &mut self,
        preset_id: WeaponPresetId,
        recipe_fingerprint: WeaponRecipeFingerprint,
    ) {
        Self::increment(&mut self.accepted_attacks, preset_id);
        self.with_source_aggregate(preset_id, recipe_fingerprint, |aggregate| {
            aggregate.accepted_attacks = aggregate.accepted_attacks.saturating_add(1);
        });
    }

    pub fn record_emitted_deliveries(
        &mut self,
        preset_id: WeaponPresetId,
        recipe_fingerprint: WeaponRecipeFingerprint,
        count: u64,
    ) {
        let emitted = self.emitted_deliveries.entry(preset_id).or_default();
        *emitted = emitted.saturating_add(count);
        self.with_source_aggregate(preset_id, recipe_fingerprint, |aggregate| {
            aggregate.emitted_deliveries = aggregate.emitted_deliveries.saturating_add(count);
        });
    }

    pub fn record_hostile_delivery_contact(
        &mut self,
        preset_id: WeaponPresetId,
        recipe_fingerprint: WeaponRecipeFingerprint,
    ) {
        Self::increment(&mut self.hostile_delivery_contacts, preset_id);
        self.with_source_aggregate(preset_id, recipe_fingerprint, |aggregate| {
            aggregate.hostile_delivery_contacts =
                aggregate.hostile_delivery_contacts.saturating_add(1);
        });
    }

    pub fn record_damage(
        &mut self,
        preset_id: WeaponPresetId,
        recipe_fingerprint: WeaponRecipeFingerprint,
        owner_damage: bool,
        band: DistanceBand,
        amount: u16,
    ) {
        if owner_damage {
            Self::increment(&mut self.self_damage, preset_id);
        } else {
            Self::increment(&mut self.hostile_damage, preset_id);
        }
        let (hits, damage) = match band {
            DistanceBand::Close => (&mut self.close_hits, &mut self.close_damage),
            DistanceBand::Mid => (&mut self.mid_hits, &mut self.mid_damage),
            DistanceBand::Long => (&mut self.long_hits, &mut self.long_damage),
        };
        Self::increment(hits, preset_id);
        let total = damage.entry(preset_id).or_default();
        *total = total.saturating_add(u64::from(amount));
        self.with_source_aggregate(preset_id, recipe_fingerprint, |aggregate| {
            if owner_damage {
                aggregate.self_damage = aggregate.self_damage.saturating_add(1);
            } else {
                aggregate.hostile_damage = aggregate.hostile_damage.saturating_add(1);
            }
            match band {
                DistanceBand::Close => {
                    aggregate.close_hits = aggregate.close_hits.saturating_add(1);
                    aggregate.close_damage =
                        aggregate.close_damage.saturating_add(u64::from(amount));
                }
                DistanceBand::Mid => {
                    aggregate.mid_hits = aggregate.mid_hits.saturating_add(1);
                    aggregate.mid_damage = aggregate.mid_damage.saturating_add(u64::from(amount));
                }
                DistanceBand::Long => {
                    aggregate.long_hits = aggregate.long_hits.saturating_add(1);
                    aggregate.long_damage = aggregate.long_damage.saturating_add(u64::from(amount));
                }
            }
        });
    }

    pub fn record_defeat(
        &mut self,
        preset_id: WeaponPresetId,
        recipe_fingerprint: WeaponRecipeFingerprint,
    ) {
        Self::increment(&mut self.defeats, preset_id);
        self.with_source_aggregate(preset_id, recipe_fingerprint, |aggregate| {
            aggregate.defeats = aggregate.defeats.saturating_add(1);
        });
    }
    pub fn record(&mut self, record: WeaponTelemetryRecord) {
        if self.bounded_records.len() < MAX_TELEMETRY_RECORDS {
            self.bounded_records.push(record);
        } else {
            self.dropped_records = self.dropped_records.saturating_add(1);
        }
    }

    pub fn record_hostile_contact(
        &mut self,
        preset_id: WeaponPresetId,
        recipe_fingerprint: WeaponRecipeFingerprint,
        attack_id: AttackId,
    ) {
        if self.contacted_attacks.contains(&attack_id) {
            return;
        }
        if self.contacted_attacks.len() >= MAX_TRACKED_CONTACTS
            && let Some(oldest) = self.contacted_attacks.pop_first()
        {
            let _ = oldest;
            self.contact_evictions = self.contact_evictions.saturating_add(1);
        }
        self.contacted_attacks.insert(attack_id);
        Self::increment(&mut self.attacks_with_hostile_contact, preset_id);
        self.with_source_aggregate(preset_id, recipe_fingerprint, |aggregate| {
            aggregate.attacks_with_hostile_contact =
                aggregate.attacks_with_hostile_contact.saturating_add(1);
        });
    }
}
