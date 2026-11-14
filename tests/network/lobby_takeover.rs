//! A fresh lobby login replacing the silent session of the same account (BRL-0208).

use super::queue::{selected_brawler, start_product_join, wait_for_product_lobby};
use super::*;

const SHARED_ACCOUNT: u128 = 77;

fn netcode_id(id: u64) -> brawler_routing::NetcodeClientId {
    brawler_routing::NetcodeClientId::new(id).unwrap()
}

fn lobby_session_count(harness: &Harness) -> usize {
    harness
        .server
        .world()
        .resource::<brawler::server::LobbyState>()
        .session_count()
}

fn has_lobby_session(harness: &Harness, client_id: u64) -> bool {
    harness
        .server
        .world()
        .resource::<brawler::server::LobbyState>()
        .session_for_client(client_id)
        .is_some()
}

fn newcomer_was_welcomed(harness: &Harness, index: usize) -> bool {
    harness.clients[index]
        .world()
        .get::<brawler_client::ClientLobbyMembership>(harness.client_entities[index])
        .is_some()
}

fn newcomer_requested_exit(harness: &Harness, index: usize) -> bool {
    !harness.clients[index]
        .world()
        .resource::<Messages<bevy::app::AppExit>>()
        .is_empty()
}

/// Steps the harness for `duration` of simulated time.
fn step_for(harness: &mut Harness, duration: std::time::Duration) {
    let mut elapsed = std::time::Duration::ZERO;
    while elapsed < duration {
        harness.step();
        elapsed += SIMULATION_TICK;
    }
}

#[test]
fn relaunch_with_the_same_account_replaces_a_killed_session_and_its_queue_ticket() {
    let mut harness = Harness::new_product_lobby(0);
    harness.add_product_lobby_client(1, Some(SHARED_ACCOUNT));
    wait_for_product_lobby(&mut harness);
    let (killed_brawler, _) = selected_brawler(&harness, 0);
    start_product_join(&mut harness, 0, 0);
    harness.step_until(|harness| {
        harness
            .server
            .world()
            .resource::<brawler::server::QueueState>()
            .ticket_for_client(netcode_id(1))
            .is_some()
    });

    // The app is killed: it stops sending without a disconnect, and relaunches at once with a
    // new Netcode identity and the same stored account.
    harness.suspend_client(0);
    harness.add_product_lobby_client(2, Some(SHARED_ACCOUNT));

    let mut steps = 0_u32;
    harness.step_until(|harness| {
        steps += 1;
        newcomer_was_welcomed(harness, 1)
    });
    let waited = SIMULATION_TICK * steps;
    assert!(
        waited >= std::time::Duration::from_millis(1_500),
        "the newcomer was admitted before the killed session had been silent ({waited:?})"
    );
    // The harness Netcode timeout is 3 s, so admission before 2.5 s proves a replacement rather
    // than the old session timing out.
    assert!(
        waited < std::time::Duration::from_millis(2_500),
        "the relaunch waited for the session timeout instead of replacing it ({waited:?})"
    );

    assert!(!has_lobby_session(&harness, 1));
    assert!(has_lobby_session(&harness, 2));
    assert_eq!(lobby_session_count(&harness), 1);
    let queue = harness
        .server
        .world()
        .resource::<brawler::server::QueueState>();
    assert!(queue.ticket_for_client(netcode_id(1)).is_none());
    assert_eq!(queue.ticket_count(), 0);
    let authority = harness
        .server
        .world()
        .resource::<brawler::profiles::ProfileAuthority>();
    assert!(authority.snapshot(1).is_none());
    assert_eq!(
        authority.account_holder(brawler::profiles::AccountId::new(SHARED_ACCOUNT).unwrap()),
        Some(2)
    );

    // The relaunch loads the same saved profile.
    harness.step_until(|harness| {
        harness.clients[1]
            .world()
            .resource::<brawler_client::ClientProfileModel>()
            .snapshot()
            .is_some()
    });
    assert_eq!(selected_brawler(&harness, 1).0, killed_brawler);
}

#[test]
fn a_live_session_keeps_its_account_and_the_duplicate_login_is_rejected() {
    let mut harness = Harness::new_product_lobby(0);
    harness.add_product_lobby_client(1, Some(SHARED_ACCOUNT));
    wait_for_product_lobby(&mut harness);

    // A second running copy with the same account, while the first keeps sending.
    harness.add_product_lobby_client(2, Some(SHARED_ACCOUNT));
    step_for(&mut harness, std::time::Duration::from_secs(1));
    assert!(!newcomer_was_welcomed(&harness, 1));
    assert!(!newcomer_requested_exit(&harness, 1));

    let mut steps = 0_u32;
    harness.step_until(|harness| {
        steps += 1;
        newcomer_requested_exit(harness, 1)
    });
    let waited = SIMULATION_TICK * steps + std::time::Duration::from_secs(1);
    assert!(
        waited >= std::time::Duration::from_secs(2),
        "the duplicate was rejected before the holder had a chance to be heard ({waited:?})"
    );
    assert!(!newcomer_was_welcomed(&harness, 1));
    assert!(has_lobby_session(&harness, 1));
    assert!(!has_lobby_session(&harness, 2));
    assert_eq!(
        harness
            .server
            .world()
            .resource::<brawler::profiles::ProfileAuthority>()
            .account_holder(brawler::profiles::AccountId::new(SHARED_ACCOUNT).unwrap()),
        Some(1)
    );

    // The first copy is unaffected and still served.
    step_for(&mut harness, std::time::Duration::from_secs(1));
    assert!(has_lobby_session(&harness, 1));
    assert!(newcomer_was_welcomed(&harness, 0));
}

#[test]
fn a_different_account_is_never_replaced() {
    let mut harness = Harness::new_product_lobby(0);
    harness.add_product_lobby_client(1, Some(SHARED_ACCOUNT));
    wait_for_product_lobby(&mut harness);
    harness.suspend_client(0);
    harness.add_product_lobby_client(2, Some(SHARED_ACCOUNT + 1));
    harness.step_until(|harness| newcomer_was_welcomed(harness, 1));
    // Past the replacement bound but before the harness's 3 s Netcode timeout.
    step_for(&mut harness, std::time::Duration::from_millis(2_500));
    assert!(has_lobby_session(&harness, 1));
    assert!(has_lobby_session(&harness, 2));
    assert_eq!(
        harness
            .server
            .world()
            .resource::<brawler::profiles::ProfileAuthority>()
            .account_holder(brawler::profiles::AccountId::new(SHARED_ACCOUNT).unwrap()),
        Some(1)
    );
}
