//! Last Stand authority over a real Sunder Ring resolution, without the network graph.

use super::*;
use crate::combat::{CombatEventId, CombatOutcomeFact, WorldPoint};
use crate::map::{MapCell, MapDynamicState, MapRoot};
use crate::matchplay::{MatchResult, ModeOutcomeCause};

const MATCH: MatchId = MatchId(7);
const ENDS_AT: u64 = 10_000;

fn app() -> App {
    let catalog = crate::map::MapContentCatalog::embedded().unwrap();
    let resolved = catalog
        .resolve_preset(crate::map::SUNDER_RING_PRESET, crate::map::MapInstanceId(1))
        .unwrap();
    let rules = LastStandRules::default();
    let mut app = App::new();
    app.insert_resource(SimulationTick(1))
        .insert_resource(rules)
        .insert_resource(BoostPickupRules {
            pickup_radius: rules.pickup_radius,
            stack_limit: rules.boost_stack_limit,
        })
        .insert_resource(MapCatalogResource(catalog))
        .init_resource::<NextBoostPickupId>()
        .init_resource::<ProcessedChests>()
        .init_resource::<LastStandRoster>()
        .init_resource::<LastStandTelemetry>()
        .init_resource::<ModeRuleOutcome>()
        .init_resource::<MatchOutcomeDiagnostics>()
        .init_resource::<CombatOutcomeFacts>()
        .add_systems(
            Update,
            (
                begin_round_and_advance_boundary,
                spawn_chest_pickups,
                resolve_eliminations,
            )
                .chain(),
        )
        .add_systems(PostUpdate, resolve_last_stand_deadline);
    app.world_mut().spawn((
        MapRoot,
        MapDynamicState {
            map_instance_id: crate::map::MapInstanceId(1),
            generation: 1,
            revision: 0,
            terminal_states: Vec::new(),
        },
    ));
    let initial = initial_state(MATCH, &rules, &resolved, ENDS_AT - 1);
    app.world_mut().spawn((
        MatchRoot,
        MatchState {
            match_id: MATCH,
            mode_definition_id: crate::map::LAST_STAND_MODE_DEFINITION,
            phase: MatchPhase::Active {
                ends_at_tick: ENDS_AT,
            },
            rules_revision: crate::matchplay::LAST_STAND_RULES_REVISION,
        },
        initial,
        bot_view(&initial.boundary, 0),
    ));
    app.insert_resource(resolved);
    app
}

/// Centre of an authored Sunder Ring spawn cell for solo team `team`.
fn spawn_position(app: &App, team: u8) -> Vec2 {
    let cell = [(3, 16), (9, 5), (23, 5), (29, 16), (23, 27), (9, 27)][usize::from(team)];
    app.world()
        .resource::<ResolvedMap>()
        .snapshot
        .dimensions
        .cell_center(MapCell::new(cell.0, cell.1))
}

fn fighter(app: &mut App, team: u8, stacks: u8) -> Entity {
    let position = spawn_position(app, team);
    app.world_mut()
        .spawn((
            Fighter,
            ActiveCombatant,
            NetworkEntityId(100 + u64::from(team)),
            MatchParticipant {
                match_id: MATCH,
                ready: true,
                restart_ready: false,
            },
            TeamId(team),
            Position(position),
            BoostStacks(stacks),
        ))
        .id()
}

fn step(app: &mut App) {
    app.update();
    app.world_mut()
        .resource_mut::<CombatOutcomeFacts>()
        .0
        .clear();
    app.world_mut().resource_mut::<SimulationTick>().0 += 1;
}

fn defeat(app: &mut App, team: u8) {
    let tick = app.world().resource::<SimulationTick>().0;
    app.world_mut()
        .resource_mut::<CombatOutcomeFacts>()
        .0
        .push(CombatOutcomeFact {
            event_id: CombatEventId(tick * 10 + u64::from(team)),
            tick,
            attack_id: crate::combat::AttackId(tick),
            source_kind: crate::combat::CombatSourceKind::PrimaryWeapon,
            source_player: None,
            source_network_id: None,
            source_team: None,
            target_network_id: NetworkEntityId(100 + u64::from(team)),
            target_kind: CombatTargetKind::Fighter,
            target_team: TeamId(team),
            preset_id: None,
            recipe_fingerprint: None,
            position: WorldPoint { x: 0.0, y: 0.0 },
            engagement_distance: 0.0,
            kind: CombatOutcomeKind::Defeat,
        });
}

fn state(app: &mut App) -> LastStandState {
    let world = app.world_mut();
    *world
        .query_filtered::<&LastStandState, With<MatchRoot>>()
        .single(world)
        .unwrap()
}

fn pickups(app: &mut App) -> Vec<(Vec2, BoostPickupSource)> {
    let world = app.world_mut();
    world
        .query_filtered::<(&Position, &BoostPickupSource), With<BoostPickup>>()
        .iter(world)
        .map(|(position, source)| (position.0, *source))
        .collect()
}

fn outcome(app: &mut App) -> Option<PendingModeRuleOutcome> {
    app.world_mut().resource_mut::<ModeRuleOutcome>().take()
}

#[test]
fn eliminations_rank_places_drop_boosts_and_crown_the_last_fighter() {
    let mut app = app();
    let fighters = [0, 1, 2].map(|team| fighter(&mut app, team, team));
    step(&mut app);
    let opening = state(&mut app);
    assert_eq!((opening.fighters, opening.alive), (3, 3));
    assert_eq!(opening.boundary.phase, 0);

    // A fighter with no boosts still drops one, next to where it fell.
    defeat(&mut app, 0);
    step(&mut app);
    let after_first = state(&mut app);
    assert_eq!(after_first.alive, 2);
    assert_eq!(after_first.placements[0], 3);
    let drops = pickups(&mut app);
    assert_eq!(drops.len(), 1);
    assert_eq!(drops[0].1, BoostPickupSource::PlayerDeath);
    assert!(drops[0].0.distance(spawn_position(&app, 0)) <= 2.0 * crate::map::MAP_CELL_SIZE_WORLD);
    assert!(outcome(&mut app).is_none());

    // Two carried stacks drop two pickups on distinct cells and reset the carrier.
    defeat(&mut app, 2);
    step(&mut app);
    let last = state(&mut app);
    assert_eq!(last.alive, 1);
    assert_eq!(last.placements, [3, 1, 2, 0, 0, 0]);
    assert_eq!(app.world().get::<BoostStacks>(fighters[2]).unwrap().0, 0);
    let drops = pickups(&mut app);
    assert_eq!(drops.len(), 3);
    assert!(drops[1].0.distance(drops[2].0) >= crate::map::MAP_CELL_SIZE_WORLD * 0.5);
    let pending = outcome(&mut app).unwrap();
    assert_eq!(pending.cause, ModeOutcomeCause::Threshold);
    assert_eq!(pending.result, MatchResult::TeamVictory { team: TeamId(1) });
    let telemetry = *app.world().resource::<LastStandTelemetry>();
    assert_eq!(telemetry.drops_spawned, 3);
    assert_eq!(telemetry.drops_skipped, 0);
}

#[test]
fn simultaneous_eliminations_share_a_place_and_a_final_pair_draws() {
    let mut app = app();
    for team in 0..4 {
        fighter(&mut app, team, 0);
    }
    step(&mut app);
    defeat(&mut app, 1);
    defeat(&mut app, 2);
    step(&mut app);
    assert_eq!(state(&mut app).placements[1..3], [3, 3]);
    assert!(outcome(&mut app).is_none());
    defeat(&mut app, 0);
    defeat(&mut app, 3);
    step(&mut app);
    let last = state(&mut app);
    assert_eq!(last.alive, 0);
    assert_eq!(last.placements[..4], [1, 3, 3, 1]);
    assert_eq!(outcome(&mut app).unwrap().result, MatchResult::Draw);
}

#[test]
fn a_departed_fighter_is_eliminated_without_drops() {
    let mut app = app();
    let fighters = [0, 1, 2].map(|team| fighter(&mut app, team, 2));
    step(&mut app);
    app.world_mut().entity_mut(fighters[1]).despawn();
    step(&mut app);
    let after = state(&mut app);
    assert_eq!(after.alive, 2);
    assert_eq!(after.placements[1], 3);
    assert!(pickups(&mut app).is_empty());
}

#[test]
fn the_deadline_draws_and_every_survivor_shares_first_place() {
    let mut app = app();
    for team in 0..4 {
        fighter(&mut app, team, 0);
    }
    step(&mut app);
    defeat(&mut app, 3);
    step(&mut app);
    app.world_mut().resource_mut::<SimulationTick>().0 = ENDS_AT;
    step(&mut app);
    let pending = outcome(&mut app).unwrap();
    assert_eq!(pending.cause, ModeOutcomeCause::Timeout);
    assert_eq!(pending.result, MatchResult::Draw);
    assert_eq!(state(&mut app).placements[..4], [1, 1, 1, 4]);
}

#[test]
fn a_defeat_on_the_deadline_tick_keeps_the_draw_standings() {
    let mut app = app();
    for team in 0..2 {
        fighter(&mut app, team, 0);
    }
    step(&mut app);
    app.world_mut().resource_mut::<SimulationTick>().0 = ENDS_AT;
    defeat(&mut app, 0);
    step(&mut app);
    let pending = outcome(&mut app).unwrap();
    assert_eq!(pending.cause, ModeOutcomeCause::Timeout);
    assert_eq!(pending.result, MatchResult::Draw);
    let last = state(&mut app);
    assert_eq!(last.placements[..2], [1, 1]);
    assert_eq!(last.alive, 2);
    assert!(pickups(&mut app).is_empty());
    assert_eq!(
        app.world()
            .resource::<MatchOutcomeDiagnostics>()
            .duplicate_mode_outcome,
        0
    );
}

#[test]
fn a_defeat_outside_the_ring_drops_inside_the_ring() {
    let mut app = app();
    let fighters = [0, 1].map(|team| fighter(&mut app, team, 2));
    step(&mut app);
    // Jump to the final ring and stand far outside it.
    app.world_mut().resource_mut::<SimulationTick>().0 = ENDS_AT - 10;
    step(&mut app);
    let boundary = state(&mut app).boundary;
    assert!(boundary.is_final());
    let tick = app.world().resource::<SimulationTick>().0;
    assert!(!boundary.contains(spawn_position(&app, 0), tick));
    let _ = fighters;
    defeat(&mut app, 0);
    step(&mut app);
    let drops = pickups(&mut app);
    assert_eq!(drops.len(), 2);
    for (position, _) in drops {
        assert!(
            boundary.contains(position, tick),
            "{position} is inside the final ring"
        );
    }
}

#[test]
fn the_boundary_publishes_phase_changes_to_state_and_bots() {
    let mut app = app();
    fighter(&mut app, 0, 0);
    fighter(&mut app, 1, 0);
    step(&mut app);
    let opening = state(&mut app).boundary;
    app.world_mut().resource_mut::<SimulationTick>().0 = opening.shrink_ends_at_tick;
    step(&mut app);
    let next = state(&mut app).boundary;
    assert_eq!(next.phase, 1);
    assert!((next.radius_from - opening.radius_to).abs() < 0.01);
    let world = app.world_mut();
    let view = *world
        .query_filtered::<&crate::matchplay::BotObjectiveView, With<MatchRoot>>()
        .single(world)
        .unwrap();
    assert!(matches!(
        view,
        crate::matchplay::BotObjectiveView::SurviveBoundary { next_radius, .. }
            if (next_radius - next.radius_to).abs() < 0.01
    ));
}
