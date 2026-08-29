use bevy::prelude::Vec2;
#[cfg(feature = "server")]
use bevy::prelude::*;
#[cfg(feature = "server")]
use std::collections::HashMap;

#[must_use]
pub fn targeted_ultimate_center(
    origin: Vec2,
    facing: Vec2,
    aim_update: Option<Vec2>,
    requested_distance: Option<f32>,
    maximum_range: f32,
    bounds: crate::map::AxisAlignedMapRect,
) -> Option<Vec2> {
    if !origin.is_finite()
        || !facing.is_finite()
        || !maximum_range.is_finite()
        || maximum_range <= 0.0
    {
        return None;
    }
    let direction = aim_update
        .and_then(Vec2::try_normalize)
        .or_else(|| facing.try_normalize())?;
    let distance = requested_distance
        .unwrap_or(maximum_range)
        .clamp(0.0, maximum_range);
    distance
        .is_finite()
        .then(|| (origin + direction * distance).clamp(bounds.min, bounds.max))
}

#[cfg(feature = "server")]
#[derive(Clone, Copy)]
struct AcceptedScan {
    source: crate::protocol::NetworkEntityId,
    team: crate::combat::TeamId,
    generation: u64,
    center: Vec2,
    radius: f32,
    expires_at_tick: u64,
}

#[cfg(feature = "server")]
#[allow(
    clippy::type_complexity,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    clippy::needless_pass_by_value,
    reason = "the fixed-tick activation coordinator consumes Bevy system parameters"
)]
pub(crate) fn activate_reveal_scan(
    mut commands: Commands,
    tick: Res<crate::timing::SimulationTick>,
    bounds: Res<crate::map::PlayableBounds>,
    input_tuning: Res<crate::movement::InputTuning>,
    mut ids: ResMut<crate::combat::NextCombatIds>,
    mut outbox: ResMut<crate::combat::CombatOutbox>,
    mut telemetry: ResMut<crate::abilities::AbilityTelemetry>,
    mut casters: Query<
        (
            Entity,
            &avian2d::prelude::Position,
            &avian2d::prelude::Rotation,
            &crate::builds::ResolvedMatchLoadout,
            &crate::protocol::NetworkEntityId,
            &crate::combat::TeamId,
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
    targets: Query<
        (
            Entity,
            &avian2d::prelude::Position,
            &crate::protocol::NetworkEntityId,
            &crate::combat::TeamId,
            Option<&crate::concealment::ForcedRevealSources>,
            Has<crate::combat::Defeated>,
            Has<crate::matchplay::ActiveCombatant>,
        ),
        With<crate::protocol::Fighter>,
    >,
) {
    let mut scans = Vec::new();
    for (
        entity,
        position,
        rotation,
        loadout,
        network_id,
        team,
        freshness,
        mut ability,
        action,
        latch,
        generation,
        defeated,
        active,
    ) in &mut casters
    {
        if loadout.ultimate.kind != crate::builds::UltimateKind::RevealScan {
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
        let held = !crate::movement::input_should_neutralize(
            tick.0,
            freshness.last_fresh_tick,
            crate::movement::AUTHORITATIVE_INPUT_STALE_TICKS,
        );
        let rejection = if !held {
            Some(crate::abilities::AbilityRejectionReason::StaleInput)
        } else if defeated {
            Some(crate::abilities::AbilityRejectionReason::Defeated)
        } else if !active {
            Some(crate::abilities::AbilityRejectionReason::Inactive)
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
        let crate::builds::UltimateParameters::RevealScan {
            maximum_range_milliunits,
            radius_milliunits,
            reveal_ticks,
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
        let Some(center) = targeted_ultimate_center(
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
                .insert(super::self_cloak::UltimateGeneration(next_generation));
        }
        *ability = crate::builds::AbilityState {
            charge: 0,
            phase: crate::builds::AbilityPhase::Charging,
        };
        commands
            .entity(entity)
            .remove::<crate::matchplay::SpawnProtection>();
        let Some(radius) = crate::builds::world_units_from_milliunits(radius_milliunits) else {
            continue;
        };
        let expires_at_tick = tick.0.saturating_add(reveal_ticks);
        outbox
            .0
            .push(crate::combat::CombatCue::RevealScanActivated {
                event_id,
                tick: tick.0,
                revealing_team: *team,
                center: center.into(),
                radius_milliunits,
                expires_at_tick,
            });
        scans.push(AcceptedScan {
            source: *network_id,
            team: *team,
            generation: next_generation,
            center,
            radius,
            expires_at_tick,
        });
    }
    scans.sort_by_key(|scan| (scan.source.0, scan.generation));
    let mut target_views: Vec<_> = targets
        .iter()
        .map(
            |(entity, position, network_id, team, existing, defeated, active)| {
                (
                    entity,
                    position.0,
                    *network_id,
                    *team,
                    existing.cloned().unwrap_or_default(),
                    defeated,
                    active,
                )
            },
        )
        .collect();
    target_views.sort_by_key(|(_, _, network_id, ..)| network_id.0);
    let mut updated_sources = HashMap::new();
    for scan in scans {
        let mut target_count = 0_u16;
        for (entity, position, network_id, team, existing, defeated, active) in &target_views {
            if *defeated
                || !*active
                || !crate::combat::teams_are_hostile(scan.team, *team)
                || position.distance_squared(scan.center) > scan.radius * scan.radius
            {
                continue;
            }
            let sources = updated_sources
                .entry(*entity)
                .or_insert_with(|| existing.clone());
            let applied = sources.apply(crate::concealment::ForcedRevealSource {
                revealing_team: scan.team,
                source_network_id: scan.source,
                source_generation: scan.generation,
                applied_at_tick: tick.0,
                expires_at_tick: scan.expires_at_tick,
            });
            if !applied {
                continue;
            }
            target_count = target_count.saturating_add(1);
            if let Some(event_id) = ids.allocate_event() {
                outbox
                    .0
                    .push(crate::combat::CombatCue::ForcedRevealApplied {
                        event_id,
                        tick: tick.0,
                        target: *network_id,
                        revealing_team: scan.team,
                        source_generation: scan.generation,
                        expires_at_tick: scan.expires_at_tick,
                    });
            }
        }
        telemetry.record(crate::abilities::AbilityTelemetryRecord {
            tick: tick.0,
            owner_network_id: scan.source,
            kind: crate::abilities::AbilityTelemetryKind::RevealScanAccepted {
                targets: target_count,
            },
        });
    }
    for (entity, sources) in updated_sources {
        commands.entity(entity).insert(sources);
    }
}
