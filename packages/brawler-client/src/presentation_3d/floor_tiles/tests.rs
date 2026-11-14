use super::*;
use bevy::mesh::VertexAttributeValues;
use std::collections::BTreeSet;

const TEST_FLOOR: FloorTiles = FloorTiles {
    tone_a: (0.50, 0.40, 0.30),
    tone_b: (0.53, 0.43, 0.32),
    value_jitter: 0.02,
    edge_darkening: 0.25,
};

fn positions(mesh: &Mesh) -> &[[f32; 3]] {
    match mesh.attribute(Mesh::ATTRIBUTE_POSITION) {
        Some(VertexAttributeValues::Float32x3(values)) => values,
        _ => panic!("floor mesh has float3 positions"),
    }
}

fn colors(mesh: &Mesh) -> &[[f32; 4]] {
    match mesh.attribute(Mesh::ATTRIBUTE_COLOR) {
        Some(VertexAttributeValues::Float32x4(values)) => values,
        _ => panic!("floor mesh has float4 colours"),
    }
}

fn value(color: [f32; 4]) -> f32 {
    color[0].max(color[1]).max(color[2])
}

/// Every built-in map's floor quads sit exactly on the authoritative cells.
#[test]
fn tiles_align_exactly_to_authoritative_cells_on_every_built_in_map() {
    let catalog = brawler::map::MapContentCatalog::embedded().unwrap();
    assert!(!catalog.presets.is_empty());
    for preset in &catalog.presets {
        let dimensions = preset.recipe.dimensions;
        let mesh = floor_tile_mesh(dimensions, &BTreeSet::new(), TEST_FLOOR, 0.25);
        let positions = positions(&mesh);
        let cells = usize::from(dimensions.width) * usize::from(dimensions.height);
        assert_eq!(positions.len(), cells * 4);
        assert_eq!(mesh.indices().unwrap().len(), cells * 6);
        for y in 0..dimensions.height {
            for x in 0..dimensions.width {
                let cell = brawler::map::MapCell::new(x, y);
                let index = (usize::from(y) * usize::from(dimensions.width) + usize::from(x)) * 4;
                let quad = &positions[index..index + 4];
                let min = ground_position(dimensions.cell_min(cell)) + Vec3::Y * 0.25;
                let max = ground_position(
                    dimensions.cell_min(cell) + Vec2::splat(brawler::map::MAP_CELL_SIZE_WORLD),
                ) + Vec3::Y * 0.25;
                assert_eq!(Vec3::from(quad[0]), min, "{} cell {x},{y}", preset.key);
                assert_eq!(Vec3::from(quad[2]), max, "{} cell {x},{y}", preset.key);
                let centre = quad.iter().map(|corner| Vec3::from(*corner)).sum::<Vec3>() / 4.0;
                assert_eq!(
                    dimensions.world_cell(ground_point(centre)),
                    Some(cell),
                    "{} cell {x},{y}",
                    preset.key
                );
            }
        }
    }
}

#[test]
fn floor_faces_point_up() {
    let dimensions = brawler::map::MapDimensions {
        width: 2,
        height: 2,
    };
    let mesh = floor_tile_mesh(dimensions, &BTreeSet::new(), TEST_FLOOR, 0.0);
    let positions = positions(&mesh);
    let Some(Indices::U16(indices)) = mesh.indices() else {
        panic!("small floors use u16 indices");
    };
    for triangle in indices.chunks(3) {
        let [a, b, c] = [0, 1, 2].map(|i| Vec3::from(positions[usize::from(triangle[i])]));
        assert!((b - a).cross(c - a).normalize().dot(Vec3::Y) > 0.99);
    }
}

#[test]
fn neighbouring_cells_alternate_between_the_two_close_tones() {
    let dimensions = brawler::map::MapDimensions {
        width: 8,
        height: 8,
    };
    let flat = FloorTiles {
        value_jitter: 0.0,
        edge_darkening: 0.0,
        ..TEST_FLOOR
    };
    let mesh = floor_tile_mesh(dimensions, &BTreeSet::new(), flat, 0.0);
    let colors = colors(&mesh);
    let a = Color::srgb(flat.tone_a.0, flat.tone_a.1, flat.tone_a.2)
        .to_linear()
        .to_f32_array();
    let b = Color::srgb(flat.tone_b.0, flat.tone_b.1, flat.tone_b.2)
        .to_linear()
        .to_f32_array();
    for y in 0..8_usize {
        for x in 0..8_usize {
            let expected = if (x + y) % 2 == 0 { a } else { b };
            for corner in 0..4 {
                let color = colors[(y * 8 + x) * 4 + corner];
                for channel in 0..4 {
                    assert!((color[channel] - expected[channel]).abs() < 1e-5);
                }
            }
        }
    }
}

#[test]
fn jitter_is_deterministic_and_bounded() {
    for x in 0..64 {
        for y in 0..64 {
            let cell = brawler::map::MapCell::new(x, y);
            let jitter = cell_jitter(cell);
            assert!((-1.0..=1.0).contains(&jitter));
            assert!((jitter - cell_jitter(cell)).abs() < f32::EPSILON);
        }
    }
    let distinct: BTreeSet<_> = (0..16)
        .map(|x| cell_jitter(brawler::map::MapCell::new(x, 3)).to_bits())
        .collect();
    assert!(distinct.len() > 8, "jitter must vary per cell");
}

#[test]
fn corners_near_walls_and_the_arena_edge_are_darker() {
    let dimensions = brawler::map::MapDimensions {
        width: 9,
        height: 9,
    };
    let wall = brawler::map::MapCell::new(4, 4);
    let occluders = BTreeSet::from([wall]);
    assert!((corner_darkness(dimensions, &occluders, 0, 3) - 1.0).abs() < f32::EPSILON);
    assert!((corner_darkness(dimensions, &occluders, 4, 4) - 1.0).abs() < f32::EPSILON);
    assert!((corner_darkness(dimensions, &occluders, 3, 3) - 0.5).abs() < f32::EPSILON);
    assert!(corner_darkness(dimensions, &occluders, 2, 2).abs() < f32::EPSILON);
    assert!(corner_darkness(dimensions, &BTreeSet::new(), 4, 4).abs() < f32::EPSILON);

    // Cells (3,4) and (6,7) share tone parity; (6,7)'s corner (7,7) touches a wall at (7,7),
    // so it is darker than every corner of the open cell (3,4).
    let dimensions = brawler::map::MapDimensions {
        width: 15,
        height: 15,
    };
    let occluders = BTreeSet::from([brawler::map::MapCell::new(7, 7)]);
    let flat = FloorTiles {
        value_jitter: 0.0,
        ..TEST_FLOOR
    };
    let mesh = floor_tile_mesh(dimensions, &occluders, flat, 0.0);
    let colors = colors(&mesh);
    let open_cell = (4 * 15 + 3) * 4;
    let beside_wall_corner = (7 * 15 + 6) * 4 + 1;
    for corner in 0..4 {
        assert!(value(colors[beside_wall_corner]) < value(colors[open_cell + corner]));
    }
}

#[test]
fn built_in_wall_occluders_exclude_destructible_cover_and_vegetation() {
    let catalog = brawler::map::MapContentCatalog::embedded().unwrap();
    let mut any = false;
    for preset in &catalog.presets {
        let snapshot = catalog
            .resolve_preset(preset.id, brawler::map::MapInstanceId(1))
            .unwrap()
            .snapshot;
        let occluders = floor_occluder_cells(&snapshot, &catalog);
        for placement in &snapshot.placements {
            let asset = catalog.asset(placement.asset_id).unwrap();
            let profile = catalog.profile(asset.gameplay_profile_id).unwrap();
            if map_profile_has_dynamic_runtime(profile)
                || profile.player_collision == brawler::map::PlayerCollision::Pass
            {
                assert!(!occluders.contains(&placement.cell), "{}", preset.key);
            }
        }
        any |= !occluders.is_empty();
    }
    assert!(any, "built-in maps have static walls");
}

#[test]
fn catalog_floor_validation_enforces_the_art_direction() {
    assert!(TEST_FLOOR.validate("test").is_ok());
    for invalid in [
        FloorTiles {
            tone_b: (0.70, 0.43, 0.32),
            ..TEST_FLOOR
        },
        FloorTiles {
            tone_a: (1.2, 0.4, 0.3),
            ..TEST_FLOOR
        },
        FloorTiles {
            value_jitter: 0.2,
            ..TEST_FLOOR
        },
        FloorTiles {
            edge_darkening: f32::NAN,
            ..TEST_FLOOR
        },
        FloorTiles {
            edge_darkening: 0.9,
            ..TEST_FLOOR
        },
    ] {
        assert!(invalid.validate("test").is_err());
    }
}
