//! Owner movement prediction: input-to-visible latency, reconciliation, and authority.
//!
//! The harness models one-way server-to-client latency with a deterministic receive-delay
//! line in client ticks; client inputs reach the server on the next step. Delays of four and
//! seven ticks approximate 60 ms and 120 ms round trips at 60 Hz.

use super::harness::Harness;
use avian2d::prelude::Position;
use bevy::prelude::{Vec2, With};
use brawler::client::prediction::{OwnerPrediction, OwnerPredictionStats, OwnerPredictionTuning};
use brawler::combat::{AuthoritativePose, ExternalMotion, KnockbackFeedback, WorldPoint};
use brawler::map::{
    MapCatalogResource, MapDynamicState, MapRoot, ResolvedMapSnapshot,
    circle_overlaps_blocking_map, placement_world_center,
};
use brawler::matchplay::{MatchPhase, MatchRoot as MatchRootMarker, MatchState};
use brawler::protocol::{Fighter, FighterInput, MatchCommand, MatchCommandRequest, PlayerId};
use lightyear::prelude::Controlled;

/// (label, receive-delay ticks): local, ~60 ms RTT, ~120 ms RTT.
const PROFILES: [(&str, usize); 3] = [("local", 0), ("rtt-60ms", 4), ("rtt-120ms", 7)];
/// Displacement that counts as visibly moved.
const VISIBLE_DELTA_UNITS: f32 = 0.5;

/// Two clients in an active Wipeout match; client 0 is measured with the given delay.
fn joined_harness(delay: usize) -> Harness {
    let mut harness = Harness::new_match(2);
    harness.set_replication_delay(0, delay);
    harness.step_until(|harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.loadout_is_ready(index))
            && try_presented(harness).is_some()
    });
    let match_id = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
        query.single(world).expect("one match").match_id
    };
    // Input held before activation is not predicted: the server holds inactive fighters.
    harness.set_controlled_input(0, walk(Vec2::X));
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 70,
                match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    let start = presented(&mut harness);
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
        query
            .single(world)
            .is_ok_and(|state| matches!(state.phase, MatchPhase::Active { .. }))
    });
    assert!(
        presented(&mut harness).distance(start) < 0.05,
        "no prediction before the match is active"
    );
    harness.set_controlled_input(0, FighterInput::default());
    for _ in 0..30 + delay {
        harness.step();
    }
    assert!(owner(&mut harness).is_some());
    harness
}

fn owner(harness: &mut Harness) -> Option<OwnerPrediction> {
    let world = harness.clients[0].world_mut();
    let mut query = world.query_filtered::<&OwnerPrediction, (With<Fighter>, With<Controlled>)>();
    query.iter(world).next().cloned()
}

/// The pose the client presents for its own fighter (the renderer and camera read it).
fn presented(harness: &mut Harness) -> Vec2 {
    try_presented(harness).expect("one presented controlled fighter")
}

fn try_presented(harness: &mut Harness) -> Option<Vec2> {
    let world = harness.clients[0].world_mut();
    let mut query = world.query_filtered::<&Position, (With<Fighter>, With<Controlled>)>();
    query.single(world).ok().map(|position| position.0)
}

/// The newest authoritative pose the client has received for its own fighter.
fn received_authority(harness: &mut Harness) -> Vec2 {
    let world = harness.clients[0].world_mut();
    let mut query = world.query_filtered::<&AuthoritativePose, (With<Fighter>, With<Controlled>)>();
    let pose = query.single(world).expect("one controlled fighter");
    Vec2::new(pose.position.x, pose.position.y)
}

fn server_pose(harness: &mut Harness) -> Vec2 {
    let player = harness.controlled_player_id(0);
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<(&PlayerId, &Position), With<Fighter>>();
    query
        .iter(world)
        .find(|(id, _)| **id == player)
        .expect("the measured server fighter")
        .1
        .0
}

fn server_fighter(harness: &mut Harness) -> bevy::prelude::Entity {
    let player = harness.controlled_player_id(0);
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<(bevy::prelude::Entity, &PlayerId), With<Fighter>>();
    query
        .iter(world)
        .find(|(_, id)| **id == player)
        .expect("the measured server fighter")
        .0
}

/// Start a fresh measurement window.
fn reset_stats(harness: &mut Harness) {
    harness.clients[0]
        .world_mut()
        .insert_resource(OwnerPredictionStats::default());
}

fn stats(harness: &Harness) -> OwnerPredictionStats {
    harness.clients[0]
        .world()
        .resource::<OwnerPredictionStats>()
        .clone()
}

fn walk(direction: Vec2) -> FighterInput {
    FighterInput::from_axes(direction, None, 0)
}

/// Frames from the input change until the predicted presentation and the received
/// authoritative pose (the pre-prediction lower bound) each move visibly.
fn input_to_visible_frames(harness: &mut Harness) -> (usize, usize) {
    harness.set_controlled_input(0, FighterInput::default());
    for _ in 0..30 {
        harness.step();
    }
    let presented_start = presented(harness);
    let authority_start = received_authority(harness);
    harness.set_controlled_input(0, walk(Vec2::X));
    let mut predicted = None;
    let mut authority = None;
    for frame in 1..=60 {
        harness.step();
        if predicted.is_none() && presented(harness).x - presented_start.x >= VISIBLE_DELTA_UNITS {
            predicted = Some(frame);
        }
        if authority.is_none()
            && received_authority(harness).x - authority_start.x >= VISIBLE_DELTA_UNITS
        {
            authority = Some(frame);
        }
        if predicted.is_some() && authority.is_some() {
            break;
        }
    }
    harness.set_controlled_input(0, FighterInput::default());
    (
        predicted.expect("presented pose moved"),
        authority.expect("authoritative pose moved"),
    )
}

#[test]
fn owner_movement_is_visible_on_the_first_frame_at_simulated_latency() {
    for (profile, delay) in PROFILES {
        let mut harness = joined_harness(delay);
        let (predicted, authority) = input_to_visible_frames(&mut harness);
        println!(
            "owner input-to-visible [{profile} delay={delay}]: predicted {predicted} frame(s), \
             received authority {authority} frame(s) (before: plus interpolation delay)"
        );
        assert_eq!(
            predicted, 1,
            "[{profile}] the presented pose must move on the first frame after input sampling"
        );
        assert!(
            authority > predicted,
            "[{profile}] prediction must lead authority"
        );
    }
}

#[test]
fn steady_walking_reconciles_without_corrections_and_settles_on_authority() {
    for (profile, delay) in PROFILES {
        let mut harness = joined_harness(delay);
        reset_stats(&mut harness);
        harness.set_controlled_input(0, walk(Vec2::new(1.0, 0.3)));
        let mut previous = presented(&mut harness);
        let mut largest_frame_step: f32 = 0.0;
        for _ in 0..90 {
            harness.step();
            let current = presented(&mut harness);
            largest_frame_step = largest_frame_step.max(current.distance(previous));
            previous = current;
        }
        harness.set_controlled_input(0, FighterInput::default());
        for _ in 0..60 {
            harness.step();
        }
        let after = stats(&harness);
        let settled = presented(&mut harness).distance(server_pose(&mut harness));
        println!(
            "owner steady walk [{profile} delay={delay}]: {after:?}, largest frame step \
             {largest_frame_step:.3}, settled error {settled:.3}"
        );
        assert!(after.reconciliations > 60);
        assert_eq!(after.snaps, 0, "[{profile}] steady walking never snaps");
        assert_eq!(
            after.rebases, 0,
            "[{profile}] history covers the round trip"
        );
        // A late-arriving input can make the server repeat one tick of the previous input;
        // that is a genuine one-tick misprediction (about 1.2 units here), never more.
        assert!(
            after.max_smoothed_error < 2.0,
            "[{profile}] prediction matches authority within one tick of motion"
        );
        assert!(
            settled < 0.05,
            "[{profile}] presented pose settles on authority"
        );
    }
}

#[test]
fn forced_motion_and_statuses_converge_without_snapping() {
    let mut harness = joined_harness(7);
    reset_stats(&mut harness);
    let tick = harness.server_simulation_tick();
    // Server-applied knockback the client learns about only after the round trip.
    let entity = server_fighter(&mut harness);
    harness.server.world_mut().entity_mut(entity).insert((
        ExternalMotion {
            velocity: Vec2::new(450.0, 0.0),
            expires_at_tick: tick + 6,
        },
        KnockbackFeedback {
            velocity: WorldPoint { x: 450.0, y: 0.0 },
            expires_at_tick: tick + 6,
        },
    ));
    let mut previous = presented(&mut harness);
    let mut largest_frame_step: f32 = 0.0;
    for _ in 0..30 {
        harness.step();
        let current = presented(&mut harness);
        largest_frame_step = largest_frame_step.max(current.distance(previous));
        previous = current;
    }
    // Server-applied freeze while the client keeps pushing input.
    let tick = harness.server_simulation_tick();
    harness
        .server
        .world_mut()
        .get_mut::<brawler::combat::ActiveEffects>(entity)
        .expect("fighter effects")
        .cold
        .frozen_until_tick = Some(tick + 30);
    harness.set_controlled_input(0, walk(Vec2::Y));
    for _ in 0..45 {
        harness.step();
        let current = presented(&mut harness);
        largest_frame_step = largest_frame_step.max(current.distance(previous));
        previous = current;
    }
    harness.set_controlled_input(0, FighterInput::default());
    for _ in 0..60 {
        harness.step();
    }
    assert_converged_without_teleport(&mut harness, "forced motion + freeze", largest_frame_step);
}

/// Shared closeout for misprediction scenarios: no snap, no frame jump near the snap
/// threshold, and the presented pose settles on the authoritative pose.
fn assert_converged_without_teleport(
    harness: &mut Harness,
    scenario: &str,
    largest_frame_step: f32,
) {
    harness.set_controlled_input(0, FighterInput::default());
    for _ in 0..60 {
        harness.step();
    }
    let after = stats(harness);
    let settled = presented(harness).distance(server_pose(harness));
    println!(
        "owner {scenario} [delay=7]: {after:?}, largest frame step {largest_frame_step:.3}, \
         settled error {settled:.3}"
    );
    assert_eq!(
        after.snaps, 0,
        "{scenario}: corrections blend below the threshold"
    );
    assert!(
        largest_frame_step < OwnerPredictionTuning::default().snap_distance / 2.0,
        "{scenario}: no visible teleport"
    );
    assert!(
        settled < 0.05,
        "{scenario}: presented pose settles on authority"
    );
}

/// Track the largest presented movement between consecutive frames over `frames` steps.
fn step_tracking(harness: &mut Harness, frames: usize, largest: &mut f32) {
    let mut previous = presented(harness);
    for _ in 0..frames {
        harness.step();
        let current = presented(harness);
        *largest = largest.max(current.distance(previous));
        previous = current;
    }
}

#[test]
fn dash_converges_without_snapping() {
    let mut harness = joined_harness(7);
    let entity = server_fighter(&mut harness);
    {
        let world = harness.server.world_mut();
        *world
            .get_mut::<brawler::builds::AbilityState>(entity)
            .expect("ability state") = brawler::builds::AbilityState {
            charge: 1_000,
            phase: brawler::builds::AbilityPhase::Ready,
        };
    }
    for _ in 0..10 {
        harness.step();
    }
    reset_stats(&mut harness);
    let mut largest_frame_step: f32 = 0.0;
    harness.set_controlled_input(
        0,
        FighterInput::from_axes(Vec2::X, Some(Vec2::X), FighterInput::ULTIMATE),
    );
    step_tracking(&mut harness, 3, &mut largest_frame_step);
    harness.set_controlled_input(0, walk(Vec2::X));
    step_tracking(&mut harness, 40, &mut largest_frame_step);
    let dashed = harness
        .server
        .world()
        .get::<brawler::builds::AbilityState>(entity)
        .is_some_and(|state| state.charge == 0);
    assert!(dashed, "the server accepted the dash");
    assert_converged_without_teleport(&mut harness, "dash", largest_frame_step);
}

#[test]
fn destroyed_cover_converges_without_snapping() {
    let mut harness = joined_harness(7);
    // Find removable player-blocking cover with free space to its left.
    let (start, cover_center) = {
        let world = harness.server.world_mut();
        let catalog = world.resource::<MapCatalogResource>().0.clone();
        let radius = world
            .resource::<brawler::builds::BuildCatalogResource>()
            .0
            .fighter_body
            .radius;
        let mut roots =
            world.query_filtered::<(&ResolvedMapSnapshot, &MapDynamicState), With<MapRoot>>();
        let (snapshot, state) = roots.single(world).expect("one map");
        snapshot
            .placements
            .iter()
            .find_map(|placement| {
                let asset = catalog.asset(placement.asset_id)?;
                let profile = catalog.profile(asset.gameplay_profile_id)?;
                if profile.player_collision != brawler::map::PlayerCollision::Block
                    || profile.destruction
                        != brawler::map::MapDestructionBehavior::RemoveOnMapDestruction
                {
                    return None;
                }
                let center = placement_world_center(snapshot.dimensions, asset, placement);
                let footprint = asset.footprint_cells.rotated(placement.quarter_turns);
                let half_width = f32::from(footprint.width) * 16.0;
                let start = center - Vec2::X * (half_width + radius + 24.0);
                let clear = (0_u8..=8).all(|step| {
                    let probe = start - Vec2::X * (f32::from(step) * 4.0);
                    !circle_overlaps_blocking_map(probe, radius, snapshot, state, &catalog)
                });
                clear.then_some((start, center))
            })
            .expect("a removable cover placement with free space to its left")
    };
    let entity = server_fighter(&mut harness);
    harness
        .server
        .world_mut()
        .get_mut::<Position>(entity)
        .expect("server pose")
        .0 = start;
    for _ in 0..40 {
        harness.step();
    }
    // Walk into the cover until both sides agree that it blocks.
    harness.set_controlled_input(0, walk(Vec2::X));
    for _ in 0..60 {
        harness.step();
    }
    let blocked_x = server_pose(&mut harness).x;
    assert!(blocked_x < cover_center.x, "the cover blocks the fighter");
    reset_stats(&mut harness);
    let mut largest_frame_step: f32 = 0.0;
    harness.inject_map_destruction(950, (cover_center.x, cover_center.y), 2.0);
    step_tracking(&mut harness, 60, &mut largest_frame_step);
    assert!(
        server_pose(&mut harness).x > blocked_x + 20.0,
        "the server walked through the destroyed cover"
    );
    assert!(
        stats(&harness).smoothed_corrections > 0,
        "the client mispredicted the cover"
    );
    assert_converged_without_teleport(&mut harness, "destroyed cover", largest_frame_step);
}

#[test]
fn client_prediction_never_moves_the_authoritative_fighter() {
    let mut harness = joined_harness(4);
    let server_before = server_pose(&mut harness);
    // Corrupt the client's prediction far away; the server never hears about it.
    {
        let world = harness.clients[0].world_mut();
        let mut query =
            world.query_filtered::<&mut OwnerPrediction, (With<Fighter>, With<Controlled>)>();
        let mut prediction = query.single_mut(world).expect("one owner prediction");
        prediction.position += Vec2::new(300.0, 0.0);
    }
    for _ in 0..30 {
        harness.step();
    }
    assert_eq!(
        server_pose(&mut harness),
        server_before,
        "the server pose is unchanged by client prediction"
    );
    let snaps = stats(&harness).snaps;
    assert!(
        snaps >= 1,
        "the corrupted prediction snaps back to authority"
    );
    assert!(presented(&mut harness).distance(server_before) < 0.05);
}
