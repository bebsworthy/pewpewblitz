use super::*;
use crate::combat::{DeliveryMethod, FighterDefinitions, PayloadEffectDefinition, WeaponCatalog};

fn catalogs() -> (
    BuildCatalog,
    WeaponCatalog,
    crate::combat::FighterDefinition,
) {
    (
        BuildCatalog::embedded().unwrap(),
        WeaponCatalog::embedded().unwrap(),
        FighterDefinitions::default().entries[0],
    )
}

fn recipe(weapon: u16, ultimate: u16, passives: [u16; 2]) -> BrawlerBuildRecipe {
    BrawlerBuildRecipe {
        weapon: WeaponChoice::Preset(crate::combat::WeaponPresetId(weapon)),
        ultimate: UltimateDefinitionId(ultimate),
        passives: passives.map(PassiveDefinitionId),
    }
}

#[test]
fn embedded_catalog_exposes_current_authored_inventory_and_ultimate_parameters() {
    let (builds, _, _) = catalogs();
    assert_eq!(builds.balance_revision, BuildRevision(10));
    assert_eq!(builds.weapon_costs.len(), 6);
    assert_eq!(builds.ultimates.len(), 11);
    assert_eq!(builds.passives.len(), 9);
    assert_eq!(
        builds.ultimate(UltimateDefinitionId(3)).unwrap().parameters,
        UltimateParameters::SelfCloak {
            duration_ticks: 360
        }
    );
    assert_eq!(
        builds.ultimate(UltimateDefinitionId(4)).unwrap().parameters,
        UltimateParameters::RevealScan {
            maximum_range_milliunits: 640_000,
            radius_milliunits: 192_000,
            reveal_ticks: 300,
        }
    );
    assert_eq!(
        builds.ultimate(UltimateDefinitionId(5)).unwrap().parameters,
        UltimateParameters::ConcealmentField {
            maximum_range_milliunits: 480_000,
            radius_milliunits: 192_000,
            duration_ticks: 360,
        }
    );
    assert_eq!(
        builds.ultimate(UltimateDefinitionId(6)).unwrap().parameters,
        UltimateParameters::DemolitionStrike {
            maximum_range_milliunits: 520_000,
            radius_milliunits: 64_000,
        }
    );
    assert_eq!(
        builds
            .ultimate(UltimateDefinitionId(11))
            .unwrap()
            .parameters,
        UltimateParameters::BigBlob {
            maximum_range_milliunits: 520_000,
            flight_ticks: 45,
            visual_arc_height_milliunits: 140_000,
            landing_clearance_milliunits: 10_000,
            child_speed_milliunits: 520_000,
            child_radius_milliunits: 6_000,
            child_range_milliunits: 213_440,
            child_lifetime_ticks: 60,
            child_fuse_ticks: 69,
            child_explosion_radius_milliunits: 42_560,
            child_damage: 140,
            max_active_per_owner: 12,
        }
    );
}

#[test]
fn duplicate_and_frame_family_passives_are_rejected() {
    let (builds, weapons, fighter) = catalogs();
    let mut candidate = recipe(1, 1, [1, 1]);
    assert_eq!(
        resolve_build_recipe(&builds, &weapons, &fighter, candidate),
        Err(BuildResolutionError::InvalidCombination)
    );
    candidate.passives = [PassiveDefinitionId(1), PassiveDefinitionId(2)];
    assert_eq!(
        resolve_build_recipe(&builds, &weapons, &fighter, candidate),
        Err(BuildResolutionError::InvalidCombination)
    );
}

#[test]
fn recipes_reject_unknown_ids_and_resolve_exact_budget_and_body_stats() {
    let (builds, weapons, fighter) = catalogs();
    let controller =
        resolve_build_recipe(&builds, &weapons, &fighter, recipe(3, 2, [5, 6])).unwrap();
    assert_eq!(controller.total_points, BUILD_POINT_BUDGET);
    assert_eq!(controller.fighter_stats.maximum_health, 1_000);
    assert!((controller.fighter_stats.movement_speed - 70.0).abs() < f32::EPSILON);
    assert_eq!(controller.fighter_stats.health_recovery_rate, 100);

    let mut unknown = recipe(1, 1, [1, 3]);
    unknown.ultimate = UltimateDefinitionId(999);
    assert_eq!(
        resolve_build_recipe(&builds, &weapons, &fighter, unknown),
        Err(BuildResolutionError::UnknownId)
    );
    unknown = recipe(1, 1, [1, 3]);
    unknown.passives[1] = PassiveDefinitionId(999);
    assert_eq!(
        resolve_build_recipe(&builds, &weapons, &fighter, unknown),
        Err(BuildResolutionError::UnknownId)
    );

    let runner = resolve_build_recipe(&builds, &weapons, &fighter, recipe(1, 1, [1, 3])).unwrap();
    assert_eq!(runner.fighter_stats.maximum_health, 85);
    assert!((runner.fighter_stats.movement_speed - 110.0).abs() < f32::EPSILON);
    let bruiser = resolve_build_recipe(&builds, &weapons, &fighter, recipe(2, 1, [2, 6])).unwrap();
    assert_eq!(bruiser.fighter_stats.maximum_health, 120);
    assert!((bruiser.fighter_stats.movement_speed - 90.0).abs() < f32::EPSILON);
}

#[test]
fn saved_fighter_elemental_baselines_are_independent_and_passives_add_to_them() {
    let (mut builds, weapons, fighter) = catalogs();
    builds.fighter_profiles.lightweight.cold_capacity = 750;
    builds
        .fighter_profiles
        .lightweight
        .cold_resistance_basis_points = 1_000;
    let resolved = resolve_saved_brawler_recipe(
        &builds,
        &weapons,
        &fighter,
        crate::profiles::FighterProfileId(2),
        crate::profiles::WeaponBaseId(1),
        UltimateDefinitionId(1),
        [PassiveDefinitionId(7), PassiveDefinitionId(6)],
    )
    .unwrap();
    assert_eq!(resolved.fighter_stats.cold_capacity, 750);
    assert_eq!(resolved.fighter_stats.cold_resistance_basis_points, 4_000);
    assert_eq!(builds.fighter_profiles.default.cold_capacity, 1_000);

    builds
        .fighter_profiles
        .lightweight
        .cold_resistance_basis_points = 5_000;
    let clamped = resolve_saved_brawler_recipe(
        &builds,
        &weapons,
        &fighter,
        crate::profiles::FighterProfileId(2),
        crate::profiles::WeaponBaseId(1),
        UltimateDefinitionId(1),
        [PassiveDefinitionId(7), PassiveDefinitionId(6)],
    )
    .unwrap();
    assert_eq!(clamped.fighter_stats.cold_resistance_basis_points, 6_000);
}

#[test]
fn all_custom_pulse_axes_resolve_with_exact_values() {
    let (builds, weapons, fighter) = catalogs();
    let powers = [PulsePower::Light, PulsePower::Balanced, PulsePower::Heavy];
    let reaches = [PulseReach::Compact, PulseReach::Standard, PulseReach::Long];
    let magazines = [
        PulseMagazine::Quick,
        PulseMagazine::Standard,
        PulseMagazine::Expanded,
    ];
    let mut count = 0;
    for power in powers {
        for reach in reaches {
            for magazine in magazines {
                let candidate = BrawlerBuildRecipe {
                    weapon: WeaponChoice::CustomPulse {
                        power,
                        reach,
                        magazine,
                    },
                    ultimate: UltimateDefinitionId(1),
                    passives: [PassiveDefinitionId(1), PassiveDefinitionId(6)],
                };
                match resolve_build_recipe(&builds, &weapons, &fighter, candidate) {
                    Ok(resolved) => {
                        assert_eq!(resolved.primary_weapon.source_preset_id, None);
                        let DeliveryMethod::Straight { lifetime_ticks, .. } =
                            resolved.primary_weapon.recipe.delivery
                        else {
                            panic!("custom Pulse must remain straight")
                        };
                        assert_eq!(
                            lifetime_ticks,
                            match reach {
                                PulseReach::Compact => 45,
                                PulseReach::Standard => 60,
                                PulseReach::Long => 81,
                            }
                        );
                        let damage = resolved
                            .primary_weapon
                            .recipe
                            .payload_bundles
                            .iter()
                            .flat_map(|bundle| &bundle.effects)
                            .find_map(|effect| match effect {
                                PayloadEffectDefinition::Damage { amount, .. } => Some(*amount),
                                _ => None,
                            })
                            .unwrap();
                        assert_eq!(
                            damage,
                            match power {
                                PulsePower::Light => 20,
                                PulsePower::Balanced => 25,
                                PulsePower::Heavy => 30,
                            }
                        );
                    }
                    Err(BuildResolutionError::OverBudget) => assert_eq!(
                        (power, reach, magazine),
                        (PulsePower::Heavy, PulseReach::Long, PulseMagazine::Expanded)
                    ),
                    Err(error) => panic!("unexpected custom resolution error: {error:?}"),
                }
                count += 1;
            }
        }
    }
    assert_eq!(count, 27);
}

#[test]
fn passive_slot_order_does_not_change_canonical_fingerprint() {
    let (builds, weapons, fighter) = catalogs();
    let candidate = recipe(1, 1, [1, 3]);
    let swapped = BrawlerBuildRecipe {
        passives: [candidate.passives[1], candidate.passives[0]],
        ..candidate
    };
    let first = resolve_build_recipe(&builds, &weapons, &fighter, candidate).unwrap();
    let second = resolve_build_recipe(&builds, &weapons, &fighter, swapped).unwrap();
    assert_eq!(
        first.identity.recipe_fingerprint,
        second.identity.recipe_fingerprint
    );
    assert_eq!(first.passives, second.passives);
}

#[test]
fn catalog_rejects_count_identity_and_cost_mutations() {
    let (builds, _, _) = catalogs();
    let mut invalid = builds.clone();
    invalid.passives.pop();
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.ultimates[0].point_cost = 12;
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.schema_version = 0;
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.balance_revision = BuildRevision(0);
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.passives[0].display_name.clear();
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.ultimates[1].id = invalid.ultimates[0].id;
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.passives[1].key = invalid.passives[0].key.clone();
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.passives[0].point_cost = 0;
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.ultimates[0].kind = UltimateKind::Sentry;
    assert!(invalid.validate().is_err());
    let mut invalid = builds;
    invalid.passives[0].key = "Upper_Case".into();
    assert!(invalid.validate().is_err());
}

#[test]
fn catalog_rejects_non_finite_and_out_of_policy_balance_values() {
    let (builds, _, _) = catalogs();
    let mut valid = builds.clone();
    valid.fighter_profiles.default.maximum_health = u16::MAX;
    valid.fighter_profiles.default.movement_speed = 0.5;
    assert!(valid.validate().is_ok());

    let mut invalid = builds.clone();
    invalid.fighter_profiles.lightweight.movement_speed = f32::NAN;
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.fighter_profiles.lightweight.movement_speed = 0.0;
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.fighter_profiles.lightweight.movement_speed = MAX_FIGHTER_MOVEMENT_SPEED + 0.1;
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.fighter_profiles.reinforced.maximum_health = 0;
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.fighter_profiles.reinforced.cold_capacity = 0;
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.fighter_profiles.reinforced.cold_capacity = MAX_COLD_CAPACITY + 1;
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.fighter_profiles.default.reveal_proximity_radius = f32::NAN;
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.fighter_profiles.default.reveal_proximity_radius = 0.0;
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.custom_pulse.long.range = f32::INFINITY;
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.custom_pulse.heavy.fire_cooldown_ticks = 0;
    assert!(invalid.validate().is_err());
    let mut invalid = builds;
    invalid.custom_pulse.expanded.capacity = 33;
    assert!(invalid.validate().is_err());
}

#[test]
#[allow(
    clippy::float_cmp,
    reason = "canonical thousandth rounding is the contract under test"
)]
fn reveal_proximity_resolution_supports_bounded_bonus_malus_and_single_rounding() {
    assert_eq!(
        resolve_reveal_proximity_radius(
            160.0,
            RevealProximityModifier {
                flat_milliunits: 12_345,
                percent_basis_points: 1_250,
            },
        )
        .unwrap(),
        192.345
    );
    assert_eq!(
        resolve_reveal_proximity_radius(
            160.0,
            RevealProximityModifier {
                flat_milliunits: -20_000,
                percent_basis_points: -2_500,
            },
        )
        .unwrap(),
        100.0
    );
    assert_eq!(
        resolve_reveal_proximity_radius(
            32.0,
            RevealProximityModifier {
                flat_milliunits: -512_000,
                percent_basis_points: -9_000,
            },
        )
        .unwrap(),
        32.0
    );
    assert!(
        resolve_reveal_proximity_radius(
            160.0,
            RevealProximityModifier {
                flat_milliunits: 0,
                percent_basis_points: 20_001,
            },
        )
        .is_err()
    );
}

#[test]
fn recipe_budget_boundaries_are_exact_and_overflow_fails_closed() {
    let (builds, weapons, fighter) = catalogs();
    for (candidate, expected) in [
        (recipe(1, 1, [6, 3]), Ok(10)),
        (recipe(1, 2, [6, 3]), Ok(11)),
        (recipe(2, 2, [6, 3]), Ok(12)),
        (recipe(2, 2, [3, 4]), Err(BuildResolutionError::OverBudget)),
    ] {
        assert_eq!(
            resolve_build_recipe(&builds, &weapons, &fighter, candidate)
                .map(|resolved| resolved.total_points),
            expected
        );
    }

    let mut overflow = builds.clone();
    overflow.passives[2].point_cost = u8::MAX;
    assert!(matches!(
        resolve_build_recipe(&overflow, &weapons, &fighter, recipe(2, 2, [3, 4])),
        Err(BuildResolutionError::OverBudget)
    ));
}
