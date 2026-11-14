//! Generation-owned 3D map surface materialization and lifecycle markers.

#[allow(
    clippy::wildcard_imports,
    reason = "the world-renderer submodule consumes the presentation_3d composition surface"
)]
use super::*;

/// Top of the playable floor; effect tiles, water, zones, and rings layer above it.
const PLAYABLE_FLOOR_Y: f32 = 0.25;

#[derive(Component)]
pub(super) struct GeneratedMapMesh(pub(super) Handle<Mesh>);

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Presented3dMap {
    pub(super) instance_id: brawler::map::MapInstanceId,
    pub(super) recipe_fingerprint: brawler::map::MapRecipeFingerprint,
    pub(super) theme_id: brawler::map::MapPresentationThemeId,
}

pub(super) fn spawn_ground_surfaces(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &environment_assets::EnvironmentThemeMaterials,
    marker: brawler::map::MapPresentationMember,
    snapshot: &brawler::map::ResolvedMapSnapshot,
    catalog: &brawler::map::MapContentCatalog,
    floor: floor_tiles::FloorTiles,
) {
    let bounds = snapshot.dimensions.bounds();
    let outer_size = bounds.size() + Vec2::splat(border::OUTER_GROUND_MARGIN * 2.0);
    let outer_mesh = meshes.add(Cuboid::new(outer_size.x, 1.0, outer_size.y));
    commands.spawn((
        marker,
        Mesh3d(outer_mesh.clone()),
        GeneratedMapMesh(outer_mesh),
        MeshMaterial3d(materials.outer_ground.clone()),
        Transform::from_translation(ground_position(bounds.center()) - Vec3::Y),
        Name::new("V4 outer ground surface"),
    ));

    let occluders = floor_tiles::floor_occluder_cells(snapshot, catalog);
    let floor_mesh = meshes.add(floor_tiles::floor_tile_mesh(
        snapshot.dimensions,
        &occluders,
        floor,
        PLAYABLE_FLOOR_Y,
    ));
    commands.spawn((
        marker,
        Mesh3d(floor_mesh.clone()),
        GeneratedMapMesh(floor_mesh),
        MeshMaterial3d(materials.floor.clone()),
        NotShadowCaster,
        Transform::IDENTITY,
        Name::new("Floor tile surface"),
    ));

    for (position, size) in brawler::map::perimeter_visual_shapes(bounds) {
        let mesh = meshes.add(Cuboid::new(size.x, 34.0, size.y));
        commands.spawn((
            marker,
            Mesh3d(mesh.clone()),
            GeneratedMapMesh(mesh),
            MeshMaterial3d(materials.perimeter.clone()),
            Transform::from_translation(ground_position(position) + Vec3::Y * 17.0),
            Name::new("V4 raised border foundation"),
        ));
    }
}
