//! Fixed 3D world-camera framing and ground-plane cursor projection.

use super::*;

pub(super) const CAMERA_VERTICAL_SPAN_3D: f32 = 900.0;
pub(super) const CAMERA_ELEVATION_RADIANS: f32 = 55.0_f32.to_radians();
pub(super) const CAMERA_DISTANCE: f32 = 1_200.0;

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
            (map.camera_bounds, position)
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
    let half_y = CAMERA_VERTICAL_SPAN_3D * 0.5 / CAMERA_ELEVATION_RADIANS.sin();
    let half_x = CAMERA_VERTICAL_SPAN_3D * 0.5 * aspect.max(0.0);
    let min = bounds.min + Vec2::new(half_x, half_y);
    let max = bounds.max - Vec2::new(half_x, half_y);
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
