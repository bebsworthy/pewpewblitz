//! Deterministic repeated-match and reconnect soak scenarios for the M11 closeout.
//!
//! These soaks prove lifecycle stability and bounded state across many cycles; combat
//! fairness and pacing judgment stay with the human playtest. Completion is forced
//! through direct authoritative state writes so each cycle exercises the full
//! complete/restart/roster transaction without waiting on combat outcomes.

use super::harness::Harness;
use bevy::prelude::{With, Without};
use brawler::matchplay::{MatchPhase, MatchRoot as MatchRootMarker, MatchState, WipeoutState};
use brawler::protocol::{MatchCommand, MatchCommandRequest};
use brawler::testing::TestDummy;

/// Soak length per mode: the M11 locked budget of 25 completed matches.
const MATCH_SOAK_ROUNDS: usize = 25;
/// Reconnect cycles with exact cleanup assertions.
const RECONNECT_SOAK_ROUNDS: usize = 20;

fn server_match(harness: &mut Harness) -> MatchState {
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
    *query.single(world).expect("one server match")
}

fn step_until_labelled(
    harness: &mut Harness,
    label: &str,
    mut condition: impl FnMut(&mut Harness) -> bool,
) {
    for _ in 0..240 {
        harness.step();
        if condition(harness) {
            return;
        }
    }
    panic!("soak wait failed: {label}");
}

fn client_match(harness: &mut Harness, index: usize) -> Option<MatchState> {
    let world = harness.clients[index].world_mut();
    let mut query = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
    query.iter(world).next().copied()
}

fn ready_participants(harness: &mut Harness, clients: usize, sequence: &mut u64) {
    let waiting = server_match(harness);
    let ready_request_id = *sequence;
    for index in 0..clients {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: ready_request_id,
                match_id: waiting.match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    *sequence += 1;
    step_until_labelled(harness, "ready -> active", |harness| {
        matches!(server_match(harness).phase, MatchPhase::Active { .. })
    });
}

/// Bounded-state snapshot taken each restart: retained fighters, projectiles, match
/// telemetry records, and combat log records on the authoritative server.
struct ServerBounds {
    fighters: usize,
    projectiles: usize,
    match_records: usize,
    summaries: usize,
    combat_records: usize,
}

fn server_bounds(harness: &mut Harness) -> ServerBounds {
    let world = harness.server.world_mut();
    let mut fighters = world.query_filtered::<&brawler::protocol::Fighter, Without<TestDummy>>();
    let match_records = world
        .resource::<brawler::matchplay::MatchTelemetry>()
        .records
        .len();
    let summaries = world
        .resource::<brawler::matchplay::MatchTelemetry>()
        .summaries
        .len();
    let combat_records = world
        .resource::<brawler::combat::CombatTelemetry>()
        .records
        .len();
    ServerBounds {
        fighters: fighters.iter(world).count(),
        projectiles: harness.server_projectile_count(),
        match_records,
        summaries,
        combat_records,
    }
}

/// Complete the active Wipeout match by installing the target score authoritatively.
fn force_wipeout_completion(harness: &mut Harness) {
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<&mut WipeoutState, With<MatchRootMarker>>();
    let mut state = query.single_mut(world).expect("one wipeout state");
    state.team_scores[0] = state.target_score.max(1);
}

/// Complete the active Hot Zone match by installing the target progress authoritatively.
fn force_hot_zone_completion(harness: &mut Harness) {
    let world = harness.server.world_mut();
    let mut query =
        world.query_filtered::<&mut brawler::matchplay::HotZoneState, With<MatchRootMarker>>();
    let mut state = query.single_mut(world).expect("one hot zone state");
    state.progress_ticks[0] = state.target_progress_ticks.max(1);
}

fn run_match_soak(harness: &mut Harness, clients: usize, hot_zone: bool) {
    let started = std::time::Instant::now();
    let mut sequence = 10_u64;
    harness.step_until(|harness| {
        (0..clients).all(|index| harness.client_is_active(index) && harness.loadout_is_ready(index))
    });
    ready_participants(harness, clients, &mut sequence);
    let mut last_match_id = server_match(harness).match_id.0;
    let mut baseline: Option<ServerBounds> = None;
    for round in 0..MATCH_SOAK_ROUNDS {
        if round > 0 {
            sequence += 1;
            ready_participants(harness, clients, &mut sequence);
        }
        let _active = server_match(harness);
        if hot_zone {
            force_hot_zone_completion(harness);
        } else {
            force_wipeout_completion(harness);
        }
        step_until_labelled(harness, "forced completion", |harness| {
            matches!(server_match(harness).phase, MatchPhase::Completed { .. })
        });
        let completed = server_match(harness);
        step_until_labelled(harness, "clients observed completion", |harness| {
            (0..clients).all(|index| client_match(harness, index) == Some(completed))
        });
        // The completed-input lock rejects an immediate restart request; wait it out,
        // then confirm from every participant.
        for _ in 0..60 {
            harness.step();
        }
        sequence += 1;
        let restart_request_id = sequence;
        for index in 0..clients {
            harness.send_match_command(
                index,
                MatchCommandRequest {
                    request_id: restart_request_id,
                    match_id: completed.match_id,
                    command: MatchCommand::ReadyForRestart,
                },
            );
        }
        step_until_labelled(harness, "restart to waiting", |harness| {
            server_match(harness).match_id.0 > last_match_id
        });
        let restarted = server_match(harness);
        assert!(matches!(restarted.phase, MatchPhase::Waiting));
        last_match_id = restarted.match_id.0;

        let bounds = server_bounds(harness);
        if let Some(baseline) = &baseline {
            assert_eq!(
                bounds.fighters, baseline.fighters,
                "round {round}: retained fighter count grew"
            );
            assert_eq!(
                bounds.projectiles, baseline.projectiles,
                "round {round}: retained projectile count grew"
            );
            // Telemetry trackers keep bounded histories: they may refill after each match
            // but must never exceed their engine ceilings across the whole soak.
            assert!(
                bounds.combat_records <= 512,
                "round {round}: combat log exceeded its 512-record ceiling"
            );
            assert!(
                bounds.match_records <= 1024,
                "round {round}: match records exceeded their 1024-record ceiling"
            );
            assert!(
                bounds.summaries <= 128,
                "round {round}: match summaries exceeded their 128-record ceiling"
            );
        } else {
            baseline = Some(bounds);
        }

        // Client headless automation re-arms readiness and restart after each completed
        // match, so the next round begins by waiting for Active again.
        let _ = round;
        let _ = hot_zone;
    }
    println!(
        "{MATCH_SOAK_ROUNDS} authoritative {} matches completed and restarted in {:.1}s",
        if hot_zone { "hot-zone" } else { "wipeout" },
        started.elapsed().as_secs_f32()
    );
}

#[test]
fn wipeout_twenty_five_match_restart_soak_retains_bounded_state() {
    let diagnostics = brawler::testing::capture_expected_late_input_diagnostics();
    let mut harness = Harness::new_match(2);
    run_match_soak(&mut harness, 2, false);
    assert!(
        diagnostics.finish() > 0,
        "wipeout soak did not exercise the expected late-input correction path"
    );
}

#[test]
fn hot_zone_twenty_five_match_restart_soak_retains_bounded_state() {
    let diagnostics = brawler::testing::capture_expected_late_input_diagnostics();
    let mut harness = Harness::new_hot_zone_match(2);
    run_match_soak(&mut harness, 2, true);
    assert!(
        diagnostics.finish() > 0,
        "hot-zone soak did not exercise the expected late-input correction path"
    );
}

#[test]
fn reconnect_soak_alternates_disconnect_and_clean_new_sessions() {
    let diagnostics = brawler::testing::capture_expected_late_input_diagnostics();
    let mut harness = Harness::new_match(2);
    let mut sequence = 10_u64;
    step_until_labelled(&mut harness, "initial join", |harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.loadout_is_ready(index))
    });
    let static_count = harness.server_static_arena_count();
    // Client worlds accumulate as sessions reconnect; track the two live indices.
    let mut live = [0_usize, 1_usize];

    for round in 0..RECONNECT_SOAK_ROUNDS {
        // Ready the server-resolved saved-brawler loadouts.
        let waiting = server_match(&mut harness);
        sequence += 1;
        let ready_request_id = sequence;
        for index in [live[0], live[1]] {
            harness.send_match_command(
                index,
                MatchCommandRequest {
                    request_id: ready_request_id,
                    match_id: waiting.match_id,
                    command: MatchCommand::SetReady(true),
                },
            );
        }
        step_until_labelled(&mut harness, "readied to active", |harness| {
            matches!(server_match(harness).phase, MatchPhase::Active { .. })
        });

        // Disconnect the second live client: its owned authoritative state must be reclaimed.
        let disconnecting = live[1];
        harness.clients[disconnecting].world_mut().trigger(
            lightyear::prelude::client::Disconnect {
                entity: harness.client_entities[disconnecting],
            },
        );
        step_until_labelled(
            &mut harness,
            "disconnect reclaimed owned state",
            |harness| harness.server_ids().len() == 1,
        );
        assert_eq!(harness.server_projectile_count(), 0);

        // Complete and restart from the remaining client so the waiting phase can admit
        // a fresh session in the next cycle.
        force_wipeout_completion(&mut harness);
        step_until_labelled(&mut harness, "forced completion", |harness| {
            matches!(server_match(harness).phase, MatchPhase::Completed { .. })
        });
        let completed = server_match(&mut harness);
        for _ in 0..60 {
            harness.step();
        }
        sequence += 1;
        let restart_request_id = sequence;
        harness.send_match_command(
            0,
            MatchCommandRequest {
                request_id: restart_request_id,
                match_id: completed.match_id,
                command: MatchCommand::ReadyForRestart,
            },
        );
        step_until_labelled(&mut harness, "restart to waiting", |harness| {
            server_match(harness).match_id.0 > completed.match_id.0
        });
        let restarted = server_match(&mut harness);
        assert!(matches!(restarted.phase, MatchPhase::Waiting));

        // Reconnect as a genuinely new session: a fresh client world with a fresh
        // development Netcode ID, per the accepted no-resumption policy.
        harness.add_client(u64::try_from(100 + round).unwrap());
        let index = harness.clients.len() - 1;
        live[1] = index;
        step_until_labelled(&mut harness, "reconnected session active", |harness| {
            harness.client_is_active(index) && harness.server_ids().len() == 2
        });
        assert_eq!(harness.server_static_arena_count(), static_count);
    }
    assert!(
        diagnostics.finish() > 0,
        "reconnect soak did not exercise the expected late-input correction path"
    );
}
