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
