use super::*;
use crate::combat::{DeliveryMethod, PayloadEffectDefinition, WeaponCatalog};

fn catalogs() -> (BuildCatalog, WeaponCatalog) {
    (
        BuildCatalog::embedded().unwrap(),
        WeaponCatalog::embedded().unwrap(),
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
fn runtime_projection_installs_the_exact_resolved_ability_capabilities() {
    let (builds, weapons) = catalogs();
    let loadout = resolve_build_recipe(&builds, &weapons, recipe(1, 6, [1, 3])).unwrap();
    let projection = MatchLoadoutProjection::new(&loadout, builds.fighter_body);

    assert_eq!(projection.ultimate, loadout.ultimate);
    assert_eq!(projection.passives.passives, loadout.passives);
}

#[test]
fn embedded_catalog_exposes_current_authored_inventory_and_ultimate_parameters() {
    let (builds, _) = catalogs();
    assert_eq!(builds.balance_revision, BuildRevision(12));
    assert_eq!(builds.weapon_costs.len(), 7);
    assert_eq!(builds.ultimates.len(), 11);
    assert_eq!(builds.passives.len(), 9);
    assert_eq!(
        builds.ultimate_charge,
        UltimateChargePolicy {
            maximum: 1_000,
            dealt_damage_multiplier: 5,
            received_damage_multiplier: 3,
        }
    );
    assert_eq!(
        builds.ultimate(UltimateDefinitionId(1)).unwrap().parameters,
        UltimateParameters::Dash {
            maximum_distance_milliunits: 360_000,
            duration_ticks: 18,
            damage: 35,
            knockback_speed_milliunits: 450_000,
            knockback_duration_ticks: 6,
            maximum_targets: 8,
        }
    );
    assert_eq!(
        builds.ultimate(UltimateDefinitionId(2)).unwrap().parameters,
        UltimateParameters::Sentry {
            placement_offsets_milliunits: [96_000, 88_000, 80_000, 72_000, 64_000, 56_000],
            body_radius_milliunits: 20_000,
            acquisition_range_milliunits: 480_000,
            acquisition_interval_ticks: 6,
            fire_interval_ticks: 30,
            lifetime_ticks: 720,
            maximum_health: 80,
            projectile_speed_milliunits: 900_000,
            projectile_radius_milliunits: 6_000,
            projectile_range_milliunits: 480_000,
            projectile_lifetime_ticks: 32,
            projectile_damage: 10,
        }
    );
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

fn catalog_with_ultimate_parameters(
    catalog: &BuildCatalog,
    kind: UltimateKind,
    parameters: UltimateParameters,
) -> BuildCatalog {
    let mut candidate = catalog.clone();
    candidate
        .ultimates
        .iter_mut()
        .find(|definition| definition.kind == kind)
        .expect("embedded ultimate kind exists")
        .parameters = parameters;
    candidate
}

fn assert_ultimate_parameters_valid(
    catalog: &BuildCatalog,
    kind: UltimateKind,
    parameters: UltimateParameters,
) {
    assert!(
        catalog_with_ultimate_parameters(catalog, kind, parameters)
            .validate()
            .is_ok(),
        "{kind:?} should accept {parameters:?}"
    );
}

fn assert_ultimate_parameters_invalid(
    catalog: &BuildCatalog,
    kind: UltimateKind,
    parameters: UltimateParameters,
) {
    assert!(
        catalog_with_ultimate_parameters(catalog, kind, parameters)
            .validate()
            .is_err(),
        "{kind:?} should reject {parameters:?}"
    );
}

fn sentry_with_relational_violation(
    parameters: UltimateParameters,
    violation: u8,
) -> UltimateParameters {
    let UltimateParameters::Sentry {
        mut placement_offsets_milliunits,
        body_radius_milliunits,
        acquisition_range_milliunits,
        acquisition_interval_ticks,
        fire_interval_ticks,
        lifetime_ticks,
        maximum_health,
        projectile_speed_milliunits,
        projectile_radius_milliunits,
        projectile_range_milliunits,
        projectile_lifetime_ticks,
        projectile_damage,
    } = parameters
    else {
        panic!("expected Sentry parameters")
    };
    if violation == 0 {
        placement_offsets_milliunits[1] = placement_offsets_milliunits[0];
    }
    UltimateParameters::Sentry {
        placement_offsets_milliunits,
        body_radius_milliunits,
        acquisition_range_milliunits,
        acquisition_interval_ticks,
        fire_interval_ticks,
        lifetime_ticks,
        maximum_health,
        projectile_speed_milliunits,
        projectile_radius_milliunits: if violation == 1 {
            body_radius_milliunits + 1
        } else {
            projectile_radius_milliunits
        },
        projectile_range_milliunits: if violation == 2 {
            acquisition_range_milliunits + 1
        } else {
            projectile_range_milliunits
        },
        projectile_lifetime_ticks,
        projectile_damage,
    }
}

#[test]
fn ultimate_parameter_bound_literals_are_exact() {
    let bounds = ULTIMATE_PARAMETER_BOUNDS;
    assert_eq!(
        (
            bounds.world_distance_milliunits.minimum,
            bounds.world_distance_milliunits.maximum
        ),
        (1, 4_096_000)
    );
    assert_eq!(
        (
            bounds.field_radius_milliunits.minimum,
            bounds.field_radius_milliunits.maximum
        ),
        (1, 2_048_000)
    );
    assert_eq!(
        (
            bounds.compact_radius_milliunits.minimum,
            bounds.compact_radius_milliunits.maximum
        ),
        (1, 512_000)
    );
    assert_eq!(
        (
            bounds.sentry_placement_offset_milliunits.minimum,
            bounds.sentry_placement_offset_milliunits.maximum
        ),
        (1, 1_024_000)
    );
    assert_eq!(
        (
            bounds.demolition_radius_milliunits.minimum,
            bounds.demolition_radius_milliunits.maximum
        ),
        (8_000, 64_000)
    );
    assert_eq!(
        (bounds.short_ticks.minimum, bounds.short_ticks.maximum),
        (1, 600)
    );
    assert_eq!(
        (bounds.duration_ticks.minimum, bounds.duration_ticks.maximum),
        (1, 3_600)
    );
    assert_eq!(
        (
            bounds.long_lifetime_ticks.minimum,
            bounds.long_lifetime_ticks.maximum
        ),
        (1, 36_000)
    );
    assert_eq!((bounds.damage.minimum, bounds.damage.maximum), (1, 1_000));
    assert_eq!((bounds.health.minimum, bounds.health.maximum), (1, 10_000));
    assert_eq!(
        (bounds.effect_amount.minimum, bounds.effect_amount.maximum),
        (1, u16::MAX)
    );
    assert_eq!(
        (bounds.target_count.minimum, bounds.target_count.maximum),
        (1, 32)
    );
    assert_eq!(
        (bounds.active_count.minimum, bounds.active_count.maximum),
        (1, 16)
    );
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "the endpoint matrix keeps every ultimate field and effect family visibly covered"
)]
fn every_ultimate_field_accepts_its_exact_lower_and_upper_endpoints() {
    let (catalog, _) = catalogs();
    let cases = [
        (
            UltimateKind::Dash,
            UltimateParameters::Dash {
                maximum_distance_milliunits: 1,
                duration_ticks: 1,
                damage: 1,
                knockback_speed_milliunits: 1,
                knockback_duration_ticks: 1,
                maximum_targets: 1,
            },
            UltimateParameters::Dash {
                maximum_distance_milliunits: 4_096_000,
                duration_ticks: 600,
                damage: 1_000,
                knockback_speed_milliunits: 4_096_000,
                knockback_duration_ticks: 600,
                maximum_targets: 32,
            },
        ),
        (
            UltimateKind::Sentry,
            UltimateParameters::Sentry {
                placement_offsets_milliunits: [6, 5, 4, 3, 2, 1],
                body_radius_milliunits: 1,
                acquisition_range_milliunits: 1,
                acquisition_interval_ticks: 1,
                fire_interval_ticks: 1,
                lifetime_ticks: 1,
                maximum_health: 1,
                projectile_speed_milliunits: 1,
                projectile_radius_milliunits: 1,
                projectile_range_milliunits: 1,
                projectile_lifetime_ticks: 1,
                projectile_damage: 1,
            },
            UltimateParameters::Sentry {
                placement_offsets_milliunits: [
                    1_024_000, 1_023_999, 1_023_998, 1_023_997, 1_023_996, 1_023_995,
                ],
                body_radius_milliunits: 512_000,
                acquisition_range_milliunits: 4_096_000,
                acquisition_interval_ticks: 600,
                fire_interval_ticks: 3_600,
                lifetime_ticks: 36_000,
                maximum_health: 10_000,
                projectile_speed_milliunits: 4_096_000,
                projectile_radius_milliunits: 512_000,
                projectile_range_milliunits: 4_096_000,
                projectile_lifetime_ticks: 600,
                projectile_damage: 1_000,
            },
        ),
        (
            UltimateKind::SelfCloak,
            UltimateParameters::SelfCloak { duration_ticks: 1 },
            UltimateParameters::SelfCloak {
                duration_ticks: 3_600,
            },
        ),
        (
            UltimateKind::RevealScan,
            UltimateParameters::RevealScan {
                maximum_range_milliunits: 1,
                radius_milliunits: 1,
                reveal_ticks: 1,
            },
            UltimateParameters::RevealScan {
                maximum_range_milliunits: 4_096_000,
                radius_milliunits: 2_048_000,
                reveal_ticks: 3_600,
            },
        ),
        (
            UltimateKind::ConcealmentField,
            UltimateParameters::ConcealmentField {
                maximum_range_milliunits: 1,
                radius_milliunits: 1,
                duration_ticks: 1,
            },
            UltimateParameters::ConcealmentField {
                maximum_range_milliunits: 4_096_000,
                radius_milliunits: 2_048_000,
                duration_ticks: 3_600,
            },
        ),
        (
            UltimateKind::DemolitionStrike,
            UltimateParameters::DemolitionStrike {
                maximum_range_milliunits: 1,
                radius_milliunits: 8_000,
            },
            UltimateParameters::DemolitionStrike {
                maximum_range_milliunits: 4_096_000,
                radius_milliunits: 64_000,
            },
        ),
        (
            UltimateKind::CryogenicField,
            UltimateParameters::ElementalField {
                maximum_range_milliunits: 1,
                radius_milliunits: 1,
                duration_ticks: 1,
                pulse_interval_ticks: 1,
                effect: ElementalFieldEffect::Cold { amount: 1 },
            },
            UltimateParameters::ElementalField {
                maximum_range_milliunits: 4_096_000,
                radius_milliunits: 2_048_000,
                duration_ticks: 3_600,
                pulse_interval_ticks: 3_600,
                effect: ElementalFieldEffect::Cold { amount: u16::MAX },
            },
        ),
        (
            UltimateKind::FireField,
            UltimateParameters::ElementalField {
                maximum_range_milliunits: 1,
                radius_milliunits: 1,
                duration_ticks: 1,
                pulse_interval_ticks: 1,
                effect: ElementalFieldEffect::DamageOverTime {
                    kind: crate::combat::DamageOverTimeKind::Fire,
                    damage_per_tick: 1,
                    tick_interval: 1,
                    duration_ticks: 1,
                },
            },
            UltimateParameters::ElementalField {
                maximum_range_milliunits: 4_096_000,
                radius_milliunits: 2_048_000,
                duration_ticks: 3_600,
                pulse_interval_ticks: 3_600,
                effect: ElementalFieldEffect::DamageOverTime {
                    kind: crate::combat::DamageOverTimeKind::Fire,
                    damage_per_tick: u16::MAX,
                    tick_interval: 3_600,
                    duration_ticks: 3_600,
                },
            },
        ),
        (
            UltimateKind::PoisonField,
            UltimateParameters::ElementalField {
                maximum_range_milliunits: 1,
                radius_milliunits: 1,
                duration_ticks: 1,
                pulse_interval_ticks: 1,
                effect: ElementalFieldEffect::DamageOverTime {
                    kind: crate::combat::DamageOverTimeKind::Poison,
                    damage_per_tick: 1,
                    tick_interval: 1,
                    duration_ticks: 1,
                },
            },
            UltimateParameters::ElementalField {
                maximum_range_milliunits: 4_096_000,
                radius_milliunits: 2_048_000,
                duration_ticks: 3_600,
                pulse_interval_ticks: 3_600,
                effect: ElementalFieldEffect::DamageOverTime {
                    kind: crate::combat::DamageOverTimeKind::Poison,
                    damage_per_tick: u16::MAX,
                    tick_interval: 3_600,
                    duration_ticks: 3_600,
                },
            },
        ),
        (
            UltimateKind::RestorationField,
            UltimateParameters::ElementalField {
                maximum_range_milliunits: 1,
                radius_milliunits: 1,
                duration_ticks: 1,
                pulse_interval_ticks: 1,
                effect: ElementalFieldEffect::Heal { amount: 1 },
            },
            UltimateParameters::ElementalField {
                maximum_range_milliunits: 4_096_000,
                radius_milliunits: 2_048_000,
                duration_ticks: 3_600,
                pulse_interval_ticks: 3_600,
                effect: ElementalFieldEffect::Heal { amount: u16::MAX },
            },
        ),
        (
            UltimateKind::BigBlob,
            UltimateParameters::BigBlob {
                maximum_range_milliunits: 1,
                flight_ticks: 1,
                visual_arc_height_milliunits: 1,
                landing_clearance_milliunits: 1,
                child_speed_milliunits: 1,
                child_radius_milliunits: 1,
                child_range_milliunits: 1,
                child_lifetime_ticks: 1,
                child_fuse_ticks: 1,
                child_explosion_radius_milliunits: 1,
                child_damage: 1,
                max_active_per_owner: 1,
            },
            UltimateParameters::BigBlob {
                maximum_range_milliunits: 4_096_000,
                flight_ticks: 600,
                visual_arc_height_milliunits: 2_048_000,
                landing_clearance_milliunits: 512_000,
                child_speed_milliunits: 4_096_000,
                child_radius_milliunits: 512_000,
                child_range_milliunits: 4_096_000,
                child_lifetime_ticks: 600,
                child_fuse_ticks: 3_600,
                child_explosion_radius_milliunits: 512_000,
                child_damage: 1_000,
                max_active_per_owner: 16,
            },
        ),
    ];
    for (kind, minimum, maximum) in cases {
        assert_ultimate_parameters_valid(&catalog, kind, minimum);
        assert_ultimate_parameters_valid(&catalog, kind, maximum);
    }
}

#[cfg(feature = "balance-lab")]
fn collect_numeric_json_pointers(
    value: &serde_json::Value,
    prefix: &str,
    pointers: &mut Vec<String>,
) {
    match value {
        serde_json::Value::Number(_) => pointers.push(prefix.to_string()),
        serde_json::Value::Array(values) => {
            for (index, value) in values.iter().enumerate() {
                collect_numeric_json_pointers(value, &format!("{prefix}/{index}"), pointers);
            }
        }
        serde_json::Value::Object(values) => {
            for (key, value) in values {
                collect_numeric_json_pointers(value, &format!("{prefix}/{key}"), pointers);
            }
        }
        serde_json::Value::Null | serde_json::Value::Bool(_) | serde_json::Value::String(_) => {}
    }
}

#[cfg(feature = "balance-lab")]
fn invalid_ultimate_upper(pointer: &str) -> Option<u64> {
    match pointer {
        "/Dash/maximum_distance_milliunits"
        | "/Dash/knockback_speed_milliunits"
        | "/Sentry/acquisition_range_milliunits"
        | "/Sentry/projectile_speed_milliunits"
        | "/Sentry/projectile_range_milliunits"
        | "/RevealScan/maximum_range_milliunits"
        | "/ConcealmentField/maximum_range_milliunits"
        | "/DemolitionStrike/maximum_range_milliunits"
        | "/ElementalField/maximum_range_milliunits"
        | "/BigBlob/maximum_range_milliunits"
        | "/BigBlob/child_speed_milliunits"
        | "/BigBlob/child_range_milliunits" => Some(4_096_001),
        "/RevealScan/radius_milliunits"
        | "/ConcealmentField/radius_milliunits"
        | "/ElementalField/radius_milliunits"
        | "/BigBlob/visual_arc_height_milliunits" => Some(2_048_001),
        "/Sentry/body_radius_milliunits"
        | "/Sentry/projectile_radius_milliunits"
        | "/BigBlob/landing_clearance_milliunits"
        | "/BigBlob/child_radius_milliunits"
        | "/BigBlob/child_explosion_radius_milliunits" => Some(512_001),
        pointer if pointer.starts_with("/Sentry/placement_offsets_milliunits/") => Some(1_024_001),
        "/DemolitionStrike/radius_milliunits" => Some(68_000),
        "/Dash/duration_ticks"
        | "/Dash/knockback_duration_ticks"
        | "/Sentry/acquisition_interval_ticks"
        | "/Sentry/projectile_lifetime_ticks"
        | "/BigBlob/flight_ticks"
        | "/BigBlob/child_lifetime_ticks" => Some(601),
        "/Sentry/fire_interval_ticks"
        | "/SelfCloak/duration_ticks"
        | "/RevealScan/reveal_ticks"
        | "/ConcealmentField/duration_ticks"
        | "/ElementalField/duration_ticks"
        | "/ElementalField/pulse_interval_ticks"
        | "/ElementalField/effect/DamageOverTime/tick_interval"
        | "/ElementalField/effect/DamageOverTime/duration_ticks"
        | "/BigBlob/child_fuse_ticks" => Some(3_601),
        "/Sentry/lifetime_ticks" => Some(36_001),
        "/Dash/damage" | "/Sentry/projectile_damage" | "/BigBlob/child_damage" => Some(1_001),
        "/Sentry/maximum_health" => Some(10_001),
        "/Dash/maximum_targets" => Some(33),
        "/BigBlob/max_active_per_owner" => Some(17),
        "/ElementalField/effect/Cold/amount"
        | "/ElementalField/effect/DamageOverTime/damage_per_tick"
        | "/ElementalField/effect/Heal/amount" => None,
        _ => panic!("missing literal upper-bound case for {pointer}"),
    }
}

#[cfg(feature = "balance-lab")]
#[test]
fn every_ultimate_numeric_field_rejects_isolated_out_of_range_mutations() {
    let (catalog, _) = catalogs();
    let mut total_numeric_fields = 0;
    for definition in &catalog.ultimates {
        let baseline = serde_json::to_value(definition.parameters).unwrap();
        let mut pointers = Vec::new();
        collect_numeric_json_pointers(&baseline, "", &mut pointers);
        for pointer in pointers {
            total_numeric_fields += 1;
            let mut below = baseline.clone();
            *below.pointer_mut(&pointer).unwrap() = serde_json::json!(0);
            let below = serde_json::from_value(below).unwrap();
            assert_ultimate_parameters_invalid(&catalog, definition.kind, below);

            let mut upper = baseline.clone();
            if let Some(invalid) = invalid_ultimate_upper(&pointer) {
                *upper.pointer_mut(&pointer).unwrap() = serde_json::json!(invalid);
                let upper = serde_json::from_value(upper).unwrap();
                assert_ultimate_parameters_invalid(&catalog, definition.kind, upper);
            } else {
                *upper.pointer_mut(&pointer).unwrap() = serde_json::json!(u16::MAX);
                let upper = serde_json::from_value(upper).unwrap();
                assert_ultimate_parameters_valid(&catalog, definition.kind, upper);
            }
        }
    }
    assert_eq!(total_numeric_fields, 68);
}

#[test]
fn sentry_and_demolition_relational_invariants_remain_validator_owned() {
    let (catalog, _) = catalogs();
    let sentry = catalog
        .ultimates
        .iter()
        .find(|definition| definition.kind == UltimateKind::Sentry)
        .unwrap()
        .parameters;
    for parameters in [
        sentry_with_relational_violation(sentry, 0),
        sentry_with_relational_violation(sentry, 1),
        sentry_with_relational_violation(sentry, 2),
    ] {
        assert_ultimate_parameters_invalid(&catalog, UltimateKind::Sentry, parameters);
    }
    assert_ultimate_parameters_invalid(
        &catalog,
        UltimateKind::DemolitionStrike,
        UltimateParameters::DemolitionStrike {
            maximum_range_milliunits: 520_000,
            radius_milliunits: 10_000,
        },
    );
}

#[test]
fn ultimate_kind_and_elemental_effect_compatibility_remain_exhaustive() {
    let (catalog, _) = catalogs();
    for definition in &catalog.ultimates {
        let wrong_kind = if definition.kind == UltimateKind::Dash {
            UltimateKind::Sentry
        } else {
            UltimateKind::Dash
        };
        let mut invalid = catalog.clone();
        invalid
            .ultimates
            .iter_mut()
            .find(|candidate| candidate.id == definition.id)
            .unwrap()
            .kind = wrong_kind;
        assert!(invalid.validate().is_err());
    }
    for (kind, effect) in [
        (
            UltimateKind::CryogenicField,
            ElementalFieldEffect::Heal { amount: 1 },
        ),
        (
            UltimateKind::FireField,
            ElementalFieldEffect::DamageOverTime {
                kind: crate::combat::DamageOverTimeKind::Poison,
                damage_per_tick: 1,
                tick_interval: 1,
                duration_ticks: 1,
            },
        ),
        (
            UltimateKind::RestorationField,
            ElementalFieldEffect::Cold { amount: 1 },
        ),
    ] {
        assert_ultimate_parameters_invalid(
            &catalog,
            kind,
            UltimateParameters::ElementalField {
                maximum_range_milliunits: 1,
                radius_milliunits: 1,
                duration_ticks: 1,
                pulse_interval_ticks: 1,
                effect,
            },
        );
    }
}

#[test]
fn elemental_timing_fields_have_no_unowned_cross_field_invariants() {
    let (catalog, _) = catalogs();
    assert_ultimate_parameters_valid(
        &catalog,
        UltimateKind::FireField,
        UltimateParameters::ElementalField {
            maximum_range_milliunits: 1,
            radius_milliunits: 1,
            duration_ticks: 1,
            pulse_interval_ticks: 3_600,
            effect: ElementalFieldEffect::DamageOverTime {
                kind: crate::combat::DamageOverTimeKind::Fire,
                damage_per_tick: u16::MAX,
                tick_interval: 3_600,
                duration_ticks: 1,
            },
        },
    );
}

#[test]
fn embedded_catalog_exposes_direct_diagnostic_policy() {
    let (builds, _) = catalogs();
    assert_eq!(
        builds.direct_diagnostic,
        definitions::DirectDiagnosticLoadoutPolicy {
            fighter_profile_id: crate::profiles::FighterProfileId(1),
            weapon_base_ids: vec![
                crate::profiles::WeaponBaseId(1),
                crate::profiles::WeaponBaseId(2),
                crate::profiles::WeaponBaseId(3),
                crate::profiles::WeaponBaseId(4),
            ],
            ultimate_id: UltimateDefinitionId(1),
            passive_ids: [PassiveDefinitionId(3), PassiveDefinitionId(4)],
        }
    );
}

#[test]
#[cfg(feature = "server")]
fn direct_diagnostic_policy_is_fingerprinted_and_cycles_stable_weapon_ids() {
    let (builds, weapons) = catalogs();
    let baseline = builds.fingerprint().unwrap();
    let mut variants = Vec::new();
    let mut changed = builds.clone();
    changed.direct_diagnostic.fighter_profile_id = crate::profiles::FighterProfileId(2);
    variants.push(changed);
    let mut changed = builds.clone();
    changed.direct_diagnostic.weapon_base_ids.rotate_left(1);
    variants.push(changed);
    let mut changed = builds.clone();
    changed.direct_diagnostic.ultimate_id = UltimateDefinitionId(2);
    variants.push(changed);
    let mut changed = builds.clone();
    changed.direct_diagnostic.passive_ids = [PassiveDefinitionId(3), PassiveDefinitionId(5)];
    variants.push(changed);
    for changed in variants {
        assert!(changed.validate_weapon_references(&weapons).is_ok());
        assert_ne!(changed.fingerprint().unwrap(), baseline);
    }

    for (player_id, expected_weapon_id) in [(1, 1), (2, 2), (3, 3), (4, 4), (5, 1), (6, 2)] {
        let loadout = resolve_direct_diagnostic_loadout(&builds, &weapons, player_id).unwrap();
        assert_eq!(
            loadout.primary_weapon.source_preset_id,
            Some(crate::combat::WeaponPresetId(expected_weapon_id))
        );
        assert_eq!(loadout.ultimate.id, UltimateDefinitionId(1));
        assert_eq!(
            loadout.passives.map(|passive| passive.id),
            [PassiveDefinitionId(3), PassiveDefinitionId(4)]
        );
    }
    let first = resolve_direct_diagnostic_loadout(&builds, &weapons, 1).unwrap();
    let historical_identity_material = postcard::to_allocvec(&(
        BUILD_FINGERPRINT_FORMAT_VERSION,
        16_u16,
        builds.balance_revision,
        1_u16,
        WeaponChoice::Preset(crate::combat::WeaponPresetId(1)),
        UltimateDefinitionId(1),
        [PassiveDefinitionId(3), PassiveDefinitionId(4)],
    ))
    .unwrap();
    assert_eq!(
        first.identity.recipe_fingerprint,
        BuildRecipeFingerprint(crate::content::fnv1a64(&historical_identity_material))
    );
    assert_eq!(
        resolve_direct_diagnostic_loadout(&builds, &weapons, 0),
        Err(BuildResolutionError::InvalidCombination)
    );
}

#[test]
fn direct_diagnostic_policy_rejects_invalid_local_and_cross_catalog_references() {
    let (builds, weapons) = catalogs();

    let mut invalid = builds.clone();
    invalid.direct_diagnostic.fighter_profile_id = crate::profiles::FighterProfileId(0);
    assert!(invalid.validate().is_err());

    let mut invalid = builds.clone();
    invalid.direct_diagnostic.weapon_base_ids.clear();
    assert!(invalid.validate().is_err());

    let mut invalid = builds.clone();
    invalid.direct_diagnostic.weapon_base_ids[0] = crate::profiles::WeaponBaseId(0);
    assert!(invalid.validate().is_err());

    let mut invalid = builds.clone();
    invalid.direct_diagnostic.weapon_base_ids =
        vec![crate::profiles::WeaponBaseId(1); definitions::MAX_DIRECT_DIAGNOSTIC_WEAPONS + 1];
    assert!(invalid.validate().is_err());

    let mut invalid = builds.clone();
    invalid.direct_diagnostic.weapon_base_ids[1] = invalid.direct_diagnostic.weapon_base_ids[0];
    assert!(invalid.validate().is_err());

    let mut invalid = builds.clone();
    invalid.direct_diagnostic.ultimate_id = UltimateDefinitionId(u16::MAX);
    assert!(invalid.validate().is_err());

    let mut invalid = builds.clone();
    invalid.direct_diagnostic.passive_ids = [PassiveDefinitionId(3), PassiveDefinitionId(3)];
    assert!(invalid.validate().is_err());

    let mut invalid = builds.clone();
    invalid.direct_diagnostic.passive_ids[0] = PassiveDefinitionId(1);
    assert!(invalid.validate().is_err());

    let mut invalid = builds.clone();
    invalid.direct_diagnostic.passive_ids[0] = PassiveDefinitionId(u16::MAX);
    assert!(invalid.validate().is_err());

    let mut invalid = builds;
    invalid.direct_diagnostic.weapon_base_ids[0] = crate::profiles::WeaponBaseId(u16::MAX);
    assert!(invalid.validate().is_ok());
    assert!(invalid.validate_weapon_references(&weapons).is_err());
}

#[test]
fn duplicate_and_frame_family_passives_are_rejected() {
    let (builds, weapons) = catalogs();
    let mut candidate = recipe(1, 1, [1, 1]);
    assert_eq!(
        resolve_build_recipe(&builds, &weapons, candidate),
        Err(BuildResolutionError::InvalidCombination)
    );
    candidate.passives = [PassiveDefinitionId(1), PassiveDefinitionId(2)];
    assert_eq!(
        resolve_build_recipe(&builds, &weapons, candidate),
        Err(BuildResolutionError::InvalidCombination)
    );
}

#[test]
fn recipes_reject_unknown_ids_and_resolve_exact_budget_and_body_stats() {
    let (builds, weapons) = catalogs();
    let controller = resolve_build_recipe(&builds, &weapons, recipe(3, 2, [5, 6])).unwrap();
    assert_eq!(controller.total_points, BUILD_POINT_BUDGET);
    assert_eq!(controller.fighter_stats.maximum_health, 1_000);
    assert!((controller.fighter_stats.movement_speed - 70.0).abs() < f32::EPSILON);
    assert_eq!(controller.fighter_stats.health_recovery_rate, 100);

    let mut unknown = recipe(1, 1, [1, 3]);
    unknown.ultimate = UltimateDefinitionId(999);
    assert_eq!(
        resolve_build_recipe(&builds, &weapons, unknown),
        Err(BuildResolutionError::UnknownId)
    );
    unknown = recipe(1, 1, [1, 3]);
    unknown.passives[1] = PassiveDefinitionId(999);
    assert_eq!(
        resolve_build_recipe(&builds, &weapons, unknown),
        Err(BuildResolutionError::UnknownId)
    );

    let runner = resolve_build_recipe(&builds, &weapons, recipe(1, 1, [1, 3])).unwrap();
    assert_eq!(runner.fighter_stats.maximum_health, 85);
    assert!((runner.fighter_stats.movement_speed - 110.0).abs() < f32::EPSILON);
    let bruiser = resolve_build_recipe(&builds, &weapons, recipe(2, 1, [2, 6])).unwrap();
    assert_eq!(bruiser.fighter_stats.maximum_health, 120);
    assert!((bruiser.fighter_stats.movement_speed - 90.0).abs() < f32::EPSILON);
}

#[test]
fn saved_fighter_elemental_baselines_are_independent_and_passives_add_to_them() {
    let (mut builds, weapons) = catalogs();
    builds.fighter_profiles.lightweight.cold_capacity = 750;
    builds
        .fighter_profiles
        .lightweight
        .cold_resistance_basis_points = 1_000;
    let resolved = resolve_saved_brawler_recipe(
        &builds,
        &weapons,
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
    let (builds, weapons) = catalogs();
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
                match resolve_build_recipe(&builds, &weapons, candidate) {
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
    let (builds, weapons) = catalogs();
    let candidate = recipe(1, 1, [1, 3]);
    let swapped = BrawlerBuildRecipe {
        passives: [candidate.passives[1], candidate.passives[0]],
        ..candidate
    };
    let first = resolve_build_recipe(&builds, &weapons, candidate).unwrap();
    let second = resolve_build_recipe(&builds, &weapons, swapped).unwrap();
    assert_eq!(
        first.identity.recipe_fingerprint,
        second.identity.recipe_fingerprint
    );
    assert_eq!(first.passives, second.passives);
}

#[test]
fn catalog_accepts_additive_inventory_and_rejects_identity_and_cost_mutations() {
    let (builds, _) = catalogs();
    let mut additive = builds.clone();
    let mut ultimate = additive.ultimates.last().unwrap().clone();
    ultimate.id = UltimateDefinitionId(12);
    ultimate.key = "alternate-big-blob".into();
    ultimate.display_name = "Alternate Big Blob".into();
    additive.ultimates.push(ultimate);
    let mut passive = additive.passives.last().unwrap().clone();
    passive.id = PassiveDefinitionId(10);
    passive.key = "alternate-heat-shielding".into();
    passive.display_name = "Alternate Heat Shielding".into();
    additive.passives.push(passive);
    assert!(additive.validate().is_ok());
    let mut invalid = builds.clone();
    invalid.ultimates[0].point_cost = BUILD_POINT_BUDGET + 1;
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
    let mut invalid = builds.clone();
    invalid.ultimate_charge.maximum = 0;
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    let UltimateParameters::Sentry {
        projectile_range_milliunits,
        ..
    } = &mut invalid.ultimates[1].parameters
    else {
        panic!("canonical second ultimate must be Sentry")
    };
    *projectile_range_milliunits = 481_000;
    assert!(invalid.validate().is_err());
    let mut invalid = builds.clone();
    invalid.passives[2].parameters = PassiveParameters::QuickCycle {
        refill_duration_basis_points: 6_000,
    };
    assert!(invalid.validate().is_err());
    let mut invalid = builds;
    invalid.passives[0].key = "Upper_Case".into();
    assert!(invalid.validate().is_err());
}

#[test]
#[allow(
    clippy::too_many_lines,
    reason = "one boundary matrix keeps every passive range and relational invariant visible"
)]
fn passive_parameter_bounds_and_cross_field_invariants_are_enforced() {
    let (builds, _) = catalogs();
    let replace = |catalog: &mut BuildCatalog, kind: PassiveKind, parameters| {
        catalog
            .passives
            .iter_mut()
            .find(|definition| definition.kind == kind)
            .expect("embedded passive family exists")
            .parameters = parameters;
    };

    let mut boundary = builds.clone();
    replace(
        &mut boundary,
        PassiveKind::AdrenalResponse,
        PassiveParameters::AdrenalResponse {
            duration_ticks: PASSIVE_ADRENAL_DURATION_TICKS_BOUNDS.minimum,
            rearm_ticks: PASSIVE_ADRENAL_REARM_TICKS_BOUNDS.maximum,
            movement_bonus_basis_points: PASSIVE_ADRENAL_MOVEMENT_BONUS_BASIS_POINTS_BOUNDS.maximum,
        },
    );
    replace(
        &mut boundary,
        PassiveKind::CloseQuarters,
        PassiveParameters::CloseQuarters {
            near_distance_milliunits: PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.minimum,
            far_distance_milliunits: PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.maximum,
            near_damage_basis_points: PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.maximum,
            far_damage_basis_points: PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.minimum,
        },
    );
    replace(
        &mut boundary,
        PassiveKind::QuickCycle,
        PassiveParameters::QuickCycle {
            refill_duration_basis_points: PASSIVE_QUICK_CYCLE_REFILL_BASIS_POINTS_BOUNDS.maximum,
        },
    );
    replace(
        &mut boundary,
        PassiveKind::Tenacity,
        PassiveParameters::Tenacity {
            slow_duration_basis_points: PASSIVE_TENACITY_SLOW_BASIS_POINTS_BOUNDS.minimum,
        },
    );
    for (kind, parameters) in [
        (
            PassiveKind::CryogenicInsulation,
            PassiveParameters::CryogenicInsulation {
                resistance_basis_points: PASSIVE_ELEMENTAL_RESISTANCE_BASIS_POINTS_BOUNDS.maximum,
            },
        ),
        (
            PassiveKind::FilteredCirculation,
            PassiveParameters::FilteredCirculation {
                resistance_basis_points: PASSIVE_ELEMENTAL_RESISTANCE_BASIS_POINTS_BOUNDS.maximum,
            },
        ),
        (
            PassiveKind::HeatShielding,
            PassiveParameters::HeatShielding {
                resistance_basis_points: PASSIVE_ELEMENTAL_RESISTANCE_BASIS_POINTS_BOUNDS.maximum,
            },
        ),
    ] {
        replace(&mut boundary, kind, parameters);
    }
    assert!(boundary.validate().is_ok());

    let adrenal = |duration_ticks, rearm_ticks, movement_bonus_basis_points| {
        PassiveParameters::AdrenalResponse {
            duration_ticks,
            rearm_ticks,
            movement_bonus_basis_points,
        }
    };
    let close_quarters = |near_distance_milliunits,
                          far_distance_milliunits,
                          near_damage_basis_points,
                          far_damage_basis_points| {
        PassiveParameters::CloseQuarters {
            near_distance_milliunits,
            far_distance_milliunits,
            near_damage_basis_points,
            far_damage_basis_points,
        }
    };
    let assert_invalid = |label: &str, kind, parameters| {
        let mut invalid = builds.clone();
        replace(&mut invalid, kind, parameters);
        assert!(
            invalid.validate().is_err(),
            "{label} unexpectedly validated"
        );
    };
    for (label, kind, parameters) in [
        (
            "Adrenal duration below minimum",
            PassiveKind::AdrenalResponse,
            adrenal(
                PASSIVE_ADRENAL_DURATION_TICKS_BOUNDS.minimum - 1,
                PASSIVE_ADRENAL_REARM_TICKS_BOUNDS.maximum,
                PASSIVE_ADRENAL_MOVEMENT_BONUS_BASIS_POINTS_BOUNDS.minimum,
            ),
        ),
        (
            "Adrenal duration above maximum",
            PassiveKind::AdrenalResponse,
            adrenal(
                PASSIVE_ADRENAL_DURATION_TICKS_BOUNDS.maximum + 1,
                PASSIVE_ADRENAL_REARM_TICKS_BOUNDS.maximum,
                PASSIVE_ADRENAL_MOVEMENT_BONUS_BASIS_POINTS_BOUNDS.minimum,
            ),
        ),
        (
            "Adrenal rearm below minimum",
            PassiveKind::AdrenalResponse,
            adrenal(
                PASSIVE_ADRENAL_DURATION_TICKS_BOUNDS.minimum,
                PASSIVE_ADRENAL_REARM_TICKS_BOUNDS.minimum - 1,
                PASSIVE_ADRENAL_MOVEMENT_BONUS_BASIS_POINTS_BOUNDS.minimum,
            ),
        ),
        (
            "Adrenal rearm above maximum",
            PassiveKind::AdrenalResponse,
            adrenal(
                PASSIVE_ADRENAL_DURATION_TICKS_BOUNDS.minimum,
                PASSIVE_ADRENAL_REARM_TICKS_BOUNDS.maximum + 1,
                PASSIVE_ADRENAL_MOVEMENT_BONUS_BASIS_POINTS_BOUNDS.minimum,
            ),
        ),
        (
            "Adrenal movement below minimum",
            PassiveKind::AdrenalResponse,
            adrenal(
                PASSIVE_ADRENAL_DURATION_TICKS_BOUNDS.minimum,
                PASSIVE_ADRENAL_REARM_TICKS_BOUNDS.maximum,
                PASSIVE_ADRENAL_MOVEMENT_BONUS_BASIS_POINTS_BOUNDS.minimum - 1,
            ),
        ),
        (
            "Adrenal movement above maximum",
            PassiveKind::AdrenalResponse,
            adrenal(
                PASSIVE_ADRENAL_DURATION_TICKS_BOUNDS.minimum,
                PASSIVE_ADRENAL_REARM_TICKS_BOUNDS.maximum,
                PASSIVE_ADRENAL_MOVEMENT_BONUS_BASIS_POINTS_BOUNDS.maximum + 1,
            ),
        ),
        (
            "Close Quarters near distance below minimum",
            PassiveKind::CloseQuarters,
            close_quarters(
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.minimum - 1,
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.maximum,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.maximum,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.minimum,
            ),
        ),
        (
            "Close Quarters near distance above maximum",
            PassiveKind::CloseQuarters,
            close_quarters(
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.maximum + 1,
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.maximum,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.maximum,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.minimum,
            ),
        ),
        (
            "Close Quarters far distance below minimum",
            PassiveKind::CloseQuarters,
            close_quarters(
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.minimum,
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.minimum - 1,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.maximum,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.minimum,
            ),
        ),
        (
            "Close Quarters far distance above maximum",
            PassiveKind::CloseQuarters,
            close_quarters(
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.minimum,
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.maximum + 1,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.maximum,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.minimum,
            ),
        ),
        (
            "Close Quarters near damage below minimum",
            PassiveKind::CloseQuarters,
            close_quarters(
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.minimum,
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.maximum,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.minimum - 1,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.minimum,
            ),
        ),
        (
            "Close Quarters near damage above maximum",
            PassiveKind::CloseQuarters,
            close_quarters(
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.minimum,
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.maximum,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.maximum + 1,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.minimum,
            ),
        ),
        (
            "Close Quarters far damage below minimum",
            PassiveKind::CloseQuarters,
            close_quarters(
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.minimum,
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.maximum,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.maximum,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.minimum - 1,
            ),
        ),
        (
            "Close Quarters far damage above maximum",
            PassiveKind::CloseQuarters,
            close_quarters(
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.minimum,
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.maximum,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.maximum,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.maximum + 1,
            ),
        ),
        (
            "Quick Cycle below minimum",
            PassiveKind::QuickCycle,
            PassiveParameters::QuickCycle {
                refill_duration_basis_points: PASSIVE_QUICK_CYCLE_REFILL_BASIS_POINTS_BOUNDS
                    .minimum
                    - 1,
            },
        ),
        (
            "Quick Cycle above maximum",
            PassiveKind::QuickCycle,
            PassiveParameters::QuickCycle {
                refill_duration_basis_points: PASSIVE_QUICK_CYCLE_REFILL_BASIS_POINTS_BOUNDS
                    .maximum
                    + 1,
            },
        ),
        (
            "Tenacity below minimum",
            PassiveKind::Tenacity,
            PassiveParameters::Tenacity {
                slow_duration_basis_points: PASSIVE_TENACITY_SLOW_BASIS_POINTS_BOUNDS.minimum - 1,
            },
        ),
        (
            "Tenacity above maximum",
            PassiveKind::Tenacity,
            PassiveParameters::Tenacity {
                slow_duration_basis_points: PASSIVE_TENACITY_SLOW_BASIS_POINTS_BOUNDS.maximum + 1,
            },
        ),
        (
            "Cryogenic resistance below minimum",
            PassiveKind::CryogenicInsulation,
            PassiveParameters::CryogenicInsulation {
                resistance_basis_points: PASSIVE_ELEMENTAL_RESISTANCE_BASIS_POINTS_BOUNDS.minimum
                    - 1,
            },
        ),
        (
            "Cryogenic resistance above maximum",
            PassiveKind::CryogenicInsulation,
            PassiveParameters::CryogenicInsulation {
                resistance_basis_points: PASSIVE_ELEMENTAL_RESISTANCE_BASIS_POINTS_BOUNDS.maximum
                    + 1,
            },
        ),
        (
            "Filtered resistance below minimum",
            PassiveKind::FilteredCirculation,
            PassiveParameters::FilteredCirculation {
                resistance_basis_points: PASSIVE_ELEMENTAL_RESISTANCE_BASIS_POINTS_BOUNDS.minimum
                    - 1,
            },
        ),
        (
            "Filtered resistance above maximum",
            PassiveKind::FilteredCirculation,
            PassiveParameters::FilteredCirculation {
                resistance_basis_points: PASSIVE_ELEMENTAL_RESISTANCE_BASIS_POINTS_BOUNDS.maximum
                    + 1,
            },
        ),
        (
            "Heat resistance below minimum",
            PassiveKind::HeatShielding,
            PassiveParameters::HeatShielding {
                resistance_basis_points: PASSIVE_ELEMENTAL_RESISTANCE_BASIS_POINTS_BOUNDS.minimum
                    - 1,
            },
        ),
        (
            "Heat resistance above maximum",
            PassiveKind::HeatShielding,
            PassiveParameters::HeatShielding {
                resistance_basis_points: PASSIVE_ELEMENTAL_RESISTANCE_BASIS_POINTS_BOUNDS.maximum
                    + 1,
            },
        ),
        (
            "Adrenal rearm precedes duration",
            PassiveKind::AdrenalResponse,
            adrenal(
                PASSIVE_ADRENAL_DURATION_TICKS_BOUNDS.minimum + 1,
                PASSIVE_ADRENAL_REARM_TICKS_BOUNDS.minimum,
                PASSIVE_ADRENAL_MOVEMENT_BONUS_BASIS_POINTS_BOUNDS.minimum,
            ),
        ),
        (
            "Close Quarters distances are equal",
            PassiveKind::CloseQuarters,
            close_quarters(
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.minimum,
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.minimum,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.maximum,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.minimum,
            ),
        ),
        (
            "Close Quarters damage is equal",
            PassiveKind::CloseQuarters,
            close_quarters(
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.minimum,
                PASSIVE_CLOSE_QUARTERS_DISTANCE_MILLIUNITS_BOUNDS.maximum,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.minimum,
                PASSIVE_CLOSE_QUARTERS_DAMAGE_BASIS_POINTS_BOUNDS.minimum,
            ),
        ),
    ] {
        assert_invalid(label, kind, parameters);
    }
}

#[test]
fn weapon_costs_must_exactly_cover_an_additive_weapon_catalog() {
    let (mut builds, mut weapons) = catalogs();
    let mut eighth = weapons.presets.last().unwrap().clone();
    eighth.id = crate::combat::WeaponPresetId(8);
    eighth.key = "eighth-preset".into();
    eighth.display_name = "Eighth Preset".into();
    weapons.presets.push(eighth);
    builds.weapon_costs.push(WeaponPointCost {
        weapon_id: crate::combat::WeaponPresetId(8),
        point_cost: 4,
    });

    assert!(weapons.validate().is_ok());
    assert!(builds.validate_weapon_references(&weapons).is_ok());

    builds.weapon_costs.pop();
    assert!(builds.validate_weapon_references(&weapons).is_err());
}

#[test]
fn additive_build_catalog_still_enforces_inventory_ceiling() {
    let (mut builds, _) = catalogs();
    let template = builds.ultimates.last().unwrap().clone();
    for id in 12..=u16::try_from(MAX_ULTIMATE_DEFINITIONS).unwrap() {
        let mut definition = template.clone();
        definition.id = UltimateDefinitionId(id);
        definition.key = format!("ultimate-{id}");
        definition.display_name = format!("Ultimate {id}");
        builds.ultimates.push(definition);
    }
    assert!(builds.validate().is_ok());

    let id = u16::try_from(MAX_ULTIMATE_DEFINITIONS + 1).unwrap();
    let mut definition = template;
    definition.id = UltimateDefinitionId(id);
    definition.key = format!("ultimate-{id}");
    definition.display_name = format!("Ultimate {id}");
    builds.ultimates.push(definition);
    assert!(builds.validate().is_err());
}

#[test]
fn catalog_rejects_non_finite_and_out_of_policy_balance_values() {
    let (builds, _) = catalogs();
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
    let (builds, weapons) = catalogs();
    for (candidate, expected) in [
        (recipe(1, 1, [6, 3]), Ok(10)),
        (recipe(1, 2, [6, 3]), Ok(11)),
        (recipe(2, 2, [6, 3]), Ok(12)),
        (recipe(2, 2, [3, 4]), Err(BuildResolutionError::OverBudget)),
    ] {
        assert_eq!(
            resolve_build_recipe(&builds, &weapons, candidate)
                .map(|resolved| resolved.total_points),
            expected
        );
    }

    let mut overflow = builds.clone();
    overflow.passives[2].point_cost = u8::MAX;
    assert!(matches!(
        resolve_build_recipe(&overflow, &weapons, recipe(2, 2, [3, 4])),
        Err(BuildResolutionError::OverBudget)
    ));
}
