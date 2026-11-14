//! BRL-0162: a phone between networks stops exchanging datagrams for a while. A gap shorter than
//! the session's Netcode keep-alive bound must keep the same authoritative session and fighter
//! and resume input; a longer gap is declared lost by both ends.

use super::*;

/// The harness client uses Lightyear's default connect-token timeout, which Netcode applies on
/// both ends (production routed sessions use `ROUTED_SESSION_TIMEOUT`, 10 s, the same way).
const HARNESS_SESSION_TIMEOUT: std::time::Duration = std::time::Duration::from_secs(3);

fn ticks(duration: std::time::Duration) -> usize {
    usize::try_from(duration.as_nanos() / SIMULATION_TICK.as_nanos()).expect("tick count fits")
}

fn active_single_client() -> Harness {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });
    harness
}

#[test]
fn a_two_way_outage_shorter_than_the_session_timeout_keeps_the_same_session_and_fighter() {
    let mut harness = active_single_client();
    let player = harness.controlled_player_id(0);
    let identities = harness.server_ids();
    let fighter = harness.controlled_entity(0);

    harness.set_link_blackout(0, true);
    for _ in 0..ticks(HARNESS_SESSION_TIMEOUT * 2 / 3) {
        harness.step();
    }
    assert_eq!(harness.active_server_sessions(), 1);
    assert!(harness.client_is_active(0));
    harness.set_link_blackout(0, false);

    let before = harness.server_positions()[0].1;
    harness.set_controlled_input(0, FighterInput::from_axes(Vec2::X, Some(Vec2::X), 0));
    harness.step_until(|harness| harness.server_positions()[0].1.0.x > before.0.x + 1.0);

    assert_eq!(harness.server_ids(), identities);
    assert_eq!(harness.controlled_player_id(0), player);
    assert_eq!(harness.controlled_entity(0), fighter);
    assert_eq!(harness.active_server_sessions(), 1);
    assert!(
        harness
            .server
            .world()
            .get::<Disconnected>(harness.server_links[0])
            .is_none()
    );
    assert!(
        harness.clients[0]
            .world()
            .get::<Disconnected>(harness.client_entities[0])
            .is_none()
    );
}

#[test]
fn an_outage_longer_than_the_session_timeout_is_declared_lost_by_both_ends() {
    let mut harness = active_single_client();
    harness.set_link_blackout(0, true);
    for _ in 0..ticks(HARNESS_SESSION_TIMEOUT + std::time::Duration::from_secs(1)) {
        harness.step();
    }
    // Netcode despawns the server-side link entity of a timed-out client.
    assert!(
        harness
            .server
            .world()
            .get_entity(harness.server_links[0])
            .is_err()
    );
    assert!(
        harness.clients[0]
            .world()
            .get::<Disconnected>(harness.client_entities[0])
            .is_some()
    );
}
