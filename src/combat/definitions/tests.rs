//! Weapon definition validation and resolution tests.

use super::*;
#[test]
fn embedded_catalog_is_exactly_seven_presets() {
    let catalog = WeaponCatalog::embedded().unwrap();
    assert_eq!(catalog.presets.len(), 7);
    assert_eq!(
        catalog.presets[1].configuration.recipe.payload_bundles[0]
            .effects
            .len(),
        1
    );
}

#[test]
fn catalog_accepts_an_additive_preset_using_existing_primitives() {
    let mut catalog = WeaponCatalog::embedded().unwrap();
    let mut eighth = catalog.presets.last().unwrap().clone();
    eighth.id = WeaponPresetId(8);
    eighth.key = "eighth-preset".into();
    eighth.display_name = "Eighth Preset".into();
    catalog.presets.push(eighth);

    assert!(catalog.validate().is_ok());
    assert!(catalog.fingerprint().is_ok());
}

#[test]
fn additive_catalog_still_rejects_invalid_metadata_and_inventory_overrun() {
    let embedded = WeaponCatalog::embedded().unwrap();
    let mut duplicate = embedded.clone();
    duplicate.presets[1].key = duplicate.presets[0].key.clone();
    assert!(duplicate.validate().is_err());

    let mut zero = embedded.clone();
    zero.presets[0].id = WeaponPresetId(0);
    assert!(zero.validate().is_err());

    let mut bounded = embedded;
    let template = bounded.presets.last().unwrap().clone();
    for id in 8..=u16::try_from(MAX_WEAPON_PRESETS).unwrap() {
        let mut preset = template.clone();
        preset.id = WeaponPresetId(id);
        preset.key = format!("preset-{id}");
        preset.display_name = format!("Preset {id}");
        bounded.presets.push(preset);
    }
    assert!(bounded.validate().is_ok());

    let id = u16::try_from(MAX_WEAPON_PRESETS + 1).unwrap();
    let mut preset = template;
    preset.id = WeaponPresetId(id);
    preset.key = format!("preset-{id}");
    preset.display_name = format!("Preset {id}");
    bounded.presets.push(preset);
    assert!(bounded.validate().is_err());
}

#[test]
fn embedded_weapon_defaults_match_the_accepted_balance_pass() {
    let catalog = WeaponCatalog::embedded().unwrap();
    let pulse = &catalog.presets[0].configuration.recipe;
    assert_eq!(
        pulse.economy,
        WeaponEconomy::Magazine {
            capacity: 4,
            refill_ticks: 60,
        }
    );
    assert_eq!(
        pulse.delivery,
        DeliveryMethod::Straight {
            speed: 500.0,
            radius: 2.0,
            range: 320.0,
            lifetime_ticks: 60,
            muzzle_offset: 34.0,
        }
    );
    assert!(matches!(
        pulse.payload_bundles[0].effects[0],
        PayloadEffectDefinition::Damage { amount: 200, .. }
    ));

    let scatter = &catalog.presets[1].configuration.recipe;
    assert_eq!(
        scatter.economy,
        WeaponEconomy::Magazine {
            capacity: 3,
            refill_ticks: 72,
        }
    );
    assert_eq!(
        scatter.firing,
        FiringPattern::Spread {
            delivery_count: 5,
            total_angle_degrees: 30.0,
        }
    );
    assert_eq!(
        scatter.delivery,
        DeliveryMethod::Straight {
            speed: 600.0,
            radius: 2.0,
            range: 320.0,
            lifetime_ticks: 60,
            muzzle_offset: 32.0,
        }
    );
    assert!(matches!(
        scatter.payload_bundles[0].effects[0],
        PayloadEffectDefinition::Damage { amount: 120, .. }
    ));

    assert_eq!(
        catalog.presets[2].configuration.recipe.economy,
        WeaponEconomy::Magazine {
            capacity: 3,
            refill_ticks: 96,
        }
    );
    assert_eq!(
        catalog.presets[3].configuration.recipe.economy,
        WeaponEconomy::Charges {
            capacity: 3,
            recharge_ticks: 60,
        }
    );

    let spray = &catalog.presets[5].configuration.recipe;
    assert_eq!(
        spray.delivery,
        DeliveryMethod::ConeSpray {
            propagation_speed: 480.0,
            reach: 240.0,
            angle_degrees: 70.0,
            linger_ticks: 30,
            pulse_interval_ticks: 10,
            map_occlusion: true,
            max_targets: 6,
        }
    );

    assert_splash_defaults(&catalog.presets[6].configuration.recipe);
}

fn assert_splash_defaults(splash: &WeaponRecipe) {
    assert_eq!(
        splash.delivery,
        DeliveryMethod::Splash {
            distance: 480.0,
            max_flight_ticks: 36,
            visual_arc_height: 110.0,
            landing_clearance_radius: 10.0,
            muzzle_offset: 34.0,
            shape: PersistentAreaShape::Circle { radius: 96.0 },
            duration_ticks: 240,
            pulse_interval_ticks: 30,
            map_occlusion: true,
            max_targets: 6,
            max_active_per_owner: 2,
        }
    );
    assert!(matches!(
        splash.payload_bundles[0].effects.as_slice(),
        [
            PayloadEffectDefinition::Damage {
                amount: 36,
                recipients: RecipientPolicy::Hostiles,
                ..
            },
            PayloadEffectDefinition::Heal {
                amount: 24,
                recipients: RecipientPolicy::AlliesAndOwner,
            }
        ]
    ));
}

#[test]
fn spread_is_symmetric_and_ordered() {
    let values = spread_angles(0.3, 7, 30.0);
    assert_eq!(values.len(), 7);
    assert!((values[0] + values[6] - 0.6).abs() < 0.0001);
    assert!((values[3] - 0.3).abs() < 0.0001);
}
#[test]
fn falloff_clamps_and_rounds_only_at_damage_boundary() {
    let falloff = DamageFalloff::Linear {
        start_distance: 140.0,
        end_distance: 360.0,
        minimum_scale: 0.5,
    };
    assert!((linear_falloff(falloff, 140.0) - 1.0).abs() < f32::EPSILON);
    assert!((linear_falloff(falloff, 360.0) - 0.5).abs() < f32::EPSILON);
}
#[test]
fn semantically_equal_catalog_text_has_stable_fingerprint() {
    let a = WeaponCatalog::embedded().unwrap();
    let mut b = WeaponCatalog::embedded().unwrap();
    b.presets.reverse();
    assert!(b.validate().is_err());
    assert!(a.fingerprint().is_ok());
    assert!(b.fingerprint().is_err());
}

#[test]
fn non_preset_configuration_uses_the_same_resolver_and_recipe_fingerprint() {
    let fighter = crate::builds::FighterBody { radius: 20.0 };
    let configuration = WeaponConfiguration {
        presentation_profile_id: WeaponPresentationProfileId(1),
        recipe: WeaponRecipe {
            economy: WeaponEconomy::Magazine {
                capacity: 2,
                refill_ticks: 30,
            },
            fire_cooldown_ticks: 6,
            firing: FiringPattern::Single,
            delivery: DeliveryMethod::Straight {
                speed: 300.0,
                radius: 4.0,
                range: 300.0,
                lifetime_ticks: 30,
                muzzle_offset: 25.0,
            },
            payload_bundles: vec![PayloadBundleDefinition {
                target: TargetSelection::Direct,
                effects: vec![PayloadEffectDefinition::Damage {
                    amount: 7,
                    falloff: DamageFalloff::None,
                    recipients: RecipientPolicy::Hostiles,
                }],
            }],
            world_effects: Vec::new(),
        },
    };
    let first = resolve_configuration(None, configuration.clone(), fighter).unwrap();
    let mut other_profile = configuration;
    other_profile.presentation_profile_id = WeaponPresentationProfileId(4);
    let second = resolve_configuration(None, other_profile, fighter).unwrap();
    assert_eq!(first.source_preset_id, None);
    assert_eq!(first.recipe_fingerprint, second.recipe_fingerprint);
    assert_ne!(
        first.presentation_profile_id,
        second.presentation_profile_id
    );
}

#[test]
fn catalog_rejects_unsupported_spread_and_unbounded_values() {
    let mut catalog = WeaponCatalog::embedded().unwrap();
    catalog.presets[1].configuration.recipe.firing = FiringPattern::Spread {
        delivery_count: 1,
        total_angle_degrees: 30.0,
    };
    assert!(catalog.validate().is_err());

    let mut catalog = WeaponCatalog::embedded().unwrap();
    catalog.presets[0].configuration.recipe.payload_bundles[0].effects[0] =
        PayloadEffectDefinition::Damage {
            amount: 1_001,
            falloff: DamageFalloff::None,
            recipients: RecipientPolicy::Hostiles,
        };
    assert!(catalog.validate().is_err());
}

#[test]
fn policy_narrows_damage_and_cannot_widen_engine_limits() {
    let mut catalog = WeaponCatalog::embedded().unwrap();
    catalog.recipe_policy.max_damage = 50;
    catalog.presets[0].configuration.recipe.payload_bundles[0].effects[0] =
        PayloadEffectDefinition::Damage {
            amount: 51,
            falloff: DamageFalloff::None,
            recipients: RecipientPolicy::Hostiles,
        };
    assert!(catalog.validate().is_err());

    let mut widened = WeaponCatalog::embedded().unwrap();
    widened.recipe_policy.max_damage = EngineWeaponLimits::default().max_damage + 1;
    assert!(widened.validate().is_err());
}

#[test]
fn policy_capabilities_disable_lob_and_reject_duplicate_entries() {
    let mut catalog = WeaponCatalog::embedded().unwrap();
    catalog
        .recipe_policy
        .permitted_delivery_methods
        .retain(|method| *method != DeliveryMethodKind::Lobbed);
    assert!(catalog.validate().is_err());

    let mut duplicate = WeaponCatalog::embedded().unwrap();
    duplicate
        .recipe_policy
        .permitted_payload_effects
        .push(PayloadEffectKind::Slow);
    assert!(duplicate.validate().is_err());
}

#[test]
fn policy_change_changes_content_fingerprint() {
    let first = WeaponCatalog::embedded().unwrap();
    let mut second = first.clone();
    second.recipe_policy.max_damage -= 1;
    assert_ne!(first.fingerprint(), second.fingerprint());
}

// --- Delivery-level world effects (Milestone 10) ---

#[test]
fn built_in_weapons_do_not_carry_map_destruction_world_effects() {
    let catalog = WeaponCatalog::embedded().unwrap();
    for preset in &catalog.presets {
        assert!(
            preset.configuration.recipe.world_effects.is_empty(),
            "preset {} must be explicit about having no world effects",
            preset.id.0
        );
    }
}

fn arc_configuration() -> WeaponConfiguration {
    WeaponCatalog::embedded()
        .unwrap()
        .preset(WeaponPresetId(3))
        .unwrap()
        .configuration
        .clone()
}

fn spray_configuration() -> WeaponConfiguration {
    WeaponCatalog::embedded()
        .unwrap()
        .preset(WeaponPresetId(6))
        .unwrap()
        .configuration
        .clone()
}

fn splash_configuration() -> WeaponConfiguration {
    WeaponCatalog::embedded()
        .unwrap()
        .preset(WeaponPresetId(7))
        .unwrap()
        .configuration
        .clone()
}

#[test]
fn splash_validation_bounds_geometry_cadence_and_effect_identity() {
    let body = crate::builds::BuildCatalog::embedded()
        .unwrap()
        .fighter_body;
    let policy = WeaponRecipePolicy::default();
    let limits = EngineWeaponLimits::default();

    let mut rectangle = splash_configuration();
    let DeliveryMethod::Splash { shape, .. } = &mut rectangle.recipe.delivery else {
        unreachable!();
    };
    *shape = PersistentAreaShape::Rectangle {
        half_extents: [96.0, 48.0],
    };
    rectangle
        .validate(&policy, limits, Some(body.radius))
        .unwrap();

    let mut duplicate = splash_configuration();
    duplicate.recipe.payload_bundles[0].effects[1] = PayloadEffectDefinition::Damage {
        amount: 24,
        falloff: DamageFalloff::None,
        recipients: RecipientPolicy::Hostiles,
    };
    assert!(
        duplicate
            .validate(&policy, limits, Some(body.radius))
            .unwrap_err()
            .contains("distinct")
    );

    let mut knockback = splash_configuration();
    knockback.recipe.payload_bundles[0].effects[1] = PayloadEffectDefinition::Knockback {
        speed: 100.0,
        duration_ticks: 5,
        recipients: RecipientPolicy::Hostiles,
    };
    assert!(
        knockback
            .validate(&policy, limits, Some(body.radius))
            .unwrap_err()
            .contains("does not support knockback")
    );

    let mut too_many_pulses = splash_configuration();
    let DeliveryMethod::Splash {
        pulse_interval_ticks,
        ..
    } = &mut too_many_pulses.recipe.delivery
    else {
        unreachable!();
    };
    *pulse_interval_ticks = 1;
    assert!(
        too_many_pulses
            .validate(&policy, limits, Some(body.radius))
            .unwrap_err()
            .contains("invalid splash delivery")
    );
}

#[test]
fn cone_spray_validation_rejects_unbounded_geometry_and_cadence() {
    let body = crate::builds::BuildCatalog::embedded()
        .unwrap()
        .fighter_body;
    let policy = WeaponRecipePolicy::default();
    let limits = EngineWeaponLimits::default();

    for delivery in [
        DeliveryMethod::ConeSpray {
            propagation_speed: 0.0,
            reach: 240.0,
            angle_degrees: 70.0,
            linger_ticks: 30,
            pulse_interval_ticks: 10,
            map_occlusion: true,
            max_targets: 6,
        },
        DeliveryMethod::ConeSpray {
            propagation_speed: 480.0,
            reach: 240.0,
            angle_degrees: 361.0,
            linger_ticks: 30,
            pulse_interval_ticks: 10,
            map_occlusion: true,
            max_targets: 6,
        },
        DeliveryMethod::ConeSpray {
            propagation_speed: 480.0,
            reach: 240.0,
            angle_degrees: 70.0,
            linger_ticks: 30,
            pulse_interval_ticks: 0,
            map_occlusion: true,
            max_targets: 6,
        },
    ] {
        let mut bad = spray_configuration();
        bad.recipe.delivery = delivery;
        assert!(bad.validate(&policy, limits, Some(body.radius)).is_err());
    }
}

#[test]
fn world_effect_validation_rejects_invalid_count_radius_and_delivery() {
    let body = crate::builds::BuildCatalog::embedded()
        .unwrap()
        .fighter_body;
    let policy = WeaponRecipePolicy::default();
    let limits = EngineWeaponLimits::default();

    // Generic authored weapon destruction remains supported even though no built-in uses it.
    let mut two = arc_configuration();
    two.recipe.world_effects = vec![WorldEffectDefinition::DestroyMap { radius: 48.0 }];
    two.recipe
        .world_effects
        .push(WorldEffectDefinition::DestroyMap { radius: 16.0 });
    assert!(
        two.validate(&policy, limits, Some(body.radius))
            .unwrap_err()
            .contains("too many world effects")
    );

    for radius in [f32::NAN, 0.0, 129.0] {
        let mut bad = arc_configuration();
        bad.recipe.world_effects = vec![WorldEffectDefinition::DestroyMap { radius }];
        assert!(
            bad.validate(&policy, limits, Some(body.radius))
                .unwrap_err()
                .contains("map destruction radius"),
            "radius {radius} must reject"
        );
    }

    // Destruction on straight (non-lobbed) delivery.
    let mut straight = arc_configuration();
    straight.recipe.world_effects = vec![WorldEffectDefinition::DestroyMap { radius: 48.0 }];
    straight.recipe.delivery = DeliveryMethod::Straight {
        speed: 900.0,
        radius: 6.0,
        range: 900.0,
        lifetime_ticks: 60,
        muzzle_offset: 34.0,
    };
    straight.recipe.payload_bundles = vec![PayloadBundleDefinition {
        target: TargetSelection::Direct,
        effects: vec![PayloadEffectDefinition::Damage {
            amount: 25,
            falloff: DamageFalloff::None,
            recipients: RecipientPolicy::Hostiles,
        }],
    }];
    assert!(
        straight
            .validate(&policy, limits, Some(body.radius))
            .unwrap_err()
            .contains("single-fire lobbed")
    );

    // A policy ceiling wider than the engine ceiling rejects during catalog validation.
    let mut catalog = WeaponCatalog::embedded().unwrap();
    catalog.recipe_policy.max_map_destruction_radius = 132.0;
    assert!(
        catalog
            .validate()
            .unwrap_err()
            .contains("policy exceeds engine limits")
    );
    let mut catalog = WeaponCatalog::embedded().unwrap();
    catalog.recipe_policy.max_world_effects_per_delivery = 2;
    assert!(
        catalog
            .validate()
            .unwrap_err()
            .contains("policy exceeds engine limits")
    );

    // The untouched Arc configuration resolves without a terrain effect.
    assert!(
        arc_configuration()
            .validate(&policy, limits, Some(body.radius))
            .is_ok()
    );
}
