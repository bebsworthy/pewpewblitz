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

/// Mutable per-tick application state accumulated across records and committed once at
/// the end of the batch.
#[derive(Default)]
pub(super) struct AppliedComposedState {
    contacted_deliveries: HashSet<(AttackId, u8, u64)>,
    cold_contacts: HashSet<(AttackId, u64)>,
    defeated: HashSet<Entity>,
    effects: HashMap<Entity, ActiveEffects>,
    motion: HashMap<Entity, ExternalMotion>,
    deferred_cues: Vec<(Entity, CombatCue)>,
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
        .filter(|(_, _, _, _, _, controlled, _)| {
            controlled.is_none_or(|controlled| !batch.disconnected.contains(&controlled.owner))
        })
        .map(
            |(entity, network_id, team, health, defeated, _, effect_tile)| {
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

// The application loop coordinates target resolution, gating, damage, and runtime effects
// across the sorted batch; its parameter list is the fixed-tick state those stages share.
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the loop hands each stage the shared fixed-tick state; the alternative is a mutable context struct that hides the data flow between ordering, gating, application, and commit"
)]
pub(super) fn apply_composed_records(
    commands: &mut Commands,
    tick: u64,
    batch: &BatchView,
    combat: &mut CombatTargetState,
    reserved_events: &mut impl Iterator<Item = CombatEventId>,
    trackers: &mut ActiveAttackTrackers,
    gameplay_telemetry: &mut AbilityWeaponTelemetry,
    transaction: &mut CombatTransactionState,
    applied: &mut AppliedComposedState,
    resolved_delivery_keys: &mut HashSet<(AttackId, u8)>,
    condition_rules: CombatConditionRules,
) {
    for record in batch.records {
        // Hold the mutable target view for this record; the disjoint match/passive/sentry
        // accesses below borrow other ParamSet members, never `targets`.
        let mut targets = combat.targets.p0();
        resolved_delivery_keys.insert((record.source.attack_id, record.delivery_index));
        if !batch
            .connected_owners
            .contains(&record.source.owner_network_entity_id.0)
            && !batch
                .retained_delivery_keys
                .contains(&(record.source.attack_id, record.delivery_index))
        {
            trackers.active.remove(&record.source.attack_id);
            continue;
        }
        let Ok((
            target_network_id,
            target_team,
            mut health,
            active_effects,
            external_motion,
            defeated,
            controlled_by,
            test_dummy,
            effect_tile,
        )) = targets.get_mut(record.target)
        else {
            continue;
        };
        if controlled_by.is_some_and(|controlled| batch.disconnected.contains(&controlled.owner)) {
            continue;
        }
        let target_kind = if combat.sentry_targets.contains(record.target) {
            CombatTargetKind::Deployable
        } else {
            CombatTargetKind::Fighter
        };
        let gate = payload_target_gate(
            combat_source_allows_target(record.source.kind, target_kind),
            combat.match_access.p0().contains(record.target),
            combat.match_access.p1().contains(record.target),
            combat.match_access.p2().contains(record.target),
            teams_are_hostile(record.source.team_id, *target_team),
        );
        match gate {
            TargetGate::Skip => continue,
            TargetGate::ProtectedContact => {
                if let Some(event_id) = reserved_events.next() {
                    transaction.outcome_facts.0.push(CombatOutcomeFact {
                        event_id,
                        tick,
                        attack_id: record.source.attack_id,
                        source_kind: record.source.kind,
                        source_player: Some(record.source.player_id),
                        source_network_id: Some(record.source.owner_network_entity_id),
                        source_team: Some(record.source.team_id),
                        target_network_id: *target_network_id,
                        target_kind,
                        target_team: *target_team,
                        preset_id: record.source.source_preset_id,
                        recipe_fingerprint: Some(record.source.recipe_fingerprint),
                        position: WorldPoint::from(record.position),
                        engagement_distance: record.engagement_distance,
                        kind: CombatOutcomeKind::ProtectedContact,
                    });
                }
                continue;
            }
            TargetGate::Apply => {}
        }
        let mut effects_state = applied
            .effects
            .get(&record.target)
            .copied()
            .unwrap_or_else(|| {
                active_effects.map_or_else(ActiveEffects::default, |effects| *effects)
            });
        let mut motion_state = applied
            .motion
            .get(&record.target)
            .copied()
            .or(external_motion.copied());
        let preset_id = record.source.source_preset_id.unwrap_or(WeaponPresetId(0));
        let legacy_compatibility = record.source.legacy_compatibility;
        let source = cue_damage_source(record.source);
        let mut target_defeated = defeated.is_some() || applied.defeated.contains(&record.target);
        let (
            maximum_health,
            tenacity,
            cold_capacity,
            cold_resistance,
            poison_resistance,
            fire_resistance,
        ) = {
            let capabilities = combat.passive_access.p0();
            capabilities.get(record.target).map_or(
                (health.0, None, 1_000, 0, 0, 0),
                |(stats, passives)| {
                    (
                        stats.maximum_health,
                        passives.find(crate::builds::PassiveKind::Tenacity),
                        stats.cold_capacity,
                        stats.cold_resistance_basis_points,
                        stats.poison_resistance_basis_points,
                        stats.fire_resistance_basis_points,
                    )
                },
            )
        };
        let owner_contact = *target_network_id == record.source.owner_network_entity_id;
        if !owner_contact
            && !target_defeated
            && teams_are_hostile(record.source.team_id, *target_team)
            && applied.contacted_deliveries.insert((
                record.source.attack_id,
                record.delivery_index,
                target_network_id.0,
            ))
        {
            gameplay_telemetry
                .weapon
                .record_hostile_delivery_contact(preset_id, record.source.recipe_fingerprint);
            if let Some(tracker) = trackers.active.get_mut(&record.source.attack_id) {
                tracker.had_hostile_contact = true;
            }
        }
        let mut effects = record.bundle.effects.clone();
        effects.sort_by_key(|effect| {
            u8::from(!matches!(effect, PayloadEffectDefinition::Damage { .. }))
        });
        for effect in effects.iter().copied() {
            if !effect_allows_target(effect, target_kind) {
                continue;
            }
            let Some(scale) =
                effect_recipient_scale(effect, record.source, *target_network_id, *target_team)
            else {
                continue;
            };
            if let PayloadEffectDefinition::Damage {
                amount,
                falloff,
                recipients,
            } = effect
            {
                let close_quarters =
                    if matches!(record.source.kind, CombatSourceKind::PrimaryWeapon) {
                        batch
                            .close_quarters_owners
                            .get(&record.source.owner_network_entity_id.0)
                            .copied()
                    } else {
                        None
                    };
                let Some(plan) = plan_damage_application(
                    health.0,
                    target_defeated,
                    amount,
                    falloff,
                    record.delivery_travel,
                    scale,
                    close_quarters.map(|passive| passive.parameters),
                    record.engagement_distance,
                ) else {
                    continue;
                };
                if let Some(passive) = close_quarters
                    && plan.applied != plan.unmodified_applied
                {
                    gameplay_telemetry
                        .ability
                        .record(crate::abilities::AbilityTelemetryRecord {
                            tick,
                            owner_network_id: record.source.owner_network_entity_id,
                            kind: crate::abilities::AbilityTelemetryKind::PassiveModified {
                                passive_id: passive.id,
                                amount: plan.applied.abs_diff(plan.unmodified_applied),
                            },
                        });
                }
                let damage_event = reserved_events
                    .next()
                    .expect("complete payload event reservation matches damage");
                let legacy_damage_event = legacy_compatibility.then(|| {
                    reserved_events
                        .next()
                        .expect("payload event reservation matches legacy damage")
                });
                let defeat_event = plan.defeats.then(|| {
                    reserved_events
                        .next()
                        .expect("payload event reservation matches defeat")
                });
                let legacy_defeat_event = if plan.defeats && legacy_compatibility {
                    Some(
                        reserved_events
                            .next()
                            .expect("payload event reservation matches legacy defeat"),
                    )
                } else {
                    None
                };
                health.0 = plan.health_after;
                record_damage_application(
                    record,
                    tick,
                    source,
                    *target_network_id,
                    *target_team,
                    target_kind,
                    preset_id,
                    (amount, falloff, recipients),
                    plan.requested,
                    plan.applied,
                    plan.health_after,
                    damage_event,
                    legacy_damage_event,
                    effects_state,
                    motion_state,
                    gameplay_telemetry,
                    transaction,
                );
                if let Some(defeat_event) = defeat_event {
                    record_target_defeat(
                        commands,
                        record,
                        tick,
                        source,
                        *target_network_id,
                        *target_team,
                        target_kind,
                        preset_id,
                        owner_contact,
                        defeat_event,
                        legacy_defeat_event,
                        test_dummy,
                        applied,
                        gameplay_telemetry,
                        transaction,
                    );
                    target_defeated = true;
                }
            } else if let PayloadEffectDefinition::Heal { amount, .. } = effect {
                if effect_tile.is_some_and(crate::map::EffectTileOccupancy::blocks_healing) {
                    continue;
                }
                let plan = plan_healing_application(health.0, maximum_health, amount, scale);
                health.0 = plan.health_after;
                let event_id = reserved_events
                    .next()
                    .expect("payload event reservation matches healing");
                let cue = CombatCue::EffectApplied {
                    event_id,
                    tick,
                    attack_id: record.source.attack_id,
                    source,
                    target: *target_network_id,
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
                    target_network_id: *target_network_id,
                    target_kind,
                    target_team: *target_team,
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
                    target: Some(*target_network_id),
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
        }
        if target_defeated {
            applied.effects.remove(&record.target);
            applied.motion.remove(&record.target);
            continue;
        }
        if combat.sentry_targets.contains(record.target) {
            applied.effects.remove(&record.target);
            applied.motion.remove(&record.target);
            continue;
        }
        (effects_state, motion_state) = apply_runtime_effects(
            record,
            tick,
            source,
            *target_network_id,
            *target_team,
            owner_contact,
            *health,
            preset_id,
            effects_state,
            motion_state,
            tenacity,
            cold_capacity,
            condition_rules.freeze_duration_ticks,
            cold_resistance,
            poison_resistance,
            fire_resistance,
            applied
                .cold_contacts
                .insert((record.source.attack_id, target_network_id.0)),
            reserved_events,
            &mut gameplay_telemetry.weapon,
            &mut gameplay_telemetry.ability,
            &mut applied.deferred_cues,
        );
        applied.effects.insert(record.target, effects_state);
        if let Some(motion) = motion_state {
            applied.motion.insert(record.target, motion);
        }
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
fn record_damage_application(
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

// A defeat commits the terminal entity state, defeat telemetry, cues, and outcome facts
// for one target destroyed by this record.
#[allow(
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "each parameter is one fact of the defeat being recorded across entity state, telemetry, cues, and outcomes"
)]
fn record_target_defeat(
    commands: &mut Commands,
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
    test_dummy: Option<&TestDummy>,
    applied: &mut AppliedComposedState,
    gameplay_telemetry: &mut AbilityWeaponTelemetry,
    transaction: &mut CombatTransactionState,
) {
    applied.defeated.insert(record.target);
    commands
        .entity(record.target)
        .insert((
            Defeated {
                event_id: defeat_event,
            },
            CollisionLayers::new(
                if target_kind == CombatTargetKind::Deployable {
                    crate::movement::DEPLOYABLE_LAYER
                } else {
                    FIGHTER_LAYER
                },
                avian2d::prelude::LayerMask::NONE,
            ),
            ActiveEffects::default(),
        ))
        .remove::<ExternalMotion>()
        .remove::<KnockbackFeedback>();
    if let Some(test_dummy) = test_dummy {
        commands
            .entity(record.target)
            .insert(TestDummyResetDeadline(
                tick.saturating_add(test_dummy.reset_delay_ticks),
            ));
    }
    applied.effects.remove(&record.target);
    applied.motion.remove(&record.target);
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

/// Commit the accumulated batch state: install effects and motion on surviving targets,
/// publish deferred effect cues, and finish delivery trackers for resolved attacks.
pub(super) fn commit_composed_batch(
    commands: &mut Commands,
    trackers: &mut ActiveAttackTrackers,
    transaction: &mut CombatTransactionState,
    applied: AppliedComposedState,
    resolved_delivery_keys: HashSet<(AttackId, u8)>,
) {
    let AppliedComposedState {
        defeated,
        effects,
        motion,
        deferred_cues,
        ..
    } = applied;
    for (entity, effects) in effects {
        if !defeated.contains(&entity) {
            commands.entity(entity).insert(effects);
        }
    }
    for (entity, motion) in motion {
        if !defeated.contains(&entity) {
            commands.entity(entity).insert((
                motion,
                KnockbackFeedback {
                    velocity: WorldPoint::from(motion.velocity),
                    expires_at_tick: motion.expires_at_tick,
                },
            ));
        }
    }
    for (entity, cue) in deferred_cues {
        if !defeated.contains(&entity) {
            transaction.legacy_telemetry.record_cue(cue.clone());
            transaction.outbox.0.push(cue);
        }
    }
    for (attack_id, _) in resolved_delivery_keys {
        finish_attack_delivery(trackers, attack_id);
    }
}
