//! Focused reconciliation rules and owner-prediction schedule ordering.

use super::*;

const DT: f32 = 1.0 / 60.0;

fn sample(tick: u32, position: Vec2) -> AuthoritativeSample {
    AuthoritativeSample {
        tick: Tick(tick),
        position,
        facing: 0.0,
    }
}

fn predicted_walk(start: Vec2, from_tick: u32, steps: u32, step: Vec2) -> OwnerPrediction {
    let tuning = OwnerPredictionTuning::default();
    let mut prediction = OwnerPrediction::from_pose(start, 0.0);
    for offset in 1..=steps {
        prediction.record_step(
            Tick(from_tick + offset),
            start + step * f32::from(u16::try_from(offset).expect("small tick offset")),
            None,
            tuning,
            DT,
        );
    }
    prediction
}

#[test]
fn matching_authority_keeps_the_prediction_and_presented_pose() {
    let tuning = OwnerPredictionTuning::default();
    let mut prediction = predicted_walk(Vec2::ZERO, 100, 5, Vec2::X);
    let outcome = prediction.reconcile(sample(102, Vec2::new(2.0, 0.0)), tuning);
    assert_eq!(outcome, Reconciliation::Smoothed { error: 0.0 });
    assert_eq!(prediction.position, Vec2::new(5.0, 0.0));
    assert_eq!(prediction.presented_position(), Vec2::new(5.0, 0.0));
    assert_eq!(
        prediction.history.len(),
        3,
        "acknowledged steps are dropped"
    );
}

#[test]
fn small_mispredictions_shift_in_flight_steps_and_blend_the_presented_pose() {
    let tuning = OwnerPredictionTuning::default();
    let mut prediction = predicted_walk(Vec2::ZERO, 100, 5, Vec2::X);
    // The server stopped the fighter one unit short (for example at a destroyed-cover edge).
    let outcome = prediction.reconcile(sample(102, Vec2::new(1.0, 0.0)), tuning);
    assert_eq!(outcome, Reconciliation::Smoothed { error: 1.0 });
    assert_eq!(prediction.position, Vec2::new(4.0, 0.0));
    assert_eq!(
        prediction.presented_position(),
        Vec2::new(5.0, 0.0),
        "no visible jump"
    );
    assert!(
        prediction.history.iter().all(|step| (step.position.x
            - f32::from(u16::try_from(step.tick.0 - 101).expect("small")))
        .abs()
            < 1e-5),
        "every unacknowledged step moved by the same error"
    );
    // The presented offset converges without a jump.
    let mut previous = prediction.presented_position();
    for tick in 106..140 {
        prediction.record_step(Tick(tick), prediction.position, None, tuning, DT);
        let presented = prediction.presented_position();
        assert!((presented - previous).length() <= 1.0);
        previous = presented;
    }
    assert_eq!(prediction.presented_position(), prediction.position);
}

#[test]
fn corrections_at_the_snap_threshold_jump_immediately() {
    let tuning = OwnerPredictionTuning::default();
    let mut prediction = predicted_walk(Vec2::ZERO, 100, 5, Vec2::X);
    let respawn = Vec2::new(400.0, -300.0);
    let outcome = prediction.reconcile(sample(103, respawn), tuning);
    assert!(matches!(outcome, Reconciliation::Snapped { .. }));
    assert_eq!(prediction.correction_offset, Vec2::ZERO);
    assert_eq!(
        prediction.presented_position(),
        respawn + Vec2::new(2.0, 0.0)
    );
}

#[test]
fn samples_without_a_matching_prediction_rebase_onto_authority() {
    let tuning = OwnerPredictionTuning::default();
    let mut prediction = predicted_walk(Vec2::ZERO, 100, 3, Vec2::X);
    // A timeline shift: the confirmed tick is newer than every local prediction.
    let outcome = prediction.reconcile(sample(140, Vec2::new(4.0, 0.0)), tuning);
    assert_eq!(outcome, Reconciliation::Rebased { error: 1.0 });
    assert!(prediction.history.is_empty());
    assert_eq!(prediction.position, Vec2::new(4.0, 0.0));
    assert_eq!(prediction.presented_position(), Vec2::new(3.0, 0.0));
}

#[test]
fn samples_older_than_the_prediction_wait_for_their_round_trip() {
    let tuning = OwnerPredictionTuning::default();
    let mut prediction = predicted_walk(Vec2::ZERO, 100, 3, Vec2::X);
    let outcome = prediction.reconcile(sample(90, Vec2::new(-5.0, 0.0)), tuning);
    assert_eq!(outcome, Reconciliation::Pending);
    assert_eq!(prediction.history.len(), 3);
    assert_eq!(prediction.position, Vec2::new(3.0, 0.0));
}

#[test]
fn suspended_movement_presents_authoritative_motion_without_smoothing() {
    let tuning = OwnerPredictionTuning::default();
    let mut prediction = predicted_walk(Vec2::ZERO, 100, 3, Vec2::ZERO);
    prediction.following_authority = true;
    // A dash step the client never predicted.
    let outcome = prediction.reconcile(sample(101, Vec2::new(20.0, 0.0)), tuning);
    assert_eq!(outcome, Reconciliation::Followed { error: 20.0 });
    assert_eq!(prediction.correction_offset, Vec2::ZERO);
    assert_eq!(prediction.presented_position(), Vec2::new(20.0, 0.0));
}

#[test]
fn stale_and_duplicate_samples_are_ignored_and_deadlines_follow_the_server_clock() {
    let tuning = OwnerPredictionTuning::default();
    let mut prediction = predicted_walk(Vec2::ZERO, 100, 5, Vec2::X);
    assert_eq!(prediction.simulation_tick_at(Tick(105)), None);
    assert!(prediction.accepts(Tick(102)));
    prediction.reconcile(sample(102, Vec2::new(2.0, 0.0)), tuning);
    assert!(!prediction.accepts(Tick(102)));
    assert!(!prediction.accepts(Tick(101)));
    assert!(prediction.accepts(Tick(103)));
    prediction.observe_server_clock(Tick(102), 900);
    assert_eq!(prediction.simulation_tick_at(Tick(105)), Some(903));
    assert_eq!(prediction.simulation_tick_at(Tick(100)), Some(898));
}

#[test]
fn facing_follows_local_aim_until_authority_acknowledges_it() {
    let tuning = OwnerPredictionTuning::default();
    let mut prediction = OwnerPrediction::from_pose(Vec2::ZERO, 0.0);
    prediction.record_step(Tick(1), Vec2::ZERO, Some(Vec2::Y), tuning, DT);
    prediction.record_step(Tick(2), Vec2::ZERO, None, tuning, DT);
    let up = core::f32::consts::FRAC_PI_2;
    assert!((prediction.facing - up).abs() < 1e-5);
    // Authority for tick 0 predates the aim; the local facing stays.
    prediction.reconcile(
        AuthoritativeSample {
            facing: 0.0,
            ..sample(0, Vec2::ZERO)
        },
        tuning,
    );
    assert!((prediction.facing - up).abs() < 1e-5);
    // Once the aimed tick is acknowledged, authority owns the facing again.
    prediction.reconcile(
        AuthoritativeSample {
            facing: up,
            ..sample(1, Vec2::ZERO)
        },
        tuning,
    );
    assert!((prediction.facing - up).abs() < 1e-5);
}

#[test]
fn history_stays_bounded() {
    let tuning = OwnerPredictionTuning::default();
    let prediction = {
        let mut prediction = OwnerPrediction::from_pose(Vec2::ZERO, 0.0);
        for tick in 0..(u32::try_from(OWNER_PREDICTION_HISTORY).expect("small") * 3) {
            prediction.record_step(Tick(tick), Vec2::ZERO, None, tuning, DT);
        }
        prediction
    };
    assert_eq!(prediction.history.len(), OWNER_PREDICTION_HISTORY);
}

fn plugin_app() -> App {
    let mut app = App::new();
    app.add_plugins(crate::gameplay::GameplayPlugin)
        .init_resource::<LocalTimeline>()
        .init_resource::<ReplicationCheckpointMap>()
        .insert_resource(crate::builds::BuildCatalogResource(
            crate::builds::BuildCatalog::embedded().expect("embedded build catalog"),
        ))
        .insert_resource(MapCatalogResource(
            crate::map::MapContentCatalog::embedded().expect("embedded map catalog"),
        ))
        .add_plugins(OwnerPredictionPlugin);
    app
}

fn loadout() -> ResolvedMatchLoadout {
    let builds = crate::builds::BuildCatalog::embedded().expect("embedded build catalog");
    let weapons = crate::combat::WeaponCatalog::embedded().expect("embedded weapon catalog");
    crate::builds::resolve_build_recipe(
        &builds,
        &weapons,
        crate::builds::BrawlerBuildRecipe {
            weapon: crate::builds::WeaponChoice::Preset(crate::combat::WeaponPresetId(1)),
            ultimate: crate::builds::UltimateDefinitionId(1),
            passives: [
                crate::builds::PassiveDefinitionId(1),
                crate::builds::PassiveDefinitionId(6),
            ],
        },
    )
    .expect("preset loadout resolves")
}

fn fixed_tick(app: &mut App) {
    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .advance_by(crate::timing::SIMULATION_TICK);
    app.world_mut()
        .resource_mut::<LocalTimeline>()
        .apply_delta(1);
    app.world_mut().run_schedule(FixedUpdate);
}

/// Stand-in for Lightyear interpolation, which writes every replicated pose in Update.
fn overwrite_with_interpolated_pose(mut poses: Query<&mut Position>) {
    for mut position in &mut poses {
        position.0 = Vec2::splat(-999.0);
    }
}

#[test]
fn plugin_attaches_predicts_on_the_input_tick_and_presents_after_interpolation() {
    let mut app = plugin_app();
    app.add_systems(
        Update,
        overwrite_with_interpolated_pose.in_set(InterpolationSystems::Interpolate),
    );
    let loadout = loadout();
    let speed = loadout.fighter_stats.movement_speed;
    let owner = app
        .world_mut()
        .spawn((
            Fighter,
            Controlled,
            AuthoritativePose {
                position: crate::combat::WorldPoint::from(Vec2::new(10.0, 20.0)),
                facing: 0.0,
                tick: 50,
            },
            ActionState(FighterInput::from_axes(Vec2::X, None, 0)),
            loadout,
            Position(Vec2::new(10.0, 20.0)),
            Rotation::default(),
        ))
        .id();

    // Attachment is applied before prediction within the same fixed tick.
    fixed_tick(&mut app);
    let prediction = app.world().get::<OwnerPrediction>(owner).expect("attached");
    assert_eq!(prediction.history.len(), 1);
    assert_eq!(
        prediction.position,
        Vec2::new(10.0, 20.0),
        "no deadline clock yet, so no movement is guessed"
    );

    // Once a server clock is known, the input moves the prediction on that same tick.
    app.world_mut()
        .get_mut::<OwnerPrediction>(owner)
        .expect("attached")
        .server_clock = Some((Tick(1), 50));
    fixed_tick(&mut app);
    let prediction = app.world().get::<OwnerPrediction>(owner).expect("attached");
    let expected = Vec2::new(10.0 + speed / 60.0, 20.0);
    assert!((prediction.position - expected).length() < 1e-3);

    // The presented pose replaces the interpolated pose before PostUpdate presentation.
    app.world_mut().run_schedule(Update);
    let position = app.world().get::<Position>(owner).expect("pose");
    assert!((position.0 - expected).length() < 1e-3);
}

#[test]
fn suspended_and_frozen_owners_do_not_integrate_input() {
    let mut app = plugin_app();
    let base = (
        Fighter,
        Controlled,
        AuthoritativePose {
            position: crate::combat::WorldPoint::from(Vec2::ZERO),
            facing: 0.0,
            tick: 50,
        },
        ActionState(FighterInput::from_axes(Vec2::X, None, 0)),
        loadout(),
        Position::default(),
        Rotation::default(),
    );
    let dashing = app
        .world_mut()
        .spawn((
            base.clone(),
            AbilityState {
                phase: crate::builds::AbilityPhase::Dashing { ends_at_tick: 500 },
                ..Default::default()
            },
        ))
        .id();
    let frozen = app
        .world_mut()
        .spawn((
            base.clone(),
            ActiveEffects {
                cold: crate::combat::ColdState {
                    frozen_until_tick: Some(500),
                    ..Default::default()
                },
                ..Default::default()
            },
        ))
        .id();
    let defeated = app
        .world_mut()
        .spawn((
            base,
            Defeated {
                event_id: crate::combat::CombatEventId(1),
            },
        ))
        .id();
    fixed_tick(&mut app);
    for entity in [dashing, frozen, defeated] {
        app.world_mut()
            .get_mut::<OwnerPrediction>(entity)
            .expect("attached")
            .server_clock = Some((Tick(1), 50));
    }
    for _ in 0..10 {
        fixed_tick(&mut app);
    }
    for entity in [dashing, frozen, defeated] {
        let prediction = app
            .world()
            .get::<OwnerPrediction>(entity)
            .expect("attached");
        assert_eq!(
            prediction.position,
            Vec2::ZERO,
            "{entity:?} must follow authority"
        );
    }
}

#[test]
fn presented_pose_extrapolates_the_newest_step_and_is_exact_at_tick_boundaries() {
    let tuning = OwnerPredictionTuning::default();
    let prediction = predicted_walk(Vec2::ZERO, 100, 5, Vec2::X);
    assert_eq!(prediction.step_velocity(), Vec2::X / DT);
    assert_eq!(
        prediction.presented_position_at(0.0, DT, tuning),
        prediction.presented_position(),
        "a tick boundary presents the committed prediction exactly"
    );
    let half = prediction.presented_position_at(DT / 2.0, DT, tuning);
    assert!((half - Vec2::new(5.5, 0.0)).length() < 1e-4);
    let full = prediction.presented_position_at(DT, DT, tuning);
    assert!(
        (full - Vec2::new(6.0, 0.0)).length() < 1e-4,
        "a full overstep meets the next step"
    );
    assert_eq!(
        prediction.presented_position_at(3.0 * DT, DT, tuning),
        full,
        "a stalled fixed loop never extrapolates beyond one tick of travel"
    );
}

#[test]
fn presented_correction_decays_continuously_across_a_tick() {
    let tuning = OwnerPredictionTuning::default();
    let mut prediction = OwnerPrediction::from_pose(Vec2::ZERO, 0.0);
    prediction.record_step(Tick(1), Vec2::ZERO, None, tuning, DT);
    prediction.correction_offset = Vec2::new(4.0, 0.0);
    let before_tick = prediction.presented_position_at(DT, DT, tuning);
    prediction.record_step(Tick(2), Vec2::ZERO, None, tuning, DT);
    let after_tick = prediction.presented_position_at(0.0, DT, tuning);
    assert!((before_tick - after_tick).length() < 1e-4);
    assert!(before_tick.x < 4.0 && before_tick.x > 0.0);
}

#[test]
fn following_authority_presents_without_extrapolation() {
    let tuning = OwnerPredictionTuning::default();
    let mut prediction = OwnerPrediction::from_pose(Vec2::ZERO, 0.0);
    prediction.following_authority = true;
    prediction.record_step(Tick(1), Vec2::new(20.0, 0.0), None, tuning, DT);
    assert_eq!(prediction.step_velocity(), Vec2::ZERO);
    assert_eq!(
        prediction.presented_position_at(DT / 2.0, DT, tuning),
        Vec2::new(20.0, 0.0)
    );
}

/// One rendered frame of `delta`: Bevy's fixed main loop followed by Update.
fn render_frame(app: &mut App, delta: core::time::Duration) -> usize {
    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .accumulate_overstep(delta);
    let mut ticks = 0;
    while app.world().resource::<Time<Fixed>>().overstep() >= crate::timing::SIMULATION_TICK {
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .discard_overstep(crate::timing::SIMULATION_TICK);
        fixed_tick(app);
        ticks += 1;
    }
    app.world_mut().run_schedule(Update);
    ticks
}

fn walking_owner_app() -> (App, Entity, f32) {
    let mut app = plugin_app();
    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .set_timestep(crate::timing::SIMULATION_TICK);
    app.add_systems(
        Update,
        overwrite_with_interpolated_pose.in_set(InterpolationSystems::Interpolate),
    );
    let loadout = loadout();
    let speed = loadout.fighter_stats.movement_speed;
    let owner = app
        .world_mut()
        .spawn((
            Fighter,
            Controlled,
            AuthoritativePose {
                position: crate::combat::WorldPoint::from(Vec2::ZERO),
                facing: 0.0,
                tick: 50,
            },
            ActionState(FighterInput::from_axes(Vec2::X, None, 0)),
            loadout,
            Position(Vec2::ZERO),
            Rotation::default(),
        ))
        .id();
    fixed_tick(&mut app);
    app.world_mut()
        .get_mut::<OwnerPrediction>(owner)
        .expect("attached")
        .server_clock = Some((Tick(1), 50));
    (app, owner, speed)
}

fn presented_x(app: &App, owner: Entity) -> f32 {
    app.world().get::<Position>(owner).expect("pose").0.x
}

/// Two 120 Hz frames that land exactly on a tick, then 144, 90, 240, and 90 Hz frames; the
/// cycle spans exactly three fixed ticks, so every cycle starts on a tick boundary.
const VARYING_FRAMES_NANOS: [u64; 6] = [
    8_333_333, 8_333_333, 6_944_444, 11_111_111, 4_166_666, 11_111_111,
];

#[test]
fn varying_frame_deltas_present_continuous_owner_motion_between_ticks() {
    let (mut app, owner, speed) = walking_owner_app();
    // The first moving tick: the render clock now sits exactly on its boundary.
    fixed_tick(&mut app);
    let start = app
        .world()
        .get::<OwnerPrediction>(owner)
        .expect("attached")
        .position
        .x;
    let mut elapsed = 0.0_f64;
    let mut previous = presented_x(&app, owner);
    let mut saw_tickless_frame = false;
    let mut saw_exact_boundary = false;
    for frame in 0..120 {
        let delta = core::time::Duration::from_nanos(
            VARYING_FRAMES_NANOS[frame % VARYING_FRAMES_NANOS.len()],
        );
        elapsed += delta.as_secs_f64();
        let ticks = render_frame(&mut app, delta);
        saw_tickless_frame |= ticks == 0;
        let presented = presented_x(&app, owner);
        assert!(
            presented > previous,
            "frame {frame}: the owner advances on every rendered frame ({previous} -> {presented})"
        );
        // A steady walk extrapolates exactly onto the continuous path through every tick.
        #[allow(
            clippy::cast_possible_truncation,
            reason = "a two-second test walk stays far inside f32 range"
        )]
        let continuous = start + speed * elapsed as f32;
        assert!(
            (presented - continuous).abs() < 1e-2,
            "frame {frame}: presented {presented} vs continuous {continuous}"
        );
        let overstep = app.world().resource::<Time<Fixed>>().overstep();
        if overstep.is_zero() {
            saw_exact_boundary = true;
            let prediction = app.world().get::<OwnerPrediction>(owner).expect("attached");
            assert!(
                (presented - prediction.presented_position().x).abs() <= f32::EPSILON,
                "frame {frame}: a tick boundary presents the committed prediction exactly"
            );
        }
        previous = presented;
    }
    assert!(saw_tickless_frame && saw_exact_boundary);
}

#[test]
fn releasing_input_withdraws_at_most_one_partial_step_and_corrections_do_not_snap() {
    let (mut app, owner, speed) = walking_owner_app();
    let frame = core::time::Duration::from_nanos(6_944_444);
    let frame_seconds = frame.as_secs_f32();
    for _ in 0..30 {
        render_frame(&mut app, frame);
    }
    // A small authoritative correction for an in-flight tick blends instead of jumping.
    let tick = {
        let prediction = app.world().get::<OwnerPrediction>(owner).expect("attached");
        prediction.history[prediction.history.len() - 3].tick
    };
    let predicted_then = app
        .world()
        .get::<OwnerPrediction>(owner)
        .expect("attached")
        .history[app
        .world()
        .get::<OwnerPrediction>(owner)
        .expect("attached")
        .history
        .len()
        - 3]
    .position;
    let tuning = *app.world().resource::<OwnerPredictionTuning>();
    let before = presented_x(&app, owner);
    let outcome = app
        .world_mut()
        .get_mut::<OwnerPrediction>(owner)
        .expect("attached")
        .reconcile(sample(tick.0, predicted_then - Vec2::new(3.0, 0.0)), tuning);
    assert_eq!(outcome, Reconciliation::Smoothed { error: 3.0 });
    let mut previous = before;
    for _ in 0..120 {
        render_frame(&mut app, frame);
        let presented = presented_x(&app, owner);
        let step = presented - previous;
        assert!(
            (-1e-3..=speed * frame_seconds + 0.6).contains(&step),
            "the correction blends below one frame of travel plus its decay ({step})"
        );
        previous = presented;
    }
    // Releasing the input withdraws only the extrapolated partial step, then holds still.
    app.world_mut()
        .get_mut::<ActionState<FighterInput>>(owner)
        .expect("input")
        .0 = FighterInput::default();
    let mut lowest = previous;
    for _ in 0..30 {
        render_frame(&mut app, frame);
        lowest = lowest.min(presented_x(&app, owner));
    }
    let settled = presented_x(&app, owner);
    assert!(
        previous - lowest <= speed / 60.0 + 1e-3,
        "withdrawal {} stays within one tick of travel",
        previous - lowest
    );
    let prediction = app.world().get::<OwnerPrediction>(owner).expect("attached");
    assert!((settled - prediction.presented_position().x).abs() < 1e-3);
}
