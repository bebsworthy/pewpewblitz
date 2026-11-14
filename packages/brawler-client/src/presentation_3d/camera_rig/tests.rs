use super::super::camera;
use super::*;
use crate::local_attack::{LocalAttackKind, LocalShotId};
use bevy::time::TimeUpdateStrategy;
use brawler::combat::{AttackId, CombatCue, CombatEventId, TeamId, WorldPoint};
use core::time::Duration;

const FRAME: f32 = 1.0 / 60.0;
const EPSILON: f32 = 0.05;

fn catalog() -> CameraRigCatalog {
    CameraRigCatalog::embedded().expect("embedded camera rig catalog")
}

fn centered_map(width_cells: f32, height_cells: f32) -> AxisAlignedMapRect {
    let half = Vec2::new(width_cells, height_cells) * MAP_CELL_SIZE_WORLD * 0.5;
    AxisAlignedMapRect {
        min: -half,
        max: half,
    }
}

fn input(fighter: Vec2, bounds: AxisAlignedMapRect) -> CameraRigFrameInput {
    CameraRigFrameInput {
        fighter: Some(fighter),
        aim: None,
        bounds,
        viewport: DEFAULT_GAMEPLAY_VIEWPORT,
        dt: FRAME,
        reduced_motion: false,
        reduced_effects: false,
        shake_scale: 1.0,
    }
}

fn visible(center: Vec2) -> AxisAlignedMapRect {
    let footprint = camera::conservative_ground_footprint(
        DEFAULT_GAMEPLAY_VIEWPORT.x / DEFAULT_GAMEPLAY_VIEWPORT.y,
    );
    AxisAlignedMapRect {
        min: center + footprint.min,
        max: center + footprint.max,
    }
}

#[test]
fn embedded_catalog_is_valid_and_ceilings_fail_closed() {
    let catalog = catalog();
    assert!(catalog.look_ahead.distance_cells * MAP_CELL_SIZE_WORLD <= MAX_CAMERA_LEAD_WORLD);

    let oversized_shake =
        CAMERA_RIG_CATALOG.replace("max_offset_world: 6.0", "max_offset_world: 40.0");
    assert!(CameraRigCatalog::from_ron(&oversized_shake).is_err());
    let oversized_lead = CAMERA_RIG_CATALOG.replace("distance_cells: 1.5", "distance_cells: 9.0");
    assert!(CameraRigCatalog::from_ron(&oversized_lead).is_err());
    let unknown = CAMERA_RIG_CATALOG.replace("exponent: 2.0,", "exponent: 2.0, roll: 1.0,");
    assert!(CameraRigCatalog::from_ron(&unknown).is_err());
    let oversized_spread = CAMERA_RIG_CATALOG.replace("spread_scale: 1.75", "spread_scale: 9.0");
    assert!(CameraRigCatalog::from_ron(&oversized_spread).is_err());
    let inverted_damage = CAMERA_RIG_CATALOG.replace("maximum: 0.6", "maximum: 0.1");
    assert!(CameraRigCatalog::from_ron(&inverted_damage).is_err());
    let schema = CAMERA_RIG_CATALOG.replace("schema_version: 2", "schema_version: 1");
    assert!(CameraRigCatalog::from_ron(&schema).is_err());
}

#[test]
fn spring_converges_without_overshoot_at_any_frame_rate() {
    let target = Vec2::new(100.0, -40.0);
    for (hz, steps) in [(30.0_f32, 30), (60.0, 60), (144.0, 144)] {
        let mut spring = Spring::at(Vec2::ZERO);
        let mut previous_distance = f32::INFINITY;
        for _ in 0..steps {
            let value = spring.step(target, 0.11, 1.0 / hz);
            let distance = value.distance(target);
            assert!(distance <= previous_distance + 1e-4, "spring moved away");
            assert!(value.x <= target.x + 1e-4 && value.y >= target.y - 1e-4);
            previous_distance = distance;
        }
        assert!(previous_distance < 0.1, "{hz} Hz did not converge");
    }
    let mut halfway = Vec::new();
    // A tenth of a second at 30 Hz and at 150 Hz.
    for (hz, steps) in [(30.0_f32, 3), (150.0, 15)] {
        let mut spring = Spring::at(Vec2::ZERO);
        for _ in 0..steps {
            spring.step(target, 0.11, 1.0 / hz);
        }
        halfway.push(spring.value);
    }
    assert!(
        halfway[0].distance(halfway[1]) < 4.0,
        "follow depends on frame rate: {halfway:?}"
    );
}

#[test]
fn follow_smooths_small_moves_and_snaps_large_jumps() {
    let catalog = catalog();
    let bounds = centered_map(50.0, 50.0);
    let mut rig = CameraRig::default();
    assert_eq!(
        rig.step(&catalog, input(Vec2::ZERO, bounds)).center,
        Vec2::ZERO
    );

    let moved = Vec2::new(64.0, 32.0);
    let first = rig.step(&catalog, input(moved, bounds)).center;
    assert!(
        first.length() > 0.0 && first.distance(moved) > 1.0,
        "follow is rigid"
    );
    let mut center = first;
    for _ in 0..60 {
        center = rig.step(&catalog, input(moved, bounds)).center;
    }
    assert!(
        center.distance(moved) < EPSILON,
        "follow did not converge: {center}"
    );

    let respawn = Vec2::new(-600.0, 500.0);
    let snapped = rig.step(&catalog, input(respawn, bounds)).center;
    assert_eq!(
        snapped,
        clamp_3d_camera_center(respawn, bounds, DEFAULT_GAMEPLAY_VIEWPORT)
    );
}

#[test]
fn reduced_motion_shortens_follow_and_disables_look_ahead_and_shake() {
    let catalog = catalog();
    let bounds = centered_map(50.0, 50.0);
    let moved = Vec2::new(64.0, 0.0);
    let mut normal = CameraRig::default();
    let mut reduced = CameraRig::default();
    let reduced_input = |fighter| CameraRigFrameInput {
        reduced_motion: true,
        aim: Some((Vec2::Y, None)),
        ..input(fighter, bounds)
    };
    normal.step(&catalog, input(Vec2::ZERO, bounds));
    reduced.step(&catalog, reduced_input(Vec2::ZERO));
    let normal_center = normal.step(&catalog, input(moved, bounds)).center;
    reduced.add_trauma(1.0);
    let reduced_frame = reduced.step(&catalog, reduced_input(moved));
    assert!(reduced_frame.center.distance(moved) < normal_center.distance(moved));
    assert_eq!(reduced_frame.shake, Vec2::ZERO);
    assert!(reduced.trauma().abs() < f32::EPSILON);
    for _ in 0..60 {
        let frame = reduced.step(&catalog, reduced_input(moved));
        assert_eq!(frame.shake, Vec2::ZERO);
    }
    assert!(
        reduced
            .step(&catalog, reduced_input(moved))
            .center
            .distance(moved)
            < EPSILON,
        "reduced motion must not lean toward aim"
    );
}

#[test]
fn reduced_effects_disables_shake() {
    let catalog = catalog();
    let mut rig = CameraRig::default();
    rig.add_trauma(1.0);
    let frame = rig.step(
        &catalog,
        CameraRigFrameInput {
            reduced_effects: true,
            ..input(Vec2::ZERO, centered_map(50.0, 50.0))
        },
    );
    assert_eq!(frame.shake, Vec2::ZERO);
}

#[test]
fn look_ahead_leans_toward_aim_and_respects_framing_at_every_edge() {
    let catalog = catalog();
    let lean = catalog.look_ahead.distance_cells * MAP_CELL_SIZE_WORLD;
    for bounds in [centered_map(50.0, 50.0), centered_map(25.0, 37.0)] {
        for fighter in [
            Vec2::ZERO,
            Vec2::new(bounds.min.x, 0.0),
            Vec2::new(bounds.max.x, 0.0),
            Vec2::new(0.0, bounds.min.y),
            Vec2::new(0.0, bounds.max.y),
            bounds.max,
            bounds.min,
        ] {
            for aim in [
                Vec2::X,
                Vec2::NEG_X,
                Vec2::Y,
                Vec2::NEG_Y,
                Vec2::ONE.normalize(),
            ] {
                let mut rig = CameraRig::default();
                let mut center = Vec2::ZERO;
                for _ in 0..120 {
                    center = rig
                        .step(
                            &catalog,
                            CameraRigFrameInput {
                                aim: Some((aim, None)),
                                ..input(fighter, bounds)
                            },
                        )
                        .center;
                }
                // The framed center is a fixed point of the unchanged framing clamp, so the
                // look-ahead never reveals space the clamp forbids.
                assert_eq!(
                    clamp_3d_camera_center(center, bounds, DEFAULT_GAMEPLAY_VIEWPORT),
                    center
                );
                let view = visible(center);
                assert!(fighter.x >= view.min.x - EPSILON && fighter.x <= view.max.x + EPSILON);
                assert!(fighter.y >= view.min.y - EPSILON && fighter.y <= view.max.y + EPSILON);
                // Framing is 1-Lipschitz per axis, so the lead ceiling bounds the framed offset.
                let rigid = clamp_3d_camera_center(fighter, bounds, DEFAULT_GAMEPLAY_VIEWPORT);
                assert!(center.distance(rigid) <= MAX_CAMERA_LEAD_WORLD + EPSILON);
                if fighter == Vec2::ZERO && bounds == centered_map(50.0, 50.0) {
                    assert!(center.distance(aim * lean) < EPSILON, "{center} vs {aim}");
                }
            }
        }
    }
}

#[test]
fn look_ahead_strength_ramps_with_aim_distance_and_clears_without_aim() {
    let tuning = catalog().look_ahead;
    let full = tuning.full_strength_aim_distance_cells * MAP_CELL_SIZE_WORLD;
    let lean = tuning.distance_cells * MAP_CELL_SIZE_WORLD;
    assert_eq!(look_ahead_target(&tuning, None), Vec2::ZERO);
    assert_eq!(
        look_ahead_target(&tuning, Some((Vec2::ZERO, None))),
        Vec2::ZERO
    );
    assert!(
        (look_ahead_target(&tuning, Some((Vec2::X, Some(full * 0.5)))).x - lean * 0.5).abs() < 1e-3
    );
    assert!((look_ahead_target(&tuning, Some((Vec2::X, Some(full * 4.0)))).x - lean).abs() < 1e-3);
    assert!((look_ahead_target(&tuning, Some((Vec2::X, None))).x - lean).abs() < 1e-3);
}

#[test]
fn trauma_decays_linearly_and_shake_stays_within_its_ceiling() {
    let catalog = catalog();
    let bounds = centered_map(50.0, 50.0);
    let mut rig = CameraRig::default();
    rig.step(&catalog, input(Vec2::ZERO, bounds));
    rig.add_trauma(0.8);
    rig.add_trauma(0.8);
    assert!(
        (rig.trauma() - 1.0).abs() < f32::EPSILON,
        "trauma saturates"
    );
    let mut previous = rig.trauma();
    let mut peak: f32 = 0.0;
    let frames = 40_u16;
    assert!(f32::from(frames) * FRAME * catalog.shake.decay_per_second >= 1.0);
    for _ in 0..frames {
        let frame = rig.step(&catalog, input(Vec2::ZERO, bounds));
        assert!(rig.trauma() <= previous);
        previous = rig.trauma();
        peak = peak.max(frame.shake.length());
        assert!(frame.shake.length() <= catalog.shake.max_offset_world * 1.42);
    }
    assert!(peak > 0.5, "full trauma must visibly shake: {peak}");
    assert!(rig.trauma().abs() < f32::EPSILON);
    assert_eq!(
        rig.step(&catalog, input(Vec2::ZERO, bounds)).shake,
        Vec2::ZERO
    );
}

#[test]
fn shake_noise_is_continuous_across_the_phase_wrap() {
    let before = shake_noise(NOISE_PHASE_WRAP - 1e-3);
    let after = shake_noise(1e-3_f32.rem_euclid(NOISE_PHASE_WRAP));
    assert!(before.distance(after) < 0.05, "{before} vs {after}");
}

#[test]
fn removing_shake_restores_the_framed_aim_view() {
    let framed = Transform::from_xyz(10.0, 400.0, 300.0).looking_at(Vec3::ZERO, Vec3::Y);
    let shake = Vec3::new(4.0, 0.0, -3.0);
    let shaken = Transform::from_translation(framed.translation + shake).looking_at(shake, Vec3::Y);
    let restored =
        unshaken_camera_transform(GlobalTransform::from(shaken), Some(ArenaCameraShake(shake)));
    assert!(restored.translation().abs_diff_eq(framed.translation, 1e-3));
    assert!(restored.rotation().abs_diff_eq(framed.rotation, 1e-5));
}

fn rig_app() -> App {
    let mut app = App::new();
    app.add_plugins(MinimalPlugins)
        .insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs_f32(
            FRAME,
        )))
        .init_resource::<CameraRig>()
        .insert_resource(catalog())
        .insert_resource(brawler::map::PresentedMap {
            source_root: Entity::PLACEHOLDER,
            instance_id: brawler::map::MapInstanceId(1),
            recipe_fingerprint: brawler::map::MapRecipeFingerprint(1),
            presentation_theme_id: brawler::map::MapPresentationThemeId(1),
            playable_bounds: centered_map(50.0, 50.0),
            camera_bounds: centered_map(50.0, 50.0),
        })
        .add_message::<LocalAttackPresented>()
        .add_message::<DeduplicatedCombatCue>()
        .add_systems(Update, (accumulate_camera_trauma, follow_3d_camera).chain());
    app.world_mut().spawn((
        ArenaCamera,
        ArenaCameraShake::default(),
        Projection::default(),
        Transform::default(),
    ));
    app
}

fn camera_center(app: &mut App) -> Vec2 {
    let translation = app
        .world_mut()
        .query_filtered::<&Transform, With<ArenaCamera>>()
        .single(app.world())
        .expect("one arena camera")
        .translation;
    let shake = camera_shake(app);
    ground_point(translation - shake - camera_offset())
}

fn camera_shake(app: &mut App) -> Vec3 {
    app.world_mut()
        .query::<&ArenaCameraShake>()
        .single(app.world())
        .expect("shake component")
        .0
}

#[test]
fn ecs_camera_follow_converges_on_the_presented_fighter() {
    let mut app = rig_app();
    let fighter = app
        .world_mut()
        .spawn((
            Fighter,
            Controlled,
            NetworkEntityId(7),
            Position(Vec2::ZERO),
        ))
        .id();
    app.update();
    app.update();
    assert!(camera_center(&mut app).length() < EPSILON);

    let target = Vec2::new(96.0, -64.0);
    app.world_mut().get_mut::<Position>(fighter).unwrap().0 = target;
    app.update();
    let first = camera_center(&mut app);
    assert!(
        first.length() > 0.0 && first.distance(target) > 1.0,
        "{first}"
    );
    for _ in 0..60 {
        app.update();
    }
    assert!(camera_center(&mut app).distance(target) < EPSILON);
    assert_eq!(camera_shake(&mut app), Vec3::ZERO);
}

#[test]
fn ecs_local_fire_and_damage_shake_then_decay_to_rest() {
    let mut app = rig_app();
    let fighter = app
        .world_mut()
        .spawn((
            Fighter,
            Controlled,
            NetworkEntityId(7),
            Position(Vec2::ZERO),
        ))
        .id();
    app.update();
    app.update();
    let catalog = catalog();
    app.world_mut().write_message(LocalAttackPresented {
        fighter,
        owner: NetworkEntityId(7),
        team: TeamId(0),
        kind: LocalAttackKind::PrimaryFire(LocalShotId(1)),
        origin: Vec2::ZERO,
        facing: 0.0,
        simulation_tick: 10,
    });
    app.world_mut()
        .write_message(DeduplicatedCombatCue(CombatCue::FighterDefeated {
            event_id: CombatEventId(3),
            tick: 10,
            attack_id: AttackId(4),
            source: None,
            target: NetworkEntityId(99),
            position: WorldPoint::from(Vec2::new(
                catalog.shake.triggers.falloff_radius_cells * MAP_CELL_SIZE_WORLD * 0.5,
                0.0,
            )),
        }));
    app.update();
    let expected = catalog.shake.triggers.local_fire.for_weapon(None)
        + catalog.shake.triggers.nearby_defeat * 0.5
        - catalog.shake.decay_per_second * FRAME;
    let trauma = app.world().resource::<CameraRig>().trauma();
    assert!((trauma - expected).abs() < 1e-3, "{trauma} vs {expected}");

    app.world_mut()
        .write_message(DeduplicatedCombatCue(CombatCue::FighterDefeated {
            event_id: CombatEventId(5),
            tick: 11,
            attack_id: AttackId(6),
            source: None,
            target: NetworkEntityId(7),
            position: WorldPoint::from(Vec2::ZERO),
        }));
    app.update();
    let mut peak: f32 = 0.0;
    for _ in 0..6 {
        peak = peak.max(camera_shake(&mut app).length());
        app.update();
    }
    assert!(peak > 0.0, "local defeat must shake the camera");
    assert!(
        camera_shake(&mut app).y.abs() < f32::EPSILON,
        "shake stays on the ground plane"
    );
    for _ in 0..90 {
        app.update();
    }
    assert!(app.world().resource::<CameraRig>().trauma().abs() < f32::EPSILON);
    assert_eq!(camera_shake(&mut app), Vec3::ZERO);
    assert!(
        camera_center(&mut app).length() < EPSILON,
        "shake never moves the framed center"
    );
}

#[test]
fn ecs_reduced_motion_keeps_shake_at_zero() {
    let mut app = rig_app();
    app.insert_resource(ClientShellSettings {
        reduced_motion: true,
        ..Default::default()
    });
    app.world_mut().spawn((
        Fighter,
        Controlled,
        NetworkEntityId(7),
        Position(Vec2::ZERO),
    ));
    app.update();
    for event in 0..10 {
        app.world_mut()
            .write_message(DeduplicatedCombatCue(CombatCue::FighterDefeated {
                event_id: CombatEventId(event),
                tick: event,
                attack_id: AttackId(event),
                source: None,
                target: NetworkEntityId(7),
                position: WorldPoint::from(Vec2::ZERO),
            }));
        app.update();
        assert_eq!(camera_shake(&mut app), Vec3::ZERO);
    }
}

#[test]
fn ecs_framing_writes_the_aspect_fair_field_of_view() {
    let mut app = rig_app();
    for mut projection in app
        .world_mut()
        .query::<&mut Projection>()
        .iter_mut(app.world_mut())
    {
        if let Projection::Perspective(perspective) = projection.as_mut() {
            perspective.fov = 1.0;
        }
    }
    app.update();
    let projection = app
        .world_mut()
        .query::<&Projection>()
        .single(app.world())
        .expect("one projection");
    let Projection::Perspective(perspective) = projection else {
        panic!("arena camera keeps a perspective projection");
    };
    let expected =
        camera::camera_vertical_fov(DEFAULT_GAMEPLAY_VIEWPORT.x / DEFAULT_GAMEPLAY_VIEWPORT.y);
    assert!((perspective.fov - expected).abs() < 1e-6);
}

fn preset_loadout(preset: u16) -> ResolvedMatchLoadout {
    let builds = brawler::builds::BuildCatalog::embedded().expect("embedded build catalog");
    let weapons = brawler::combat::WeaponCatalog::embedded().expect("embedded weapon catalog");
    brawler::builds::resolve_build_recipe(
        &builds,
        &weapons,
        brawler::builds::BrawlerBuildRecipe {
            weapon: brawler::builds::WeaponChoice::Preset(brawler::combat::WeaponPresetId(preset)),
            ultimate: brawler::builds::UltimateDefinitionId(1),
            passives: [
                brawler::builds::PassiveDefinitionId(1),
                brawler::builds::PassiveDefinitionId(6),
            ],
        },
    )
    .expect("preset loadout resolves")
}

fn preset_fire_trauma(preset: u16) -> f32 {
    catalog()
        .shake
        .triggers
        .local_fire
        .for_weapon(Some(&preset_loadout(preset).primary_weapon.recipe))
}

#[test]
fn heavy_weapons_shake_more_than_light_ones() {
    let pulse_sidearm = preset_fire_trauma(1);
    let scatter_cannon = preset_fire_trauma(2);
    let arc_launcher = preset_fire_trauma(3);
    let spray = preset_fire_trauma(6);
    assert!(
        scatter_cannon > pulse_sidearm,
        "{scatter_cannon} vs {pulse_sidearm}"
    );
    assert!(
        arc_launcher > pulse_sidearm,
        "{arc_launcher} vs {pulse_sidearm}"
    );
    assert!(spray < pulse_sidearm, "{spray} vs {pulse_sidearm}");
    let fire = catalog().shake.triggers.local_fire;
    assert!((fire.for_weapon(None) - fire.straight).abs() < f32::EPSILON);
    assert!((scatter_cannon - fire.straight * fire.spread_scale).abs() < 1e-6);
    for preset in 1..=7 {
        let trauma = preset_fire_trauma(preset);
        assert!((0.0..=1.0).contains(&trauma), "preset {preset}: {trauma}");
    }
}

#[test]
fn damage_trauma_scales_with_the_hit_share_of_maximum_health() {
    let damage = catalog().shake.triggers.local_damage;
    let light = damage.for_hit(10, Some(1_000));
    let heavy = damage.for_hit(400, Some(1_000));
    assert!(light < heavy, "{light} vs {heavy}");
    assert!((damage.for_hit(0, Some(1_000)) - damage.minimum).abs() < f32::EPSILON);
    assert!(
        (heavy - damage.maximum).abs() < f32::EPSILON,
        "saturates at full strength"
    );
    assert!((damage.for_hit(u16::MAX, Some(1)) - damage.maximum).abs() < f32::EPSILON);
    assert!((damage.for_hit(500, None) - damage.minimum).abs() < f32::EPSILON);
    assert!((damage.for_hit(500, Some(0)) - damage.minimum).abs() < f32::EPSILON);
}

#[test]
fn shake_intensity_scales_amplitude_and_zero_turns_it_off() {
    let catalog = catalog();
    let bounds = centered_map(50.0, 50.0);
    let shaken = |scale: f32| {
        let mut rig = CameraRig::default();
        rig.step(&catalog, input(Vec2::ZERO, bounds));
        rig.add_trauma(1.0);
        let frame = rig.step(
            &catalog,
            CameraRigFrameInput {
                shake_scale: scale,
                ..input(Vec2::ZERO, bounds)
            },
        );
        (frame.shake, rig.trauma())
    };
    let (full, full_trauma) = shaken(1.0);
    let (half, half_trauma) = shaken(0.5);
    let (off, off_trauma) = shaken(0.0);
    assert!(full.length() > 0.0);
    assert!(half.abs_diff_eq(full * 0.5, 1e-4), "{half} vs {full}");
    assert_eq!(off, Vec2::ZERO);
    assert_eq!(shaken(f32::NAN).0, Vec2::ZERO);
    assert!(
        shaken(4.0).0.abs_diff_eq(full, 1e-4),
        "scale is clamped to 1"
    );
    // The intensity is an output scale: trauma decays identically at every setting.
    assert!((full_trauma - half_trauma).abs() < f32::EPSILON);
    assert!((full_trauma - off_trauma).abs() < f32::EPSILON);

    for (reduced_motion, reduced_effects) in [(true, false), (false, true)] {
        let mut rig = CameraRig::default();
        rig.add_trauma(1.0);
        let frame = rig.step(
            &catalog,
            CameraRigFrameInput {
                reduced_motion,
                reduced_effects,
                shake_scale: 1.0,
                ..input(Vec2::ZERO, bounds)
            },
        );
        assert_eq!(
            frame.shake,
            Vec2::ZERO,
            "reduced settings override intensity"
        );
    }
}

#[test]
fn ecs_local_fire_follows_the_loadout_and_damage_follows_the_hit() {
    let mut app = rig_app();
    let loadout = preset_loadout(3);
    let maximum_health = loadout.fighter_stats.maximum_health;
    let fighter = app
        .world_mut()
        .spawn((
            Fighter,
            Controlled,
            NetworkEntityId(7),
            Position(Vec2::ZERO),
            loadout,
        ))
        .id();
    app.update();
    app.update();
    let catalog = catalog();
    let triggers = catalog.shake.triggers;
    let decay = catalog.shake.decay_per_second * FRAME;
    let trauma_after = |app: &mut App, kind: LocalAttackKind, damage: Option<u16>| {
        app.world_mut().resource_mut::<CameraRig>().reset();
        app.world_mut().write_message(LocalAttackPresented {
            fighter,
            owner: NetworkEntityId(7),
            team: TeamId(0),
            kind,
            origin: Vec2::ZERO,
            facing: 0.0,
            simulation_tick: 10,
        });
        if let Some(amount) = damage {
            app.world_mut()
                .write_message(DeduplicatedCombatCue(CombatCue::DamageApplied {
                    event_id: CombatEventId(20),
                    tick: 10,
                    attack_id: AttackId(21),
                    source: brawler::combat::DamageSource::HostileNpc {
                        npc_id: NetworkEntityId(99),
                        definition_id: 1,
                    },
                    target: NetworkEntityId(7),
                    position: WorldPoint::from(Vec2::ZERO),
                    amount,
                    health_after: maximum_health.saturating_sub(amount),
                    distance_band: brawler::combat::DistanceBand::Close,
                }));
        }
        app.update();
        app.world().resource::<CameraRig>().trauma()
    };
    let lobbed = trauma_after(&mut app, LocalAttackKind::PrimaryFire(LocalShotId(1)), None);
    assert!(
        (lobbed - (triggers.local_fire.lobbed - decay)).abs() < 1e-4,
        "{lobbed}"
    );
    let ultimate = trauma_after(
        &mut app,
        LocalAttackKind::Ultimate {
            kind: brawler::builds::UltimateKind::Dash,
            cast: local_attack::LocalCastId(1),
        },
        None,
    );
    assert!(
        (ultimate - (triggers.local_ultimate - decay)).abs() < 1e-4,
        "{ultimate}"
    );
    let heavy_hit = trauma_after(
        &mut app,
        LocalAttackKind::PrimaryFire(LocalShotId(2)),
        Some(maximum_health),
    );
    let expected = (triggers.local_fire.lobbed + triggers.local_damage.maximum).min(1.0) - decay;
    assert!(
        (heavy_hit - expected).abs() < 1e-4,
        "{heavy_hit} vs {expected}"
    );
}

#[test]
fn ecs_camera_shake_setting_scales_the_applied_offset() {
    let mut peaks = Vec::new();
    for intensity in [100, 50, 0] {
        let mut app = rig_app();
        app.insert_resource(ClientShellSettings {
            camera_shake: intensity,
            ..Default::default()
        });
        app.world_mut().spawn((
            Fighter,
            Controlled,
            NetworkEntityId(7),
            Position(Vec2::ZERO),
        ));
        app.update();
        app.world_mut().resource_mut::<CameraRig>().add_trauma(1.0);
        let mut peak: f32 = 0.0;
        for _ in 0..30 {
            app.update();
            peak = peak.max(camera_shake(&mut app).length());
        }
        peaks.push(peak);
    }
    assert!(peaks[0] > 0.0);
    assert!((peaks[1] - peaks[0] * 0.5).abs() < 1e-3, "{peaks:?}");
    assert!(peaks[2].abs() < f32::EPSILON, "{peaks:?}");
}
