//! Damage, defeat, slow, knockback, and passive runtime math.

#![allow(clippy::wildcard_imports)]
use super::*;
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
pub(super) fn effect_recipient_scale(
    effect: PayloadEffectDefinition,
    source: AttackSource,
    target_network_id: NetworkEntityId,
    target_team: TeamId,
) -> Option<f32> {
    let recipients = match effect {
        PayloadEffectDefinition::Damage { recipients, .. }
        | PayloadEffectDefinition::Knockback { recipients, .. }
        | PayloadEffectDefinition::Slow { recipients, .. }
        | PayloadEffectDefinition::Cold { recipients, .. }
        | PayloadEffectDefinition::DamageOverTime { recipients, .. }
        | PayloadEffectDefinition::Heal { recipients, .. } => recipients,
    };
    if target_network_id == source.owner_network_entity_id {
        match recipients {
            RecipientPolicy::HostilesAndOwner { owner_scale } => Some(owner_scale),
            RecipientPolicy::AlliesAndOwner => Some(1.0),
            RecipientPolicy::Hostiles | RecipientPolicy::Allies => None,
        }
    } else if teams_are_hostile(source.team_id, target_team) {
        matches!(
            recipients,
            RecipientPolicy::Hostiles | RecipientPolicy::HostilesAndOwner { .. }
        )
        .then_some(1.0)
    } else {
        matches!(
            recipients,
            RecipientPolicy::Allies | RecipientPolicy::AlliesAndOwner
        )
        .then_some(1.0)
    }
}

#[cfg(feature = "server")]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub(crate) fn requested_damage(
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
pub(super) type PlannedTarget = (NetworkEntityId, TeamId, u16, bool, CombatTargetKind);

#[cfg(feature = "server")]
pub(super) const fn combat_source_allows_target(
    source: CombatSourceKind,
    target: CombatTargetKind,
) -> bool {
    match (source, target) {
        (
            CombatSourceKind::PrimaryWeapon
            | CombatSourceKind::Environment
            | CombatSourceKind::Ultimate { .. }
            | CombatSourceKind::Deployable { .. },
            CombatTargetKind::Fighter | CombatTargetKind::Deployable,
        ) => true,
    }
}

#[cfg(feature = "server")]
pub(super) const fn effect_allows_target(
    effect: PayloadEffectDefinition,
    target: CombatTargetKind,
) -> bool {
    matches!(effect, PayloadEffectDefinition::Damage { .. })
        || matches!(target, CombatTargetKind::Fighter)
}

#[must_use]
pub(crate) fn apply_resistance(value: u16, resistance_basis_points: u16) -> u16 {
    if value == 0 {
        return 0;
    }
    let resistance = u32::from(resistance_basis_points.min(6_000));
    let numerator = u32::from(value)
        .saturating_mul(10_000_u32.saturating_sub(resistance))
        .saturating_add(5_000);
    u16::try_from(numerator / 10_000).unwrap_or(u16::MAX).max(1)
}

/// Applies one target-owned Cold contribution and returns whether it triggered Freeze.
pub(crate) fn apply_cold_contribution(
    cold: &mut ColdState,
    authored_amount: u16,
    resistance_basis_points: u16,
    capacity: u16,
    tick: u64,
    source: ConditionSource,
) -> bool {
    let capacity = capacity.max(1);
    let amount = apply_resistance(authored_amount, resistance_basis_points);
    cold.meter = cold.meter.saturating_add(amount).min(capacity);
    cold.last_contribution_tick = tick;
    cold.source = Some(source);
    if cold.meter < capacity {
        return false;
    }
    cold.meter = 0;
    cold.frozen_until_tick = Some(tick.saturating_add(conditions::FREEZE_TICKS));
    cold.immunity_until_tick = None;
    true
}

pub(crate) fn refresh_damage_over_time(
    slot: &mut Option<DamageOverTime>,
    source: ConditionSource,
    damage_per_tick: u16,
    tick_interval: u64,
    applied_at_tick: u64,
    duration_ticks: u64,
) {
    let next = DamageOverTime {
        source,
        damage_per_tick,
        tick_interval,
        next_tick: applied_at_tick.saturating_add(tick_interval),
        expires_at_tick: applied_at_tick.saturating_add(duration_ticks),
    };
    match slot {
        None => *slot = Some(next),
        Some(current) if damage_per_tick > current.damage_per_tick => *slot = Some(next),
        Some(current) if damage_per_tick == current.damage_per_tick => *current = next,
        Some(_) => {}
    }
}

#[cfg(feature = "server")]
pub(super) fn cue_damage_source(source: AttackSource) -> DamageSource {
    match source.kind {
        CombatSourceKind::PrimaryWeapon => DamageSource::PlayerWeapon {
            player_id: source.player_id,
            fighter_id: source.owner_network_entity_id,
            weapon_definition_id: WeaponDefinitionId(
                source.source_preset_id.map_or(0, |preset| preset.0),
            ),
            shot_id: ShotId(source.attack_id.0),
        },
        CombatSourceKind::Environment => DamageSource::Environment {
            map_instance_id: 0,
            generation: 0,
            placement_id: 0,
            initiating_player: Some(source.player_id),
            initiating_fighter: Some(source.owner_network_entity_id),
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
#[allow(clippy::too_many_lines)]
#[allow(clippy::too_many_arguments)]
pub(super) fn apply_runtime_effects(
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
    cold_capacity: u16,
    cold_resistance_basis_points: u16,
    poison_resistance_basis_points: u16,
    fire_resistance_basis_points: u16,
    allow_cold: bool,
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
            PayloadEffectDefinition::Damage { .. } | PayloadEffectDefinition::Heal { .. } => {}
            PayloadEffectDefinition::Cold { amount, .. } => {
                if !allow_cold {
                    continue;
                }
                if effects_state
                    .cold
                    .frozen_until_tick
                    .is_some_and(|deadline| tick < deadline)
                    || effects_state
                        .cold
                        .immunity_until_tick
                        .is_some_and(|deadline| tick < deadline)
                {
                    continue;
                }
                apply_cold_contribution(
                    &mut effects_state.cold,
                    amount,
                    cold_resistance_basis_points,
                    cold_capacity,
                    tick,
                    record.source.into(),
                );
                let event_id = reserved_events
                    .next()
                    .expect("payload event reservation matches Cold");
                deferred_effect_cues.push((
                    record.target,
                    CombatCue::EffectApplied {
                        event_id,
                        tick,
                        attack_id: record.source.attack_id,
                        source,
                        target: target_network_id,
                        position: WorldPoint::from(record.position),
                        effect: CombatEffectCue::Cold {
                            meter: effects_state.cold.meter,
                            frozen_until_tick: effects_state.cold.frozen_until_tick,
                        },
                        presentation_profile_id: record.source.presentation_profile_id,
                    },
                ));
            }
            PayloadEffectDefinition::DamageOverTime {
                kind,
                damage_per_tick,
                tick_interval,
                duration_ticks,
                ..
            } => {
                let resistance = match kind {
                    DamageOverTimeKind::Poison => poison_resistance_basis_points,
                    DamageOverTimeKind::Fire => fire_resistance_basis_points,
                };
                let damage_per_tick = apply_resistance(damage_per_tick, resistance);
                let slot = match kind {
                    DamageOverTimeKind::Poison => &mut effects_state.poison,
                    DamageOverTimeKind::Fire => &mut effects_state.fire,
                };
                refresh_damage_over_time(
                    slot,
                    record.source.into(),
                    damage_per_tick,
                    tick_interval,
                    tick,
                    duration_ticks,
                );
                let event_id = reserved_events
                    .next()
                    .expect("payload event reservation matches damage over time");
                deferred_effect_cues.push((
                    record.target,
                    CombatCue::EffectApplied {
                        event_id,
                        tick,
                        attack_id: record.source.attack_id,
                        source,
                        target: target_network_id,
                        position: WorldPoint::from(record.position),
                        effect: CombatEffectCue::DamageOverTime {
                            kind,
                            damage_per_tick,
                            expires_at_tick: tick.saturating_add(duration_ticks),
                        },
                        presentation_profile_id: record.source.presentation_profile_id,
                    },
                ));
            }
        }
    }
    (effects_state, motion_state)
}

#[cfg(test)]
mod elemental_tests {
    use super::*;

    fn source(action: u64) -> ConditionSource {
        ConditionSource {
            action_id: AttackId(action),
            kind: CombatSourceKind::PrimaryWeapon,
            player_id: PlayerId(1),
            network_entity_id: NetworkEntityId(1),
            team_id: TeamId(0),
            source_preset_id: Some(WeaponPresetId(1)),
            recipe_fingerprint: None,
            presentation_profile_id: None,
        }
    }

    #[test]
    fn resistance_reduces_only_the_applied_contribution() {
        assert_eq!(apply_resistance(100, 0), 100);
        assert_eq!(apply_resistance(100, 3_000), 70);
        assert_eq!(apply_resistance(1, 6_000), 1);
    }

    #[test]
    fn cold_uses_the_targets_capacity_after_resistance() {
        let mut cold = ColdState::default();
        assert!(!apply_cold_contribution(
            &mut cold,
            125,
            0,
            250,
            10,
            source(1),
        ));
        assert_eq!(cold.meter, 125);
        assert!(apply_cold_contribution(
            &mut cold,
            125,
            0,
            250,
            20,
            source(2),
        ));
        assert_eq!(cold.meter, 0);
        assert_eq!(cold.frozen_until_tick, Some(20 + conditions::FREEZE_TICKS));

        let mut resistant = ColdState::default();
        assert!(!apply_cold_contribution(
            &mut resistant,
            125,
            3_000,
            100,
            30,
            source(3),
        ));
        assert_eq!(resistant.meter, 88);
    }

    #[test]
    fn damage_over_time_refresh_keeps_the_stronger_condition() {
        let mut slot = None;
        refresh_damage_over_time(&mut slot, source(1), 10, 30, 5, 120);
        refresh_damage_over_time(&mut slot, source(2), 8, 10, 20, 300);
        assert_eq!(slot.expect("condition").source.action_id, AttackId(1));

        refresh_damage_over_time(&mut slot, source(3), 10, 30, 20, 120);
        let refreshed = slot.expect("condition");
        assert_eq!(refreshed.source.action_id, AttackId(3));
        assert_eq!(refreshed.next_tick, 50);
        assert_eq!(refreshed.expires_at_tick, 140);
    }
}
