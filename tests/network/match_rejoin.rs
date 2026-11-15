//! BRL-yyeg: a routed participant whose match session is lost for longer than the session
//! timeout keeps a held seat for 60 s, driven meanwhile by the server-hosted Practice bot, and a
//! supervisor-rebound fresh session reclaims the same fighter. These scenarios use the production
//! match-worker graph and immutable manifest; the supervisor's rebind fact is delivered through
//! the worker's own control queue.

use super::*;
use brawler::testing::{
    SEAT_HOLD_TICKS, TestRoutedParticipant, expire_seat_hold_at, fighter_is_bot_controlled,
    queue_seat_rebind, seat_hold_expiry, take_seat_rebind_replies,
};
use brawler_routing::{ParticipantRebindBody, ParticipantReboundBody, RejoinReason};

/// Upper bound for a blacked-out session to be declared lost: the routed match worker applies
/// the production 10 s routed session timeout on its side.
const SESSION_LOSS_TICKS: usize = 12 * 60;

const PLAYER_ONE: TestRoutedParticipant = TestRoutedParticipant {
    player_id: 7,
    netcode_client_id: 8,
    peer_id: 9,
    team: 0,
};
const PLAYER_TWO: TestRoutedParticipant = TestRoutedParticipant {
    player_id: 10,
    netcode_client_id: 11,
    peer_id: 12,
    team: 1,
};

/// A 1v1 routed match with both participants connected, checked in, and Active.
fn active_routed_match() -> Harness {
    let mut harness = Harness::new_routed_match(&[PLAYER_ONE, PLAYER_TWO]);
    for participant in [PLAYER_ONE, PLAYER_TWO] {
        harness.add_routed_match_client(participant.netcode_client_id, participant.peer_id);
    }
    harness.step_until(|harness| harness.client_is_active(0) && harness.client_is_active(1));
    for _ in 0..240 {
        harness.check_in_routed_match(0);
        harness.check_in_routed_match(1);
        harness.step();
        if matches!(harness.server_match_phase(), MatchPhase::Active { .. }) {
            return harness;
        }
    }
    panic!("the routed match never became Active");
}

/// Cut one client off for longer than the session timeout and stop updating it, as a phone that
/// lost its network (or an app that was killed) would be. Returns once the server holds the seat.
fn lose_session(harness: &mut Harness, index: usize, player_id: u64) -> Entity {
    let fighter = harness.server_fighter(player_id).expect("fighter exists");
    harness.set_link_blackout(index, true);
    for _ in 0..SESSION_LOSS_TICKS {
        harness.step();
        if seat_hold_expiry(harness.server.world(), fighter).is_some() {
            break;
        }
    }
    harness.suspend_client(index);
    assert!(
        seat_hold_expiry(harness.server.world(), fighter).is_some(),
        "a session lost mid-match holds its seat"
    );
    fighter
}

fn rebind(
    harness: &mut Harness,
    rejoin_id: u64,
    participant: TestRoutedParticipant,
    netcode_client_id: u64,
    peer_id: u128,
) -> ParticipantReboundBody {
    queue_seat_rebind(
        harness.server.world_mut(),
        ParticipantRebindBody {
            rejoin_id: brawler_routing::RequestId::new(rejoin_id).unwrap(),
            player_id: brawler_routing::PlayerId::new(participant.player_id).unwrap(),
            peer_id: brawler_routing::PeerId::new(peer_id).unwrap(),
            netcode_client_id: brawler_routing::NetcodeClientId::new(netcode_client_id).unwrap(),
        },
    );
    harness.step();
    let mut replies = take_seat_rebind_replies(harness.server.world_mut());
    assert_eq!(replies.len(), 1, "every rebind is answered once");
    replies.remove(0)
}

/// Connect the rebound identity and wait until the same fighter is controlled again.
fn reclaim(harness: &mut Harness, fighter: Entity, netcode_client_id: u64, peer_id: u128) -> usize {
    let index = harness.add_routed_match_client(netcode_client_id, peer_id);
    harness.step_until(|harness| {
        harness.client_is_active(index)
            && seat_hold_expiry(harness.server.world(), fighter).is_none()
    });
    index
}

#[test]
fn a_session_lost_beyond_the_timeout_rejoins_the_same_bot_held_fighter() {
    let mut harness = active_routed_match();
    let fighter = lose_session(&mut harness, 0, PLAYER_ONE.player_id);
    let held_at = harness.server_simulation_tick();
    let expiry = seat_hold_expiry(harness.server.world(), fighter).unwrap();
    let remaining = expiry.saturating_sub(held_at);
    assert!(
        remaining <= SEAT_HOLD_TICKS && remaining + 60 >= SEAT_HOLD_TICKS,
        "the hold lasts 60 s from the loss"
    );
    assert!(fighter_is_bot_controlled(harness.server.world(), fighter));

    // The held seat counts for its team: a lone player's team does not forfeit, and the bot
    // drives the fighter through the authoritative input path meanwhile.
    let freshness_before = harness
        .server
        .world()
        .get::<brawler::movement::InputFreshness>(fighter)
        .unwrap()
        .last_fresh_tick;
    for _ in 0..120 {
        harness.step();
    }
    assert!(matches!(
        harness.server_match_phase(),
        MatchPhase::Active { .. }
    ));
    let freshness_after = harness
        .server
        .world()
        .get::<brawler::movement::InputFreshness>(fighter)
        .unwrap()
        .last_fresh_tick;
    assert!(
        freshness_after > freshness_before,
        "the bot commits fresh authoritative intent while the seat is held"
    );

    // A relaunched app: a new Netcode client on the fresh supervisor route.
    let reply = rebind(&mut harness, 1, PLAYER_ONE, 80, 81);
    assert_eq!(reply.refusal, None);
    let index = reclaim(&mut harness, fighter, 80, 81);

    assert_eq!(
        harness.server_fighter(PLAYER_ONE.player_id),
        Some(fighter),
        "the same authoritative fighter is reclaimed"
    );
    assert!(!fighter_is_bot_controlled(harness.server.world(), fighter));
    harness.step_until(|harness| {
        harness.clients[index]
            .world_mut()
            .query_filtered::<&PlayerId, (With<Fighter>, With<Controlled>)>()
            .iter(harness.clients[index].world())
            .next()
            .is_some()
    });
    assert_eq!(harness.controlled_player_id(index).0, PLAYER_ONE.player_id);

    // Control is the player's again: their input, not the bot's, moves the fighter.
    let start = harness.server.world().get::<Position>(fighter).unwrap().0;
    harness.set_controlled_input(index, FighterInput::from_axes(Vec2::X, Some(Vec2::X), 0));
    harness.step_until(|harness| {
        harness.server.world().get::<Position>(fighter).unwrap().0.x > start.x + 20.0
    });
    assert!(matches!(
        harness.server_match_phase(),
        MatchPhase::Active { .. }
    ));
}

#[test]
fn an_expired_hold_removes_the_fighter_and_forfeits_a_lone_team() {
    let mut harness = active_routed_match();
    let fighter = lose_session(&mut harness, 0, PLAYER_ONE.player_id);
    assert!(matches!(
        harness.server_match_phase(),
        MatchPhase::Active { .. }
    ));
    let now = harness.server_simulation_tick();
    expire_seat_hold_at(harness.server.world_mut(), fighter, now + 1);
    for _ in 0..10 {
        harness.step();
    }
    assert_eq!(harness.server_fighter(PLAYER_ONE.player_id), None);
    assert!(matches!(
        harness.server_match_phase(),
        MatchPhase::Completed {
            result: brawler::matchplay::MatchResult::Forfeit {
                winner: TeamId(1),
                departed_team: TeamId(0),
            },
            ..
        }
    ));
    // An expired seat cannot be rebound.
    let reply = rebind(&mut harness, 1, PLAYER_ONE, 80, 81);
    assert_eq!(reply.refusal, Some(RejoinReason::SeatExpired));
}

#[test]
fn another_identity_cannot_reclaim_a_held_seat() {
    let mut harness = active_routed_match();
    let fighter = lose_session(&mut harness, 0, PLAYER_ONE.player_id);

    // A connected participant's seat is never rebound.
    let reply = rebind(&mut harness, 1, PLAYER_TWO, 90, 91);
    assert_eq!(reply.refusal, Some(RejoinReason::SeatConnected));

    let reply = rebind(&mut harness, 2, PLAYER_ONE, 80, 81);
    assert_eq!(reply.refusal, None);
    // Neither another Netcode client on its own route, nor the replaced manifest identity, nor
    // another client on the rebound route is admitted.
    for (netcode_client_id, peer_id) in [(60, 61), (PLAYER_ONE.netcode_client_id, 62), (70, 81)] {
        let index = harness.add_routed_match_client(netcode_client_id, peer_id);
        for _ in 0..30 {
            harness.step();
        }
        assert!(!harness.client_is_active(index));
        harness.suspend_client(index);
        assert!(seat_hold_expiry(harness.server.world(), fighter).is_some());
        assert!(fighter_is_bot_controlled(harness.server.world(), fighter));
    }
    // The rebound identity still reclaims it.
    reclaim(&mut harness, fighter, 80, 81);
    assert_eq!(harness.server_fighter(PLAYER_ONE.player_id), Some(fighter));
}

#[test]
fn a_seat_rejoins_at_most_three_times_per_match() {
    let mut harness = active_routed_match();
    let mut index = 0;
    let mut fighter = harness.server_fighter(PLAYER_ONE.player_id).unwrap();
    for rejoin in 1..=u64::from(brawler_routing::MAX_REJOINS_PER_SEAT) {
        fighter = lose_session(&mut harness, index, PLAYER_ONE.player_id);
        let (netcode_client_id, peer_id) = (100 + rejoin, 200 + u128::from(rejoin));
        let reply = rebind(&mut harness, rejoin, PLAYER_ONE, netcode_client_id, peer_id);
        assert_eq!(reply.refusal, None, "rejoin {rejoin} is accepted");
        index = reclaim(&mut harness, fighter, netcode_client_id, peer_id);
        assert_eq!(harness.server_fighter(PLAYER_ONE.player_id), Some(fighter));
    }
    lose_session(&mut harness, index, PLAYER_ONE.player_id);
    let reply = rebind(&mut harness, 9, PLAYER_ONE, 150, 250);
    assert_eq!(reply.refusal, Some(RejoinReason::LimitReached));
    let index = harness.add_routed_match_client(150, 250);
    for _ in 0..30 {
        harness.step();
    }
    assert!(!harness.client_is_active(index));
    assert!(seat_hold_expiry(harness.server.world(), fighter).is_some());
}
