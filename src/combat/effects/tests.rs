use crate::combat::effects::runtime::*;
use crate::combat::*;
#[test]
fn slow_keeps_strongest_magnitude_and_latest_expiry() {
    let mut effects = ActiveEffects::default();
    refresh_strongest_slow(&mut effects, AttackId(1), NetworkEntityId(1), 700, 20);
    refresh_strongest_slow(&mut effects, AttackId(2), NetworkEntityId(2), 800, 30);
    assert_eq!(effects.slow.unwrap().movement_multiplier_milli, 700);
    refresh_strongest_slow(&mut effects, AttackId(3), NetworkEntityId(3), 500, 25);
    assert_eq!(effects.slow.unwrap().movement_multiplier_milli, 500);
}

#[cfg(feature = "server")]
#[test]
fn close_quarters_damage_is_identical_in_reservation_and_application_math() {
    assert_eq!(
        requested_damage(90, DamageFalloff::None, 0.0, 1.0, true, 240.0),
        104
    );
    assert_eq!(
        requested_damage(90, DamageFalloff::None, 0.0, 1.0, false, 240.0),
        90
    );
    assert_eq!(
        requested_damage(
            25,
            DamageFalloff::Linear {
                start_distance: 0.0,
                end_distance: 100.0,
                minimum_scale: 0.5,
            },
            100.0,
            1.0,
            true,
            240.0,
        ),
        14,
        "falloff and Close Quarters must compose before the single final rounding",
    );
}

#[cfg(feature = "server")]
#[test]
fn deployable_target_policy_is_explicit_for_every_m08_source_and_effect() {
    for source in [
        CombatSourceKind::PrimaryWeapon,
        CombatSourceKind::Ultimate {
            ultimate_id: crate::builds::UltimateDefinitionId(1),
        },
        CombatSourceKind::Deployable {
            ultimate_id: crate::builds::UltimateDefinitionId(2),
            deployable_id: crate::builds::DeployableId(1),
        },
    ] {
        assert!(combat_source_allows_target(
            source,
            CombatTargetKind::Deployable,
        ));
    }
    assert!(effect_allows_target(
        PayloadEffectDefinition::Damage {
            amount: 1,
            falloff: DamageFalloff::None,
            recipients: RecipientPolicy::Hostiles,
        },
        CombatTargetKind::Deployable,
    ));
    assert!(!effect_allows_target(
        PayloadEffectDefinition::Slow {
            movement_multiplier: 0.5,
            duration_ticks: 1,
            stacking: SlowStacking::StrongestRefreshes,
            recipients: RecipientPolicy::Hostiles,
        },
        CombatTargetKind::Deployable,
    ));
}

#[cfg(feature = "server")]
#[test]
fn payload_target_gate_orders_kind_combat_and_protection_rules() {
    use crate::combat::effects::application::{TargetGate, payload_target_gate};

    // Kind mismatch and out-of-combat participants never act, regardless of protection.
    assert_eq!(
        payload_target_gate(false, true, true, false, true),
        TargetGate::Skip
    );
    assert_eq!(
        payload_target_gate(true, true, false, true, true),
        TargetGate::Skip
    );
    // A non-participant (deployables) or active combatant applies unless protected.
    assert_eq!(
        payload_target_gate(true, false, false, false, false),
        TargetGate::Apply
    );
    assert_eq!(
        payload_target_gate(true, true, true, false, true),
        TargetGate::Apply
    );
    // Spawn protection only blocks hostile payloads; owner-contact records still apply.
    assert_eq!(
        payload_target_gate(true, true, true, true, true),
        TargetGate::ProtectedContact
    );
    assert_eq!(
        payload_target_gate(true, true, true, true, false),
        TargetGate::Apply
    );
}
