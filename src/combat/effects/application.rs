//! Composed-payload stage functions: batch collection and ordering, event planning and
//! reservation, per-record application, and deferred commit. The system in `super` only
//! sequences these stages; the rules live here and in the sibling `planning`/`runtime`
//! modules.

#![allow(clippy::wildcard_imports)]
use super::planning::{
    delivery_survives_owner_disconnect, pending_delivery_kind_order, required_payload_event_count,
};
use super::runtime::*;
use super::*;

mod transaction;
pub(super) use transaction::{commit_composed_plan, plan_composed_records, project_composed_plan};

/// One tick's collected, deterministically ordered composed-payload work.
pub(super) struct ComposedBatch {
    pub(super) disconnected: HashSet<Entity>,
    pub(super) connected_owners: HashSet<u64>,
    pub(super) close_quarters_owners: HashMap<u64, crate::builds::ResolvedPassive>,
    pub(super) records: Vec<PendingPayload>,
    pub(super) deliveries: Vec<PendingDelivery>,
}

/// Borrowed view of a batch after its deliveries were consumed by delivery resolution.
pub(super) struct BatchView<'a> {
    pub(super) disconnected: &'a HashSet<Entity>,
    pub(super) connected_owners: &'a HashSet<u64>,
    pub(super) close_quarters_owners: &'a HashMap<u64, crate::builds::ResolvedPassive>,
    pub(super) records: &'a [PendingPayload],
    pub(super) retained_delivery_keys: &'a HashSet<(AttackId, u8)>,
}

/// Collect this tick's pending payloads and deliveries in the deterministic order the
/// rest of the pipeline relies on: target, contact fraction, attack, delivery, bundle for
/// payloads; attack, delivery index, tick, kind for deliveries.
pub(super) fn collect_composed_batch<'a>(
    combat: &mut CombatTargetState,
    payloads: impl Iterator<Item = &'a PendingPayload>,
    deliveries: impl Iterator<Item = &'a PendingDelivery>,
) -> ComposedBatch {
    let disconnected: HashSet<_> = combat.disconnected.iter().collect();
    let connected_owners: HashSet<_> = combat
        .owners
        .iter()
        .filter(|(_, controlled)| {
            controlled.is_none_or(|controlled| !disconnected.contains(&controlled.owner))
        })
        .map(|(network_id, _)| network_id.0)
        .collect();
    let close_quarters_owners: HashMap<_, _> = combat
        .passive_access
        .p1()
        .iter()
        .filter_map(|(network_id, passives)| {
            passives
                .find(crate::builds::PassiveKind::CloseQuarters)
                .map(|passive| (network_id.0, passive))
        })
        .collect();
    let mut records: Vec<_> = payloads.cloned().collect();
    records.sort_by(|left, right| {
        left.target_network_id
            .0
            .cmp(&right.target_network_id.0)
            .then_with(|| left.contact_fraction.total_cmp(&right.contact_fraction))
            .then_with(|| left.source.attack_id.0.cmp(&right.source.attack_id.0))
            .then_with(|| left.delivery_index.cmp(&right.delivery_index))
            .then_with(|| left.bundle_index.cmp(&right.bundle_index))
    });
    let mut deliveries: Vec<_> = deliveries.cloned().collect();
    deliveries.sort_by(|left, right| {
        left.source
            .attack_id
            .0
            .cmp(&right.source.attack_id.0)
            .then_with(|| left.delivery_index.cmp(&right.delivery_index))
            .then_with(|| left.tick.cmp(&right.tick))
            .then_with(|| {
                pending_delivery_kind_order(&left.kind)
                    .cmp(&pending_delivery_kind_order(&right.kind))
            })
    });
    ComposedBatch {
        disconnected,
        connected_owners,
        close_quarters_owners,
        records,
        deliveries,
    }
}

impl ComposedBatch {
    pub(super) fn retained_delivery_keys(&self) -> HashSet<(AttackId, u8)> {
        self.deliveries
            .iter()
            .filter(|delivery| delivery_survives_owner_disconnect(&delivery.kind))
            .map(|delivery| (delivery.source.attack_id, delivery.delivery_index))
            .collect()
    }
}

/// Dry-run the complete sorted batch against a target snapshot, then reserve every event
/// ID the batch can consume. Returns `None` on event-ID exhaustion after recording the
/// reservation drop; the caller must abort the whole batch.
pub(super) fn plan_composed_events(
    combat: &mut CombatTargetState,
    ids: &mut NextCombatIds,
    batch: &ComposedBatch,
    telemetry: &mut WeaponTelemetry,
) -> Option<Vec<CombatEventId>> {
    let mut planned_targets: HashMap<Entity, PlannedTarget> = combat
        .targets
        .p1()
        .iter()
        .filter(|(_, _, _, _, _, _, _, controlled, _, _)| {
            controlled.is_none_or(|controlled| !batch.disconnected.contains(&controlled.owner))
        })
        .map(
            |(entity, network_id, team, health, _, _, defeated, _, _, effect_tile)| {
                (
                    entity,
                    (
                        *network_id,
                        *team,
                        health.0,
                        defeated.is_some(),
                        if combat.sentry_targets.contains(entity) {
                            CombatTargetKind::Deployable
                        } else {
                            CombatTargetKind::Fighter
                        },
                        effect_tile.is_some_and(crate::map::EffectTileOccupancy::blocks_healing),
                    ),
                )
            },
        )
        .collect();
    let Some(required_event_count) = required_payload_event_count(
        &batch.deliveries,
        &batch.records,
        &batch.connected_owners,
        &batch.close_quarters_owners,
        &mut planned_targets,
    ) else {
        telemetry.event_reservation_drops = telemetry.event_reservation_drops.saturating_add(1);
        return None;
    };
    let Some(reserved_events) = server::reserve_event_ids(ids, required_event_count) else {
        telemetry.event_reservation_drops = telemetry.event_reservation_drops.saturating_add(1);
        return None;
    };
    Some(reserved_events)
}

/// What the application loop should do with one payload record.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum TargetGate {
    /// The record cannot act: kind mismatch or the target is out of active combat.
    Skip,
    /// The target is spawn-protected; emit a protected-contact outcome instead of applying.
    ProtectedContact,
    /// Apply the record's effects against the target.
    Apply,
}

/// Gate one payload record against target-kind and match-state rules. Target resolution
/// and connection filtering happen before this gate in the application loop.
#[allow(
    clippy::fn_params_excessive_bools,
    reason = "the gate is an explicit truth table over the target's independent match-state facts"
)]
pub(super) const fn payload_target_gate(
    combat_source_allows: bool,
    match_participant: bool,
    active_combatant: bool,
    spawn_protected: bool,
    hostile: bool,
) -> TargetGate {
    if !combat_source_allows {
        return TargetGate::Skip;
    }
    if match_participant && !active_combatant {
        return TargetGate::Skip;
    }
    if spawn_protected && hostile {
        return TargetGate::ProtectedContact;
    }
    TargetGate::Apply
}

/// Immutable damage calculation consumed by the authoritative batch transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct DamageApplicationPlan {
    pub(super) requested: u16,
    pub(super) unmodified_applied: u16,
    pub(super) applied: u16,
    pub(super) health_after: u16,
    pub(super) defeats: bool,
}

#[allow(
    clippy::too_many_arguments,
    reason = "the plan exposes each authored and current-state input to the pure damage calculation"
)]
pub(super) fn plan_damage_application(
    current_health: u16,
    target_defeated: bool,
    amount: u16,
    falloff: DamageFalloff,
    delivery_travel: f32,
    recipient_scale: f32,
    close_quarters: Option<crate::builds::PassiveParameters>,
    engagement_distance: f32,
) -> Option<DamageApplicationPlan> {
    let unmodified_requested = requested_damage(
        amount,
        falloff,
        delivery_travel,
        recipient_scale,
        None,
        engagement_distance,
    );
    let requested = requested_damage(
        amount,
        falloff,
        delivery_travel,
        recipient_scale,
        close_quarters,
        engagement_distance,
    );
    let applied = requested.min(current_health);
    if applied == 0 {
        return None;
    }
    Some(DamageApplicationPlan {
        requested,
        unmodified_applied: unmodified_requested.min(current_health),
        applied,
        health_after: current_health.saturating_sub(applied),
        defeats: !target_defeated && current_health == applied,
    })
}

/// Immutable healing calculation consumed by the authoritative batch transaction.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct HealingApplicationPlan {
    pub(super) requested: u16,
    pub(super) applied: u16,
    pub(super) health_after: u16,
}

pub(super) fn plan_healing_application(
    current_health: u16,
    maximum_health: u16,
    amount: u16,
    recipient_scale: f32,
) -> HealingApplicationPlan {
    let requested = (f32::from(amount) * recipient_scale)
        .round()
        .clamp(1.0, f32::from(u16::MAX)) as u16;
    let applied = requested.min(maximum_health.saturating_sub(current_health));
    HealingApplicationPlan {
        requested,
        applied,
        health_after: current_health.saturating_add(applied).min(maximum_health),
    }
}

// One applied damage fact fans out into weapon telemetry, legacy telemetry, cues, and
// outcome facts; every parameter is a fact of that single application.
#[allow(
    clippy::large_types_passed_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "each parameter is one fact of the damage application being recorded across the telemetry, cue, and outcome sinks"
)]
fn project_committed_damage(
    record: &PendingPayload,
    tick: u64,
    source: DamageSource,
    target_network_id: NetworkEntityId,
    target_team: TeamId,
    target_kind: CombatTargetKind,
    preset_id: WeaponPresetId,
    applied_effect: (u16, DamageFalloff, RecipientPolicy),
    requested: u16,
    applied_damage: u16,
    health_after: u16,
    damage_event: CombatEventId,
    legacy_damage_event: Option<CombatEventId>,
    effects_state: ActiveEffects,
    motion_state: Option<ExternalMotion>,
    gameplay_telemetry: &mut AbilityWeaponTelemetry,
    transaction: &mut CombatTransactionState,
) {
    let owner_damage = target_network_id == record.source.owner_network_entity_id;
    let band = distance_band(record.engagement_distance);
    gameplay_telemetry.weapon.record_damage(
        preset_id,
        record.source.recipe_fingerprint,
        owner_damage,
        band,
        applied_damage,
    );
    transaction.legacy_telemetry.applied_damage = transaction
        .legacy_telemetry
        .applied_damage
        .saturating_add(u64::from(applied_damage));
    if owner_damage {
        transaction.legacy_telemetry.close_hits =
            transaction.legacy_telemetry.close_hits.saturating_add(1);
    } else {
        transaction.legacy_telemetry.hostile_fighter_hits = transaction
            .legacy_telemetry
            .hostile_fighter_hits
            .saturating_add(1);
    }
    let damage_cue = CombatCue::DamageApplied {
        event_id: damage_event,
        tick,
        attack_id: record.source.attack_id,
        source,
        target: target_network_id,
        position: WorldPoint::from(record.position),
        amount: applied_damage,
        health_after,
        distance_band: band,
    };
    transaction.legacy_telemetry.record_cue(damage_cue.clone());
    transaction.outbox.0.push(damage_cue);
    if let Some(legacy_damage_event) = legacy_damage_event {
        let legacy_cue = CombatCue::Damage {
            event_id: legacy_damage_event,
            tick,
            source,
            target: target_network_id,
            amount: applied_damage,
            health_after,
            distance_band: band,
        };
        transaction.legacy_telemetry.record_cue(legacy_cue.clone());
        transaction
            .legacy_telemetry
            .record(CombatLogRecord::Damage {
                tick,
                event_id: legacy_damage_event,
                source,
                target: target_network_id,
                requested,
                applied: applied_damage,
                health_after,
            });
        transaction.outbox.0.push(legacy_cue);
    }
    gameplay_telemetry.weapon.record(WeaponTelemetryRecord {
        tick,
        event_id: damage_event,
        attack_id: record.source.attack_id,
        preset_id,
        recipe_fingerprint: record.source.recipe_fingerprint,
        delivery_index: Some(record.delivery_index),
        source: record.source.owner_network_entity_id,
        target: Some(target_network_id),
        position: WorldPoint::from(record.position),
        requested_value: requested,
        applied_value: applied_damage,
        engagement_distance: record.engagement_distance,
        delivery_travel: record.delivery_travel,
        hostile_contact: !owner_damage,
        effect: Some(PayloadEffectDefinition::Damage {
            amount: applied_effect.0,
            falloff: applied_effect.1,
            recipients: applied_effect.2,
        }),
        resulting_health: Some(health_after),
        resulting_effects: Some(effects_state),
        resulting_motion: motion_state,
        outcome: WeaponTelemetryOutcome::DamageApplied,
    });
    transaction.outcome_facts.0.push(CombatOutcomeFact {
        event_id: damage_event,
        tick,
        attack_id: record.source.attack_id,
        source_kind: record.source.kind,
        source_player: Some(record.source.player_id),
        source_network_id: Some(record.source.owner_network_entity_id),
        source_team: Some(record.source.team_id),
        target_network_id,
        target_kind,
        target_team,
        preset_id: record.source.source_preset_id,
        recipe_fingerprint: Some(record.source.recipe_fingerprint),
        position: WorldPoint::from(record.position),
        engagement_distance: record.engagement_distance,
        kind: CombatOutcomeKind::Damage {
            amount: applied_damage,
        },
    });
}

#[allow(
    clippy::large_types_passed_by_value,
    clippy::too_many_arguments,
    reason = "each parameter is a committed healing fact projected to the existing sinks"
)]
fn project_committed_healing(
    record: &PendingPayload,
    tick: u64,
    source: DamageSource,
    target_network_id: NetworkEntityId,
    target_team: TeamId,
    target_kind: CombatTargetKind,
    preset_id: WeaponPresetId,
    event_id: CombatEventId,
    effect: PayloadEffectDefinition,
    plan: HealingApplicationPlan,
    effects_state: ActiveEffects,
    motion_state: Option<ExternalMotion>,
    gameplay_telemetry: &mut AbilityWeaponTelemetry,
    transaction: &mut CombatTransactionState,
) {
    let cue = CombatCue::EffectApplied {
        event_id,
        tick,
        attack_id: record.source.attack_id,
        source,
        target: target_network_id,
        position: WorldPoint::from(record.position),
        effect: CombatEffectCue::Healing {
            amount: plan.applied,
            health_after: plan.health_after,
        },
    };
    transaction.legacy_telemetry.record_cue(cue.clone());
    transaction.outbox.0.push(cue);
    transaction.outcome_facts.0.push(CombatOutcomeFact {
        event_id,
        tick,
        attack_id: record.source.attack_id,
        source_kind: record.source.kind,
        source_player: Some(record.source.player_id),
        source_network_id: Some(record.source.owner_network_entity_id),
        source_team: Some(record.source.team_id),
        target_network_id,
        target_kind,
        target_team,
        preset_id: record.source.source_preset_id,
        recipe_fingerprint: Some(record.source.recipe_fingerprint),
        position: WorldPoint::from(record.position),
        engagement_distance: record.engagement_distance,
        kind: CombatOutcomeKind::Healing {
            requested: plan.requested,
            applied: plan.applied,
            resulting_health: plan.health_after,
        },
    });
    gameplay_telemetry.weapon.record(WeaponTelemetryRecord {
        tick,
        event_id,
        attack_id: record.source.attack_id,
        preset_id,
        recipe_fingerprint: record.source.recipe_fingerprint,
        delivery_index: Some(record.delivery_index),
        source: record.source.owner_network_entity_id,
        target: Some(target_network_id),
        position: WorldPoint::from(record.position),
        requested_value: plan.requested,
        applied_value: plan.applied,
        engagement_distance: record.engagement_distance,
        delivery_travel: record.delivery_travel,
        hostile_contact: false,
        effect: Some(effect),
        resulting_health: Some(plan.health_after),
        resulting_effects: Some(effects_state),
        resulting_motion: motion_state,
        outcome: WeaponTelemetryOutcome::HealingApplied,
    });
}

// A committed defeat projects telemetry, cues, and outcome facts for one target.
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "each parameter is one committed defeat fact projected to the existing sinks"
)]
fn project_committed_defeat(
    record: &PendingPayload,
    tick: u64,
    source: DamageSource,
    target_network_id: NetworkEntityId,
    target_team: TeamId,
    target_kind: CombatTargetKind,
    preset_id: WeaponPresetId,
    owner_contact: bool,
    defeat_event: CombatEventId,
    legacy_defeat_event: Option<CombatEventId>,
    gameplay_telemetry: &mut AbilityWeaponTelemetry,
    transaction: &mut CombatTransactionState,
) {
    gameplay_telemetry
        .weapon
        .record_defeat(preset_id, record.source.recipe_fingerprint);
    gameplay_telemetry.weapon.record(WeaponTelemetryRecord {
        tick,
        event_id: defeat_event,
        attack_id: record.source.attack_id,
        preset_id,
        recipe_fingerprint: record.source.recipe_fingerprint,
        delivery_index: Some(record.delivery_index),
        source: record.source.owner_network_entity_id,
        target: Some(target_network_id),
        position: WorldPoint::from(record.position),
        requested_value: 0,
        applied_value: 0,
        engagement_distance: record.engagement_distance,
        delivery_travel: record.delivery_travel,
        hostile_contact: !owner_contact,
        effect: None,
        resulting_health: Some(0),
        resulting_effects: Some(ActiveEffects::default()),
        resulting_motion: None,
        outcome: WeaponTelemetryOutcome::Defeat,
    });
    transaction.legacy_telemetry.defeats = transaction.legacy_telemetry.defeats.saturating_add(1);
    let defeated_cue = CombatCue::FighterDefeated {
        event_id: defeat_event,
        tick,
        attack_id: record.source.attack_id,
        source: Some(source),
        target: target_network_id,
        position: WorldPoint::from(record.position),
    };
    transaction
        .legacy_telemetry
        .record_cue(defeated_cue.clone());
    transaction.outbox.0.push(defeated_cue);
    transaction.outcome_facts.0.push(CombatOutcomeFact {
        event_id: defeat_event,
        tick,
        attack_id: record.source.attack_id,
        source_kind: record.source.kind,
        source_player: Some(record.source.player_id),
        source_network_id: Some(record.source.owner_network_entity_id),
        source_team: Some(record.source.team_id),
        target_network_id,
        target_kind,
        target_team,
        preset_id: record.source.source_preset_id,
        recipe_fingerprint: Some(record.source.recipe_fingerprint),
        position: WorldPoint::from(record.position),
        engagement_distance: record.engagement_distance,
        kind: if target_kind == CombatTargetKind::Deployable {
            CombatOutcomeKind::DeployableDestroyed
        } else {
            CombatOutcomeKind::Defeat
        },
    });
    if let Some(legacy_defeat_event) = legacy_defeat_event {
        let legacy_cue = CombatCue::Defeat {
            event_id: legacy_defeat_event,
            tick,
            source: Some(source),
            target: target_network_id,
        };
        transaction.legacy_telemetry.record_cue(legacy_cue.clone());
        transaction
            .legacy_telemetry
            .record(CombatLogRecord::Defeat {
                tick,
                event_id: legacy_defeat_event,
                source: Some(source),
                target: target_network_id,
            });
        transaction.outbox.0.push(legacy_cue);
    }
}
