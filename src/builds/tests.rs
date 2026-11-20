use super::*;
use crate::combat::WeaponCatalog;

fn catalogs() -> (BuildCatalog, WeaponCatalog) {
    (
        BuildCatalog::embedded().unwrap(),
        WeaponCatalog::embedded().unwrap(),
    )
}

fn stats() -> ResolvedFighterStats {
    ResolvedFighterStats {
        maximum_health: 1_000,
        movement_speed: 70.0,
        health_recovery_rate: 100,
        idle_attack_delay_ticks: 180,
        reveal_proximity_radius: 160.0,
        cold_capacity: 1_000,
        cold_resistance_basis_points: 0,
        poison_resistance_basis_points: 0,
        fire_resistance_basis_points: 0,
    }
}

fn recipe(weapon: u16, ultimate: u16, passive: u16) -> LoadoutRecipe {
    LoadoutRecipe {
        character_id: crate::characters::CharacterId(1),
        fighter_stats: stats(),
        weapon_base: crate::combat::WeaponPresetId(weapon),
        weapon_modifiers: crate::weapon_parts::CanonicalWeaponModifiers::default(),
        ultimate: UltimateDefinitionId(ultimate),
        passive: PassiveDefinitionId(passive),
    }
}

#[test]
fn runtime_projection_installs_the_exact_resolved_ability_capabilities() {
    let (builds, weapons) = catalogs();
    let loadout = resolve_loadout(&builds, &weapons, recipe(1, 6, 3)).unwrap();
    let projection = MatchLoadoutProjection::new(&loadout, builds.fighter_body);

    assert_eq!(projection.ultimate, loadout.ultimate);
    assert_eq!(projection.passives.passives, loadout.passives);
    assert_eq!(loadout.character_id, crate::characters::CharacterId(1));
}

#[test]
fn embedded_catalog_exposes_current_authored_inventory_and_ultimate_parameters() {
    let (builds, _) = catalogs();
    assert_eq!(builds.balance_revision, BuildRevision(13));
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
fn direct_diagnostic_picks_are_validated_and_fingerprinted() {
    let (builds, _) = catalogs();
    let baseline = builds.fingerprint().unwrap();
    assert_eq!(builds.direct_diagnostic_picks().len(), 4);
    let mut rotated = builds.clone();
    rotated.direct_diagnostic.picks.rotate_left(1);
    assert!(rotated.validate().is_ok());
    assert_ne!(rotated.fingerprint().unwrap(), baseline);

    let mut empty = builds.clone();
    empty.direct_diagnostic.picks.clear();
    assert!(empty.validate().is_err());
    let mut too_many = builds.clone();
    too_many.direct_diagnostic.picks =
        vec![builds.direct_diagnostic.picks[0]; definitions::MAX_DIRECT_DIAGNOSTIC_PICKS + 1];
    assert!(too_many.validate().is_err());
    let mut unknown_ultimate = builds.clone();
    unknown_ultimate.direct_diagnostic.picks[0].ultimate_id = UltimateDefinitionId(u16::MAX);
    assert!(unknown_ultimate.validate().is_err());
    let mut unknown_passive = builds;
    unknown_passive.direct_diagnostic.picks[0].passive_id = PassiveDefinitionId(u16::MAX);
    assert!(unknown_passive.validate().is_err());
}

#[test]
fn loadouts_reject_unknown_ids_and_keep_the_character_stats() {
    let (builds, weapons) = catalogs();
    let resolved = resolve_loadout(&builds, &weapons, recipe(3, 2, 5)).unwrap();
    assert_eq!(resolved.fighter_stats, stats());
    assert_eq!(resolved.total_points, 0);
    assert_eq!(
        resolved.primary_weapon.source_preset_id,
        Some(crate::combat::WeaponPresetId(3))
    );
    assert_eq!(
        resolved.passives.map(|passive| passive.id),
        [PassiveDefinitionId(5)]
    );

    for (candidate, expected) in [
        (recipe(1, 999, 3), BuildResolutionError::UnknownId),
        (recipe(1, 1, 999), BuildResolutionError::UnknownId),
        (recipe(999, 1, 3), BuildResolutionError::InvalidCombination),
    ] {
        assert_eq!(resolve_loadout(&builds, &weapons, candidate), Err(expected));
    }
    let mut invalid_stats = recipe(1, 1, 3);
    invalid_stats.fighter_stats.maximum_health = 0;
    assert_eq!(
        resolve_loadout(&builds, &weapons, invalid_stats),
        Err(BuildResolutionError::InvalidCombination)
    );
}

#[test]
fn frame_passives_scale_the_characters_own_health_and_speed() {
    let (builds, weapons) = catalogs();
    let lightweight = resolve_loadout(&builds, &weapons, recipe(1, 1, 1)).unwrap();
    assert_eq!(lightweight.fighter_stats.maximum_health, 850);
    assert!((lightweight.fighter_stats.movement_speed - 77.0).abs() < 1e-4);
    let reinforced = resolve_loadout(&builds, &weapons, recipe(1, 1, 2)).unwrap();
    assert_eq!(reinforced.fighter_stats.maximum_health, 1_200);
    assert!((reinforced.fighter_stats.movement_speed - 63.0).abs() < 1e-4);

    let mut out_of_bounds = builds;
    out_of_bounds.passives[0].parameters = PassiveParameters::LightweightFrame {
        maximum_health_basis_points: PASSIVE_FRAME_HEALTH_BASIS_POINTS_BOUNDS.minimum - 1,
        movement_speed_basis_points: 10_000,
    };
    assert!(out_of_bounds.validate().is_err());
}

#[test]
fn resistance_passives_add_to_the_characters_baseline_and_clamp() {
    let (builds, weapons) = catalogs();
    let mut candidate = recipe(1, 1, 7);
    candidate.fighter_stats.cold_capacity = 750;
    candidate.fighter_stats.cold_resistance_basis_points = 1_000;
    let resolved = resolve_loadout(&builds, &weapons, candidate).unwrap();
    assert_eq!(resolved.fighter_stats.cold_capacity, 750);
    assert_eq!(resolved.fighter_stats.cold_resistance_basis_points, 4_000);

    candidate.fighter_stats.cold_resistance_basis_points = 5_000;
    let clamped = resolve_loadout(&builds, &weapons, candidate).unwrap();
    assert_eq!(clamped.fighter_stats.cold_resistance_basis_points, 6_000);
}

#[test]
fn every_recipe_input_changes_the_build_identity() {
    let (builds, weapons) = catalogs();
    let baseline = resolve_loadout(&builds, &weapons, recipe(1, 1, 3))
        .unwrap()
        .identity;
    let mut variants = vec![recipe(2, 1, 3), recipe(1, 3, 3), recipe(1, 1, 5)];
    let mut other_character = recipe(1, 1, 3);
    other_character.character_id = crate::characters::CharacterId(6);
    variants.push(other_character);
    for candidate in variants {
        assert_ne!(
            resolve_loadout(&builds, &weapons, candidate)
                .unwrap()
                .identity,
            baseline
        );
    }
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
fn fighter_stats_reject_non_finite_and_out_of_policy_values() {
    let mut valid = stats();
    valid.maximum_health = u16::MAX;
    valid.movement_speed = 0.5;
    assert!(validate_fighter_stats(&valid).is_ok());

    let invalid: [fn(&mut ResolvedFighterStats); 8] = [
        |stats| stats.movement_speed = f32::NAN,
        |stats| stats.movement_speed = 0.0,
        |stats| stats.movement_speed = MAX_FIGHTER_MOVEMENT_SPEED + 0.1,
        |stats| stats.maximum_health = 0,
        |stats| stats.cold_capacity = 0,
        |stats| stats.cold_capacity = MAX_COLD_CAPACITY + 1,
        |stats| stats.reveal_proximity_radius = f32::NAN,
        |stats| stats.reveal_proximity_radius = 0.0,
    ];
    for mutate in invalid {
        let mut candidate = stats();
        mutate(&mut candidate);
        assert!(validate_fighter_stats(&candidate).is_err());
    }
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
