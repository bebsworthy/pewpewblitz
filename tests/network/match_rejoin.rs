//! BRL-yyeg: a routed participant whose match session is lost for longer than the session
//! timeout keeps a held seat for 60 s, driven meanwhile by the server-hosted Practice bot, and a
//! supervisor-rebound fresh session reclaims the same fighter. These scenarios use the production
//! match-worker graph and immutable manifest; the supervisor's rebind fact is delivered through
//! the worker's own control queue.

use super::*;
use brawler::matchplay::{ParticipantControl, PublicParticipantState};
use brawler::testing::{
    SEAT_HOLD_TICKS, TestRoutedParticipant, expire_seat_hold_at, fighter_is_bot_controlled,
    queue_seat_rebind, ready_match_result, seat_hold_expiry, take_seat_rebind_replies,
    take_seat_releases,
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

/// The control fact client `index` sees in `player_id`'s public projection, if it has one.
fn client_seat_control(
    harness: &mut Harness,
    index: usize,
    player_id: u64,
) -> Option<ParticipantControl> {
    let world = harness.clients[index].world_mut();
    let mut query = world.query::<&PublicParticipantState>();
    query
        .iter(world)
        .find(|state| state.player_id.0 == player_id)
        .map(|state| state.control)
}

/// BRL-dwqx: the other player learns from the public roster that a bot holds the seat, with the
/// server's own hold expiry, while its own seat stays `Player`.
#[test]
fn a_held_seat_is_published_as_bot_controlled_to_the_other_players() {
    let mut harness = active_routed_match();
    assert_eq!(
        client_seat_control(&mut harness, 1, PLAYER_ONE.player_id),
        Some(ParticipantControl::Player)
    );
    let fighter = lose_session(&mut harness, 0, PLAYER_ONE.player_id);
    let expires_at_tick = seat_hold_expiry(harness.server.world(), fighter).unwrap();
    let held = ParticipantControl::HeldByBot { expires_at_tick };
    step_until_within(&mut harness, 60, |harness| {
        client_seat_control(harness, 1, PLAYER_ONE.player_id) == Some(held)
    });
    assert_eq!(
        client_seat_control(&mut harness, 1, PLAYER_TWO.player_id),
        Some(ParticipantControl::Player),
        "the connected player's own seat stays Player"
    );
}

/// BRL-dwqx: a reclaimed seat is `Player` again for everyone, and the rejoined client never sees
/// its own seat as bot-held.
#[test]
fn a_reclaimed_seat_is_published_as_player_again() {
    let mut harness = active_routed_match();
    let fighter = lose_session(&mut harness, 0, PLAYER_ONE.player_id);
    step_until_within(&mut harness, 60, |harness| {
        matches!(
            client_seat_control(harness, 1, PLAYER_ONE.player_id),
            Some(ParticipantControl::HeldByBot { .. })
        )
    });
    let reply = rebind(&mut harness, 1, PLAYER_ONE, 80, 81);
    assert_eq!(reply.refusal, None);
    let index = harness.add_routed_match_client(80, 81);
    let mut observed_own = Vec::new();
    for _ in 0..240 {
        harness.step();
        if let Some(control) = client_seat_control(&mut harness, index, PLAYER_ONE.player_id) {
            observed_own.push(control);
        }
        if harness.client_is_active(index)
            && seat_hold_expiry(harness.server.world(), fighter).is_none()
            && client_seat_control(&mut harness, 1, PLAYER_ONE.player_id)
                == Some(ParticipantControl::Player)
            && !observed_own.is_empty()
        {
            break;
        }
    }
    assert_eq!(
        client_seat_control(&mut harness, 1, PLAYER_ONE.player_id),
        Some(ParticipantControl::Player),
        "the other player sees the seat reclaimed"
    );
    assert!(
        !observed_own.is_empty(),
        "the rejoined client sees its seat"
    );
    for _ in 0..30 {
        harness.step();
        if let Some(control) = client_seat_control(&mut harness, index, PLAYER_ONE.player_id) {
            observed_own.push(control);
        }
    }
    assert!(
        observed_own
            .iter()
            .all(|control| *control == ParticipantControl::Player),
        "the rejoined client never sees its own seat held: {observed_own:?}"
    );
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
    assert_eq!(
        harness.controlled_player_ids(index),
        vec![PLAYER_ONE.player_id]
    );

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

/// BRL-4zx2: the rejoining client must own only its own fighter while another seat is held by a
/// server bot. A held fighter must not look "controlled" to a session that connects later, or the
/// client's first-`Controlled` lookups (HUD owner, camera, self ring, name plate, kill-feed
/// colour, input target) can land on the opponent.
#[test]
fn a_rejoin_while_the_other_seat_is_bot_held_controls_only_its_own_fighter() {
    let mut harness = active_routed_match();
    // The opponent's session ends first: its seat is held and a bot drives it.
    let opponent = lose_session(&mut harness, 1, PLAYER_TWO.player_id);
    assert!(fighter_is_bot_controlled(harness.server.world(), opponent));
    let fighter = lose_session(&mut harness, 0, PLAYER_ONE.player_id);
    assert!(matches!(
        harness.server_match_phase(),
        MatchPhase::Active { .. }
    ));

    let reply = rebind(&mut harness, 1, PLAYER_ONE, 80, 81);
    assert_eq!(reply.refusal, None);
    let index = reclaim(&mut harness, fighter, 80, 81);
    assert!(
        fighter_is_bot_controlled(harness.server.world(), opponent),
        "the opponent's seat stays bot-held"
    );
    harness.step_until(|harness| !harness.controlled_player_ids(index).is_empty());
    for _ in 0..30 {
        harness.step();
    }
    assert_eq!(
        harness.admitted_player_id(index),
        Some(PLAYER_ONE.player_id)
    );
    assert_eq!(
        harness.controlled_player_ids(index),
        vec![PLAYER_ONE.player_id],
        "the rejoined client owns its own fighter and nothing else"
    );
    // The local input buffer, and so the input target, is the player's own fighter only.
    let input_targets: Vec<u64> = {
        let world = harness.clients[index].world_mut();
        let mut query = world.query_filtered::<&PlayerId, (
            With<Fighter>,
            With<lightyear::prelude::input::native::InputMarker<FighterInput>>,
        )>();
        query.iter(world).map(|player| player.0).collect()
    };
    assert_eq!(input_targets, vec![PLAYER_ONE.player_id]);

    // The player's input reaches their own fighter, and the server authorizes every target.
    let start = harness.server.world().get::<Position>(fighter).unwrap().0;
    harness.set_controlled_input(index, FighterInput::from_axes(Vec2::X, Some(Vec2::X), 0));
    harness.step_until(|harness| {
        harness.server.world().get::<Position>(fighter).unwrap().0.x > start.x + 20.0
    });
    let link = harness.server_links[index];
    let rejections = harness
        .server
        .world()
        .get::<InputValidationState>(link)
        .expect("the rejoined link validates input")
        .ownership_rejections;
    assert_eq!(
        rejections, 0,
        "the client sends input only for its own fighter"
    );
    assert!(fighter_is_bot_controlled(harness.server.world(), opponent));

    // The opponent reclaims its seat afterwards: each client owns exactly its own fighter.
    let reply = rebind(&mut harness, 2, PLAYER_TWO, 90, 91);
    assert_eq!(reply.refusal, None);
    let opponent_index = reclaim(&mut harness, opponent, 90, 91);
    harness.step_until(|harness| !harness.controlled_player_ids(opponent_index).is_empty());
    for _ in 0..30 {
        harness.step();
    }
    assert_eq!(
        harness.controlled_player_ids(opponent_index),
        vec![PLAYER_TWO.player_id]
    );
    assert_eq!(
        harness.controlled_player_ids(index),
        vec![PLAYER_ONE.player_id]
    );
}

#[test]
fn an_expired_hold_removes_the_fighter_and_forfeits_a_lone_team() {
    let mut harness = active_routed_match();
    let fighter = lose_session(&mut harness, 0, PLAYER_ONE.player_id);
    assert!(matches!(
        harness.server_match_phase(),
        MatchPhase::Active { .. }
    ));
    step_until_within(&mut harness, 60, |harness| {
        matches!(
            client_seat_control(harness, 1, PLAYER_ONE.player_id),
            Some(ParticipantControl::HeldByBot { .. })
        )
    });
    let now = harness.server_simulation_tick();
    expire_seat_hold_at(harness.server.world_mut(), fighter, now + 1);
    for _ in 0..10 {
        harness.step();
    }
    assert_eq!(harness.server_fighter(PLAYER_ONE.player_id), None);
    // BRL-dwqx: the expired seat's public projection goes with the fighter.
    step_until_within(&mut harness, 60, |harness| {
        client_seat_control(harness, 1, PLAYER_ONE.player_id).is_none()
    });
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

    only_the_rebound_identity_reclaims(&mut harness, fighter, 2);
}

/// BRL-jxfy: the identity rules are unchanged once the match completed while the seat is held.
#[test]
fn another_identity_cannot_reclaim_a_held_seat_after_completion() {
    let mut harness = active_routed_match();
    let fighter = lose_session(&mut harness, 0, PLAYER_ONE.player_id);
    leave_on_purpose(&mut harness, 1, PLAYER_TWO.player_id);
    harness.step_until(|harness| server_match_result(harness).is_some());

    // The seat that left (and completion released) is never rebound.
    let reply = rebind(&mut harness, 1, PLAYER_TWO, 90, 91);
    assert_eq!(reply.refusal, Some(RejoinReason::SeatExpired));

    only_the_rebound_identity_reclaims(&mut harness, fighter, 2);
    assert!(server_match_result(&mut harness).is_some());
}

fn only_the_rebound_identity_reclaims(harness: &mut Harness, fighter: Entity, rejoin_id: u64) {
    let reply = rebind(harness, rejoin_id, PLAYER_ONE, 80, 81);
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
    reclaim(harness, fighter, 80, 81);
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

/// BRL-8y1f: Leave Match through the product path. The client tells the worker it is leaving and
/// only then disconnects; the worker releases the seat (one `SeatReleased` fact for the
/// supervisor and lobby), never holds it, removes the fighter under today's departure rules (a
/// lone 1v1 team forfeits) and refuses any later rebind of the seat.
#[test]
fn leaving_on_purpose_releases_the_seat_instead_of_holding_it() {
    let mut harness = active_routed_match();
    let fighter = harness.server_fighter(PLAYER_ONE.player_id).unwrap();
    {
        let mut lifecycle = harness.clients[0]
            .world_mut()
            .resource_mut::<brawler_client::RoutedClientLifecycle>();
        lifecycle.phase = brawler_client::RoutedClientPhase::Match;
        assert!(lifecycle.request_leave_match());
    }
    // The client returns to the lobby only after the worker answered its leave.
    let mut releases = Vec::new();
    harness.step_until(|harness| {
        releases.extend(take_seat_releases(harness.server.world_mut()));
        harness.clients[0]
            .world()
            .resource::<brawler_client::RoutedClientLifecycle>()
            .phase
            == brawler_client::RoutedClientPhase::AwaitingMatchUnlink
    });
    releases.extend(take_seat_releases(harness.server.world_mut()));
    assert_eq!(releases.len(), 1, "the left seat is released exactly once");
    assert_eq!(releases[0].player_id.get(), PLAYER_ONE.player_id);
    assert!(
        harness.server_fighter(PLAYER_ONE.player_id).is_some(),
        "the fighter stays until the session ends"
    );

    // The session ends; a pre-BRL-yyeg departure follows instead of a held seat.
    let client = harness.client_entities[0];
    harness.clients[0]
        .world_mut()
        .trigger(Disconnect { entity: client });
    for _ in 0..SESSION_LOSS_TICKS {
        harness.step();
        assert!(
            seat_hold_expiry(harness.server.world(), fighter).is_none(),
            "a seat its player left is never held"
        );
        if harness.server_fighter(PLAYER_ONE.player_id).is_none() {
            break;
        }
    }
    harness.suspend_client(0);
    assert_eq!(harness.server_fighter(PLAYER_ONE.player_id), None);
    for _ in 0..10 {
        harness.step();
    }
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
    let reply = rebind(&mut harness, 1, PLAYER_ONE, 80, 81);
    assert_eq!(reply.refusal, Some(RejoinReason::SeatExpired));
    // Completion releases the remaining connected seat (BRL-jxfy); the left one is not repeated.
    assert_eq!(
        released_players(&take_seat_releases(harness.server.world_mut())),
        vec![PLAYER_TWO.player_id]
    );
}

/// BRL-8y1f: a leave refuses rebinds even while the leaving session is still connected, so a
/// lobby rejoin request that races the release can never put the player back in.
#[test]
fn a_seat_its_player_left_refuses_a_racing_rebind() {
    let mut harness = active_routed_match();
    let match_id = {
        let world = harness.server.world_mut();
        let mut query = world.query_filtered::<&MatchState, With<MatchRootMarker>>();
        query.single(world).expect("one match").match_id
    };
    harness.send_match_command(
        0,
        MatchCommandRequest {
            request_id: 90,
            match_id,
            command: MatchCommand::LeaveMatch,
        },
    );
    harness.step_until(|harness| !take_seat_releases(harness.server.world_mut()).is_empty());
    let reply = rebind(&mut harness, 1, PLAYER_ONE, 80, 81);
    assert_eq!(reply.refusal, Some(RejoinReason::SeatExpired));
    // The other participant's seat is untouched.
    assert!(harness.server_fighter(PLAYER_TWO.player_id).is_some());
}

/// Leave the match through the product path (BRL-8y1f) and end that session, collecting the
/// seat releases the worker queues meanwhile. Returns once the leaver's fighter is removed.
fn leave_on_purpose(
    harness: &mut Harness,
    index: usize,
    player_id: u64,
) -> Vec<brawler_routing::SeatReleasedBody> {
    {
        let mut lifecycle = harness.clients[index]
            .world_mut()
            .resource_mut::<brawler_client::RoutedClientLifecycle>();
        lifecycle.phase = brawler_client::RoutedClientPhase::Match;
        assert!(lifecycle.request_leave_match());
    }
    let mut releases = Vec::new();
    harness.step_until(|harness| {
        releases.extend(take_seat_releases(harness.server.world_mut()));
        harness.clients[index]
            .world()
            .resource::<brawler_client::RoutedClientLifecycle>()
            .phase
            == brawler_client::RoutedClientPhase::AwaitingMatchUnlink
    });
    let client = harness.client_entities[index];
    harness.clients[index]
        .world_mut()
        .trigger(Disconnect { entity: client });
    for _ in 0..SESSION_LOSS_TICKS {
        harness.step();
        releases.extend(take_seat_releases(harness.server.world_mut()));
        if harness.server_fighter(player_id).is_none() {
            break;
        }
    }
    harness.suspend_client(index);
    assert_eq!(harness.server_fighter(player_id), None);
    releases
}

/// `Harness::step_until` with a caller-chosen bound, for conditions that wait on a session bound.
fn step_until_within(
    harness: &mut Harness,
    ticks: usize,
    mut condition: impl FnMut(&mut Harness) -> bool,
) {
    for _ in 0..ticks {
        harness.step();
        if condition(harness) {
            return;
        }
    }
    panic!("condition did not become true within {ticks} ticks");
}

fn released_players(releases: &[brawler_routing::SeatReleasedBody]) -> Vec<u64> {
    releases
        .iter()
        .map(|released| released.player_id.get())
        .collect()
}

const PLAYER_ONE_FORFEIT_VICTORY: brawler::matchplay::MatchResult =
    brawler::matchplay::MatchResult::Forfeit {
        winner: TeamId(0),
        departed_team: TeamId(1),
    };

fn server_match_result(harness: &mut Harness) -> Option<brawler::matchplay::MatchResult> {
    match harness.server_match_phase() {
        MatchPhase::Completed { result, .. } => Some(result),
        _ => None,
    }
}

/// BRL-jxfy: player one's session is lost and held; player two leaves, so the lone team
/// forfeits and the match completes while player one's seat is held. The worker defers its
/// Result, releases only the seats that cannot rejoin, and admits player one's rebound session in
/// Completed; that session receives the completed match (and so its Results), and the Result
/// goes out once it disconnects.
#[test]
fn a_match_that_completes_while_a_seat_is_held_defers_its_result_until_the_rejoin() {
    let mut harness = active_routed_match();
    let fighter = lose_session(&mut harness, 0, PLAYER_ONE.player_id);
    let mut releases = leave_on_purpose(&mut harness, 1, PLAYER_TWO.player_id);
    harness.step_until(|harness| server_match_result(harness).is_some());
    assert_eq!(
        server_match_result(&mut harness),
        Some(PLAYER_ONE_FORFEIT_VICTORY)
    );
    for _ in 0..120 {
        harness.step();
        releases.extend(take_seat_releases(harness.server.world_mut()));
        assert!(
            ready_match_result(harness.server.world()).is_none(),
            "no Result while a seat is held"
        );
    }
    assert!(seat_hold_expiry(harness.server.world(), fighter).is_some());
    assert_eq!(
        released_players(&releases),
        vec![PLAYER_TWO.player_id],
        "only the seat that cannot rejoin is released, once"
    );

    // The supervisor's rebind is accepted in Completed, and the rebound identity is admitted.
    let reply = rebind(&mut harness, 1, PLAYER_ONE, 80, 81);
    assert_eq!(reply.refusal, None);
    let index = reclaim(&mut harness, fighter, 80, 81);
    assert_eq!(harness.server_fighter(PLAYER_ONE.player_id), Some(fighter));
    assert_eq!(
        harness.admitted_player_id(index),
        Some(PLAYER_ONE.player_id)
    );
    // The rejoined client sees the completed match with its own fighter and team.
    harness.step_until(|harness| {
        let world = harness.clients[index].world_mut();
        let completed = world
            .query_filtered::<&MatchState, With<MatchRootMarker>>()
            .iter(world)
            .any(|state| {
                matches!(
                    state.phase,
                    MatchPhase::Completed { result, .. } if result == PLAYER_ONE_FORFEIT_VICTORY
                )
            });
        let own_team = world
            .query_filtered::<(&PlayerId, &TeamId), With<Fighter>>()
            .iter(world)
            .any(|(player, team)| player.0 == PLAYER_ONE.player_id && *team == TeamId(0));
        completed && own_team
    });
    assert_eq!(
        harness.controlled_player_ids(index),
        vec![PLAYER_ONE.player_id]
    );
    // The reclaimed seat is released at once, so the player's next lobby welcome is ordinary.
    let reclaimed_releases = take_seat_releases(harness.server.world_mut());
    assert_eq!(
        released_players(&reclaimed_releases),
        vec![PLAYER_ONE.player_id]
    );
    assert!(
        ready_match_result(harness.server.world()).is_none(),
        "the Result waits while the rejoined session is connected"
    );
    // Another rebind of the reclaimed seat is refused.
    let reply = rebind(&mut harness, 2, PLAYER_ONE, 82, 83);
    assert_eq!(reply.refusal, Some(RejoinReason::SeatExpired));

    // The client leaves for its Results; the Result follows and the match stays completed.
    let client = harness.client_entities[index];
    harness.clients[index]
        .world_mut()
        .trigger(Disconnect { entity: client });
    step_until_within(&mut harness, SESSION_LOSS_TICKS, |harness| {
        ready_match_result(harness.server.world()).is_some()
    });
    harness.suspend_client(index);
    assert_eq!(
        ready_match_result(harness.server.world()),
        Some(PLAYER_ONE_FORFEIT_VICTORY),
        "the Result describes the completed match"
    );
    assert!(take_seat_releases(harness.server.world_mut()).is_empty());
}

/// BRL-jxfy: without a rejoin, a held seat's expiry after completion releases the Result, and a
/// later rebind is refused.
#[test]
fn a_held_seat_that_expires_after_completion_releases_the_result() {
    let mut harness = active_routed_match();
    let fighter = lose_session(&mut harness, 0, PLAYER_ONE.player_id);
    let expiry = seat_hold_expiry(harness.server.world(), fighter).unwrap();
    let mut releases = leave_on_purpose(&mut harness, 1, PLAYER_TWO.player_id);
    harness.step_until(|harness| server_match_result(harness).is_some());
    assert!(ready_match_result(harness.server.world()).is_none());
    // The hold keeps counting in Completed: step the full remaining hold, never early.
    while harness.server_simulation_tick() + 1 < expiry {
        harness.step();
        releases.extend(take_seat_releases(harness.server.world_mut()));
        assert!(
            ready_match_result(harness.server.world()).is_none(),
            "the Result waits for the whole hold"
        );
    }
    harness.step_until(|harness| ready_match_result(harness.server.world()).is_some());
    assert!(harness.server_simulation_tick() >= expiry);
    assert_eq!(harness.server_fighter(PLAYER_ONE.player_id), None);
    releases.extend(take_seat_releases(harness.server.world_mut()));
    assert_eq!(
        released_players(&releases),
        vec![PLAYER_TWO.player_id, PLAYER_ONE.player_id],
        "the expired seat is released too"
    );
    let reply = rebind(&mut harness, 1, PLAYER_ONE, 80, 81);
    assert_eq!(reply.refusal, Some(RejoinReason::SeatExpired));
}

/// BRL-jxfy regression: with no held seat, the Result is ready on the completion frame, and every
/// seat is released then.
#[test]
fn a_match_with_no_held_seat_sends_its_result_at_completion() {
    let mut harness = active_routed_match();
    {
        let mut lifecycle = harness.clients[1]
            .world_mut()
            .resource_mut::<brawler_client::RoutedClientLifecycle>();
        lifecycle.phase = brawler_client::RoutedClientPhase::Match;
        assert!(lifecycle.request_leave_match());
    }
    let mut releases = Vec::new();
    harness.step_until(|harness| {
        releases.extend(take_seat_releases(harness.server.world_mut()));
        harness.clients[1]
            .world()
            .resource::<brawler_client::RoutedClientLifecycle>()
            .phase
            == brawler_client::RoutedClientPhase::AwaitingMatchUnlink
    });
    assert_eq!(released_players(&releases), vec![PLAYER_TWO.player_id]);
    let client = harness.client_entities[1];
    harness.clients[1]
        .world_mut()
        .trigger(Disconnect { entity: client });
    let mut completed = false;
    for _ in 0..SESSION_LOSS_TICKS {
        harness.step();
        releases.extend(take_seat_releases(harness.server.world_mut()));
        let result = server_match_result(&mut harness);
        assert_eq!(
            ready_match_result(harness.server.world()),
            result,
            "the Result is ready exactly from the completion frame"
        );
        if let Some(result) = result {
            assert_eq!(result, PLAYER_ONE_FORFEIT_VICTORY);
            completed = true;
            break;
        }
    }
    assert!(completed, "the lone team forfeits");
    harness.suspend_client(1);
    // Player one is connected and sees its Results now; its seat cannot be rejoined.
    assert_eq!(
        released_players(&releases),
        vec![PLAYER_TWO.player_id, PLAYER_ONE.player_id]
    );
    let reply = rebind(&mut harness, 1, PLAYER_ONE, 80, 81);
    assert_eq!(reply.refusal, Some(RejoinReason::SeatExpired));
}
