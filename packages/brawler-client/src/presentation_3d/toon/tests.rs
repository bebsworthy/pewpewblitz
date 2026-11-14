use super::*;
use bevy::camera::Exposure;
use bevy::ecs::system::RunSystemOnce;

fn shading() -> ToonShading {
    ToonShading::embedded().expect("embedded toon shading catalog")
}

fn settings() -> ToonSettings {
    shading().extension.settings
}

#[test]
fn embedded_catalog_matches_the_decided_art_direction() {
    let settings = settings();
    // Shadow side about 0.65x albedo, hue-shifted toward blue/purple.
    let tint = settings.shadow_tint;
    let average = (tint.x + tint.y + tint.z) / 3.0;
    assert!((0.55..=0.75).contains(&average), "shadow tint {tint}");
    assert!(
        tint.z > tint.x && tint.z > tint.y,
        "shadow band shifts toward blue"
    );
    // Rim about 0.35 on fighters, relationship coloured; the world rim is weaker.
    let [world, local, ally, enemy] = settings.rim_colors;
    for fighter in [local, ally, enemy] {
        assert!((fighter.w - 0.35).abs() < 0.1);
    }
    assert!(world.w < local.w);
    assert!(local.y > local.x && local.y > local.z, "local rim is green");
    assert!(ally.z > ally.x, "ally rim is blue");
    assert!(enemy.x > enemy.y && enemy.x > enemy.z, "enemy rim is red");
}

#[test]
fn catalog_rejects_unknown_fields_bad_schema_and_out_of_range_values() {
    let parse = |source: &str| ToonShading::from_ron(source);
    assert!(parse(TOON_SHADING_CATALOG).is_ok());
    let unknown = TOON_SHADING_CATALOG.replace("softness: 0.03,", "softness: 0.03, glow: 1.0,");
    assert!(parse(&unknown).is_err());
    let schema = TOON_SHADING_CATALOG.replace("schema_version: 1", "schema_version: 2");
    assert!(parse(&schema).is_err());
    let crushed = TOON_SHADING_CATALOG.replace("(0.62, 0.60, 0.80)", "(0.1, 0.1, 0.1)");
    assert!(parse(&crushed).is_err(), "shadow band must not crush");
    let glowing = TOON_SHADING_CATALOG.replace("(0.62, 0.60, 0.80)", "(1.4, 0.60, 0.80)");
    assert!(parse(&glowing).is_err(), "shadow band must not exceed lit");
    let inverted_rim = TOON_SHADING_CATALOG.replace("end: 0.7,", "end: 0.3,");
    assert!(parse(&inverted_rim).is_err());
    let inverted_side = TOON_SHADING_CATALOG.replace("side_end: 0.85", "side_end: 0.4");
    assert!(parse(&inverted_side).is_err());
    let loud_rim = TOON_SHADING_CATALOG.replace("strength: 0.0)", "strength: 0.9)");
    assert!(parse(&loud_rim).is_err());
    let dark = TOON_SHADING_CATALOG.replace("flat_level: 0.75", "flat_level: 0.1");
    assert!(parse(&dark).is_err());
}

#[test]
fn material_factory_keeps_the_base_and_shares_one_uniform() {
    let shading = shading();
    let base = StandardMaterial {
        base_color: Color::srgba(0.3, 0.6, 0.2, 0.5),
        alpha_mode: AlphaMode::Blend,
        ..default()
    };
    let first = shading.material(base.clone());
    let second = shading.material(StandardMaterial::default());
    assert_eq!(first.base.base_color, base.base_color);
    assert_eq!(first.base.alpha_mode, AlphaMode::Blend);
    assert_eq!(first.extension, second.extension);
}

#[test]
fn rim_relations_map_to_the_shader_palette_slots() {
    // Tag 0 (no `MeshTag`) is the world rim slot.
    assert_eq!(RimRelation::Local.tag(), MeshTag(1));
    assert_eq!(RimRelation::Ally.tag(), MeshTag(2));
    assert_eq!(RimRelation::Enemy.tag(), MeshTag(3));
}

/// CPU mirror of `toon_lighting` in `assets/shaders/toon.wgsl` for a white surface under a white
/// light, before exposure: `radiance` is the directional light's Lambert radiance (illuminance /
/// pi) and `shadow_tint` the shadow-band channel considered.
fn toon_channel(
    settings: &ToonSettings,
    shadow_tint: f32,
    n_dot_l: f32,
    shadow: f32,
    radiance: f32,
) -> f32 {
    let ramp = settings.ramp;
    let edge0 = ramp.x - ramp.y;
    let edge1 = ramp.x + ramp.y;
    let t = ((n_dot_l - edge0) / (edge1 - edge0)).clamp(0.0, 1.0);
    let band = t * t * (3.0 - 2.0 * t) * shadow;
    let lit_level = ramp.z + (n_dot_l.clamp(0.0, 1.0) - ramp.z) * ramp.w;
    let shadow_level = shadow_tint * ramp.z;
    radiance * (shadow_level + (lit_level - shadow_level) * band)
}

#[test]
fn shadow_band_never_crushes_and_stays_darker_than_the_lit_band() {
    let settings = settings();
    let tint = settings.shadow_tint;
    // Perceived shadow level: the average of the hue-shifted channels.
    let average_tint = (tint.x + tint.y + tint.z) / 3.0;
    for n_dot_l in [-1.0, -0.4, 0.0] {
        let shadowed = toon_channel(&settings, average_tint, n_dot_l, 1.0, 1.0);
        assert!(
            shadowed >= MIN_SHADOW_TINT * MIN_FLAT_LEVEL,
            "back faces keep a readable fraction of the key light"
        );
    }
    // Faces of a block under the arena sun: top N·L 0.74, lit side 0.41.
    let cast_shadow = toon_channel(&settings, average_tint, 0.74, 0.0, 1.0);
    let top_face = toon_channel(&settings, average_tint, 0.74, 1.0, 1.0);
    let side_face = toon_channel(&settings, average_tint, 0.41, 1.0, 1.0);
    assert!(
        cast_shadow < side_face && side_face < top_face,
        "faces stay distinguishable"
    );
}

/// With every theme's light, a lit white toon surface plus ambient (and any rim, which mixes toward
/// at most the key light) stays below the BRL-0146 bloom knee, so only deliberate HDR glows bloom.
#[test]
fn toon_lighting_with_rim_stays_below_the_bloom_knee_on_every_theme() {
    let settings = settings();
    let tint = settings.shadow_tint;
    let brightest_tint = tint.x.max(tint.y).max(tint.z);
    let look = WorldLookCatalog::embedded().unwrap();
    let knee = look.bloom.threshold * (1.0 - look.bloom.threshold_softness);
    let exposure = Exposure::default().exposure();
    let shared = brawler::map::MapContentCatalog::embedded().unwrap();
    let visuals = environment_assets::MapVisualCatalog::embedded(&shared).unwrap();
    for theme in visuals.themes.values() {
        let radiance = theme.directional_illuminance / core::f32::consts::PI;
        let brightest = [-1.0_f32, 0.0, 0.12, 0.5, 0.74, 1.0]
            .into_iter()
            .map(|n_dot_l| toon_channel(&settings, brightest_tint, n_dot_l, 1.0, radiance))
            .fold(0.0_f32, f32::max);
        // The rim mixes toward at most the full key light.
        let lit_white = (brightest.max(radiance) + theme.ambient_brightness) * exposure;
        assert!(
            lit_white < knee,
            "theme {} toon lit white {lit_white} reaches the bloom knee {knee}",
            theme.id.0
        );
    }
}

/// Runs the scene-ready conversion for `root`, as the `WorldInstanceReady` observer does.
fn convert(app: &mut App, root: Entity) {
    app.world_mut()
        .run_system_once(move |mut conversion: ToonSceneConversion| conversion.convert(root))
        .expect("toon scene conversion runs");
}

#[test]
fn ready_toon_scenes_convert_their_materials_once() {
    let mut app = App::new();
    app.init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<ToonMaterial>>()
        .insert_resource(shading())
        .init_resource::<ToonSceneMaterials>();
    let source = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial {
            base_color: Color::srgb(1.0, 0.5, 0.5),
            ..default()
        });
    let spawn_scene = |app: &mut App| {
        let root = app.world_mut().spawn(ToonScene).id();
        let mesh = app
            .world_mut()
            .spawn((MeshMaterial3d(source.clone()), ChildOf(root)))
            .id();
        convert(app, root);
        mesh
    };
    let fighter = spawn_scene(&mut app);
    let environment = spawn_scene(&mut app);

    let toon_of = |app: &App, mesh: Entity| {
        assert!(
            app.world()
                .get::<MeshMaterial3d<StandardMaterial>>(mesh)
                .is_none(),
            "the PBR material is replaced, not doubled"
        );
        app.world()
            .get::<MeshMaterial3d<ToonMaterial>>(mesh)
            .unwrap()
            .0
            .clone()
    };
    let plain = toon_of(&app, fighter);
    assert_eq!(toon_of(&app, environment), plain, "conversions are cached");
    let materials = app.world().resource::<Assets<ToonMaterial>>();
    let color = materials.get(&plain).unwrap().base.base_color.to_srgba();
    assert!(
        (color.red - 1.0).abs() < 1e-5 && (color.green - 0.5).abs() < 1e-5,
        "the authored colour is kept"
    );
    assert_eq!(materials.len(), 1);

    // A world asset without a toon root keeps its PBR material.
    let plain_root = app.world_mut().spawn_empty().id();
    let plain_mesh = app
        .world_mut()
        .spawn((MeshMaterial3d(source.clone()), ChildOf(plain_root)))
        .id();
    convert(&mut app, plain_root);
    assert!(
        app.world()
            .get::<MeshMaterial3d<StandardMaterial>>(plain_mesh)
            .is_some()
    );
    // Arena roots carry no fixed rim: their fighter relation is tagged per frame.
    assert!(app.world().get::<MeshTag>(fighter).is_none());
}

/// BRL-0179: a shell preview root converts through the same cached arena material and tags each
/// converted mesh, including nested ones, with the player's local rim.
#[test]
fn player_preview_scenes_share_the_arena_material_and_wear_the_local_rim() {
    let mut app = App::new();
    app.init_resource::<Assets<StandardMaterial>>()
        .init_resource::<Assets<ToonMaterial>>()
        .insert_resource(shading())
        .init_resource::<ToonSceneMaterials>();
    let source = app
        .world_mut()
        .resource_mut::<Assets<StandardMaterial>>()
        .add(StandardMaterial::default());
    let arena_root = app.world_mut().spawn(ToonScene).id();
    let arena_mesh = app
        .world_mut()
        .spawn((MeshMaterial3d(source.clone()), ChildOf(arena_root)))
        .id();
    convert(&mut app, arena_root);

    let preview_root = app.world_mut().spawn(player_preview_toon_scene()).id();
    let joint = app.world_mut().spawn(ChildOf(preview_root)).id();
    let preview_mesh = app
        .world_mut()
        .spawn((MeshMaterial3d(source.clone()), ChildOf(joint)))
        .id();
    let bone = app.world_mut().spawn(ChildOf(preview_root)).id();
    convert(&mut app, preview_root);

    let material = |mesh: Entity| {
        app.world()
            .get::<MeshMaterial3d<ToonMaterial>>(mesh)
            .unwrap()
            .0
            .clone()
    };
    assert_eq!(
        material(preview_mesh),
        material(arena_mesh),
        "the preview reuses the arena's converted material"
    );
    assert_eq!(app.world().resource::<Assets<ToonMaterial>>().len(), 1);
    assert_eq!(
        app.world().get::<MeshTag>(preview_mesh),
        Some(&RimRelation::Local.tag())
    );
    assert!(
        app.world().get::<MeshTag>(bone).is_none(),
        "only meshes are tagged"
    );
}
