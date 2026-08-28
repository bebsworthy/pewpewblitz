//! Server-authoritative targeted Demolition Strike activation.

use bevy::prelude::*;

#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "the fixed-tick activation coordinator consumes Bevy system parameters"
)]
pub(crate) fn activate_demolition_strike(
    mut commands: Commands,
    tick: Res<crate::timing::SimulationTick>,
    bounds: Res<crate::map::PlayableBounds>,
    input_tuning: Res<crate::movement::InputTuning>,
    mut ids: ResMut<crate::combat::NextCombatIds>,
    mut outbox: ResMut<crate::combat::CombatOutbox>,
    mut world_effects: ResMut<crate::combat::CombatWorldEffectFacts>,
    mut telemetry: ResMut<crate::abilities::AbilityTelemetry>,
    mut casters: Query<
        (
            Entity,
            &avian2d::prelude::Position,
            &avian2d::prelude::Rotation,
            &crate::builds::ResolvedMatchLoadout,
            &crate::protocol::NetworkEntityId,
            &crate::movement::InputFreshness,
            &mut crate::builds::AbilityState,
            Option<&lightyear::prelude::input::native::ActionState<crate::protocol::FighterInput>>,
            Option<&mut crate::abilities::UltimateInputLatch>,
            Has<crate::combat::Defeated>,
            Has<crate::matchplay::ActiveCombatant>,
        ),
        With<crate::protocol::Fighter>,
    >,
) {
    for (
        entity,
        position,
        rotation,
        loadout,
        network_id,
        freshness,
        mut ability,
        action,
        latch,
        defeated,
        active,
    ) in &mut casters
    {
        if loadout.ultimate.kind != crate::builds::UltimateKind::DemolitionStrike {
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
        let fresh =
            !crate::movement::input_should_neutralize(tick.0, freshness.last_fresh_tick, 12);
        let rejection = if !fresh {
            Some(crate::abilities::AbilityRejectionReason::StaleInput)
        } else if defeated {
            Some(crate::abilities::AbilityRejectionReason::Defeated)
        } else if !active {
            Some(crate::abilities::AbilityRejectionReason::Inactive)
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
        let crate::builds::UltimateParameters::DemolitionStrike {
            maximum_range_milliunits,
            radius_milliunits,
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
        let Some(center) = super::targeted_ultimate_center(
            position.0,
            Vec2::from_angle(rotation.as_radians()),
            aim,
            distance,
            maximum_range,
            bounds.0,
        ) else {
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
        let Some(radius) = crate::builds::world_units_from_milliunits(radius_milliunits) else {
            continue;
        };
        *ability = crate::builds::AbilityState {
            charge: 0,
            phase: crate::builds::AbilityPhase::Charging,
        };
        commands
            .entity(entity)
            .remove::<crate::matchplay::SpawnProtection>();
        world_effects.0.push(crate::combat::CombatWorldEffectFact {
            tick: tick.0,
            source: crate::combat::CombatWorldEffectSource::Ultimate {
                event_id,
                owner_network_entity_id: *network_id,
                ultimate_id: loadout.ultimate.id,
            },
            position: center.into(),
            effect: crate::combat::WorldEffectDefinition::DestroyMap { radius },
        });
        outbox
            .0
            .push(crate::combat::CombatCue::DemolitionStrikeActivated {
                event_id,
                tick: tick.0,
                source: *network_id,
                center: center.into(),
                radius_milliunits,
            });
    }
}
