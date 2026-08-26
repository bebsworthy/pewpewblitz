//! Fixed 3D world-camera framing and ground-plane cursor projection.

use super::*;

pub(super) const CAMERA_VERTICAL_FOV_RADIANS: f32 = 27.0_f32.to_radians();
pub(super) const CAMERA_ELEVATION_RADIANS: f32 = 55.0_f32.to_radians();
/// Matches the accepted mobile-combat target of approximately fourteen map cells vertically.
pub(super) const CAMERA_DISTANCE: f32 = 743.0;
/// Keep a visible environment band beyond authoritative containment when following edge players.
const PRESENTATION_MARGIN: f32 = 224.0;

/// Intersect a viewport ray with the sole gameplay ground plane (`Y = 0`).
pub(crate) fn cursor_ground_point(
    camera: &Camera,
    camera_transform: &GlobalTransform,
    cursor: Vec2,
) -> Option<Vec2> {
    let ray = camera.viewport_to_world(camera_transform, cursor).ok()?;
    ray.plane_intersection_point(Vec3::ZERO, InfinitePlane3d::new(Vec3::Y))
        .map(ground_point)
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "the camera follows the one controlled replicated fighter"
)]
pub(super) fn follow_3d_camera(
    map: Option<Res<crate::map::PresentedMap>>,
    fighters: Query<&Position, (With<Fighter>, With<Controlled>, Without<ArenaCamera>)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<&mut Transform, With<ArenaCamera>>,
) {
    let (bounds, position) = map.as_ref().map_or_else(
        || {
            let neutral = crate::map::AxisAlignedMapRect {
                min: Vec2::ZERO,
                max: Vec2::ZERO,
            };
            (neutral, Vec2::ZERO)
        },
        |map| {
            let position = fighters
                .iter()
                .next()
                .map_or_else(|| map.camera_bounds.center(), |position| position.0);
            (presentation_camera_bounds(map.camera_bounds), position)
        },
    );
    let viewport = windows
        .iter()
        .next()
        .map_or(Vec2::new(16.0, 9.0), |window| {
            Vec2::new(window.width(), window.height())
        });
    let center = clamp_3d_camera_center(position, bounds, viewport);
    let target = ground_position(center);
    let offset = Vec3::new(
        0.0,
        CAMERA_ELEVATION_RADIANS.sin(),
        CAMERA_ELEVATION_RADIANS.cos(),
    ) * CAMERA_DISTANCE;
    for mut transform in &mut cameras {
        *transform = Transform::from_translation(target + offset).looking_at(target, Vec3::Y);
    }
}

pub(super) fn presentation_camera_bounds(
    bounds: crate::map::AxisAlignedMapRect,
) -> crate::map::AxisAlignedMapRect {
    crate::map::AxisAlignedMapRect {
        min: bounds.min - Vec2::splat(PRESENTATION_MARGIN),
        max: bounds.max + Vec2::splat(PRESENTATION_MARGIN),
    }
}

pub(super) fn clamp_3d_camera_center(
    position: Vec2,
    bounds: crate::map::AxisAlignedMapRect,
    viewport: Vec2,
) -> Vec2 {
    let aspect = if viewport.is_finite() && viewport.x > 0.0 && viewport.y > 0.0 {
        viewport.x / viewport.y
    } else {
        16.0 / 9.0
    };
    let footprint = perspective_ground_footprint(aspect);
    let min = bounds.min - footprint.min;
    let max = bounds.max - footprint.max;
    Vec2::new(
        if min.x > max.x {
            f32::midpoint(bounds.min.x, bounds.max.x)
        } else {
            position.x.clamp(min.x, max.x)
        },
        if min.y > max.y {
            f32::midpoint(bounds.min.y, bounds.max.y)
        } else {
            position.y.clamp(min.y, max.y)
        },
    )
}

pub(super) fn perspective_ground_footprint(aspect: f32) -> crate::map::AxisAlignedMapRect {
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        16.0 / 9.0
    };
    let sin_elevation = CAMERA_ELEVATION_RADIANS.sin();
    let cos_elevation = CAMERA_ELEVATION_RADIANS.cos();
    let camera_height = CAMERA_DISTANCE * sin_elevation;
    let camera_ground_distance = CAMERA_DISTANCE * cos_elevation;
    let vertical_tangent = (CAMERA_VERTICAL_FOV_RADIANS * 0.5).tan();
    let horizontal_tangent = vertical_tangent * aspect;
    let far_parameter = camera_height / (sin_elevation - vertical_tangent * cos_elevation);
    let near_parameter = camera_height / (sin_elevation + vertical_tangent * cos_elevation);
    let far_y =
        far_parameter * (cos_elevation + vertical_tangent * sin_elevation) - camera_ground_distance;
    let near_y = near_parameter * (cos_elevation - vertical_tangent * sin_elevation)
        - camera_ground_distance;
    let far_half_width = far_parameter * horizontal_tangent;

    crate::map::AxisAlignedMapRect {
        min: Vec2::new(-far_half_width, near_y),
        max: Vec2::new(far_half_width, far_y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn landscape_camera_matches_the_fourteen_cell_mobile_target() {
        let footprint = perspective_ground_footprint(16.0 / 9.0);
        let visible_cells = footprint.size() / crate::map::MAP_CELL_SIZE_WORLD;

        assert!((visible_cells.y - 14.0).abs() < 0.01);
        assert!((visible_cells.x - 23.82).abs() < 0.02);
    }
}
