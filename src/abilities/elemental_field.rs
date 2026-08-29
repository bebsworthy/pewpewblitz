//! Shared server-authoritative activation for the four elemental ultimate fields.

use bevy::prelude::*;

#[allow(
    clippy::too_many_arguments,
    clippy::type_complexity,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "the activation coordinator declares the complete authoritative input and spawn view"
)]
pub(crate) fn activate_elemental_field(
    mut commands: Commands,
    tick: Res<crate::timing::SimulationTick>,
    bounds: Res<crate::map::PlayableBounds>,
    input_tuning: Res<crate::movement::InputTuning>,
    mut field_ids: ResMut<crate::combat::fields::NextElementalFieldId>,
    mut combat_ids: ResMut<crate::combat::NextCombatIds>,
    mut outbox: ResMut<crate::combat::CombatOutbox>,
    mut telemetry: ResMut<crate::abilities::AbilityTelemetry>,
    fields: Query<(), With<crate::combat::ElementalFieldState>>,
    mut casters: Query<
        (
            Entity,
            &avian2d::prelude::Position,
            &avian2d::prelude::Rotation,
            &crate::builds::ResolvedMatchLoadout,
            &crate::protocol::PlayerId,
            &crate::protocol::NetworkEntityId,
            &crate::combat::TeamId,
            &crate::matchplay::MatchParticipant,
            &crate::movement::InputFreshness,
            &crate::combat::ActiveEffects,
            &mut crate::builds::AbilityState,
            Option<&lightyear::prelude::input::native::ActionState<crate::protocol::FighterInput>>,
            Option<&mut crate::abilities::UltimateInputLatch>,
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
        player_id,
        network_id,
        team,
        participant,
        freshness,
        effects,
        mut ability,
        action,
        latch,
        defeated,
        active,
    ) in &mut casters
    {
        let Some(kind) = crate::combat::fields::field_kind_for_ultimate(loadout.ultimate.kind)
        else {
            continue;
        };
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
        let fresh = !crate::movement::input_should_neutralize(
            tick.0,
            freshness.last_fresh_tick,
            crate::movement::AUTHORITATIVE_INPUT_STALE_TICKS,
        );
        let owns_field = fields.iter().any(|()| {
            // Ability phase is the exact per-owner capacity record; the global query owns only
            // the hard match ceiling.
            matches!(
                ability.phase,
                crate::builds::AbilityPhase::ElementalFieldActive { .. }
            )
        });
        let rejection = if !fresh {
            Some(crate::abilities::AbilityRejectionReason::StaleInput)
        } else if defeated {
            Some(crate::abilities::AbilityRejectionReason::Defeated)
        } else if !active {
            Some(crate::abilities::AbilityRejectionReason::Inactive)
        } else if effects.is_frozen(tick.0) {
            Some(crate::abilities::AbilityRejectionReason::Frozen)
        } else if owns_field
            || fields.iter().count() >= crate::combat::fields::MAX_ACTIVE_ELEMENTAL_FIELDS
        {
            Some(crate::abilities::AbilityRejectionReason::ActiveFieldCeiling)
        } else if ability.charge != loadout.ultimate.charge_policy.maximum
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
        let crate::builds::UltimateParameters::ElementalField {
            maximum_range_milliunits,
            radius_milliunits,
            duration_ticks,
            pulse_interval_ticks,
            effect,
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
        let (Some(field_id), Some(action_id), Some(event_id)) = (
            field_ids.allocate(),
            combat_ids.allocate_attack(),
            combat_ids.allocate_event(),
        ) else {
            telemetry.record(crate::abilities::AbilityTelemetryRecord {
                tick: tick.0,
                owner_network_id: *network_id,
                kind: crate::abilities::AbilityTelemetryKind::ActivationRejected(
                    crate::abilities::AbilityRejectionReason::IdentifierExhausted,
                ),
            });
            continue;
        };
        let expires_at_tick = tick.0.saturating_add(duration_ticks);
        let source = crate::combat::ConditionSource {
            action_id,
            kind: crate::combat::CombatSourceKind::Ultimate {
                ultimate_id: loadout.ultimate.id,
            },
            player_id: *player_id,
            network_entity_id: *network_id,
            team_id: *team,
            source_preset_id: None,
            recipe_fingerprint: None,
            presentation_profile_id: None,
        };
        commands.spawn((
            crate::combat::ElementalFieldState {
                id: field_id,
                kind,
                owner_network_entity_id: *network_id,
                team_id: *team,
                center: center.into(),
                radius_milliunits,
                activated_at_tick: tick.0,
                next_pulse_tick: tick.0,
                expires_at_tick,
            },
            crate::combat::ElementalFieldRuntime {
                source,
                match_id: participant.match_id,
                pulse_interval_ticks,
                effect: crate::combat::fields::field_payload(effect),
            },
            Replicate::to_clients(NetworkTarget::All),
        ));
        *ability = crate::builds::AbilityState {
            charge: 0,
            phase: crate::builds::AbilityPhase::ElementalFieldActive {
                field_id,
                expires_at_tick,
            },
        };
        commands
            .entity(entity)
            .remove::<crate::matchplay::SpawnProtection>();
        outbox
            .0
            .push(crate::combat::CombatCue::ElementalFieldActivated {
                event_id,
                tick: tick.0,
                source: *network_id,
                field_id,
                kind,
                center: center.into(),
                radius_milliunits,
                expires_at_tick,
            });
    }
}
