//! Per-axis map/viewport framing and ground-plane cursor projection.

use super::*;

pub(super) const CAMERA_VERTICAL_FOV_RADIANS: f32 = 15.0_f32.to_radians();
pub(super) const CAMERA_ELEVATION_RADIANS: f32 = 62.0_f32.to_radians();
/// Keeps approximately fourteen map cells visible vertically while the conservative near edge
/// remains wider than the 25-cell production maps at the reference aspect.
pub(super) const CAMERA_DISTANCE: f32 = 1_495.0;

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
        .map_or(DEFAULT_GAMEPLAY_VIEWPORT, |window| {
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

fn viewport_aspect(viewport: Vec2) -> f32 {
    if viewport.is_finite() && viewport.x > 0.0 && viewport.y > 0.0 {
        viewport.x / viewport.y
    } else {
        DEFAULT_GAMEPLAY_WINDOW_ASPECT
    }
}

pub(super) fn clamp_3d_camera_center(
    position: Vec2,
    bounds: crate::map::AxisAlignedMapRect,
    viewport: Vec2,
) -> Vec2 {
    let footprint = conservative_ground_footprint(viewport_aspect(viewport));
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

fn conservative_ground_footprint(aspect: f32) -> crate::map::AxisAlignedMapRect {
    let aspect = if aspect.is_finite() && aspect > 0.0 {
        aspect
    } else {
        DEFAULT_GAMEPLAY_WINDOW_ASPECT
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
    // A tilted perspective view is a trapezoid on the ground. Axis-fit decisions must use its
    // narrow near edge so a map judged to fit cannot be clipped lower in the viewport.
    let near_half_width = near_parameter * horizontal_tangent;

    crate::map::AxisAlignedMapRect {
        min: Vec2::new(-near_half_width, near_y),
        max: Vec2::new(near_half_width, far_y),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const EPSILON: f32 = 0.02;

    fn centered_map(width_cells: f32, height_cells: f32) -> crate::map::AxisAlignedMapRect {
        let half_size =
            Vec2::new(width_cells, height_cells) * crate::map::MAP_CELL_SIZE_WORLD * 0.5;
        crate::map::AxisAlignedMapRect {
            min: -half_size,
            max: half_size,
        }
    }

    fn visible_ground_bounds(center: Vec2, viewport: Vec2) -> crate::map::AxisAlignedMapRect {
        let footprint = conservative_ground_footprint(viewport.x / viewport.y);
        crate::map::AxisAlignedMapRect {
            min: center + footprint.min,
            max: center + footprint.max,
        }
    }

    fn assert_approximately(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() < EPSILON,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn reference_view_matches_the_fourteen_cell_mobile_target() {
        let footprint = conservative_ground_footprint(DEFAULT_GAMEPLAY_WINDOW_ASPECT);
        let visible_cells = footprint.size() / crate::map::MAP_CELL_SIZE_WORLD;

        assert!((visible_cells.y - 14.0).abs() < 0.01);
        assert!((visible_cells.x - 25.40).abs() < 0.02);
    }

    #[test]
    fn production_map_centers_width_and_follows_height() {
        let bounds = centered_map(25.0, 37.0);
        let viewport = DEFAULT_GAMEPLAY_VIEWPORT;
        let player = Vec2::new(390.0, 300.0);
        let center = clamp_3d_camera_center(player, bounds, viewport);
        let visible = visible_ground_bounds(center, viewport);

        assert_approximately(center.x, bounds.center().x);
        assert_approximately(center.y, player.y);
        assert!(visible.min.x <= bounds.min.x);
        assert!(visible.max.x >= bounds.max.x);
    }

    #[test]
    fn larger_map_follows_the_player_on_both_axes() {
        let bounds = centered_map(50.0, 50.0);
        let viewport = DEFAULT_GAMEPLAY_VIEWPORT;
        let player = Vec2::new(300.0, 300.0);
        let center = clamp_3d_camera_center(player, bounds, viewport);

        assert_eq!(center, player);
    }

    #[test]
    fn player_remains_inside_view_when_follow_is_clamped_at_every_edge() {
        let bounds = centered_map(50.0, 50.0);
        let viewport = DEFAULT_GAMEPLAY_VIEWPORT;

        for player in [
            Vec2::new(bounds.min.x, 0.0),
            Vec2::new(bounds.max.x, 0.0),
            Vec2::new(0.0, bounds.min.y),
            Vec2::new(0.0, bounds.max.y),
        ] {
            let center = clamp_3d_camera_center(player, bounds, viewport);
            let visible = visible_ground_bounds(center, viewport);
            assert!(player.x >= visible.min.x - EPSILON);
            assert!(player.x <= visible.max.x + EPSILON);
            assert!(player.y >= visible.min.y - EPSILON);
            assert!(player.y <= visible.max.y + EPSILON);
        }
    }

    #[test]
    fn resizing_recomputes_fit_vs_follow_per_axis() {
        let bounds = centered_map(25.0, 37.0);
        let player = Vec2::new(40.0, 0.0);
        let reference = clamp_3d_camera_center(player, bounds, DEFAULT_GAMEPLAY_VIEWPORT);
        let narrow = clamp_3d_camera_center(player, bounds, Vec2::new(16.0, 9.0));

        assert_approximately(reference.x, bounds.center().x);
        assert_approximately(narrow.x, player.x);
    }
}
