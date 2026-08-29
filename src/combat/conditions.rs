//! Bounded target-owned Cold, Freeze, Poison, and Fire lifecycle.

#![allow(
    clippy::wildcard_imports,
    reason = "the server-only condition transaction consumes the combat composition surface"
)]

use super::*;
use avian2d::prelude::{CollisionLayers, Position};
use bevy::prelude::*;

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

pub(crate) fn advance_cold_lifecycle(cold: &mut ColdState, tick: u64, rules: CombatConditionRules) {
    if cold
        .frozen_until_tick
        .is_some_and(|deadline| tick >= deadline)
    {
        cold.frozen_until_tick = None;
        cold.immunity_until_tick = Some(tick.saturating_add(rules.thaw_immunity_ticks));
        cold.source = None;
    }
    if cold
        .immunity_until_tick
        .is_some_and(|deadline| tick >= deadline)
    {
        cold.immunity_until_tick = None;
    }
    if cold.meter != 0
        && tick
            >= cold
                .last_contribution_tick
                .saturating_add(rules.cold_decay_delay_ticks)
    {
        cold.meter = cold.meter.saturating_sub(rules.cold_decay_per_tick);
        if cold.meter == 0 {
            cold.source = None;
        }
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
    condition_rules: Res<CombatConditionRulesResource>,
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

        if effects.is_frozen(tick.0)
            && matches!(ability.phase, crate::builds::AbilityPhase::Dashing { .. })
        {
            ability.phase = crate::abilities::settled_ability_phase(ability.charge);
            commands
                .entity(entity)
                .remove::<crate::abilities::DashRuntime>();
        }
        advance_cold_lifecycle(&mut effects.cold, tick.0, condition_rules.0);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn field_source() -> ConditionSource {
        ConditionSource {
            action_id: AttackId(7),
            kind: CombatSourceKind::Ultimate {
                ultimate_id: crate::builds::UltimateDefinitionId(8),
            },
            player_id: PlayerId(1),
            network_entity_id: NetworkEntityId(1),
            team_id: TeamId(0),
            source_preset_id: None,
            recipe_fingerprint: None,
            presentation_profile_id: None,
        }
    }

    fn refresh_due_fire(mut fighters: Query<&mut ActiveEffects, With<Fighter>>) {
        let mut effects = fighters.single_mut().expect("one test fighter");
        effects::refresh_damage_over_time(&mut effects.fire, field_source(), 18, 30, 30, 90);
    }

    #[test]
    fn authored_rules_drive_decay_thaw_and_immunity() {
        let mut rules = CombatConditionRules::embedded().unwrap();
        rules.cold_decay_delay_ticks = 5;
        rules.cold_decay_per_tick = 40;
        rules.thaw_immunity_ticks = 7;

        let mut cold = ColdState {
            meter: 100,
            last_contribution_tick: 10,
            frozen_until_tick: Some(20),
            immunity_until_tick: None,
            source: None,
        };
        advance_cold_lifecycle(&mut cold, 14, rules);
        assert_eq!(cold.meter, 100);
        assert_eq!(cold.frozen_until_tick, Some(20));

        advance_cold_lifecycle(&mut cold, 20, rules);
        assert_eq!(cold.meter, 60);
        assert_eq!(cold.frozen_until_tick, None);
        assert_eq!(cold.immunity_until_tick, Some(27));

        advance_cold_lifecycle(&mut cold, 27, rules);
        assert_eq!(cold.meter, 20);
        assert_eq!(cold.immunity_until_tick, None);
        advance_cold_lifecycle(&mut cold, 28, rules);
        assert_eq!(cold.meter, 0);
    }

    #[test]
    fn due_fire_field_refresh_does_not_prevent_condition_damage() {
        let mut app = App::new();
        app.insert_resource(SimulationTick(30))
            .init_resource::<CombatConditionRulesResource>()
            .init_resource::<NextCombatIds>()
            .init_resource::<CombatOutcomeFacts>()
            .init_resource::<CombatOutbox>()
            .init_resource::<CombatTelemetry>()
            .init_resource::<WeaponTelemetry>()
            .add_systems(Update, (refresh_due_fire, advance_conditions).chain());
        let fighter = app
            .world_mut()
            .spawn((
                Fighter,
                NetworkEntityId(2),
                TeamId(1),
                Position(Vec2::ZERO),
                CurrentHealth(100),
                ActiveEffects {
                    fire: Some(DamageOverTime {
                        source: field_source(),
                        damage_per_tick: 18,
                        tick_interval: 30,
                        next_tick: 30,
                        expires_at_tick: 90,
                    }),
                    ..default()
                },
                crate::builds::AbilityState::default(),
            ))
            .id();

        app.update();

        assert_eq!(
            app.world().get::<CurrentHealth>(fighter),
            Some(&CurrentHealth(82))
        );
        let fire = app
            .world()
            .get::<ActiveEffects>(fighter)
            .and_then(|effects| effects.fire)
            .expect("Fire remains active after its due tick");
        assert_eq!(fire.next_tick, 60);
        assert_eq!(fire.expires_at_tick, 120);
    }
}
