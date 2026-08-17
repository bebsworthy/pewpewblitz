//! Deterministic immediate effect policies.

#[allow(clippy::wildcard_imports)]
#[cfg(feature = "server")]
use super::*;
use super::{ActiveEffects, AttackId, ExternalMotion, NetworkEntityId, SlowEffect};
use bevy::prelude::Vec2;

pub const MAX_EXTERNAL_MOTION_SPEED: f32 = 900.0;

#[must_use]
pub fn combine_knockback(
    existing: Option<ExternalMotion>,
    velocity: Vec2,
    expires_at_tick: u64,
) -> ExternalMotion {
    let combined = existing.map_or(velocity, |old| old.velocity + velocity);
    ExternalMotion {
        velocity: combined.clamp_length_max(MAX_EXTERNAL_MOTION_SPEED),
        expires_at_tick: existing.map_or(expires_at_tick, |old| {
            old.expires_at_tick.max(expires_at_tick)
        }),
    }
}

pub fn refresh_strongest_slow(
    effects: &mut ActiveEffects,
    source_attack_id: AttackId,
    source_network_entity_id: NetworkEntityId,
    movement_multiplier_milli: u16,
    expires_at_tick: u64,
) {
    let next = SlowEffect {
        source_attack_id,
        source_network_entity_id,
        movement_multiplier_milli,
        expires_at_tick,
    };
    match effects.slow {
        None => effects.slow = Some(next),
        Some(current) if movement_multiplier_milli < current.movement_multiplier_milli => {
            effects.slow = Some(next);
        }
        Some(mut current) => {
            current.expires_at_tick = current.expires_at_tick.max(expires_at_tick);
            effects.slow = Some(current);
        }
    }
}

#[cfg(feature = "server")]
pub(super) fn payload_target_visible(
    source: AttackSource,
    team: TeamId,
    network_id: NetworkEntityId,
) -> bool {
    network_id == source.owner_network_entity_id || teams_are_hostile(source.team_id, team)
}

#[cfg(feature = "server")]
fn pending_delivery_kind_order(kind: &PendingDeliveryKind) -> u8 {
    match kind {
        PendingDeliveryKind::StraightImpact { .. } => 0,
        PendingDeliveryKind::LobLanded { .. } => 1,
        PendingDeliveryKind::MeleeContact { .. } => 2,
    }
}

#[cfg(feature = "server")]
fn record_delivery_telemetry(
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
fn abort_composed_event_batch(
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
pub(super) fn finish_attack_delivery(trackers: &mut ActiveAttackTrackers, attack_id: AttackId) {
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
pub(super) fn flush_completed_attack_telemetry(
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
fn effect_recipient_scale(
    effect: PayloadEffectDefinition,
    source: AttackSource,
    target_network_id: NetworkEntityId,
    target_team: TeamId,
) -> Option<f32> {
    let recipients = match effect {
        PayloadEffectDefinition::Damage { recipients, .. }
        | PayloadEffectDefinition::Knockback { recipients, .. }
        | PayloadEffectDefinition::Slow { recipients, .. } => recipients,
    };
    if target_network_id == source.owner_network_entity_id {
        match recipients {
            RecipientPolicy::HostilesAndOwner { owner_scale } => Some(owner_scale),
            RecipientPolicy::Hostiles => None,
        }
    } else if teams_are_hostile(source.team_id, target_team) {
        Some(1.0)
    } else {
        None
    }
}

#[cfg(feature = "server")]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
fn requested_damage(
    amount: u16,
    falloff: DamageFalloff,
    delivery_travel: f32,
    recipient_scale: f32,
    close_quarters: bool,
    engagement_distance: f32,
) -> u16 {
    let base = f32::from(amount) * linear_falloff(falloff, delivery_travel) * recipient_scale;
    let requested = if close_quarters {
        crate::abilities::apply_close_quarters_scale(base, engagement_distance)
    } else {
        base
    };
    requested.clamp(1.0, f32::from(u16::MAX)).round() as u16
}

#[cfg(feature = "server")]
type PlannedTarget = (NetworkEntityId, TeamId, u16, bool, CombatTargetKind);

#[cfg(feature = "server")]
const fn combat_source_allows_target(source: CombatSourceKind, target: CombatTargetKind) -> bool {
    match (source, target) {
        (
            CombatSourceKind::PrimaryWeapon
            | CombatSourceKind::Ultimate { .. }
            | CombatSourceKind::Deployable { .. },
            CombatTargetKind::Fighter | CombatTargetKind::Deployable,
        ) => true,
    }
}

#[cfg(feature = "server")]
const fn effect_allows_target(effect: PayloadEffectDefinition, target: CombatTargetKind) -> bool {
    matches!(effect, PayloadEffectDefinition::Damage { .. })
        || matches!(target, CombatTargetKind::Fighter)
}

#[cfg(feature = "server")]
fn cue_damage_source(source: AttackSource) -> DamageSource {
    match source.kind {
        CombatSourceKind::PrimaryWeapon => DamageSource::PlayerWeapon {
            player_id: source.player_id,
            fighter_id: source.owner_network_entity_id,
            weapon_definition_id: WeaponDefinitionId(
                source.source_preset_id.map_or(0, |preset| preset.0),
            ),
            shot_id: ShotId(source.attack_id.0),
        },
        CombatSourceKind::Ultimate { ultimate_id } => DamageSource::Ultimate {
            player_id: source.player_id,
            fighter_id: source.owner_network_entity_id,
            ultimate_id,
            attack_id: source.attack_id,
        },
        CombatSourceKind::Deployable {
            ultimate_id,
            deployable_id,
        } => DamageSource::Deployable {
            player_id: source.player_id,
            fighter_id: source.owner_network_entity_id,
            ultimate_id,
            deployable_id,
            attack_id: source.attack_id,
        },
    }
}

#[cfg(feature = "server")]
fn required_payload_event_count(
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
fn resolve_pending_deliveries(
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
                source: delivery.source,
                delivery_index: delivery.delivery_index,
                effect_index,
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

#[cfg(feature = "server")]
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
fn apply_runtime_effects(
    record: &PendingPayload,
    tick: u64,
    source: DamageSource,
    target_network_id: NetworkEntityId,
    target_team: TeamId,
    owner_contact: bool,
    health: CurrentHealth,
    preset_id: WeaponPresetId,
    mut effects_state: ActiveEffects,
    mut motion_state: Option<ExternalMotion>,
    tenacity: bool,
    reserved_events: &mut impl Iterator<Item = CombatEventId>,
    telemetry: &mut WeaponTelemetry,
    ability_telemetry: &mut crate::abilities::AbilityTelemetry,
    deferred_effect_cues: &mut Vec<(Entity, CombatCue)>,
) -> (ActiveEffects, Option<ExternalMotion>) {
    for effect in record.bundle.effects.iter().copied() {
        let Some(scale) =
            effect_recipient_scale(effect, record.source, target_network_id, target_team)
        else {
            continue;
        };
        match effect {
            PayloadEffectDefinition::Knockback {
                speed,
                duration_ticks,
                recipients,
            } => {
                let direction =
                    (record.position - record.source.origin.as_vec2()).normalize_or_zero();
                let motion = combine_knockback(
                    motion_state,
                    direction * speed * scale,
                    tick.saturating_add(duration_ticks),
                );
                motion_state = Some(motion);
                let event_id = reserved_events
                    .next()
                    .expect("payload event reservation matches knockback");
                let effect_cue = CombatCue::EffectApplied {
                    event_id,
                    tick,
                    attack_id: record.source.attack_id,
                    source,
                    target: target_network_id,
                    position: WorldPoint::from(record.position),
                    effect: CombatEffectCue::Knockback {
                        velocity: WorldPoint::from(motion.velocity),
                        expires_at_tick: motion.expires_at_tick,
                    },
                    presentation_profile_id: record.source.presentation_profile_id,
                };
                telemetry.record(WeaponTelemetryRecord {
                    tick,
                    event_id,
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
                    effect: Some(PayloadEffectDefinition::Knockback {
                        speed,
                        duration_ticks,
                        recipients,
                    }),
                    resulting_health: Some(health.0),
                    resulting_effects: Some(effects_state),
                    resulting_motion: Some(motion),
                    outcome: WeaponTelemetryOutcome::KnockbackApplied,
                });
                deferred_effect_cues.push((record.target, effect_cue));
            }
            PayloadEffectDefinition::Slow {
                movement_multiplier,
                duration_ticks,
                stacking,
                recipients,
            } => {
                let base_duration_ticks = duration_ticks;
                let duration_ticks = if tenacity {
                    crate::abilities::apply_tenacity_ticks(duration_ticks)
                } else {
                    duration_ticks
                };
                if tenacity && duration_ticks < base_duration_ticks {
                    ability_telemetry.record(crate::abilities::AbilityTelemetryRecord {
                        tick,
                        owner_network_id: target_network_id,
                        kind: crate::abilities::AbilityTelemetryKind::PassiveModified {
                            passive_id: crate::builds::PassiveDefinitionId(6),
                            amount: u16::try_from(
                                base_duration_ticks.saturating_sub(duration_ticks),
                            )
                            .unwrap_or(u16::MAX),
                        },
                    });
                }
                refresh_strongest_slow(
                    &mut effects_state,
                    record.source.attack_id,
                    record.source.owner_network_entity_id,
                    (movement_multiplier * scale * 1000.0)
                        .round()
                        .clamp(1.0, 1000.0) as u16,
                    tick.saturating_add(duration_ticks),
                );
                if let Some(slow) = effects_state.slow {
                    let event_id = reserved_events
                        .next()
                        .expect("payload event reservation matches slow");
                    let effect_cue = CombatCue::EffectApplied {
                        event_id,
                        tick,
                        attack_id: record.source.attack_id,
                        source,
                        target: target_network_id,
                        position: WorldPoint::from(record.position),
                        effect: CombatEffectCue::Slow {
                            movement_multiplier_milli: slow.movement_multiplier_milli,
                            expires_at_tick: slow.expires_at_tick,
                        },
                        presentation_profile_id: record.source.presentation_profile_id,
                    };
                    telemetry.record(WeaponTelemetryRecord {
                        tick,
                        event_id,
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
                        effect: Some(PayloadEffectDefinition::Slow {
                            movement_multiplier,
                            duration_ticks,
                            stacking,
                            recipients,
                        }),
                        resulting_health: Some(health.0),
                        resulting_effects: Some(effects_state),
                        resulting_motion: motion_state,
                        outcome: WeaponTelemetryOutcome::SlowApplied,
                    });
                    deferred_effect_cues.push((record.target, effect_cue));
                }
            }
            PayloadEffectDefinition::Damage { .. } => {}
        }
    }
    (effects_state, motion_state)
}

/// The bounded authoritative transaction outputs produced by payload resolution, grouped
/// to keep the scheduling system within the engine's system-parameter budget.
#[cfg(feature = "server")]
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct CombatTransactionState<'w> {
    legacy_telemetry: ResMut<'w, CombatTelemetry>,
    outbox: ResMut<'w, CombatOutbox>,
    world_effect_facts: ResMut<'w, CombatWorldEffectFacts>,
    outcome_facts: ResMut<'w, CombatOutcomeFacts>,
}

#[cfg(feature = "server")]
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub(super) fn resolve_composed_payloads(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    mut ids: ResMut<NextCombatIds>,
    mut trackers: ResMut<ActiveAttackTrackers>,
    mut payloads: MessageReader<PendingPayload>,
    mut deliveries: MessageReader<PendingDelivery>,
    mut gameplay_telemetry: AbilityWeaponTelemetry,
    mut transaction: CombatTransactionState,
    mut target_queries: ParamSet<(
        Query<
            (
                &NetworkEntityId,
                &TeamId,
                &mut CurrentHealth,
                Option<&mut ActiveEffects>,
                Option<&ExternalMotion>,
                Option<&Defeated>,
                Option<&lightyear::prelude::ControlledBy>,
                Option<&TestDummy>,
            ),
            Or<(With<Fighter>, With<crate::abilities::Sentry>)>,
        >,
        Query<
            (
                Entity,
                &NetworkEntityId,
                &TeamId,
                &CurrentHealth,
                Option<&Defeated>,
                Option<&lightyear::prelude::ControlledBy>,
            ),
            Or<(With<Fighter>, With<crate::abilities::Sentry>)>,
        >,
    )>,
    owners: Query<(&NetworkEntityId, Option<&lightyear::prelude::ControlledBy>), With<Fighter>>,
    mut passive_access: ParamSet<(
        Query<&crate::builds::ResolvedMatchLoadout>,
        Query<(&NetworkEntityId, &crate::builds::ResolvedMatchLoadout), With<Fighter>>,
    )>,
    sentry_targets: Query<(), With<crate::abilities::Sentry>>,
    disconnected: Query<Entity, (With<LinkOf>, With<lightyear::prelude::Disconnected>)>,
    mut match_access: ParamSet<(
        Query<(), With<crate::matchplay::MatchParticipant>>,
        Query<(), With<crate::matchplay::ActiveCombatant>>,
        Query<(), With<crate::matchplay::SpawnProtection>>,
    )>,
) {
    let disconnected: HashSet<_> = disconnected.iter().collect();
    let connected_owners: HashSet<_> = owners
        .iter()
        .filter(|(_, controlled)| {
            controlled.is_none_or(|controlled| !disconnected.contains(&controlled.owner))
        })
        .map(|(network_id, _)| network_id.0)
        .collect();
    let close_quarters_owners: HashSet<_> = passive_access
        .p1()
        .iter()
        .filter(|(_, loadout)| {
            loadout
                .passives
                .iter()
                .any(|passive| passive.kind == crate::builds::PassiveKind::CloseQuarters)
        })
        .map(|(network_id, _)| network_id.0)
        .collect();
    let mut records: Vec<_> = payloads.read().cloned().collect();
    records.sort_by(|left, right| {
        left.target_network_id
            .0
            .cmp(&right.target_network_id.0)
            .then_with(|| left.contact_fraction.total_cmp(&right.contact_fraction))
            .then_with(|| left.source.attack_id.0.cmp(&right.source.attack_id.0))
            .then_with(|| left.delivery_index.cmp(&right.delivery_index))
            .then_with(|| left.bundle_index.cmp(&right.bundle_index))
    });
    let mut delivery_records: Vec<_> = deliveries.read().cloned().collect();
    delivery_records.sort_by(|left, right| {
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

    // Dry-run the complete sorted batch against a snapshot before reserving any outcome IDs or
    // mutating health/effects. Event exhaustion must abort the whole batch, not leave an earlier
    // target partially committed while a later record fails to reserve its IDs.
    let mut planned_targets: HashMap<Entity, PlannedTarget> = target_queries
        .p1()
        .iter()
        .filter(|(_, _, _, _, _, controlled)| {
            controlled.is_none_or(|controlled| !disconnected.contains(&controlled.owner))
        })
        .map(|(entity, network_id, team, health, defeated, _)| {
            (
                entity,
                (
                    *network_id,
                    *team,
                    health.0,
                    defeated.is_some(),
                    if sentry_targets.contains(entity) {
                        CombatTargetKind::Deployable
                    } else {
                        CombatTargetKind::Fighter
                    },
                ),
            )
        })
        .collect();
    let Some(required_event_count) = required_payload_event_count(
        &delivery_records,
        &records,
        &connected_owners,
        &close_quarters_owners,
        &mut planned_targets,
    ) else {
        gameplay_telemetry.weapon.event_reservation_drops = gameplay_telemetry
            .weapon
            .event_reservation_drops
            .saturating_add(1);
        abort_composed_event_batch(&mut commands, &mut trackers, &delivery_records, &records);
        return;
    };
    let Some(reserved_events) = server::reserve_event_ids(&mut ids, required_event_count) else {
        gameplay_telemetry.weapon.event_reservation_drops = gameplay_telemetry
            .weapon
            .event_reservation_drops
            .saturating_add(1);
        abort_composed_event_batch(&mut commands, &mut trackers, &delivery_records, &records);
        return;
    };
    let mut reserved_events = reserved_events.into_iter();
    let mut targets = target_queries.p0();
    let mut contacted_deliveries = HashSet::new();
    let mut defeated_this_tick = HashSet::new();
    let mut accumulated_effects: HashMap<Entity, ActiveEffects> = HashMap::new();
    let mut accumulated_motion: HashMap<Entity, ExternalMotion> = HashMap::new();
    let mut deferred_effect_cues: Vec<(Entity, CombatCue)> = Vec::new();
    let mut resolved_delivery_keys = resolve_pending_deliveries(
        &mut commands,
        delivery_records,
        &connected_owners,
        &mut reserved_events,
        &mut trackers,
        &mut gameplay_telemetry.weapon,
        &mut transaction.legacy_telemetry,
        &mut transaction.outbox,
        &mut transaction.world_effect_facts,
    );
    for record in records {
        resolved_delivery_keys.insert((record.source.attack_id, record.delivery_index));
        if !connected_owners.contains(&record.source.owner_network_entity_id.0) {
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
        )) = targets.get_mut(record.target)
        else {
            continue;
        };
        if controlled_by.is_some_and(|controlled| disconnected.contains(&controlled.owner)) {
            continue;
        }
        let target_kind = if sentry_targets.contains(record.target) {
            CombatTargetKind::Deployable
        } else {
            CombatTargetKind::Fighter
        };
        if !combat_source_allows_target(record.source.kind, target_kind) {
            continue;
        }
        let match_participant = match_access.p0().contains(record.target);
        let active_combatant = match_access.p1().contains(record.target);
        if match_participant && !active_combatant {
            continue;
        }
        if match_access.p2().contains(record.target)
            && teams_are_hostile(record.source.team_id, *target_team)
        {
            if let Some(event_id) = reserved_events.next() {
                transaction.outcome_facts.0.push(CombatOutcomeFact {
                    event_id,
                    tick: tick.0,
                    attack_id: record.source.attack_id,
                    source_kind: record.source.kind,
                    source_player: Some(record.source.player_id),
                    source_network_id: Some(record.source.owner_network_entity_id),
                    source_team: Some(record.source.team_id),
                    target_network_id: *target_network_id,
                    target_kind: if sentry_targets.contains(record.target) {
                        CombatTargetKind::Deployable
                    } else {
                        CombatTargetKind::Fighter
                    },
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
        let mut effects_state = accumulated_effects
            .get(&record.target)
            .copied()
            .unwrap_or_else(|| {
                active_effects.map_or_else(ActiveEffects::default, |effects| *effects)
            });
        let mut motion_state = accumulated_motion
            .get(&record.target)
            .copied()
            .or(external_motion.copied());
        let preset_id = record.source.source_preset_id.unwrap_or(WeaponPresetId(0));
        let legacy_compatibility = record.source.legacy_compatibility;
        let source = cue_damage_source(record.source);
        let mut target_defeated = defeated.is_some() || defeated_this_tick.contains(&record.target);
        let owner_contact = *target_network_id == record.source.owner_network_entity_id;
        if !owner_contact
            && !target_defeated
            && teams_are_hostile(record.source.team_id, *target_team)
            && contacted_deliveries.insert((
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
        let mut projected_health = health.0;
        let mut projected_defeated = target_defeated;
        for effect in effects.iter().copied() {
            if !effect_allows_target(effect, target_kind) {
                continue;
            }
            let Some(scale) =
                effect_recipient_scale(effect, record.source, *target_network_id, *target_team)
            else {
                continue;
            };
            match effect {
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
                    let applied = requested.min(projected_health);
                    if applied > 0 {
                        // IDs for the complete batch were reserved by the dry-run above.
                        if !projected_defeated && projected_health == applied {
                            projected_defeated = true;
                        }
                        projected_health = projected_health.saturating_sub(applied);
                    }
                }
                PayloadEffectDefinition::Knockback { .. }
                | PayloadEffectDefinition::Slow { .. }
                    if !projected_defeated => {}
                PayloadEffectDefinition::Knockback { .. }
                | PayloadEffectDefinition::Slow { .. } => {}
            }
        }
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
                let close_quarters = matches!(record.source.kind, CombatSourceKind::PrimaryWeapon)
                    && close_quarters_owners.contains(&record.source.owner_network_entity_id.0);
                let unmodified_requested = requested_damage(
                    amount,
                    falloff,
                    record.delivery_travel,
                    scale,
                    false,
                    record.engagement_distance,
                );
                let requested = requested_damage(
                    amount,
                    falloff,
                    record.delivery_travel,
                    scale,
                    close_quarters,
                    record.engagement_distance,
                );
                let applied = requested.min(health.0);
                if applied == 0 {
                    continue;
                }
                let defeats = applied > 0 && !target_defeated && health.0 == applied;
                let unmodified_applied = unmodified_requested.min(health.0);
                if close_quarters && applied != unmodified_applied {
                    gameplay_telemetry
                        .ability
                        .record(crate::abilities::AbilityTelemetryRecord {
                            tick: tick.0,
                            owner_network_id: record.source.owner_network_entity_id,
                            kind: crate::abilities::AbilityTelemetryKind::PassiveModified {
                                passive_id: crate::builds::PassiveDefinitionId(4),
                                amount: applied.abs_diff(unmodified_applied),
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
                let defeat_event = if defeats {
                    Some(
                        reserved_events
                            .next()
                            .expect("payload event reservation matches defeat"),
                    )
                } else {
                    None
                };
                let legacy_defeat_event = if defeats && legacy_compatibility {
                    Some(
                        reserved_events
                            .next()
                            .expect("payload event reservation matches legacy defeat"),
                    )
                } else {
                    None
                };
                health.0 = health.0.saturating_sub(applied);
                if applied > 0 {
                    let owner_damage = *target_network_id == record.source.owner_network_entity_id;
                    let band = distance_band(record.engagement_distance);
                    gameplay_telemetry.weapon.record_damage(
                        preset_id,
                        record.source.recipe_fingerprint,
                        owner_damage,
                        band,
                        applied,
                    );
                    transaction.legacy_telemetry.applied_damage = transaction
                        .legacy_telemetry
                        .applied_damage
                        .saturating_add(u64::from(applied));
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
                        tick: tick.0,
                        attack_id: record.source.attack_id,
                        source,
                        target: *target_network_id,
                        position: WorldPoint::from(record.position),
                        amount: applied,
                        health_after: health.0,
                        distance_band: distance_band(record.engagement_distance),
                        presentation_profile_id: record.source.presentation_profile_id,
                    };
                    transaction.legacy_telemetry.record_cue(damage_cue.clone());
                    transaction.outbox.0.push(damage_cue);
                    if let Some(legacy_damage_event) = legacy_damage_event {
                        let legacy_cue = CombatCue::Damage {
                            event_id: legacy_damage_event,
                            tick: tick.0,
                            source,
                            target: *target_network_id,
                            amount: applied,
                            health_after: health.0,
                            distance_band: distance_band(record.engagement_distance),
                        };
                        transaction.legacy_telemetry.record_cue(legacy_cue.clone());
                        transaction
                            .legacy_telemetry
                            .record(CombatLogRecord::Damage {
                                tick: tick.0,
                                event_id: legacy_damage_event,
                                source,
                                target: *target_network_id,
                                requested,
                                applied,
                                health_after: health.0,
                            });
                        transaction.outbox.0.push(legacy_cue);
                    }
                    gameplay_telemetry.weapon.record(WeaponTelemetryRecord {
                        tick: tick.0,
                        event_id: damage_event,
                        attack_id: record.source.attack_id,
                        preset_id,
                        recipe_fingerprint: record.source.recipe_fingerprint,
                        delivery_index: Some(record.delivery_index),
                        source: record.source.owner_network_entity_id,
                        target: Some(*target_network_id),
                        position: WorldPoint::from(record.position),
                        requested_value: requested,
                        applied_value: applied,
                        engagement_distance: record.engagement_distance,
                        delivery_travel: record.delivery_travel,
                        hostile_contact: !owner_damage,
                        effect: Some(PayloadEffectDefinition::Damage {
                            amount,
                            falloff,
                            recipients,
                        }),
                        resulting_health: Some(health.0),
                        resulting_effects: Some(effects_state),
                        resulting_motion: motion_state,
                        outcome: WeaponTelemetryOutcome::DamageApplied,
                    });
                    transaction.outcome_facts.0.push(CombatOutcomeFact {
                        event_id: damage_event,
                        tick: tick.0,
                        attack_id: record.source.attack_id,
                        source_kind: record.source.kind,
                        source_player: Some(record.source.player_id),
                        source_network_id: Some(record.source.owner_network_entity_id),
                        source_team: Some(record.source.team_id),
                        target_network_id: *target_network_id,
                        target_kind: if sentry_targets.contains(record.target) {
                            CombatTargetKind::Deployable
                        } else {
                            CombatTargetKind::Fighter
                        },
                        target_team: *target_team,
                        preset_id: record.source.source_preset_id,
                        recipe_fingerprint: Some(record.source.recipe_fingerprint),
                        position: WorldPoint::from(record.position),
                        engagement_distance: record.engagement_distance,
                        kind: CombatOutcomeKind::Damage { amount: applied },
                    });
                }
                if let Some(defeat_event) = defeat_event {
                    defeated_this_tick.insert(record.target);
                    target_defeated = true;
                    commands
                        .entity(record.target)
                        .insert((
                            Defeated {
                                event_id: defeat_event,
                            },
                            CollisionLayers::new(
                                if sentry_targets.contains(record.target) {
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
                    if test_dummy.is_some() {
                        commands
                            .entity(record.target)
                            .insert(TestDummyResetDeadline(tick.0.saturating_add(90)));
                    }
                    accumulated_effects.remove(&record.target);
                    accumulated_motion.remove(&record.target);
                    gameplay_telemetry
                        .weapon
                        .record_defeat(preset_id, record.source.recipe_fingerprint);
                    gameplay_telemetry.weapon.record(WeaponTelemetryRecord {
                        tick: tick.0,
                        event_id: defeat_event,
                        attack_id: record.source.attack_id,
                        preset_id,
                        recipe_fingerprint: record.source.recipe_fingerprint,
                        delivery_index: Some(record.delivery_index),
                        source: record.source.owner_network_entity_id,
                        target: Some(*target_network_id),
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
                    transaction.legacy_telemetry.defeats =
                        transaction.legacy_telemetry.defeats.saturating_add(1);
                    let defeated_cue = CombatCue::FighterDefeated {
                        event_id: defeat_event,
                        tick: tick.0,
                        attack_id: record.source.attack_id,
                        source: Some(source),
                        target: *target_network_id,
                        position: WorldPoint::from(record.position),
                        presentation_profile_id: Some(record.source.presentation_profile_id),
                    };
                    transaction
                        .legacy_telemetry
                        .record_cue(defeated_cue.clone());
                    transaction.outbox.0.push(defeated_cue);
                    transaction.outcome_facts.0.push(CombatOutcomeFact {
                        event_id: defeat_event,
                        tick: tick.0,
                        attack_id: record.source.attack_id,
                        source_kind: record.source.kind,
                        source_player: Some(record.source.player_id),
                        source_network_id: Some(record.source.owner_network_entity_id),
                        source_team: Some(record.source.team_id),
                        target_network_id: *target_network_id,
                        target_kind: if sentry_targets.contains(record.target) {
                            CombatTargetKind::Deployable
                        } else {
                            CombatTargetKind::Fighter
                        },
                        target_team: *target_team,
                        preset_id: record.source.source_preset_id,
                        recipe_fingerprint: Some(record.source.recipe_fingerprint),
                        position: WorldPoint::from(record.position),
                        engagement_distance: record.engagement_distance,
                        kind: if sentry_targets.contains(record.target) {
                            CombatOutcomeKind::DeployableDestroyed
                        } else {
                            CombatOutcomeKind::Defeat
                        },
                    });
                    if let Some(legacy_defeat_event) = legacy_defeat_event {
                        let legacy_cue = CombatCue::Defeat {
                            event_id: legacy_defeat_event,
                            tick: tick.0,
                            source: Some(source),
                            target: *target_network_id,
                        };
                        transaction.legacy_telemetry.record_cue(legacy_cue.clone());
                        transaction
                            .legacy_telemetry
                            .record(CombatLogRecord::Defeat {
                                tick: tick.0,
                                event_id: legacy_defeat_event,
                                source: Some(source),
                                target: *target_network_id,
                            });
                        transaction.outbox.0.push(legacy_cue);
                    }
                }
            }
        }
        if target_defeated {
            accumulated_effects.remove(&record.target);
            accumulated_motion.remove(&record.target);
            continue;
        }
        if sentry_targets.contains(record.target) {
            accumulated_effects.remove(&record.target);
            accumulated_motion.remove(&record.target);
            continue;
        }
        (effects_state, motion_state) = apply_runtime_effects(
            &record,
            tick.0,
            source,
            *target_network_id,
            *target_team,
            owner_contact,
            *health,
            preset_id,
            effects_state,
            motion_state,
            passive_access.p0().get(record.target).is_ok_and(|loadout| {
                loadout
                    .passives
                    .iter()
                    .any(|passive| passive.kind == crate::builds::PassiveKind::Tenacity)
            }),
            &mut reserved_events,
            &mut gameplay_telemetry.weapon,
            &mut gameplay_telemetry.ability,
            &mut deferred_effect_cues,
        );
        accumulated_effects.insert(record.target, effects_state);
        if let Some(motion) = motion_state {
            accumulated_motion.insert(record.target, motion);
        }
    }
    for (entity, effects) in accumulated_effects {
        if !defeated_this_tick.contains(&entity) {
            commands.entity(entity).insert(effects);
        }
    }
    for (entity, motion) in accumulated_motion {
        if !defeated_this_tick.contains(&entity) {
            commands.entity(entity).insert((
                motion,
                KnockbackFeedback {
                    velocity: WorldPoint::from(motion.velocity),
                    expires_at_tick: motion.expires_at_tick,
                },
            ));
        }
    }
    for (entity, cue) in deferred_effect_cues {
        if !defeated_this_tick.contains(&entity) {
            transaction.legacy_telemetry.record_cue(cue.clone());
            transaction.outbox.0.push(cue);
        }
    }
    for (attack_id, _) in resolved_delivery_keys {
        finish_attack_delivery(&mut trackers, attack_id);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn slow_keeps_strongest_magnitude_and_latest_expiry() {
        let mut effects = ActiveEffects::default();
        refresh_strongest_slow(&mut effects, AttackId(1), NetworkEntityId(1), 700, 20);
        refresh_strongest_slow(&mut effects, AttackId(2), NetworkEntityId(2), 800, 30);
        assert_eq!(effects.slow.unwrap().movement_multiplier_milli, 700);
        refresh_strongest_slow(&mut effects, AttackId(3), NetworkEntityId(3), 500, 25);
        assert_eq!(effects.slow.unwrap().movement_multiplier_milli, 500);
    }

    #[cfg(feature = "server")]
    #[test]
    fn close_quarters_damage_is_identical_in_reservation_and_application_math() {
        assert_eq!(
            requested_damage(90, DamageFalloff::None, 0.0, 1.0, true, 240.0),
            104
        );
        assert_eq!(
            requested_damage(90, DamageFalloff::None, 0.0, 1.0, false, 240.0),
            90
        );
        assert_eq!(
            requested_damage(
                25,
                DamageFalloff::Linear {
                    start_distance: 0.0,
                    end_distance: 100.0,
                    minimum_scale: 0.5,
                },
                100.0,
                1.0,
                true,
                240.0,
            ),
            14,
            "falloff and Close Quarters must compose before the single final rounding",
        );
    }

    #[cfg(feature = "server")]
    #[test]
    fn deployable_target_policy_is_explicit_for_every_m08_source_and_effect() {
        for source in [
            CombatSourceKind::PrimaryWeapon,
            CombatSourceKind::Ultimate {
                ultimate_id: crate::builds::UltimateDefinitionId(1),
            },
            CombatSourceKind::Deployable {
                ultimate_id: crate::builds::UltimateDefinitionId(2),
                deployable_id: crate::builds::DeployableId(1),
            },
        ] {
            assert!(combat_source_allows_target(
                source,
                CombatTargetKind::Deployable,
            ));
        }
        assert!(effect_allows_target(
            PayloadEffectDefinition::Damage {
                amount: 1,
                falloff: DamageFalloff::None,
                recipients: RecipientPolicy::Hostiles,
            },
            CombatTargetKind::Deployable,
        ));
        assert!(!effect_allows_target(
            PayloadEffectDefinition::Slow {
                movement_multiplier: 0.5,
                duration_ticks: 1,
                stacking: SlowStacking::StrongestRefreshes,
                recipients: RecipientPolicy::Hostiles,
            },
            CombatTargetKind::Deployable,
        ));
    }
}
