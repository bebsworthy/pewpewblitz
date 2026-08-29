pub const ADRENAL_DURATION_TICKS: u64 = 90;
pub const ADRENAL_REARM_TICKS: u64 = 240;

#[must_use]
#[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
pub fn apply_close_quarters_damage(base: u16, distance: f32) -> u16 {
    apply_close_quarters_scale(f32::from(base), distance)
        .clamp(0.0, f32::from(u16::MAX))
        .round() as u16
}

#[must_use]
pub(crate) fn apply_close_quarters_scale(base: f32, distance: f32) -> f32 {
    let scale = if distance <= 240.0 {
        1.15
    } else if distance >= 480.0 {
        0.85
    } else {
        1.15 - ((distance - 240.0) / 240.0) * 0.30
    };
    base * scale
}

#[must_use]
pub fn apply_quick_cycle_ticks(base_ticks: u64) -> u64 {
    base_ticks.saturating_mul(60).div_ceil(100).max(1)
}

#[must_use]
pub fn apply_tenacity_ticks(base_ticks: u64) -> u64 {
    base_ticks.saturating_mul(65).div_ceil(100).max(1)
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
            &crate::builds::ResolvedMatchLoadout,
            &mut crate::builds::PassiveRuntimeState,
            Option<&crate::matchplay::ActiveCombatant>,
        ),
        bevy::prelude::With<crate::protocol::Fighter>,
    >,
) {
    for (network_id, loadout, mut runtime, active) in &mut fighters {
        let adrenal = loadout
            .passives
            .iter()
            .find(|passive| passive.kind == crate::builds::PassiveKind::AdrenalResponse)
            .map(|passive| passive.id);
        let quick_cycle = loadout
            .passives
            .iter()
            .find(|passive| passive.kind == crate::builds::PassiveKind::QuickCycle)
            .map(|passive| passive.id);
        if active.is_some() {
            for passive in &loadout.passives {
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
            if let Some(passive_id) = adrenal
                && fact.target_network_id == *network_id
                && matches!(fact.kind, crate::combat::CombatOutcomeKind::Damage { .. })
                && runtime
                    .adrenaline_rearm_at_tick
                    .is_none_or(|deadline| tick.0 >= deadline)
            {
                runtime.adrenaline_until_tick = Some(tick.0.saturating_add(ADRENAL_DURATION_TICKS));
                runtime.adrenaline_rearm_at_tick = Some(tick.0.saturating_add(ADRENAL_REARM_TICKS));
                telemetry.record(crate::abilities::AbilityTelemetryRecord {
                    tick: tick.0,
                    owner_network_id: *network_id,
                    kind: crate::abilities::AbilityTelemetryKind::PassiveTriggered(passive_id),
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
