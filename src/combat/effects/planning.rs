//! Event reservation and deterministic target/delivery planning.

#![allow(clippy::wildcard_imports)]
use super::runtime::*;
use super::*;

pub(crate) fn payload_target_visible(
    source: AttackSource,
    team: TeamId,
    network_id: NetworkEntityId,
) -> bool {
    network_id == source.owner_network_entity_id || teams_are_hostile(source.team_id, team)
}

#[cfg(feature = "server")]
pub(super) fn pending_delivery_kind_order(kind: &PendingDeliveryKind) -> u8 {
    match kind {
        PendingDeliveryKind::StraightImpact { .. } => 0,
        PendingDeliveryKind::LobLanded { .. } => 1,
        PendingDeliveryKind::MeleeContact { .. } => 2,
    }
}

#[cfg(feature = "server")]
pub(super) fn record_delivery_telemetry(
    telemetry: &mut WeaponTelemetry,
    delivery: &PendingDelivery,
    event_id: CombatEventId,
    target: Option<NetworkEntityId>,
    position: WorldPoint,
    outcome: WeaponTelemetryOutcome,
) {
    telemetry.record(WeaponTelemetryRecord {
        tick: delivery.tick,
        event_id,
        attack_id: delivery.source.attack_id,
        preset_id: delivery
            .source
            .source_preset_id
            .unwrap_or(WeaponPresetId(0)),
        recipe_fingerprint: delivery.source.recipe_fingerprint,
        delivery_index: Some(delivery.delivery_index),
        source: delivery.source.owner_network_entity_id,
        target,
        position,
        requested_value: 0,
        applied_value: 0,
        engagement_distance: delivery.engagement_distance,
        delivery_travel: delivery.delivery_travel,
        hostile_contact: target.is_some(),
        effect: None,
        resulting_health: None,
        resulting_effects: None,
        resulting_motion: None,
        outcome,
    });
}

#[cfg(feature = "server")]
pub(super) fn abort_composed_event_batch(
    commands: &mut Commands,
    trackers: &mut ActiveAttackTrackers,
    deliveries: &[PendingDelivery],
    payloads: &[PendingPayload],
) {
    let mut affected_attacks = HashSet::new();
    for delivery in deliveries {
        affected_attacks.insert(delivery.source.attack_id);
        if let Some(entity) = delivery.entity {
            commands.entity(entity).try_despawn();
        }
    }
    for payload in payloads {
        affected_attacks.insert(payload.source.attack_id);
    }
    for attack_id in affected_attacks {
        trackers.active.remove(&attack_id);
    }
}

#[cfg(feature = "server")]
pub(crate) fn finish_attack_delivery(trackers: &mut ActiveAttackTrackers, attack_id: AttackId) {
    let Some(tracker) = trackers.active.get_mut(&attack_id) else {
        return;
    };
    tracker.resolved_deliveries = tracker
        .resolved_deliveries
        .saturating_add(1)
        .min(tracker.expected_deliveries);
    if tracker.resolved_deliveries >= tracker.expected_deliveries
        && let Some(tracker) = trackers.active.remove(&attack_id)
    {
        trackers.completed.push(CompletedAttack {
            source_preset_id: tracker.source.source_preset_id,
            recipe_fingerprint: tracker.source.recipe_fingerprint,
            had_hostile_contact: tracker.had_hostile_contact,
        });
    }
}

#[cfg(feature = "server")]
pub(crate) fn flush_completed_attack_telemetry(
    mut trackers: ResMut<ActiveAttackTrackers>,
    mut telemetry: ResMut<WeaponTelemetry>,
) {
    for completed in trackers.completed.drain(..) {
        telemetry.record_attack_completion(
            completed.source_preset_id.unwrap_or(WeaponPresetId(0)),
            completed.recipe_fingerprint,
            completed.had_hostile_contact,
        );
    }
}

#[cfg(feature = "server")]
pub(super) fn required_payload_event_count(
    delivery_records: &[PendingDelivery],
    records: &[PendingPayload],
    connected_owners: &HashSet<u64>,
    close_quarters_owners: &HashSet<u64>,
    planned_targets: &mut HashMap<Entity, PlannedTarget>,
) -> Option<usize> {
    let mut required = 0_usize;
    for delivery in delivery_records
        .iter()
        .filter(|delivery| connected_owners.contains(&delivery.source.owner_network_entity_id.0))
    {
        required = required.checked_add(
            1 + usize::from(
                delivery.source.legacy_compatibility
                    && matches!(delivery.kind, PendingDeliveryKind::StraightImpact { .. }),
            ),
        )?;
    }
    for record in records {
        if !connected_owners.contains(&record.source.owner_network_entity_id.0) {
            continue;
        }
        let Some((target_network_id, target_team, health, defeated, target_kind)) =
            planned_targets.get_mut(&record.target)
        else {
            continue;
        };
        let legacy_compatibility = record.source.legacy_compatibility;
        let mut effects = record.bundle.effects.clone();
        effects.sort_by_key(|effect| {
            u8::from(!matches!(effect, PayloadEffectDefinition::Damage { .. }))
        });
        for effect in effects {
            if !combat_source_allows_target(record.source.kind, *target_kind)
                || !effect_allows_target(effect, *target_kind)
            {
                continue;
            }
            let Some(scale) =
                effect_recipient_scale(effect, record.source, *target_network_id, *target_team)
            else {
                continue;
            };
            let event_count = match effect {
                PayloadEffectDefinition::Damage {
                    amount, falloff, ..
                } => {
                    let requested = requested_damage(
                        amount,
                        falloff,
                        record.delivery_travel,
                        scale,
                        matches!(record.source.kind, CombatSourceKind::PrimaryWeapon)
                            && close_quarters_owners
                                .contains(&record.source.owner_network_entity_id.0),
                        record.engagement_distance,
                    );
                    let applied = requested.min(*health);
                    if applied == 0 {
                        0
                    } else {
                        let defeats = !*defeated && *health == applied;
                        *health = health.saturating_sub(applied);
                        if defeats {
                            *defeated = true;
                            2 + usize::from(legacy_compatibility) * 2
                        } else {
                            1 + usize::from(legacy_compatibility)
                        }
                    }
                }
                PayloadEffectDefinition::Knockback { .. }
                | PayloadEffectDefinition::Slow { .. }
                    if !*defeated =>
                {
                    1
                }
                PayloadEffectDefinition::Knockback { .. }
                | PayloadEffectDefinition::Slow { .. } => 0,
            };
            required = required.checked_add(event_count)?;
        }
    }
    Some(required)
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_pending_deliveries(
    commands: &mut Commands,
    delivery_records: Vec<PendingDelivery>,
    connected_owners: &HashSet<u64>,
    reserved_events: &mut impl Iterator<Item = CombatEventId>,
    trackers: &mut ActiveAttackTrackers,
    telemetry: &mut WeaponTelemetry,
    legacy_telemetry: &mut CombatTelemetry,
    outbox: &mut CombatOutbox,
    world_effect_facts: &mut CombatWorldEffectFacts,
) -> HashSet<(AttackId, u8)> {
    let mut resolved_delivery_keys = HashSet::new();
    for delivery in delivery_records {
        resolved_delivery_keys.insert((delivery.source.attack_id, delivery.delivery_index));
        if !connected_owners.contains(&delivery.source.owner_network_entity_id.0) {
            if let Some(entity) = delivery.entity {
                commands.entity(entity).try_despawn();
            }
            finish_attack_delivery(trackers, delivery.source.attack_id);
            continue;
        }
        let event_id = reserved_events
            .next()
            .expect("delivery event reservation matches pending deliveries");
        let weapon_definition_id = WeaponDefinitionId(
            delivery
                .source
                .source_preset_id
                .map_or(0, |preset| preset.0),
        );
        match delivery.kind {
            PendingDeliveryKind::StraightImpact {
                target,
                position,
                normal,
                distance_band,
            } => {
                let cue = CombatCue::DeliveryImpact {
                    event_id,
                    tick: delivery.tick,
                    attack_id: delivery.source.attack_id,
                    delivery_index: delivery.delivery_index,
                    source: delivery.source.owner_network_entity_id,
                    weapon_definition_id,
                    presentation_profile_id: delivery.source.presentation_profile_id,
                    target,
                    position,
                    normal,
                    distance_band,
                };
                legacy_telemetry.record_cue(cue.clone());
                outbox.0.push(cue);
                record_delivery_telemetry(
                    telemetry,
                    &delivery,
                    event_id,
                    target,
                    position,
                    WeaponTelemetryOutcome::DeliveryImpact,
                );
                if delivery.source.legacy_compatibility {
                    let legacy_event = reserved_events
                        .next()
                        .expect("legacy impact reservation matches delivery");
                    let legacy_source = ProjectileSource {
                        shot_id: ShotId(delivery.source.attack_id.0),
                        player_id: delivery.source.player_id,
                        owner_network_entity_id: delivery.source.owner_network_entity_id,
                        team_id: delivery.source.team_id,
                        weapon_definition_id: PULSE_SIDEARM_DEFINITION,
                    };
                    let legacy_cue = CombatCue::Impact {
                        event_id: legacy_event,
                        tick: delivery.tick,
                        source: delivery.source.owner_network_entity_id,
                        shot_id: legacy_source.shot_id,
                        weapon_definition_id: legacy_source.weapon_definition_id,
                        target,
                        position,
                        normal,
                        distance_band,
                    };
                    legacy_telemetry.record_cue(legacy_cue.clone());
                    legacy_telemetry.record(CombatLogRecord::Hit {
                        tick: delivery.tick,
                        event_id: legacy_event,
                        shot_id: legacy_source.shot_id,
                        source: delivery.source.owner_network_entity_id,
                        target,
                        weapon: legacy_source.weapon_definition_id,
                        position,
                        distance: delivery
                            .source
                            .origin
                            .as_vec2()
                            .distance(position.as_vec2()),
                        band: distance_band,
                    });
                    outbox.0.push(legacy_cue);
                }
            }
            PendingDeliveryKind::LobLanded { position } => {
                let cue = CombatCue::LobLanded {
                    event_id,
                    tick: delivery.tick,
                    attack_id: delivery.source.attack_id,
                    delivery_index: delivery.delivery_index,
                    source: delivery.source.owner_network_entity_id,
                    weapon_definition_id,
                    presentation_profile_id: delivery.source.presentation_profile_id,
                    position,
                };
                legacy_telemetry.record_cue(cue.clone());
                outbox.0.push(cue);
                record_delivery_telemetry(
                    telemetry,
                    &delivery,
                    event_id,
                    None,
                    position,
                    WeaponTelemetryOutcome::DeliveryLanding,
                );
            }
            PendingDeliveryKind::MeleeContact { target, position } => {
                let cue = CombatCue::MeleeContact {
                    event_id,
                    tick: delivery.tick,
                    attack_id: delivery.source.attack_id,
                    delivery_index: delivery.delivery_index,
                    source: delivery.source.owner_network_entity_id,
                    weapon_definition_id,
                    presentation_profile_id: delivery.source.presentation_profile_id,
                    target,
                    position,
                };
                legacy_telemetry.record_cue(cue.clone());
                outbox.0.push(cue);
                record_delivery_telemetry(
                    telemetry,
                    &delivery,
                    event_id,
                    Some(target),
                    position,
                    WeaponTelemetryOutcome::MeleeContact,
                );
            }
        }
        // World effects are delivery-level facts: exactly one per authored effect for this
        // committed delivery, independent of target count.
        for (effect_index, effect) in delivery.world_effects.iter().enumerate() {
            let effect_index = u8::try_from(effect_index).unwrap_or(u8::MAX);
            let position = match &delivery.kind {
                PendingDeliveryKind::StraightImpact { position, .. }
                | PendingDeliveryKind::LobLanded { position }
                | PendingDeliveryKind::MeleeContact { position, .. } => *position,
            };
            world_effect_facts.0.push(CombatWorldEffectFact {
                tick: delivery.tick,
                source: CombatWorldEffectSource::Weapon {
                    attack: delivery.source,
                    delivery_index: delivery.delivery_index,
                    effect_index,
                },
                position,
                effect: *effect,
            });
        }
        if let Some(entity) = delivery.entity {
            commands.entity(entity).try_despawn();
        }
    }
    resolved_delivery_keys
}
