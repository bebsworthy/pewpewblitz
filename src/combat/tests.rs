//! Focused shared combat model and catalog tests.

use super::*;

#[test]
fn authored_catalogs_validate_and_have_expected_values() {
    let fighters = FighterDefinitions::default();
    let weapons = WeaponDefinitions::default();
    assert!(fighters.validate(&weapons).is_ok());
    assert!(weapons.validate(&fighters).is_ok());
    assert_eq!(
        fighters
            .get(STANDARD_FIGHTER_DEFINITION)
            .unwrap()
            .maximum_health,
        100
    );
    assert!(
        (fighters
            .get(STANDARD_FIGHTER_DEFINITION)
            .unwrap()
            .movement_speed
            - 100.0)
            .abs()
            < f32::EPSILON
    );
    assert_eq!(
        weapons
            .get(PULSE_SIDEARM_DEFINITION)
            .unwrap()
            .magazine_capacity,
        6
    );
}

#[test]
fn catalog_validation_rejects_duplicate_and_unsafe_values() {
    let mut fighters = FighterDefinitions::default();
    fighters.entries.push(fighters.entries[0]);
    assert!(fighters.validate(&WeaponDefinitions::default()).is_err());
    fighters.entries.pop();
    fighters.entries[0].movement_speed = f32::NAN;
    assert!(fighters.validate(&WeaponDefinitions::default()).is_err());
    assert!(
        FighterDefinitions {
            entries: Vec::new()
        }
        .validate(&WeaponDefinitions::default())
        .is_err()
    );
    let mut weapons = WeaponDefinitions::default();
    weapons.entries[0].muzzle_offset = 1.0;
    assert!(weapons.validate(&FighterDefinitions::default()).is_err());
    weapons.entries[0].maximum_range = 0.0;
    assert!(weapons.validate(&FighterDefinitions::default()).is_err());
    assert!(
        WeaponDefinitions {
            entries: Vec::new()
        }
        .validate(&FighterDefinitions::default())
        .is_err()
    );
}

#[test]
fn projectile_body_validates_and_preserves_authored_circle_geometry() {
    let body = ProjectileBody::circle(6.0);
    assert!(body.shape.is_valid());
    assert!((body.shape.bounding_radius() - 6.0).abs() < f32::EPSILON);
    assert!(!ProjectileBody::circle(0.0).shape.is_valid());
    assert!(!ProjectileBody::circle(-1.0).shape.is_valid());
    assert!(!ProjectileBody::circle(f32::NAN).shape.is_valid());
}

#[test]
fn combat_cue_evidence_encoding_round_trips_full_payload() {
    let cue = CombatCue::Impact {
        event_id: CombatEventId(7),
        tick: 42,
        source: NetworkEntityId(11),
        shot_id: ShotId(13),
        weapon_definition_id: PULSE_SIDEARM_DEFINITION,
        target: Some(NetworkEntityId(17)),
        position: WorldPoint { x: 1.5, y: -2.5 },
        normal: WorldPoint { x: -1.0, y: 0.0 },
        distance_band: DistanceBand::Mid,
    };

    let encoded = encode_combat_cue(&cue);
    assert_eq!(decode_combat_cue(&encoded), Some(cue));
    assert!(decode_combat_cue("abc").is_none());
}

#[cfg(feature = "server")]
#[test]
fn fire_economy_boundaries_are_integer_and_deterministic() {
    let fighters = FighterDefinitions::default();
    let fighter = fighters
        .get(STANDARD_FIGHTER_DEFINITION)
        .expect("standard fighter definition");
    let recipe = WeaponCatalog::embedded()
        .expect("embedded catalog")
        .resolve_preset(WeaponPresetId(1), fighter)
        .expect("pulse preset")
        .recipe;
    let mut state = WeaponState {
        ammo: 1,
        phase: WeaponPhase::Ready,
        ammo_recovery: None,
    };
    state.ammo -= 1;
    state.ammo_recovery = Some(AmmoRecovery {
        started_at_tick: 1,
        ready_at_tick: 61,
    });
    assert_eq!(state.ammo, 0);
    advance_composed_weapon_state(&mut state, &recipe, 60);
    assert_eq!(state.ammo, 0);
    assert!(state.ammo_recovery.is_some());
    advance_composed_weapon_state(&mut state, &recipe, 61);
    assert_eq!(state.ammo, 1);
    assert_eq!(state.phase, WeaponPhase::Ready);
    assert_eq!(state.ammo_recovery, None);
    state.phase = WeaponPhase::Cooldown { ready_at_tick: 73 };
    advance_composed_weapon_state(&mut state, &recipe, 72);
    assert_eq!(state.phase, WeaponPhase::Cooldown { ready_at_tick: 73 });
    advance_composed_weapon_state(&mut state, &recipe, 73);
    assert_eq!(state.phase, WeaponPhase::Ready);
}

#[cfg(feature = "server")]
#[test]
fn fighter_runtime_reads_health_and_ammo_from_selected_definitions() {
    let mut fighters = FighterDefinitions::default();
    fighters.entries[0].maximum_health = 77;
    let mut weapons = WeaponDefinitions::default();
    weapons.entries[0].magazine_capacity = 3;

    let (_, _, health, weapon) = default_fighter_runtime(TeamId(4), &fighters, &weapons);

    assert_eq!(health, CurrentHealth(77));
    assert_eq!(weapon.ammo, 3);
}

#[test]
fn allocators_are_monotonic_and_reject_exhaustion() {
    let mut ids = NextCombatIds::default();
    assert_eq!(ids.allocate_shot(), Some(ShotId(1)));
    assert_eq!(ids.allocate_event(), Some(CombatEventId(1)));
    ids.next_shot_id = u64::MAX;
    assert_eq!(ids.allocate_shot(), None);
    assert_eq!(ids.next_shot_id, u64::MAX);
}

#[test]
fn lethal_event_pair_reservation_is_atomic_at_exhaustion() {
    let mut ids = NextCombatIds {
        next_attack_id: 1,
        next_shot_id: 1,
        next_event_id: u64::MAX - 1,
    };
    assert_eq!(ids.allocate_event_pair(), None);
    assert_eq!(ids.next_event_id, u64::MAX - 1);
    assert_eq!(ids.allocate_event(), Some(CombatEventId(u64::MAX - 1)));
}

#[test]
fn neutral_entities_are_hostile_to_every_team() {
    assert!(teams_are_hostile(NEUTRAL_TEAM, NEUTRAL_TEAM));
    assert!(teams_are_hostile(NEUTRAL_TEAM, TeamId(0)));
    assert!(teams_are_hostile(TeamId(0), NEUTRAL_TEAM));
    assert!(teams_are_hostile(TeamId(0), TeamId(1)));
    assert!(!teams_are_hostile(TeamId(0), TeamId(0)));
}

#[test]
fn diagnostic_record_history_is_bounded() {
    let mut telemetry = CombatTelemetry::default();
    for index in 0..(MAX_COMBAT_RECORDS + 32) {
        telemetry.record(CombatLogRecord::Shot {
            event_id: CombatEventId(index as u64 + 1),
            tick: index as u64,
            shot_id: ShotId(index as u64 + 1),
            source: NetworkEntityId(1),
            weapon: PULSE_SIDEARM_DEFINITION,
            muzzle_position: WorldPoint { x: 0.0, y: 0.0 },
            ammo_after: 5,
        });
    }
    assert_eq!(telemetry.records.len(), MAX_COMBAT_RECORDS);
}

#[test]
fn distance_bands_follow_the_authored_boundaries() {
    assert_eq!(distance_band(249.9), DistanceBand::Close);
    assert_eq!(distance_band(250.0), DistanceBand::Mid);
    assert_eq!(distance_band(599.9), DistanceBand::Mid);
    assert_eq!(distance_band(600.0), DistanceBand::Long);
}

#[cfg(feature = "server")]
#[test]
fn muzzle_position_is_finite_and_follows_authoritative_facing() {
    let position = muzzle_position(Vec2::new(10.0, -5.0), 0.0, 34.0);
    assert_eq!(position, Vec2::new(44.0, -5.0));
    assert!(muzzle_position(Vec2::ZERO, std::f32::consts::FRAC_PI_2, 34.0).is_finite());
}

#[cfg(feature = "server")]
#[test]
fn reset_deadline_is_inactive_before_and_active_at_the_deadline() {
    assert!(!reset_is_due(89, 90));
    assert!(reset_is_due(90, 90));
}
