#[cfg(feature = "server")]
use bevy::prelude::*;

#[cfg(feature = "server")]
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct UltimateGeneration(pub u64);

#[cfg(feature = "server")]
#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "the fixed-tick activation coordinator consumes Bevy system parameters"
)]
pub(crate) fn activate_self_cloak(
    mut commands: Commands,
    tick: Res<crate::timing::SimulationTick>,
    mut ids: ResMut<crate::combat::NextCombatIds>,
    mut outbox: ResMut<crate::combat::CombatOutbox>,
    mut telemetry: ResMut<crate::abilities::AbilityTelemetry>,
    mut fighters: Query<
        (
            Entity,
            &crate::builds::ResolvedMatchLoadout,
            &crate::protocol::NetworkEntityId,
            &crate::movement::InputFreshness,
            &mut crate::builds::AbilityState,
            Option<&lightyear::prelude::input::native::ActionState<crate::protocol::FighterInput>>,
            Option<&mut crate::abilities::UltimateInputLatch>,
            Option<&mut UltimateGeneration>,
            Has<crate::combat::Defeated>,
            Has<crate::matchplay::ActiveCombatant>,
            Has<crate::combat::AwaitingPostSelectionInput>,
            Has<crate::concealment::ObjectiveCarrier>,
        ),
        With<crate::protocol::Fighter>,
    >,
) {
    for (
        entity,
        loadout,
        network_id,
        freshness,
        mut ability,
        action,
        latch,
        generation,
        defeated,
        active,
        barrier,
        objective_carrier,
    ) in &mut fighters
    {
        if loadout.ultimate.kind != crate::builds::UltimateKind::SelfCloak {
            continue;
        }
        let requested = action.is_some_and(|action| {
            action.0.is_valid()
                && action.0.gameplay_buttons & crate::protocol::FighterInput::ULTIMATE != 0
        });
        let was_held = latch.as_deref().is_some_and(|latch| latch.0);
        if let Some(mut latch) = latch {
            latch.0 = requested;
        } else {
            commands
                .entity(entity)
                .insert(crate::abilities::UltimateInputLatch(requested));
        }
        if !requested || was_held {
            continue;
        }
        telemetry.record(crate::abilities::AbilityTelemetryRecord {
            tick: tick.0,
            owner_network_id: *network_id,
            kind: crate::abilities::AbilityTelemetryKind::ActivationAttempt,
        });
        let held = !barrier
            && !crate::movement::input_should_neutralize(tick.0, freshness.last_fresh_tick, 12);
        let rejection = if !held {
            Some(crate::abilities::AbilityRejectionReason::StaleInput)
        } else if defeated {
            Some(crate::abilities::AbilityRejectionReason::Defeated)
        } else if !active {
            Some(crate::abilities::AbilityRejectionReason::Inactive)
        } else if objective_carrier {
            Some(crate::abilities::AbilityRejectionReason::ObjectiveCarrier)
        } else if ability.charge != crate::abilities::ULTIMATE_CHARGE_MAX
            || !matches!(ability.phase, crate::builds::AbilityPhase::Ready)
        {
            Some(crate::abilities::AbilityRejectionReason::NotCharged)
        } else {
            None
        };
        if let Some(reason) = rejection {
            telemetry.record(crate::abilities::AbilityTelemetryRecord {
                tick: tick.0,
                owner_network_id: *network_id,
                kind: crate::abilities::AbilityTelemetryKind::ActivationRejected(reason),
            });
            continue;
        }
        let crate::builds::UltimateParameters::SelfCloak { duration_ticks } =
            loadout.ultimate.parameters
        else {
            continue;
        };
        let next_generation = generation
            .as_deref()
            .map_or(Some(1), |value| value.0.checked_add(1));
        let Some(next_generation) = next_generation else {
            telemetry.record(crate::abilities::AbilityTelemetryRecord {
                tick: tick.0,
                owner_network_id: *network_id,
                kind: crate::abilities::AbilityTelemetryKind::ActivationRejected(
                    crate::abilities::AbilityRejectionReason::IdentifierExhausted,
                ),
            });
            continue;
        };
        let Some(event_id) = ids.allocate_event() else {
            telemetry.record(crate::abilities::AbilityTelemetryRecord {
                tick: tick.0,
                owner_network_id: *network_id,
                kind: crate::abilities::AbilityTelemetryKind::ActivationRejected(
                    crate::abilities::AbilityRejectionReason::IdentifierExhausted,
                ),
            });
            continue;
        };
        if let Some(mut generation) = generation {
            generation.0 = next_generation;
        } else {
            commands
                .entity(entity)
                .insert(UltimateGeneration(next_generation));
        }
        let expires_at_tick = tick.0.saturating_add(duration_ticks);
        *ability = crate::builds::AbilityState {
            charge: 0,
            phase: crate::builds::AbilityPhase::Cloaked {
                generation: next_generation,
                activated_at_tick: tick.0,
                expires_at_tick,
            },
        };
        commands
            .entity(entity)
            .remove::<crate::matchplay::SpawnProtection>();
        outbox.0.push(crate::combat::CombatCue::SelfCloakActivated {
            event_id,
            tick: tick.0,
            source: *network_id,
            generation: next_generation,
            expires_at_tick,
        });
        telemetry.record(crate::abilities::AbilityTelemetryRecord {
            tick: tick.0,
            owner_network_id: *network_id,
            kind: crate::abilities::AbilityTelemetryKind::SelfCloakAccepted,
        });
    }
}

#[cfg(feature = "server")]
#[allow(
    clippy::needless_pass_by_value,
    reason = "the fixed-post lifecycle coordinator consumes Bevy system parameters"
)]
pub(crate) fn resolve_self_cloak_lifecycle(
    tick: Res<crate::timing::SimulationTick>,
    outcomes: Res<crate::combat::CombatOutcomeFacts>,
    mut ids: ResMut<crate::combat::NextCombatIds>,
    mut outbox: ResMut<crate::combat::CombatOutbox>,
    mut telemetry: ResMut<crate::abilities::AbilityTelemetry>,
    mut fighters: Query<
        (
            &crate::protocol::NetworkEntityId,
            &mut crate::builds::AbilityState,
            Has<crate::combat::Defeated>,
        ),
        With<crate::protocol::Fighter>,
    >,
) {
    for (network_id, mut ability, defeated) in &mut fighters {
        let crate::builds::AbilityPhase::Cloaked {
            generation,
            activated_at_tick,
            expires_at_tick,
        } = ability.phase
        else {
            continue;
        };
        let attacked = outbox.0.iter().any(|cue| matches!(cue, crate::combat::CombatCue::AttackAccepted { source, .. } if source == network_id));
        let damaged = outcomes.0.iter().any(|fact| fact.target_network_id == *network_id && matches!(fact.kind, crate::combat::CombatOutcomeKind::Damage { amount } if amount > 0));
        let reason = if attacked {
            Some(crate::combat::SelfCloakEndReason::Attack)
        } else if damaged {
            Some(crate::combat::SelfCloakEndReason::Damage)
        } else if defeated {
            Some(crate::combat::SelfCloakEndReason::Defeated)
        } else if tick.0 >= expires_at_tick {
            Some(crate::combat::SelfCloakEndReason::Expired)
        } else {
            None
        };
        let Some(reason) = reason else {
            continue;
        };
        *ability = crate::builds::AbilityState {
            charge: 0,
            phase: crate::builds::AbilityPhase::Charging,
        };
        if let Some(event_id) = ids.allocate_event() {
            outbox.0.push(crate::combat::CombatCue::SelfCloakEnded {
                event_id,
                tick: tick.0,
                source: *network_id,
                generation,
                reason,
            });
        }
        telemetry.record(crate::abilities::AbilityTelemetryRecord {
            tick: tick.0,
            owner_network_id: *network_id,
            kind: crate::abilities::AbilityTelemetryKind::SelfCloakEnded {
                reason,
                active_ticks: tick.0.saturating_sub(activated_at_tick),
            },
        });
    }
}
