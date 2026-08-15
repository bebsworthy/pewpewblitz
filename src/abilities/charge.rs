use crate::builds::{AbilityPhase, AbilityState};

pub const ULTIMATE_CHARGE_MAX: u16 = 1_000;

#[must_use]
pub fn apply_charge(state: AbilityState, dealt_damage: u16, received_damage: u16) -> AbilityState {
    let earned = u32::from(dealt_damage)
        .saturating_mul(5)
        .saturating_add(u32::from(received_damage).saturating_mul(3));
    let charge = u16::try_from(
        u32::from(state.charge)
            .saturating_add(earned)
            .min(u32::from(ULTIMATE_CHARGE_MAX)),
    )
    .expect("ultimate charge is capped to a u16 constant");
    let phase = match state.phase {
        AbilityPhase::Charging | AbilityPhase::Ready => settled_ability_phase(charge),
        active @ (AbilityPhase::Dashing { .. } | AbilityPhase::Deployed { .. }) => active,
    };
    AbilityState { charge, phase }
}

#[must_use]
pub(crate) const fn settled_ability_phase(charge: u16) -> AbilityPhase {
    if charge >= ULTIMATE_CHARGE_MAX {
        AbilityPhase::Ready
    } else {
        AbilityPhase::Charging
    }
}

#[cfg(feature = "server")]
#[derive(bevy::prelude::Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(crate) struct ChargeObservationState {
    last_event_id: u64,
}

#[cfg(feature = "server")]
#[allow(clippy::needless_pass_by_value)]
pub(crate) fn observe_primary_damage_charge(
    facts: bevy::prelude::Res<crate::combat::CombatOutcomeFacts>,
    mut fighters: bevy::prelude::Query<
        (
            bevy::prelude::Entity,
            &crate::protocol::NetworkEntityId,
            &mut AbilityState,
            Option<&mut ChargeObservationState>,
        ),
        bevy::prelude::With<crate::protocol::Fighter>,
    >,
    mut commands: bevy::prelude::Commands,
    mut telemetry: bevy::prelude::ResMut<crate::abilities::AbilityTelemetry>,
) {
    for (entity, network_id, mut ability, observed) in &mut fighters {
        let previous = observed.as_deref().map_or(0, |state| state.last_event_id);
        let mut newest = previous;
        let mut dealt = 0_u16;
        let mut received = 0_u16;
        for fact in facts.0.iter().filter(|fact| fact.event_id.0 > previous) {
            newest = newest.max(fact.event_id.0);
            let crate::combat::CombatOutcomeKind::Damage { amount } = fact.kind else {
                continue;
            };
            if !matches!(
                fact.source_kind,
                crate::combat::CombatSourceKind::PrimaryWeapon
            ) || fact.target_kind != crate::combat::CombatTargetKind::Fighter
                || fact.source_team.is_none_or(|team| team == fact.target_team)
            {
                continue;
            }
            if fact.source_network_id == Some(*network_id) {
                dealt = dealt.saturating_add(amount);
            }
            if fact.target_network_id == *network_id {
                received = received.saturating_add(amount);
            }
        }
        if dealt != 0 || received != 0 {
            let was_full = ability.charge == ULTIMATE_CHARGE_MAX;
            let potential_earned = u32::from(dealt)
                .saturating_mul(5)
                .saturating_add(u32::from(received).saturating_mul(3));
            let available = u32::from(ULTIMATE_CHARGE_MAX.saturating_sub(ability.charge));
            let wasted = potential_earned.saturating_sub(available);
            *ability = apply_charge(*ability, dealt, received);
            let event_tick = facts.0.iter().map(|fact| fact.tick).max().unwrap_or(0);
            if dealt != 0 {
                telemetry.record(crate::abilities::AbilityTelemetryRecord {
                    tick: event_tick,
                    owner_network_id: *network_id,
                    kind: crate::abilities::AbilityTelemetryKind::ChargeDealt(dealt),
                });
            }
            if received != 0 {
                telemetry.record(crate::abilities::AbilityTelemetryRecord {
                    tick: event_tick,
                    owner_network_id: *network_id,
                    kind: crate::abilities::AbilityTelemetryKind::ChargeReceived(received),
                });
            }
            if wasted != 0 {
                telemetry.record(crate::abilities::AbilityTelemetryRecord {
                    tick: event_tick,
                    owner_network_id: *network_id,
                    kind: crate::abilities::AbilityTelemetryKind::ChargeWasted(wasted),
                });
            }
            if !was_full && ability.charge == ULTIMATE_CHARGE_MAX {
                telemetry.record(crate::abilities::AbilityTelemetryRecord {
                    tick: event_tick,
                    owner_network_id: *network_id,
                    kind: crate::abilities::AbilityTelemetryKind::FullCharge,
                });
            }
        }
        if let Some(mut observed) = observed {
            observed.last_event_id = newest;
        } else if newest != 0 {
            commands.entity(entity).insert(ChargeObservationState {
                last_event_id: newest,
            });
        }
    }
}
