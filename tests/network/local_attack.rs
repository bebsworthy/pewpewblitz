//! Local attack presentation: input-frame latency, echo suppression, rejection, and authority.
//!
//! Uses the same deterministic receive-delay profiles as owner prediction: four and seven
//! client ticks approximate 60 ms and 120 ms round trips at 60 Hz.

use super::harness::Harness;
use bevy::prelude::{Vec2, With};
use brawler::builds::{AbilityPhase, AbilityState, ResolvedMatchLoadout, UltimateKind};
use brawler::combat::{CombatTelemetry, WeaponPhase, WeaponState};
use brawler::matchplay::{MatchPhase, MatchRoot as MatchRootMarker, MatchState};
use brawler::protocol::{Fighter, FighterInput, MatchCommand, MatchCommandRequest, PlayerId};
use brawler_client::local_attack::{LocalAttackLedger, LocalAttackStats};

/// (label, receive-delay ticks): local, ~60 ms RTT, ~120 ms RTT.
const PROFILES: [(&str, usize); 3] = [("local", 0), ("rtt-60ms", 4), ("rtt-120ms", 7)];

/// Two clients in an active Wipeout match with settled prediction; client 0 has the delay.
fn active_harness(delay: usize) -> Harness {
    active_harness_with(delay, |_| {})
}

/// [`active_harness`] with a loadout setup applied before the match starts.
fn active_harness_with(delay: usize, setup: impl FnOnce(&mut Harness)) -> Harness {
    let mut harness = Harness::new_match(2);
    harness.set_replication_delay(0, delay);
    harness.step_until(|harness| {
        (0..2).all(|index| harness.client_is_active(index) && harness.loadout_is_ready(index))
    });
    setup(&mut harness);
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

/// Client 0 casts Self Cloak (ultimate 3), whose activation sound is a launch.
fn cloak_harness(delay: usize) -> Harness {
    let mut harness = active_harness_with(delay, |harness| {
        harness.install_saved_brawler_loadout(0, 1, 3, [4, 6]);
    });
    set_server_ability(&mut harness, |maximum| AbilityState {
        charge: maximum,
        phase: AbilityPhase::Ready,
    });
    for _ in 0..20 + 2 * delay {
        harness.step();
    }
    harness
}

fn set_server_ability(harness: &mut Harness, state: impl Fn(u16) -> AbilityState) {
    set_server_ability_of(harness, UltimateKind::SelfCloak, state);
}

fn set_server_ability_of(
    harness: &mut Harness,
    kind: UltimateKind,
    state: impl Fn(u16) -> AbilityState,
) {
    let player = harness.controlled_player_id(0);
    let world = harness.server.world_mut();
    let mut query = world
        .query_filtered::<(&PlayerId, &ResolvedMatchLoadout, &mut AbilityState), With<Fighter>>();
    let (_, loadout, mut ability) = query
        .iter_mut(world)
        .find(|(id, ..)| **id == player)
        .expect("the measured server fighter");
    assert_eq!(loadout.ultimate.kind, kind);
    *ability = state(loadout.ultimate.charge_policy.maximum);
}

fn server_ability(harness: &mut Harness) -> AbilityState {
    let player = harness.controlled_player_id(0);
    let world = harness.server.world_mut();
    let mut query = world.query_filtered::<(&PlayerId, &AbilityState), With<Fighter>>();
    *query
        .iter(world)
        .find(|(id, _)| **id == player)
        .expect("the measured server fighter")
        .1
}

fn ultimate() -> FighterInput {
    FighterInput::from_axes(Vec2::ZERO, None, FighterInput::ULTIMATE)
}

#[test]
fn local_ultimate_is_presented_on_the_input_frame_and_its_activation_echo_pairs() {
    for (profile, delay) in PROFILES {
        let mut harness = cloak_harness(delay);
        let start = stats(&harness);
        harness.set_controlled_input(0, ultimate());
        let mut presented = None;
        let mut echoed = None;
        for frame in 1..=90 {
            harness.step();
            if frame == 3 {
                harness.set_controlled_input(0, FighterInput::default());
            }
            let now = stats(&harness);
            if presented.is_none() && now.ultimates_presented > start.ultimates_presented {
                presented = Some(frame);
            }
            if echoed.is_none() && now.ultimate_activation_echoes > start.ultimate_activation_echoes
            {
                echoed = Some(frame);
            }
        }
        // Press again while the replicated charge is spent: nothing more is cast.
        harness.set_controlled_input(0, ultimate());
        for _ in 0..10 {
            harness.step();
        }
        harness.set_controlled_input(0, FighterInput::default());
        let stats = stats(&harness);
        let presented = presented.expect("cast presented locally");
        let echoed = echoed.expect("authoritative activation paired");
        println!(
            "local ultimate input-to-presented [{profile} delay={delay}]: local {presented} \
             frame(s), authoritative activation cue {echoed} frame(s) (before: cue-driven)"
        );
        assert_eq!(
            presented, 1,
            "[{profile}] cast presented on the input frame"
        );
        assert!(
            echoed > presented,
            "[{profile}] local presentation leads the cue"
        );
        assert_eq!(
            stats.ultimates_presented - start.ultimates_presented,
            1,
            "[{profile}] one cast"
        );
        assert_eq!(
            stats.ultimate_activation_echoes - start.ultimate_activation_echoes,
            1,
            "[{profile}] the activation sound is suppressed once (no double playback)"
        );
        assert_eq!(
            stats.unmatched_own_activations, start.unmatched_own_activations,
            "[{profile}] no activation is presented late"
        );
        assert_eq!(stats.ultimates_rejected, start.ultimates_rejected);
        assert!(
            matches!(
                server_ability(&mut harness).phase,
                AbilityPhase::Cloaked { .. }
            ),
            "[{profile}] the server activated the cloak"
        );
    }
}

#[test]
fn server_rejected_ultimate_is_rejected_locally_and_authority_is_unchanged() {
    for (profile, delay) in PROFILES.into_iter().skip(1) {
        let mut harness = cloak_harness(delay);
        // Authority spends the charge; the client still sees the stale full charge.
        let spent = AbilityState {
            charge: 0,
            phase: AbilityPhase::Charging,
        };
        set_server_ability(&mut harness, |_| spent);
        let start = stats(&harness);
        harness.set_controlled_input(0, ultimate());
        harness.step();
        harness.set_controlled_input(0, FighterInput::default());
        let mut rejected_after = None;
        let mut confirmed_after = None;
        for frame in 2..=90 {
            harness.step();
            let now = stats(&harness);
            if rejected_after.is_none() && now.ultimates_rejected > start.ultimates_rejected {
                rejected_after = Some(frame);
            }
            if confirmed_after.is_none() && now.ultimates_confirmed > start.ultimates_confirmed {
                confirmed_after = Some(frame);
            }
        }
        let stats = stats(&harness);
        println!(
            "rejected ultimate [{profile} delay={delay}]: presented {}, rejected after {:?} \
             frame(s), confirmed after {:?}",
            stats.ultimates_presented - start.ultimates_presented,
            rejected_after,
            confirmed_after,
        );
        assert_eq!(
            stats.ultimates_presented - start.ultimates_presented,
            1,
            "[{profile}] stale gate presents"
        );
        assert_eq!(
            stats.ultimate_activation_echoes, start.ultimate_activation_echoes,
            "[{profile}] no activation"
        );
        // A charge spent without an activation signal never confirms the cast; the transient
        // launch flash has already ended when it is rejected.
        assert_eq!(confirmed_after, None, "[{profile}] nothing confirmed");
        let rejected_after = rejected_after.expect("the unanswered cast is rejected");
        assert!(
            rejected_after <= 2 * delay + 12,
            "[{profile}] rejected within a bounded time"
        );
        assert!(
            harness.clients[0]
                .world()
                .resource::<LocalAttackLedger>()
                .ultimate_outstanding()
                .eq(&false),
            "[{profile}] the charge is not held by the rejected cast"
        );
        assert_eq!(
            server_ability(&mut harness),
            spent,
            "[{profile}] authority unchanged by the local presentation"
        );
    }
}

/// Client 0 casts Dash (ultimate 1) with full charge.
fn dash_harness(delay: usize) -> Harness {
    let mut harness = active_harness_with(delay, |harness| {
        harness.install_saved_brawler_loadout(0, 1, 1, [4, 6]);
    });
    set_server_ability_of(&mut harness, UltimateKind::Dash, |maximum| AbilityState {
        charge: maximum,
        phase: AbilityPhase::Ready,
    });
    for _ in 0..20 + 2 * delay {
        harness.step();
    }
    harness
}

/// The controlled fighter's replicated dash end tick on client 0, if it is dashing.
fn client_dash_end(harness: &mut Harness) -> Option<u64> {
    let world = harness.clients[0].world_mut();
    let mut query = world
        .query_filtered::<&AbilityState, (With<Fighter>, With<lightyear::prelude::Controlled>)>();
    query.iter(world).find_map(|ability| match ability.phase {
        AbilityPhase::Dashing { ends_at_tick } => Some(ends_at_tick),
        _ => None,
    })
}

#[test]
fn local_dash_sounds_on_the_input_frame_and_the_replicated_dash_is_its_echo() {
    for (profile, delay) in PROFILES {
        let mut harness = dash_harness(delay);
        let start = stats(&harness);
        harness.set_controlled_input(0, ultimate());
        let mut presented = None;
        let mut replicated = None;
        let mut echo_on_arrival = None;
        for frame in 1..=90 {
            harness.step();
            if frame == 3 {
                harness.set_controlled_input(0, FighterInput::default());
            }
            let now = stats(&harness);
            if presented.is_none() && now.ultimates_presented > start.ultimates_presented {
                presented = Some(frame);
            }
            if replicated.is_none()
                && let Some(ends_at_tick) = client_dash_end(&mut harness)
            {
                replicated = Some(frame);
                // The ability audio producer plays `dash` on this frame's replicated phase
                // change unless reconciliation already recorded it as an echo.
                echo_on_arrival = Some(
                    harness.clients[0]
                        .world()
                        .resource::<LocalAttackLedger>()
                        .is_activation_echo_dash(ends_at_tick),
                );
            }
        }
        let stats = stats(&harness);
        let presented = presented.expect("dash whoosh presented locally");
        let replicated = replicated.expect("the server dashed and its phase replicated");
        println!(
            "local dash input-to-sound [{profile} delay={delay}]: local {presented} frame(s), \
             replicated Dashing (before: sound start) {replicated} frame(s); echo on arrival \
             {echo_on_arrival:?}"
        );
        assert_eq!(presented, 1, "[{profile}] whoosh on the input frame");
        assert!(
            replicated > presented,
            "[{profile}] local sound leads the dash"
        );
        assert_eq!(
            echo_on_arrival,
            Some(true),
            "[{profile}] the replicated dash is paired before the producer sees it (no double \
             playback)"
        );
        assert_eq!(
            (
                stats.ultimates_presented - start.ultimates_presented,
                stats.ultimate_activation_echoes - start.ultimate_activation_echoes,
                stats.unmatched_own_activations - start.unmatched_own_activations,
                stats.ultimates_rejected - start.ultimates_rejected,
            ),
            (1, 1, 0, 0),
            "[{profile}] one cast, one echo, nothing unmatched or rejected"
        );
    }
}

#[test]
fn server_rejected_dash_leaves_only_the_local_sound() {
    for (profile, delay) in PROFILES.into_iter().skip(1) {
        let mut harness = dash_harness(delay);
        // Authority spends the charge; the client still sees the stale full charge.
        set_server_ability_of(&mut harness, UltimateKind::Dash, |_| AbilityState {
            charge: 0,
            phase: AbilityPhase::Charging,
        });
        let start = stats(&harness);
        harness.set_controlled_input(0, ultimate());
        harness.step();
        harness.set_controlled_input(0, FighterInput::default());
        let mut rejected_after = None;
        let mut dashed = false;
        for frame in 2..=90 {
            harness.step();
            dashed |= client_dash_end(&mut harness).is_some();
            if rejected_after.is_none()
                && stats(&harness).ultimates_rejected > start.ultimates_rejected
            {
                rejected_after = Some(frame);
            }
        }
        let stats = stats(&harness);
        println!(
            "rejected dash [{profile} delay={delay}]: presented {}, rejected after {rejected_after:?} \
             frame(s)",
            stats.ultimates_presented - start.ultimates_presented,
        );
        assert_eq!(stats.ultimates_presented - start.ultimates_presented, 1);
        assert!(!dashed, "[{profile}] the server never dashed");
        assert_eq!(stats.ultimates_confirmed, start.ultimates_confirmed);
        let rejected_after = rejected_after.expect("the unanswered dash is rejected");
        assert!(
            rejected_after <= 2 * delay + 12,
            "[{profile}] rejected within a bounded time"
        );
        assert!(
            matches!(server_ability(&mut harness).phase, AbilityPhase::Charging),
            "[{profile}] authority unchanged by the local presentation"
        );
    }
}
