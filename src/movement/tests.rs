//! Focused movement, arena, and input validation tests.

use super::*;
use crate::protocol::QuantizedAxis2;
use core::time::Duration;

#[test]
fn radial_deadzone_remaps_and_clamps_diagonal_input() {
    assert_eq!(radial_deadzone(Vec2::splat(0.1), 0.2), Vec2::ZERO);
    let diagonal = radial_deadzone(Vec2::splat(1.0), 0.2);
    assert!(diagonal.length() <= 1.0 + f32::EPSILON);
    assert!((radial_deadzone(Vec2::new(0.6, 0.0), 0.2).x - 0.5).abs() < 1e-5);
}

#[test]
fn movement_deadzone_is_applied_once_by_authoritative_decode() {
    let decoded = decoded_move(
        QuantizedAxis2::from_vec2(Vec2::new(0.6, 0.0)),
        InputTuning::default(),
    );
    assert!((decoded.x - 0.5).abs() < 0.01);
}

#[test]
fn aim_threshold_preserves_last_valid_direction_by_returning_none() {
    let tuning = InputTuning::default();
    assert_eq!(committed_aim(Vec2::new(0.1, 0.0), tuning), None);
    assert_eq!(committed_aim(Vec2::new(1.0, 0.0), tuning), Some(Vec2::X));
}

#[test]
fn trigger_hysteresis_does_not_chatter_between_thresholds() {
    let tuning = InputTuning::default();
    assert!(trigger_pressed(false, 0.56, tuning));
    assert!(trigger_pressed(true, 0.50, tuning));
    assert!(!trigger_pressed(true, 0.44, tuning));
}

#[test]
fn known_fixed_input_moves_at_normalized_speed_and_keeps_facing_without_aim() {
    let tuning = MovementTuning::default();
    let input_tuning = InputTuning::default();
    let (position, facing, velocity) = desired_pose_step(
        Vec2::ZERO,
        0.7,
        FighterInput::from_axes(Vec2::splat(1.0), None, 0),
        tuning,
        input_tuning,
        Duration::from_secs_f32(1.0 / 60.0),
    );
    assert!((position.length() - tuning.speed / 60.0).abs() < 1e-4);
    assert!((facing - 0.7).abs() < f32::EPSILON);
    assert!((velocity.length() - tuning.speed).abs() < 1e-4);
}

#[test]
fn missing_input_neutralizes_after_twelve_ticks() {
    assert!(!input_should_neutralize(12, Some(0), 12));
    assert!(input_should_neutralize(13, Some(0), 12));
    assert!(input_should_neutralize(1, None, 12));
}

#[test]
fn absent_remote_input_does_not_refresh_freshness() {
    let input = FighterInput::from_axes(Vec2::X, None, 0);
    let mut buffer = NativeBuffer::<FighterInput>::default();
    buffer.set(lightyear::prelude::Tick(10), ActionState(input));
    buffer.last_remote_tick = Some(lightyear::prelude::Tick(10));
    assert_eq!(latest_present_remote_tick(&buffer), Some(10));

    buffer.set_empty(lightyear::prelude::Tick(11));
    buffer.last_remote_tick = Some(lightyear::prelude::Tick(11));
    assert_eq!(latest_present_remote_tick(&buffer), None);
}

#[test]
fn camera_and_spawn_bounds_are_stable() {
    let catalog = crate::map::MapContentCatalog::embedded().unwrap();
    let resolved = catalog
        .resolve_preset(
            crate::map::MapPresetId(1),
            crate::map::MapInstanceId(1),
            &crate::map::MapLayoutRequirements::wipeout(),
        )
        .unwrap();
    assert_eq!(
        resolved.snapshot.camera_bounds.min,
        Vec2::new(-896.0, -576.0)
    );
    assert_eq!(resolved.snapshot.camera_bounds.max, Vec2::new(896.0, 576.0));
    assert_eq!(
        resolved.spawn_points_by_team[&0][0].position,
        Vec2::new(-768.0, -288.0)
    );
    assert_eq!(
        resolved.spawn_points_by_team[&1][3].position,
        Vec2::new(768.0, 288.0)
    );
    assert_eq!(
        resolved
            .snapshot
            .playable_bounds
            .clamp_circle(Vec2::new(9_000.0, -9_000.0), 24.0),
        Vec2::new(872.0, -552.0)
    );
}

#[test]
fn pose_validation_uses_fighter_center_bounds() {
    let bounds = crate::map::PlayableBounds(crate::map::AxisAlignedMapRect {
        min: Vec2::new(-896.0, -576.0),
        max: Vec2::new(896.0, 576.0),
    });
    assert!(pose_is_valid(Vec2::new(872.0, 0.0), 0.0, bounds, 24.0));
    assert!(!pose_is_valid(Vec2::new(896.0, 0.0), 0.0, bounds, 24.0));
    assert!(!pose_is_valid(Vec2::new(0.0, -576.0), 0.0, bounds, 24.0));
}

#[cfg(feature = "server")]
#[test]
fn input_watermark_rejects_invalid_order_future_and_rate_excess() {
    let tuning = InputTuning::default();
    assert!(input_history_len_is_valid(1, tuning));
    assert!(!input_history_len_is_valid(0, tuning));
    assert!(!input_history_len_is_valid(
        tuning.max_history_ticks + 1,
        tuning
    ));
    assert!(input_end_tick_is_acceptable(
        100,
        100,
        Some(99),
        1.0,
        tuning
    ));
    assert!(!input_end_tick_is_acceptable(
        99,
        100,
        Some(99),
        1.0,
        tuning
    ));
    assert!(!input_end_tick_is_acceptable(
        117,
        100,
        Some(99),
        1.0,
        tuning
    ));
    assert!(!input_end_tick_is_acceptable(
        100,
        100,
        Some(99),
        0.5,
        tuning
    ));
    assert!(input_target_is_entity(
        lightyear::input::input_message::InputTarget::Entity(
            Entity::from_raw_u32(1).expect("valid test entity index"),
        )
    ));
    assert!(!input_target_is_entity(
        lightyear::input::input_message::InputTarget::PreSpawned(1)
    ));
}

#[cfg(feature = "server")]
#[test]
fn malformed_input_bits_and_axes_are_rejected_without_client_masking() {
    let mut malformed = FighterInput::from_axes(Vec2::X, None, 0);
    malformed.gameplay_buttons = 0x80;
    assert!(!decoded_input_is_valid(malformed));

    let mut too_fast = FighterInput::from_axes(Vec2::X, None, 0);
    too_fast.move_axis = QuantizedAxis2 {
        x: QuantizedAxis2::MAX,
        y: QuantizedAxis2::MAX,
    };
    assert!(!decoded_input_is_valid(too_fast));
}

#[cfg(feature = "server")]
#[test]
fn absent_end_state_cannot_refresh_input_validation() {
    let input = FighterInput::from_axes(Vec2::X, None, 0);
    assert!(input_sequence_ends_with_present_state(
        [
            Compressed::Input(ActionState(input)),
            Compressed::SameAsPrecedent,
        ]
        .into_iter(),
    ));
    assert!(!input_sequence_ends_with_present_state(
        [Compressed::Input(ActionState(input)), Compressed::Absent,].into_iter(),
    ));
    assert!(!input_sequence_ends_with_present_state(
        [Compressed::Absent, Compressed::SameAsPrecedent].into_iter(),
    ));
}
