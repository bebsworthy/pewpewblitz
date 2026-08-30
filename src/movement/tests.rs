//! Focused movement, arena, and input validation tests.

#[cfg(feature = "server")]
use super::authority::{
    MovementDecision, MovementModifiers, movement_decision, repaired_pose,
    resolved_movement_velocity,
};
#[cfg(feature = "server")]
use super::input::{
    decoded_input_is_valid, input_end_tick_is_acceptable, input_history_len_is_valid,
    input_sequence_ends_with_present_state, input_target_is_entity,
};
use super::*;
use crate::protocol::FighterInput;
use crate::protocol::QuantizedAxis2;
use core::time::Duration;
#[cfg(feature = "server")]
use lightyear::input::input_buffer::Compressed;
use lightyear::prelude::input::native::{ActionState, NativeBuffer};

#[test]
fn standard_fighter_has_clearance_inside_one_map_cell() {
    assert!((crate::builds::MAX_FIGHTER_BODY_RADIUS - 14.0).abs() < f32::EPSILON);
    assert!(
        (crate::map::MAP_CELL_SIZE_WORLD - crate::builds::MAX_FIGHTER_BODY_RADIUS * 2.0 - 4.0)
            .abs()
            < f32::EPSILON
    );
    assert_eq!(
        crate::builds::BuildCatalog::embedded()
            .unwrap()
            .fighter_body,
        crate::builds::FighterBody {
            radius: crate::builds::MAX_FIGHTER_BODY_RADIUS,
        }
    );
}

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
    let movement_speed = 100.0;
    let input_tuning = InputTuning::default();
    let (position, facing, velocity) = desired_pose_step(
        Vec2::ZERO,
        0.7,
        FighterInput::from_axes(Vec2::splat(1.0), None, 0),
        movement_speed,
        input_tuning,
        Duration::from_secs_f32(1.0 / 60.0),
    );
    assert!((position.length() - movement_speed / 60.0).abs() < 1e-4);
    assert!((facing - 0.7).abs() < f32::EPSILON);
    assert!((velocity.length() - movement_speed).abs() < 1e-4);
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
        .resolve_preset(crate::map::MapPresetId(1), crate::map::MapInstanceId(1))
        .unwrap();
    let bounds = resolved.snapshot.dimensions.bounds();
    assert_eq!(bounds.min, Vec2::new(-896.0, -576.0));
    assert_eq!(bounds.max, Vec2::new(896.0, 576.0));
    assert_eq!(
        resolved.spawn_points_by_team[&0][0].position,
        Vec2::new(-752.0, -272.0)
    );
    assert_eq!(
        resolved.spawn_points_by_team[&1][3].position,
        Vec2::new(752.0, 272.0)
    );
    assert_eq!(
        bounds.clamp_circle(
            Vec2::new(9_000.0, -9_000.0),
            crate::builds::MAX_FIGHTER_BODY_RADIUS
        ),
        Vec2::new(882.0, -562.0)
    );
}

#[test]
fn pose_validation_uses_fighter_center_bounds() {
    let bounds = crate::map::PlayableBounds(crate::map::AxisAlignedMapRect {
        min: Vec2::new(-896.0, -576.0),
        max: Vec2::new(896.0, 576.0),
    });
    assert!(pose_is_valid(
        Vec2::new(882.0, 0.0),
        0.0,
        bounds,
        crate::builds::MAX_FIGHTER_BODY_RADIUS
    ));
    assert!(!pose_is_valid(
        Vec2::new(896.0, 0.0),
        0.0,
        bounds,
        crate::builds::MAX_FIGHTER_BODY_RADIUS
    ));
    assert!(!pose_is_valid(
        Vec2::new(0.0, -576.0),
        0.0,
        bounds,
        crate::builds::MAX_FIGHTER_BODY_RADIUS
    ));
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

#[cfg(feature = "server")]
#[test]
fn movement_decision_combines_freshness_and_staleness() {
    let tuning = InputTuning::default();
    let input = FighterInput::from_axes(Vec2::new(0.6, 0.0), Some(Vec2::X), 0);
    let mut buffer = NativeBuffer::<FighterInput>::default();
    buffer.set(lightyear::prelude::Tick(10), ActionState(input));
    buffer.last_remote_tick = Some(lightyear::prelude::Tick(10));

    // A present buffer state refreshes the freshness watermark even from a stale component.
    let decision = movement_decision(
        20,
        &InputFreshness::default(),
        Some(&ActionState(input)),
        Some(&buffer),
        &tuning,
        12,
    );
    assert!(!decision.stale);
    assert_eq!(decision.freshness.last_fresh_tick, Some(10));
    assert!((decision.movement.x - 0.5).abs() < 1e-5);
    assert!(decision.aim.is_some());

    // Without any present input the stream goes stale and the axis neutralizes.
    let decision = movement_decision(
        100,
        &InputFreshness {
            last_fresh_tick: Some(10),
        },
        None,
        None,
        &tuning,
        12,
    );
    assert!(decision.stale);
    assert_eq!(decision.movement, Vec2::ZERO);
}

#[cfg(feature = "server")]
#[test]
fn resolved_velocity_applies_modifiers_and_external_motion() {
    let decision = MovementDecision {
        movement: Vec2::X,
        ..MovementDecision::default()
    };
    let velocity = resolved_movement_velocity(0, &decision, 320.0, MovementModifiers::default());
    assert!((velocity.x - 320.0).abs() < 1e-5);

    let slow = crate::combat::ActiveEffects {
        slow: Some(crate::combat::SlowEffect {
            source_attack_id: crate::combat::AttackId(1),
            source_network_entity_id: crate::protocol::NetworkEntityId(1),
            movement_multiplier_milli: 500,
            expires_at_tick: 10,
        }),
        ..default()
    };
    let velocity = resolved_movement_velocity(
        5,
        &decision,
        300.0,
        MovementModifiers {
            active_effects: Some(&slow),
            ..default()
        },
    );
    assert!((velocity.x - 150.0).abs() < 1e-5);

    // An expired slow no longer applies.
    let velocity = resolved_movement_velocity(
        20,
        &decision,
        300.0,
        MovementModifiers {
            active_effects: Some(&slow),
            ..default()
        },
    );
    assert!((velocity.x - 300.0).abs() < 1e-5);

    let external = crate::combat::ExternalMotion {
        velocity: Vec2::new(0.0, 40.0),
        expires_at_tick: 30,
    };
    let velocity = resolved_movement_velocity(
        5,
        &decision,
        300.0,
        MovementModifiers {
            external_motion: Some(&external),
            ..default()
        },
    );
    assert!((velocity.y - 40.0).abs() < 1e-5 && (velocity.x - 300.0).abs() < 1e-5);

    let speed_tile = crate::map::EffectTileOccupancy {
        generation: crate::map::MapDynamicGeneration {
            map_instance_id: crate::map::MapInstanceId(1),
            generation: 1,
        },
        placement_id: crate::map::MapPlacementId(1),
        behavior: crate::map::MapEffectTileBehavior::Speed {
            movement_multiplier_milli: 1_500,
        },
        entered_at_tick: 1,
        next_pulse_at_tick: None,
    };
    let velocity = resolved_movement_velocity(
        5,
        &decision,
        300.0,
        MovementModifiers {
            active_effects: Some(&slow),
            effect_tile: Some(&speed_tile),
            external_motion: Some(&external),
            ..default()
        },
    );
    assert!((velocity.x - 225.0).abs() < 1e-5);
    assert!(
        (velocity.y - 40.0).abs() < 1e-5,
        "tile speed must not scale external motion"
    );
}

#[cfg(feature = "server")]
#[test]
fn resolved_velocity_uses_authored_adrenal_movement_bonus() {
    let decision = MovementDecision {
        movement: Vec2::X,
        ..MovementDecision::default()
    };
    let builds = crate::builds::BuildCatalog::embedded().unwrap();
    let weapons = crate::combat::WeaponCatalog::embedded().unwrap();
    let mut loadout = crate::builds::resolve_build_recipe(
        &builds,
        &weapons,
        crate::builds::BrawlerBuildRecipe {
            weapon: crate::builds::WeaponChoice::Preset(crate::combat::WeaponPresetId(1)),
            ultimate: crate::builds::UltimateDefinitionId(1),
            passives: [
                crate::builds::PassiveDefinitionId(3),
                crate::builds::PassiveDefinitionId(4),
            ],
        },
    )
    .unwrap();
    loadout.passives[0].parameters = crate::builds::PassiveParameters::AdrenalResponse {
        duration_ticks: 90,
        rearm_ticks: 240,
        movement_bonus_basis_points: 2_500,
    };
    let passive_state = crate::builds::PassiveRuntimeState {
        adrenaline_until_tick: Some(10),
        ..Default::default()
    };
    let velocity = resolved_movement_velocity(
        5,
        &decision,
        300.0,
        MovementModifiers {
            passive_loadout: Some(&loadout),
            passive_state: Some(&passive_state),
            ..default()
        },
    );
    assert!((velocity.x - 375.0).abs() < 1e-5);
}

#[cfg(feature = "server")]
#[test]
fn repaired_pose_clamps_finite_positions_and_resets_non_finite_facing() {
    let bounds = crate::map::PlayableBounds(crate::map::AxisAlignedMapRect {
        min: Vec2::new(-500.0, -300.0),
        max: Vec2::new(500.0, 300.0),
    });
    let (position, facing) = repaired_pose(
        Vec2::new(600.0, 0.0),
        1.0,
        &bounds,
        crate::builds::MAX_FIGHTER_BODY_RADIUS,
        0.5,
    );
    assert_eq!(position, Vec2::new(486.0, 0.0));
    assert!((facing - 1.0).abs() < f32::EPSILON);

    let (position, facing) = repaired_pose(
        Vec2::INFINITY,
        f32::NAN,
        &bounds,
        crate::builds::MAX_FIGHTER_BODY_RADIUS,
        0.5,
    );
    assert_eq!(position, Vec2::ZERO);
    assert!((facing - 0.5).abs() < f32::EPSILON);
}
