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
    let close_quarters = crate::builds::PassiveParameters::CloseQuarters {
        near_distance_milliunits: 240_000,
        far_distance_milliunits: 480_000,
        near_damage_basis_points: 11_500,
        far_damage_basis_points: 8_500,
    };
    assert_eq!(
        requested_damage(
            90,
            DamageFalloff::None,
            0.0,
            1.0,
            Some(close_quarters),
            240.0,
        ),
        104
    );
    assert_eq!(
        requested_damage(90, DamageFalloff::None, 0.0, 1.0, None, 240.0),
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
            Some(close_quarters),
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

#[cfg(feature = "server")]
#[test]
fn armed_sticky_detonation_survives_owner_disconnect_but_live_deliveries_do_not() {
    use crate::combat::effects::planning::delivery_survives_owner_disconnect;

    assert!(delivery_survives_owner_disconnect(
        &PendingDeliveryKind::StickyDetonated {
            position: WorldPoint::from(Vec2::ZERO),
        }
    ));
    assert!(!delivery_survives_owner_disconnect(
        &PendingDeliveryKind::LobLanded {
            position: WorldPoint::from(Vec2::ZERO),
        }
    ));
    assert!(delivery_survives_owner_disconnect(
        &PendingDeliveryKind::SplashPulse {
            center: WorldPoint::from(Vec2::ZERO),
        }
    ));
}

#[cfg(feature = "server")]
#[test]
fn dual_splash_payload_routes_damage_to_enemies_and_healing_to_allies() {
    let source = AttackSource {
        kind: CombatSourceKind::PrimaryWeapon,
        attack_id: AttackId(1),
        player_id: PlayerId(1),
        owner_network_entity_id: NetworkEntityId(1),
        team_id: TeamId(1),
        recipe_fingerprint: WeaponRecipeFingerprint(1),
        presentation_profile_id: WeaponPresentationProfileId(7),
        legacy_compatibility: false,
        source_preset_id: Some(WeaponPresetId(7)),
        origin: WorldPoint::from(Vec2::ZERO),
        facing: 0.0,
    };
    let damage = PayloadEffectDefinition::Damage {
        amount: 36,
        falloff: DamageFalloff::None,
        recipients: RecipientPolicy::Hostiles,
    };
    let heal = PayloadEffectDefinition::Heal {
        amount: 24,
        recipients: RecipientPolicy::AlliesAndOwner,
    };

    assert_eq!(
        effect_recipient_scale(damage, source, NetworkEntityId(2), TeamId(2)),
        Some(1.0)
    );
    assert_eq!(
        effect_recipient_scale(heal, source, NetworkEntityId(2), TeamId(2)),
        None
    );
    assert_eq!(
        effect_recipient_scale(damage, source, NetworkEntityId(3), TeamId(1)),
        None
    );
    assert_eq!(
        effect_recipient_scale(heal, source, NetworkEntityId(3), TeamId(1)),
        Some(1.0)
    );
    assert_eq!(
        effect_recipient_scale(damage, source, NetworkEntityId(1), TeamId(1)),
        None
    );
    assert_eq!(
        effect_recipient_scale(heal, source, NetworkEntityId(1), TeamId(1)),
        Some(1.0)
    );
}

#[cfg(feature = "server")]
#[test]
fn damage_tile_suppresses_composed_heal_event_reservation() {
    use crate::combat::effects::planning::required_payload_event_count;
    use std::collections::{HashMap, HashSet};

    let target = Entity::from_raw_u32(1).expect("valid target entity");
    let source = AttackSource {
        kind: CombatSourceKind::PrimaryWeapon,
        attack_id: AttackId(1),
        player_id: PlayerId(1),
        owner_network_entity_id: NetworkEntityId(1),
        team_id: TeamId(1),
        recipe_fingerprint: WeaponRecipeFingerprint(1),
        presentation_profile_id: WeaponPresentationProfileId(7),
        legacy_compatibility: false,
        source_preset_id: Some(WeaponPresetId(7)),
        origin: WorldPoint::from(Vec2::ZERO),
        facing: 0.0,
    };
    let records = [PendingPayload {
        source,
        delivery_index: 0,
        bundle_index: 0,
        target,
        target_network_id: NetworkEntityId(2),
        position: Vec2::ZERO,
        engagement_distance: 0.0,
        delivery_travel: 0.0,
        contact_fraction: 0.0,
        bundle: PayloadBundleDefinition {
            target: TargetSelection::Direct,
            effects: vec![PayloadEffectDefinition::Heal {
                amount: 24,
                recipients: RecipientPolicy::AlliesAndOwner,
            }],
        },
    }];
    let connected = HashSet::from([source.owner_network_entity_id.0]);
    let mut blocked = HashMap::from([(
        target,
        (
            NetworkEntityId(2),
            TeamId(1),
            50,
            false,
            CombatTargetKind::Fighter,
            true,
        ),
    )]);
    assert_eq!(
        required_payload_event_count(&[], &records, &connected, &HashMap::new(), &mut blocked),
        Some(0),
    );

    blocked.get_mut(&target).expect("target exists").5 = false;
    assert_eq!(
        required_payload_event_count(&[], &records, &connected, &HashMap::new(), &mut blocked),
        Some(1),
    );
}
