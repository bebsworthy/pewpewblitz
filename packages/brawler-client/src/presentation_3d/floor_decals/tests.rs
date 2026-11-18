use super::*;
use crate::presentation_3d::environment_assets;
use bevy::mesh::VertexAttributeValues;

fn shared_catalog() -> brawler::map::MapContentCatalog {
    brawler::map::MapContentCatalog::embedded().unwrap()
}

fn visuals(catalog: &brawler::map::MapContentCatalog) -> environment_assets::MapVisualCatalog {
    environment_assets::MapVisualCatalog::embedded(catalog).unwrap()
}

/// Every built-in map, resolved exactly as the client presents it.
fn built_in_snapshots(
    catalog: &brawler::map::MapContentCatalog,
) -> Vec<(String, brawler::map::ResolvedMapSnapshot)> {
    catalog
        .presets
        .iter()
        .map(|preset| {
            let snapshot = catalog
                .resolve_preset(preset.id, brawler::map::MapInstanceId(1))
                .unwrap_or_else(|error| panic!("{} resolves: {error}", preset.key))
                .snapshot;
            (preset.key.clone(), snapshot)
        })
        .collect()
}

fn theme_decals(
    visuals: &environment_assets::MapVisualCatalog,
    snapshot: &brawler::map::ResolvedMapSnapshot,
) -> FloorDecals {
    visuals
        .theme(snapshot.presentation_theme_id)
        .unwrap()
        .floor_decals
        .clone()
}

fn place(
    catalog: &brawler::map::MapContentCatalog,
    snapshot: &brawler::map::ResolvedMapSnapshot,
    decals: &FloorDecals,
) -> Vec<PlacedFloorDecal> {
    let occluders = floor_tiles::floor_occluder_cells(snapshot, catalog);
    place_floor_decals(snapshot, catalog, &occluders, decals)
}

/// Placement is deterministic per map, sparse, wall-adjacent, and never on a placement, an
/// effect tile, water, a spawn, the Hot Zone, a Heist safe, or a Zombie ingress.
#[test]
fn decals_are_deterministic_wall_adjacent_and_avoid_surfaces_and_objectives() {
    let catalog = shared_catalog();
    let visuals = visuals(&catalog);
    let snapshots = built_in_snapshots(&catalog);
    assert!(!snapshots.is_empty());
    for (key, snapshot) in &snapshots {
        let decals = theme_decals(&visuals, snapshot);
        let placed = place(&catalog, snapshot, &decals);
        assert_eq!(placed, place(&catalog, snapshot, &decals), "{key}");
        assert!(!placed.is_empty(), "{key} has no decals");
        assert!(placed.len() <= MAX_FLOOR_DECALS, "{key}");
        let occluders = floor_tiles::floor_occluder_cells(snapshot, &catalog);
        let excluded = excluded_decal_cells(snapshot, &catalog);
        let placement_cells: BTreeSet<_> = snapshot
            .placements
            .iter()
            .filter_map(|placement| {
                brawler::map::placement_cells(
                    snapshot.dimensions,
                    catalog.asset(placement.asset_id)?,
                    placement,
                )
            })
            .flatten()
            .collect();
        let hot_zone = hot_zone_visual_geometry(snapshot);
        for decal in &placed {
            let cell = decal.cell;
            assert!(snapshot.dimensions.contains(cell), "{key}");
            assert!(
                !placement_cells.contains(&cell),
                "{key} decal on a placement {cell:?}"
            );
            assert!(!occluders.contains(&cell), "{key}");
            assert!(!excluded.contains(&cell), "{key}");
            // The wall the sprite faces really is a wall or the arena edge.
            let neighbour = cell_offset(snapshot.dimensions, cell, decal.wall.offset());
            assert!(
                neighbour.is_none_or(|neighbour| occluders.contains(&neighbour)),
                "{key} decal {cell:?} faces open floor"
            );
            if let Some((center, radius)) = hot_zone {
                let min = snapshot.dimensions.cell_min(cell);
                let max = min + Vec2::splat(brawler::map::MAP_CELL_SIZE_WORLD);
                assert!(
                    center.clamp(min, max).distance(center) >= radius,
                    "{key} decal {cell:?} inside the Hot Zone"
                );
            }
            // Every corner stays inside the decal's own cell.
            let min = snapshot.dimensions.cell_min(cell);
            let max = min + Vec2::splat(brawler::map::MAP_CELL_SIZE_WORLD);
            for corner in decal_corners(snapshot.dimensions, decal) {
                assert!(
                    corner.cmpge(min - 1e-3).all() && corner.cmple(max + 1e-3).all(),
                    "{key} decal {cell:?} leaves its cell"
                );
            }
            assert!(decal.opacity > 0.0 && decal.opacity <= MAX_DECAL_OPACITY);
        }
        // Sparse: decals never touch each other.
        for (index, a) in placed.iter().enumerate() {
            for b in &placed[index + 1..] {
                let dx = a.cell.x.abs_diff(b.cell.x);
                let dy = a.cell.y.abs_diff(b.cell.y);
                assert!(dx.max(dy) >= MIN_DECAL_SPACING_CELLS, "{key} {a:?} {b:?}");
            }
        }
    }
}

/// Spawns, Heist safes (with their attack sectors), and the Hot Zone are excluded where the
/// built-in maps have them.
#[test]
fn objective_and_spawn_cells_are_excluded() {
    let catalog = shared_catalog();
    let mut saw_heist = false;
    let mut saw_hot_zone = false;
    for (key, snapshot) in built_in_snapshots(&catalog) {
        let excluded = excluded_decal_cells(&snapshot, &catalog);
        for placement in &snapshot.placements {
            assert!(
                excluded.contains(&placement.cell),
                "{key} {:?}",
                placement.cell
            );
        }
        for anchor in &snapshot.mode_anchors {
            match anchor.kind {
                brawler::map::MapModeAnchorKind::HeistSafe { origin_cell, .. } => {
                    saw_heist = true;
                    let sector = brawler::map::MapCell::new(origin_cell.x - 2, origin_cell.y);
                    assert!(excluded.contains(&origin_cell), "{key}");
                    assert!(excluded.contains(&sector), "{key}");
                }
                brawler::map::MapModeAnchorKind::HotZoneCircle {
                    center_half_cell, ..
                } => {
                    saw_hot_zone = true;
                    let center = snapshot
                        .dimensions
                        .half_cell_world(center_half_cell)
                        .and_then(|point| snapshot.dimensions.world_cell(point))
                        .unwrap();
                    assert!(excluded.contains(&center), "{key}");
                }
                brawler::map::MapModeAnchorKind::ZombieIngress { .. } => {}
            }
        }
    }
    assert!(saw_heist && saw_hot_zone);
}

#[test]
fn placement_depends_on_the_map_seed() {
    let catalog = shared_catalog();
    let visuals = visuals(&catalog);
    let (_, snapshot) = built_in_snapshots(&catalog)
        .into_iter()
        .max_by_key(|(_, snapshot)| snapshot.dimensions.width)
        .unwrap();
    let decals = theme_decals(&visuals, &snapshot);
    let mut reseeded = snapshot.clone();
    reseeded.identity.recipe_id.0 ^= 0x5A5A_1234;
    assert_ne!(
        place(&catalog, &snapshot, &decals),
        place(&catalog, &reseeded, &decals)
    );
    // The instance (per match) does not change the decals.
    let mut other_instance = snapshot.clone();
    other_instance.identity.instance_id = brawler::map::MapInstanceId(99);
    assert_eq!(
        place(&catalog, &snapshot, &decals),
        place(&catalog, &other_instance, &decals)
    );
}

#[test]
fn density_zero_places_nothing_and_the_cap_bounds_dense_sets() {
    let catalog = shared_catalog();
    let visuals = visuals(&catalog);
    for (key, snapshot) in built_in_snapshots(&catalog) {
        let mut decals = theme_decals(&visuals, &snapshot);
        decals.density = 0.0;
        assert!(place(&catalog, &snapshot, &decals).is_empty(), "{key}");
        decals.density = 1.0;
        assert!(place(&catalog, &snapshot, &decals).len() <= MAX_FLOOR_DECALS);
    }
}

#[test]
fn decal_mesh_faces_up_and_samples_one_atlas_cell_each() {
    let dimensions = brawler::map::MapDimensions {
        width: 4,
        height: 4,
    };
    let decals: Vec<_> = WallSide::ALL
        .into_iter()
        .zip([false, true, false, true])
        .enumerate()
        .map(|(index, (wall, mirrored))| PlacedFloorDecal {
            cell: brawler::map::MapCell::new(u16::try_from(index).unwrap(), 1),
            sprite: [
                FloorDecalSprite::Crack,
                FloorDecalSprite::Pebbles,
                FloorDecalSprite::SandDrift,
                FloorDecalSprite::Leaves,
            ][index],
            wall,
            mirrored,
            scale: 0.9,
            slide: 0.5,
            tint: (0.3, 0.2, 0.1),
            opacity: 0.5,
        })
        .collect();
    assert!(floor_decal_mesh(dimensions, &[], 0.4).is_none());
    let mesh = floor_decal_mesh(dimensions, &decals, 0.4).unwrap();
    let Some(VertexAttributeValues::Float32x3(positions)) =
        mesh.attribute(Mesh::ATTRIBUTE_POSITION)
    else {
        panic!("positions");
    };
    let Some(VertexAttributeValues::Float32x2(uvs)) = mesh.attribute(Mesh::ATTRIBUTE_UV_0) else {
        panic!("uvs");
    };
    let Some(Indices::U16(indices)) = mesh.indices() else {
        panic!("u16 indices");
    };
    assert_eq!(positions.len(), decals.len() * 4);
    for triangle in indices.chunks(3) {
        let [a, b, c] = [0, 1, 2].map(|i| Vec3::from(positions[usize::from(triangle[i])]));
        assert!((b - a).cross(c - a).normalize().dot(Vec3::Y) > 0.99);
        assert!((a.y - 0.4).abs() < f32::EPSILON);
    }
    for (decal, quad) in decals.iter().zip(uvs.chunks(4)) {
        let index = decal.sprite.atlas_index();
        let column = f32::from(index % ATLAS_COLUMNS) / f32::from(ATLAS_COLUMNS);
        let row = f32::from(index / ATLAS_COLUMNS) / f32::from(ATLAS_ROWS);
        for uv in quad {
            assert!(uv[0] >= column && uv[0] <= column + 1.0 / f32::from(ATLAS_COLUMNS));
            assert!(uv[1] >= row && uv[1] <= row + 1.0 / f32::from(ATLAS_ROWS));
        }
    }
    // The sprite's wall edge (v = 0) lies on the wall side of the cell.
    for decal in &decals {
        let corners = decal_corners(dimensions, decal);
        let wall_edge = (corners[0] + corners[1]) * 0.5;
        let far_edge = (corners[2] + corners[3]) * 0.5;
        assert!((wall_edge - far_edge).dot(decal.wall.direction()) > 0.0);
    }
}

#[test]
fn catalog_decals_are_valid_and_validation_rejects_loud_sets() {
    let catalog = shared_catalog();
    let visuals = visuals(&catalog);
    for theme in visuals.themes.values() {
        theme.floor_decals.validate("theme", theme.floor).unwrap();
        let mut bright = theme.floor_decals.clone();
        bright.kinds[0].tint = (1.0, 1.0, 1.0);
        assert!(bright.validate("theme", theme.floor).is_err());
        let mut opaque = theme.floor_decals.clone();
        opaque.opacity = 0.9;
        assert!(opaque.validate("theme", theme.floor).is_err());
        let mut dense = theme.floor_decals.clone();
        dense.density = 0.8;
        assert!(dense.validate("theme", theme.floor).is_err());
        let mut duplicate = theme.floor_decals.clone();
        duplicate.kinds.push(duplicate.kinds[0]);
        assert!(duplicate.validate("theme", theme.floor).is_err());
        let mut weightless = theme.floor_decals.clone();
        weightless.kinds[0].weight = 0;
        assert!(weightless.validate("theme", theme.floor).is_err());
    }
}

/// The atlas the script generates decodes through Bevy's KTX2 loader path (zstd supercompression,
/// sRGB RGBA8) with the code-owned layout and its pre-filtered mip chain (BRL-dq06), and the
/// sampler the client loads it with filters across those mips.
#[test]
fn generated_atlas_ships_mipmapped_with_the_expected_layout() {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets")
        .join(FLOOR_DECAL_ATLAS_PATH);
    let bytes = std::fs::read(&path).unwrap();
    let image = Image::from_buffer(
        &bytes,
        bevy::image::ImageType::Extension("ktx2"),
        bevy::image::CompressedImageFormats::NONE,
        true,
        floor_decal_atlas_sampler(),
        RenderAssetUsages::default(),
    )
    .unwrap();
    assert_eq!(image.width(), 128 * u32::from(ATLAS_COLUMNS));
    assert_eq!(image.height(), 128 * u32::from(ATLAS_ROWS));
    assert_eq!(
        image.texture_descriptor.format,
        bevy::render::render_resource::TextureFormat::Rgba8UnormSrgb
    );
    // 128 px cells down to 8 px cells: enough to stay stable at any gameplay zoom, few enough
    // that no level averages two sprites together.
    assert_eq!(image.texture_descriptor.mip_level_count, 5);
    let texels: usize = (0..5)
        .map(|level| (384 >> level) * (256 >> level) * 4)
        .sum();
    assert_eq!(image.data.as_ref().map(Vec::len), Some(texels));
    let bevy::image::ImageSampler::Descriptor(sampler) = &image.sampler else {
        panic!("the decal atlas needs an explicit sampler");
    };
    assert_eq!(sampler.mipmap_filter, bevy::image::ImageFilterMode::Linear);
    assert_eq!(sampler.min_filter, bevy::image::ImageFilterMode::Linear);
    assert_eq!(sampler.mag_filter, bevy::image::ImageFilterMode::Linear);
    assert!(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("../../asset_src/environment/floor-decals/build_decals.py")
            .is_file()
    );
}

/// The decal material is lit (never unlit/HDR), has no emissive, blends, and sorts under every
/// other transparent layer.
#[test]
fn decal_material_is_lit_blended_and_below_other_layers() {
    let material = floor_decal_material(Handle::default());
    assert!(!material.unlit);
    assert_eq!(material.emissive, LinearRgba::BLACK);
    assert_eq!(material.alpha_mode, AlphaMode::Blend);
    assert!(material.depth_bias < 0.0);
    assert_eq!(material.base_color, Color::WHITE);
}

#[test]
fn low_tier_hides_decals_and_absent_quality_shows_them() {
    let mut app = App::new();
    app.add_systems(Update, apply_floor_decal_quality);
    let layer = app
        .world_mut()
        .spawn((FloorDecalLayer, Visibility::Hidden))
        .id();
    app.update();
    assert_eq!(
        app.world().get::<Visibility>(layer),
        Some(&Visibility::Inherited)
    );
    let catalog = render_quality::RenderQualityCatalog::embedded().unwrap();
    assert!(
        !catalog
            .profile(render_quality::QualityTier::Low)
            .floor_decals
    );
    assert!(
        catalog
            .profile(render_quality::QualityTier::Medium)
            .floor_decals
    );
    assert!(
        catalog
            .profile(render_quality::QualityTier::High)
            .floor_decals
    );
}
