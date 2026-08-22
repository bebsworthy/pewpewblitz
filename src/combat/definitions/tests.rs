//! Weapon definition validation and resolution tests.

use super::*;
#[test]
fn embedded_catalog_is_exactly_four_presets() {
    let catalog = WeaponCatalog::embedded().unwrap();
    assert_eq!(catalog.presets.len(), 4);
    assert_eq!(
        catalog.presets[1].configuration.recipe.payload_bundles[0]
            .effects
            .len(),
        1
    );
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
    let mut fighter = super::super::FighterDefinitions::default().entries[0];
    fighter.body_radius = 20.0;
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
    let first = resolve_configuration(None, configuration.clone(), &fighter).unwrap();
    let mut other_profile = configuration;
    other_profile.presentation_profile_id = WeaponPresentationProfileId(4);
    let second = resolve_configuration(None, other_profile, &fighter).unwrap();
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
fn only_the_arc_launcher_carries_a_terrain_world_effect() {
    let catalog = WeaponCatalog::embedded().unwrap();
    for preset in &catalog.presets {
        match preset.id.0 {
            3 => {
                assert_eq!(
                    preset.configuration.recipe.world_effects,
                    vec![WorldEffectDefinition::DestroyTerrain { radius: 48.0 }],
                    "Arc Launcher carries exactly one radius-48 terrain brush"
                );
            }
            _ => assert!(
                preset.configuration.recipe.world_effects.is_empty(),
                "preset {} must be explicit about having no world effects",
                preset.id.0
            ),
        }
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

#[test]
fn world_effect_validation_rejects_invalid_count_radius_and_delivery() {
    let fighter = super::super::FighterDefinitions::default().entries[0];
    let policy = WeaponRecipePolicy::default();
    let limits = EngineWeaponLimits::default();

    // More than one world effect per delivery.
    let mut two = arc_configuration();
    two.recipe
        .world_effects
        .push(WorldEffectDefinition::DestroyTerrain { radius: 16.0 });
    assert!(
        two.validate(&policy, limits, Some(fighter.body_radius))
            .unwrap_err()
            .contains("too many world effects")
    );

    for radius in [f32::NAN, 4.0, 7.5, 50.0, 68.0] {
        let mut bad = arc_configuration();
        bad.recipe.world_effects = vec![WorldEffectDefinition::DestroyTerrain { radius }];
        assert!(
            bad.validate(&policy, limits, Some(fighter.body_radius))
                .unwrap_err()
                .contains("brush radius"),
            "radius {radius} must reject"
        );
    }

    // Destruction on straight (non-lobbed) delivery.
    let mut straight = arc_configuration();
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
            .validate(&policy, limits, Some(fighter.body_radius))
            .unwrap_err()
            .contains("single-fire lobbed")
    );

    // A policy ceiling wider than the engine ceiling rejects during catalog validation.
    let mut catalog = WeaponCatalog::embedded().unwrap();
    catalog.recipe_policy.max_terrain_brush_radius = 132.0;
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

    // The untouched Arc configuration still resolves with its terrain effect.
    assert!(
        arc_configuration()
            .validate(&policy, limits, Some(fighter.body_radius))
            .is_ok()
    );
}
