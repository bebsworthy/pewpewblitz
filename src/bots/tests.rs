use super::{
    diagnostics::{BotDecisionTrace, BotDiagnostics},
    entropy,
    model::{BotObservation, BotRole, BotTactic, PracticeBotController},
    navigation::{BotNavigationSnapshot, BotRouteProgress, BotRouteStart},
    policy,
    profile::{BotCatalog, BotProfile},
    team::{BotPlanMember, assign_roles},
};
use crate::map::MapDimensions;
use bevy::prelude::*;
use std::collections::{BTreeMap, BTreeSet};

#[test]
fn profile_is_valid_and_entropy_streams_are_repeatable() {
    let profile = BotProfile::embedded().unwrap();
    assert!(profile.validate().is_ok());
    assert_eq!(profile.search_budget_per_bot(1), 512);
    assert!(profile.search_budget_per_bot(5) * 5 <= 512);
    assert!(profile.search_budget_per_bot(8) * 8 <= 512);
    assert_eq!(entropy::sample_u64(7, 2, 19), entropy::sample_u64(7, 2, 19));
    assert_ne!(entropy::sample_u64(7, 2, 19), entropy::sample_u64(7, 3, 19));
}

#[test]
fn embedded_profile_is_valid_bounded_and_fingerprinted() {
    let catalog = BotCatalog::embedded().unwrap();
    assert_eq!(
        catalog.schema_version,
        super::profile::BOT_CATALOG_SCHEMA_VERSION
    );
    let fingerprint = |catalog: &BotCatalog| {
        crate::content::fnv1a64(&catalog.canonical_fingerprint_material().unwrap())
    };
    assert_ne!(fingerprint(&catalog), 0);

    let baseline = fingerprint(&catalog);
    let mut tuned = catalog;
    tuned.practice.preferred_range_fraction = 0.8;
    assert!(tuned.validate().is_ok());
    assert_ne!(fingerprint(&tuned), baseline);

    let mut invalid = catalog;
    invalid.practice.reaction_ticks = super::model::MAX_OBSERVATION_HISTORY as u64;
    assert!(invalid.validate().is_err());
    invalid = catalog;
    invalid.practice.maximum_aim_error_radians = f32::NAN;
    assert!(invalid.validate().is_err());
    invalid = catalog;
    invalid.practice.maximum_search_expansions = u32::MAX;
    assert!(invalid.validate().is_err());
    invalid = catalog;
    invalid.practice.waypoint_reach_distance = f32::MAX;
    assert_eq!(
        invalid.validate(),
        Err("Practice bot distances exceed map-derived engine bounds".into())
    );
    invalid = catalog;
    invalid.practice.waypoint_reach_distance = crate::map::MAP_CELL_SIZE_WORLD;
    assert_eq!(
        invalid.validate(),
        Err("Practice bot distances exceed map-derived engine bounds".into())
    );
    invalid = catalog;
    invalid.practice.ally_separation_distance = f32::MAX;
    assert_eq!(
        invalid.validate(),
        Err("Practice bot distances exceed map-derived engine bounds".into())
    );
}

#[test]
fn navigation_is_stable_and_does_not_cut_a_blocked_corner() {
    let dimensions = MapDimensions {
        width: 32,
        height: 24,
    };
    let start_cell = crate::map::MapCell::new(1, 1);
    let goal_cell = crate::map::MapCell::new(4, 4);
    let start = dimensions.cell_center(start_cell);
    let goal = dimensions.cell_center(goal_cell);
    let blocked = BTreeSet::from([
        u32::from(dimensions.width) + 2,
        2 * u32::from(dimensions.width) + 1,
    ]);
    let navigation = BotNavigationSnapshot {
        dimensions,
        blocked,
        terrain_cost_milli: BTreeMap::new(),
        minimum_terrain_cost_milli: 1_000,
    };
    let first = navigation.route(start, goal, &[], 1_000, 64).unwrap();
    let second = navigation.route(start, goal, &[], 1_000, 64).unwrap();
    assert_eq!(first, second);
    assert_ne!(
        first.first().copied(),
        Some(dimensions.cell_center(crate::map::MapCell::new(2, 2)))
    );
}

#[test]
fn navigation_routes_around_expensive_effect_tiles() {
    let dimensions = MapDimensions {
        width: 5,
        height: 3,
    };
    let expensive = BTreeMap::from([
        (u32::from(dimensions.width) + 1, 2_000),
        (u32::from(dimensions.width) + 2, 2_000),
        (u32::from(dimensions.width) + 3, 2_000),
    ]);
    let navigation = BotNavigationSnapshot {
        dimensions,
        blocked: BTreeSet::new(),
        terrain_cost_milli: expensive,
        minimum_terrain_cost_milli: 1_000,
    };
    let start = dimensions.cell_center(crate::map::MapCell::new(0, 1));
    let goal = dimensions.cell_center(crate::map::MapCell::new(4, 1));
    let route = navigation.route(start, goal, &[], 128, 32).unwrap();
    assert!(
        route
            .iter()
            .any(|point| (point.y - start.y).abs() > f32::EPSILON)
    );
}

#[test]
fn authored_tile_multipliers_drive_costs_and_the_admissible_heuristic() {
    let catalog = crate::map::MapContentCatalog::embedded().unwrap();
    let mut map = catalog
        .resolve_preset(
            crate::map::FEATURE_YARD_WIPEOUT_PRESET,
            crate::map::MapInstanceId(1),
        )
        .unwrap();
    let speed = map
        .effect_tiles
        .iter_mut()
        .find(|tile| tile.behavior.kind() == Some(crate::map::EffectTileKind::Speed))
        .unwrap();
    speed.behavior = crate::map::MapEffectTileBehavior::Speed {
        movement_multiplier_milli: 2_000,
    };
    let speed_cell = speed.cell;
    let navigation = BotNavigationSnapshot::from_map(&map, 15.0, 2_000).unwrap();
    let speed_index = u32::from(speed_cell.y) * u32::from(map.snapshot.dimensions.width)
        + u32::from(speed_cell.x);

    assert_eq!(navigation.terrain_cost_milli[&speed_index], 500);
    assert_eq!(navigation.minimum_terrain_cost_milli, 500);
}

#[test]
fn delayed_observation_never_selects_a_newer_tick() {
    let mut controller = PracticeBotController::new(11);
    for tick in 1..=12 {
        controller.push_observation(observation(tick));
    }
    assert_eq!(controller.delayed_observation(12, 9).unwrap().tick, 3);
    assert!(controller.delayed_observation(8, 9).is_none());
}

#[test]
fn team_roles_are_stable_and_single_heist_bot_attacks() {
    use crate::{combat::TeamId, protocol::NetworkEntityId};
    let members = [
        BotPlanMember {
            network_id: NetworkEntityId(30),
            team: TeamId(1),
            mode: super::model::BotModeView::Heist,
        },
        BotPlanMember {
            network_id: NetworkEntityId(10),
            team: TeamId(0),
            mode: super::model::BotModeView::Heist,
        },
        BotPlanMember {
            network_id: NetworkEntityId(20),
            team: TeamId(0),
            mode: super::model::BotModeView::Heist,
        },
    ];
    let forward = assign_roles(&members);
    let reversed = assign_roles(&members.into_iter().rev().collect::<Vec<_>>());
    assert_eq!(forward, reversed);
    assert_eq!(forward[&NetworkEntityId(10)], BotRole::Defender);
    assert_eq!(forward[&NetworkEntityId(20)], BotRole::Objective);
    assert_eq!(forward[&NetworkEntityId(30)], BotRole::Objective);
}

#[test]
fn hot_zone_objective_keeps_contesting_when_an_enemy_is_visible() {
    let mut observed = observation(10);
    observed.self_view.position = Vec2::new(-700.0, 0.0);
    observed.mode = super::model::BotModeView::HotZone {
        center: Vec2::ZERO,
        radius: 160.0,
        status: crate::matchplay::HotZoneStatus::Empty,
        progress: [0, 0],
    };
    observed.visible_enemies.push(super::model::BotFighterView {
        network_id: crate::protocol::NetworkEntityId(2),
        team: crate::combat::TeamId(1),
        position: Vec2::new(-500.0, 300.0),
        velocity: Vec2::ZERO,
        current_health: 100,
        maximum_health: 100,
        active: true,
        cold_meter: 0,
        frozen: false,
        poisoned: false,
        burning: false,
    });
    let mut state = super::model::BotState::default();
    let intent = policy::choose_intent(
        &observed,
        &mut state,
        BotProfile::embedded().unwrap(),
        BotRole::Objective,
    );
    assert_eq!(state.tactic, BotTactic::Contest);
    assert!(intent.move_goal.unwrap().distance(Vec2::ZERO) < 160.0);
    assert_eq!(intent.aim_target.unwrap().0, Vec2::new(-500.0, 300.0));
}

#[test]
fn nondefault_hot_zone_hold_fraction_changes_the_policy_goal() {
    let mut observed = observation(10);
    observed.self_view.position = Vec2::new(-700.0, 0.0);
    observed.mode = super::model::BotModeView::HotZone {
        center: Vec2::ZERO,
        radius: 160.0,
        status: crate::matchplay::HotZoneStatus::Empty,
        progress: [0, 0],
    };
    let baseline = BotProfile::embedded().unwrap();
    let mut tuned = baseline;
    tuned.hot_zone_hold_radius_fraction = 0.8;

    let baseline_goal = policy::choose_intent(
        &observed,
        &mut super::model::BotState::default(),
        baseline,
        BotRole::Objective,
    )
    .move_goal
    .unwrap();
    let tuned_goal = policy::choose_intent(
        &observed,
        &mut super::model::BotState::default(),
        tuned,
        BotRole::Objective,
    )
    .move_goal
    .unwrap();

    assert!((baseline_goal.length() - 72.0).abs() < 0.001);
    assert!((tuned_goal.length() - 128.0).abs() < 0.001);
}

#[test]
fn demolition_bot_uses_ordinary_ultimate_input_toward_a_visible_target() {
    let dimensions = MapDimensions {
        width: 32,
        height: 24,
    };
    let navigation = BotNavigationSnapshot {
        dimensions,
        blocked: BTreeSet::new(),
        terrain_cost_milli: BTreeMap::new(),
        minimum_terrain_cost_milli: 1_000,
    };
    let mut observed = observation(20);
    observed.ability_ready = true;
    observed.ultimate_kind = crate::builds::UltimateKind::DemolitionStrike;
    observed.ultimate_range = 520.0;
    observed.visible_enemies.push(super::model::BotFighterView {
        network_id: crate::protocol::NetworkEntityId(2),
        team: crate::combat::TeamId(1),
        position: Vec2::new(160.0, 0.0),
        velocity: Vec2::ZERO,
        current_health: 100,
        maximum_health: 100,
        active: true,
        cold_meter: 0,
        frozen: false,
        poisoned: false,
        burning: false,
    });
    let decision = policy::decide(
        &observed,
        &mut super::model::BotState::default(),
        BotProfile::embedded().unwrap(),
        &navigation,
        7,
        BotRole::Pressure,
        512,
    );
    assert_ne!(
        decision.input.gameplay_buttons & crate::protocol::FighterInput::ULTIMATE,
        0
    );
    assert!(decision.input.aim_distance.is_some());
}

#[test]
fn restoration_bot_targets_an_injured_ally_and_uses_the_field() {
    let navigation = BotNavigationSnapshot {
        dimensions: MapDimensions {
            width: 32,
            height: 24,
        },
        blocked: BTreeSet::new(),
        terrain_cost_milli: BTreeMap::new(),
        minimum_terrain_cost_milli: 1_000,
    };
    let mut observed = observation(20);
    observed.ability_ready = true;
    observed.ultimate_kind = crate::builds::UltimateKind::RestorationField;
    observed.ultimate_range = 520.0;
    observed.allies.push(super::model::BotFighterView {
        network_id: crate::protocol::NetworkEntityId(3),
        team: crate::combat::TeamId(0),
        position: Vec2::new(120.0, 0.0),
        velocity: Vec2::ZERO,
        current_health: 40,
        maximum_health: 100,
        active: true,
        cold_meter: 0,
        frozen: false,
        poisoned: false,
        burning: false,
    });
    let decision = policy::decide(
        &observed,
        &mut super::model::BotState::default(),
        BotProfile::embedded().unwrap(),
        &navigation,
        7,
        BotRole::Pressure,
        512,
    );
    assert_ne!(
        decision.input.gameplay_buttons & crate::protocol::FighterInput::ULTIMATE,
        0
    );
}

#[test]
fn healing_weapon_targets_an_injured_ally() {
    let mut observed = observation(20);
    observed.healing_weapon = true;
    observed.weapon_range = 300.0;
    observed.allies.push(super::model::BotFighterView {
        network_id: crate::protocol::NetworkEntityId(3),
        team: crate::combat::TeamId(0),
        position: Vec2::new(120.0, 0.0),
        velocity: Vec2::ZERO,
        current_health: 40,
        maximum_health: 100,
        active: true,
        cold_meter: 0,
        frozen: false,
        poisoned: false,
        burning: false,
    });
    let intent = policy::choose_intent(
        &observed,
        &mut super::model::BotState::default(),
        BotProfile::embedded().unwrap(),
        BotRole::Pressure,
    );
    assert_eq!(intent.aim_target.unwrap().0, Vec2::new(120.0, 0.0));
    assert!(intent.fire);
}

#[test]
fn heist_attacker_targets_the_hostile_safe_despite_visible_enemies() {
    use crate::{
        combat::TeamId,
        map::{DamageableTargetIdentity, ModeAnchorId},
        matchplay::MatchId,
    };
    let mut observed = observation(10);
    observed.mode = super::model::BotModeView::Heist;
    observed.self_view.position = Vec2::new(-700.0, 0.0);
    observed.visible_enemies.push(super::model::BotFighterView {
        network_id: crate::protocol::NetworkEntityId(2),
        team: TeamId(1),
        position: Vec2::new(-500.0, 0.0),
        velocity: Vec2::ZERO,
        current_health: 100,
        maximum_health: 100,
        active: true,
        cold_meter: 0,
        frozen: false,
        poisoned: false,
        burning: false,
    });
    for (anchor, team, position) in [
        (1, TeamId(0), Vec2::new(-900.0, 0.0)),
        (2, TeamId(1), Vec2::new(900.0, 0.0)),
    ] {
        observed.objects.push(super::model::BotObjectView {
            identity: DamageableTargetIdentity::HeistSafe {
                match_id: MatchId(1),
                anchor_id: ModeAnchorId(anchor),
                defending_team: team,
            },
            kind: super::model::BotObjectKind::HeistSafe {
                defending_team: team,
            },
            position,
            current_health: 1_000,
            maximum_health: 1_000,
            live: true,
        });
    }
    let mut state = super::model::BotState::default();
    let intent = policy::choose_intent(
        &observed,
        &mut state,
        BotProfile::embedded().unwrap(),
        BotRole::Objective,
    );
    assert_eq!(state.tactic, BotTactic::AttackSafe);
    assert_eq!(intent.aim_target.unwrap().0, Vec2::new(900.0, 0.0));
    assert_ne!(intent.move_goal, Some(Vec2::new(900.0, 0.0)));
}

#[test]
fn object_attack_holds_weapon_standoff_instead_of_entering_the_collider() {
    let mut observed = observation(10);
    observed.self_view.position = Vec2::new(-700.0, -400.0);
    observed.weapon_range = 500.0;
    let object_position = Vec2::new(-950.0, -550.0);
    observed.objects.push(super::model::BotObjectView {
        identity: crate::map::DamageableTargetIdentity::MapObject {
            generation: observed.map_generation,
            placement_id: crate::map::MapPlacementId(1),
        },
        kind: super::model::BotObjectKind::TreasureChest,
        position: object_position,
        current_health: 100,
        maximum_health: 100,
        live: true,
    });
    let mut state = super::model::BotState::default();
    let intent = policy::choose_intent(
        &observed,
        &mut state,
        BotProfile::embedded().unwrap(),
        BotRole::Pressure,
    );
    assert_eq!(state.tactic, BotTactic::BreakObject);
    assert_eq!(intent.move_goal, Some(observed.self_view.position));
    assert_ne!(intent.move_goal, Some(object_position));
    assert_eq!(intent.aim_target.unwrap().0, object_position);
    assert!(intent.fire);
}

#[test]
fn corner_stall_starts_a_bounded_inward_escape() {
    let dimensions = MapDimensions {
        width: 64,
        height: 40,
    };
    let navigation = BotNavigationSnapshot {
        dimensions,
        blocked: BTreeSet::new(),
        terrain_cost_milli: BTreeMap::new(),
        minimum_terrain_cost_milli: 1_000,
    };
    let profile = BotProfile::embedded().unwrap();
    let mut observed = observation(50);
    observed.self_view.position = dimensions.bounds().min + Vec2::splat(32.0);
    let mut state = super::model::BotState {
        last_position: Some(observed.self_view.position),
        stationary_ticks: profile.stuck_ticks,
        ..default()
    };
    let decision = policy::decide(
        &observed,
        &mut state,
        profile,
        &navigation,
        7,
        BotRole::Pressure,
        512,
    );
    let movement = decision.input.move_axis.to_vec2();
    assert!(
        movement.x > 0.0 && movement.y > 0.0,
        "movement={movement:?}"
    );
    assert!(state.stuck_escape_until_tick > observed.tick);
    assert!(state.route.is_empty());
    assert_eq!(state.route_goal, None);
}

#[test]
fn low_health_retreat_recovers_from_the_perimeter_without_reselecting_the_corner() {
    let dimensions = MapDimensions {
        width: 64,
        height: 40,
    };
    let navigation = BotNavigationSnapshot {
        dimensions,
        blocked: BTreeSet::new(),
        terrain_cost_milli: BTreeMap::new(),
        minimum_terrain_cost_milli: 1_000,
    };
    let profile = BotProfile::embedded().unwrap();
    let bounds = dimensions.bounds();
    let mut observed = observation(50);
    observed.self_view.position = bounds.max - Vec2::splat(32.0);
    observed.self_view.current_health = 16;
    observed.weapon_range = 600.0;
    observed.visible_enemies.push(super::model::BotFighterView {
        network_id: crate::protocol::NetworkEntityId(2),
        team: crate::combat::TeamId(1),
        position: Vec2::ZERO,
        velocity: Vec2::ZERO,
        current_health: 58,
        maximum_health: 100,
        active: true,
        cold_meter: 0,
        frozen: false,
        poisoned: false,
        burning: false,
    });
    let mut state = super::model::BotState::default();

    let corner_decision = policy::decide(
        &observed,
        &mut state,
        profile,
        &navigation,
        7,
        BotRole::Pressure,
        512,
    );
    let corner_movement = corner_decision.input.move_axis.to_vec2();
    assert!(
        corner_movement.x < 0.0 && corner_movement.y < 0.0,
        "movement={corner_movement:?}"
    );
    assert!(state.perimeter_recovery);

    observed.tick += 1;
    observed.self_view.position = bounds.max - Vec2::splat(96.0);
    let recovering_decision = policy::decide(
        &observed,
        &mut state,
        profile,
        &navigation,
        7,
        BotRole::Pressure,
        512,
    );
    let recovering_movement = recovering_decision.input.move_axis.to_vec2();
    assert!(
        recovering_movement.x < 0.0 && recovering_movement.y < 0.0,
        "movement={recovering_movement:?}"
    );
    assert!(state.perimeter_recovery);

    observed.tick += 1;
    observed.self_view.position = bounds.max - Vec2::splat(192.0);
    let interior_decision = policy::decide(
        &observed,
        &mut state,
        profile,
        &navigation,
        7,
        BotRole::Pressure,
        512,
    );
    assert_eq!(interior_decision.input.move_axis.to_vec2(), Vec2::ZERO);
    assert!(!state.perimeter_recovery);
}

#[test]
fn decision_trace_is_bounded_and_counts_drops() {
    let mut world = World::new();
    let mut diagnostics = BotDiagnostics::from_world(&mut world);
    diagnostics.enable_trace_for_test();
    for tick in 0..300 {
        diagnostics.record(BotDecisionTrace {
            tick,
            network_id: crate::protocol::NetworkEntityId(1),
            role: BotRole::Pressure,
            tactic: BotTactic::Pressure,
            input: crate::protocol::FighterInput::default(),
        });
    }
    assert_eq!(diagnostics.decisions, 300);
    assert_eq!(diagnostics.neutral_decisions, 300);
    assert_eq!(diagnostics.traces.len(), 256);
    assert_eq!(diagnostics.trace_drops, 44);
    assert_eq!(diagnostics.traces.front().unwrap().tick, 44);
    diagnostics.record_navigation(policy::BotNavigationDecisionDiagnostics {
        search_started: true,
        status: policy::BotNavigationSearchStatus::Pending,
        expansions: 17,
    });
    assert_eq!(diagnostics.navigation_searches_started, 1);
    assert_eq!(diagnostics.navigation_searches_pending, 1);
    assert_eq!(diagnostics.navigation_searches_completed, 0);
    assert_eq!(diagnostics.navigation_searches_exhausted, 0);
    assert_eq!(diagnostics.navigation_expansions, 17);
}

#[test]
fn large_navigation_is_bounded_and_exhaustion_fails_closed() {
    let dimensions = MapDimensions {
        width: 512,
        height: 512,
    };
    let navigation = BotNavigationSnapshot {
        dimensions,
        blocked: BTreeSet::from([256 * u32::from(dimensions.width) + 256]),
        terrain_cost_milli: BTreeMap::new(),
        minimum_terrain_cost_milli: 1_000,
    };
    let start = dimensions.cell_center(crate::map::MapCell::new(0, 0));
    let goal = dimensions.cell_center(crate::map::MapCell::new(511, 511));
    assert!(navigation.route(start, goal, &[], 65_536, 2_048).is_some());
    assert!(navigation.route(start, goal, &[], 1, 1_024).is_none());
    let outside = Vec2::splat(100_000.0);
    let clamped = navigation.clamp_goal(outside);
    assert!(dimensions.bounds().contains(clamped));
}

#[test]
fn navigation_search_resumes_deterministically_across_tick_budgets() {
    let dimensions = MapDimensions {
        width: 32,
        height: 24,
    };
    let navigation = BotNavigationSnapshot {
        dimensions,
        blocked: BTreeSet::from([12 * u32::from(dimensions.width) + 16]),
        terrain_cost_milli: BTreeMap::new(),
        minimum_terrain_cost_milli: 1_000,
    };
    let start = dimensions.cell_center(crate::map::MapCell::new(1, 1));
    let goal = dimensions.cell_center(crate::map::MapCell::new(30, 22));
    let BotRouteStart::Search(mut search) = navigation
        .begin_route(start, goal, &[], 16_384, 1_024)
        .unwrap()
    else {
        panic!("the central blocker should require graph search");
    };
    assert_eq!(search.advance(&navigation, 1), BotRouteProgress::Pending);

    let route = (0..1_000).find_map(|_| match search.advance(&navigation, 8) {
        BotRouteProgress::Pending => None,
        BotRouteProgress::Complete(route) => Some(route),
        BotRouteProgress::Exhausted => panic!("resumable search exhausted unexpectedly"),
    });
    assert!(route.is_some());
}

#[test]
fn contact_memory_contains_only_observed_facts_and_expires() {
    let dimensions = MapDimensions {
        width: 32,
        height: 24,
    };
    let navigation = BotNavigationSnapshot {
        dimensions,
        blocked: BTreeSet::new(),
        terrain_cost_milli: BTreeMap::new(),
        minimum_terrain_cost_milli: 1_000,
    };
    let profile = BotProfile::embedded().unwrap();
    let mut state = super::model::BotState::default();
    let mut visible = observation(10);
    visible.visible_enemies.push(super::model::BotFighterView {
        network_id: crate::protocol::NetworkEntityId(2),
        team: crate::combat::TeamId(1),
        position: Vec2::new(64.0, 0.0),
        velocity: Vec2::X,
        current_health: 100,
        maximum_health: 100,
        active: true,
        cold_meter: 0,
        frozen: false,
        poisoned: false,
        burning: false,
    });
    let _ = policy::decide(
        &visible,
        &mut state,
        profile,
        &navigation,
        7,
        BotRole::Pressure,
        512,
    );
    assert_eq!(state.contacts.len(), 1);
    assert_eq!(state.contacts[0].position, Vec2::new(64.0, 0.0));

    let hidden = observation(131);
    let _ = policy::decide(
        &hidden,
        &mut state,
        profile,
        &navigation,
        7,
        BotRole::Pressure,
        512,
    );
    assert!(state.contacts.is_empty());
}

#[test]
fn life_reset_clears_every_private_decision_boundary() {
    let mut controller = PracticeBotController::new(11);
    controller.push_observation(observation(1));
    controller.state.route.push(Vec2::X);
    controller.state.route_goal = Some(Vec2::X);
    controller.last_decision_tick = Some(1);
    controller.reset_life();
    assert_eq!(controller.life_generation, 1);
    assert!(controller.history.is_empty());
    assert!(controller.state.route.is_empty());
    assert_eq!(controller.state.route_goal, None);
    assert_eq!(controller.last_decision_tick, None);
}

#[test]
fn maximum_practice_roster_pure_decisions_stay_inside_one_fixed_tick() {
    let dimensions = MapDimensions {
        width: 64,
        height: 40,
    };
    let navigation = BotNavigationSnapshot {
        dimensions,
        blocked: BTreeSet::from([20 * u32::from(dimensions.width) + 32]),
        terrain_cost_milli: BTreeMap::new(),
        minimum_terrain_cost_milli: 1_000,
    };
    let profile = BotProfile::embedded().unwrap();
    let budget = profile.search_budget_per_bot(5);
    let mut samples = Vec::with_capacity(200);
    for sample in 0..200_u64 {
        let started = std::time::Instant::now();
        for ordinal in 0..5_u16 {
            let mut observed = observation(100 + sample);
            observed.self_view.network_id =
                crate::protocol::NetworkEntityId(100 + u64::from(ordinal));
            observed.self_view.position = Vec2::new(-800.0, -400.0 + f32::from(ordinal) * 80.0);
            observed.visible_enemies.push(super::model::BotFighterView {
                network_id: crate::protocol::NetworkEntityId(2),
                team: crate::combat::TeamId(1),
                position: Vec2::new(800.0, 400.0),
                velocity: Vec2::ZERO,
                current_health: 100,
                maximum_health: 100,
                active: true,
                cold_meter: 0,
                frozen: false,
                poisoned: false,
                burning: false,
            });
            let _ = policy::decide(
                &observed,
                &mut super::model::BotState::default(),
                profile,
                &navigation,
                u64::from(ordinal),
                BotRole::Pressure,
                budget,
            );
        }
        samples.push(started.elapsed());
    }
    samples.sort_unstable();
    let p95 = samples[189];
    assert!(p95 < crate::timing::SIMULATION_TICK, "bot p95 was {p95:?}");
}

fn observation(tick: u64) -> BotObservation {
    use super::model::{BotFighterView, BotModeView};
    use crate::{
        combat::{TeamId, WeaponPhase},
        map::{MapDynamicGeneration, MapInstanceId},
        matchplay::MatchId,
        protocol::NetworkEntityId,
    };
    BotObservation {
        tick,
        match_id: MatchId(1),
        map_instance_id: MapInstanceId(1),
        map_generation: MapDynamicGeneration {
            map_instance_id: MapInstanceId(1),
            generation: 1,
        },
        map_revision: 0,
        match_active: true,
        self_view: BotFighterView {
            network_id: NetworkEntityId(1),
            team: TeamId(0),
            position: Vec2::ZERO,
            velocity: Vec2::ZERO,
            current_health: 100,
            maximum_health: 100,
            active: true,
            cold_meter: 0,
            frozen: false,
            poisoned: false,
            burning: false,
        },
        allies: Vec::new(),
        visible_enemies: Vec::new(),
        objects: Vec::new(),
        pickups: Vec::new(),
        mode: BotModeView::Wipeout { scores: [0, 0] },
        weapon_phase: WeaponPhase::Ready,
        weapon_ammo: 1,
        ability_ready: false,
        ultimate_kind: crate::builds::UltimateKind::Dash,
        ultimate_range: 0.0,
        weapon_range: 100.0,
        projectile_speed: 100.0,
        healing_weapon: false,
    }
}
