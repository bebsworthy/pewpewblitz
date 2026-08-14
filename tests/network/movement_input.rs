//! Network integration scenarios extracted from the shared harness.

use super::*;

#[test]
fn lost_input_repeats_briefly_then_neutralizes_without_server_pause() {
    let mut harness = Harness::new(1);
    harness.step_until(|harness| {
        harness.client_is_active(0)
            && harness.server_ids().len() == 1
            && harness.client_ids(0).len() == 1
            && harness.selection_is_complete(0)
    });

    harness.set_controlled_input(0, FighterInput::from_axes(Vec2::X, None, 0));
    for _ in 0..36 {
        harness.step();
    }
    let moving_position = harness.server_positions()[0].1.0;

    // Native input redundancy can leave a few already-received states in the
    // authoritative buffer; the server must drain those before neutralizing.
    for _ in 0..24 {
        harness.step_server_only();
    }
    let neutralized_position = harness.server_positions()[0].1.0;
    for _ in 0..4 {
        harness.step_server_only();
    }
    let settled_position = harness.server_positions()[0].1.0;

    assert!(
        neutralized_position.x > moving_position.x,
        "lost input did not advance before neutralization: moving={moving_position:?} neutralized={neutralized_position:?}"
    );
    assert!(
        settled_position.distance(neutralized_position) < 0.001,
        "server kept moving after neutralization: neutralized={neutralized_position:?} settled={settled_position:?}"
    );
}
