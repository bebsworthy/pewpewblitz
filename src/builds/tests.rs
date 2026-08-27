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

#[test]
fn embedded_catalog_resolves_seven_legal_named_builds_and_new_ultimate_parameters() {
    let (builds, weapons, fighter) = catalogs();
    assert_eq!(builds.balance_revision, BuildRevision(4));
    assert_eq!(builds.presets.len(), 7);
    assert_eq!(builds.ultimates.len(), 5);
    for preset in &builds.presets {
        let resolved =
            resolve_build_recipe(&builds, &weapons, &fighter, preset.recipe, Some(preset.id))
                .unwrap();
        assert!(resolved.total_points <= 12);
        assert_eq!(resolved.identity.source_build_preset_id, Some(preset.id));
    }
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
}

#[test]
fn match_build_snapshots_round_trip_and_revalidate_preset_and_custom_candidates() {
    let (builds, weapons, fighter) = catalogs();
    let preset = builds.presets[0].clone();
    let custom_recipe = BrawlerBuildRecipe {
        weapon: WeaponChoice::CustomPulse {
            power: PulsePower::Balanced,
            reach: PulseReach::Standard,
            magazine: PulseMagazine::Quick,
        },
        ultimate: preset.recipe.ultimate,
        passives: preset.recipe.passives,
    };

    for (selection, recipe, source_preset) in [
        (
            BuildSelection::Preset(preset.id),
            preset.recipe,
            Some(preset.id),
        ),
        (BuildSelection::Custom(custom_recipe), custom_recipe, None),
    ] {
        let resolved =
            resolve_build_recipe(&builds, &weapons, &fighter, recipe, source_preset).unwrap();
        let snapshot = MatchBuildSnapshotV1 {
            schema_version: MatchBuildSnapshotV1::SCHEMA_VERSION,
            candidate: BuildCandidate {
                build_revision: builds.balance_revision,
                selection,
            },
            accepted: AcceptedBuildSummary {
                canonical_recipe: recipe,
                identity: resolved.identity,
                total_points: resolved.total_points,
            },
        };

        let encoded = snapshot.encode().unwrap();
        assert!(encoded.as_bytes().len() <= brawler_routing::MAX_MATCH_BUILD_SNAPSHOT_BYTES);
        let decoded = MatchBuildSnapshotV1::decode(&encoded).unwrap();
        assert_eq!(decoded, snapshot);

        let (decoded_recipe, decoded_preset) = match decoded.candidate.selection {
            BuildSelection::Preset(id) => (builds.preset(id).unwrap().recipe, Some(id)),
            BuildSelection::Custom(recipe) => (recipe, None),
        };
        let revalidated =
            resolve_build_recipe(&builds, &weapons, &fighter, decoded_recipe, decoded_preset)
                .unwrap();
        assert_eq!(revalidated.identity, decoded.accepted.identity);
        assert_eq!(revalidated.total_points, decoded.accepted.total_points);
        assert_eq!(decoded_recipe, decoded.accepted.canonical_recipe);
    }
}

#[test]
fn duplicate_and_frame_family_passives_are_rejected() {
    let (builds, weapons, fighter) = catalogs();
    let mut recipe = builds.presets[0].recipe;
    recipe.passives = [PassiveDefinitionId(1), PassiveDefinitionId(1)];
    assert_eq!(
        resolve_build_recipe(&builds, &weapons, &fighter, recipe, None),
        Err(BuildResolutionError::InvalidCombination)
    );
    recipe.passives = [PassiveDefinitionId(1), PassiveDefinitionId(2)];
    assert_eq!(
        resolve_build_recipe(&builds, &weapons, &fighter, recipe, None),
        Err(BuildResolutionError::InvalidCombination)
    );
}

#[test]
fn candidate_rejects_unknown_ids_and_resolves_exact_budget_and_body_stats() {
    let (builds, weapons, fighter) = catalogs();
    let controller = resolve_build_recipe(
        &builds,
        &weapons,
        &fighter,
        builds.presets[2].recipe,
        Some(builds.presets[2].id),
    )
    .unwrap();
    assert_eq!(controller.total_points, BUILD_POINT_BUDGET);
    assert_eq!(controller.fighter_stats.maximum_health, 100);
    assert!((controller.fighter_stats.movement_speed - 100.0).abs() < f32::EPSILON);

    let mut unknown = builds.presets[0].recipe;
    unknown.ultimate = UltimateDefinitionId(999);
    assert_eq!(
        resolve_build_recipe(&builds, &weapons, &fighter, unknown, None),
        Err(BuildResolutionError::UnknownId)
    );
    assert_eq!(
        resolve_build_recipe(
            &builds,
            &weapons,
            &fighter,
            builds.presets[0].recipe,
            Some(builds.presets[1].id),
        ),
        Err(BuildResolutionError::InvalidCombination)
    );
    unknown = builds.presets[0].recipe;
    unknown.passives[1] = PassiveDefinitionId(999);
    assert_eq!(
        resolve_build_recipe(&builds, &weapons, &fighter, unknown, None),
        Err(BuildResolutionError::UnknownId)
    );

    let runner = resolve_build_recipe(
        &builds,
        &weapons,
        &fighter,
        builds.presets[0].recipe,
        Some(builds.presets[0].id),
    )
    .unwrap();
    assert_eq!(runner.fighter_stats.maximum_health, 85);
    assert!((runner.fighter_stats.movement_speed - 110.0).abs() < f32::EPSILON);
    let bruiser = resolve_build_recipe(
        &builds,
        &weapons,
        &fighter,
        builds.presets[1].recipe,
        Some(builds.presets[1].id),
    )
    .unwrap();
    assert_eq!(bruiser.fighter_stats.maximum_health, 120);
    assert!((bruiser.fighter_stats.movement_speed - 90.0).abs() < f32::EPSILON);
}

#[test]
fn all_custom_pulse_axes_resolve_with_exact_values_and_no_preset_identity() {
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
                let recipe = BrawlerBuildRecipe {
                    weapon: WeaponChoice::CustomPulse {
                        power,
                        reach,
                        magazine,
                    },
                    ultimate: UltimateDefinitionId(1),
                    passives: [PassiveDefinitionId(1), PassiveDefinitionId(6)],
                };
                match resolve_build_recipe(&builds, &weapons, &fighter, recipe, None) {
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
    let recipe = builds.presets[0].recipe;
    let swapped = BrawlerBuildRecipe {
        passives: [recipe.passives[1], recipe.passives[0]],
        ..recipe
    };
    let first = resolve_build_recipe(&builds, &weapons, &fighter, recipe, None).unwrap();
    let second = resolve_build_recipe(&builds, &weapons, &fighter, swapped, None).unwrap();
    assert_eq!(
        first.identity.recipe_fingerprint,
        second.identity.recipe_fingerprint
    );
    assert_eq!(first.passives, second.passives);
}

#[test]
fn catalog_rejects_count_identity_cost_and_cross_reference_mutations() {
    let (builds, _, _) = catalogs();
    let mut invalid = builds.clone();
    invalid.passives.pop();
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.ultimates[0].point_cost = 12;
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.presets[0].recipe.weapon = WeaponChoice::Preset(crate::combat::WeaponPresetId(999));
    assert!(invalid.validate().is_err());
    let mut invalid = builds;
    invalid.presets[1].key = invalid.presets[0].key.clone();
    assert!(invalid.validate().is_err());
    let (mut invalid, _, _) = catalogs();
    invalid.schema_version = 0;
    assert!(invalid.validate().is_err());
    let (mut invalid, _, _) = catalogs();
    invalid.balance_revision = BuildRevision(0);
    assert!(invalid.validate().is_err());
    let (mut invalid, _, _) = catalogs();
    invalid.passives[0].display_name.clear();
    assert!(invalid.validate().is_err());

    let (mut invalid, _, _) = catalogs();
    invalid.ultimates[1].id = invalid.ultimates[0].id;
    assert!(invalid.validate().is_err());
    let (mut invalid, _, _) = catalogs();
    invalid.passives[1].key = invalid.passives[0].key.clone();
    assert!(invalid.validate().is_err());
    let (mut invalid, _, _) = catalogs();
    invalid.presets[0].recipe.ultimate = UltimateDefinitionId(999);
    assert!(invalid.validate().is_err());
    let (mut invalid, _, _) = catalogs();
    invalid.presets[0].recipe.passives[0] = PassiveDefinitionId(999);
    assert!(invalid.validate().is_err());
    let (mut invalid, _, _) = catalogs();
    invalid.passives[0].point_cost = 0;
    assert!(invalid.validate().is_err());
    let (mut invalid, _, _) = catalogs();
    invalid.ultimates[0].kind = UltimateKind::Sentry;
    assert!(invalid.validate().is_err());
    let (mut invalid, _, _) = catalogs();
    invalid.passives[0].key = "Upper_Case".into();
    assert!(invalid.validate().is_err());
    let (mut invalid, _, _) = catalogs();
    invalid.presets[0].display_name = "x".repeat(65);
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
fn candidate_budget_boundaries_are_exact_and_overflow_fails_closed() {
    let (builds, weapons, fighter) = catalogs();
    let recipe = |weapon: u16, ultimate: u16, passives: [u16; 2]| BrawlerBuildRecipe {
        weapon: WeaponChoice::Preset(crate::combat::WeaponPresetId(weapon)),
        ultimate: UltimateDefinitionId(ultimate),
        passives: passives.map(PassiveDefinitionId),
    };
    for (candidate, expected) in [
        (recipe(1, 1, [6, 3]), Ok(10)),
        (recipe(1, 2, [6, 3]), Ok(11)),
        (recipe(2, 2, [6, 3]), Ok(12)),
        (recipe(2, 2, [3, 4]), Err(BuildResolutionError::OverBudget)),
    ] {
        assert_eq!(
            resolve_build_recipe(&builds, &weapons, &fighter, candidate, None)
                .map(|resolved| resolved.total_points),
            expected
        );
    }

    let mut overflow = builds.clone();
    overflow.passives[2].point_cost = u8::MAX;
    assert!(matches!(
        resolve_build_recipe(&overflow, &weapons, &fighter, recipe(2, 2, [3, 4]), None,),
        Err(BuildResolutionError::OverBudget)
    ));
}

#[cfg(feature = "server")]
#[test]
fn build_telemetry_is_bounded_without_losing_drop_evidence() {
    let (builds, weapons, fighter) = catalogs();
    let resolved = resolve_build_recipe(
        &builds,
        &weapons,
        &fighter,
        builds.presets[0].recipe,
        Some(builds.presets[0].id),
    )
    .unwrap();
    let mut telemetry = BuildTelemetry::default();
    for request_id in 0..=telemetry::MAX_BUILD_TELEMETRY_RECORDS as u64 {
        telemetry.record(BuildSelectionTelemetryRecord {
            tick: request_id,
            request_id,
            owner_network_id: crate::protocol::NetworkEntityId(1),
            identity: resolved.identity,
            total_points: resolved.total_points,
            weapon_fingerprint: resolved.primary_weapon.recipe_fingerprint,
            ultimate_id: resolved.ultimate.id,
            passive_ids: resolved.passives.map(|passive| passive.id),
        });
    }
    assert_eq!(
        telemetry.selections.len(),
        telemetry::MAX_BUILD_TELEMETRY_RECORDS
    );
    assert_eq!(telemetry.dropped_records, 1);
    assert_eq!(telemetry.selections.front().unwrap().request_id, 1);
}
