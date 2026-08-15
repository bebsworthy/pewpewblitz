use super::*;
use bevy::prelude::Vec2;

fn resolved_builtin() -> (MapContentCatalog, ResolvedMap) {
    let catalog = MapContentCatalog::embedded().expect("embedded catalog");
    let resolved = catalog
        .resolve_preset(
            MapPresetId(1),
            MapInstanceId(1),
            &MapLayoutRequirements::wipeout(),
        )
        .expect("built-in preset resolves");
    (catalog, resolved)
}

#[test]
fn embedded_catalog_resolves_exact_bounded_arena() {
    let (catalog, resolved) = resolved_builtin();
    assert_eq!(catalog.presets.len(), 1);
    assert_eq!(resolved.snapshot.geometry.len(), 6);
    assert_eq!(resolved.snapshot.visual_instances.len(), 28 * 18);
    assert_eq!(resolved.snapshot.spawn_areas.len(), 2);
    assert_eq!(resolved.snapshot.spawn_points.len(), 8);
    assert_eq!(resolved.snapshot.regions.len(), 1);
    assert!(resolved.snapshot.mode_anchors.is_empty());
    assert_eq!(
        resolved.snapshot.playable_bounds.min,
        Vec2::new(-896.0, -576.0)
    );
    assert_eq!(
        resolved.snapshot.playable_bounds.max,
        Vec2::new(896.0, 576.0)
    );
    assert!(postcard::to_allocvec(&resolved.snapshot).unwrap().len() < 64 * 1_024);
}

#[test]
fn recipe_ron_round_trip_preserves_equality() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let recipe = &catalog.presets[0].recipe;
    let text = ron::to_string(recipe).unwrap();
    let round_trip: MapRecipe = ron::from_str(&text).unwrap();
    assert_eq!(&round_trip, recipe);
}

#[test]
fn canonical_recipe_identity_ignores_order_rotation_equivalence_and_signed_zero() {
    let (catalog, first) = resolved_builtin();
    let mut equivalent = catalog.presets[0].recipe.clone();
    equivalent.geometry.reverse();
    equivalent.spawn_points.reverse();
    equivalent.geometry[0].rotation = std::f32::consts::TAU;
    equivalent.geometry[0].position.y = -0.0;
    let second = resolve_map_recipe(
        &equivalent,
        None,
        MapInstanceId(2),
        &catalog,
        &MapLayoutRequirements::wipeout(),
        EngineMapLimits::default(),
    )
    .unwrap();
    assert_eq!(
        first.snapshot.identity.recipe_fingerprint,
        second.snapshot.identity.recipe_fingerprint
    );
    let mut changed = catalog.presets[0].recipe.clone();
    changed.regions[0].position.y -= 1.0;
    let changed = resolve_map_recipe(
        &changed,
        None,
        MapInstanceId(3),
        &catalog,
        &MapLayoutRequirements::wipeout(),
        EngineMapLimits::default(),
    )
    .unwrap();
    assert_ne!(
        first.snapshot.identity.recipe_fingerprint,
        changed.snapshot.identity.recipe_fingerprint
    );
}

#[test]
fn legal_non_preset_recipe_uses_the_same_resolver() {
    let (catalog, built_in) = resolved_builtin();
    let mut recipe = catalog.presets[0].recipe.clone();
    recipe.recipe_id = MapRecipeId(77);
    recipe.revision = 2;
    let resolved = resolve_map_recipe(
        &recipe,
        None,
        MapInstanceId(8),
        &catalog,
        &MapLayoutRequirements::wipeout(),
        EngineMapLimits::default(),
    )
    .unwrap();
    assert_eq!(resolved.snapshot.identity.source_preset_id, None);
    assert_eq!(resolved.snapshot.identity.recipe_id, MapRecipeId(77));
    assert_eq!(resolved.snapshot.geometry, built_in.snapshot.geometry);
}

#[test]
fn duplicate_global_placement_and_missing_anchor_fail_closed() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let mut duplicate = catalog.presets[0].recipe.clone();
    duplicate.spawn_points[0].placement_id = duplicate.geometry[0].placement_id;
    assert!(
        resolve_map_recipe(
            &duplicate,
            None,
            MapInstanceId(2),
            &catalog,
            &MapLayoutRequirements::wipeout(),
            EngineMapLimits::default(),
        )
        .unwrap_err()
        .contains("globally unique")
    );

    assert!(catalog.presets[0].recipe.mode_anchors.is_empty());
}

#[test]
fn blocked_and_wrong_facing_spawns_fail_closed() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let mut blocked = catalog.presets[0].recipe.clone();
    blocked.spawn_points[0].position = Vec2::new(-576.0, 0.0);
    assert!(
        resolve_map_recipe(
            &blocked,
            None,
            MapInstanceId(2),
            &catalog,
            &MapLayoutRequirements::wipeout(),
            EngineMapLimits::default(),
        )
        .is_err()
    );

    let mut backwards = catalog.presets[0].recipe.clone();
    backwards.spawn_points[0].facing = std::f32::consts::PI;
    assert!(
        resolve_map_recipe(
            &backwards,
            None,
            MapInstanceId(2),
            &catalog,
            &MapLayoutRequirements::wipeout(),
            EngineMapLimits::default(),
        )
        .is_err()
    );
}

#[test]
fn wipeout_rejects_unapproved_mode_anchors() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let mut blocked = catalog.presets[0].recipe.clone();
    blocked.mode_anchors.push(ModeAnchorPlacement {
        placement_id: MapPlacementId(400),
        anchor_id: ModeAnchorId(1),
        definition_id: PRACTICE_DUMMY_ANCHOR_DEFINITION,
        shape: ModeAnchorShape::Point {
            position: Vec2::ZERO,
            facing: 0.0,
        },
    });
    assert!(
        resolve_map_recipe(
            &blocked,
            None,
            MapInstanceId(2),
            &catalog,
            &MapLayoutRequirements::wipeout(),
            EngineMapLimits::default(),
        )
        .is_err()
    );
}

#[test]
fn built_in_gameplay_placements_are_mirrored() {
    let (_, resolved) = resolved_builtin();
    for geometry in &resolved.snapshot.geometry {
        assert!(resolved.snapshot.geometry.iter().any(|candidate| {
            candidate.position == -geometry.position && candidate.shape == geometry.shape
        }));
    }
    for point in &resolved.snapshot.spawn_points {
        assert!(resolved.snapshot.spawn_points.iter().any(|candidate| {
            candidate.position == -point.position && candidate.team_slot != point.team_slot
        }));
    }
}

#[test]
fn code_owned_recipe_and_snapshot_byte_limits_fail_closed() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let recipe = &catalog.presets[0].recipe;
    let recipe_limit = EngineMapLimits {
        max_recipe_bytes: postcard::to_allocvec(recipe).unwrap().len() - 1,
        ..EngineMapLimits::default()
    };
    assert!(
        resolve_map_recipe(
            recipe,
            None,
            MapInstanceId(2),
            &catalog,
            &MapLayoutRequirements::wipeout(),
            recipe_limit,
        )
        .unwrap_err()
        .contains("recipe exceeds")
    );

    let non_preset = resolve_map_recipe(
        recipe,
        None,
        MapInstanceId(2),
        &catalog,
        &MapLayoutRequirements::wipeout(),
        EngineMapLimits::default(),
    )
    .unwrap();
    let snapshot_size = postcard::to_allocvec(&non_preset.snapshot).unwrap().len();
    let snapshot_limit = EngineMapLimits {
        max_snapshot_bytes: snapshot_size - 1,
        ..EngineMapLimits::default()
    };
    assert!(
        resolve_map_recipe(
            recipe,
            None,
            MapInstanceId(2),
            &catalog,
            &MapLayoutRequirements::wipeout(),
            snapshot_limit,
        )
        .unwrap_err()
        .contains("snapshot exceeds")
    );
}
