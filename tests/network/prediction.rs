//! M03 owner-prediction comparison matrix: baseline versus the experimental candidate
//! under deterministic impairment profiles, measuring the accepted gate facts.
//!
//! Gates (milestone-03): reduce p95 input-to-visible latency by at least two fixed ticks
//! at the 100 ms-RTT-equivalent profile, converge to within one world unit within twelve
//! ticks after an impairment/correction, never cross or persistently penetrate terrain,
//! and keep p95 render-space correction within the 24-unit fighter radius.

use super::harness::Harness;
use bevy::prelude::{With, Without};
use brawler::client::prediction::{
    OwnerPredictedPose, OwnerPredictionSettings, OwnerPredictionStats,
};
use brawler::combat::{AuthoritativePose, TestDummy};
use brawler::map::{MapRoot, ResolvedMapSnapshot};
use brawler::protocol::{Fighter, FighterInput};
use lightyear::prelude::Controlled;

/// Receive-delay ticks per profile: 0 (local), 1 (~33 ms RTT), 3 (~100 ms RTT at 60 Hz).
const PROFILES: [(&str, usize); 3] = [("local", 0), ("typical", 1), ("adverse", 3)];
/// Fighter radius used by the correction-magnitude gate.
const FIGHTER_RADIUS: f32 = 24.0;
/// Movement displacement that counts as visibly moved for the latency probe.
const VISIBLE_DELTA_UNITS: f32 = 1.0;
/// Convergence window and bound after a correction.
const CONVERGENCE_WINDOW_TICKS: usize = 12;
const CONVERGENCE_BOUND_UNITS: f32 = 1.0;

fn joined_sandbox_harness(delay: usize, prediction: bool) -> Harness {
    let mut harness = Harness::new(1);
    harness.set_replication_delay(0, delay);
    if prediction {
        harness.clients[0]
            .world_mut()
            .insert_resource(OwnerPredictionSettings { enabled: true });
    }
    harness.step_until(|harness| harness.client_is_active(0) && harness.server_ids().len() == 1);
    // Let the initial authoritative pose and loadout replicate fully.
    for _ in 0..24 {
        harness.step();
    }
    harness
}

fn client_authoritative_position(harness: &mut Harness) -> Option<(u64, bevy::prelude::Vec2)> {
    let world = harness.clients[0].world_mut();
    let mut query = world.query_filtered::<&AuthoritativePose, (With<Fighter>, With<Controlled>)>();
    query.iter(world).next().map(|pose| {
        (
            pose.tick,
            bevy::prelude::Vec2::new(pose.position.x, pose.position.y),
        )
    })
}

fn client_predicted_position(harness: &mut Harness) -> Option<(u64, bevy::prelude::Vec2)> {
    let world = harness.clients[0].world_mut();
    let mut query =
        world.query_filtered::<&OwnerPredictedPose, (With<Fighter>, With<Controlled>)>();
    query
        .iter(world)
        .next()
        .map(|pose| (pose.tick, pose.position))
}

/// Fixed ticks from the input change until the visible pose first reflects it.
fn visible_latency(harness: &mut Harness, predicted: bool, direction: f32) -> usize {
    harness.set_controlled_input(0, FighterInput::default());
    for _ in 0..12 {
        harness.step();
    }
    let baseline = if predicted {
        client_predicted_position(harness).map(|(_, position)| position)
    } else {
        client_authoritative_position(harness).map(|(_, position)| position)
    }
    .expect("visible pose present");
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(bevy::prelude::Vec2::new(direction, 0.0), None, 0),
    );
    for latency in 0..60 {
        harness.step();
        let visible = if predicted {
            client_predicted_position(harness).map(|(_, position)| position)
        } else {
            client_authoritative_position(harness).map(|(_, position)| position)
        }
        .expect("visible pose present");
        if direction * (visible.x - baseline.x) >= VISIBLE_DELTA_UNITS {
            harness.set_controlled_input(0, FighterInput::default());
            return latency + 1;
        }
    }
    panic!("visible pose never reflected the input change");
}

#[test]
fn owner_prediction_reduces_visible_latency_by_at_least_two_ticks_at_adverse() {
    let mut results = Vec::new();
    for (profile, delay) in PROFILES {
        let mut baseline = joined_sandbox_harness(delay, false);
        let baseline_latency = visible_latency(&mut baseline, false, 1.0);
        let mut candidate = joined_sandbox_harness(delay, true);
        let candidate_latency = visible_latency(&mut candidate, true, 1.0);
        println!(
            "prediction latency [{profile} delay={delay}]: baseline {baseline_latency} ticks, candidate {candidate_latency} ticks"
        );
        results.push((profile, baseline_latency, candidate_latency));
    }
    let (_, baseline_latency, candidate_latency) = results
        .iter()
        .find(|(profile, _delay, _candidate)| *profile == "adverse")
        .copied()
        .expect("adverse profile measured");
    assert!(
        baseline_latency - candidate_latency >= 2,
        "gate: candidate must beat baseline by >=2 ticks at adverse (baseline {baseline_latency}, candidate {candidate_latency})"
    );
    for (_, baseline, candidate) in results {
        assert!(candidate <= baseline);
    }
}

#[test]
fn owner_prediction_corrections_stay_within_the_fighter_radius() {
    for (profile, delay) in PROFILES {
        let mut harness = joined_sandbox_harness(delay, true);
        // Weave left/right so each direction change produces a divergence to correct.
        for cycle in 0..12 {
            let direction = if cycle % 2 == 0 { 1.0 } else { -1.0 };
            harness.set_controlled_input(
                0,
                FighterInput::from_axes(bevy::prelude::Vec2::new(direction, 0.0), None, 0),
            );
            for _ in 0..10 {
                harness.step();
            }
        }
        harness.set_controlled_input(0, FighterInput::default());
        for _ in 0..12 {
            harness.step();
        }
        let stats = harness.clients[0]
            .world()
            .resource::<OwnerPredictionStats>();
        let p95 = stats.correction_percentile(0.95);
        println!(
            "prediction corrections [{profile} delay={delay}]: reconciliations {}, corrections {}, p95 {p95:.2} units",
            stats.reconciliations, stats.corrections
        );
        assert!(
            stats.reconciliations > 0,
            "authoritative poses must reconcile the prediction"
        );
        assert!(
            p95 <= FIGHTER_RADIUS,
            "gate: p95 correction {p95:.2} exceeds the 24-unit fighter radius"
        );
    }
}

#[test]
fn owner_prediction_converges_within_one_unit_after_an_impairment_burst() {
    let mut harness = joined_sandbox_harness(3, true);
    harness.set_controlled_input(0, FighterInput::from_axes(bevy::prelude::Vec2::X, None, 0));
    for _ in 0..10 {
        harness.step();
    }
    harness.arm_packet_impairment(0);
    // Zero the input for the window: both paths then describe matching simulation ticks,
    // so the measured gap is the correction residue the gate is about.
    harness.set_controlled_input(0, FighterInput::default());
    let mut converged_at: Option<usize> = None;
    for step in 0..CONVERGENCE_WINDOW_TICKS + 8 {
        harness.step();
        let predicted = client_predicted_position(&mut harness).map(|(tick, _)| tick);
        let authoritative = client_authoritative_position(&mut harness);
        if let (Some(_predicted_tick), Some((_, authoritative_position))) =
            (predicted, authoritative)
            && let Some((_, predicted_position)) = client_predicted_position(&mut harness)
        {
            let error = predicted_position.distance(authoritative_position);
            if error <= CONVERGENCE_BOUND_UNITS && converged_at.is_none() {
                converged_at = Some(step + 1);
            }
        }
    }
    harness.set_controlled_input(0, FighterInput::default());
    let converged_at = converged_at.expect("prediction reconverged after the burst");
    println!("prediction convergence after impairment: {converged_at} ticks");
    assert!(
        converged_at <= CONVERGENCE_WINDOW_TICKS,
        "gate: convergence took {converged_at} ticks (> {CONVERGENCE_WINDOW_TICKS})"
    );
}

#[test]
fn owner_prediction_never_persistently_penetrates_static_arena_geometry() {
    let mut harness = joined_sandbox_harness(1, true);
    // Find a static wall placement to drive into.
    let (wall, spawn) = {
        let world = harness.server.world_mut();
        let snapshot = world
            .query_filtered::<&ResolvedMapSnapshot, With<MapRoot>>()
            .single(world)
            .expect("resolved map snapshot")
            .clone();
        let mut fighters = world
            .query_filtered::<&avian2d::prelude::Position, (With<Fighter>, Without<TestDummy>)>();
        let spawn = fighters.single(world).expect("one fighter").0;
        let wall = snapshot
            .geometry
            .iter()
            .max_by_key(|placement| {
                let target = placement.position;
                -(spawn.distance_squared(target) as i64)
            })
            .expect("static geometry exists")
            .position;
        (wall, spawn)
    };
    let direction = (wall - spawn).normalize();
    harness.set_controlled_input(0, FighterInput::from_axes(direction, None, 0));
    let snapshot = {
        let world = harness.clients[0].world_mut();
        world
            .query_filtered::<&ResolvedMapSnapshot, With<MapRoot>>()
            .single(world)
            .expect("client map snapshot")
            .clone()
    };
    let mut consecutive_penetrations = 0;
    let mut worst_streak = 0;
    for _ in 0..120 {
        harness.step();
        if let Some((_, position)) = client_predicted_position(&mut harness) {
            // Penetration means pushed meaningfully into geometry, not grazing the
            // surface within the resolution tolerance.
            let resolved = brawler::client::prediction::resolve_static_arena(
                position,
                FIGHTER_RADIUS,
                &snapshot,
            );
            let depth = position.distance(resolved);
            if depth > 0.5 {
                consecutive_penetrations += 1;
                worst_streak = worst_streak.max(consecutive_penetrations);
            } else {
                consecutive_penetrations = 0;
            }
        }
    }
    harness.set_controlled_input(0, FighterInput::default());
    println!(
        "prediction static-arena penetration: worst streak {worst_streak} ticks (drive toward a wall)"
    );
    assert!(
        worst_streak <= 1,
        "gate: predicted pose persistently penetrated static arena geometry for {worst_streak} ticks"
    );
}

#[test]
fn owner_prediction_diverges_across_solid_destructible_terrain() {
    // The candidate resolves only static geometry from the map snapshot. Destructible
    // cells are server-authoritative and unmodelled, so driving into a still-solid
    // destructible region must produce measured divergence: this is the recorded
    // evidence for the keep/defer decision rather than a pass/fail gate.
    let mut harness = joined_sandbox_harness(3, true);
    let convergence = {
        let world = harness.clients[0].world_mut();
        world
            .resource::<brawler::terrain::ClientTerrainConvergence>()
            .clone()
    };
    // The central destructible reservation is the only occupied region before any brush.
    // Find its easternmost occupied cell so the staged drive enters real cells.
    let easternmost = convergence
        .chunks()
        .iter()
        .flat_map(|(chunk, bits)| {
            (0..brawler::terrain::TERRAIN_CHUNK_SIDE_CELLS).flat_map(move |local_y| {
                (0..brawler::terrain::TERRAIN_CHUNK_SIDE_CELLS).filter_map(move |local_x| {
                    bits.get(local_x, local_y).then(|| {
                        brawler::terrain::grid::chunk_min_world(*chunk)
                            + bevy::prelude::Vec2::new(
                                local_x as f32 * brawler::terrain::TERRAIN_CELL_SIZE_WORLD,
                                local_y as f32 * brawler::terrain::TERRAIN_CELL_SIZE_WORLD,
                            )
                    })
                })
            })
        })
        .max_by(|a, b| a.x.total_cmp(&b.x))
        .expect("committed destructible occupancy");
    let occupied_center = easternmost + bevy::prelude::Vec2::X * 8.0;
    // Teleport the authoritative fighter beside the region so the scripted drive reaches
    // still-solid cells deterministically regardless of arena layout.
    let edge = occupied_center + bevy::prelude::Vec2::X * (FIGHTER_RADIUS + 32.0);
    {
        let world = harness.server.world_mut();
        let mut fighters = world
            .query_filtered::<&mut avian2d::prelude::Position, (With<Fighter>, Without<TestDummy>)>(
            );
        *fighters.single_mut(world).expect("one fighter") = avian2d::prelude::Position(edge);
    }
    // Let the teleport resync settle, then reset the stats so the recorded correction
    // magnitude reflects the terrain drive rather than the staging jump.
    for _ in 0..12 {
        harness.step();
    }
    harness.clients[0]
        .world_mut()
        .insert_resource(OwnerPredictionStats::default());
    let direction = bevy::prelude::Vec2::NEG_X;
    harness.set_controlled_input(0, FighterInput::from_axes(direction, None, 0));
    let mut predicted_penetrations = 0;
    let mut authoritative_penetrations = 0;
    for step in 0..150 {
        harness.step();
        if step == 12 || step == 60 {
            // Bursts let the prediction run uncorrected across still-solid cells, which
            // is the gameplay-visible risk the decision must weigh.
            harness.arm_packet_impairment(0);
        }
        if let Some((_, position)) = client_predicted_position(&mut harness)
            && brawler::terrain::grid::circle_overlaps_occupied(
                position,
                FIGHTER_RADIUS,
                convergence.chunks(),
            )
        {
            predicted_penetrations += 1;
        }
        if let Some((_, position)) = client_authoritative_position(&mut harness)
            && brawler::terrain::grid::circle_overlaps_occupied(
                position,
                FIGHTER_RADIUS,
                convergence.chunks(),
            )
        {
            authoritative_penetrations += 1;
        }
    }
    harness.set_controlled_input(0, FighterInput::default());
    let stats = harness.clients[0]
        .world()
        .resource::<OwnerPredictionStats>()
        .clone();
    println!(
        "prediction destructible-terrain divergence: predicted penetration ticks {predicted_penetrations}, authoritative penetration ticks {authoritative_penetrations}, corrections {}, p95 {:.2} units",
        stats.corrections,
        stats.correction_percentile(0.95),
    );
    // Evidence expectation: the predicted pose crosses still-solid destructible cells
    // while the authoritative pose never does. Record it; the decision follows in the
    // milestone file.
    assert!(
        predicted_penetrations > 0 && authoritative_penetrations == 0,
        "destructible divergence probe must show predicted-only penetration"
    );
}
