//! Network integration scenarios extracted from the shared harness.

use super::*;

#[test]
fn two_clients_connect_and_receive_the_same_server_owned_roster() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
    });
    harness.add_client(2);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.client_is_active(1)
            && harness.server_ids().len() == 2
            && harness.client_ids(0).len() == 2
            && harness.client_ids(1).len() == 2
            && harness.loadout_is_ready(0)
            && harness.loadout_is_ready(1)
    });

    let server_ids = harness.server_ids();
    assert_eq!(harness.client_ids(0), server_ids);
    assert_eq!(harness.client_ids(1), server_ids);
    assert_eq!(harness.active_server_sessions(), 2);

    let mut query = harness.server.world_mut().query_filtered::<(
        &lightyear::prelude::Replicate,
        &lightyear::prelude::ControlledBy,
    ), (With<PlaceholderPlayer>, Without<TestDummy>)>(
    );
    assert_eq!(query.iter(harness.server.world()).count(), 2);
}
