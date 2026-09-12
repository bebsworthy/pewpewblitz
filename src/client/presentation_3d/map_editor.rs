//! Local-only 3D draft preview and editor overlays.

use super::*;

const EDITOR_INSTANCE_PREFIX: u64 = 0xFFFF_0000_0000_0000;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(in crate::client) enum MapEditorPointerMode {
    Place,
    Erase,
    Move,
}

pub(in crate::client) struct MapEditorPreviewOptions {
    pub(in crate::client) cursor: crate::map::MapCell,
    pub(in crate::client) grid_visible: bool,
    pub(in crate::client) camera_center: Vec2,
    pub(in crate::client) camera_zoom: f32,
    pub(in crate::client) symmetry: crate::map::MapSymmetry,
    pub(in crate::client) pointer_mode: MapEditorPointerMode,
    pub(in crate::client) pointer_valid: bool,
    pub(in crate::client) selected: Vec<crate::map::MapPlacementId>,
    pub(in crate::client) hovered: Option<crate::map::MapPlacementId>,
    pub(in crate::client) selection_rect: Option<(crate::map::MapCell, crate::map::MapCell)>,
}

#[derive(Resource, Clone, Debug, PartialEq)]
pub(in crate::client) struct MapEditorPreview {
    pub(in crate::client) snapshot: crate::map::ResolvedMapSnapshot,
    pub(in crate::client) cursor: crate::map::MapCell,
    pub(in crate::client) grid_visible: bool,
    pub(in crate::client) camera_center: Vec2,
    pub(in crate::client) camera_zoom: f32,
    pub(in crate::client) symmetry: crate::map::MapSymmetry,
    pub(in crate::client) pointer_mode: MapEditorPointerMode,
    pub(in crate::client) pointer_valid: bool,
    pub(in crate::client) selected: Vec<crate::map::MapPlacementId>,
    pub(in crate::client) hovered: Option<crate::map::MapPlacementId>,
    pub(in crate::client) selection_rect: Option<(crate::map::MapCell, crate::map::MapCell)>,
}

impl MapEditorPreview {
    pub(in crate::client) fn from_recipe(
        draft_id: MapDraftId,
        recipe: &crate::map::MapRecipe,
        catalog: &crate::map::MapContentCatalog,
        options: MapEditorPreviewOptions,
    ) -> Self {
        let instance_id = crate::map::MapInstanceId(
            EDITOR_INSTANCE_PREFIX | (draft_id.0 & !EDITOR_INSTANCE_PREFIX),
        );
        let snapshot = catalog.resolve_recipe(recipe, instance_id).map_or_else(
            |_| preview_snapshot(recipe, catalog, instance_id),
            |resolved| resolved.snapshot,
        );
        Self {
            snapshot,
            cursor: options.cursor,
            grid_visible: options.grid_visible,
            camera_center: options.camera_center,
            camera_zoom: options.camera_zoom,
            symmetry: options.symmetry,
            pointer_mode: options.pointer_mode,
            pointer_valid: options.pointer_valid,
            selected: options.selected,
            hovered: options.hovered,
            selection_rect: options.selection_rect,
        }
    }
}

fn preview_snapshot(
    recipe: &crate::map::MapRecipe,
    catalog: &crate::map::MapContentCatalog,
    instance_id: crate::map::MapInstanceId,
) -> crate::map::ResolvedMapSnapshot {
    let fingerprint =
        postcard::to_allocvec(recipe).map_or(0, |bytes| crate::content::fnv1a64(&bytes));
    crate::map::ResolvedMapSnapshot {
        identity: crate::map::ResolvedMapIdentity {
            instance_id,
            source_preset_id: None,
            recipe_id: recipe.recipe_id,
            recipe_revision: recipe.revision,
            recipe_fingerprint: crate::map::MapRecipeFingerprint(fingerprint),
        },
        catalog_schema_version: catalog.schema_version,
        recipe_schema_version: recipe.recipe_version,
        presentation_theme_id: recipe.presentation_theme_id,
        mode_definition_id: recipe.mode_definition_id,
        dimensions: recipe.dimensions,
        default_surface_asset_id: recipe.default_surface_asset_id,
        placements: recipe.placements.clone(),
        mode_anchors: recipe.mode_anchors.clone(),
    }
}

#[derive(Component)]
pub(super) struct MapEditorOverlay;

#[derive(Component)]
pub(super) struct MapEditorCursor;

#[derive(Resource)]
pub(super) struct MapEditorOverlayMaterials {
    grid_minor: Handle<StandardMaterial>,
    grid_major: Handle<StandardMaterial>,
    place: Handle<StandardMaterial>,
    erase: Handle<StandardMaterial>,
    move_selection: Handle<StandardMaterial>,
    selection_fill: Handle<StandardMaterial>,
    hover: Handle<StandardMaterial>,
    blocked: Handle<StandardMaterial>,
    symmetry: Handle<StandardMaterial>,
}

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PresentedMapEditorOverlay {
    instance_id: crate::map::MapInstanceId,
    recipe_revision: u32,
    grid_visible: bool,
    symmetry: crate::map::MapSymmetry,
    pointer_mode: MapEditorPointerMode,
    pointer_valid: bool,
    cursor: crate::map::MapCell,
    selection_hash: u64,
    hovered: Option<crate::map::MapPlacementId>,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    clippy::too_many_lines,
    reason = "the overlay reconciler consumes distinct Bevy-owned rendering resources and queries"
)]
pub(super) fn reconcile_map_editor_overlay(
    mut commands: Commands,
    preview: Option<Res<MapEditorPreview>>,
    catalog: Res<crate::map::MapCatalogResource>,
    primitives: Res<Primitive3dAssets>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    overlay_materials: Option<Res<MapEditorOverlayMaterials>>,
    presented: Option<Res<PresentedMapEditorOverlay>>,
    overlays: Query<Entity, With<MapEditorOverlay>>,
) {
    let Some(preview) = preview else {
        for entity in &overlays {
            commands.entity(entity).try_despawn();
        }
        commands.remove_resource::<PresentedMapEditorOverlay>();
        return;
    };
    let Some(materials) = overlay_materials else {
        commands.insert_resource(MapEditorOverlayMaterials {
            grid_minor: standard_materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 0.96, 0.80, 0.12),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
            grid_major: standard_materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 0.96, 0.80, 0.30),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
            place: standard_materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 0.42, 0.04, 0.34),
                emissive: LinearRgba::new(1.4, 0.28, 0.01, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
            erase: standard_materials.add(StandardMaterial {
                base_color: Color::srgba(0.95, 0.08, 0.06, 0.38),
                emissive: LinearRgba::new(1.2, 0.02, 0.01, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
            move_selection: standard_materials.add(StandardMaterial {
                base_color: Color::srgba(0.01, 0.96, 0.90, 0.82),
                emissive: LinearRgba::new(0.01, 2.2, 1.9, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
            selection_fill: standard_materials.add(StandardMaterial {
                base_color: Color::srgba(0.02, 0.88, 0.82, 0.14),
                emissive: LinearRgba::new(0.01, 0.22, 0.20, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
            hover: standard_materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 0.58, 0.04, 0.42),
                emissive: LinearRgba::new(1.1, 0.34, 0.01, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
            blocked: standard_materials.add(StandardMaterial {
                base_color: Color::srgba(0.95, 0.08, 0.06, 0.5),
                emissive: LinearRgba::new(1.2, 0.02, 0.01, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
            symmetry: standard_materials.add(StandardMaterial {
                base_color: Color::srgba(0.1, 0.9, 0.85, 0.72),
                emissive: LinearRgba::new(0.02, 0.8, 0.7, 1.0),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
        });
        return;
    };

    let desired = PresentedMapEditorOverlay {
        instance_id: preview.snapshot.identity.instance_id,
        recipe_revision: preview.snapshot.identity.recipe_revision,
        grid_visible: preview.grid_visible,
        symmetry: preview.symmetry,
        pointer_mode: preview.pointer_mode,
        pointer_valid: preview.pointer_valid,
        cursor: preview.cursor,
        selection_hash: selection_hash(&preview),
        hovered: preview.hovered,
    };
    if presented.as_deref() != Some(&desired) {
        for entity in &overlays {
            commands.entity(entity).try_despawn();
        }
        if preview.grid_visible {
            spawn_grid(&mut commands, &primitives, &materials, &preview.snapshot);
        }
        spawn_symmetry_guides(&mut commands, &primitives, &materials, &preview);
        spawn_selection(&mut commands, &primitives, &materials, &preview, &catalog.0);
        spawn_hover(&mut commands, &primitives, &materials, &preview, &catalog.0);
        let cursor_material = if preview.pointer_valid {
            match preview.pointer_mode {
                MapEditorPointerMode::Place => materials.place.clone(),
                MapEditorPointerMode::Erase => materials.erase.clone(),
                MapEditorPointerMode::Move => materials.move_selection.clone(),
            }
        } else {
            materials.blocked.clone()
        };
        spawn_pointer_marker(&mut commands, &primitives, &cursor_material, &preview);
        commands.insert_resource(desired);
    }
}

fn spawn_pointer_marker(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    material: &Handle<StandardMaterial>,
    preview: &MapEditorPreview,
) {
    let center =
        ground_position(preview.snapshot.dimensions.cell_center(preview.cursor)) + Vec3::Y * 1.7;
    let cell = crate::map::MAP_CELL_SIZE_WORLD;
    let cross = !preview.pointer_valid || preview.pointer_mode == MapEditorPointerMode::Erase;
    if cross {
        for angle in [core::f32::consts::FRAC_PI_4, -core::f32::consts::FRAC_PI_4] {
            commands.spawn((
                MapEditorOverlay,
                MapEditorCursor,
                Mesh3d(primitives.unit_cuboid.clone()),
                MeshMaterial3d(material.clone()),
                NotShadowCaster,
                Transform {
                    translation: center,
                    rotation: Quat::from_rotation_y(angle),
                    scale: Vec3::new(cell * 0.72, 0.9, 2.8),
                },
                Name::new(if preview.pointer_valid {
                    "Map Maker erase marker"
                } else {
                    "Map Maker invalid placement marker"
                }),
            ));
        }
        return;
    }
    let half = cell * 0.43;
    for (offset, scale) in [
        (Vec3::new(0.0, 0.0, half), Vec3::new(cell * 0.86, 0.8, 2.2)),
        (Vec3::new(0.0, 0.0, -half), Vec3::new(cell * 0.86, 0.8, 2.2)),
        (Vec3::new(half, 0.0, 0.0), Vec3::new(2.2, 0.8, cell * 0.86)),
        (Vec3::new(-half, 0.0, 0.0), Vec3::new(2.2, 0.8, cell * 0.86)),
    ] {
        commands.spawn((
            MapEditorOverlay,
            MapEditorCursor,
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(material.clone()),
            NotShadowCaster,
            Transform {
                translation: center + offset,
                scale,
                ..default()
            },
            Name::new(match preview.pointer_mode {
                MapEditorPointerMode::Place => "Map Maker placement footprint",
                MapEditorPointerMode::Move => "Map Maker move target",
                MapEditorPointerMode::Erase => "Map Maker erase target",
            }),
        ));
    }
}

fn spawn_grid(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &MapEditorOverlayMaterials,
    snapshot: &crate::map::ResolvedMapSnapshot,
) {
    let bounds = snapshot.dimensions.bounds();
    let size = bounds.size();
    for x in 0..=snapshot.dimensions.width {
        let world_x = bounds.min.x + f32::from(x) * crate::map::MAP_CELL_SIZE_WORLD;
        let major = x.is_multiple_of(5);
        commands.spawn((
            MapEditorOverlay,
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(if major {
                materials.grid_major.clone()
            } else {
                materials.grid_minor.clone()
            }),
            NotShadowCaster,
            Transform {
                translation: ground_position(Vec2::new(world_x, bounds.center().y)) + Vec3::Y * 1.1,
                scale: Vec3::new(if major { 0.9 } else { 0.4 }, 0.4, size.y),
                ..default()
            },
            Name::new("Map Maker grid column"),
        ));
    }
    for y in 0..=snapshot.dimensions.height {
        let world_y = bounds.min.y + f32::from(y) * crate::map::MAP_CELL_SIZE_WORLD;
        let major = y.is_multiple_of(5);
        commands.spawn((
            MapEditorOverlay,
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(if major {
                materials.grid_major.clone()
            } else {
                materials.grid_minor.clone()
            }),
            NotShadowCaster,
            Transform {
                translation: ground_position(Vec2::new(bounds.center().x, world_y)) + Vec3::Y * 1.1,
                scale: Vec3::new(size.x, 0.4, if major { 0.9 } else { 0.4 }),
                ..default()
            },
            Name::new("Map Maker grid row"),
        ));
    }
}

fn selection_hash(preview: &MapEditorPreview) -> u64 {
    let mut hash = preview
        .selected
        .iter()
        .fold(0_u64, |hash, id| hash.rotate_left(7) ^ u64::from(id.0));
    if let Some((first, second)) = preview.selection_rect {
        hash ^= u64::from(first.x)
            | (u64::from(first.y) << 16)
            | (u64::from(second.x) << 32)
            | (u64::from(second.y) << 48);
    }
    hash
}

fn spawn_symmetry_guides(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &MapEditorOverlayMaterials,
    preview: &MapEditorPreview,
) {
    let bounds = preview.snapshot.dimensions.bounds();
    let size = bounds.size();
    let vertical = matches!(
        preview.symmetry,
        crate::map::MapSymmetry::LeftRight
            | crate::map::MapSymmetry::Rotate180
            | crate::map::MapSymmetry::FourWay
    );
    let horizontal = matches!(
        preview.symmetry,
        crate::map::MapSymmetry::TopBottom
            | crate::map::MapSymmetry::Rotate180
            | crate::map::MapSymmetry::FourWay
    );
    if vertical {
        commands.spawn((
            MapEditorOverlay,
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(materials.symmetry.clone()),
            NotShadowCaster,
            Transform {
                translation: ground_position(bounds.center()) + Vec3::Y * 2.0,
                scale: Vec3::new(1.8, 0.7, size.y),
                ..default()
            },
            Name::new("Map Maker left-right symmetry axis"),
        ));
    }
    if horizontal {
        commands.spawn((
            MapEditorOverlay,
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(materials.symmetry.clone()),
            NotShadowCaster,
            Transform {
                translation: ground_position(bounds.center()) + Vec3::Y * 2.0,
                scale: Vec3::new(size.x, 0.7, 1.8),
                ..default()
            },
            Name::new("Map Maker top-bottom symmetry axis"),
        ));
    }
}

fn spawn_selection(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &MapEditorOverlayMaterials,
    preview: &MapEditorPreview,
    catalog: &crate::map::MapContentCatalog,
) {
    let mut selected_cells = Vec::new();
    for placement in preview
        .snapshot
        .placements
        .iter()
        .filter(|placement| preview.selected.contains(&placement.placement_id))
    {
        let Some(asset) = catalog.asset(placement.asset_id) else {
            continue;
        };
        let Some(cells) =
            crate::map::placement_cells(preview.snapshot.dimensions, asset, placement)
        else {
            continue;
        };
        selected_cells.extend(cells.iter().copied());
        let footprint = asset.footprint_cells.rotated(placement.quarter_turns);
        let center =
            crate::map::placement_world_center(preview.snapshot.dimensions, asset, placement);
        spawn_outline(
            commands,
            primitives,
            &materials.move_selection,
            center,
            Vec2::new(f32::from(footprint.width), f32::from(footprint.height))
                * crate::map::MAP_CELL_SIZE_WORLD
                - Vec2::splat(4.0),
            2.4,
            "Map Maker selected placement",
        );
    }
    for anchor in preview
        .snapshot
        .mode_anchors
        .iter()
        .filter(|anchor| preview.selected.contains(&anchor.placement_id))
    {
        let cell = match anchor.kind {
            crate::map::MapModeAnchorKind::HotZoneCircle {
                center_half_cell, ..
            }
            | crate::map::MapModeAnchorKind::ZombieIngress { center_half_cell } => {
                crate::map::MapCell::new(center_half_cell.x / 2, center_half_cell.y / 2)
            }
            crate::map::MapModeAnchorKind::HeistSafe { origin_cell, .. } => origin_cell,
        };
        selected_cells.push(cell);
        let center = preview.snapshot.dimensions.cell_center(cell);
        spawn_outline(
            commands,
            primitives,
            &materials.move_selection,
            center,
            Vec2::splat(crate::map::MAP_CELL_SIZE_WORLD - 4.0),
            2.4,
            "Map Maker selected objective",
        );
    }
    spawn_selection_group(commands, primitives, materials, preview, &selected_cells);
    if let Some((first, second)) = preview.selection_rect {
        spawn_marquee(commands, primitives, materials, preview, first, second);
    }
}

fn spawn_hover(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &MapEditorOverlayMaterials,
    preview: &MapEditorPreview,
    catalog: &crate::map::MapContentCatalog,
) {
    let Some(hovered) = preview.hovered else {
        return;
    };
    if let Some(placement) = preview
        .snapshot
        .placements
        .iter()
        .find(|placement| placement.placement_id == hovered)
        && let Some(asset) = catalog.asset(placement.asset_id)
    {
        let footprint = asset.footprint_cells.rotated(placement.quarter_turns);
        let center =
            crate::map::placement_world_center(preview.snapshot.dimensions, asset, placement);
        spawn_outline(
            commands,
            primitives,
            &materials.hover,
            center,
            Vec2::new(f32::from(footprint.width), f32::from(footprint.height))
                * crate::map::MAP_CELL_SIZE_WORLD
                - Vec2::splat(2.0),
            2.8,
            "Map Maker hovered placement",
        );
        return;
    }
    let Some(anchor) = preview
        .snapshot
        .mode_anchors
        .iter()
        .find(|anchor| anchor.placement_id == hovered)
    else {
        return;
    };
    let cell = match anchor.kind {
        crate::map::MapModeAnchorKind::HotZoneCircle {
            center_half_cell, ..
        }
        | crate::map::MapModeAnchorKind::ZombieIngress { center_half_cell } => {
            crate::map::MapCell::new(center_half_cell.x / 2, center_half_cell.y / 2)
        }
        crate::map::MapModeAnchorKind::HeistSafe { origin_cell, .. } => origin_cell,
    };
    spawn_outline(
        commands,
        primitives,
        &materials.hover,
        preview.snapshot.dimensions.cell_center(cell),
        Vec2::splat(crate::map::MAP_CELL_SIZE_WORLD - 2.0),
        2.8,
        "Map Maker hovered objective",
    );
}

fn spawn_selection_group(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &MapEditorOverlayMaterials,
    preview: &MapEditorPreview,
    selected_cells: &[crate::map::MapCell],
) {
    let (Some(min_x), Some(max_x), Some(min_y), Some(max_y)) = (
        selected_cells.iter().map(|cell| cell.x).min(),
        selected_cells.iter().map(|cell| cell.x).max(),
        selected_cells.iter().map(|cell| cell.y).min(),
        selected_cells.iter().map(|cell| cell.y).max(),
    ) else {
        return;
    };
    let min = crate::map::MapCell::new(min_x, min_y);
    let max = crate::map::MapCell::new(max_x, max_y);
    let a = preview.snapshot.dimensions.cell_center(min);
    let b = preview.snapshot.dimensions.cell_center(max);
    let center = (a + b) * 0.5;
    let size = Vec2::new(
        (b.x - a.x).abs() + crate::map::MAP_CELL_SIZE_WORLD + 5.0,
        (b.y - a.y).abs() + crate::map::MAP_CELL_SIZE_WORLD + 5.0,
    );
    commands.spawn((
        MapEditorOverlay,
        Mesh3d(primitives.unit_cuboid.clone()),
        MeshMaterial3d(materials.selection_fill.clone()),
        NotShadowCaster,
        Transform {
            translation: ground_position(center) + Vec3::Y * 1.8,
            scale: Vec3::new(size.x, 0.35, size.y),
            ..default()
        },
        Name::new("Map Maker selection group fill"),
    ));
    spawn_outline(
        commands,
        primitives,
        &materials.move_selection,
        center,
        size,
        3.0,
        "Map Maker selection group",
    );
    let half = size * 0.5;
    for offset in [
        Vec2::new(-half.x, -half.y),
        Vec2::new(half.x, -half.y),
        Vec2::new(-half.x, half.y),
        Vec2::new(half.x, half.y),
    ] {
        commands.spawn((
            MapEditorOverlay,
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(materials.move_selection.clone()),
            NotShadowCaster,
            Transform {
                translation: ground_position(center + offset) + Vec3::Y * 3.2,
                scale: Vec3::new(11.0, 1.8, 11.0),
                ..default()
            },
            Name::new("Map Maker selection corner handle"),
        ));
    }
    for scale in [Vec3::new(28.0, 1.4, 5.0), Vec3::new(5.0, 1.4, 28.0)] {
        commands.spawn((
            MapEditorOverlay,
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(materials.move_selection.clone()),
            NotShadowCaster,
            Transform {
                translation: ground_position(center) + Vec3::Y * 3.5,
                scale,
                ..default()
            },
            Name::new("Map Maker selection move handle"),
        ));
    }
}

fn spawn_marquee(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    materials: &MapEditorOverlayMaterials,
    preview: &MapEditorPreview,
    first: crate::map::MapCell,
    second: crate::map::MapCell,
) {
    let min = crate::map::MapCell::new(first.x.min(second.x), first.y.min(second.y));
    let max = crate::map::MapCell::new(first.x.max(second.x), first.y.max(second.y));
    let a = preview.snapshot.dimensions.cell_center(min);
    let b = preview.snapshot.dimensions.cell_center(max);
    spawn_outline(
        commands,
        primitives,
        &materials.move_selection,
        (a + b) * 0.5,
        Vec2::new(
            (b.x - a.x).abs() + crate::map::MAP_CELL_SIZE_WORLD - 3.0,
            (b.y - a.y).abs() + crate::map::MAP_CELL_SIZE_WORLD - 3.0,
        ),
        2.2,
        "Map Maker box selection",
    );
}

fn spawn_outline(
    commands: &mut Commands,
    primitives: &Primitive3dAssets,
    material: &Handle<StandardMaterial>,
    center: Vec2,
    size: Vec2,
    height: f32,
    name: &'static str,
) {
    let thickness = 4.2;
    let half = size * 0.5;
    for (offset, scale) in [
        (Vec2::new(0.0, half.y), Vec2::new(size.x, thickness)),
        (Vec2::new(0.0, -half.y), Vec2::new(size.x, thickness)),
        (Vec2::new(half.x, 0.0), Vec2::new(thickness, size.y)),
        (Vec2::new(-half.x, 0.0), Vec2::new(thickness, size.y)),
    ] {
        commands.spawn((
            MapEditorOverlay,
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(material.clone()),
            NotShadowCaster,
            Transform {
                translation: ground_position(center + offset) + Vec3::Y * height,
                scale: Vec3::new(scale.x, 0.9, scale.y),
                ..default()
            },
            Name::new(name),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn options(cursor: crate::map::MapCell) -> MapEditorPreviewOptions {
        MapEditorPreviewOptions {
            cursor,
            grid_visible: true,
            camera_center: Vec2::ZERO,
            camera_zoom: 1.0,
            symmetry: crate::map::MapSymmetry::None,
            pointer_mode: MapEditorPointerMode::Place,
            pointer_valid: true,
            selected: Vec::new(),
            hovered: None,
            selection_rect: None,
        }
    }

    #[test]
    fn incomplete_draft_gets_local_preview_without_preset_provenance() {
        let catalog = crate::map::MapContentCatalog::embedded().unwrap();
        let mut recipe = catalog.presets[0].recipe.clone();
        recipe.placements.clear();
        recipe.filled_rects.clear();
        recipe.mode_anchors.clear();
        let preview = MapEditorPreview::from_recipe(
            MapDraftId(41),
            &recipe,
            &catalog,
            options(crate::map::MapCell::new(3, 5)),
        );

        assert_eq!(preview.snapshot.identity.source_preset_id, None);
        assert_eq!(preview.snapshot.identity.recipe_id, recipe.recipe_id);
        assert_eq!(preview.snapshot.placements, recipe.placements);
        assert_eq!(preview.cursor, crate::map::MapCell::new(3, 5));
    }

    #[test]
    fn cursor_motion_does_not_change_preview_recipe_identity() {
        let catalog = crate::map::MapContentCatalog::embedded().unwrap();
        let recipe = catalog.presets[0].recipe.clone();
        let first = MapEditorPreview::from_recipe(
            MapDraftId(9),
            &recipe,
            &catalog,
            options(crate::map::MapCell::new(1, 1)),
        );
        let moved = MapEditorPreview::from_recipe(
            MapDraftId(9),
            &recipe,
            &catalog,
            options(crate::map::MapCell::new(7, 8)),
        );

        assert_eq!(
            first.snapshot.identity.recipe_fingerprint,
            moved.snapshot.identity.recipe_fingerprint
        );
        assert_ne!(first.cursor, moved.cursor);
    }
}
