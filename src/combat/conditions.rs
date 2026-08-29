//! Bounded target-owned Cold, Freeze, Poison, and Fire lifecycle.

#![allow(
    clippy::wildcard_imports,
    reason = "the server-only condition transaction consumes the combat composition surface"
)]

use super::*;
use avian2d::prelude::{CollisionLayers, Position};
use bevy::prelude::*;

pub(crate) const COLD_DECAY_DELAY_TICKS: u64 = 90;
pub(crate) const COLD_DECAY_PER_TICK: u16 = 10;
pub(crate) const FREEZE_TICKS: u64 = 60;
pub(crate) const THAW_IMMUNITY_TICKS: u64 = 90;

#[must_use]
pub(crate) fn condition_damage_source(source: ConditionSource) -> DamageSource {
    match source.kind {
        CombatSourceKind::PrimaryWeapon => DamageSource::PlayerWeapon {
            player_id: source.player_id,
            fighter_id: source.network_entity_id,
            weapon_definition_id: WeaponDefinitionId(
                source.source_preset_id.map_or(0, |preset| preset.0),
            ),
            shot_id: ShotId(source.action_id.0),
        },
        CombatSourceKind::Ultimate { ultimate_id } => DamageSource::Ultimate {
            player_id: source.player_id,
            fighter_id: source.network_entity_id,
            ultimate_id,
            attack_id: source.action_id,
        },
        CombatSourceKind::Deployable {
            ultimate_id,
            deployable_id,
        } => DamageSource::Deployable {
            player_id: source.player_id,
            fighter_id: source.network_entity_id,
            ultimate_id,
            deployable_id,
            attack_id: source.action_id,
        },
        CombatSourceKind::Environment => DamageSource::Environment {
            map_instance_id: 0,
            generation: 0,
            placement_id: 0,
            initiating_player: Some(source.player_id),
            initiating_fighter: Some(source.network_entity_id),
        },
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    reason = "condition lifecycle declares the complete authoritative target and outcome view"
)]
pub(crate) fn advance_conditions(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    mut ids: ResMut<NextCombatIds>,
    mut facts: ResMut<CombatOutcomeFacts>,
    mut outbox: ResMut<CombatOutbox>,
    mut telemetry: ResMut<CombatTelemetry>,
    mut weapon_telemetry: ResMut<WeaponTelemetry>,
    mut fighters: Query<
        (
            Entity,
            &NetworkEntityId,
            &TeamId,
            &Position,
            &mut CurrentHealth,
            &mut ActiveEffects,
            &mut crate::builds::AbilityState,
            Has<TestDummy>,
        ),
        (With<Fighter>, Without<Defeated>),
    >,
) {
    let mut ordered: Vec<_> = fighters.iter_mut().collect();
    ordered.sort_by_key(|(_, network_id, ..)| **network_id);
    for (entity, network_id, team, position, mut health, mut effects, mut ability, test_dummy) in
        ordered
    {
        let mut defeated = false;
        for kind in [DamageOverTimeKind::Poison, DamageOverTimeKind::Fire] {
            let slot = match kind {
                DamageOverTimeKind::Poison => &mut effects.poison,
                DamageOverTimeKind::Fire => &mut effects.fire,
            };
            let Some(mut condition) = *slot else { continue };
            while tick.0 >= condition.next_tick && condition.next_tick <= condition.expires_at_tick
            {
                let defeats = condition.damage_per_tick >= health.0;
                let required = 1 + usize::from(defeats);
                let Some(events) = server::reserve_event_ids(&mut ids, required) else {
                    break;
                };
                let damage_event = events[0];
                let applied = condition.damage_per_tick.min(health.0);
                health.0 = health.0.saturating_sub(applied);
                let source = condition.source;
                let cue_source = condition_damage_source(source);
                let presentation_profile_id = source
                    .presentation_profile_id
                    .unwrap_or(WeaponPresentationProfileId(1));
                let cue = CombatCue::DamageApplied {
                    event_id: damage_event,
                    tick: tick.0,
                    attack_id: source.action_id,
                    source: cue_source,
                    target: *network_id,
                    position: position.0.into(),
                    amount: applied,
                    health_after: health.0,
                    distance_band: DistanceBand::Close,
                    presentation_profile_id,
                };
                telemetry.applied_damage =
                    telemetry.applied_damage.saturating_add(u64::from(applied));
                telemetry.record_cue(cue.clone());
                outbox.0.push(cue);
                facts.0.push(CombatOutcomeFact {
                    event_id: damage_event,
                    tick: tick.0,
                    attack_id: source.action_id,
                    source_kind: source.kind,
                    source_player: Some(source.player_id),
                    source_network_id: Some(source.network_entity_id),
                    source_team: Some(source.team_id),
                    target_network_id: *network_id,
                    target_kind: CombatTargetKind::Fighter,
                    target_team: *team,
                    preset_id: source.source_preset_id,
                    recipe_fingerprint: source.recipe_fingerprint,
                    position: position.0.into(),
                    engagement_distance: 0.0,
                    kind: CombatOutcomeKind::Damage { amount: applied },
                });
                weapon_telemetry.record(WeaponTelemetryRecord {
                    tick: tick.0,
                    event_id: damage_event,
                    attack_id: source.action_id,
                    preset_id: source.source_preset_id.unwrap_or(WeaponPresetId(0)),
                    recipe_fingerprint: source.recipe_fingerprint.unwrap_or_default(),
                    delivery_index: None,
                    source: source.network_entity_id,
                    target: Some(*network_id),
                    position: position.0.into(),
                    requested_value: condition.damage_per_tick,
                    applied_value: applied,
                    engagement_distance: 0.0,
                    delivery_travel: 0.0,
                    hostile_contact: source.team_id != *team,
                    effect: Some(PayloadEffectDefinition::DamageOverTime {
                        kind,
                        damage_per_tick: condition.damage_per_tick,
                        tick_interval: condition.tick_interval,
                        duration_ticks: condition
                            .expires_at_tick
                            .saturating_sub(condition.next_tick),
                        recipients: RecipientPolicy::Hostiles,
                    }),
                    resulting_health: Some(health.0),
                    resulting_effects: None,
                    resulting_motion: None,
                    outcome: WeaponTelemetryOutcome::DamageApplied,
                });
                condition.next_tick = condition.next_tick.saturating_add(condition.tick_interval);
                if defeats {
                    let defeat_event = events[1];
                    defeated = true;
                    telemetry.defeats = telemetry.defeats.saturating_add(1);
                    let cue = CombatCue::FighterDefeated {
                        event_id: defeat_event,
                        tick: tick.0,
                        attack_id: source.action_id,
                        source: Some(cue_source),
                        target: *network_id,
                        position: position.0.into(),
                        presentation_profile_id: source.presentation_profile_id,
                    };
                    telemetry.record_cue(cue.clone());
                    outbox.0.push(cue);
                    facts.0.push(CombatOutcomeFact {
                        event_id: defeat_event,
                        tick: tick.0,
                        attack_id: source.action_id,
                        source_kind: source.kind,
                        source_player: Some(source.player_id),
                        source_network_id: Some(source.network_entity_id),
                        source_team: Some(source.team_id),
                        target_network_id: *network_id,
                        target_kind: CombatTargetKind::Fighter,
                        target_team: *team,
                        preset_id: source.source_preset_id,
                        recipe_fingerprint: source.recipe_fingerprint,
                        position: position.0.into(),
                        engagement_distance: 0.0,
                        kind: CombatOutcomeKind::Defeat,
                    });
                    commands
                        .entity(entity)
                        .insert((
                            Defeated {
                                event_id: defeat_event,
                            },
                            CollisionLayers::new(FIGHTER_LAYER, avian2d::prelude::LayerMask::NONE),
                            ActiveEffects::default(),
                        ))
                        .remove::<ExternalMotion>()
                        .remove::<KnockbackFeedback>()
                        .remove::<crate::abilities::DashRuntime>();
                    if test_dummy {
                        commands
                            .entity(entity)
                            .insert(TestDummyResetDeadline(tick.0.saturating_add(90)));
                    }
                    break;
                }
            }
            if defeated {
                break;
            }
            if tick.0 >= condition.expires_at_tick
                || condition.next_tick > condition.expires_at_tick
            {
                *slot = None;
            } else {
                *slot = Some(condition);
            }
        }
        if defeated {
            continue;
        }

        if effects.is_frozen(tick.0) {
            if matches!(ability.phase, crate::builds::AbilityPhase::Dashing { .. }) {
                ability.phase = crate::abilities::settled_ability_phase(ability.charge);
                commands
                    .entity(entity)
                    .remove::<crate::abilities::DashRuntime>();
            }
        } else if effects
            .cold
            .frozen_until_tick
            .is_some_and(|deadline| tick.0 >= deadline)
        {
            effects.cold.frozen_until_tick = None;
            effects.cold.immunity_until_tick = Some(tick.0.saturating_add(THAW_IMMUNITY_TICKS));
            effects.cold.source = None;
        }
        if effects
            .cold
            .immunity_until_tick
            .is_some_and(|deadline| tick.0 >= deadline)
        {
            effects.cold.immunity_until_tick = None;
        }
        if effects.cold.meter != 0
            && tick.0
                >= effects
                    .cold
                    .last_contribution_tick
                    .saturating_add(COLD_DECAY_DELAY_TICKS)
        {
            effects.cold.meter = effects.cold.meter.saturating_sub(COLD_DECAY_PER_TICK);
            if effects.cold.meter == 0 {
                effects.cold.source = None;
            }
        }
    }
}
