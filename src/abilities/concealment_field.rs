#[cfg(feature = "server")]
use bevy::prelude::*;

#[cfg(feature = "server")]
#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "the fixed-tick activation coordinator consumes Bevy system parameters"
)]
pub(crate) fn activate_concealment_field(
    mut commands: Commands,
    tick: Res<crate::timing::SimulationTick>,
    bounds: Res<crate::map::PlayableBounds>,
    input_tuning: Res<crate::movement::InputTuning>,
    mut field_ids: ResMut<crate::concealment::NextConcealmentFieldId>,
    mut telemetry: ResMut<crate::abilities::AbilityTelemetry>,
    fields: Query<(), With<crate::concealment::ConcealmentFieldState>>,
    mut casters: Query<
        (
            Entity,
            &avian2d::prelude::Position,
            &avian2d::prelude::Rotation,
            &crate::builds::ResolvedMatchLoadout,
            &crate::protocol::NetworkEntityId,
            &crate::combat::TeamId,
            &crate::matchplay::MatchParticipant,
            &crate::movement::InputFreshness,
            &mut crate::builds::AbilityState,
            Option<&lightyear::prelude::input::native::ActionState<crate::protocol::FighterInput>>,
            Option<&mut crate::abilities::UltimateInputLatch>,
            Option<&mut super::self_cloak::UltimateGeneration>,
            Has<crate::combat::Defeated>,
            Has<crate::matchplay::ActiveCombatant>,
        ),
        With<crate::protocol::Fighter>,
    >,
) {
    use lightyear::prelude::{NetworkTarget, Replicate};

    for (
        entity,
        position,
        rotation,
        loadout,
        network_id,
        team,
        participant,
        freshness,
        mut ability,
        action,
        latch,
        generation,
        defeated,
        active,
    ) in &mut casters
    {
        if loadout.ultimate.kind != crate::builds::UltimateKind::ConcealmentField {
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
        let held = !crate::movement::input_should_neutralize(tick.0, freshness.last_fresh_tick, 12);
        let rejection = if !held {
            Some(crate::abilities::AbilityRejectionReason::StaleInput)
        } else if defeated {
            Some(crate::abilities::AbilityRejectionReason::Defeated)
        } else if !active {
            Some(crate::abilities::AbilityRejectionReason::Inactive)
        } else if fields.iter().count() >= crate::concealment::MAX_ACTIVE_CONCEALMENT_FIELDS {
            Some(crate::abilities::AbilityRejectionReason::ActiveFieldCeiling)
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
        let crate::builds::UltimateParameters::ConcealmentField {
            maximum_range_milliunits,
            radius_milliunits,
            duration_ticks,
        } = loadout.ultimate.parameters
        else {
            continue;
        };
        let input = action.map(|action| action.0);
        let aim = input
            .and_then(|input| input.aim_update)
            .and_then(|axis| crate::movement::committed_aim(axis.to_vec2(), *input_tuning));
        let distance = input
            .and_then(|input| input.aim_distance)
            .map(crate::protocol::QuantizedAimDistance::to_world_units);
        let Some(maximum_range) =
            crate::builds::world_units_from_milliunits(maximum_range_milliunits)
        else {
            continue;
        };
        let Some(center) = crate::abilities::targeted_ultimate_center(
            position.0,
            Vec2::from_angle(rotation.as_radians()),
            aim,
            distance,
            maximum_range,
            bounds.0,
        ) else {
            continue;
        };
        let next_generation = generation
            .as_deref()
            .map_or(Some(1), |value| value.0.checked_add(1));
        let (Some(next_generation), Some(field_id)) = (next_generation, field_ids.allocate())
        else {
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
                .insert(super::self_cloak::UltimateGeneration(next_generation));
        }
        let expires_at_tick = tick.0.saturating_add(duration_ticks);
        commands.spawn((
            crate::concealment::ConcealmentFieldState {
                id: field_id,
                team: *team,
                center: center.into(),
                radius_milliunits,
                activated_at_tick: tick.0,
                expires_at_tick,
            },
            crate::concealment::ConcealmentFieldOwner {
                owner_network_id: *network_id,
                owner_generation: next_generation,
                match_id: participant.match_id,
            },
            Replicate::to_clients(NetworkTarget::All),
        ));
        *ability = crate::builds::AbilityState {
            charge: 0,
            phase: crate::builds::AbilityPhase::FieldActive {
                field_id,
                expires_at_tick,
            },
        };
        commands
            .entity(entity)
            .remove::<crate::matchplay::SpawnProtection>();
        telemetry.record(crate::abilities::AbilityTelemetryRecord {
            tick: tick.0,
            owner_network_id: *network_id,
            kind: crate::abilities::AbilityTelemetryKind::ConcealmentFieldAccepted,
        });
    }
}

#[cfg(feature = "server")]
#[allow(clippy::type_complexity, clippy::needless_pass_by_value)]
pub(crate) fn cleanup_concealment_fields(
    mut commands: Commands,
    tick: Res<crate::timing::SimulationTick>,
    mut telemetry: ResMut<crate::abilities::AbilityTelemetry>,
    roots: Query<&crate::matchplay::MatchState, With<crate::matchplay::MatchRoot>>,
    fields: Query<(
        Entity,
        &crate::concealment::ConcealmentFieldState,
        &crate::concealment::ConcealmentFieldOwner,
    )>,
    mut owners: Query<(
        &crate::protocol::NetworkEntityId,
        &crate::builds::ResolvedMatchLoadout,
        &mut crate::builds::AbilityState,
        Option<&crate::combat::Defeated>,
        Option<&crate::matchplay::ActiveCombatant>,
        Option<&lightyear::prelude::ControlledBy>,
    )>,
    disconnected: Query<
        Entity,
        (
            With<lightyear::prelude::LinkOf>,
            With<lightyear::prelude::Disconnected>,
        ),
    >,
) {
    let root = roots.single().ok();
    let mut ordered: Vec<_> = fields.iter().collect();
    ordered.sort_by_key(|(_, state, _)| state.id);
    for (entity, state, owner) in ordered {
        use crate::abilities::ConcealmentFieldCleanupReason as Reason;
        let mut reason = (tick.0 >= state.expires_at_tick).then_some(Reason::Expired);
        if let Some((_, loadout, mut ability, defeated, active, controlled)) = owners
            .iter_mut()
            .find(|(network_id, ..)| **network_id == owner.owner_network_id)
        {
            if reason.is_none() && defeated.is_some() {
                reason = Some(Reason::OwnerDefeated);
            }
            if reason.is_none()
                && controlled.is_some_and(|value| disconnected.contains(value.owner))
            {
                reason = Some(Reason::OwnerDisconnected);
            }
            if reason.is_none()
                && loadout.ultimate.kind != crate::builds::UltimateKind::ConcealmentField
            {
                reason = Some(Reason::BuildReplaced);
            }
            if reason.is_none()
                && root.is_some_and(|root| {
                    matches!(root.phase, crate::matchplay::MatchPhase::Completed { .. })
                })
            {
                reason = Some(Reason::MatchCompleted);
            }
            if reason.is_none()
                && (active.is_none() || root.is_none_or(|root| root.match_id != owner.match_id))
            {
                reason = Some(Reason::MatchRestarted);
            }
            if reason.is_some()
                && matches!(ability.phase, crate::builds::AbilityPhase::FieldActive { field_id, .. } if field_id == state.id)
            {
                ability.phase = crate::abilities::settled_ability_phase(ability.charge);
            }
        } else {
            reason.get_or_insert(Reason::OwnerDisconnected);
        }
        if let Some(reason) = reason {
            telemetry.record(crate::abilities::AbilityTelemetryRecord {
                tick: tick.0,
                owner_network_id: owner.owner_network_id,
                kind: crate::abilities::AbilityTelemetryKind::ConcealmentFieldCleanup {
                    reason,
                    active_ticks: tick.0.saturating_sub(state.activated_at_tick),
                },
            });
            commands.entity(entity).despawn();
        }
    }
}
