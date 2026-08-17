#![allow(
    clippy::cast_possible_truncation,
    clippy::cast_possible_wrap,
    clippy::cast_precision_loss,
    clippy::cast_sign_loss,
    reason = "deliberate boundary-value test math"
)]

use super::*;
use crate::map::HOT_ZONE_MAP_PRESET;
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
    assert_eq!(catalog.presets.len(), 2);
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
fn embedded_hot_zone_preset_resolves_one_central_area_anchor() {
    let (catalog, _) = resolved_builtin();
    let resolved = catalog
        .resolve_preset(
            HOT_ZONE_MAP_PRESET,
            MapInstanceId(2),
            &MapLayoutRequirements::hot_zone(),
        )
        .expect("built-in Hot Zone preset resolves");
    assert_eq!(
        resolved.snapshot.mode_definition_id,
        HOT_ZONE_MODE_DEFINITION
    );
    assert_eq!(resolved.snapshot.mode_anchors.len(), 1);
    let anchor = &resolved.snapshot.mode_anchors[0];
    assert_eq!(anchor.definition_id, HOT_ZONE_OBJECTIVE_ANCHOR_DEFINITION);
    assert_eq!(
        objective_presentation_profile(anchor.definition_id),
        Some(HOT_ZONE_OBJECTIVE_PRESENTATION_PROFILE)
    );
    match anchor.shape {
        ModeAnchorShape::Area {
            position,
            shape: MapShape::Circle { radius },
        } => {
            assert_eq!(position, Vec2::ZERO);
            assert_eq!(radius.to_bits(), 160.0_f32.to_bits());
        }
        _ => panic!("objective anchor must be an area"),
    }
    assert_eq!(resolved.snapshot.geometry.len(), 6);
    assert_eq!(resolved.snapshot.spawn_points.len(), 8);
}

#[test]
fn hot_zone_requirements_reject_point_anchors_and_wipeout_rejects_area_anchors() {
    let (catalog, _) = resolved_builtin();
    let mut recipe = catalog.presets[1].recipe.clone();
    recipe.mode_anchors[0].shape = ModeAnchorShape::Point {
        position: Vec2::ZERO,
        facing: 0.0,
    };
    recipe.revision += 1;
    assert!(
        resolve_map_recipe(
            &recipe,
            Some(HOT_ZONE_MAP_PRESET),
            MapInstanceId(3),
            &catalog,
            &MapLayoutRequirements::hot_zone(),
            EngineMapLimits::default(),
        )
        .is_err()
    );

    let mut wipeout_recipe = catalog.presets[0].recipe.clone();
    wipeout_recipe.mode_anchors.push(ModeAnchorPlacement {
        placement_id: MapPlacementId(400),
        anchor_id: ModeAnchorId(1),
        definition_id: HOT_ZONE_OBJECTIVE_ANCHOR_DEFINITION,
        shape: ModeAnchorShape::Area {
            position: Vec2::ZERO,
            shape: MapShape::Circle { radius: 160.0 },
        },
    });
    wipeout_recipe.revision += 1;
    assert!(
        resolve_map_recipe(
            &wipeout_recipe,
            Some(MapPresetId(1)),
            MapInstanceId(4),
            &catalog,
            &MapLayoutRequirements::wipeout(),
            EngineMapLimits::default(),
        )
        .is_err()
    );
}

#[test]
fn objective_areas_must_stay_inside_bounds_and_clear_permanent_terrain() {
    let (catalog, _) = resolved_builtin();
    let base = catalog.presets[1].recipe.clone();
    for (position, radius, description) in [
        (Vec2::new(0.0, 216.0), 160.0, "overlaps central wall"),
        (Vec2::new(780.0, 0.0), 160.0, "outside playable bounds"),
        (Vec2::ZERO, 0.0, "zero radius"),
        (Vec2::new(f32::NAN, 0.0), 160.0, "non-finite position"),
    ] {
        let mut recipe = base.clone();
        recipe.mode_anchors[0].shape = ModeAnchorShape::Area {
            position,
            shape: MapShape::Circle { radius },
        };
        recipe.revision += 1;
        assert!(
            resolve_map_recipe(
                &recipe,
                Some(HOT_ZONE_MAP_PRESET),
                MapInstanceId(5),
                &catalog,
                &MapLayoutRequirements::hot_zone(),
                EngineMapLimits::default(),
            )
            .is_err(),
            "expected rejection for an objective that {description}"
        );
    }
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

// --- Initial destructible terrain resolution (Milestone 10) ---

use crate::map::resolve_initial_terrain;
use crate::terrain::TerrainChunkId;

fn region(placement: u32, position: Vec2, half_extents: Vec2) -> MapRegionPlacement {
    MapRegionPlacement {
        placement_id: MapPlacementId(placement),
        region_id: RegionId(1),
        profile_id: DESTRUCTIBLE_TERRAIN_REGION_PROFILE,
        presentation_profile_id: MapPresentationProfileId(3),
        position,
        rotation: 0.0,
        shape: MapShape::Rectangle { half_extents },
    }
}

fn resolve_terrain(
    bounds: AxisAlignedMapRect,
    regions: &[MapRegionPlacement],
) -> Result<InitialTerrainLayout, String> {
    resolve_initial_terrain(bounds, &[], regions, &[], &[], EngineMapLimits::default())
}

#[test]
fn built_in_presets_allocate_the_exact_central_terrain_block() {
    for preset in MapContentCatalog::embedded().unwrap().presets {
        let requirements =
            MapLayoutRequirements::for_mode_definition(preset.recipe.mode_definition_id).unwrap();
        let resolved = catalog_resolve(preset.id, &requirements);
        let layout = resolve_initial_terrain(
            resolved.snapshot.playable_bounds,
            &resolved.snapshot.geometry,
            &resolved.snapshot.regions,
            &resolved.snapshot.spawn_points,
            &resolved.snapshot.mode_anchors,
            EngineMapLimits::default(),
        )
        .unwrap();
        assert_eq!(layout.total_cells, 576, "192x192 units at 8-unit cells");
        assert_eq!(
            layout.chunks.keys().copied().collect::<Vec<_>>(),
            vec![
                TerrainChunkId { x: -1, y: -1 },
                TerrainChunkId { x: -1, y: 0 },
                TerrainChunkId { x: 0, y: -1 },
                TerrainChunkId { x: 0, y: 0 },
            ]
        );
        for bits in layout.chunks.values() {
            assert_eq!(
                bits.count(),
                144,
                "576 cells evenly split across four chunks"
            );
        }
        assert!(!layout.is_empty());
        assert!(layout.is_occupied((0, 0)));
        assert!(!layout.is_occupied((13, 0)));
        // The exact-center cell block spans [-96, 96]: cells -12..=11 on both axes.
        assert!(layout.is_occupied((-12, -12)) && layout.is_occupied((11, 11)));
        assert!(!layout.is_occupied((-13, 0)) && !layout.is_occupied((12, 12)));
        // Re-resolution is deterministic and produces the identical fingerprint.
        let again = resolve_initial_terrain(
            resolved.snapshot.playable_bounds,
            &resolved.snapshot.geometry,
            &resolved.snapshot.regions,
            &resolved.snapshot.spawn_points,
            &resolved.snapshot.mode_anchors,
            EngineMapLimits::default(),
        )
        .unwrap();
        assert_eq!(again.terrain_fingerprint, layout.terrain_fingerprint);
        assert_eq!(again, layout);
    }
}

fn catalog_resolve(preset: MapPresetId, requirements: &MapLayoutRequirements) -> ResolvedMap {
    MapContentCatalog::embedded()
        .unwrap()
        .resolve_preset(preset, MapInstanceId(9), requirements)
        .unwrap()
}

#[test]
fn circular_and_rotated_regions_rasterize_expected_cells() {
    let bounds = AxisAlignedMapRect {
        min: Vec2::new(-512.0, -360.0),
        max: Vec2::new(512.0, 360.0),
    };
    // Circle of radius 20 centered on a cell center selects cells with centers inside.
    let circle = MapRegionPlacement {
        placement_id: MapPlacementId(1),
        region_id: RegionId(1),
        profile_id: DESTRUCTIBLE_TERRAIN_REGION_PROFILE,
        presentation_profile_id: MapPresentationProfileId(3),
        position: Vec2::new(100.0, 4.0),
        rotation: 0.0,
        shape: MapShape::Circle { radius: 20.0 },
    };
    let layout = resolve_terrain(bounds, &[circle]).unwrap();
    assert_eq!(layout.total_cells, 21, "known small-disc cell count");
    assert!(layout.is_occupied((12, 0)) && layout.is_occupied((11, 2)));

    // A 45-degree thin rectangle selects its diagonal band of cell centers.
    let diagonal = MapRegionPlacement {
        placement_id: MapPlacementId(2),
        region_id: RegionId(1),
        profile_id: DESTRUCTIBLE_TERRAIN_REGION_PROFILE,
        presentation_profile_id: MapPresentationProfileId(3),
        position: Vec2::new(0.0, 0.0),
        rotation: std::f32::consts::FRAC_PI_4,
        shape: MapShape::Rectangle {
            half_extents: Vec2::new(200.0, 4.0),
        },
    };
    let layout = resolve_terrain(bounds, &[diagonal]).unwrap();
    // Only exact-diagonal cell centers satisfy the 8-unit-thick 45-degree band.
    assert_eq!(
        layout.total_cells, 36,
        "centers (4+8k, 4+8k) within |x| <= 141"
    );
    assert!(layout.is_occupied((17, 17)));
    assert!(
        !layout.is_occupied((12, 11)),
        "the thin band excludes side cells"
    );
    // Cells selected by the diagonal live in both the (0,0) chunk and neighbors along the
    // diagonal direction.
    assert!(layout.chunks.contains_key(&TerrainChunkId { x: 0, y: 0 }));
    assert!(layout.chunks.contains_key(&TerrainChunkId { x: -1, y: -1 }));
}

#[test]
fn destructible_validation_rejects_overlaps_geometry_bounds_and_budgets() {
    let bounds = AxisAlignedMapRect {
        min: Vec2::new(-512.0, -360.0),
        max: Vec2::new(512.0, 360.0),
    };
    // Overlapping reservations double-select one cell.
    let overlap = vec![
        region(1, Vec2::new(0.0, 0.0), Vec2::new(96.0, 96.0)),
        region(2, Vec2::new(64.0, 0.0), Vec2::new(96.0, 96.0)),
    ];
    assert!(
        resolve_terrain(bounds, &overlap)
            .unwrap_err()
            .contains("selected by two reservations")
    );

    // A reservation overlapping permanent geometry rejects.
    let geometry = vec![GeometryPlacement {
        placement_id: MapPlacementId(9),
        collision_profile_id: CollisionProfileId(1),
        presentation_profile_id: None,
        position: Vec2::new(0.0, -256.0),
        rotation: 0.0,
        shape: MapShape::Rectangle {
            half_extents: Vec2::new(160.0, 32.0),
        },
    }];
    let on_wall = vec![region(1, Vec2::new(0.0, -256.0), Vec2::new(64.0, 64.0))];
    assert!(
        resolve_initial_terrain(
            bounds,
            &geometry,
            &on_wall,
            &[],
            &[],
            EngineMapLimits::default()
        )
        .unwrap_err()
        .contains("intersects permanent geometry")
    );

    // A cell whose complete AABB leaves the playable bounds rejects even when the authored
    // shape's center is inside.
    let tight_bounds = AxisAlignedMapRect {
        min: Vec2::new(-512.0, -360.0),
        max: Vec2::new(511.0, 360.0),
    };
    let flush = vec![region(1, Vec2::new(507.0, 0.0), Vec2::new(4.0, 4.0))];
    assert!(
        resolve_terrain(tight_bounds, &flush)
            .unwrap_err()
            .contains("leaves the playable bounds")
    );

    // A non-empty reservation that selects no complete cell rejects.
    let dot = vec![MapRegionPlacement {
        placement_id: MapPlacementId(1),
        region_id: RegionId(1),
        profile_id: DESTRUCTIBLE_TERRAIN_REGION_PROFILE,
        presentation_profile_id: MapPresentationProfileId(3),
        position: Vec2::new(104.0, 104.0),
        rotation: 0.0,
        shape: MapShape::Circle { radius: 4.0 },
    }];
    assert!(
        resolve_terrain(bounds, &dot)
            .unwrap_err()
            .contains("selects no complete cell")
    );

    // More reservations than the engine count budget.
    let five: Vec<_> = (0..5)
        .map(|index| {
            region(
                10 + index,
                Vec2::new(-400.0 + 40.0 * index as f32, 0.0),
                Vec2::new(8.0, 8.0),
            )
        })
        .collect();
    assert!(
        resolve_terrain(bounds, &five)
            .unwrap_err()
            .contains("too many destructible terrain reservations")
    );
}

#[test]
fn destructible_validation_rejects_spawn_and_reachability_conflicts() {
    let bounds = AxisAlignedMapRect {
        min: Vec2::new(-512.0, -360.0),
        max: Vec2::new(512.0, 360.0),
    };
    // Spawn circles may not intersect initial occupied cells.
    let spawns = vec![TeamSpawnPoint {
        placement_id: MapPlacementId(2),
        spawn_point_id: SpawnPointId(1),
        team_slot: 0,
        position: Vec2::new(40.0, 0.0),
        facing: 0.0,
    }];
    let central = vec![region(1, Vec2::new(0.0, 0.0), Vec2::new(96.0, 96.0))];
    assert!(
        resolve_initial_terrain(
            bounds,
            &[],
            &central,
            &spawns,
            &[],
            EngineMapLimits::default()
        )
        .unwrap_err()
        .contains("intersects initial destructible terrain")
    );

    // Terrain fully enclosing both team spawns fails reachability.
    let both_spawns = vec![
        TeamSpawnPoint {
            placement_id: MapPlacementId(2),
            spawn_point_id: SpawnPointId(1),
            team_slot: 0,
            position: Vec2::new(-448.0, 0.0),
            facing: 0.0,
        },
        TeamSpawnPoint {
            placement_id: MapPlacementId(3),
            spawn_point_id: SpawnPointId(2),
            team_slot: 1,
            position: Vec2::new(448.0, 0.0),
            facing: std::f32::consts::PI,
        },
    ];
    let wall = vec![region(1, Vec2::new(-24.0, 0.0), Vec2::new(16.0, 340.0))];
    assert!(
        resolve_initial_terrain(
            bounds,
            &[],
            &wall,
            &both_spawns,
            &[],
            EngineMapLimits::default()
        )
        .unwrap_err()
        .contains("reach"),
        "a full-height destructible wall separates the team spawns"
    );
}

#[test]
fn maximum_grid_fixtures_resolve_and_reject_at_the_aggregate_ceilings() {
    // The aligned maximum map fully destructible is exactly at both engine ceilings.
    let aligned = AxisAlignedMapRect {
        min: Vec2::new(0.0, 0.0),
        max: Vec2::new(4096.0, 3072.0),
    };
    let full = vec![region(
        1,
        Vec2::new(2048.0, 1536.0),
        Vec2::new(2048.0, 1536.0),
    )];
    let layout = resolve_terrain(aligned, &full).unwrap();
    assert_eq!(layout.total_cells, crate::terrain::MAX_TERRAIN_CELLS);
    assert_eq!(
        layout.chunks.len(),
        192,
        "aligned 4096x3072 spans 16x12 chunks"
    );

    // The same maximum size at an arbitrary off-grid offset intersects 17x13 = 221 global
    // chunks. Inset by one half cell so every complete cell AABB stays inside bounds; the
    // layout resolves with the full off-grid chunk span, proving allocation does not
    // assume aligned demo coordinates.
    let off_grid = AxisAlignedMapRect {
        min: Vec2::new(-100.0, -100.0),
        max: Vec2::new(3996.0, 2972.0),
    };
    assert_eq!(off_grid.size(), aligned.size());
    let inset = vec![region(
        1,
        Vec2::new(1948.0, 1436.0),
        Vec2::new(2044.0, 1532.0),
    )];
    let layout = resolve_terrain(off_grid, &inset).unwrap();
    assert_eq!(layout.chunks.len(), crate::terrain::MAX_TERRAIN_CHUNKS);
    assert_eq!(layout.total_cells, 195_713);
    assert_eq!(
        layout.chunks.keys().next().copied(),
        Some(TerrainChunkId { x: -1, y: -1 })
    );
    assert_eq!(
        layout.chunks.keys().next_back().copied(),
        Some(TerrainChunkId { x: 15, y: 11 })
    );

    // A flush off-grid reservation selects boundary-center cells whose complete AABB
    // leaves the playable bounds and rejects instead of clipping silently.
    let flush = vec![region(
        1,
        Vec2::new(1948.0, 1436.0),
        Vec2::new(2048.0, 1536.0),
    )];
    assert!(
        resolve_terrain(off_grid, &flush)
            .unwrap_err()
            .contains("leaves the playable bounds")
    );

    // One cell above the cell ceiling rejects (193rd chunk with a single extra cell).
    let over = AxisAlignedMapRect {
        min: Vec2::new(0.0, 0.0),
        max: Vec2::new(4104.0, 3072.0),
    };
    let over_regions = vec![
        region(1, Vec2::new(2048.0, 1536.0), Vec2::new(2048.0, 1536.0)),
        region(2, Vec2::new(4100.0, 4.0), Vec2::new(4.0, 4.0)),
    ];
    let error = resolve_terrain(over, &over_regions).unwrap_err();
    assert!(
        error.contains("cell ceiling"),
        "one cell above the ceiling rejects: {error}"
    );

    // A narrowed chunk ceiling rejects immediately above it (192 chunks > 191 aligned,
    // 221 > 220 off-grid).
    for (bounds, regions, ceiling) in [(aligned, &full, 191_usize), (off_grid, &inset, 220)] {
        let narrowed = EngineMapLimits {
            max_destructible_chunks: ceiling,
            ..EngineMapLimits::default()
        };
        assert!(
            resolve_initial_terrain(bounds, &[], regions, &[], &[], narrowed)
                .unwrap_err()
                .contains("chunk ceiling")
        );
    }

    // A narrowed recovery-byte ceiling rejects the aligned maximum's snapshot.
    let narrowed = EngineMapLimits {
        max_terrain_recovery_bytes: 1_024,
        ..EngineMapLimits::default()
    };
    assert!(
        resolve_initial_terrain(aligned, &[], &full, &[], &[], narrowed)
            .unwrap_err()
            .contains("byte ceiling")
    );
}

#[test]
fn minimum_and_off_grid_playable_sizes_resolve_with_terrain() {
    // Minimum playable size at an arbitrary off-grid offset resolves a small reservation
    // without demo-aligned assumptions.
    let bounds = AxisAlignedMapRect {
        min: Vec2::new(-13.0, -7.0),
        max: Vec2::new(1011.0, 713.0),
    };
    let regions = vec![region(1, Vec2::new(511.0, 511.0), Vec2::new(96.0, 96.0))];
    let layout = resolve_terrain(bounds, &regions).unwrap();
    assert_eq!(layout.total_cells, 576);
    assert_eq!(layout.chunks.len(), 4);
    // The off-grid offset allocates chunks at nonzero global coordinates straddling the
    // 512/256-aligned global chunk boundaries.
    assert_eq!(
        layout.chunks.keys().copied().collect::<Vec<_>>(),
        vec![
            TerrainChunkId { x: 1, y: 1 },
            TerrainChunkId { x: 1, y: 2 },
            TerrainChunkId { x: 2, y: 1 },
            TerrainChunkId { x: 2, y: 2 },
        ]
    );
}

#[test]
fn hot_zone_initial_terrain_keeps_objective_and_spawns_reachable() {
    // Resolution itself is the proof: terrain-aware spawn/objective reachability runs inside
    // resolve_initial_terrain for every accepted snapshot, and the Hot Zone preset places
    // its objective on the central destructible block.
    let resolved = catalog_resolve(HOT_ZONE_MAP_PRESET, &MapLayoutRequirements::hot_zone());
    let layout = resolve_initial_terrain(
        resolved.snapshot.playable_bounds,
        &resolved.snapshot.geometry,
        &resolved.snapshot.regions,
        &resolved.snapshot.spawn_points,
        &resolved.snapshot.mode_anchors,
        EngineMapLimits::default(),
    )
    .unwrap();
    assert_eq!(layout.total_cells, 576, "the Hot Zone center stays solid");
    let anchor = &resolved.snapshot.mode_anchors[0];
    let ModeAnchorShape::Area { position, shape } = anchor.shape else {
        panic!("Hot Zone objective is an area anchor");
    };
    let area = NormalizedArea {
        center: position,
        shape,
    };
    // The objective circle extends past the 192x192 block: legal fighter positions inside
    // the zone exist against the block edge.
    let legal_inside_zone = (-6..=6).any(|x| {
        (-6..=6).any(|y| {
            let point = Vec2::new(120.0 + 32.0 * x as f32, 120.0 + 32.0 * y as f32);
            area.contains_point(point) && !layout.circle_hits(point, 24.0)
        })
    });
    assert!(
        legal_inside_zone,
        "the zone retains legal fighter centers beside the initial block"
    );
}
