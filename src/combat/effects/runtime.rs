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
