use bevy::prelude::Resource;
use std::collections::{BTreeMap, VecDeque};

#[cfg(feature = "server")]
pub const MAX_ABILITY_TELEMETRY_RECORDS: usize = 1_024;

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum AbilityRejectionReason {
    NotCharged,
    AlreadyExecuting,
    Defeated,
    Inactive,
    StaleInput,
    ExistingSentry,
    PlacementBlocked,
    ZeroLengthDash,
    IdentifierExhausted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum DashInterruptionReason {
    Defeated,
    MatchInactive,
    OutOfBounds,
}

#[derive(
    serde::Serialize, serde::Deserialize, Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord,
)]
pub enum SentryCleanupReason {
    Expired,
    Destroyed,
    OwnerDefeated,
    OwnerDisconnected,
    MatchCompleted,
    MatchRestarted,
    BuildReplaced,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SentryTelemetryAggregate {
    pub owner_network_id: crate::protocol::NetworkEntityId,
    pub spawned_at_tick: u64,
    pub lifetime_ticks: u64,
    pub shots: u64,
    pub hits: u64,
    pub damage: u64,
    pub destructions: u64,
    pub cleanup_reason: Option<SentryCleanupReason>,
}

impl Default for SentryTelemetryAggregate {
    fn default() -> Self {
        Self {
            owner_network_id: crate::protocol::NetworkEntityId(0),
            spawned_at_tick: 0,
            lifetime_ticks: 0,
            shots: 0,
            hits: 0,
            damage: 0,
            destructions: 0,
            cleanup_reason: None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum AbilityTelemetryKind {
    ChargeDealt(u16),
    ChargeReceived(u16),
    FullCharge,
    ChargeWasted(u32),
    ActivationAttempt,
    ActivationRejected(AbilityRejectionReason),
    DashAccepted,
    DashTravel {
        requested_distance_milli: u32,
        actual_distance_milli: u32,
        terrain_truncated: bool,
    },
    DashContact,
    DashInterrupted(DashInterruptionReason),
    SentryAccepted,
    SentrySpawned(crate::builds::DeployableId),
    SentryShot(crate::builds::DeployableId),
    SentryHit {
        deployable_id: crate::builds::DeployableId,
        damage: u16,
    },
    SentryDestroyed(crate::builds::DeployableId),
    SentryCleanup {
        deployable_id: crate::builds::DeployableId,
        reason: SentryCleanupReason,
        lifetime_ticks: u64,
    },
    AbilityDamage(u16),
    AbilityTarget,
    AbilityDefeat,
    PassiveTriggered(crate::builds::PassiveDefinitionId),
    PassiveModified {
        passive_id: crate::builds::PassiveDefinitionId,
        amount: u16,
    },
    PassiveUnused(crate::builds::PassiveDefinitionId),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct AbilityTelemetryRecord {
    pub tick: u64,
    pub owner_network_id: crate::protocol::NetworkEntityId,
    pub kind: AbilityTelemetryKind,
}

#[derive(Resource, Clone, Debug, Default, PartialEq, Eq)]
pub struct AbilityTelemetry {
    pub records: VecDeque<AbilityTelemetryRecord>,
    pub dropped_records: u64,
    pub attempts: u64,
    pub accepts: u64,
    pub dash_uses: u64,
    pub sentry_uses: u64,
    pub sentry_shots: u64,
    pub wasted_charge: u64,
    pub ready_to_use_delay_ticks: u64,
    pub ready_to_use_count: u64,
    pub rejections_by_reason: BTreeMap<AbilityRejectionReason, u64>,
    pub dash_requested_distance_milli_by_owner: BTreeMap<crate::protocol::NetworkEntityId, u64>,
    pub dash_actual_distance_milli_by_owner: BTreeMap<crate::protocol::NetworkEntityId, u64>,
    pub dash_terrain_truncations_by_owner: BTreeMap<crate::protocol::NetworkEntityId, u64>,
    pub dash_contacts_by_owner: BTreeMap<crate::protocol::NetworkEntityId, u64>,
    pub dash_interruptions_by_owner: BTreeMap<crate::protocol::NetworkEntityId, u64>,
    pub ability_damage_by_owner: BTreeMap<crate::protocol::NetworkEntityId, u64>,
    pub ability_targets_by_owner: BTreeMap<crate::protocol::NetworkEntityId, u64>,
    pub ability_defeats_by_owner: BTreeMap<crate::protocol::NetworkEntityId, u64>,
    pub sentries: BTreeMap<crate::builds::DeployableId, SentryTelemetryAggregate>,
    pub sentry_cleanup_reasons: BTreeMap<SentryCleanupReason, u64>,
    pub concurrent_sentries: u64,
    pub concurrent_sentry_high_water: u64,
    pub first_full_charge_tick_by_owner: BTreeMap<crate::protocol::NetworkEntityId, u64>,
    pub full_charge_ticks_by_owner: BTreeMap<crate::protocol::NetworkEntityId, VecDeque<u64>>,
    pub uses_by_owner: BTreeMap<crate::protocol::NetworkEntityId, u64>,
    pub charge_damage_dealt_by_owner: BTreeMap<crate::protocol::NetworkEntityId, u64>,
    pub charge_damage_received_by_owner: BTreeMap<crate::protocol::NetworkEntityId, u64>,
    pub passive_triggers: BTreeMap<crate::builds::PassiveDefinitionId, u64>,
    pub passive_active_ticks: BTreeMap<crate::builds::PassiveDefinitionId, u64>,
    pub passive_modified_amounts: BTreeMap<crate::builds::PassiveDefinitionId, u64>,
    pub passive_unused_triggers: BTreeMap<crate::builds::PassiveDefinitionId, u64>,
    ready_since_by_owner: BTreeMap<crate::protocol::NetworkEntityId, u64>,
}

impl AbilityTelemetry {
    #[cfg(feature = "server")]
    #[allow(clippy::too_many_lines)]
    pub(crate) fn record(&mut self, record: AbilityTelemetryRecord) {
        match record.kind {
            AbilityTelemetryKind::ChargeDealt(amount) => {
                let total = self
                    .charge_damage_dealt_by_owner
                    .entry(record.owner_network_id)
                    .or_default();
                *total = total.saturating_add(u64::from(amount));
            }
            AbilityTelemetryKind::ChargeReceived(amount) => {
                let total = self
                    .charge_damage_received_by_owner
                    .entry(record.owner_network_id)
                    .or_default();
                *total = total.saturating_add(u64::from(amount));
            }
            AbilityTelemetryKind::FullCharge => {
                self.first_full_charge_tick_by_owner
                    .entry(record.owner_network_id)
                    .or_insert(record.tick);
                let ticks = self
                    .full_charge_ticks_by_owner
                    .entry(record.owner_network_id)
                    .or_default();
                if ticks.len() == 32 {
                    ticks.pop_front();
                }
                ticks.push_back(record.tick);
                self.ready_since_by_owner
                    .entry(record.owner_network_id)
                    .or_insert(record.tick);
            }
            AbilityTelemetryKind::ChargeWasted(amount) => {
                self.wasted_charge = self.wasted_charge.saturating_add(u64::from(amount));
            }
            AbilityTelemetryKind::ActivationAttempt => {
                self.attempts = self.attempts.saturating_add(1);
            }
            AbilityTelemetryKind::ActivationRejected(reason) => {
                let count = self.rejections_by_reason.entry(reason).or_default();
                *count = count.saturating_add(1);
            }
            AbilityTelemetryKind::DashAccepted => {
                self.accepts = self.accepts.saturating_add(1);
                self.dash_uses = self.dash_uses.saturating_add(1);
                let uses = self
                    .uses_by_owner
                    .entry(record.owner_network_id)
                    .or_default();
                *uses = uses.saturating_add(1);
                self.record_ready_to_use_delay(record.owner_network_id, record.tick);
            }
            AbilityTelemetryKind::DashTravel {
                requested_distance_milli,
                actual_distance_milli,
                terrain_truncated,
            } => {
                add_owner_total(
                    &mut self.dash_requested_distance_milli_by_owner,
                    record.owner_network_id,
                    u64::from(requested_distance_milli),
                );
                add_owner_total(
                    &mut self.dash_actual_distance_milli_by_owner,
                    record.owner_network_id,
                    u64::from(actual_distance_milli),
                );
                if terrain_truncated {
                    add_owner_total(
                        &mut self.dash_terrain_truncations_by_owner,
                        record.owner_network_id,
                        1,
                    );
                }
            }
            AbilityTelemetryKind::DashContact => {
                add_owner_total(&mut self.dash_contacts_by_owner, record.owner_network_id, 1);
            }
            AbilityTelemetryKind::DashInterrupted(_) => add_owner_total(
                &mut self.dash_interruptions_by_owner,
                record.owner_network_id,
                1,
            ),
            AbilityTelemetryKind::SentryAccepted => {
                self.accepts = self.accepts.saturating_add(1);
                self.sentry_uses = self.sentry_uses.saturating_add(1);
                let uses = self
                    .uses_by_owner
                    .entry(record.owner_network_id)
                    .or_default();
                *uses = uses.saturating_add(1);
                self.record_ready_to_use_delay(record.owner_network_id, record.tick);
            }
            AbilityTelemetryKind::SentrySpawned(deployable_id) => {
                self.sentries.insert(
                    deployable_id,
                    SentryTelemetryAggregate {
                        owner_network_id: record.owner_network_id,
                        spawned_at_tick: record.tick,
                        ..Default::default()
                    },
                );
                self.concurrent_sentries = self.concurrent_sentries.saturating_add(1);
                self.concurrent_sentry_high_water = self
                    .concurrent_sentry_high_water
                    .max(self.concurrent_sentries);
            }
            AbilityTelemetryKind::SentryShot(deployable_id) => {
                self.sentry_shots = self.sentry_shots.saturating_add(1);
                if let Some(sentry) = self.sentries.get_mut(&deployable_id) {
                    sentry.shots = sentry.shots.saturating_add(1);
                }
            }
            AbilityTelemetryKind::SentryHit {
                deployable_id,
                damage,
            } => {
                if let Some(sentry) = self.sentries.get_mut(&deployable_id) {
                    sentry.hits = sentry.hits.saturating_add(1);
                    sentry.damage = sentry.damage.saturating_add(u64::from(damage));
                }
            }
            AbilityTelemetryKind::SentryDestroyed(deployable_id) => {
                if let Some(sentry) = self.sentries.get_mut(&deployable_id) {
                    sentry.destructions = sentry.destructions.saturating_add(1);
                }
            }
            AbilityTelemetryKind::SentryCleanup {
                deployable_id,
                reason,
                lifetime_ticks,
            } => {
                if let Some(sentry) = self.sentries.get_mut(&deployable_id) {
                    sentry.lifetime_ticks = lifetime_ticks;
                    sentry.cleanup_reason = Some(reason);
                }
                let count = self.sentry_cleanup_reasons.entry(reason).or_default();
                *count = count.saturating_add(1);
                self.concurrent_sentries = self.concurrent_sentries.saturating_sub(1);
            }
            AbilityTelemetryKind::AbilityDamage(amount) => add_owner_total(
                &mut self.ability_damage_by_owner,
                record.owner_network_id,
                u64::from(amount),
            ),
            AbilityTelemetryKind::AbilityTarget => add_owner_total(
                &mut self.ability_targets_by_owner,
                record.owner_network_id,
                1,
            ),
            AbilityTelemetryKind::AbilityDefeat => add_owner_total(
                &mut self.ability_defeats_by_owner,
                record.owner_network_id,
                1,
            ),
            AbilityTelemetryKind::PassiveModified { passive_id, amount } => {
                let total = self.passive_modified_amounts.entry(passive_id).or_default();
                *total = total.saturating_add(u64::from(amount));
            }
            AbilityTelemetryKind::PassiveTriggered(passive_id) => {
                let triggers = self.passive_triggers.entry(passive_id).or_default();
                *triggers = triggers.saturating_add(1);
            }
            AbilityTelemetryKind::PassiveUnused(passive_id) => {
                let total = self.passive_unused_triggers.entry(passive_id).or_default();
                *total = total.saturating_add(1);
            }
        }
        if self.records.len() == MAX_ABILITY_TELEMETRY_RECORDS {
            self.records.pop_front();
            self.dropped_records = self.dropped_records.saturating_add(1);
        }
        self.records.push_back(record);
    }

    #[cfg(feature = "server")]
    pub(crate) fn record_passive_active_tick(
        &mut self,
        passive_id: crate::builds::PassiveDefinitionId,
    ) {
        let ticks = self.passive_active_ticks.entry(passive_id).or_default();
        *ticks = ticks.saturating_add(1);
    }

    #[cfg(feature = "server")]
    fn record_ready_to_use_delay(
        &mut self,
        owner: crate::protocol::NetworkEntityId,
        used_at_tick: u64,
    ) {
        if let Some(ready_at_tick) = self.ready_since_by_owner.remove(&owner) {
            self.ready_to_use_delay_ticks = self
                .ready_to_use_delay_ticks
                .saturating_add(used_at_tick.saturating_sub(ready_at_tick));
            self.ready_to_use_count = self.ready_to_use_count.saturating_add(1);
        }
    }

    #[must_use]
    pub(crate) fn delta_since(&self, start: &Self, active_started_at_tick: u64) -> Self {
        let mut delta = Self {
            dropped_records: self.dropped_records.saturating_sub(start.dropped_records),
            attempts: self.attempts.saturating_sub(start.attempts),
            accepts: self.accepts.saturating_sub(start.accepts),
            dash_uses: self.dash_uses.saturating_sub(start.dash_uses),
            sentry_uses: self.sentry_uses.saturating_sub(start.sentry_uses),
            sentry_shots: self.sentry_shots.saturating_sub(start.sentry_shots),
            wasted_charge: self.wasted_charge.saturating_sub(start.wasted_charge),
            ready_to_use_delay_ticks: self
                .ready_to_use_delay_ticks
                .saturating_sub(start.ready_to_use_delay_ticks),
            ready_to_use_count: self
                .ready_to_use_count
                .saturating_sub(start.ready_to_use_count),
            concurrent_sentries: self.concurrent_sentries,
            ..Default::default()
        };
        delta.records = self
            .records
            .iter()
            .filter(|record| record.tick >= active_started_at_tick)
            .copied()
            .collect();
        delta.rejections_by_reason =
            count_map_delta(&self.rejections_by_reason, &start.rejections_by_reason);
        delta.dash_requested_distance_milli_by_owner = count_map_delta(
            &self.dash_requested_distance_milli_by_owner,
            &start.dash_requested_distance_milli_by_owner,
        );
        delta.dash_actual_distance_milli_by_owner = count_map_delta(
            &self.dash_actual_distance_milli_by_owner,
            &start.dash_actual_distance_milli_by_owner,
        );
        delta.dash_terrain_truncations_by_owner = count_map_delta(
            &self.dash_terrain_truncations_by_owner,
            &start.dash_terrain_truncations_by_owner,
        );
        delta.dash_contacts_by_owner =
            count_map_delta(&self.dash_contacts_by_owner, &start.dash_contacts_by_owner);
        delta.dash_interruptions_by_owner = count_map_delta(
            &self.dash_interruptions_by_owner,
            &start.dash_interruptions_by_owner,
        );
        delta.ability_damage_by_owner = count_map_delta(
            &self.ability_damage_by_owner,
            &start.ability_damage_by_owner,
        );
        delta.ability_targets_by_owner = count_map_delta(
            &self.ability_targets_by_owner,
            &start.ability_targets_by_owner,
        );
        delta.ability_defeats_by_owner = count_map_delta(
            &self.ability_defeats_by_owner,
            &start.ability_defeats_by_owner,
        );
        delta.uses_by_owner = count_map_delta(&self.uses_by_owner, &start.uses_by_owner);
        delta.charge_damage_dealt_by_owner = count_map_delta(
            &self.charge_damage_dealt_by_owner,
            &start.charge_damage_dealt_by_owner,
        );
        delta.charge_damage_received_by_owner = count_map_delta(
            &self.charge_damage_received_by_owner,
            &start.charge_damage_received_by_owner,
        );
        delta.passive_triggers = count_map_delta(&self.passive_triggers, &start.passive_triggers);
        delta.passive_active_ticks =
            count_map_delta(&self.passive_active_ticks, &start.passive_active_ticks);
        delta.passive_modified_amounts = count_map_delta(
            &self.passive_modified_amounts,
            &start.passive_modified_amounts,
        );
        delta.passive_unused_triggers = count_map_delta(
            &self.passive_unused_triggers,
            &start.passive_unused_triggers,
        );
        delta.sentry_cleanup_reasons =
            count_map_delta(&self.sentry_cleanup_reasons, &start.sentry_cleanup_reasons);
        delta.sentries = self
            .sentries
            .iter()
            .filter(|(_, aggregate)| aggregate.spawned_at_tick >= active_started_at_tick)
            .map(|(id, aggregate)| (*id, aggregate.clone()))
            .collect();
        delta.concurrent_sentry_high_water = sentry_high_water(&delta.sentries);
        for (owner, ticks) in &self.full_charge_ticks_by_owner {
            let matching: VecDeque<_> = ticks
                .iter()
                .copied()
                .filter(|tick| *tick >= active_started_at_tick)
                .collect();
            if let Some(first) = matching.front() {
                delta.first_full_charge_tick_by_owner.insert(*owner, *first);
                delta.full_charge_ticks_by_owner.insert(*owner, matching);
            }
        }
        delta
    }
}

#[cfg(feature = "server")]
fn add_owner_total(
    totals: &mut BTreeMap<crate::protocol::NetworkEntityId, u64>,
    owner: crate::protocol::NetworkEntityId,
    amount: u64,
) {
    let total = totals.entry(owner).or_default();
    *total = total.saturating_add(amount);
}

fn count_map_delta<K: Copy + Ord>(
    end: &BTreeMap<K, u64>,
    start: &BTreeMap<K, u64>,
) -> BTreeMap<K, u64> {
    end.iter()
        .filter_map(|(key, value)| {
            let delta = value.saturating_sub(start.get(key).copied().unwrap_or(0));
            (delta != 0).then_some((*key, delta))
        })
        .collect()
}

fn sentry_high_water(
    sentries: &BTreeMap<crate::builds::DeployableId, SentryTelemetryAggregate>,
) -> u64 {
    let mut edges = Vec::with_capacity(sentries.len().saturating_mul(2));
    for sentry in sentries.values() {
        edges.push((sentry.spawned_at_tick, 1_i8));
        edges.push((
            sentry.spawned_at_tick.saturating_add(sentry.lifetime_ticks),
            -1_i8,
        ));
    }
    // A spawn and cleanup at the same tick overlap for that authoritative tick. Process positive
    // edges first so the high-water mark does not undercount that bounded lifetime.
    edges.sort_by_key(|(tick, delta)| (*tick, std::cmp::Reverse(*delta)));
    let mut live = 0_u64;
    let mut high = 0_u64;
    for (_, edge) in edges {
        if edge > 0 {
            live = live.saturating_add(1);
            high = high.max(live);
        } else {
            live = live.saturating_sub(1);
        }
    }
    high
}

#[cfg(feature = "server")]
#[derive(Resource, Debug, Default)]
pub(crate) struct AbilityOutcomeObservationState {
    last_event_id: u64,
}

#[cfg(feature = "server")]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn observe_ability_outcomes(
    facts: bevy::prelude::Res<crate::combat::CombatOutcomeFacts>,
    mut observed: bevy::prelude::ResMut<AbilityOutcomeObservationState>,
    mut telemetry: bevy::prelude::ResMut<AbilityTelemetry>,
) {
    let previous = observed.last_event_id;
    for fact in facts.0.iter().filter(|fact| fact.event_id.0 > previous) {
        observed.last_event_id = observed.last_event_id.max(fact.event_id.0);
        let Some(owner) = fact.source_network_id else {
            continue;
        };
        let deployable_id = match fact.source_kind {
            crate::combat::CombatSourceKind::PrimaryWeapon => None,
            crate::combat::CombatSourceKind::Ultimate { .. } => Some(None),
            crate::combat::CombatSourceKind::Deployable { deployable_id, .. } => {
                Some(Some(deployable_id))
            }
        };
        let Some(deployable_id) = deployable_id else {
            continue;
        };
        match fact.kind {
            crate::combat::CombatOutcomeKind::Damage { amount } => {
                telemetry.record(AbilityTelemetryRecord {
                    tick: fact.tick,
                    owner_network_id: owner,
                    kind: AbilityTelemetryKind::AbilityDamage(amount),
                });
                telemetry.record(AbilityTelemetryRecord {
                    tick: fact.tick,
                    owner_network_id: owner,
                    kind: AbilityTelemetryKind::AbilityTarget,
                });
                if let Some(deployable_id) = deployable_id {
                    telemetry.record(AbilityTelemetryRecord {
                        tick: fact.tick,
                        owner_network_id: owner,
                        kind: AbilityTelemetryKind::SentryHit {
                            deployable_id,
                            damage: amount,
                        },
                    });
                }
            }
            crate::combat::CombatOutcomeKind::Defeat => {
                telemetry.record(AbilityTelemetryRecord {
                    tick: fact.tick,
                    owner_network_id: owner,
                    kind: AbilityTelemetryKind::AbilityDefeat,
                });
            }
            crate::combat::CombatOutcomeKind::DeployableDestroyed => {
                let target_id =
                    crate::builds::DeployableId(fact.target_network_id.0 & !(1_u64 << 63));
                telemetry.record(AbilityTelemetryRecord {
                    tick: fact.tick,
                    owner_network_id: owner,
                    kind: AbilityTelemetryKind::SentryDestroyed(target_id),
                });
            }
            crate::combat::CombatOutcomeKind::ProtectedContact => {}
        }
    }
}
