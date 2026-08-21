//! Generation-owned 3D map surface materialization and lifecycle markers.

use super::*;

const PLAYABLE_GROUND_CENTER_Y: f32 = -0.25;
const PLAYABLE_GROUND_THICKNESS: f32 = 1.0;
const GROUND_ACCENT_Y: f32 = 0.27;
const GROUND_ACCENT_COUNT: usize = 18;

#[derive(Component)]
pub(super) struct GeneratedMapMesh(pub(super) Handle<Mesh>);

#[derive(Resource, Clone, Copy)]
pub(super) struct Presented3dMap(pub(super) crate::map::MapInstanceId);

pub(super) fn spawn_ground_surfaces(
    commands: &mut Commands,
    meshes: &mut Assets<Mesh>,
    materials: &Material3dAssets,
    marker: crate::map::MapPresentationMember,
    bounds: crate::map::AxisAlignedMapRect,
) {
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

    let floor_size = bounds.size();
    let floor_mesh = meshes.add(Cuboid::new(
        floor_size.x,
        PLAYABLE_GROUND_THICKNESS,
        floor_size.y,
    ));
    commands.spawn((
        marker,
        Mesh3d(floor_mesh.clone()),
        GeneratedMapMesh(floor_mesh),
        MeshMaterial3d(materials.floor.clone()),
        NotShadowCaster,
        Transform::from_translation(
            ground_position(bounds.center()) + Vec3::Y * PLAYABLE_GROUND_CENTER_Y,
        ),
        Name::new("V4 playable ground surface"),
    ));

    let patch_mesh = meshes.add(organic_ground_patch_mesh());
    let accents: [(Vec2, Vec2, f32); GROUND_ACCENT_COUNT] = [
        (Vec2::new(-0.76, -0.67), Vec2::new(62.0, 23.0), -0.32),
        (Vec2::new(-0.48, -0.72), Vec2::new(38.0, 17.0), 0.18),
        (Vec2::new(-0.14, -0.62), Vec2::new(76.0, 29.0), -0.08),
        (Vec2::new(0.21, -0.73), Vec2::new(46.0, 19.0), 0.27),
        (Vec2::new(0.58, -0.61), Vec2::new(70.0, 25.0), -0.22),
        (Vec2::new(0.78, -0.35), Vec2::new(35.0, 15.0), 0.11),
        (Vec2::new(-0.67, -0.26), Vec2::new(51.0, 21.0), 0.24),
        (Vec2::new(-0.29, -0.19), Vec2::new(82.0, 31.0), -0.17),
        (Vec2::new(0.10, -0.28), Vec2::new(42.0, 18.0), 0.34),
        (Vec2::new(0.48, -0.17), Vec2::new(64.0, 24.0), -0.28),
        (Vec2::new(0.73, 0.06), Vec2::new(45.0, 16.0), 0.07),
        (Vec2::new(-0.73, 0.19), Vec2::new(69.0, 27.0), -0.14),
        (Vec2::new(-0.38, 0.31), Vec2::new(40.0, 16.0), 0.29),
        (Vec2::new(-0.03, 0.18), Vec2::new(73.0, 28.0), -0.25),
        (Vec2::new(0.33, 0.33), Vec2::new(48.0, 19.0), 0.16),
        (Vec2::new(0.66, 0.48), Vec2::new(79.0, 30.0), -0.09),
        (Vec2::new(0.18, 0.69), Vec2::new(57.0, 22.0), 0.31),
        (Vec2::new(-0.51, 0.66), Vec2::new(67.0, 25.0), -0.20),
    ];
    for (offset, scale, rotation) in accents {
        let position = bounds.center() + offset * bounds.size() * 0.5;
        commands.spawn((
            marker,
            Mesh3d(patch_mesh.clone()),
            GeneratedMapMesh(patch_mesh.clone()),
            MeshMaterial3d(materials.floor_accent.clone()),
            NotShadowCaster,
            Transform {
                translation: ground_position(position) + Vec3::Y * GROUND_ACCENT_Y,
                rotation: Quat::from_rotation_y(rotation)
                    * Quat::from_rotation_x(-core::f32::consts::FRAC_PI_2),
                scale: Vec3::new(scale.x, scale.y, 1.0),
            },
            Name::new("V4 ground accent patch"),
        ));
    }

    for (position, size) in crate::map::perimeter_visual_shapes(bounds) {
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

fn organic_ground_patch_mesh() -> Mesh {
    let outline = [
        [1.0, 0.02, 0.0],
        [0.78, 0.48, 0.0],
        [0.30, 0.82, 0.0],
        [-0.24, 0.76, 0.0],
        [-0.78, 0.43, 0.0],
        [-0.94, -0.08, 0.0],
        [-0.61, -0.55, 0.0],
        [-0.09, -0.84, 0.0],
        [0.46, -0.70, 0.0],
        [0.87, -0.35, 0.0],
    ];
    let mut positions = vec![[0.0, 0.0, 0.0]];
    positions.extend(outline);
    let normals = vec![[0.0, 0.0, 1.0]; positions.len()];
    let uvs = positions
        .iter()
        .map(|position| [position[0] * 0.5 + 0.5, position[1] * 0.5 + 0.5])
        .collect::<Vec<_>>();
    let mut indices = Vec::with_capacity(outline.len() * 3);
    for edge in 0_u16..10 {
        indices.extend([0, edge + 1, (edge + 1) % 10 + 1]);
    }

    let mut mesh = Mesh::new(
        PrimitiveTopology::TriangleList,
        RenderAssetUsages::default(),
    );
    mesh.insert_attribute(Mesh::ATTRIBUTE_POSITION, positions);
    mesh.insert_attribute(Mesh::ATTRIBUTE_NORMAL, normals);
    mesh.insert_attribute(Mesh::ATTRIBUTE_UV_0, uvs);
    mesh.insert_indices(Indices::U16(indices));
    mesh
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn ground_accents_render_above_the_playable_floor() {
        let floor_top = PLAYABLE_GROUND_CENTER_Y + PLAYABLE_GROUND_THICKNESS * 0.5;

        assert!(GROUND_ACCENT_Y > floor_top);
    }

    #[test]
    fn ground_accents_are_bounded_small_irregular_details() {
        assert_eq!(GROUND_ACCENT_COUNT, 18);
        assert_eq!(organic_ground_patch_mesh().count_vertices(), 11);
    }
}
