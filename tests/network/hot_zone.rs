//! Deterministic Hot Zone authority, progress, recovery, and restart network scenarios.

use super::*;

fn server_match(harness: &mut Harness) -> MatchState {
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
    *query.single(world).expect("one server match")
}

fn server_hot_zone(harness: &mut Harness) -> brawler::matchplay::HotZoneState {
    let world = harness.server.world_mut();
    let mut query =
        world.query_filtered::<&brawler::matchplay::HotZoneState, With<MatchRootMarker>>();
    *query.single(world).expect("one server hot zone state")
}

fn client_hot_zone(
    harness: &mut Harness,
    index: usize,
) -> Option<brawler::matchplay::HotZoneState> {
    let world = harness.clients[index].world_mut();
    let mut query =
        world.query_filtered::<&brawler::matchplay::HotZoneState, With<MatchRootMarker>>();
    query.iter(world).next().copied()
}

fn server_position(harness: &mut Harness, player_id: PlayerId) -> Vec2 {
    let world = harness.server.world_mut();
    let mut fighters = world.query_filtered::<(&PlayerId, &Position), With<Fighter>>();
    fighters
        .iter(world)
        .find(|(player, _)| **player == player_id)
        .map(|(_, position)| position.0)
        .expect("server player position")
}

fn phase_result(state: &MatchState) -> Option<brawler::matchplay::MatchResult> {
    match state.phase {
        MatchPhase::Completed { result, .. } => Some(result),
        _ => None,
    }
}

/// The default harness budget is too short for spawn-to-objective travel; this variant
/// keeps every step deterministic while allowing the longer verification distances.
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

fn select_ready_and_activate(harness: &mut Harness, request_base: u64) {
    harness.step_until(|harness| (0..2).all(|index| harness.client_is_active(index)));
    let waiting = server_match(harness);
    for index in 0..2 {
        harness.send_build_selection(
            index,
            BuildSelectionRequest {
                request_id: request_base,
                match_id: waiting.match_id,
                selection: BuildSelection::Preset(BuildPresetId(u16::try_from(index + 1).unwrap())),
            },
        );
    }
    harness.step_until(|harness| (0..2).all(|index| harness.selection_is_complete(index)));
    let waiting = server_match(harness);
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: request_base + 1,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    step_until_budget(harness, 600, |harness| {
        matches!(server_match(harness).phase, MatchPhase::Active { .. })
    });
}

fn step_toward(harness: &mut Harness, index: usize, player: PlayerId, target: Vec2) {
    let position = server_position(harness, player);
    let axis = (target - position).normalize_or_zero();
    harness.set_controlled_input(index, FighterInput::from_axes(axis, Some(axis), 0));
    harness.step();
}

fn drive_to(harness: &mut Harness, index: usize, player: PlayerId, target: Vec2, tolerance: f32) {
    for _ in 0..2_000 {
        if server_position(harness, player).distance(target) <= tolerance {
            return;
        }
        step_toward(harness, index, player, target);
    }
    panic!("controlled fighter never reached {target:?}");
}

/// Route through the low lane then stage just outside the objective so symmetric entries
/// cannot race past the short verification threshold before the second team arrives.
fn stage_outside_zone(harness: &mut Harness, index: usize) -> PlayerId {
    let player = harness.controlled_player_id(index);
    let spawn = server_position(harness, player);
    let lane = Vec2::new(spawn.x.signum() * 700.0, -400.0);
    drive_to(harness, index, player, lane, 48.0);
    harness.set_controlled_input(index, FighterInput::default());
    let stage = Vec2::new(spawn.x.signum() * 230.0, 0.0);
    drive_to(harness, index, player, stage, 24.0);
    harness.set_controlled_input(index, FighterInput::default());
    player
}

fn release_into_zone(harness: &mut Harness, index: usize, player: PlayerId) {
    // The canonical 100-unit movement speed needs more ticks than the historical 320-unit speed
    // to route around the staging cover and enter the zone.
    for _ in 0..800 {
        step_toward(harness, index, player, Vec2::ZERO);
        let occupants = server_hot_zone(harness).occupants;
        if occupants[index.min(1)] > 0 {
            harness.set_controlled_input(index, FighterInput::default());
            return;
        }
    }
    panic!("staged fighter never entered the objective");
}

fn release_both_into_zone(harness: &mut Harness, players: [PlayerId; 2]) {
    for _ in 0..800 {
        let occupants = server_hot_zone(harness).occupants;
        if occupants == [1, 1] {
            for index in 0..2 {
                harness.set_controlled_input(index, FighterInput::default());
            }
            return;
        }
        for (index, player) in players.into_iter().enumerate() {
            if occupants[index] == 0 {
                let position = server_position(harness, player);
                let axis = (-position).normalize_or_zero();
                harness.set_controlled_input(index, FighterInput::from_axes(axis, Some(axis), 0));
            } else {
                harness.set_controlled_input(index, FighterInput::default());
            }
        }
        harness.step();
    }
    panic!(
        "staged fighters never contested the objective; occupants={:?}, positions={:?}",
        server_hot_zone(harness).occupants,
        players.map(|player| server_position(harness, player))
    );
}

/// Pin the match to complete within a few ticks while keeping occupancy ineligible, so
/// injected progress decides the boundary result exactly as recovery would present it.
fn force_deadline_with_progress(harness: &mut Harness, progress: [u16; 2]) {
    let tick = harness.server_simulation_tick();
    let world = harness.server.world_mut();
    let mut roots =
        world.query_filtered::<&mut brawler::matchplay::HotZoneState, With<MatchRootMarker>>();
    let mut hot_zone = roots.single_mut(world).unwrap();
    hot_zone.progress_ticks = progress;
    hot_zone.next_evaluation_tick = tick + 4;
    drop(roots);
    let mut states = world.query_filtered::<&mut MatchState, With<MatchRootMarker>>();
    let mut state = states.single_mut(world).unwrap();
    if let MatchPhase::Active { ends_at_tick } = state.phase {
        state.phase = MatchPhase::Active {
            ends_at_tick: ends_at_tick.min(tick + 3),
        };
    }
}

fn restart_after_completion(harness: &mut Harness, request_base: u64) {
    step_until_budget(harness, 40, |harness| {
        matches!(
            server_match(harness).phase,
            MatchPhase::Completed {
                restart_unlocked_at_tick,
                ..
            } if harness.server_simulation_tick() >= restart_unlocked_at_tick
        )
    });
    let completed = server_match(harness);
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: request_base,
                match_id: completed.match_id,
                command: MatchCommand::ReadyForRestart,
            },
        );
    }
    step_until_budget(harness, 60, |harness| {
        matches!(server_match(harness).phase, MatchPhase::Waiting)
    });
}

#[test]
fn hot_zone_unopposed_control_completes_threshold_and_converges() {
    let mut harness = Harness::new_hot_zone_match(2);
    select_ready_and_activate(&mut harness, 10);

    let player = stage_outside_zone(&mut harness, 0);
    release_into_zone(&mut harness, 0, player);
    assert_eq!(server_hot_zone(&mut harness).progress_ticks[0], 1);

    // Control alone advances one progress unit per eligible tick.
    step_until_budget(&mut harness, 200, |harness| {
        matches!(server_match(harness).phase, MatchPhase::Completed { .. })
    });

    let hot_zone = server_hot_zone(&mut harness);
    assert_eq!(hot_zone.target_progress_ticks, 30);
    assert_eq!(hot_zone.progress_ticks, [30, 0]);
    assert_eq!(hot_zone.occupants, [1, 0]);
    assert_eq!(
        phase_result(&server_match(&mut harness)),
        Some(brawler::matchplay::MatchResult::TeamVictory { team: TeamId(0) })
    );

    // Both clients converge on the same durable objective generation and totals.
    step_until_budget(&mut harness, 240, |harness| {
        (0..2).all(|index| {
            client_hot_zone(harness, index).is_some_and(|state| state.progress_ticks == [30, 0])
        })
    });
    for index in 0..2 {
        let client_state = client_hot_zone(&mut harness, index).unwrap();
        assert_eq!(client_state.match_id, hot_zone.match_id);
        assert_eq!(client_state.zone_anchor_id, hot_zone.zone_anchor_id);
        assert_eq!(client_state.status, hot_zone.status);
    }
}

#[test]
fn hot_zone_contested_control_advances_neither_team() {
    let mut harness = Harness::new_hot_zone_match(2);
    select_ready_and_activate(&mut harness, 10);

    let players = [
        stage_outside_zone(&mut harness, 0),
        stage_outside_zone(&mut harness, 1),
    ];
    // Release both from symmetric staging points; neither side can accumulate the short
    // verification threshold before the other arrives.
    release_both_into_zone(&mut harness, players);
    let contested = server_hot_zone(&mut harness);
    assert_eq!(contested.occupants, [1, 1]);
    assert_eq!(
        contested.status,
        brawler::matchplay::HotZoneStatus::Contested
    );

    let frozen = contested.progress_ticks;
    for _ in 0..120 {
        harness.step();
    }
    let after = server_hot_zone(&mut harness);
    assert_eq!(after.occupants, [1, 1]);
    assert_eq!(
        after.status,
        brawler::matchplay::HotZoneStatus::Contested,
        "contested time does not advance either side"
    );
    assert_eq!(after.progress_ticks, frozen);
    assert!(frozen[0] < 30 && frozen[1] < 30);
    assert!(matches!(
        server_match(&mut harness).phase,
        MatchPhase::Active { .. }
    ));
}

#[test]
fn hot_zone_timeout_leader_tie_and_recovered_threshold_use_precedence() {
    let mut harness = Harness::new_hot_zone_match(2);
    select_ready_and_activate(&mut harness, 10);

    // Injected leader progress resolves the timeout comparison.
    force_deadline_with_progress(&mut harness, [7, 5]);
    step_until_budget(&mut harness, 40, |harness| {
        matches!(server_match(harness).phase, MatchPhase::Completed { .. })
    });
    assert_eq!(
        phase_result(&server_match(&mut harness)),
        Some(brawler::matchplay::MatchResult::TeamVictory { team: TeamId(0) })
    );
    assert_eq!(server_hot_zone(&mut harness).progress_ticks, [7, 5]);

    // Restart, then inject an exact tie: the timeout comparison draws.
    restart_after_completion(&mut harness, 20);
    select_ready_and_activate(&mut harness, 30);
    force_deadline_with_progress(&mut harness, [6, 6]);
    step_until_budget(&mut harness, 40, |harness| {
        matches!(server_match(harness).phase, MatchPhase::Completed { .. })
    });
    assert_eq!(
        phase_result(&server_match(&mut harness)),
        Some(brawler::matchplay::MatchResult::Draw)
    );

    // Restart, then inject a simultaneous threshold: recovered threshold outranks timeout
    // at the boundary tick without another capture evaluation.
    restart_after_completion(&mut harness, 40);
    select_ready_and_activate(&mut harness, 50);
    force_deadline_with_progress(&mut harness, [30, 30]);
    step_until_budget(&mut harness, 40, |harness| {
        matches!(server_match(harness).phase, MatchPhase::Completed { .. })
    });
    assert_eq!(server_hot_zone(&mut harness).progress_ticks, [30, 30]);
    assert_eq!(
        phase_result(&server_match(&mut harness)),
        Some(brawler::matchplay::MatchResult::Draw)
    );
}

#[test]
fn hot_zone_restart_resets_state_in_place_and_converges() {
    let mut harness = Harness::new_hot_zone_match(2);
    select_ready_and_activate(&mut harness, 10);
    let player = stage_outside_zone(&mut harness, 0);
    release_into_zone(&mut harness, 0, player);
    step_until_budget(&mut harness, 200, |harness| {
        matches!(server_match(harness).phase, MatchPhase::Completed { .. })
    });
    let completed = server_match(&mut harness);
    let previous_anchor = server_hot_zone(&mut harness).zone_anchor_id;

    restart_after_completion(&mut harness, 20);
    let restarted = server_match(&mut harness);
    assert_ne!(restarted.match_id, completed.match_id);
    let hot_zone = server_hot_zone(&mut harness);
    assert_eq!(hot_zone.match_id, restarted.match_id);
    assert_eq!(hot_zone.progress_ticks, [0, 0]);
    assert_eq!(hot_zone.occupants, [0, 0]);
    assert_eq!(hot_zone.status, brawler::matchplay::HotZoneStatus::Empty);
    assert_eq!(
        hot_zone.next_evaluation_tick,
        brawler::matchplay::HotZoneState::UNINITIALIZED_EVALUATION_TICK
    );
    assert_eq!(hot_zone.zone_anchor_id, previous_anchor);

    step_until_budget(&mut harness, 240, |harness| {
        (0..2).all(|index| {
            client_hot_zone(harness, index).is_some_and(|state| state.progress_ticks == [0, 0])
        })
    });
}

#[test]
fn hot_zone_client_forgery_cannot_mutate_authority_and_clock_generation_recovers() {
    let mut harness = Harness::new_hot_zone_match(2);
    select_ready_and_activate(&mut harness, 10);

    // Forge client-owned objective state and a stale clock generation.
    {
        let world = harness.clients[0].world_mut();
        let mut zones =
            world.query_filtered::<&mut brawler::matchplay::HotZoneState, With<MatchRootMarker>>();
        if let Some(mut zone) = zones.iter_mut(world).next() {
            zone.progress_ticks = [29, 29];
            zone.status = brawler::matchplay::HotZoneStatus::Contested;
        }
        let mut clocks =
            world.query_filtered::<&mut brawler::matchplay::MatchClock, With<MatchRootMarker>>();
        if let Some(mut clock) = clocks.iter_mut(world).next() {
            clock.match_id = brawler::matchplay::MatchId(999);
        }
    }
    for _ in 0..30 {
        harness.step();
    }
    let authoritative = server_hot_zone(&mut harness);
    assert_eq!(authoritative.progress_ticks, [0, 0]);
    assert_eq!(
        authoritative.status,
        brawler::matchplay::HotZoneStatus::Empty
    );

    // The client converges back to the authoritative durable state and clock generation.
    step_until_budget(&mut harness, 240, |harness| {
        client_hot_zone(harness, 0)
            .is_some_and(|state| state.progress_ticks == authoritative.progress_ticks)
    });
    let world = harness.clients[0].world_mut();
    let mut clocks =
        world.query_filtered::<&brawler::matchplay::MatchClock, With<MatchRootMarker>>();
    let clock = clocks.iter(world).next().copied().unwrap();
    let mut states = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
    let state = states.iter(world).next().copied().unwrap();
    assert_eq!(clock.match_id, state.match_id);
}

/// Three clients keep both teams connected after one disconnect, so the lingering
/// disconnected fighter inside the zone loses occupancy without resolving a forfeit.
#[test]
fn hot_zone_disconnected_fighter_inside_zone_loses_occupancy_while_active() {
    let mut harness = Harness::new_hot_zone_match(3);
    harness.step_until(|harness| (0..3).all(|index| harness.client_is_active(index)));
    let waiting = server_match(&mut harness);
    for index in 0..3 {
        harness.send_build_selection(
            index,
            BuildSelectionRequest {
                request_id: 1,
                match_id: waiting.match_id,
                selection: BuildSelection::Preset(BuildPresetId(u16::try_from(index + 1).unwrap())),
            },
        );
    }
    harness.step_until(|harness| (0..3).all(|index| harness.selection_is_complete(index)));
    let waiting = server_match(&mut harness);
    for index in 0..3 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 2,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    step_until_budget(&mut harness, 600, |harness| {
        matches!(server_match(harness).phase, MatchPhase::Active { .. })
    });

    let player = stage_outside_zone(&mut harness, 0);
    release_into_zone(&mut harness, 0, player);
    assert_eq!(server_hot_zone(&mut harness).occupants, [1, 0]);

    // Disconnect only this fighter's link; its team keeps another connected participant,
    // so the match stays active and occupancy must drop through roster membership alone.
    harness.clients[0].world_mut().trigger(Disconnect {
        entity: harness.client_entities[0],
    });
    step_until_budget(&mut harness, 40, |harness| {
        server_hot_zone(harness).occupants == [0, 0]
    });
    assert!(matches!(
        server_match(&mut harness).phase,
        MatchPhase::Active { .. }
    ));
    let frozen = server_hot_zone(&mut harness).progress_ticks;
    for _ in 0..60 {
        harness.step();
    }
    assert_eq!(server_hot_zone(&mut harness).progress_ticks, frozen);
}

#[test]
fn hot_zone_packet_impairment_and_duplicate_input_cannot_double_advance_progress() {
    let mut harness = Harness::new_hot_zone_match(2);
    select_ready_and_activate(&mut harness, 10);
    let player = stage_outside_zone(&mut harness, 0);
    release_into_zone(&mut harness, 0, player);

    // Arm deterministic drop/duplicate/delay/reorder impairment on the controlling client
    // and replay forged inputs for ticks the server has already consumed. Objective
    // progress is server-owned: it may advance at most one unit per server tick.
    harness.arm_packet_impairment(0);
    let target = harness.controlled_entity(0);
    let forged_tick = harness.server_tick().saturating_add(1);
    let forged = FighterInput::from_axes(Vec2::ZERO, Some(Vec2::X), FighterInput::PRIMARY_FIRE);
    for _ in 0..4 {
        harness.send_forged_input(
            0,
            lightyear::input::input_message::InputTarget::Entity(target),
            forged_tick,
            forged,
        );
    }
    let before = server_hot_zone(&mut harness).progress_ticks;
    let start_tick = harness.server_simulation_tick();
    for _ in 0..60 {
        harness.step();
    }
    let elapsed = harness.server_simulation_tick().saturating_sub(start_tick);
    let after = server_hot_zone(&mut harness).progress_ticks;
    // One unit per server tick until the verification threshold completes the match.
    let remaining = u64::from(server_hot_zone(&mut harness).target_progress_ticks - before[0]);
    assert_eq!(
        u64::from(after[0] - before[0]),
        elapsed.min(remaining),
        "exactly one unit per server tick until threshold"
    );
    assert_eq!(after[1], before[1]);
    assert!(
        harness.packet_impairment(0).injected,
        "the impaired batch actually exercised the link"
    );
}
