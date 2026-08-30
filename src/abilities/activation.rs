use crate::{
    abilities::AbilityRejectionReason,
    builds::{AbilityPhase, AbilityState},
    protocol::FighterInput,
};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct UltimateRequestDecision {
    pub requested: bool,
    pub rising_edge: bool,
}

#[must_use]
pub(super) fn ultimate_request(
    input: Option<FighterInput>,
    was_requested: bool,
) -> UltimateRequestDecision {
    let requested = input.is_some_and(|input| {
        input.is_valid() && input.gameplay_buttons & FighterInput::ULTIMATE != 0
    });
    UltimateRequestDecision {
        requested,
        rising_edge: requested && !was_requested,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ActivationGateContext {
    pub input_fresh: bool,
    pub defeated: bool,
    pub active: bool,
    pub state: AbilityState,
    pub maximum_charge: u16,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct ActivationRestrictions {
    pub before_readiness: Option<AbilityRejectionReason>,
    pub after_readiness: Option<AbilityRejectionReason>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct ActivationPermit;

pub(super) fn evaluate_activation_gate(
    context: ActivationGateContext,
    restrictions: ActivationRestrictions,
) -> Result<ActivationPermit, AbilityRejectionReason> {
    if !context.input_fresh {
        return Err(AbilityRejectionReason::StaleInput);
    }
    if context.defeated {
        return Err(AbilityRejectionReason::Defeated);
    }
    if !context.active {
        return Err(AbilityRejectionReason::Inactive);
    }
    if let Some(reason) = restrictions.before_readiness {
        return Err(reason);
    }
    if context.state.charge != context.maximum_charge
        || !matches!(context.state.phase, AbilityPhase::Ready)
    {
        return Err(AbilityRejectionReason::NotCharged);
    }
    if let Some(reason) = restrictions.after_readiness {
        return Err(reason);
    }
    Ok(ActivationPermit)
}

#[must_use]
pub(super) const fn next_ultimate_generation(current: Option<u64>) -> Option<u64> {
    match current {
        Some(current) => current.checked_add(1),
        None => Some(1),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::protocol::QuantizedAxis2;

    fn input(buttons: u8) -> FighterInput {
        FighterInput {
            move_axis: QuantizedAxis2::default(),
            aim_update: None,
            aim_distance: None,
            gameplay_buttons: buttons,
        }
    }

    fn ready_context() -> ActivationGateContext {
        ActivationGateContext {
            input_fresh: true,
            defeated: false,
            active: true,
            state: AbilityState {
                charge: 100,
                phase: AbilityPhase::Ready,
            },
            maximum_charge: 100,
        }
    }

    #[test]
    fn raw_request_edges_require_release_and_repress() {
        assert_eq!(
            ultimate_request(None, false),
            UltimateRequestDecision {
                requested: false,
                rising_edge: false,
            }
        );
        let pressed = input(FighterInput::ULTIMATE);
        assert!(ultimate_request(Some(pressed), false).rising_edge);
        assert!(!ultimate_request(Some(pressed), true).rising_edge);
        assert!(!ultimate_request(Some(input(0)), true).requested);
        assert!(ultimate_request(Some(pressed), false).rising_edge);
    }

    #[test]
    fn rejected_edge_is_consumed_until_release() {
        let pressed = input(FighterInput::ULTIMATE);
        let first = ultimate_request(Some(pressed), false);
        let mut context = ready_context();
        context.input_fresh = false;
        assert_eq!(
            evaluate_activation_gate(context, ActivationRestrictions::default()),
            Err(AbilityRejectionReason::StaleInput)
        );
        assert!(!ultimate_request(Some(pressed), first.requested).rising_edge);
        assert!(!ultimate_request(Some(input(0)), first.requested).requested);
        assert!(ultimate_request(Some(pressed), false).rising_edge);
    }

    #[test]
    fn rejection_precedence_and_exact_readiness_are_stable() {
        let restrictions = ActivationRestrictions {
            before_readiness: Some(AbilityRejectionReason::ObjectiveCarrier),
            after_readiness: Some(AbilityRejectionReason::ExistingSentry),
        };
        let mut context = ready_context();
        context.input_fresh = false;
        context.defeated = true;
        context.active = false;
        assert_eq!(
            evaluate_activation_gate(context, restrictions),
            Err(AbilityRejectionReason::StaleInput)
        );
        context.input_fresh = true;
        assert_eq!(
            evaluate_activation_gate(context, restrictions),
            Err(AbilityRejectionReason::Defeated)
        );
        context.defeated = false;
        assert_eq!(
            evaluate_activation_gate(context, restrictions),
            Err(AbilityRejectionReason::Inactive)
        );
        context.active = true;
        assert_eq!(
            evaluate_activation_gate(context, restrictions),
            Err(AbilityRejectionReason::ObjectiveCarrier)
        );

        for state in [
            AbilityState {
                charge: 99,
                phase: AbilityPhase::Ready,
            },
            AbilityState {
                charge: 101,
                phase: AbilityPhase::Ready,
            },
            AbilityState {
                charge: 100,
                phase: AbilityPhase::Charging,
            },
        ] {
            assert_eq!(
                evaluate_activation_gate(
                    ActivationGateContext {
                        state,
                        ..ready_context()
                    },
                    ActivationRestrictions::default(),
                ),
                Err(AbilityRejectionReason::NotCharged)
            );
        }
        assert!(
            evaluate_activation_gate(ready_context(), ActivationRestrictions::default()).is_ok()
        );
        assert_eq!(
            evaluate_activation_gate(
                ready_context(),
                ActivationRestrictions {
                    after_readiness: Some(AbilityRejectionReason::ExistingSentry),
                    ..Default::default()
                }
            ),
            Err(AbilityRejectionReason::ExistingSentry)
        );
    }

    #[test]
    fn generation_rollover_is_checked_and_non_mutating() {
        assert_eq!(next_ultimate_generation(None), Some(1));
        assert_eq!(next_ultimate_generation(Some(0)), Some(1));
        assert_eq!(next_ultimate_generation(Some(u64::MAX - 1)), Some(u64::MAX));
        assert_eq!(next_ultimate_generation(Some(u64::MAX)), None);
    }
}
