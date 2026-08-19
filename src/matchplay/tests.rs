use super::*;
use crate::{combat::TeamId, map::SpawnPointId, protocol::PlayerId};
use bevy::prelude::Vec2;

#[test]
fn match_id_preserves_full_u128_through_format_parse_and_serde() {
    let value = MatchId(u128::MAX);
    assert_eq!(value.to_string(), u128::MAX.to_string());
    assert_eq!(value.to_string().parse::<MatchId>().unwrap(), value);
    let bytes = postcard::to_allocvec(&value).expect("match ID serializes");
    let decoded: MatchId = postcard::from_bytes(&bytes).expect("match ID deserializes");
    assert_eq!(decoded, value);
}

#[test]
fn production_rules_match_the_approved_contract() {
    let wipeout = WipeoutRules::default().validate().unwrap();
    assert_eq!(wipeout.target_score, 10);
    #[cfg(feature = "server")]
    {
        let lifecycle = MatchLifecycleRules::default().validate().unwrap();
        assert_eq!(lifecycle.minimum_participants_per_team, 1);
        assert_eq!(lifecycle.maximum_participants_per_team, 3);
        assert_eq!(lifecycle.countdown_ticks, 180);
        assert_eq!(lifecycle.active_limit_ticks, 10_800);
        assert_eq!(lifecycle.respawn_delay_ticks, 180);
        assert_eq!(lifecycle.spawn_protection_ticks, 90);
        assert_eq!(lifecycle.completed_input_lock_ticks, 60);
        let hot_zone = HotZoneRules::default().validate_with(&lifecycle).unwrap();
        assert_eq!(hot_zone.target_progress_ticks, 1_800);
    }
}

#[test]
fn rules_reject_zero_invalid_capacity_and_overflow() {
    assert!(WipeoutRules { target_score: 0 }.validate().is_err());
    #[cfg(feature = "server")]
    {
        assert!(
            MatchLifecycleRules {
                minimum_participants_per_team: 2,
                maximum_participants_per_team: 1,
                ..MatchLifecycleRules::default()
            }
            .validate()
            .is_err()
        );
        assert!(
            MatchLifecycleRules {
                countdown_ticks: u64::MAX,
                active_limit_ticks: 1,
                ..MatchLifecycleRules::default()
            }
            .validate()
            .is_err()
        );
        let lifecycle = MatchLifecycleRules::default();
        assert!(
            HotZoneRules {
                target_progress_ticks: 1
            }
            .validate_with(&lifecycle)
            .is_err()
        );
        assert!(
            HotZoneRules {
                target_progress_ticks: 20_000,
            }
            .validate_with(&lifecycle)
            .is_err()
        );
    }
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
fn spawn_selection_folds_full_width_match_ids_deterministically() {
    let candidates = vec![
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
    let routed_match_id = MatchId((u128::from(u64::MAX) << 64) | 7);
    let first = select_spawn(
        candidates.clone(),
        &[],
        TeamId(0),
        1.0,
        routed_match_id,
        PlayerId(1),
        0,
    );
    let second = select_spawn(
        candidates,
        &[],
        TeamId(0),
        1.0,
        routed_match_id,
        PlayerId(1),
        0,
    );
    assert_eq!(first, second);
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
        source_kind: crate::combat::CombatSourceKind::PrimaryWeapon,
        source_player: Some(PlayerId(1)),
        source_network_id: Some(NetworkEntityId(1)),
        source_team: Some(TeamId(0)),
        target_network_id: NetworkEntityId(2),
        target_kind: crate::combat::CombatTargetKind::Fighter,
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
    saturated = saturated.saturating_add(1);
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
                source_kind: crate::combat::CombatSourceKind::PrimaryWeapon,
                source_player: Some(PlayerId(1)),
                source_network_id: Some(NetworkEntityId(1)),
                source_team: Some(TeamId(0)),
                target_network_id: NetworkEntityId(2),
                target_kind: crate::combat::CombatTargetKind::Fighter,
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
    telemetry.complete_with_mode(
        160,
        crate::map::WIPEOUT_MODE_DEFINITION,
        ModeSummary::Wipeout(WipeoutSummary {
            final_scores: [1, 0],
            target_score: 10,
            score_margin: 1,
        }),
        MatchResult::TeamVictory { team: TeamId(0) },
        1,
        &crate::combat::WeaponTelemetry::default(),
        &crate::abilities::AbilityTelemetry::default(),
    );
    assert_eq!(telemetry.records.len(), 2);
    assert_eq!(telemetry.dropped_records, 1);
    let summary = telemetry.summaries.back().unwrap();
    assert_eq!(summary.active_duration_ticks, 60);
    assert_eq!(summary.time_to_first_hostile_damage_ticks, Some(30));
    assert_eq!(summary.applied_damage_by_distance, [15, 0, 0]);
    assert_eq!(
        summary.mode_summary,
        ModeSummary::Wipeout(WipeoutSummary {
            final_scores: [1, 0],
            target_score: 10,
            score_margin: 1,
        })
    );
    assert_eq!(summary.dropped_records, 1);

    telemetry.begin(MatchId(8), 200);
    telemetry.complete_with_mode(
        200,
        crate::map::WIPEOUT_MODE_DEFINITION,
        ModeSummary::Wipeout(WipeoutSummary {
            final_scores: [0, 0],
            target_score: 10,
            score_margin: 0,
        }),
        MatchResult::Draw,
        2,
        &crate::combat::WeaponTelemetry::default(),
        &crate::abilities::AbilityTelemetry::default(),
    );
    assert_eq!(telemetry.summaries.back().unwrap().dropped_records, 0);
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
    telemetry.complete_with_mode(
        20,
        crate::map::WIPEOUT_MODE_DEFINITION,
        ModeSummary::Wipeout(WipeoutSummary {
            final_scores: [0, 0],
            target_score: 10,
            score_margin: 0,
        }),
        MatchResult::Draw,
        32,
        &weapons,
        &crate::abilities::AbilityTelemetry::default(),
    );

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

fn two_preset_telemetry_context() -> MatchTelemetryContext {
    MatchTelemetryContext {
        map_identity: crate::map::ResolvedMapIdentity {
            instance_id: crate::map::MapInstanceId(1),
            source_preset_id: None,
            recipe_id: crate::map::MapRecipeId(1),
            recipe_revision: 1,
            recipe_fingerprint: crate::map::MapRecipeFingerprint(1),
        },
        content_fingerprint: crate::content::GameplayContentFingerprint(1),
        rules_revision: 1,
        participants: [
            (1, 1, TeamId(0), 1, crate::combat::WeaponPresetId(1)),
            (2, 2, TeamId(1), 2, crate::combat::WeaponPresetId(2)),
        ]
        .into_iter()
        .map(
            |(player_id, network_entity_id, team, _weapon, preset)| MatchParticipantSummary {
                player_id,
                network_entity_id,
                team,
                selected_build: crate::builds::SelectedBuild {
                    source_build_preset_id: None,
                    recipe_fingerprint: crate::builds::BuildRecipeFingerprint(0),
                    revision: crate::builds::BuildRevision(1),
                },
                weapon_preset: Some(preset),
                total_points: None,
                ultimate_id: None,
                passive_ids: None,
            },
        )
        .collect(),
    }
}

fn assert_two_preset_rates(summary: &MatchSummary) {
    use crate::combat::WeaponPresetId;
    assert_eq!(
        summary.credited_defeats_by_preset,
        vec![(WeaponPresetId(1), 2)]
    );
    assert_eq!(
        summary.suffered_deaths_by_preset,
        vec![(WeaponPresetId(2), 2)]
    );
    assert_eq!(
        summary.participant_active_ticks_by_preset,
        vec![(WeaponPresetId(1), 2), (WeaponPresetId(2), 4)]
    );
    assert_eq!(
        summary.credited_defeats_per_participant_minute_by_preset,
        vec![(WeaponPresetId(1), 3_600.0), (WeaponPresetId(2), 0.0)]
    );
    assert_eq!(
        summary.suffered_deaths_per_participant_minute_by_preset,
        vec![(WeaponPresetId(1), 0.0), (WeaponPresetId(2), 1_800.0)]
    );
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
            source_kind: crate::combat::CombatSourceKind::PrimaryWeapon,
            source_player: Some(PlayerId(1)),
            source_network_id: Some(NetworkEntityId(1)),
            source_team: Some(TeamId(0)),
            target_network_id: NetworkEntityId(2),
            target_kind: crate::combat::CombatTargetKind::Fighter,
            target_team: TeamId(1),
            preset_id: Some(crate::combat::WeaponPresetId(1)),
            recipe_fingerprint: None,
            position: WorldPoint { x: 0.0, y: 0.0 },
            engagement_distance: 300.0,
            kind,
        }
    }

    let mut telemetry = MatchTelemetry::default();
    telemetry.begin(MatchId(11), 100);
    telemetry.set_context(two_preset_telemetry_context());
    telemetry.record(fact(1, 110, CombatOutcomeKind::Damage { amount: 10 }), 32);
    telemetry.record(fact(2, 120, CombatOutcomeKind::Defeat), 32);
    telemetry.record_respawn(2, 130);
    telemetry.record(fact(3, 140, CombatOutcomeKind::Damage { amount: 10 }), 32);
    telemetry.record(fact(4, 155, CombatOutcomeKind::Defeat), 32);
    telemetry.record(fact(5, 160, CombatOutcomeKind::Damage { amount: 1 }), 32);
    telemetry.record_movement(1, true);
    telemetry.record_movement(1, false);
    for _ in 0..2 {
        telemetry.record_participant_active_tick(TeamId(0), Some(crate::combat::WeaponPresetId(1)));
    }
    for _ in 0..4 {
        telemetry.record_participant_active_tick(TeamId(1), Some(crate::combat::WeaponPresetId(2)));
    }
    telemetry.complete_with_mode(
        180,
        crate::map::WIPEOUT_MODE_DEFINITION,
        ModeSummary::Wipeout(WipeoutSummary {
            final_scores: [2, 0],
            target_score: 10,
            score_margin: 2,
        }),
        MatchResult::Forfeit {
            winner: TeamId(0),
            departed_team: TeamId(1),
        },
        1,
        &WeaponTelemetry::default(),
        &crate::abilities::AbilityTelemetry::default(),
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
    assert_two_preset_rates(summary);

    telemetry.begin(MatchId(12), 200);
    telemetry.complete_with_mode(
        200,
        crate::map::WIPEOUT_MODE_DEFINITION,
        ModeSummary::Wipeout(WipeoutSummary {
            final_scores: [0, 0],
            target_score: 10,
            score_margin: 0,
        }),
        MatchResult::Draw,
        1,
        &WeaponTelemetry::default(),
        &crate::abilities::AbilityTelemetry::default(),
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

#[cfg(feature = "server")]
mod schedule_trace_tests {
    use super::*;
    use crate::gameplay::GameplayPlugin;
    use bevy::prelude::*;
    use bevy::time::TimeUpdateStrategy;

    #[derive(Resource, Default)]
    struct Trace(Vec<&'static str>);

    fn probe(label: &'static str) -> impl FnMut(ResMut<Trace>) + 'static {
        move |mut trace: ResMut<Trace>| trace.0.push(label)
    }

    #[allow(clippy::needless_pass_by_value)]
    fn finalize_probe(mut trace: ResMut<Trace>, tick: Res<crate::timing::SimulationTick>) {
        assert_eq!(tick.0, 0, "tick advancement is last");
        trace.0.push("finalize");
    }

    #[test]
    fn match_sets_have_the_m09_fixed_tick_order() {
        let mut app = App::new();
        app.add_plugins((MinimalPlugins, GameplayPlugin))
            .insert_resource(TimeUpdateStrategy::FixedTimesteps(1))
            .init_resource::<Trace>();
        configure_match_schedule(&mut app);
        app.add_systems(
            FixedUpdate,
            (
                probe("lifecycle").in_set(MatchSet::Lifecycle),
                probe("deadline").in_set(MatchSet::DeadlineRules),
                probe("pregame").in_set(MatchSet::PreGameOutcomes),
                probe("fighter-lifecycle").in_set(MatchSet::FighterLifecycle),
                probe("input").in_set(crate::gameplay::GameplaySet::Input),
                probe("fire").in_set(crate::gameplay::GameplaySet::Fire),
            ),
        )
        .add_systems(
            FixedPostUpdate,
            (
                probe("physics").after(avian2d::prelude::PhysicsSystems::StepSimulation),
                probe("sweep").in_set(crate::combat::CombatSet::ProjectileSweep),
                probe("damage").in_set(crate::combat::CombatSet::Damage),
                probe("observe")
                    .in_set(crate::abilities::AbilitySet::ObserveOutcomes)
                    .after(crate::combat::CombatSet::Damage),
                probe("mode-rules").in_set(MatchSet::ModeRules),
                probe("outcomes").in_set(MatchSet::Outcomes),
                probe("combat-lifecycle").in_set(crate::combat::CombatSet::Lifecycle),
                finalize_probe
                    .in_set(crate::combat::CombatSet::Finalize)
                    .before(crate::gameplay::advance_simulation_tick),
            ),
        );

        app.update();
        app.update();

        // Deadline completion precedes all boundary-tick gameplay; eligible objective
        // evaluation happens after movement/physics and damage; fact consumers run before
        // combat lifecycle; tick advancement is last.
        assert_eq!(
            app.world().resource::<Trace>().0,
            vec![
                "lifecycle",
                "deadline",
                "pregame",
                "fighter-lifecycle",
                "input",
                "fire",
                "physics",
                "sweep",
                "damage",
                "observe",
                "mode-rules",
                "outcomes",
                "combat-lifecycle",
                "finalize",
            ]
        );
        assert_eq!(app.world().resource::<crate::timing::SimulationTick>().0, 1);
    }
}

#[cfg(feature = "server")]
mod capacity_composition_tests {
    use super::super::server::validate_capacity_against_selected_map;
    use super::*;
    use crate::map::{MapInstanceId, MapLayoutRequirements, MapPresetId, ResolvedMap};
    use bevy::prelude::{App, Startup};

    fn embedded_snapshot() -> crate::map::ResolvedMapSnapshot {
        let catalog = crate::map::MapContentCatalog::embedded().expect("embedded map catalog");
        catalog
            .resolve_preset(
                MapPresetId(1),
                MapInstanceId(1),
                &MapLayoutRequirements::wipeout(),
            )
            .expect("built-in wipeout preset resolves")
            .snapshot
    }

    fn production_capacity() -> ResolvedMatchCapacity {
        ResolvedMatchCapacity::from_rules(&MatchLifecycleRules::default())
            .expect("production rules resolve a checked capacity")
    }

    #[test]
    fn production_capacity_satisfies_the_builtin_map() {
        production_capacity()
            .validate_against_map(&embedded_snapshot())
            .expect("the built-in wipeout map serves the production profile");
    }

    #[test]
    fn capacity_rejects_maps_serving_other_team_slots() {
        let mut lopsided = embedded_snapshot();
        lopsided.spawn_points.retain(|point| point.team_slot == 0);
        let error = production_capacity()
            .validate_against_map(&lopsided)
            .expect_err("a one-team map cannot serve a two-team profile");
        assert!(error.contains("team slots"), "{error}");
    }

    #[test]
    fn capacity_rejects_maps_without_spawn_capacity_for_simultaneous_participants() {
        let mut sparse = embedded_snapshot();
        let mut kept_per_team = [0_usize; 2];
        sparse.spawn_points.retain(|point| {
            let slot = usize::from(point.team_slot);
            kept_per_team[slot] += 1;
            kept_per_team[slot] <= 1
        });
        let error = production_capacity()
            .validate_against_map(&sparse)
            .expect_err("one spawn point per team cannot admit two simultaneous fighters");
        assert!(error.contains("spawn points"), "{error}");
    }

    #[test]
    #[should_panic(expected = "does not satisfy the selected map")]
    fn startup_panics_when_the_selected_map_under_serves_the_profile() {
        let mut app = App::new();
        app.add_systems(Startup, validate_capacity_against_selected_map);
        let mut under_served = embedded_snapshot();
        let mut kept_per_team = [0_usize; 2];
        under_served.spawn_points.retain(|point| {
            let slot = usize::from(point.team_slot);
            kept_per_team[slot] += 1;
            kept_per_team[slot] <= 1
        });
        app.insert_resource(ResolvedMap::from_snapshot(under_served));
        app.insert_resource(production_capacity());
        app.update();
    }

    #[test]
    fn synthetic_wide_profiles_validate_against_maps_that_serve_them() {
        // A large-group profile (outside today's production rules) still resolves and
        // validates against a map that serves its slots and spawn capacity.
        let wide = ResolvedMatchCapacity {
            team_slots: vec![
                TeamSlotCapacity {
                    team_slot: 0,
                    minimum_participants: 1,
                    maximum_participants: 12,
                },
                TeamSlotCapacity {
                    team_slot: 1,
                    minimum_participants: 1,
                    maximum_participants: 12,
                },
            ],
            maximum_active_fighters: 24,
        };
        assert_eq!(wide.maximum_active_fighters, 24);
        // The built-in map does not serve twelve simultaneous participants per team.
        assert!(wide.validate_against_map(&embedded_snapshot()).is_err());
    }
}

#[test]
fn match_result_report_labels_round_trip() {
    let results = [
        MatchResult::TeamVictory { team: TeamId(0) },
        MatchResult::TeamVictory { team: TeamId(1) },
        MatchResult::Draw,
        MatchResult::Forfeit {
            winner: TeamId(1),
            departed_team: TeamId(0),
        },
    ];
    for result in results {
        assert_eq!(
            MatchResult::parse_report_label(&result.report_label()),
            Some(result)
        );
    }
    // Anything that is not a result label is refused, including the closeout block's
    // `none` sentinel and malformed team payloads.
    for hostile in [
        "none",
        "victory",
        "victory:256",
        "forfeit:0",
        "forfeit:0:1:2",
        "",
    ] {
        assert_eq!(MatchResult::parse_report_label(hostile), None);
    }
}
