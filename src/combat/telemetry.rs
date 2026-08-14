//! Bounded per-weapon telemetry aggregation.

use super::{
    ActiveEffects, AttackDelivery, AttackId, AuthoritativeTick, CombatEventId, Defeated,
    DistanceBand, ExternalMotion, KnockbackFeedback, NetworkEntityId, PayloadEffectDefinition,
    ProjectileDeadline, ReplicatedAttackSource, ResolvedWeapon, SelectedBuild, StraightFlight,
    WeaponPresetId, WeaponRecipeFingerprint, WeaponState, WorldPoint,
};
use bevy::prelude::{Resource, Vec2};
use serde::{Deserialize, Serialize};
use std::collections::BTreeMap;
use std::fmt::Write as _;

pub const MAX_STATE_SNAPSHOT_BYTES: usize = 32 * 1024;

/// Stable, bounded state evidence used by the two-client impairment harness. It deliberately
/// contains only network-visible gameplay state and stable IDs; ECS entities and presentation
/// objects never enter the comparison.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CombatStateSnapshot {
    pub authoritative_tick: u64,
    pub fighters: Vec<CombatFighterSnapshot>,
    pub projectiles: Vec<CombatProjectileSnapshot>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CombatFighterSnapshot {
    pub network_entity_id: NetworkEntityId,
    pub selected_build: Option<SelectedBuild>,
    pub resolved_weapon: Option<ResolvedWeapon>,
    pub weapon_state: Option<WeaponState>,
    pub active_effects: Option<ActiveEffects>,
    pub knockback_feedback: Option<KnockbackFeedback>,
    pub defeated: Option<Defeated>,
    pub health: Option<u16>,
    pub position: WorldPoint,
    pub facing: f32,
    pub authoritative_tick: AuthoritativeTick,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct CombatProjectileSnapshot {
    pub attack_id: AttackId,
    pub delivery_index: u8,
    pub presentation_profile_id: Option<super::WeaponPresentationProfileId>,
    pub recipe_fingerprint: Option<WeaponRecipeFingerprint>,
    pub position: WorldPoint,
    pub lobbed_flight: Option<super::LobbedFlight>,
    pub deadline: Option<ProjectileDeadline>,
}

impl CombatProjectileSnapshot {
    #[must_use]
    pub fn from_components(
        position: WorldPoint,
        delivery: Option<&AttackDelivery>,
        source: Option<&ReplicatedAttackSource>,
        lobbed_flight: Option<&super::LobbedFlight>,
        deadline: Option<&ProjectileDeadline>,
        straight_flight: Option<&StraightFlight>,
        authoritative_tick: u64,
    ) -> Option<Self> {
        let delivery = delivery?;
        let position = straight_flight
            .map(|flight| {
                let elapsed_ticks = authoritative_tick.saturating_sub(flight.launched_at_tick);
                let distance = (elapsed_ticks as f32 * flight.speed / 60.0)
                    .min(flight.maximum_range)
                    .max(0.0);
                WorldPoint::from(
                    flight.origin.as_vec2() + Vec2::from_angle(flight.facing) * distance,
                )
            })
            .or_else(|| {
                lobbed_flight.map(|flight| {
                    let progress = authoritative_tick.saturating_sub(flight.launched_at_tick)
                        as f32
                        / flight
                            .lands_at_tick
                            .saturating_sub(flight.launched_at_tick)
                            .max(1) as f32;
                    WorldPoint::from(
                        flight.launch.as_vec2()
                            + (flight.landing.as_vec2() - flight.launch.as_vec2())
                                * progress.clamp(0.0, 1.0),
                    )
                })
            })
            .unwrap_or(position);
        Some(Self {
            attack_id: delivery.attack_id,
            delivery_index: delivery.delivery_index,
            presentation_profile_id: source.map(|source| source.attack.presentation_profile_id),
            recipe_fingerprint: source.map(|source| source.attack.recipe_fingerprint),
            position,
            lobbed_flight: lobbed_flight.copied(),
            deadline: deadline.copied(),
        })
    }
}

#[must_use]
pub fn encode_state_snapshot(snapshot: &CombatStateSnapshot) -> Option<String> {
    postcard::to_allocvec(snapshot)
        .ok()
        .filter(|bytes| bytes.len() <= MAX_STATE_SNAPSHOT_BYTES)
        .map(|bytes| {
            let mut encoded = String::with_capacity(bytes.len() * 2);
            for byte in bytes {
                let _ = write!(encoded, "{byte:02x}");
            }
            encoded
        })
}

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
    pub hostile_damage_events: BTreeMap<WeaponPresetId, u64>,
    pub self_damage_events: BTreeMap<WeaponPresetId, u64>,
    pub defeats: BTreeMap<WeaponPresetId, u64>,
    pub close_hits: BTreeMap<WeaponPresetId, u64>,
    pub mid_hits: BTreeMap<WeaponPresetId, u64>,
    pub long_hits: BTreeMap<WeaponPresetId, u64>,
    pub close_damage: BTreeMap<WeaponPresetId, u64>,
    pub mid_damage: BTreeMap<WeaponPresetId, u64>,
    pub long_damage: BTreeMap<WeaponPresetId, u64>,
    pub tracker_drops: u64,
    pub event_reservation_drops: u64,
    pub dropped_aggregate_entries: u64,
    pub dropped_records: u64,
    /// Bounded aggregates keyed by both the selected preset and the resolved recipe fingerprint.
    /// The preset is an attribution label; the fingerprint proves which immutable rules produced
    /// the outcome when content revisions reuse a preset ID.
    pub source_aggregates: BTreeMap<WeaponTelemetryKey, WeaponTelemetryAggregate>,
    pub selection_records: Vec<WeaponSelectionTelemetryRecord>,
    pub dropped_selection_records: u64,
    pub bounded_records: Vec<WeaponTelemetryRecord>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Ord, PartialOrd)]
pub struct WeaponTelemetryKey {
    pub preset_id: WeaponPresetId,
    pub recipe_fingerprint: WeaponRecipeFingerprint,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct WeaponSelectionTelemetryRecord {
    pub tick: u64,
    pub request_id: u64,
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
    pub hostile_damage_events: u64,
    pub self_damage_events: u64,
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
    pub outcome: WeaponTelemetryOutcome,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum WeaponTelemetryOutcome {
    SelectionAccepted,
    AttackAccepted,
    DeliveryImpact,
    DeliveryLanding,
    DeliveryExpired,
    DeliveryCancelled,
    MeleeContact,
    DamageApplied,
    KnockbackApplied,
    SlowApplied,
    Defeat,
}

const MAX_TELEMETRY_RECORDS: usize = 512;
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
        tick: u64,
        request_id: u64,
    ) {
        if self.selection_records.len() < MAX_TELEMETRY_RECORDS {
            self.selection_records.push(WeaponSelectionTelemetryRecord {
                tick,
                request_id,
                preset_id,
                recipe_fingerprint,
            });
        } else {
            self.dropped_selection_records = self.dropped_selection_records.saturating_add(1);
        }
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
            let total = self.self_damage.entry(preset_id).or_default();
            *total = total.saturating_add(u64::from(amount));
            Self::increment(&mut self.self_damage_events, preset_id);
        } else {
            let total = self.hostile_damage.entry(preset_id).or_default();
            *total = total.saturating_add(u64::from(amount));
            Self::increment(&mut self.hostile_damage_events, preset_id);
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
                aggregate.self_damage = aggregate.self_damage.saturating_add(u64::from(amount));
                aggregate.self_damage_events = aggregate.self_damage_events.saturating_add(1);
            } else {
                aggregate.hostile_damage =
                    aggregate.hostile_damage.saturating_add(u64::from(amount));
                aggregate.hostile_damage_events = aggregate.hostile_damage_events.saturating_add(1);
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

    pub fn record_attack_completion(
        &mut self,
        preset_id: WeaponPresetId,
        recipe_fingerprint: WeaponRecipeFingerprint,
        had_hostile_contact: bool,
    ) {
        if !had_hostile_contact {
            return;
        }
        Self::increment(&mut self.attacks_with_hostile_contact, preset_id);
        self.with_source_aggregate(preset_id, recipe_fingerprint, |aggregate| {
            aggregate.attacks_with_hostile_contact =
                aggregate.attacks_with_hostile_contact.saturating_add(1);
        });
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const PRESET: WeaponPresetId = WeaponPresetId(1);
    const FINGERPRINT: WeaponRecipeFingerprint = WeaponRecipeFingerprint(7);

    #[test]
    fn damage_aggregates_sum_amount_and_keep_event_counts_separate() {
        let mut telemetry = WeaponTelemetry::default();
        telemetry.record_damage(PRESET, FINGERPRINT, false, DistanceBand::Close, 25);
        telemetry.record_damage(PRESET, FINGERPRINT, false, DistanceBand::Close, 25);
        telemetry.record_damage(PRESET, FINGERPRINT, true, DistanceBand::Close, 20);
        assert_eq!(telemetry.hostile_damage.get(&PRESET), Some(&50));
        assert_eq!(telemetry.hostile_damage_events.get(&PRESET), Some(&2));
        assert_eq!(telemetry.self_damage.get(&PRESET), Some(&20));
        assert_eq!(telemetry.self_damage_events.get(&PRESET), Some(&1));
        let aggregate = telemetry
            .source_aggregates
            .get(&WeaponTelemetryKey {
                preset_id: PRESET,
                recipe_fingerprint: FINGERPRINT,
            })
            .expect("source aggregate");
        assert_eq!(aggregate.hostile_damage, 50);
        assert_eq!(aggregate.hostile_damage_events, 2);
    }

    #[test]
    fn attack_contact_is_folded_only_when_the_tracker_resolves() {
        let mut telemetry = WeaponTelemetry::default();
        telemetry.record_attack_completion(PRESET, FINGERPRINT, true);
        telemetry.record_attack_completion(PRESET, FINGERPRINT, false);
        assert_eq!(
            telemetry.attacks_with_hostile_contact.get(&PRESET),
            Some(&1)
        );
    }

    #[test]
    fn bounded_records_drop_history_without_losing_aggregates() {
        let mut telemetry = WeaponTelemetry::default();
        for _ in 0..513 {
            telemetry.record(WeaponTelemetryRecord {
                tick: 1,
                event_id: CombatEventId(1),
                attack_id: AttackId(1),
                preset_id: PRESET,
                recipe_fingerprint: FINGERPRINT,
                delivery_index: None,
                source: NetworkEntityId(1),
                target: None,
                position: WorldPoint { x: 0.0, y: 0.0 },
                requested_value: 0,
                applied_value: 0,
                engagement_distance: 0.0,
                delivery_travel: 0.0,
                hostile_contact: false,
                effect: None,
                resulting_health: None,
                resulting_effects: None,
                resulting_motion: None,
                outcome: WeaponTelemetryOutcome::AttackAccepted,
            });
        }
        assert_eq!(telemetry.bounded_records.len(), 512);
        assert_eq!(telemetry.dropped_records, 1);
    }
}
