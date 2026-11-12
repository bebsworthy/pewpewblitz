use super::*;
use bevy::camera::Exposure;

fn catalog() -> WorldLookCatalog {
    WorldLookCatalog::embedded().expect("embedded world look catalog")
}

#[test]
fn embedded_catalog_is_valid_and_restrained() {
    let catalog = catalog();
    assert!(catalog.bloom.intensity > 0.0);
    assert!(catalog.bloom.threshold >= MIN_BLOOM_THRESHOLD);
    let bloom = catalog.bloom.bloom();
    assert_eq!(bloom.composite_mode, BloomCompositeMode::Additive);
    assert!((bloom.prefilter.threshold - catalog.bloom.threshold).abs() < f32::EPSILON);
}

#[test]
fn catalog_rejects_unknown_fields_bad_schema_and_out_of_range_values() {
    let unknown = WORLD_LOOK_CATALOG.replace("intensity: 0.16,", "intensity: 0.16, glow: 1.0,");
    assert!(WorldLookCatalog::from_ron(&unknown).is_err());
    let schema = WORLD_LOOK_CATALOG.replace("schema_version: 1", "schema_version: 2");
    assert!(WorldLookCatalog::from_ron(&schema).is_err());
    let haze = WORLD_LOOK_CATALOG.replace("intensity: 0.16,", "intensity: 0.9,");
    assert!(WorldLookCatalog::from_ron(&haze).is_err());
    let lit_bloom = WORLD_LOOK_CATALOG.replace("threshold: 3.0,", "threshold: 0.4,");
    assert!(WorldLookCatalog::from_ron(&lit_bloom).is_err());
    let oversaturated = WORLD_LOOK_CATALOG.replace("saturation: 1.15", "saturation: 2.0");
    assert!(WorldLookCatalog::from_ron(&oversaturated).is_err());
}

#[test]
fn theme_grading_bounds_are_enforced() {
    let valid = ThemeGrading {
        exposure: 0.9,
        temperature: 0.0,
        tint: 0.0,
        saturation: 1.2,
        contrast: 1.05,
    };
    assert!(valid.validate("test").is_ok());
    for invalid in [
        ThemeGrading {
            exposure: f32::NAN,
            ..valid
        },
        ThemeGrading {
            exposure: 5.0,
            ..valid
        },
        ThemeGrading {
            temperature: 0.9,
            ..valid
        },
        ThemeGrading {
            saturation: 0.2,
            ..valid
        },
        ThemeGrading {
            contrast: 2.0,
            ..valid
        },
    ] {
        assert!(invalid.validate("test").is_err());
    }
    let grading = valid.color_grading();
    assert!((grading.global.exposure - 0.9).abs() < f32::EPSILON);
    assert!(
        grading
            .all_sections()
            .all(|section| (section.saturation - 1.2).abs() < f32::EPSILON)
    );
}

/// Every theme keeps a white surface lit head-on below the bloom knee, so only deliberately HDR
/// materials bloom; grading exposure (applied after the prefilter) restores display brightness.
#[test]
fn theme_lighting_stays_below_the_bloom_knee() {
    let catalog = catalog();
    let knee = catalog.bloom.threshold * (1.0 - catalog.bloom.threshold_softness);
    let exposure = Exposure::default().exposure();
    let shared = crate::map::MapContentCatalog::embedded().unwrap();
    let visuals = environment_assets::MapVisualCatalog::embedded(&shared).unwrap();
    assert!(!visuals.themes.is_empty());
    for theme in visuals.themes.values() {
        let lit_white = theme.directional_illuminance * exposure / core::f32::consts::PI
            + theme.ambient_brightness * exposure;
        assert!(
            lit_white < knee,
            "theme {} lit white {lit_white} reaches the bloom knee {knee}",
            theme.id.0
        );
        assert!(theme.grading.validate("theme").is_ok());
    }
}

#[test]
fn glow_materials_exceed_the_bloom_threshold() {
    let threshold = catalog().bloom.threshold;
    for glow in [
        GLOW_PROJECTILE_BLUE,
        GLOW_PROJECTILE_RED,
        GLOW_MUZZLE,
        GLOW_PICKUP,
    ] {
        assert!(glow.red.max(glow.green).max(glow.blue) * glow.alpha > threshold);
    }
}

fn camera_app(reduced: bool) -> (App, Entity, Entity) {
    let mut app = App::new();
    app.insert_resource(catalog())
        .insert_resource(ClientShellSettings {
            reduced_combat_effects: reduced,
            ..Default::default()
        })
        .add_systems(Update, sync_arena_bloom);
    let arena = app
        .world_mut()
        .spawn((ArenaCamera, arena_camera_look()))
        .id();
    let hud = app.world_mut().spawn(hud_camera_look()).id();
    app.update();
    (app, arena, hud)
}

#[test]
fn arena_camera_is_hdr_filmic_and_blooms_while_hud_stays_untouched() {
    let (app, arena, hud) = camera_app(false);
    let world = app.world();
    assert!(world.get::<Hdr>(arena).is_some());
    assert_eq!(world.get::<Tonemapping>(arena), Some(&WORLD_TONEMAPPING));
    assert_eq!(WORLD_TONEMAPPING, Tonemapping::TonyMcMapface);
    assert!(world.get::<Bloom>(arena).is_some());
    assert!(world.get::<Hdr>(hud).is_some());
    assert_eq!(world.get::<Tonemapping>(hud), Some(&Tonemapping::None));
    assert!(world.get::<Bloom>(hud).is_none());
}

#[test]
fn reduced_effects_toggles_bloom_without_changing_hdr_or_tonemapping() {
    let (mut app, arena, _) = camera_app(true);
    assert!(app.world().get::<Bloom>(arena).is_none());
    assert!(app.world().get::<Hdr>(arena).is_some());
    assert_eq!(
        app.world().get::<Tonemapping>(arena),
        Some(&WORLD_TONEMAPPING)
    );

    app.world_mut()
        .resource_mut::<ClientShellSettings>()
        .reduced_combat_effects = false;
    app.update();
    assert!(app.world().get::<Bloom>(arena).is_some());

    app.world_mut()
        .resource_mut::<ClientShellSettings>()
        .reduced_combat_effects = true;
    app.update();
    assert!(app.world().get::<Bloom>(arena).is_none());
    assert!(app.world().get::<Hdr>(arena).is_some());
}

#[test]
fn theme_grading_is_written_to_the_arena_camera_only() {
    let mut app = App::new();
    let arena = app
        .world_mut()
        .spawn((ArenaCamera, arena_camera_look()))
        .id();
    let preview = app.world_mut().spawn(ColorGrading::default()).id();
    let grading = ThemeGrading {
        exposure: 0.8,
        temperature: 0.02,
        tint: 0.0,
        saturation: 1.2,
        contrast: 1.05,
    };
    app.add_systems(
        Update,
        move |mut cameras: Query<&mut ColorGrading, With<ArenaCamera>>| {
            apply_theme_grading(grading, &mut cameras);
        },
    );
    app.update();
    let applied = app.world().get::<ColorGrading>(arena).unwrap();
    assert!((applied.global.exposure - 0.8).abs() < f32::EPSILON);
    assert!((applied.global.temperature - 0.02).abs() < f32::EPSILON);
    let untouched = app.world().get::<ColorGrading>(preview).unwrap();
    assert!(untouched.global.exposure.abs() < f32::EPSILON);
}

#[test]
fn preview_cameras_share_the_world_tonemapper() {
    let (tonemapping, grading) = catalog().preview_camera_look();
    assert_eq!(tonemapping, WORLD_TONEMAPPING);
    assert!(grading.global.exposure.is_finite());
}
