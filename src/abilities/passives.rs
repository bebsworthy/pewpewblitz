#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn apply_close_quarters_damage(
    base: u16,
    distance: f32,
    parameters: crate::builds::PassiveParameters,
) -> u16 {
    apply_close_quarters_scale(f32::from(base), distance, parameters)
        .clamp(0.0, f32::from(u16::MAX))
        .round() as u16
}

#[must_use]
pub(crate) fn apply_close_quarters_scale(
    base: f32,
    distance: f32,
    parameters: crate::builds::PassiveParameters,
) -> f32 {
    let crate::builds::PassiveParameters::CloseQuarters {
        near_distance_milliunits,
        far_distance_milliunits,
        near_damage_basis_points,
        far_damage_basis_points,
    } = parameters
    else {
        return base;
    };
    let Some(near_distance) = crate::builds::world_units_from_milliunits(near_distance_milliunits)
    else {
        return base;
    };
    let Some(far_distance) = crate::builds::world_units_from_milliunits(far_distance_milliunits)
    else {
        return base;
    };
    let near_scale = f32::from(near_damage_basis_points) / 10_000.0;
    let far_scale = f32::from(far_damage_basis_points) / 10_000.0;
    let scale = if distance <= near_distance {
        near_scale
    } else if distance >= far_distance {
        far_scale
    } else {
        near_scale
            - ((distance - near_distance) / (far_distance - near_distance))
                * (near_scale - far_scale)
    };
    base * scale
}

#[must_use]
pub fn apply_quick_cycle_ticks(base_ticks: u64, refill_duration_basis_points: u16) -> u64 {
    scale_duration_ticks(base_ticks, refill_duration_basis_points)
}

#[must_use]
pub fn apply_tenacity_ticks(base_ticks: u64, slow_duration_basis_points: u16) -> u64 {
    scale_duration_ticks(base_ticks, slow_duration_basis_points)
}

fn scale_duration_ticks(base_ticks: u64, duration_basis_points: u16) -> u64 {
    base_ticks
        .saturating_mul(u64::from(duration_basis_points))
        .div_ceil(10_000)
        .max(1)
}

#[cfg(feature = "server")]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn observe_passive_triggers(
    facts: bevy::prelude::Res<crate::combat::CombatOutcomeFacts>,
    tick: bevy::prelude::Res<crate::timing::SimulationTick>,
    mut telemetry: bevy::prelude::ResMut<crate::abilities::AbilityTelemetry>,
    mut fighters: bevy::prelude::Query<
        (
            &crate::protocol::NetworkEntityId,
            &crate::builds::ResolvedPassives,
            &mut crate::builds::PassiveRuntimeState,
            Option<&crate::matchplay::ActiveCombatant>,
        ),
        bevy::prelude::With<crate::protocol::Fighter>,
    >,
) {
    for (network_id, passives, mut runtime, active) in &mut fighters {
        let adrenal = passives.find(crate::builds::PassiveKind::AdrenalResponse);
        let quick_cycle = passives
            .find(crate::builds::PassiveKind::QuickCycle)
            .map(|passive| passive.id);
        if active.is_some() {
            for passive in &passives.passives {
                let is_active = match passive.kind {
                    crate::builds::PassiveKind::AdrenalResponse => runtime
                        .adrenaline_until_tick
                        .is_some_and(|deadline| tick.0 < deadline),
                    crate::builds::PassiveKind::QuickCycle => runtime.quick_cycle_primed,
                    crate::builds::PassiveKind::LightweightFrame
                    | crate::builds::PassiveKind::ReinforcedFrame
                    | crate::builds::PassiveKind::CloseQuarters
                    | crate::builds::PassiveKind::Tenacity
                    | crate::builds::PassiveKind::CryogenicInsulation
                    | crate::builds::PassiveKind::FilteredCirculation
                    | crate::builds::PassiveKind::HeatShielding => true,
                };
                if is_active {
                    telemetry.record_passive_active_tick(passive.id);
                }
            }
        }
        for fact in &facts.0 {
            if !matches!(
                fact.source_kind,
                crate::combat::CombatSourceKind::PrimaryWeapon
            ) || fact.target_kind != crate::combat::CombatTargetKind::Fighter
                || fact
                    .source_team
                    .is_none_or(|source_team| source_team == fact.target_team)
            {
                continue;
            }
            if let Some(passive) = adrenal
                && fact.target_network_id == *network_id
                && matches!(fact.kind, crate::combat::CombatOutcomeKind::Damage { .. })
                && runtime
                    .adrenaline_rearm_at_tick
                    .is_none_or(|deadline| tick.0 >= deadline)
            {
                let crate::builds::PassiveParameters::AdrenalResponse {
                    duration_ticks,
                    rearm_ticks,
                    ..
                } = passive.parameters
                else {
                    continue;
                };
                runtime.adrenaline_until_tick = Some(tick.0.saturating_add(duration_ticks));
                runtime.adrenaline_rearm_at_tick = Some(tick.0.saturating_add(rearm_ticks));
                telemetry.record(crate::abilities::AbilityTelemetryRecord {
                    tick: tick.0,
                    owner_network_id: *network_id,
                    kind: crate::abilities::AbilityTelemetryKind::PassiveTriggered(passive.id),
                });
            }
            if let Some(passive_id) = quick_cycle
                && fact.source_network_id == Some(*network_id)
                && matches!(fact.kind, crate::combat::CombatOutcomeKind::Defeat)
            {
                if runtime.quick_cycle_primed {
                    telemetry.record(crate::abilities::AbilityTelemetryRecord {
                        tick: tick.0,
                        owner_network_id: *network_id,
                        kind: crate::abilities::AbilityTelemetryKind::PassiveUnused(passive_id),
                    });
                }
                runtime.quick_cycle_primed = true;
                telemetry.record(crate::abilities::AbilityTelemetryRecord {
                    tick: tick.0,
                    owner_network_id: *network_id,
                    kind: crate::abilities::AbilityTelemetryKind::PassiveTriggered(passive_id),
                });
            }
        }
    }
}
