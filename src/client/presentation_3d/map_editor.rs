//! Local-only 3D draft preview and editor overlays.

use super::*;

const EDITOR_INSTANCE_PREFIX: u64 = 0xFFFF_0000_0000_0000;

#[derive(Resource, Clone, Debug, PartialEq)]
pub(in crate::client) struct MapEditorPreview {
    pub(in crate::client) snapshot: crate::map::ResolvedMapSnapshot,
    pub(in crate::client) cursor: crate::map::MapCell,
    pub(in crate::client) grid_visible: bool,
    pub(in crate::client) camera_center: Vec2,
    pub(in crate::client) camera_zoom: f32,
}

impl MapEditorPreview {
    pub(in crate::client) fn from_recipe(
        draft_id: MapDraftId,
        recipe: &crate::map::MapRecipe,
        catalog: &crate::map::MapContentCatalog,
        cursor: crate::map::MapCell,
        grid_visible: bool,
        camera_center: Vec2,
        camera_zoom: f32,
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
            cursor,
            grid_visible,
            camera_center,
            camera_zoom,
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
    grid: Handle<StandardMaterial>,
    cursor: Handle<StandardMaterial>,
}

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct PresentedMapEditorOverlay {
    instance_id: crate::map::MapInstanceId,
    recipe_revision: u32,
    grid_visible: bool,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "the overlay reconciler consumes distinct Bevy-owned rendering resources and queries"
)]
pub(super) fn reconcile_map_editor_overlay(
    mut commands: Commands,
    preview: Option<Res<MapEditorPreview>>,
    primitives: Res<Primitive3dAssets>,
    mut standard_materials: ResMut<Assets<StandardMaterial>>,
    overlay_materials: Option<Res<MapEditorOverlayMaterials>>,
    presented: Option<Res<PresentedMapEditorOverlay>>,
    overlays: Query<Entity, With<MapEditorOverlay>>,
    mut cursors: Query<&mut Transform, With<MapEditorCursor>>,
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
            grid: standard_materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 0.96, 0.80, 0.34),
                alpha_mode: AlphaMode::Blend,
                unlit: true,
                ..default()
            }),
            cursor: standard_materials.add(StandardMaterial {
                base_color: Color::srgba(1.0, 0.42, 0.04, 0.34),
                emissive: LinearRgba::new(1.4, 0.28, 0.01, 1.0),
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
    };
    if presented.as_deref() != Some(&desired) {
        for entity in &overlays {
            commands.entity(entity).try_despawn();
        }
        if preview.grid_visible {
            spawn_grid(&mut commands, &primitives, &materials, &preview.snapshot);
        }
        let center = preview.snapshot.dimensions.cell_center(preview.cursor);
        commands.spawn((
            MapEditorOverlay,
            MapEditorCursor,
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(materials.cursor.clone()),
            NotShadowCaster,
            Transform {
                translation: ground_position(center) + Vec3::Y * 1.4,
                scale: Vec3::new(
                    crate::map::MAP_CELL_SIZE_WORLD - 2.0,
                    1.2,
                    crate::map::MAP_CELL_SIZE_WORLD - 2.0,
                ),
                ..default()
            },
            Name::new("Map Maker hovered cell"),
        ));
        commands.insert_resource(desired);
        return;
    }

    let center = preview.snapshot.dimensions.cell_center(preview.cursor);
    for mut transform in &mut cursors {
        transform.translation = ground_position(center) + Vec3::Y * 1.4;
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
        commands.spawn((
            MapEditorOverlay,
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(materials.grid.clone()),
            NotShadowCaster,
            Transform {
                translation: ground_position(Vec2::new(world_x, bounds.center().y)) + Vec3::Y * 1.1,
                scale: Vec3::new(0.75, 0.5, size.y),
                ..default()
            },
            Name::new("Map Maker grid column"),
        ));
    }
    for y in 0..=snapshot.dimensions.height {
        let world_y = bounds.min.y + f32::from(y) * crate::map::MAP_CELL_SIZE_WORLD;
        commands.spawn((
            MapEditorOverlay,
            Mesh3d(primitives.unit_cuboid.clone()),
            MeshMaterial3d(materials.grid.clone()),
            NotShadowCaster,
            Transform {
                translation: ground_position(Vec2::new(bounds.center().x, world_y)) + Vec3::Y * 1.1,
                scale: Vec3::new(size.x, 0.5, 0.75),
                ..default()
            },
            Name::new("Map Maker grid row"),
        ));
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
            crate::map::MapCell::new(3, 5),
            true,
            Vec2::ZERO,
            1.0,
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
            crate::map::MapCell::new(1, 1),
            true,
            Vec2::ZERO,
            1.0,
        );
        let moved = MapEditorPreview::from_recipe(
            MapDraftId(9),
            &recipe,
            &catalog,
            crate::map::MapCell::new(7, 8),
            true,
            Vec2::ZERO,
            1.0,
        );

        assert_eq!(
            first.snapshot.identity.recipe_fingerprint,
            moved.snapshot.identity.recipe_fingerprint
        );
        assert_ne!(first.cursor, moved.cursor);
    }
}
