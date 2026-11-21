//! Deterministic Shard Rush authority, replication, hold, results, and restart scenarios.

use super::*;
use brawler::matchplay::{
    MatchResult, ObjectiveResult, ObjectiveTotals, Shard, ShardCarrier, ShardRushState,
};

fn server_match(harness: &mut Harness) -> MatchState {
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
    *query.single(world).expect("one server match")
}

fn server_shard_rush(harness: &mut Harness) -> ShardRushState {
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<&ShardRushState, With<MatchRootMarker>>();
    *query.single(world).expect("one server Shard Rush state")
}

fn client_shard_rush(harness: &mut Harness, index: usize) -> Option<ShardRushState> {
    let world = harness.clients[index].world_mut();
    let mut query = world.query_filtered::<&ShardRushState, With<MatchRootMarker>>();
    query.iter(world).next().copied()
}

fn shard_count(world: &mut bevy::prelude::World) -> usize {
    world
        .query_filtered::<(), With<Shard>>()
        .iter(world)
        .count()
}

fn server_fighter(harness: &mut Harness, player_id: PlayerId) -> (Vec2, u8) {
    let world = harness.server.world_mut();
    let mut fighters =
        world.query_filtered::<(&PlayerId, &Position, &ShardCarrier), With<Fighter>>();
    fighters
        .iter(world)
        .find(|(player, ..)| **player == player_id)
        .map(|(_, position, carrier)| (position.0, carrier.0))
        .expect("server fighter")
}

fn set_server_carried(harness: &mut Harness, player_id: PlayerId, count: u8) {
    let world = harness.server.world_mut();
    let mut fighters = world.query_filtered::<(&PlayerId, &mut ShardCarrier), With<Fighter>>();
    for (player, mut carrier) in fighters.iter_mut(world) {
        if *player == player_id {
            carrier.0 = count;
        }
    }
}

fn step_until_budget(
    harness: &mut Harness,
    budget: usize,
    mut condition: impl FnMut(&mut Harness) -> bool,
) {
    for _ in 0..budget {
        harness.step();
        if condition(harness) {
            return;
        }
    }
    panic!("network harness condition did not become true within {budget} steps");
}

fn ready_and_activate(harness: &mut Harness, clients: usize, request_id: u64) {
    harness.step_until(|harness| (0..clients).all(|index| harness.client_is_active(index)));
    harness.step_until(|harness| (0..clients).all(|index| harness.loadout_is_ready(index)));
    let waiting = server_match(harness);
    for index in 0..clients {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    step_until_budget(harness, 600, |harness| {
        matches!(server_match(harness).phase, MatchPhase::Active { .. })
    });
}

fn cell(harness: &Harness, x: u16, y: u16) -> Vec2 {
    harness
        .server
        .world()
        .resource::<ResolvedMap>()
        .snapshot
        .dimensions
        .cell_center(brawler::map::MapCell::new(x, y))
}

fn source(harness: &Harness) -> Vec2 {
    harness
        .server
        .world()
        .resource::<ResolvedMap>()
        .shard_source
        .expect("Prism Quarry has a shard source")
        .position
}

/// Drive the controlled fighter through waypoints until it carries `carried` shards.
fn collect_at_source(harness: &mut Harness, index: usize, carried: u8) -> PlayerId {
    let player = harness.controlled_player_id(index);
    let (spawn, _) = server_fighter(harness, player);
    // Team 0 spawns on the left; step out past its spawn shield before heading for the plaza.
    let lane = if spawn.x < 0.0 {
        cell(harness, 7, 10)
    } else {
        cell(harness, 25, 14)
    };
    let target = source(harness);
    for waypoint in [lane, target] {
        for _ in 0..2_000 {
            let (position, count) = server_fighter(harness, player);
            if count >= carried || position.distance(waypoint) <= 12.0 {
                break;
            }
            let axis = (waypoint - position).normalize_or_zero();
            harness.set_controlled_input(index, FighterInput::from_axes(axis, Some(axis), 0));
            harness.step();
        }
    }
    harness.set_controlled_input(index, FighterInput::default());
    step_until_budget(harness, 400, |harness| {
        server_fighter(harness, player).1 >= carried
    });
    player
}

#[test]
fn shard_rush_source_shards_replicate_and_collection_is_server_owned() {
    let mut harness = Harness::new_shard_rush_match(2);
    ready_and_activate(&mut harness, 2, 1);

    // The first active tick spawns one shard at the source; both clients see the entity.
    step_until_budget(&mut harness, 20, |harness| {
        shard_count(harness.server.world_mut()) >= 1
    });
    step_until_budget(&mut harness, 240, |harness| {
        (0..2).all(|index| shard_count(harness.clients[index].world_mut()) >= 1)
    });

    let player = collect_at_source(&mut harness, 0, 1);
    assert!(server_fighter(&mut harness, player).1 >= 1);
    let state = server_shard_rush(&mut harness);
    assert!(state.team_shards[0] >= 1);
    assert_eq!(state.team_shards[1], 0);

    // Both clients converge on the authoritative held totals.
    let expected = state.team_shards;
    step_until_budget(&mut harness, 240, |harness| {
        (0..2).all(|index| {
            client_shard_rush(harness, index).is_some_and(|client| {
                client.team_shards[0] >= expected[0] && client.match_id == state.match_id
            })
        })
    });
}

#[test]
fn shard_rush_hold_wins_publishes_results_and_restart_clears_every_shard() {
    let mut harness = Harness::new_shard_rush_match(2);
    let diagnostics =
        brawler::testing::capture_expected_late_input_diagnostics(&mut harness.server);
    ready_and_activate(&mut harness, 2, 1);
    let leader = harness.controlled_player_id(0);
    let rules = *harness
        .server
        .world()
        .resource::<brawler::matchplay::ShardRushRules>();

    // Reach the target on the server: the countdown starts at the full hold duration.
    set_server_carried(&mut harness, leader, rules.target_shards);
    step_until_budget(&mut harness, 10, |harness| {
        server_shard_rush(harness).countdown.is_some()
    });
    let countdown = server_shard_rush(&mut harness).countdown.unwrap();
    assert_eq!(countdown.team, TeamId(0));
    assert_eq!(
        countdown.ends_at_tick - countdown.started_at_tick,
        u64::from(rules.policy.hold_ticks)
    );

    // Losing the strict lead resets it; regaining it restarts from the full duration.
    set_server_carried(&mut harness, leader, rules.target_shards - 1);
    step_until_budget(&mut harness, 10, |harness| {
        server_shard_rush(harness).countdown.is_none()
    });
    set_server_carried(&mut harness, leader, rules.target_shards);
    step_until_budget(&mut harness, 10, |harness| {
        server_shard_rush(harness).countdown.is_some()
    });
    let restarted = server_shard_rush(&mut harness).countdown.unwrap();
    assert!(restarted.started_at_tick > countdown.started_at_tick);

    step_until_budget(
        &mut harness,
        usize::from(rules.policy.hold_ticks) + 20,
        |harness| matches!(server_match(harness).phase, MatchPhase::Completed { .. }),
    );
    let completed = server_match(&mut harness);
    assert!(matches!(
        completed.phase,
        MatchPhase::Completed {
            result: MatchResult::TeamVictory { team: TeamId(0) },
            ..
        }
    ));
    // Completion removes every ground shard on the next fixed tick.
    step_until_budget(&mut harness, 5, |harness| {
        shard_count(harness.server.world_mut()) == 0
    });

    // Every client receives the atomic terminal objective result.
    step_until_budget(&mut harness, 240, |harness| {
        (0..2).all(|index| {
            let world = harness.clients[index].world_mut();
            let mut results = world.query::<&ObjectiveResult>();
            results.iter(world).next().is_some_and(|result| {
                result.match_id == completed.match_id
                    && result.totals
                        == ObjectiveTotals::ShardRush {
                            shards: [rules.target_shards, 0],
                            target: rules.target_shards,
                        }
            })
        })
    });

    // Restart resets in place: no shard, no carried count, a fresh state for the next match.
    step_until_budget(&mut harness, 120, |harness| {
        matches!(
            server_match(harness).phase,
            MatchPhase::Completed {
                restart_unlocked_at_tick,
                ..
            } if harness.server_simulation_tick() >= restart_unlocked_at_tick
        )
    });
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 2,
                match_id: completed.match_id,
                command: MatchCommand::ReadyForRestart,
            },
        );
    }
    step_until_budget(&mut harness, 60, |harness| {
        matches!(server_match(harness).phase, MatchPhase::Waiting)
    });
    let next = server_match(&mut harness);
    assert_ne!(next.match_id, completed.match_id);
    let state = server_shard_rush(&mut harness);
    assert_eq!(state.match_id, next.match_id);
    assert_eq!(state.team_shards, [0, 0]);
    assert_eq!(state.countdown, None);
    assert_eq!(server_fighter(&mut harness, leader).1, 0);
    assert_eq!(shard_count(harness.server.world_mut()), 0);
    step_until_budget(&mut harness, 240, |harness| {
        (0..2).all(|index| {
            shard_count(harness.clients[index].world_mut()) == 0
                && client_shard_rush(harness, index)
                    .is_some_and(|client| client.match_id == next.match_id)
        })
    });
    let _late_inputs = diagnostics.finish();
}

/// Three clients keep both teams connected; the departing carrier scatters its shards at its
/// last position and its team total drops without a forfeit.
#[test]
fn shard_rush_disconnected_carrier_drops_its_shards() {
    let mut harness = Harness::new_shard_rush_match(3);
    ready_and_activate(&mut harness, 3, 1);
    let carrier = harness.controlled_player_id(0);
    set_server_carried(&mut harness, carrier, 2);
    step_until_budget(&mut harness, 10, |harness| {
        server_shard_rush(harness).team_shards[0] == 2
    });
    let before = shard_count(harness.server.world_mut());
    harness.clients[0].world_mut().trigger(Disconnect {
        entity: harness.client_entities[0],
    });
    step_until_budget(&mut harness, 60, |harness| {
        server_shard_rush(harness).team_shards[0] == 0
    });
    // The departed fighter is gone; its shards are loose on the floor for anyone to take.
    step_until_budget(&mut harness, 5, |harness| {
        shard_count(harness.server.world_mut()) >= before + 2
    });
    assert!(matches!(
        server_match(&mut harness).phase,
        MatchPhase::Active { .. }
    ));
}
