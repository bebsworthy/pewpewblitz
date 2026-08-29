use super::catalog::{
    BARREL_YARD_PRESET, CROSSROADS_PRESET, MAX_MAP_OBJECT_HEALTH, TIDAL_GARDEN_PRESET,
};
use super::geometry::{DerivedColliderShape, circle_overlaps_derived_shape};
use super::resolution::{
    resolve_grid_recipe, validate_and_resolve_mode_anchors, validate_placement_capacity,
};
use super::*;
use bevy::prelude::Vec2;
use std::collections::BTreeSet;

#[test]
fn canonical_fighter_fits_a_one_cell_passage_with_safety_margin() {
    let passage_center = Vec2::new(MAP_CELL_SIZE_WORLD * 0.5, 0.0);
    let left_wall = DerivedColliderShape::Rectangle {
        center: Vec2::new(-MAP_CELL_SIZE_WORLD * 0.5, 0.0),
        half_extents: Vec2::splat(MAP_CELL_SIZE_WORLD * 0.5),
    };
    let right_wall = DerivedColliderShape::Rectangle {
        center: Vec2::new(MAP_CELL_SIZE_WORLD * 1.5, 0.0),
        half_extents: Vec2::splat(MAP_CELL_SIZE_WORLD * 0.5),
    };

    for wall in [left_wall, right_wall] {
        assert!(!circle_overlaps_derived_shape(
            passage_center,
            crate::movement::STANDARD_FIGHTER_RADIUS,
            wall
        ));
        assert!(!circle_overlaps_derived_shape(passage_center, 15.0, wall));
        assert!(
            !circle_overlaps_derived_shape(passage_center, 16.0, wall),
            "an exact-cell body is only tangent and has no movement safety margin"
        );
    }
    assert!(
        (MAP_CELL_SIZE_WORLD - crate::movement::STANDARD_FIGHTER_RADIUS * 2.0 - 4.0).abs()
            < f32::EPSILON
    );
}

#[test]
fn feature_yard_variants_share_geometry_and_own_only_legal_mode_anchors() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let wipeout = &catalog.preset(FEATURE_YARD_WIPEOUT_PRESET).unwrap().recipe;
    let hot_zone = &catalog.preset(FEATURE_YARD_HOT_ZONE_PRESET).unwrap().recipe;
    let heist = &catalog.preset(FEATURE_YARD_HEIST_PRESET).unwrap().recipe;

    for variant in [hot_zone, heist] {
        assert_eq!(variant.presentation_theme_id, wipeout.presentation_theme_id);
        assert_eq!(variant.dimensions, wipeout.dimensions);
        assert_eq!(
            variant.default_surface_asset_id,
            wipeout.default_surface_asset_id
        );
        let wipeout_structural: Vec<_> = wipeout
            .placements
            .iter()
            .filter(|placement| {
                catalog
                    .asset(placement.asset_id)
                    .and_then(|asset| catalog.profile(asset.gameplay_profile_id))
                    .is_some_and(|profile| profile.effect_tile == MapEffectTileBehavior::None)
            })
            .collect();
        assert_eq!(
            variant.placements.iter().collect::<Vec<_>>(),
            wipeout_structural
        );
        assert_eq!(variant.filled_rects, wipeout.filled_rects);
    }
    assert_eq!(wipeout.mode_definition_id, WIPEOUT_MODE_DEFINITION);
    assert!(wipeout.mode_anchors.is_empty());
    let resolved_wipeout = catalog
        .resolve_preset(FEATURE_YARD_WIPEOUT_PRESET, MapInstanceId(1))
        .unwrap();
    assert_eq!(resolved_wipeout.effect_tiles.len(), 92);
    for (kind, expected_count) in [
        (EffectTileKind::Speed, 36),
        (EffectTileKind::Slow, 36),
        (EffectTileKind::Damage, 20),
    ] {
        assert_eq!(
            resolved_wipeout
                .effect_tiles
                .iter()
                .filter(|tile| tile.behavior.kind() == Some(kind))
                .count(),
            expected_count
        );
    }
    for (kind, min_x, min_y) in [
        (EffectTileKind::Speed, 17, 18),
        (EffectTileKind::Speed, 44, 18),
        (EffectTileKind::Slow, 26, 22),
        (EffectTileKind::Slow, 35, 22),
        (EffectTileKind::Speed, 4, 12),
        (EffectTileKind::Speed, 57, 12),
        (EffectTileKind::Slow, 4, 18),
        (EffectTileKind::Slow, 57, 18),
        (EffectTileKind::Damage, 4, 24),
        (EffectTileKind::Damage, 57, 24),
    ] {
        for y in min_y..min_y + 3 {
            for x in min_x..min_x + 3 {
                assert!(resolved_wipeout.effect_tiles.iter().any(|tile| {
                    tile.behavior.kind() == Some(kind) && tile.cell == MapCell::new(x, y)
                }));
            }
        }
    }
    for cell in [MapCell::new(31, 19), MapCell::new(32, 20)] {
        assert!(resolved_wipeout.effect_tiles.iter().any(|tile| {
            tile.behavior.kind() == Some(EffectTileKind::Damage) && tile.cell == cell
        }));
    }
    assert_eq!(hot_zone.mode_definition_id, HOT_ZONE_MODE_DEFINITION);
    assert_eq!(hot_zone.mode_anchors.len(), 1);
    assert!(matches!(
        hot_zone.mode_anchors[0].kind,
        MapModeAnchorKind::HotZoneCircle { .. }
    ));
    assert_eq!(heist.mode_definition_id, HEIST_MODE_DEFINITION);
    assert_eq!(heist.mode_anchors.len(), 2);
    assert!(
        heist
            .mode_anchors
            .iter()
            .all(|anchor| matches!(anchor.kind, MapModeAnchorKind::HeistSafe { .. }))
    );

    for preset in [
        FEATURE_YARD_WIPEOUT_PRESET,
        FEATURE_YARD_HOT_ZONE_PRESET,
        FEATURE_YARD_HEIST_PRESET,
    ] {
        let resolved = catalog.resolve_preset(preset, MapInstanceId(1)).unwrap();
        assert!(
            resolved
                .spawn_points_by_team
                .values()
                .all(|spawns| spawns.len() == 3)
        );
    }
}

#[test]
fn feature_yard_contains_every_completed_map_capability_with_bounded_terminal_states() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let resolved = catalog
        .resolve_preset(FEATURE_YARD_WIPEOUT_PRESET, MapInstanceId(1))
        .unwrap();
    let count = |asset_id| {
        resolved
            .snapshot
            .placements
            .iter()
            .filter(|placement| placement.asset_id == asset_id)
            .count()
    };
    assert_eq!(count(WATER_ASSET), 8);
    assert_eq!(count(TALL_GRASS_ASSET), 8);
    assert_eq!(count(BREAKABLE_BARRIER_ASSET), 4);
    assert_eq!(count(DESTRUCTIBLE_COVER_ASSET), 4);
    assert_eq!(count(OIL_BARREL_ASSET), 4);
    assert_eq!(count(TREASURE_CHEST_ASSET), 2);
    assert_eq!(count(PLAYER_SPAWN_ASSET), 6);
    assert_eq!(resolved.dynamic_placements.len(), 14);
    assert_eq!(
        resolved
            .player_only_surface_rects
            .iter()
            .map(|rectangle| usize::from(rectangle.width) * usize::from(rectangle.height))
            .sum::<usize>(),
        8
    );

    for placement in &resolved.dynamic_placements {
        let asset = catalog.asset(placement.asset_id).unwrap();
        let profile = catalog.profile(asset.gameplay_profile_id).unwrap();
        let terminal_asset = match profile.durability {
            MapDurabilityBehavior::HitPoints(id) => {
                match catalog.damage_profile(id).unwrap().terminal {
                    MapObjectTerminalBehavior::Explode {
                        outcome: MapPlacementOutcome::ReplacedWith(id),
                        ..
                    }
                    | MapObjectTerminalBehavior::DropPickup {
                        outcome: MapPlacementOutcome::ReplacedWith(id),
                        ..
                    } => Some(id),
                    MapObjectTerminalBehavior::Explode {
                        outcome: MapPlacementOutcome::Removed,
                        ..
                    }
                    | MapObjectTerminalBehavior::DropPickup {
                        outcome: MapPlacementOutcome::Removed,
                        ..
                    } => None,
                }
            }
            MapDurabilityBehavior::Indestructible => match profile.destruction {
                MapDestructionBehavior::ReplaceOnMapDestruction(id) => Some(id),
                MapDestructionBehavior::RemoveOnMapDestruction => None,
                MapDestructionBehavior::Indestructible => {
                    panic!("resolved dynamic placement must terminate")
                }
            },
        };
        if let Some(terminal_asset) = terminal_asset {
            let replacement = catalog.asset(terminal_asset).unwrap();
            let replacement_profile = catalog.profile(replacement.gameplay_profile_id).unwrap();
            assert_eq!(replacement_profile.player_collision, PlayerCollision::Pass);
        }
    }
}

#[test]
fn crossroads_grid_resolves_exact_structural_bounds_and_counts() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let resolved = catalog
        .resolve_preset(CROSSROADS_PRESET, MapInstanceId(7))
        .unwrap();
    assert_eq!(
        resolved.snapshot.dimensions,
        MapDimensions {
            width: 56,
            height: 36
        }
    );
    assert_eq!(
        resolved.snapshot.dimensions.bounds().min,
        Vec2::new(-896.0, -576.0)
    );
    assert_eq!(resolved.static_colliders.len(), 6);
    assert_eq!(
        resolved
            .spawn_points_by_team
            .values()
            .map(Vec::len)
            .sum::<usize>(),
        8
    );
    assert_eq!(resolved.dynamic_placements.len(), 36);
    assert_eq!(
        resolved
            .dynamic_placements
            .iter()
            .map(|placement| placement.placement_id.0)
            .collect::<Vec<_>>(),
        (100..=135).collect::<Vec<_>>()
    );
    let wall_bounds: Vec<_> = resolved
        .static_colliders
        .iter()
        .map(|wall| (wall.position, wall.shape))
        .collect();
    assert_eq!(
        wall_bounds,
        vec![
            (
                Vec2::new(0.0, -256.0),
                MapShape::Rectangle {
                    half_extents: Vec2::new(160.0, 32.0)
                }
            ),
            (
                Vec2::new(0.0, 256.0),
                MapShape::Rectangle {
                    half_extents: Vec2::new(160.0, 32.0)
                }
            ),
            (
                Vec2::new(-384.0, 0.0),
                MapShape::Rectangle {
                    half_extents: Vec2::new(32.0, 128.0)
                }
            ),
            (
                Vec2::new(384.0, 0.0),
                MapShape::Rectangle {
                    half_extents: Vec2::new(32.0, 128.0)
                }
            ),
            (
                Vec2::new(-576.0, 0.0),
                MapShape::Rectangle {
                    half_extents: Vec2::new(32.0, 96.0)
                }
            ),
            (
                Vec2::new(576.0, 0.0),
                MapShape::Rectangle {
                    half_extents: Vec2::new(32.0, 96.0)
                }
            ),
        ]
    );
}

#[test]
fn even_grid_uses_cell_centers_for_shifted_points() {
    let dimensions = MapDimensions {
        width: 56,
        height: 36,
    };
    assert_eq!(
        dimensions.cell_center(MapCell::new(4, 9)),
        Vec2::new(-752.0, -272.0)
    );
    assert_eq!(
        dimensions.cell_center(MapCell::new(51, 26)),
        Vec2::new(752.0, 272.0)
    );
}

#[test]
fn half_cell_points_represent_odd_grid_centers_exactly() {
    let dimensions = MapDimensions {
        width: 25,
        height: 37,
    };

    assert_eq!(
        dimensions.half_cell_world(MapHalfCellPoint { x: 25, y: 37 }),
        Some(Vec2::ZERO)
    );
    assert_eq!(
        dimensions.half_cell_world(MapHalfCellPoint { x: 25, y: 37 }),
        Some(dimensions.cell_center(MapCell::new(12, 18)))
    );
    assert_eq!(
        dimensions.half_cell_world(MapHalfCellPoint { x: 50, y: 74 }),
        Some(dimensions.bounds().max)
    );
    assert_eq!(
        dimensions.half_cell_world(MapHalfCellPoint { x: 51, y: 37 }),
        None
    );
}

#[test]
fn hot_zone_anchor_resolves_half_cell_center_and_radius() {
    let anchors = [MapModeAnchorPlacement {
        placement_id: MapPlacementId(1),
        anchor_id: ModeAnchorId(1),
        kind: MapModeAnchorKind::HotZoneCircle {
            center_half_cell: MapHalfCellPoint { x: 25, y: 37 },
            radius_half_cells: 7,
        },
    }];
    let (objective, heist_safes) = validate_and_resolve_mode_anchors(
        HOT_ZONE_MODE_DEFINITION,
        MapDimensions {
            width: 25,
            height: 37,
        },
        &anchors,
        &mut BTreeSet::new(),
    )
    .unwrap();

    assert!(heist_safes.is_empty());
    assert_eq!(
        objective.unwrap().area,
        NormalizedArea {
            center: Vec2::ZERO,
            shape: MapShape::Circle { radius: 112.0 },
        }
    );
}

#[test]
fn proper_three_vs_three_maps_resolve_exact_mode_topology() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let wipeout = catalog
        .resolve_preset(VERDANT_CROSSFIRE_PRESET, MapInstanceId(10))
        .unwrap();
    let hot_zone = catalog
        .resolve_preset(SWITCHBACK_BASIN_PRESET, MapInstanceId(11))
        .unwrap();
    let heist = catalog
        .resolve_preset(POWDERLINE_VAULT_PRESET, MapInstanceId(12))
        .unwrap();

    for resolved in [&wipeout, &hot_zone, &heist] {
        assert_eq!(
            resolved.snapshot.dimensions,
            MapDimensions {
                width: 25,
                height: 37,
            }
        );
        assert_eq!(resolved.spawn_points_by_team.len(), 2);
        assert!(
            resolved
                .spawn_points_by_team
                .values()
                .all(|spawns| spawns.len() == 3)
        );
        assert!(
            resolved
                .snapshot
                .placements
                .iter()
                .any(|placement| placement.asset_id == TALL_GRASS_ASSET)
        );
    }

    assert_eq!(wipeout.snapshot.mode_definition_id, WIPEOUT_MODE_DEFINITION);
    assert_eq!(wipeout.snapshot.identity.recipe_revision, 2);
    assert!(wipeout.objective_zone.is_none());
    assert!(wipeout.heist_safes.is_empty());

    assert_eq!(
        hot_zone.snapshot.mode_definition_id,
        HOT_ZONE_MODE_DEFINITION
    );
    assert_eq!(hot_zone.snapshot.identity.recipe_revision, 2);
    assert_eq!(
        hot_zone.objective_zone.unwrap().area,
        NormalizedArea {
            center: Vec2::ZERO,
            shape: MapShape::Circle { radius: 112.0 },
        }
    );
    assert!(hot_zone.heist_safes.is_empty());

    assert_eq!(heist.snapshot.mode_definition_id, HEIST_MODE_DEFINITION);
    assert_eq!(heist.snapshot.identity.recipe_revision, 3);
    assert!(heist.objective_zone.is_none());
    assert_eq!(heist.heist_safes.len(), 2);
    assert_eq!(
        heist.heist_safes[0].defending_team,
        crate::combat::TeamId(0)
    );
    assert_eq!(
        heist.heist_safes[1].defending_team,
        crate::combat::TeamId(1)
    );
    assert_eq!(heist.heist_safes[0].center, Vec2::new(0.0, -400.0));
    assert_eq!(heist.heist_safes[1].center, Vec2::new(0.0, 400.0));
    let cactus_cells = heist
        .snapshot
        .placements
        .iter()
        .filter(|placement| placement.asset_id == CACTUS_ASSET)
        .map(|placement| placement.cell)
        .collect::<BTreeSet<_>>();
    assert_eq!(
        cactus_cells,
        BTreeSet::from([MapCell::new(9, 20), MapCell::new(15, 17)])
    );
    assert_eq!(
        catalog.asset(CACTUS_ASSET).unwrap().gameplay_profile_id,
        catalog
            .asset(DESTRUCTIBLE_COVER_ASSET)
            .unwrap()
            .gameplay_profile_id
    );
}

#[test]
fn proper_three_vs_three_maps_resolve_the_kaykit_visual_variants() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let wipeout = catalog
        .resolve_preset(VERDANT_CROSSFIRE_PRESET, MapInstanceId(10))
        .unwrap();
    let hot_zone = catalog
        .resolve_preset(SWITCHBACK_BASIN_PRESET, MapInstanceId(11))
        .unwrap();
    let heist = catalog
        .resolve_preset(POWDERLINE_VAULT_PRESET, MapInstanceId(12))
        .unwrap();

    for asset_id in [GREEN_SYMBOL_WALL_ASSET, YELLOW_STRIPED_COVER_ASSET] {
        assert!(
            wipeout
                .snapshot
                .placements
                .iter()
                .any(|placement| placement.asset_id == asset_id)
        );
    }
    for asset_id in [RED_BRICK_WALL_ASSET, GREEN_STRIPED_COVER_ASSET] {
        assert!(
            hot_zone
                .snapshot
                .placements
                .iter()
                .any(|placement| placement.asset_id == asset_id)
        );
    }

    let cells_for_asset = |asset_id| {
        heist
            .snapshot
            .placements
            .iter()
            .filter(|placement| placement.asset_id == asset_id)
            .map(|placement| placement.cell)
            .collect::<BTreeSet<_>>()
    };
    let expected_metal_cells = (18..=25)
        .map(|y| MapCell::new(20, y))
        .chain((12..=18).map(|y| MapCell::new(4, y)))
        .chain((2..=4).map(|x| MapCell::new(x, 22)))
        .chain((20..=22).map(|x| MapCell::new(x, 14)))
        .collect::<BTreeSet<_>>();
    let expected_wood_cells = (22..=26)
        .map(|y| MapCell::new(17, y))
        .chain((10..=14).map(|y| MapCell::new(7, y)))
        .collect::<BTreeSet<_>>();
    assert_eq!(cells_for_asset(METAL_WALL_ASSET), expected_metal_cells);
    assert_eq!(cells_for_asset(WOOD_WALL_ASSET), expected_wood_cells);
    assert!(!cells_for_asset(RED_BRICK_WALL_ASSET).is_empty());
    assert!(!cells_for_asset(YELLOW_STRIPED_COVER_ASSET).is_empty());
    for resolved in [&wipeout, &hot_zone, &heist] {
        assert!(resolved.snapshot.placements.iter().all(|placement| {
            !matches!(
                placement.asset_id,
                GARDEN_WALL_ASSET | DESTRUCTIBLE_COVER_ASSET
            )
        }));
    }
}

#[test]
fn map_dimensions_separate_engine_safety_from_server_policy() {
    let limits = MapDimensionLimits::default();
    assert_eq!(limits.minimum_width, 20);
    assert_eq!(limits.minimum_height, 20);
    assert_eq!(limits.maximum_width, 512);
    assert_eq!(limits.maximum_height, 512);
    assert!(limits.validate().is_ok());
    assert!(
        limits
            .validate_dimensions(MapDimensions {
                width: 20,
                height: 20,
            })
            .is_ok()
    );
    assert!(
        limits
            .validate_dimensions(MapDimensions {
                width: 512,
                height: 512,
            })
            .is_ok()
    );
    assert!(
        limits
            .validate_dimensions(MapDimensions {
                width: 19,
                height: 20,
            })
            .is_err()
    );
    assert!(
        MapDimensions {
            width: 1,
            height: 1,
        }
        .validate()
        .is_ok()
    );
    assert!(
        MapDimensions {
            width: 513,
            height: 512,
        }
        .validate()
        .is_err()
    );
}

#[test]
fn map_dimension_policy_rejects_invalid_operator_envelopes() {
    for limits in [
        MapDimensionLimits {
            minimum_width: 0,
            ..MapDimensionLimits::default()
        },
        MapDimensionLimits {
            minimum_width: 100,
            maximum_width: 99,
            ..MapDimensionLimits::default()
        },
        MapDimensionLimits {
            maximum_height: 513,
            ..MapDimensionLimits::default()
        },
    ] {
        assert!(limits.validate().is_err());
    }
}

#[test]
fn placement_capacity_scales_with_map_cells_and_asset_slots() {
    let dimensions = MapDimensions {
        width: 512,
        height: 512,
    };
    assert_eq!(dimensions.cell_count(), 262_144);
    assert_eq!(dimensions.placement_capacity(), 1_048_576);
    assert!(validate_placement_capacity(dimensions, 1_048_576, 262_144).is_ok());
    assert!(validate_placement_capacity(dimensions, 1_048_577, 262_144).is_err());
    assert!(validate_placement_capacity(dimensions, 262_144, 262_145).is_err());
}

#[test]
fn tidal_garden_resolves_exact_authored_counts_and_mirrored_footprints() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let tall_grass = catalog.asset(TALL_GRASS_ASSET).unwrap();
    let tall_grass_profile = catalog.profile(tall_grass.gameplay_profile_id).unwrap();
    assert_eq!(
        tall_grass_profile.concealment,
        MapConcealmentBehavior::HideOccupants
    );
    assert_ne!(tall_grass.gameplay_profile_id, MapGameplayProfileId(1));
    assert_eq!(
        catalog
            .profile(MapGameplayProfileId(1))
            .unwrap()
            .concealment,
        MapConcealmentBehavior::None
    );
    let resolved = catalog
        .resolve_preset(TIDAL_GARDEN_PRESET, MapInstanceId(8))
        .unwrap();
    assert_eq!(
        resolved.snapshot.dimensions,
        MapDimensions {
            width: 40,
            height: 28,
        }
    );
    let count = |asset_id| {
        resolved
            .snapshot
            .placements
            .iter()
            .filter(|placement| placement.asset_id == asset_id)
            .count()
    };
    assert_eq!(count(WATER_ASSET), 48);
    assert_eq!(count(TALL_GRASS_ASSET), 40);
    assert_eq!(count(GARDEN_WALL_ASSET), 36);
    assert_eq!(count(BREAKABLE_BARRIER_ASSET), 4);
    assert_eq!(count(MapAssetId(15)), 6);
    assert_eq!(count(PLAYER_SPAWN_ASSET), 8);
    assert_eq!(resolved.dynamic_placements.len(), 4);
    assert_eq!(
        resolved
            .player_only_surface_rects
            .iter()
            .map(|rectangle| usize::from(rectangle.width) * usize::from(rectangle.height))
            .sum::<usize>(),
        48
    );

    for asset_id in [
        WATER_ASSET,
        TALL_GRASS_ASSET,
        GARDEN_WALL_ASSET,
        BREAKABLE_BARRIER_ASSET,
    ] {
        let cells: BTreeSet<_> = resolved
            .snapshot
            .placements
            .iter()
            .filter(|placement| placement.asset_id == asset_id)
            .flat_map(|placement| {
                let asset = catalog.asset(asset_id).unwrap();
                placement_cells(resolved.snapshot.dimensions, asset, placement).unwrap()
            })
            .collect();
        let mirrored: BTreeSet<_> = cells
            .iter()
            .map(|cell| MapCell::new(resolved.snapshot.dimensions.width - 1 - cell.x, cell.y))
            .collect();
        assert_eq!(cells, mirrored, "asset {} is not mirrored", asset_id.0);
    }
}

#[test]
fn feature_yard_hot_zone_preserves_exact_objective_and_shared_topology() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let resolved = catalog
        .resolve_preset(FEATURE_YARD_HOT_ZONE_PRESET, MapInstanceId(9))
        .unwrap();
    assert_eq!(
        resolved.snapshot.dimensions,
        MapDimensions {
            width: 64,
            height: 40
        }
    );
    assert_eq!(resolved.static_colliders.len(), 4);
    assert_eq!(resolved.dynamic_placements.len(), 14);
    let grass = resolved
        .snapshot
        .placements
        .iter()
        .filter(|placement| placement.asset_id == TALL_GRASS_ASSET)
        .map(|placement| placement.cell)
        .collect::<BTreeSet<_>>();
    assert_eq!(grass.len(), 8);
    assert_eq!(
        grass
            .iter()
            .map(|cell| MapCell::new(resolved.snapshot.dimensions.width - 1 - cell.x, cell.y))
            .collect::<BTreeSet<_>>(),
        grass
    );
    assert_eq!(resolved.snapshot.mode_anchors.len(), 1);
    let objective = resolved.objective_zone.unwrap();
    assert_eq!(objective.anchor_id, ModeAnchorId(1));
    assert_eq!(
        objective.area,
        NormalizedArea {
            center: Vec2::ZERO,
            shape: MapShape::Circle { radius: 160.0 },
        }
    );
}

#[test]
fn feature_yard_resolves_exact_mirrored_heist_safe_anchors() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let resolved = catalog
        .resolve_preset(FEATURE_YARD_HEIST_PRESET, MapInstanceId(10))
        .unwrap();
    assert_eq!(resolved.snapshot.mode_definition_id, HEIST_MODE_DEFINITION);
    assert!(resolved.objective_zone.is_none());
    assert_eq!(resolved.heist_safes.len(), 2);
    assert_eq!(resolved.heist_safes[0].anchor_id, ModeAnchorId(1));
    assert_eq!(
        resolved.heist_safes[0].defending_team,
        crate::combat::TeamId(0)
    );
    assert_eq!(resolved.heist_safes[1].anchor_id, ModeAnchorId(2));
    assert_eq!(
        resolved.heist_safes[1].defending_team,
        crate::combat::TeamId(1)
    );
    assert_eq!(resolved.heist_safes[0].half_extents, Vec2::new(48.0, 32.0));
    assert!(
        (resolved.heist_safes[0].center.x + resolved.heist_safes[1].center.x).abs() < f32::EPSILON
    );
    assert!(
        (resolved.heist_safes[0].center.y - resolved.heist_safes[1].center.y).abs() < f32::EPSILON
    );
    assert_eq!(resolved.dynamic_placements.len(), 14);
    assert!(
        resolved
            .spawn_points_by_team
            .values()
            .all(|spawns| spawns.len() == 3)
    );
}

#[test]
fn feature_yard_rejects_safe_overlap_wrong_visual_and_sealed_access() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let recipe = catalog
        .presets
        .iter()
        .find(|preset| preset.id == FEATURE_YARD_HEIST_PRESET)
        .unwrap()
        .recipe
        .clone();

    let mut overlap = recipe.clone();
    overlap.placements.push(MapAssetPlacement {
        placement_id: MapPlacementId(900),
        cell: MapCell::new(3, 19),
        asset_id: WALL_ARENA_ASSET,
        quarter_turns: 0,
        parameters: MapPlacementParameters::None,
    });
    assert!(
        resolve_grid_recipe(
            &overlap,
            FEATURE_YARD_HEIST_PRESET,
            MapInstanceId(1),
            &catalog
        )
        .unwrap_err()
        .contains("safe reservation overlaps")
    );

    let mut wrong_visual = recipe.clone();
    let MapModeAnchorKind::HeistSafe {
        ref mut objective_visual_profile_id,
        ..
    } = wrong_visual.mode_anchors[0].kind
    else {
        unreachable!()
    };
    *objective_visual_profile_id = MapVisualProfileId(1);
    assert!(
        resolve_grid_recipe(
            &wrong_visual,
            FEATURE_YARD_HEIST_PRESET,
            MapInstanceId(1),
            &catalog
        )
        .is_err()
    );

    let mut sealed = recipe;
    for (offset, cell) in [
        MapCell::new(1, 19),
        MapCell::new(1, 20),
        MapCell::new(3, 17),
        MapCell::new(4, 17),
        MapCell::new(5, 17),
        MapCell::new(3, 22),
        MapCell::new(4, 22),
        MapCell::new(5, 22),
    ]
    .into_iter()
    .enumerate()
    {
        sealed.placements.push(MapAssetPlacement {
            placement_id: MapPlacementId(910 + u32::try_from(offset).unwrap()),
            cell,
            asset_id: WALL_ARENA_ASSET,
            quarter_turns: 0,
            parameters: MapPlacementParameters::None,
        });
    }
    assert!(
        resolve_grid_recipe(
            &sealed,
            FEATURE_YARD_HEIST_PRESET,
            MapInstanceId(1),
            &catalog
        )
        .unwrap_err()
        .contains("fewer than two legal attack sectors")
    );
}

#[test]
fn converted_ashen_preserves_walls_round_obstacles_and_reviewed_quantization() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let resolved = catalog
        .resolve_preset(ASHEN_COURT_PRESET, MapInstanceId(10))
        .unwrap();
    assert_eq!(
        resolved.snapshot.dimensions,
        MapDimensions {
            width: 48,
            height: 32
        }
    );
    assert_eq!(resolved.static_colliders.len(), 10);
    assert_eq!(
        resolved
            .spawn_points_by_team
            .values()
            .map(Vec::len)
            .sum::<usize>(),
        8
    );
    assert_eq!(resolved.dynamic_placements.len(), 18);
    assert!(resolved.objective_zone.is_none());

    let circles = resolved
        .static_colliders
        .iter()
        .filter_map(|collider| match collider.shape {
            MapShape::Circle { radius } => Some((collider.position, radius)),
            MapShape::Rectangle { .. } => None,
        })
        .collect::<Vec<_>>();
    assert_eq!(
        circles,
        vec![
            (Vec2::new(-128.0, 0.0), 28.0),
            (Vec2::new(128.0, 0.0), 28.0),
            (Vec2::new(0.0, -320.0), 28.0),
            (Vec2::new(0.0, 320.0), 28.0),
        ]
    );
    let spawns = resolved
        .spawn_points_by_team
        .values()
        .flatten()
        .collect::<Vec<_>>();
    assert_eq!(spawns[0].position, Vec2::new(-624.0, -240.0));
    assert_eq!(spawns[3].position, Vec2::new(-624.0, 240.0));
    assert_eq!(spawns[4].position, Vec2::new(624.0, -240.0));
    assert_eq!(spawns[7].position, Vec2::new(624.0, 240.0));

    let cover_cells = resolved
        .dynamic_placements
        .iter()
        .map(|placement| placement.cell)
        .collect::<BTreeSet<_>>();
    assert!(cover_cells.contains(&MapCell::new(17, 2)));
    assert!(cover_cells.contains(&MapCell::new(19, 4)));
    assert!(cover_cells.contains(&MapCell::new(28, 27)));
    assert!(cover_cells.contains(&MapCell::new(30, 29)));
}

#[test]
fn catalog_rejects_false_collider_profiles_and_bad_mode_anchors() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let mut oversized_circle = catalog.clone();
    oversized_circle.gameplay_profiles[6].collider_shape = MapColliderShape::Circle {
        radius_world_units: 33,
    };
    assert!(oversized_circle.validate().is_err());

    let mut inert_collider = catalog.clone();
    inert_collider.gameplay_profiles[0].collider_shape = MapColliderShape::FootprintRectangle;
    assert!(inert_collider.validate().is_err());

    let hot_zone = catalog
        .preset(FEATURE_YARD_HOT_ZONE_PRESET)
        .unwrap()
        .recipe
        .clone();
    let mut missing = hot_zone.clone();
    missing.mode_anchors.clear();
    assert!(
        resolve_grid_recipe(
            &missing,
            FEATURE_YARD_HOT_ZONE_PRESET,
            MapInstanceId(1),
            &catalog
        )
        .is_err()
    );
    let mut outside = hot_zone;
    outside.mode_anchors[0].kind = MapModeAnchorKind::HotZoneCircle {
        center_half_cell: MapHalfCellPoint { x: 0, y: 0 },
        radius_half_cells: 10,
    };
    assert!(
        resolve_grid_recipe(
            &outside,
            FEATURE_YARD_HOT_ZONE_PRESET,
            MapInstanceId(1),
            &catalog
        )
        .is_err()
    );
}

#[test]
fn damageable_profiles_reject_invalid_bounds_references_and_incompatible_behavior() {
    let catalog = MapContentCatalog::embedded().unwrap();

    let barrel = catalog.asset(OIL_BARREL_ASSET).unwrap();
    let damage = match catalog
        .profile(barrel.gameplay_profile_id)
        .unwrap()
        .durability
    {
        MapDurabilityBehavior::HitPoints(id) => catalog.damage_profile(id).unwrap(),
        MapDurabilityBehavior::Indestructible => panic!("oil barrel must be damageable"),
    };
    assert_eq!(
        damage.terminal,
        MapObjectTerminalBehavior::Explode {
            explosion_profile_id: EnvironmentExplosionProfileId(1),
            outcome: MapPlacementOutcome::ReplacedWith(BARREL_WOOD_DEBRIS_ASSET),
        }
    );
    let debris = catalog.asset(BARREL_WOOD_DEBRIS_ASSET).unwrap();
    assert_eq!(barrel.slot, debris.slot);
    assert_eq!(barrel.footprint_cells, debris.footprint_cells);
    assert_eq!(
        catalog.profile(debris.gameplay_profile_id).unwrap(),
        &MapGameplayProfile {
            id: MapGameplayProfileId(1),
            player_collision: PlayerCollision::Pass,
            projectile_collision: ProjectileCollision::Pass,
            collider_shape: MapColliderShape::None,
            destruction: MapDestructionBehavior::Indestructible,
            durability: MapDurabilityBehavior::Indestructible,
            interaction: MapInteractionBehavior::None,
            concealment: MapConcealmentBehavior::None,
            effect_tile: MapEffectTileBehavior::None,
        }
    );

    let mut zero_health = catalog.clone();
    zero_health.damage_profiles[0].maximum_health = 0;
    assert!(zero_health.validate().is_err());

    let mut excessive_health = catalog.clone();
    excessive_health.damage_profiles[0].maximum_health = MAX_MAP_OBJECT_HEALTH + 1;
    assert!(excessive_health.validate().is_err());

    let mut invalid_explosion = catalog.clone();
    invalid_explosion.explosion_profiles[0].maximum_chain_reactions = 17;
    assert!(invalid_explosion.validate().is_err());

    let barrel_profile = catalog
        .gameplay_profiles
        .iter()
        .position(|profile| profile.id == MapGameplayProfileId(9))
        .unwrap();
    let mut destroy_bypass = catalog.clone();
    destroy_bypass.gameplay_profiles[barrel_profile].destruction =
        MapDestructionBehavior::RemoveOnMapDestruction;
    assert!(destroy_bypass.validate().is_err());

    let mut concealed = catalog;
    concealed.gameplay_profiles[barrel_profile].concealment = MapConcealmentBehavior::HideOccupants;
    assert!(concealed.validate().is_err());
}

#[test]
fn effect_tiles_reject_noncanonical_values_and_spawn_hazards() {
    let mut invalid_value = MapContentCatalog::embedded().unwrap();
    invalid_value.gameplay_profiles[10].effect_tile = MapEffectTileBehavior::Speed {
        movement_multiplier_milli: 1_249,
    };
    assert!(invalid_value.validate().is_err());

    let mut unsafe_spawn = MapContentCatalog::embedded().unwrap();
    let preset = unsafe_spawn
        .presets
        .iter_mut()
        .find(|preset| preset.id == FEATURE_YARD_WIPEOUT_PRESET)
        .unwrap();
    let damage_tile = preset
        .recipe
        .placements
        .iter_mut()
        .find(|placement| placement.asset_id == DAMAGE_TILE_ASSET)
        .unwrap();
    damage_tile.cell = MapCell::new(9, 10);
    assert!(
        unsafe_spawn
            .resolve_preset(FEATURE_YARD_WIPEOUT_PRESET, MapInstanceId(1))
            .is_err_and(|error| error.contains("spawn safety"))
    );
}

#[test]
fn barrel_yard_places_one_dungeon_wall_directly_beside_the_reference_barrel() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let resolved = catalog
        .resolve_preset(BARREL_YARD_PRESET, MapInstanceId(1))
        .unwrap();
    let wall = resolved
        .snapshot
        .placements
        .iter()
        .find(|placement| placement.placement_id == MapPlacementId(90))
        .unwrap();
    let barrel = resolved
        .snapshot
        .placements
        .iter()
        .find(|placement| placement.placement_id == MapPlacementId(100))
        .unwrap();

    assert_eq!(wall.asset_id, WALL_DUNGEON_ASSET);
    assert_eq!(barrel.asset_id, OIL_BARREL_ASSET);
    assert_eq!(wall.cell.y, barrel.cell.y);
    assert_eq!(wall.cell.x + 1, barrel.cell.x);
}

#[test]
fn barrier_footprint_rotates_and_replaces_with_matching_rubble() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let barrier = catalog.asset(BREAKABLE_BARRIER_ASSET).unwrap();
    let rubble = catalog.asset(RUBBLE_ASSET).unwrap();
    assert_eq!(
        barrier.footprint_cells,
        MapFootprint {
            width: 2,
            height: 1
        }
    );
    assert_eq!(
        barrier.footprint_cells.rotated(1),
        MapFootprint {
            width: 1,
            height: 2
        }
    );
    assert_eq!(barrier.footprint_cells, rubble.footprint_cells);
    assert_eq!(
        catalog
            .profile(barrier.gameplay_profile_id)
            .unwrap()
            .destruction,
        MapDestructionBehavior::ReplaceOnMapDestruction(RUBBLE_ASSET)
    );
}

#[test]
fn cardinal_adjacency_mask_covers_all_sixteen_neighbor_shapes() {
    let center = MapCell::new(10, 10);
    let neighbors = [
        MapCell::new(10, 11),
        MapCell::new(11, 10),
        MapCell::new(10, 9),
        MapCell::new(9, 10),
    ];
    for expected in 0_u8..16 {
        let occupied = neighbors
            .iter()
            .enumerate()
            .filter_map(|(bit, cell)| (expected & (1 << bit) != 0).then_some(*cell))
            .collect();
        assert_eq!(cardinal_adjacency_mask(center, &occupied), expected);
    }
}

#[test]
fn canonical_grid_fingerprint_ignores_source_placement_order() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let mut shuffled = catalog.clone();
    shuffled.presets[0].recipe.placements.reverse();
    shuffled.presets[0].recipe.filled_rects.reverse();
    let left = catalog
        .resolve_preset(CROSSROADS_PRESET, MapInstanceId(1))
        .unwrap();
    let right = shuffled
        .resolve_preset(CROSSROADS_PRESET, MapInstanceId(1))
        .unwrap();
    assert_eq!(left.snapshot, right.snapshot);
    assert_eq!(
        catalog.canonical_fingerprint_material().unwrap(),
        shuffled.canonical_fingerprint_material().unwrap()
    );
}

#[test]
fn crossroads_wire_payloads_stay_bounded() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let resolved = catalog
        .resolve_preset(CROSSROADS_PRESET, MapInstanceId(1))
        .unwrap();
    let terminal_states: Vec<_> = resolved
        .dynamic_placements
        .iter()
        .map(|placement| MapPlacementTransition {
            placement_id: placement.placement_id,
            outcome: MapPlacementOutcome::Removed,
        })
        .collect();
    let generation = MapDynamicGeneration {
        map_instance_id: MapInstanceId(1),
        generation: 1,
    };
    let event = MapMutationEvent {
        generation,
        revision: 1,
        transitions: terminal_states.clone(),
    };
    let recovery = MapDynamicRecoverySnapshot {
        state: MapDynamicState {
            map_instance_id: MapInstanceId(1),
            generation: 1,
            revision: 1,
            terminal_states,
        },
    };
    let recipe_source_bytes =
        include_str!("../../content/maps/builtin/crossroads-facility.ron").len();
    let snapshot_bytes = postcard::to_allocvec(&resolved.snapshot).unwrap().len();
    let event_bytes = postcard::to_allocvec(&event).unwrap().len();
    let recovery_bytes = postcard::to_allocvec(&recovery).unwrap().len();

    println!(
        "crossroads bytes: recipe={recipe_source_bytes} snapshot={snapshot_bytes} max_event={event_bytes} full_recovery={recovery_bytes}"
    );
    assert!(recipe_source_bytes <= 96 * 1024);
    assert!(snapshot_bytes <= 64 * 1024);
    assert!(event_bytes <= 4 * 1024);
    assert!(recovery_bytes <= 4 * 1024);
}

#[test]
fn tidal_garden_wire_payloads_stay_bounded() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let resolved = catalog
        .resolve_preset(TIDAL_GARDEN_PRESET, MapInstanceId(1))
        .unwrap();
    let transitions = resolved
        .dynamic_placements
        .iter()
        .map(|placement| MapPlacementTransition {
            placement_id: placement.placement_id,
            outcome: MapPlacementOutcome::ReplacedWith(RUBBLE_ASSET),
        })
        .collect::<Vec<_>>();
    let state = MapDynamicState {
        map_instance_id: MapInstanceId(1),
        generation: 1,
        revision: 1,
        terminal_states: transitions.clone(),
    };
    let event = MapMutationEvent {
        generation: MapDynamicGeneration {
            map_instance_id: state.map_instance_id,
            generation: state.generation,
        },
        revision: state.revision,
        transitions,
    };
    let recipe_source_bytes = include_str!("../../content/maps/builtin/tidal-garden.ron").len();
    let snapshot_bytes = postcard::to_allocvec(&resolved.snapshot).unwrap().len();
    let event_bytes = postcard::to_allocvec(&event).unwrap().len();
    let recovery_bytes = postcard::to_allocvec(&MapDynamicRecoverySnapshot { state })
        .unwrap()
        .len();

    println!(
        "tidal garden bytes: recipe={recipe_source_bytes} snapshot={snapshot_bytes} max_event={event_bytes} full_recovery={recovery_bytes}"
    );
    assert!(recipe_source_bytes <= 96 * 1024);
    assert!(snapshot_bytes <= 64 * 1024);
    assert!(event_bytes <= 4 * 1024);
    assert!(recovery_bytes <= 4 * 1024);
}

#[test]
fn converted_map_wire_payloads_stay_bounded() {
    let catalog = MapContentCatalog::embedded().unwrap();
    for (preset_id, recipe_source) in [
        (
            FEATURE_YARD_HOT_ZONE_PRESET,
            include_str!("../../content/maps/builtin/feature-yard-hot-zone.ron"),
        ),
        (
            ASHEN_COURT_PRESET,
            include_str!("../../content/maps/builtin/ashen-court.ron"),
        ),
    ] {
        let resolved = catalog.resolve_preset(preset_id, MapInstanceId(1)).unwrap();
        let terminal_states = resolved
            .dynamic_placements
            .iter()
            .map(|placement| MapPlacementTransition {
                placement_id: placement.placement_id,
                outcome: MapPlacementOutcome::Removed,
            })
            .collect::<Vec<_>>();
        let state = MapDynamicState {
            map_instance_id: MapInstanceId(1),
            generation: 1,
            revision: 1,
            terminal_states: terminal_states.clone(),
        };
        let event = MapMutationEvent {
            generation: MapDynamicGeneration {
                map_instance_id: state.map_instance_id,
                generation: state.generation,
            },
            revision: state.revision,
            transitions: terminal_states,
        };
        let snapshot_bytes = postcard::to_allocvec(&resolved.snapshot).unwrap().len();
        let event_bytes = postcard::to_allocvec(&event).unwrap().len();
        let recovery_bytes = postcard::to_allocvec(&MapDynamicRecoverySnapshot { state })
            .unwrap()
            .len();

        println!(
            "converted preset {} bytes: recipe={} snapshot={snapshot_bytes} max_event={event_bytes} full_recovery={recovery_bytes}",
            preset_id.0,
            recipe_source.len()
        );
        assert!(recipe_source.len() <= 96 * 1024);
        assert!(snapshot_bytes <= 64 * 1024);
        assert!(event_bytes <= 4 * 1024);
        assert!(recovery_bytes <= 4 * 1024);
    }
}

#[test]
fn grid_recipe_rejects_invalid_references_bounds_rotation_and_slot_conflicts() {
    let catalog = MapContentCatalog::embedded().unwrap();
    let base = catalog.presets[0].recipe.clone();
    let mut cases = Vec::new();

    let mut unknown_asset = base.clone();
    unknown_asset.placements[0].asset_id = MapAssetId(u16::MAX);
    cases.push(unknown_asset);

    let mut out_of_bounds = base.clone();
    out_of_bounds.placements[0].cell = MapCell::new(base.dimensions.width, 0);
    cases.push(out_of_bounds);

    let mut bad_rotation = base.clone();
    bad_rotation.placements[0].quarter_turns = 4;
    cases.push(bad_rotation);

    let mut slot_conflict = base.clone();
    let mut duplicate = slot_conflict.placements[0].clone();
    duplicate.placement_id = MapPlacementId(9_999);
    duplicate.asset_id = slot_conflict.placements[1].asset_id;
    slot_conflict.placements.push(duplicate);
    cases.push(slot_conflict);

    let mut bad_schema = base;
    bad_schema.recipe_version = MAP_RECIPE_SCHEMA_VERSION + 1;
    cases.push(bad_schema);

    for recipe in cases {
        assert!(
            resolve_grid_recipe(&recipe, CROSSROADS_PRESET, MapInstanceId(1), &catalog).is_err()
        );
    }
}
