//! Per-axis map/viewport framing, the arena camera writer, and ground-plane cursor projection.

use super::camera_rig::{ArenaCameraShake, CameraRig, CameraRigCatalog, CameraRigFrameInput};
#[allow(
    clippy::wildcard_imports,
    reason = "the world-renderer submodule consumes the presentation_3d composition surface"
)]
use super::*;

/// Vertical field of view at the reference (default native) aspect and every narrower one.
pub(super) const CAMERA_VERTICAL_FOV_RADIANS: f32 = 30.0_f32.to_radians();
pub(super) const CAMERA_ELEVATION_RADIANS: f32 = 55.0_f32.to_radians();
/// Keeps approximately twelve map cells visible vertically (BRL-0148).
pub(super) const CAMERA_DISTANCE: f32 = 566.0;

/// Vertical field of view for a viewport aspect. Narrower viewports keep the reference vertical
/// view and see less width; wider ones keep the reference horizontal view and see less depth, so
/// no window shape sees more of the arena than the reference aspect on either axis.
pub(super) fn camera_vertical_fov(aspect: f32) -> f32 {
    let aspect = valid_aspect(aspect);
    if aspect <= DEFAULT_GAMEPLAY_WINDOW_ASPECT {
        CAMERA_VERTICAL_FOV_RADIANS
    } else {
        2.0 * ((CAMERA_VERTICAL_FOV_RADIANS * 0.5).tan() * DEFAULT_GAMEPLAY_WINDOW_ASPECT / aspect)
            .atan()
    }
}

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

/// Presentation inputs of the camera rig: frame time, rig state and tuning, the local aim
/// sample, and the motion/effects accessibility settings.
#[derive(bevy::ecs::system::SystemParam)]
pub(super) struct CameraRigParams<'w> {
    time: Res<'w, Time>,
    rig: ResMut<'w, CameraRig>,
    catalog: Res<'w, CameraRigCatalog>,
    pending: Option<Res<'w, PendingLocalActions>>,
    settings: Option<Res<'w, ClientShellSettings>>,
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::type_complexity,
    reason = "the camera follows the one controlled replicated fighter"
)]
pub(super) fn follow_3d_camera(
    map: Option<Res<crate::map::PresentedMap>>,
    editor: Option<Res<MapEditorPreview>>,
    fighters: Query<&Position, (With<Fighter>, With<Controlled>, Without<ArenaCamera>)>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut cameras: Query<
        (
            &mut Transform,
            &mut Projection,
            Option<&mut ArenaCameraShake>,
        ),
        With<ArenaCamera>,
    >,
    mut rig: CameraRigParams,
) {
    let viewport = windows
        .iter()
        .next()
        .map_or(DEFAULT_GAMEPLAY_VIEWPORT, |window| {
            Vec2::new(window.width(), window.height())
        });
    let fov = camera_vertical_fov(viewport_aspect(viewport));
    for (_, mut projection, _) in &mut cameras {
        if let Projection::Perspective(perspective) = projection.bypass_change_detection()
            && (perspective.fov - fov).abs() > f32::EPSILON
        {
            perspective.fov = fov;
            projection.set_changed();
        }
    }
    if let Some(editor) = editor {
        let bounds = editor.snapshot.dimensions.bounds();
        let footprint = conservative_ground_footprint(viewport_aspect(viewport));
        let fit_scale = (bounds.size().x / footprint.size().x)
            .max(bounds.size().y / footprint.size().y)
            .max(0.25)
            * 1.12;
        let distance = CAMERA_DISTANCE * fit_scale * editor.camera_zoom.clamp(0.5, 3.0);
        let center = editor.camera_center;
        let target = ground_position(center);
        let offset = camera_offset() * (distance / CAMERA_DISTANCE);
        // The editor camera is direct; leaving the editor snaps the gameplay rig.
        rig.rig.reset();
        for (mut transform, _, shake) in &mut cameras {
            *transform = Transform::from_translation(target + offset).looking_at(target, Vec3::Y);
            if let Some(mut shake) = shake {
                shake.0 = Vec3::ZERO;
            }
        }
        return;
    }
    let (bounds, fighter) = map.as_ref().map_or_else(
        || {
            let neutral = crate::map::AxisAlignedMapRect {
                min: Vec2::ZERO,
                max: Vec2::ZERO,
            };
            (neutral, None)
        },
        |map| {
            (
                map.camera_bounds,
                fighters.iter().next().map(|position| position.0),
            )
        },
    );
    let settings = rig.settings.as_deref().copied().unwrap_or_default();
    let aim = rig.pending.as_deref().and_then(|pending| {
        pending
            .aim_axis
            .map(|direction| (direction, pending.aim_distance))
    });
    let dt = rig.time.delta_secs();
    let catalog = *rig.catalog;
    let frame = rig.rig.step(
        &catalog,
        CameraRigFrameInput {
            fighter,
            aim,
            bounds,
            viewport,
            dt,
            reduced_motion: settings.reduced_motion,
            reduced_effects: settings.reduced_combat_effects,
        },
    );
    let target = ground_position(frame.center);
    let shake = ground_position(frame.shake);
    let offset = camera_offset();
    for (mut transform, _, shake_component) in &mut cameras {
        *transform = Transform::from_translation(target + offset + shake)
            .looking_at(target + shake, Vec3::Y);
        if let Some(mut shake_component) = shake_component {
            shake_component.0 = shake;
        }
    }
}

/// Camera position relative to its ground look target at the framing distance.
pub(super) fn camera_offset() -> Vec3 {
    Vec3::new(
        0.0,
        CAMERA_ELEVATION_RADIANS.sin(),
        CAMERA_ELEVATION_RADIANS.cos(),
    ) * CAMERA_DISTANCE
}

fn viewport_aspect(viewport: Vec2) -> f32 {
    if viewport.is_finite() && viewport.x > 0.0 && viewport.y > 0.0 {
        viewport.x / viewport.y
    } else {
        DEFAULT_GAMEPLAY_WINDOW_ASPECT
    }
}

fn valid_aspect(aspect: f32) -> f32 {
    if aspect.is_finite() && aspect > 0.0 {
        aspect
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

pub(super) fn conservative_ground_footprint(aspect: f32) -> crate::map::AxisAlignedMapRect {
    let aspect = valid_aspect(aspect);
    let sin_elevation = CAMERA_ELEVATION_RADIANS.sin();
    let cos_elevation = CAMERA_ELEVATION_RADIANS.cos();
    let camera_height = CAMERA_DISTANCE * sin_elevation;
    let camera_ground_distance = CAMERA_DISTANCE * cos_elevation;
    let vertical_tangent = (camera_vertical_fov(aspect) * 0.5).tan();
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
    fn reference_view_matches_the_twelve_cell_target() {
        let footprint = conservative_ground_footprint(DEFAULT_GAMEPLAY_WINDOW_ASPECT);
        let visible_cells = footprint.size() / crate::map::MAP_CELL_SIZE_WORLD;

        assert!((visible_cells.y - 12.0).abs() < 0.02, "{visible_cells}");
        assert!((visible_cells.x - 17.64).abs() < 0.02, "{visible_cells}");
        // The fighter sits nearer the bottom edge: ~4.9 cells behind, ~7.1 cells ahead.
        assert!((footprint.min.y / crate::map::MAP_CELL_SIZE_WORLD + 4.87).abs() < 0.02);
        assert!((footprint.max.y / crate::map::MAP_CELL_SIZE_WORLD - 7.12).abs() < 0.02);
    }

    #[test]
    fn standard_pulse_range_fits_beside_the_fighter_at_the_reference_aspect() {
        // At the look target the view is wider than the trapezoid's conservative near edge.
        let half_width_at_target = CAMERA_DISTANCE
            * (camera_vertical_fov(DEFAULT_GAMEPLAY_WINDOW_ASPECT) * 0.5).tan()
            * DEFAULT_GAMEPLAY_WINDOW_ASPECT;
        // Standard Pulse Sidearm reach (`content/catalogs/weapons.ron`).
        assert!(half_width_at_target >= 320.0, "{half_width_at_target}");
    }

    #[test]
    fn no_viewport_shape_sees_more_than_the_reference_aspect() {
        let reference = conservative_ground_footprint(DEFAULT_GAMEPLAY_WINDOW_ASPECT);
        let reference_horizontal =
            (CAMERA_VERTICAL_FOV_RADIANS * 0.5).tan() * DEFAULT_GAMEPLAY_WINDOW_ASPECT;
        for aspect in [4.0 / 3.0, 16.0 / 9.0, 21.0 / 9.0, 32.0 / 9.0, 8.0] {
            let fov = camera_vertical_fov(aspect);
            let horizontal = (fov * 0.5).tan() * aspect;
            let footprint = conservative_ground_footprint(aspect);
            assert!(fov <= CAMERA_VERTICAL_FOV_RADIANS + 1e-6);
            assert!(horizontal <= reference_horizontal + 1e-5, "{aspect}");
            assert!(footprint.min.y >= reference.min.y - EPSILON, "{aspect}");
            assert!(footprint.max.y <= reference.max.y + EPSILON, "{aspect}");
            if aspect < DEFAULT_GAMEPLAY_WINDOW_ASPECT {
                // Narrower windows keep the reference depth and lose width.
                assert_approximately(footprint.min.y, reference.min.y);
                assert_approximately(footprint.max.y, reference.max.y);
                assert!(footprint.size().x < reference.size().x);
            } else {
                // Wider windows keep the reference width at the look target and lose depth.
                assert!((horizontal - reference_horizontal).abs() < 1e-5, "{aspect}");
                assert!(footprint.size().y < reference.size().y);
            }
        }
        assert!(
            (camera_vertical_fov(f32::NAN) - camera_vertical_fov(DEFAULT_GAMEPLAY_WINDOW_ASPECT))
                .abs()
                < 1e-6
        );
    }

    #[test]
    fn narrow_map_centers_width_and_follows_height() {
        let bounds = centered_map(16.0, 37.0);
        let viewport = DEFAULT_GAMEPLAY_VIEWPORT;
        let player = Vec2::new(150.0, 300.0);
        let center = clamp_3d_camera_center(player, bounds, viewport);
        let visible = visible_ground_bounds(center, viewport);

        assert_approximately(center.x, bounds.center().x);
        assert_approximately(center.y, player.y);
        assert!(visible.min.x <= bounds.min.x);
        assert!(visible.max.x >= bounds.max.x);
    }

    #[test]
    fn production_map_follows_both_axes_and_clamps_at_the_perimeter() {
        let bounds = centered_map(25.0, 37.0);
        let viewport = DEFAULT_GAMEPLAY_VIEWPORT;
        let near_center = Vec2::new(60.0, 100.0);
        assert_eq!(
            clamp_3d_camera_center(near_center, bounds, viewport),
            near_center
        );

        let corner = clamp_3d_camera_center(bounds.max, bounds, viewport);
        let visible = visible_ground_bounds(corner, viewport);
        assert_approximately(visible.max.x, bounds.max.x);
        assert_approximately(visible.max.y, bounds.max.y);
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
        let bounds = centered_map(16.0, 37.0);
        let player = Vec2::new(20.0, 0.0);
        let reference = clamp_3d_camera_center(player, bounds, DEFAULT_GAMEPLAY_VIEWPORT);
        let narrow = clamp_3d_camera_center(player, bounds, Vec2::new(16.0, 9.0));

        assert_approximately(reference.x, bounds.center().x);
        assert_approximately(narrow.x, player.x);
    }
}
