//! One-way conversion between the authoritative X/Y plane and the rendered X/Z ground plane.

use avian2d::prelude::Rotation;
use bevy::prelude::*;

/// Map simulation +X to render +X and simulation +Y to render -Z.
#[must_use]
pub(crate) fn ground_position(position: Vec2) -> Vec3 {
    Vec3::new(position.x, 0.0, -position.y)
}

/// Map a simulation direction without introducing render height.
#[must_use]
pub(crate) fn ground_direction(direction: Vec2) -> Vec3 {
    Vec3::new(direction.x, 0.0, -direction.y)
}

/// Recover a simulation point while intentionally discarding presentation-only height.
#[must_use]
pub(crate) fn ground_point(point: Vec3) -> Vec2 {
    Vec2::new(point.x, -point.z)
}

/// Convert planar facing to a yaw around Bevy's render-height axis.
#[must_use]
pub(crate) fn ground_rotation(rotation: Rotation) -> Quat {
    Quat::from_rotation_y(rotation.as_radians())
}

/// Convert full simulation rectangle dimensions to X/Z dimensions.
#[must_use]
pub(crate) fn ground_extents(extents: Vec2) -> Vec3 {
    Vec3::new(extents.x, 0.0, extents.y)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn assert_vec3_close(actual: Vec3, expected: Vec3) {
        assert!(
            actual.abs_diff_eq(expected, 0.0001),
            "{actual:?} != {expected:?}"
        );
    }

    #[test]
    fn positions_round_trip_and_discard_render_height() {
        for point in [
            Vec2::ZERO,
            Vec2::new(12.5, -40.0),
            Vec2::new(-2_048.0, 1_536.0),
        ] {
            assert_eq!(ground_point(ground_position(point)), point);
            assert_eq!(ground_point(ground_position(point) + Vec3::Y * 99.0), point);
        }
    }

    #[test]
    fn basis_directions_and_extents_preserve_the_gameplay_plane() {
        assert_eq!(ground_direction(Vec2::X), Vec3::X);
        assert_eq!(ground_direction(Vec2::Y), Vec3::NEG_Z);
        assert_eq!(
            ground_extents(Vec2::new(320.0, 64.0)),
            Vec3::new(320.0, 0.0, 64.0)
        );
    }

    #[test]
    fn yaw_points_positive_x_toward_simulation_facing() {
        for angle in [
            0.0,
            core::f32::consts::FRAC_PI_2,
            core::f32::consts::PI,
            -0.73,
        ] {
            let rotation = Rotation::radians(angle);
            assert_vec3_close(
                ground_rotation(rotation) * Vec3::X,
                ground_direction(Vec2::from_angle(angle)),
            );
        }
    }
}
