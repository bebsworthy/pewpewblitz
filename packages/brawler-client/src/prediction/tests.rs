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
fn the_first_sample_replays_the_inputs_the_server_already_applied() {
    let tuning = OwnerPredictionTuning::default();
    // Attached from the first authoritative pose, which arrives a round trip after the server
    // started applying the walking input.
    let mut prediction = OwnerPrediction::from_pose(Vec2::new(1.0, 0.0), 0.0);
    let mut replayed = Vec::new();
    let outcome = prediction.reconcile_replaying(
        sample(101, Vec2::new(1.0, 0.0)),
        tuning,
        Tick(105),
        |tick, position| {
            replayed.push(tick.0);
            ReplayedStep {
                position: position + Vec2::X,
                aim: None,
                suspended: false,
            }
        },
    );
    assert_eq!(
        replayed,
        [102, 103, 104],
        "every input after the sample, before now"
    );
    assert_eq!(outcome, Reconciliation::Rebased { error: 0.0 });
    assert_eq!(prediction.position, Vec2::new(4.0, 0.0));
    assert_eq!(
        prediction.presented_position(),
        prediction.position,
        "replayed motion is prediction, not a correction"
    );
    // The next sample for a replayed tick matches without a correction.
    let outcome = prediction.reconcile_replaying(
        sample(102, Vec2::new(2.0, 0.0)),
        tuning,
        Tick(106),
        |_, _| unreachable!("a matched sample replays nothing"),
    );
    assert_eq!(outcome, Reconciliation::Smoothed { error: 0.0 });
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
    app.add_plugins(brawler::gameplay::GameplayPlugin)
        .init_resource::<LocalTimeline>()
        .init_resource::<ReplicationCheckpointMap>()
        .insert_resource(brawler::builds::BuildCatalogResource(
            brawler::builds::BuildCatalog::embedded().expect("embedded build catalog"),
        ))
        .insert_resource(MapCatalogResource(
            brawler::map::MapContentCatalog::embedded().expect("embedded map catalog"),
        ))
        .add_plugins(OwnerPredictionPlugin);
    app
}

fn loadout() -> ResolvedMatchLoadout {
    let builds = brawler::builds::BuildCatalog::embedded().expect("embedded build catalog");
    let weapons = brawler::combat::WeaponCatalog::embedded().expect("embedded weapon catalog");
    brawler::builds::resolve_loadout(
        &builds,
        &weapons,
        brawler::builds::LoadoutRecipe::test_default(
            brawler::combat::WeaponPresetId(1),
            brawler::builds::UltimateDefinitionId(1),
            brawler::builds::PassiveDefinitionId(6),
        ),
    )
    .expect("preset loadout resolves")
}

fn fixed_tick(app: &mut App) {
    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .advance_by(brawler::timing::SIMULATION_TICK);
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
                position: brawler::combat::WorldPoint::from(Vec2::new(10.0, 20.0)),
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
            position: brawler::combat::WorldPoint::from(Vec2::ZERO),
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
                phase: AbilityPhase::Dashing { ends_at_tick: 500 },
                ..Default::default()
            },
        ))
        .id();
    let frozen = app
        .world_mut()
        .spawn((
            base.clone(),
            ActiveEffects {
                cold: brawler::combat::ColdState {
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
                event_id: brawler::combat::CombatEventId(1),
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
        prediction.presented_position_at(0.0, DT, tuning, core::convert::identity),
        prediction.presented_position(),
        "a tick boundary presents the committed prediction exactly"
    );
    let half = prediction.presented_position_at(DT / 2.0, DT, tuning, core::convert::identity);
    assert!((half - Vec2::new(5.5, 0.0)).length() < 1e-4);
    let full = prediction.presented_position_at(DT, DT, tuning, core::convert::identity);
    assert!(
        (full - Vec2::new(6.0, 0.0)).length() < 1e-4,
        "a full overstep meets the next step"
    );
    assert_eq!(
        prediction.presented_position_at(3.0 * DT, DT, tuning, core::convert::identity),
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
    let before_tick = prediction.presented_position_at(DT, DT, tuning, core::convert::identity);
    prediction.record_step(Tick(2), Vec2::ZERO, None, tuning, DT);
    let after_tick = prediction.presented_position_at(0.0, DT, tuning, core::convert::identity);
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
        prediction.presented_position_at(DT / 2.0, DT, tuning, core::convert::identity),
        Vec2::new(20.0, 0.0)
    );
}

/// One rendered frame of `delta`: Bevy's fixed main loop followed by Update.
fn render_frame(app: &mut App, delta: core::time::Duration) -> usize {
    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .accumulate_overstep(delta);
    let mut ticks = 0;
    while app.world().resource::<Time<Fixed>>().overstep() >= brawler::timing::SIMULATION_TICK {
        app.world_mut()
            .resource_mut::<Time<Fixed>>()
            .discard_overstep(brawler::timing::SIMULATION_TICK);
        fixed_tick(app);
        ticks += 1;
    }
    app.world_mut().run_schedule(Update);
    ticks
}

fn walking_owner_app() -> (App, Entity, f32) {
    walking_owner_app_from(Vec2::ZERO, Vec2::X)
}

fn walking_owner_app_from(start: Vec2, axis: Vec2) -> (App, Entity, f32) {
    let mut app = plugin_app();
    app.world_mut()
        .resource_mut::<Time<Fixed>>()
        .set_timestep(brawler::timing::SIMULATION_TICK);
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
                position: brawler::combat::WorldPoint::from(start),
                facing: 0.0,
                tick: 50,
            },
            ActionState(FighterInput::from_axes(axis, None, 0)),
            loadout,
            Position(start),
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

#[test]
fn presented_extrapolation_is_resolved_by_the_body_resolver() {
    let tuning = OwnerPredictionTuning::default();
    let prediction = predicted_walk(Vec2::ZERO, 100, 5, Vec2::new(1.0, 0.5));
    let wall = |body: Vec2| Vec2::new(body.x.min(5.25), body.y);
    let half = prediction.presented_position_at(DT / 2.0, DT, tuning, wall);
    assert!((half - Vec2::new(5.25, 2.75)).length() < 1e-4, "{half}");
    assert_eq!(
        prediction.presented_position_at(0.0, DT, tuning, |_| Vec2::splat(-99.0)),
        prediction.presented_position(),
        "a tick boundary presents the committed prediction without resolving"
    );
    let still = OwnerPrediction::from_pose(Vec2::ONE, 0.0);
    assert_eq!(
        still.presented_position_at(DT / 2.0, DT, tuning, |_| Vec2::splat(-99.0)),
        Vec2::ONE,
        "a still owner skips resolution"
    );
}

/// A nine-cell vertical column of dungeon walls right of the origin; returns the x coordinate
/// at which a fighter rests against its left face (body radius plus the authoritative
/// move-and-slide contact skin).
fn spawn_wall_column(app: &mut App) -> f32 {
    let catalog = brawler::map::MapContentCatalog::embedded().expect("embedded map catalog");
    let mut snapshot = catalog
        .resolve_preset(
            brawler::map::MapPresetId(1),
            brawler::map::MapInstanceId(169),
        )
        .expect("resolved fixture")
        .snapshot;
    let column = snapshot.dimensions.width / 2 + 1;
    let middle = snapshot.dimensions.height / 2;
    snapshot.placements = (middle - 4..=middle + 4)
        .zip(1..)
        .map(|(y, id)| brawler::map::MapAssetPlacement {
            placement_id: brawler::map::MapPlacementId(id),
            cell: brawler::map::MapCell::new(column, y),
            asset_id: brawler::map::WALL_DUNGEON_ASSET,
            quarter_turns: 0,
            parameters: brawler::map::MapPlacementParameters::None,
        })
        .collect();
    let face = snapshot
        .dimensions
        .half_cell_world(brawler::map::MapHalfCellPoint {
            x: column * 2,
            y: middle * 2,
        })
        .expect("inside the map")
        .x;
    assert!(face > 20.0, "the column lies right of the origin");
    let state = MapDynamicState {
        map_instance_id: snapshot.identity.instance_id,
        generation: 1,
        revision: 0,
        terminal_states: Vec::new(),
    };
    app.world_mut().spawn((MapRoot, snapshot, state));
    let radius = app
        .world()
        .resource::<brawler::builds::BuildCatalogResource>()
        .0
        .fighter_body
        .radius;
    let skin = app
        .world()
        .resource::<MovementTuning>()
        .contact_skin_world();
    face - radius - skin
}

/// Walk into the wall column with constant frames of `frame`; returns (contact x, presented
/// poses, whether a tickless frame was presented while touching the wall).
fn walk_into_wall(
    axis: Vec2,
    frame: core::time::Duration,
    frames: usize,
) -> (f32, Vec<Vec2>, bool) {
    let (mut app, owner, speed) = walking_owner_app_from(Vec2::new(-12.0, -40.0), axis);
    let contact = spawn_wall_column(&mut app);
    let tuning = *app.world().resource::<OwnerPredictionTuning>();
    let mut poses = Vec::with_capacity(frames);
    let mut tickless_contact = false;
    for index in 0..frames {
        let ticks = render_frame(&mut app, frame);
        let presented = app.world().get::<Position>(owner).expect("pose").0;
        let prediction = app.world().get::<OwnerPrediction>(owner).expect("attached");
        tickless_contact |= ticks == 0 && (prediction.position.x - contact).abs() < 1e-3;
        // Open space, more than one tick of travel from the wall: unchanged extrapolation.
        if prediction.position.x < contact - speed / 60.0 - 1.0 {
            let overstep = app
                .world()
                .resource::<Time<Fixed>>()
                .overstep()
                .as_secs_f32();
            let open =
                prediction.presented_position_at(overstep, DT, tuning, core::convert::identity);
            assert!(
                (presented - open).length() < 1e-4,
                "frame {index}: open-space presentation changed ({presented} vs {open})"
            );
        }
        poses.push(presented);
    }
    let settled = app
        .world()
        .get::<OwnerPrediction>(owner)
        .expect("attached")
        .position;
    assert!(
        (settled.x - contact).abs() < 1e-3,
        "the walk reached the wall ({} vs {contact})",
        settled.x
    );
    (contact, poses, tickless_contact)
}

const HIGH_REFRESH_FRAMES_NANOS: [u64; 2] = [8_333_333, 6_944_444];

#[test]
fn running_into_a_wall_never_presents_past_contact_or_pulls_back() {
    for nanos in HIGH_REFRESH_FRAMES_NANOS {
        let frame = core::time::Duration::from_nanos(nanos);
        let (contact, poses, tickless_contact) = walk_into_wall(Vec2::X, frame, 240);
        assert!(
            tickless_contact,
            "{nanos} ns: frames between ticks were presented at the wall"
        );
        let mut previous = poses[0].x;
        for (index, pose) in poses.iter().enumerate() {
            assert!(
                pose.x <= contact + 1e-3,
                "{nanos} ns frame {index}: presented {} past contact {contact}",
                pose.x
            );
            assert!(
                pose.x >= previous - 1e-3,
                "{nanos} ns frame {index}: pulled back {} -> {}",
                previous,
                pose.x
            );
            previous = pose.x;
        }
    }
}

#[test]
fn sliding_along_a_wall_stays_smooth() {
    let axis = Vec2::new(1.0, 1.0).normalize();
    for nanos in HIGH_REFRESH_FRAMES_NANOS {
        let frame = core::time::Duration::from_nanos(nanos);
        let (contact, poses, tickless_contact) = walk_into_wall(axis, frame, 240);
        assert!(
            tickless_contact,
            "{nanos} ns: frames between ticks were presented at the wall"
        );
        let touching: Vec<Vec2> = poses
            .iter()
            .copied()
            .filter(|pose| (pose.x - contact).abs() < 1e-3)
            .collect();
        assert!(
            touching.len() > 60,
            "{nanos} ns: the walk slid along the wall"
        );
        let steps: Vec<f32> = touching
            .windows(2)
            .map(|pair| pair[1].y - pair[0].y)
            .collect();
        let mean = steps.iter().sum::<f32>() / f32::from(u16::try_from(steps.len()).expect("few"));
        assert!(mean > 0.0);
        for (index, step) in steps.iter().enumerate() {
            assert!(
                (0.5 * mean..=1.5 * mean).contains(step),
                "{nanos} ns slide frame {index}: step {step} vs mean {mean}"
            );
        }
        for (index, pose) in poses.iter().enumerate() {
            assert!(
                pose.x <= contact + 1e-3,
                "{nanos} ns frame {index}: past contact"
            );
        }
    }
}

/// Fold one followed-authority sample confirmed at `tick` (server simulation tick `1000 + tick -
/// 100`) and record the suspended step for the input tick after it, as the fixed loop does.
fn follow_tick(prediction: &mut OwnerPrediction, tick: u32, position: Vec2, ends_at: Option<u64>) {
    let tuning = OwnerPredictionTuning::default();
    prediction.following_authority = true;
    prediction.observe_server_clock(Tick(tick), 1_000 + u64::from(tick - 100));
    prediction.reconcile_replaying(sample(tick, position), tuning, Tick(tick + 1), |_, from| {
        ReplayedStep {
            position: from,
            aim: None,
            suspended: true,
        }
    });
    stale_follow_tick(prediction, tick + 1, ends_at);
}

/// Record a suspended step for input tick `tick` without a new authoritative sample.
fn stale_follow_tick(prediction: &mut OwnerPrediction, tick: u32, ends_at: Option<u64>) {
    prediction.set_followed_motion_end(ends_at);
    let position = prediction.position;
    prediction.record_step(
        Tick(tick),
        position,
        None,
        OwnerPredictionTuning::default(),
        DT,
    );
}

fn presented_x_at(prediction: &OwnerPrediction, overstep: f32) -> f32 {
    prediction
        .presented_position_at(
            overstep,
            DT,
            OwnerPredictionTuning::default(),
            core::convert::identity,
        )
        .x
}

#[test]
fn followed_dash_presents_authoritative_motion_between_ticks_without_overshoot() {
    // An 18-tick dash at 20 units per tick ends at simulation tick 1006: the final step lands on
    // the endpoint at simulation tick 1005.
    let ends_at = Some(1_006);
    let mut prediction = OwnerPrediction::from_pose(Vec2::ZERO, 0.0);
    follow_tick(&mut prediction, 100, Vec2::ZERO, ends_at);
    assert!(
        (presented_x_at(&prediction, DT / 2.0)).abs() < 1e-4,
        "one sample measures no velocity yet"
    );
    follow_tick(&mut prediction, 101, Vec2::new(20.0, 0.0), ends_at);
    assert!((prediction.followed_velocity(DT).x - 20.0 / DT).abs() < 1e-2);
    assert!((presented_x_at(&prediction, 0.0) - 20.0).abs() < 1e-4);
    assert!((presented_x_at(&prediction, DT / 2.0) - 30.0).abs() < 1e-4);
    assert!(
        (presented_x_at(&prediction, DT) - 40.0).abs() < 1e-4,
        "a full overstep meets the next confirmed pose"
    );
    // A late sample holds the presented pose at the next confirmed pose, so a next sample
    // arriving alone continues from there instead of pulling back.
    stale_follow_tick(&mut prediction, 103, ends_at);
    assert!((presented_x_at(&prediction, 0.0) - 40.0).abs() < 1e-4);
    assert!((presented_x_at(&prediction, DT / 2.0) - 40.0).abs() < 1e-4);
    follow_tick(&mut prediction, 102, Vec2::new(40.0, 0.0), ends_at);
    assert!((presented_x_at(&prediction, 0.0) - 40.0).abs() < 1e-4);
    assert!((presented_x_at(&prediction, DT / 2.0) - 50.0).abs() < 1e-4);
    stale_follow_tick(&mut prediction, 105, ends_at);
    // Delayed samples arrive together; only the newest is folded and the pose catches up.
    follow_tick(&mut prediction, 104, Vec2::new(80.0, 0.0), ends_at);
    assert!((presented_x_at(&prediction, DT / 2.0) - 90.0).abs() < 1e-4);
    // Simulation tick 1004 is followed by the final step: travel stops at the endpoint.
    assert!((presented_x_at(&prediction, DT) - 100.0).abs() < 1e-4);
    stale_follow_tick(&mut prediction, 106, ends_at);
    assert!(
        (presented_x_at(&prediction, DT) - 100.0).abs() < 1e-4,
        "a late final sample never presents past the dash end"
    );
    follow_tick(&mut prediction, 105, Vec2::new(100.0, 0.0), ends_at);
    assert!(
        (presented_x_at(&prediction, DT) - 100.0).abs() < 1e-4,
        "the final dash pose is presented without extrapolation"
    );
}

#[test]
fn followed_motion_without_a_dash_end_or_after_a_snap_is_not_extrapolated() {
    // Defeat or an inactive phase follows authority without a dash deadline.
    let mut defeated = OwnerPrediction::from_pose(Vec2::ZERO, 0.0);
    follow_tick(&mut defeated, 100, Vec2::ZERO, None);
    follow_tick(&mut defeated, 101, Vec2::new(5.0, 0.0), None);
    assert!((presented_x_at(&defeated, DT / 2.0) - 5.0).abs() < 1e-4);
    // A respawn teleport snaps; its displacement is not presented as followed velocity.
    let mut respawned = OwnerPrediction::from_pose(Vec2::ZERO, 0.0);
    follow_tick(&mut respawned, 100, Vec2::ZERO, Some(2_000));
    follow_tick(&mut respawned, 101, Vec2::new(900.0, 0.0), Some(2_000));
    assert_eq!(respawned.followed_velocity(DT), Vec2::ZERO);
    assert!((presented_x_at(&respawned, DT / 2.0) - 900.0).abs() < 1e-4);
    // Resuming predicted movement drops the followed velocity.
    let mut resumed = OwnerPrediction::from_pose(Vec2::ZERO, 0.0);
    follow_tick(&mut resumed, 100, Vec2::ZERO, Some(2_000));
    follow_tick(&mut resumed, 101, Vec2::new(20.0, 0.0), Some(2_000));
    resumed.following_authority = false;
    assert_eq!(resumed.followed_velocity(DT), Vec2::ZERO);
}

#[test]
fn followed_velocity_includes_withdrawn_walking_steps_so_extrapolation_never_pulls_back() {
    // Before the client learned of the dash it predicted two walking steps of 5 units past the
    // newest sample; each dash sample withdraws one, so the presented pose advances 15 per tick.
    let ends_at = Some(1_020);
    let tuning = OwnerPredictionTuning::default();
    let mut prediction = OwnerPrediction::from_pose(Vec2::ZERO, 0.0);
    prediction.observe_server_clock(Tick(100), 1_000);
    prediction.reconcile(sample(100, Vec2::ZERO), tuning);
    prediction.record_step(Tick(101), Vec2::new(5.0, 0.0), None, tuning, DT);
    prediction.record_step(Tick(102), Vec2::new(10.0, 0.0), None, tuning, DT);
    prediction.following_authority = true;
    prediction.set_followed_motion_end(ends_at);
    prediction.record_step(Tick(103), Vec2::new(10.0, 0.0), None, tuning, DT);
    let mut previous_presented = presented_x_at(&prediction, DT);
    for (tick, dash_x) in [(101_u32, 20.0_f32), (102, 40.0), (103, 60.0)] {
        prediction.reconcile_replaying(
            sample(tick, Vec2::new(dash_x, 0.0)),
            tuning,
            Tick(tick + 3),
            |_, from| ReplayedStep {
                position: from,
                aim: None,
                suspended: true,
            },
        );
        prediction.record_step(Tick(tick + 3), prediction.position, None, tuning, DT);
        let start = presented_x_at(&prediction, 0.0);
        assert!(
            start + 1e-4 >= previous_presented,
            "tick {tick}: {start} < {previous_presented}"
        );
        previous_presented = presented_x_at(&prediction, DT);
    }
    assert!(
        (prediction.followed_velocity(DT).x - 20.0 / DT).abs() < 1e-2,
        "walking withdrawn"
    );
}
