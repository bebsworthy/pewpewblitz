use super::*;
use crate::{combat::TeamId, map::SpawnPointId, protocol::PlayerId};
use bevy::prelude::Vec2;

#[test]
fn production_rules_match_the_approved_contract() {
    let rules = WipeoutRules::default().validate().unwrap();
    assert_eq!(rules.target_score, 10);
    assert_eq!(rules.countdown_ticks, 180);
    assert_eq!(rules.active_limit_ticks, 10_800);
    assert_eq!(rules.respawn_delay_ticks, 180);
    assert_eq!(rules.spawn_protection_ticks, 90);
    assert_eq!(rules.completed_input_lock_ticks, 60);
}

#[test]
fn rules_reject_zero_invalid_capacity_and_overflow() {
    assert!(
        WipeoutRules {
            target_score: 0,
            ..WipeoutRules::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        WipeoutRules {
            minimum_participants_per_team: 2,
            maximum_participants_per_team: 1,
            ..WipeoutRules::default()
        }
        .validate()
        .is_err()
    );
    assert!(
        WipeoutRules {
            countdown_ticks: u64::MAX,
            active_limit_ticks: 1,
            ..WipeoutRules::default()
        }
        .validate()
        .is_err()
    );
}

#[test]
fn team_assignment_balances_and_caps() {
    assert_eq!(assigned_team([0, 0], 2), Some(TeamId(0)));
    assert_eq!(assigned_team([1, 0], 2), Some(TeamId(1)));
    assert_eq!(assigned_team([1, 1], 2), Some(TeamId(0)));
    assert_eq!(assigned_team([2, 1], 2), Some(TeamId(1)));
    assert_eq!(assigned_team([2, 2], 2), None);
}

#[test]
fn spawn_selection_prefers_clear_distance_then_stable_id() {
    let candidates = vec![
        SpawnCandidate {
            id: SpawnPointId(2),
            position: Vec2::new(-8.0, 0.0),
            facing: 0.0,
        },
        SpawnCandidate {
            id: SpawnPointId(1),
            position: Vec2::new(-10.0, 0.0),
            facing: 0.0,
        },
        SpawnCandidate {
            id: SpawnPointId(3),
            position: Vec2::new(1.0, 0.0),
            facing: 0.0,
        },
    ];
    let selected = select_spawn(
        candidates,
        &[(TeamId(1), Vec2::ZERO)],
        TeamId(0),
        2.0,
        MatchId(1),
        PlayerId(1),
        0,
    )
    .unwrap();
    assert_eq!(selected.id, SpawnPointId(1));
}

#[test]
fn spawn_selection_is_order_independent_cycles_and_falls_back_safely() {
    let candidates = vec![
        SpawnCandidate {
            id: SpawnPointId(3),
            position: Vec2::new(30.0, 0.0),
            facing: 0.0,
        },
        SpawnCandidate {
            id: SpawnPointId(1),
            position: Vec2::new(10.0, 0.0),
            facing: 0.0,
        },
        SpawnCandidate {
            id: SpawnPointId(2),
            position: Vec2::new(20.0, 0.0),
            facing: 0.0,
        },
    ];
    let mut reversed = candidates.clone();
    reversed.reverse();
    let living = [(TeamId(1), Vec2::ZERO)];
    let selected = select_spawn(
        candidates.clone(),
        &living,
        TeamId(0),
        1.0,
        MatchId(4),
        PlayerId(2),
        0,
    );
    assert_eq!(
        selected,
        select_spawn(
            reversed,
            &living,
            TeamId(0),
            1.0,
            MatchId(4),
            PlayerId(2),
            0,
        )
    );
    assert_eq!(selected.unwrap().id, SpawnPointId(3));

    let first = select_spawn(
        candidates.clone(),
        &[],
        TeamId(0),
        1.0,
        MatchId(1),
        PlayerId(1),
        0,
    )
    .unwrap();
    let second = select_spawn(
        candidates.clone(),
        &[],
        TeamId(0),
        1.0,
        MatchId(1),
        PlayerId(1),
        1,
    )
    .unwrap();
    assert_ne!(first.id, second.id);

    let occupied: Vec<_> = candidates
        .iter()
        .map(|candidate| (TeamId(0), candidate.position))
        .collect();
    assert!(
        select_spawn(
            candidates,
            &occupied,
            TeamId(0),
            100.0,
            MatchId(1),
            PlayerId(1),
            0,
        )
        .is_some()
    );
    assert!(
        select_spawn(
            vec![SpawnCandidate {
                id: SpawnPointId(1),
                position: Vec2::new(f32::NAN, 0.0),
                facing: 0.0,
            }],
            &[],
            TeamId(0),
            1.0,
            MatchId(1),
            PlayerId(1),
            0,
        )
        .is_none()
    );
}

#[test]
fn simultaneous_threshold_is_a_draw() {
    assert_eq!(score_result([10, 10], 10), Some(MatchResult::Draw));
    assert_eq!(timeout_result([3, 3]), MatchResult::Draw);
}

#[test]
#[cfg(feature = "server")]
fn defeat_credit_rejects_self_allied_environmental_and_invalid_sources() {
    use crate::combat::{
        AttackId, CombatEventId, CombatOutcomeFact, CombatOutcomeKind, WorldPoint,
    };
    use crate::protocol::NetworkEntityId;

    let hostile = CombatOutcomeFact {
        event_id: CombatEventId(1),
        tick: 1,
        attack_id: AttackId(1),
        source_player: Some(PlayerId(1)),
        source_network_id: Some(NetworkEntityId(1)),
        source_team: Some(TeamId(0)),
        target_network_id: NetworkEntityId(2),
        target_team: TeamId(1),
        preset_id: None,
        recipe_fingerprint: None,
        position: WorldPoint { x: 0.0, y: 0.0 },
        engagement_distance: 10.0,
        kind: CombatOutcomeKind::Defeat,
    };
    assert_eq!(credited_defeat_team(&hostile, TeamId(1)), Some(TeamId(0)));
    assert_eq!(
        credited_defeat_team(
            &CombatOutcomeFact {
                source_player: None,
                ..hostile
            },
            TeamId(1),
        ),
        Some(TeamId(0)),
        "stable source attribution remains valid after source disconnect"
    );
    assert_eq!(
        credited_defeat_team(
            &CombatOutcomeFact {
                source_network_id: Some(NetworkEntityId(2)),
                ..hostile
            },
            TeamId(1),
        ),
        None
    );
    assert_eq!(
        credited_defeat_team(
            &CombatOutcomeFact {
                source_team: Some(TeamId(1)),
                ..hostile
            },
            TeamId(1),
        ),
        None
    );
    assert_eq!(
        credited_defeat_team(
            &CombatOutcomeFact {
                source_team: None,
                source_network_id: None,
                ..hostile
            },
            TeamId(1),
        ),
        None
    );
    assert_eq!(
        credited_defeat_team(
            &CombatOutcomeFact {
                source_team: Some(TeamId(2)),
                ..hostile
            },
            TeamId(1),
        ),
        None
    );
    let mut saturated = u16::MAX;
    increment_score(&mut saturated);
    assert_eq!(saturated, u16::MAX);
}

#[test]
fn match_telemetry_is_bounded_and_derives_tick_metrics() {
    use crate::combat::{
        AttackId, CombatEventId, CombatOutcomeFact, CombatOutcomeKind, WorldPoint,
    };
    use crate::protocol::NetworkEntityId;

    let mut telemetry = MatchTelemetry::default();
    telemetry.begin(MatchId(7), 100);
    for event in 0..3 {
        telemetry.record(
            CombatOutcomeFact {
                event_id: CombatEventId(event),
                tick: 130 + event,
                attack_id: AttackId(event),
                source_player: Some(PlayerId(1)),
                source_network_id: Some(NetworkEntityId(1)),
                source_team: Some(TeamId(0)),
                target_network_id: NetworkEntityId(2),
                target_team: TeamId(1),
                preset_id: None,
                recipe_fingerprint: None,
                position: WorldPoint { x: 0.0, y: 0.0 },
                engagement_distance: 100.0,
                kind: CombatOutcomeKind::Damage { amount: 5 },
            },
            2,
        );
    }
    telemetry.complete(
        160,
        [1, 0],
        MatchResult::TeamVictory { team: TeamId(0) },
        1,
        &crate::combat::WeaponTelemetry::default(),
    );
    assert_eq!(telemetry.records.len(), 2);
    assert_eq!(telemetry.dropped_records, 1);
    let summary = telemetry.summaries.back().unwrap();
    assert_eq!(summary.active_duration_ticks, 60);
    assert_eq!(summary.time_to_first_hostile_damage_ticks, Some(30));
    assert_eq!(summary.applied_damage_by_distance, [15, 0, 0]);
    assert_eq!(summary.score_margin, 1);
}

#[test]
fn match_summary_archives_only_weapon_deltas_for_the_active_match() {
    use crate::combat::{
        WeaponPresetId, WeaponRecipeFingerprint, WeaponTelemetry, WeaponTelemetryAggregate,
        WeaponTelemetryKey,
    };

    let key = WeaponTelemetryKey {
        preset_id: WeaponPresetId(3),
        recipe_fingerprint: WeaponRecipeFingerprint(77),
    };
    let mut weapons = WeaponTelemetry::default();
    weapons.source_aggregates.insert(
        key,
        WeaponTelemetryAggregate {
            accepted_attacks: 5,
            attacks_with_hostile_contact: 2,
            ..WeaponTelemetryAggregate::default()
        },
    );
    let mut telemetry = MatchTelemetry::default();
    telemetry.begin_with_weapons(MatchId(9), 10, &weapons);
    let aggregate = weapons.source_aggregates.get_mut(&key).unwrap();
    aggregate.accepted_attacks += 4;
    aggregate.attacks_with_hostile_contact += 3;
    telemetry.complete(20, [0, 0], MatchResult::Draw, 32, &weapons);

    let summary = telemetry.summaries.back().unwrap();
    assert_eq!(summary.weapon_aggregates.len(), 1);
    assert_eq!(summary.weapon_aggregates[0].0, key);
    assert_eq!(summary.weapon_aggregates[0].1.accepted_attacks, 4);
    assert_eq!(
        summary.weapon_aggregates[0].1.attacks_with_hostile_contact,
        3
    );
    assert_eq!(summary.weapon_hostile_contact_rates, vec![(key, 0.75)]);
}

#[test]
fn telemetry_handles_multiple_lives_incomplete_fights_rates_and_summary_drops() {
    use crate::combat::{
        AttackId, CombatEventId, CombatOutcomeFact, CombatOutcomeKind, WeaponTelemetry, WorldPoint,
    };
    use crate::protocol::NetworkEntityId;

    fn fact(event: u64, tick: u64, kind: CombatOutcomeKind) -> CombatOutcomeFact {
        CombatOutcomeFact {
            event_id: CombatEventId(event),
            tick,
            attack_id: AttackId(event),
            source_player: Some(PlayerId(1)),
            source_network_id: Some(NetworkEntityId(1)),
            source_team: Some(TeamId(0)),
            target_network_id: NetworkEntityId(2),
            target_team: TeamId(1),
            preset_id: None,
            recipe_fingerprint: None,
            position: WorldPoint { x: 0.0, y: 0.0 },
            engagement_distance: 300.0,
            kind,
        }
    }

    let mut telemetry = MatchTelemetry::default();
    telemetry.begin(MatchId(11), 100);
    telemetry.record(fact(1, 110, CombatOutcomeKind::Damage { amount: 10 }), 32);
    telemetry.record(fact(2, 120, CombatOutcomeKind::Defeat), 32);
    telemetry.record_respawn(2, 130);
    telemetry.record(fact(3, 140, CombatOutcomeKind::Damage { amount: 10 }), 32);
    telemetry.record(fact(4, 155, CombatOutcomeKind::Defeat), 32);
    telemetry.record(fact(5, 160, CombatOutcomeKind::Damage { amount: 1 }), 32);
    telemetry.record_movement(1, true);
    telemetry.record_movement(1, false);
    for _ in 0..2 {
        telemetry.record_participant_active_tick(TeamId(0));
    }
    for _ in 0..4 {
        telemetry.record_participant_active_tick(TeamId(1));
    }
    telemetry.complete(
        180,
        [2, 0],
        MatchResult::Forfeit {
            winner: TeamId(0),
            departed_team: TeamId(1),
        },
        1,
        &WeaponTelemetry::default(),
    );
    let summary = telemetry.summaries.back().unwrap();
    assert_eq!(summary.fight_duration_ticks, vec![10, 15]);
    assert_eq!(summary.respawn_to_defeat_ticks, vec![20, 25]);
    assert_eq!(summary.movement_ticks_by_player, vec![(1, 1, 2)]);
    assert_eq!(summary.participant_active_ticks_by_team, [2, 4]);
    assert!((summary.credited_defeats_per_participant_minute[0] - 3_600.0).abs() < f64::EPSILON);
    assert!(summary.credited_defeats_per_participant_minute[1].abs() < f64::EPSILON);
    assert!(summary.suffered_deaths_per_participant_minute[0].abs() < f64::EPSILON);
    assert!((summary.suffered_deaths_per_participant_minute[1] - 1_800.0).abs() < f64::EPSILON);

    telemetry.begin(MatchId(12), 200);
    telemetry.complete(
        200,
        [0, 0],
        MatchResult::Draw,
        1,
        &WeaponTelemetry::default(),
    );
    let no_damage = telemetry.summaries.back().unwrap();
    assert_eq!(no_damage.time_to_first_hostile_damage_ticks, None);
    assert_eq!(no_damage.active_duration_ticks, 0);
    assert!(
        no_damage
            .credited_defeats_per_participant_minute
            .iter()
            .all(|rate| rate.abs() < f64::EPSILON)
    );
    assert!(
        no_damage
            .suffered_deaths_per_participant_minute
            .iter()
            .all(|rate| rate.abs() < f64::EPSILON)
    );
    assert_eq!(telemetry.dropped_summaries, 1);
}
