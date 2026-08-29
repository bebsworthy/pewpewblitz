//! Bounded server-owned elemental ultimate fields.

#![allow(
    clippy::wildcard_imports,
    reason = "the server-only field transaction consumes the combat composition surface"
)]

use super::*;
use avian2d::prelude::Position;
use bevy::prelude::*;

pub(crate) const MAX_ACTIVE_ELEMENTAL_FIELDS: usize = 6;

#[must_use]
pub(crate) const fn field_payload(
    effect: crate::builds::ElementalFieldEffect,
) -> PayloadEffectDefinition {
    match effect {
        crate::builds::ElementalFieldEffect::Cold { amount } => PayloadEffectDefinition::Cold {
            amount,
            recipients: RecipientPolicy::Hostiles,
        },
        crate::builds::ElementalFieldEffect::DamageOverTime {
            kind,
            damage_per_tick,
            tick_interval,
            duration_ticks,
        } => PayloadEffectDefinition::DamageOverTime {
            kind,
            damage_per_tick,
            tick_interval,
            duration_ticks,
            recipients: RecipientPolicy::Hostiles,
        },
        crate::builds::ElementalFieldEffect::Heal { amount } => PayloadEffectDefinition::Heal {
            amount,
            recipients: RecipientPolicy::AlliesAndOwner,
        },
    }
}

#[derive(Resource, Default)]
pub(crate) struct NextElementalFieldId(pub u64);

impl NextElementalFieldId {
    pub(crate) fn allocate(&mut self) -> Option<ElementalFieldId> {
        let next = self.0.checked_add(1)?;
        self.0 = next;
        Some(ElementalFieldId(next))
    }
}

#[must_use]
pub(crate) const fn field_kind_for_ultimate(
    kind: crate::builds::UltimateKind,
) -> Option<ElementalFieldKind> {
    match kind {
        crate::builds::UltimateKind::CryogenicField => Some(ElementalFieldKind::Cryogenic),
        crate::builds::UltimateKind::FireField => Some(ElementalFieldKind::Fire),
        crate::builds::UltimateKind::PoisonField => Some(ElementalFieldKind::Poison),
        crate::builds::UltimateKind::RestorationField => Some(ElementalFieldKind::Restoration),
        _ => None,
    }
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::type_complexity,
    reason = "field pulses declare their complete authoritative field, target, and outcome views"
)]
pub(crate) fn pulse_and_cleanup_elemental_fields(
    mut commands: Commands,
    tick: Res<SimulationTick>,
    condition_rules: Res<CombatConditionRulesResource>,
    mut ids: ResMut<NextCombatIds>,
    roots: Query<&crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>,
    mut fields: Query<(Entity, &mut ElementalFieldState, &ElementalFieldRuntime)>,
    mut fighters: Query<
        (
            Entity,
            &NetworkEntityId,
            &TeamId,
            &Position,
            &crate::builds::ResolvedMatchLoadout,
            &mut CurrentHealth,
            &mut ActiveEffects,
            Has<crate::matchplay::SpawnProtection>,
            Option<&crate::map::EffectTileOccupancy>,
        ),
        (With<Fighter>, Without<Defeated>),
    >,
    mut owners: Query<(
        &NetworkEntityId,
        &crate::builds::ResolvedMatchLoadout,
        &mut crate::builds::AbilityState,
    )>,
    mut facts: ResMut<CombatOutcomeFacts>,
    mut outbox: ResMut<CombatOutbox>,
    mut telemetry: ResMut<CombatTelemetry>,
) {
    let root = roots.single().ok();
    let mut target_order: Vec<_> = fighters
        .iter_mut()
        .map(|(entity, network_id, team, position, ..)| (entity, *network_id, *team, position.0))
        .collect();
    target_order.sort_by_key(|(_, network_id, ..)| *network_id);
    let mut field_order: Vec<_> = fields.iter_mut().collect();
    field_order.sort_by_key(|(_, state, _)| state.id);

    for (field_entity, mut state, runtime) in field_order {
        let owner_replaced = owners
            .iter_mut()
            .find(|(network_id, ..)| **network_id == state.owner_network_entity_id)
            .is_some_and(|(_, loadout, _)| {
                field_kind_for_ultimate(loadout.ultimate.kind) != Some(state.kind)
            });
        let match_invalid = root.is_none_or(|root| {
            root.match_id != runtime.match_id
                || matches!(root.phase, crate::matchplay::MatchPhase::Completed { .. })
        });
        let expired = tick.0 > state.expires_at_tick;
        if owner_replaced || match_invalid || expired {
            settle_field_owner(&mut owners, state.id, state.owner_network_entity_id);
            commands.entity(field_entity).despawn();
            continue;
        }

        while tick.0 >= state.next_pulse_tick && state.next_pulse_tick <= state.expires_at_tick {
            let pulse_tick = state.next_pulse_tick;
            let radius_squared = state.radius().map_or(0.0, |radius| radius * radius);
            for (target_entity, _, _, target_position) in &target_order {
                if target_position.distance_squared(state.center_vec2()) > radius_squared {
                    continue;
                }
                let Ok((
                    _,
                    target_network_id,
                    target_team,
                    position,
                    loadout,
                    mut health,
                    mut effects,
                    protected,
                    effect_tile,
                )) = fighters.get_mut(*target_entity)
                else {
                    continue;
                };
                let hostile = teams_are_hostile(state.team_id, *target_team);
                let owner = *target_network_id == state.owner_network_entity_id;
                let eligible = match runtime.effect {
                    PayloadEffectDefinition::Cold { .. }
                    | PayloadEffectDefinition::DamageOverTime { .. } => hostile && !protected,
                    PayloadEffectDefinition::Heal { .. } => {
                        (!hostile || owner)
                            && !effect_tile
                                .is_some_and(crate::map::EffectTileOccupancy::blocks_healing)
                    }
                    _ => false,
                };
                if !eligible {
                    continue;
                }
                let Some(event_id) = ids.allocate_event() else {
                    continue;
                };
                let cue_source = conditions::condition_damage_source(runtime.source);
                let effect_cue = match runtime.effect {
                    PayloadEffectDefinition::Cold { amount, .. } => {
                        if effects.is_frozen(pulse_tick)
                            || effects
                                .cold
                                .immunity_until_tick
                                .is_some_and(|deadline| pulse_tick < deadline)
                        {
                            continue;
                        }
                        effects::apply_cold_contribution(
                            &mut effects.cold,
                            amount,
                            loadout.fighter_stats.cold_resistance_basis_points,
                            loadout.fighter_stats.cold_capacity,
                            condition_rules.0.freeze_duration_ticks,
                            pulse_tick,
                            runtime.source,
                        );
                        CombatEffectCue::Cold {
                            meter: effects.cold.meter,
                            frozen_until_tick: effects.cold.frozen_until_tick,
                        }
                    }
                    PayloadEffectDefinition::DamageOverTime {
                        kind,
                        damage_per_tick,
                        tick_interval,
                        duration_ticks,
                        ..
                    } => {
                        let resistance = match kind {
                            DamageOverTimeKind::Poison => {
                                loadout.fighter_stats.poison_resistance_basis_points
                            }
                            DamageOverTimeKind::Fire => {
                                loadout.fighter_stats.fire_resistance_basis_points
                            }
                        };
                        let damage_per_tick =
                            effects::apply_resistance(damage_per_tick, resistance);
                        let slot = match kind {
                            DamageOverTimeKind::Poison => &mut effects.poison,
                            DamageOverTimeKind::Fire => &mut effects.fire,
                        };
                        effects::refresh_damage_over_time(
                            slot,
                            runtime.source,
                            damage_per_tick,
                            tick_interval,
                            pulse_tick,
                            duration_ticks,
                        );
                        CombatEffectCue::DamageOverTime {
                            kind,
                            damage_per_tick,
                            expires_at_tick: pulse_tick.saturating_add(duration_ticks),
                        }
                    }
                    PayloadEffectDefinition::Heal { amount, .. } => {
                        let requested = amount;
                        let applied = requested.min(
                            loadout
                                .fighter_stats
                                .maximum_health
                                .saturating_sub(health.0),
                        );
                        health.0 = health
                            .0
                            .saturating_add(applied)
                            .min(loadout.fighter_stats.maximum_health);
                        facts.0.push(CombatOutcomeFact {
                            event_id,
                            tick: pulse_tick,
                            attack_id: runtime.source.action_id,
                            source_kind: runtime.source.kind,
                            source_player: Some(runtime.source.player_id),
                            source_network_id: Some(runtime.source.network_entity_id),
                            source_team: Some(runtime.source.team_id),
                            target_network_id: *target_network_id,
                            target_kind: CombatTargetKind::Fighter,
                            target_team: *target_team,
                            preset_id: None,
                            recipe_fingerprint: None,
                            position: position.0.into(),
                            engagement_distance: state.center_vec2().distance(position.0),
                            kind: CombatOutcomeKind::Healing {
                                requested,
                                applied,
                                resulting_health: health.0,
                            },
                        });
                        CombatEffectCue::Healing {
                            amount: applied,
                            health_after: health.0,
                        }
                    }
                    _ => continue,
                };
                let cue = CombatCue::EffectApplied {
                    event_id,
                    tick: pulse_tick,
                    attack_id: runtime.source.action_id,
                    source: cue_source,
                    target: *target_network_id,
                    position: position.0.into(),
                    effect: effect_cue,
                    presentation_profile_id: WeaponPresentationProfileId(1),
                };
                telemetry.record_cue(cue.clone());
                outbox.0.push(cue);
            }
            state.next_pulse_tick = state
                .next_pulse_tick
                .saturating_add(runtime.pulse_interval_ticks);
        }

        if tick.0 >= state.expires_at_tick {
            settle_field_owner(&mut owners, state.id, state.owner_network_entity_id);
            commands.entity(field_entity).despawn();
        }
    }
}

fn settle_field_owner(
    owners: &mut Query<(
        &NetworkEntityId,
        &crate::builds::ResolvedMatchLoadout,
        &mut crate::builds::AbilityState,
    )>,
    field_id: ElementalFieldId,
    owner_network_id: NetworkEntityId,
) {
    if let Some((_, loadout, mut ability)) = owners
        .iter_mut()
        .find(|(network_id, ..)| **network_id == owner_network_id)
        && matches!(
            ability.phase,
            crate::builds::AbilityPhase::ElementalFieldActive { field_id: active, .. }
                if active == field_id
        )
    {
        ability.phase = crate::abilities::settled_ability_phase(
            ability.charge,
            loadout.ultimate.charge_policy.maximum,
        );
    }
}
