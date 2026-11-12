//! Local attack presentation: input-frame latency, echo suppression, rejection, and authority.
//!
//! Uses the same deterministic receive-delay profiles as owner prediction: four and seven
//! client ticks approximate 60 ms and 120 ms round trips at 60 Hz.

use super::harness::Harness;
use bevy::prelude::{Vec2, With};
use brawler::client::local_attack::{LocalAttackLedger, LocalAttackStats};
use brawler::combat::{CombatTelemetry, WeaponPhase, WeaponState};
use brawler::matchplay::{MatchPhase, MatchRoot as MatchRootMarker, MatchState};
use brawler::protocol::{Fighter, FighterInput, MatchCommand, MatchCommandRequest, PlayerId};

/// (label, receive-delay ticks): local, ~60 ms RTT, ~120 ms RTT.
const PROFILES: [(&str, usize); 3] = [("local", 0), ("rtt-60ms", 4), ("rtt-120ms", 7)];

/// Two clients in an active Wipeout match with settled prediction; client 0 has the delay.
fn active_harness(delay: usize) -> Harness {
    let mut harness = Harness::new_match(2);
    harness.set_replication_delay(0, delay);
    harness.step_until(|harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.loadout_is_ready(index))
    });
    let match_id = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
        query.single(world).expect("one match").match_id
    };
    for index in 0..2 {
        harness.send_match_command(
            index,
            MatchCommandRequest {
                request_id: 71,
                match_id,
                command: MatchCommand::SetReady(true),
            },
        );
    }
    harness.step_until(|harness| {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
        query
            .single(world)
            .is_ok_and(|state| matches!(state.phase, MatchPhase::Active { .. }))
    });
    for _ in 0..30 + delay {
        harness.step();
    }
    harness
}

fn fire() -> FighterInput {
    FighterInput::from_axes(Vec2::ZERO, Some(Vec2::Y), FighterInput::PRIMARY_FIRE)
}

fn stats(harness: &Harness) -> LocalAttackStats {
    harness.clients[0]
        .world()
        .resource::<LocalAttackStats>()
        .clone()
}

fn server_accepted_attacks(harness: &Harness) -> u64 {
    harness
        .server
        .world()
        .resource::<CombatTelemetry>()
        .accepted_shots
}

fn server_weapon(harness: &mut Harness) -> WeaponState {
    let player = harness.controlled_player_id(0);
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<(&PlayerId, &WeaponState), With<Fighter>>();
    *query
        .iter(world)
        .find(|(id, _)| **id == player)
        .expect("the measured server fighter")
        .1
}

fn set_server_weapon(harness: &mut Harness, weapon: WeaponState) {
    let player = harness.controlled_player_id(0);
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<(&PlayerId, &mut WeaponState), With<Fighter>>();
    let (_, mut state) = query
        .iter_mut(world)
        .find(|(id, _)| **id == player)
        .expect("the measured server fighter");
    *state = weapon;
}

/// Frames from pressing fire until the local presentation, and until the authoritative cue
/// (the pre-change presentation start, excluding render) reaches the client.
fn input_to_presented_frames(harness: &mut Harness) -> (usize, usize) {
    let start = stats(harness);
    harness.set_controlled_input(0, fire());
    let mut presented = None;
    let mut confirmed = None;
    for frame in 1..=60 {
        harness.step();
        let now = stats(harness);
        if presented.is_none() && now.presented > start.presented {
            presented = Some(frame);
        }
        if confirmed.is_none() && now.confirmed > start.confirmed {
            confirmed = Some(frame);
        }
        if presented.is_some() && confirmed.is_some() {
            break;
        }
    }
    harness.set_controlled_input(0, FighterInput::default());
    (
        presented.expect("fire presented locally"),
        confirmed.expect("server cue paired"),
    )
}

#[test]
fn local_fire_is_presented_on_the_input_frame_at_simulated_latency() {
    for (profile, delay) in PROFILES {
        let mut harness = active_harness(delay);
        let (presented, confirmed) = input_to_presented_frames(&mut harness);
        println!(
            "local fire input-to-presented [{profile} delay={delay}]: local {presented} \
             frame(s), authoritative cue {confirmed} frame(s) (before: cue-driven)"
        );
        assert_eq!(
            presented, 1,
            "[{profile}] the attack must be presented on the first frame after input sampling"
        );
        assert!(
            confirmed > presented,
            "[{profile}] local presentation leads the cue"
        );
    }
}

#[test]
fn held_fire_pairs_every_server_attack_with_its_local_presentation() {
    for (profile, delay) in PROFILES {
        let mut harness = active_harness(delay);
        let accepted_before = server_accepted_attacks(&harness);
        let start = stats(&harness);
        harness.set_controlled_input(0, fire());
        for _ in 0..120 {
            harness.step();
        }
        harness.set_controlled_input(0, FighterInput::default());
        for _ in 0..30 + delay {
            harness.step();
        }
        let stats = stats(&harness);
        let presented = stats.presented - start.presented;
        let confirmed = stats.confirmed - start.confirmed;
        let unmatched = stats.unmatched_own_attacks - start.unmatched_own_attacks;
        let rejected = stats.rejected - start.rejected;
        let accepted = server_accepted_attacks(&harness) - accepted_before;
        println!(
            "held fire [{profile} delay={delay}]: server accepted {accepted}, local presented \
             {presented}, paired echoes {confirmed}, unpaired server attacks {unmatched}, \
             rejected {rejected}"
        );
        assert!(accepted >= 2, "[{profile}] the weapon fired repeatedly");
        assert_eq!(
            confirmed, accepted,
            "[{profile}] every accepted attack echoes a local presentation (no double playback)"
        );
        assert_eq!(
            unmatched, 0,
            "[{profile}] no attack is presented twice or late"
        );
        assert_eq!(presented, confirmed + rejected);
        assert_eq!(
            harness.clients[0]
                .world()
                .resource::<LocalAttackLedger>()
                .unconfirmed(),
            0,
            "[{profile}] nothing is left pending"
        );
    }
}

#[test]
fn server_rejected_fire_is_rejected_locally_and_authority_is_unchanged() {
    for (profile, delay) in PROFILES.into_iter().skip(1) {
        let mut harness = active_harness(delay);
        // Authority empties the weapon; the client still sees the stale loaded state.
        let empty = WeaponState {
            ammo: 0,
            phase: WeaponPhase::Cooldown {
                ready_at_tick: u64::MAX / 2,
            },
            ammo_recovery: None,
        };
        set_server_weapon(&mut harness, empty);
        let accepted_before = server_accepted_attacks(&harness);
        let start = stats(&harness);
        harness.set_controlled_input(0, fire());
        harness.step();
        harness.set_controlled_input(0, FighterInput::default());
        let mut rejected_after = None;
        for frame in 2..=90 {
            harness.step();
            if rejected_after.is_none() && stats(&harness).rejected > start.rejected {
                rejected_after = Some(frame);
            }
        }
        let stats = stats(&harness);
        println!(
            "rejected fire [{profile} delay={delay}]: presented {}, rejected after {:?} \
             frame(s), confirmed {}",
            stats.presented - start.presented,
            rejected_after,
            stats.confirmed - start.confirmed
        );
        assert_eq!(
            stats.presented - start.presented,
            1,
            "[{profile}] stale gate presents"
        );
        assert_eq!(
            stats.confirmed, start.confirmed,
            "[{profile}] nothing confirmed"
        );
        let rejected_after = rejected_after.expect("the unconfirmed shot is rejected");
        assert!(
            rejected_after <= 2 * delay + 12,
            "[{profile}] rejected within a bounded time"
        );
        assert_eq!(
            server_accepted_attacks(&harness),
            accepted_before,
            "[{profile}] the server never fired"
        );
        assert_eq!(
            server_weapon(&mut harness).phase,
            empty.phase,
            "[{profile}] authority unchanged by the local presentation"
        );
    }
}
