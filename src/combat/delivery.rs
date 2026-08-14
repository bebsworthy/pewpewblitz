//! Pure geometry shared by server delivery systems and client previews.

use bevy::prelude::Vec2;

#[must_use]
pub fn lob_height(progress: f32, visual_arc_height: f32) -> f32 {
    let progress = progress.clamp(0.0, 1.0);
    4.0 * visual_arc_height * progress * (1.0 - progress)
}

#[must_use]
pub fn sector_contains(
    origin: Vec2,
    facing: f32,
    reach: f32,
    angle_degrees: f32,
    target_center: Vec2,
    target_radius: f32,
) -> bool {
    let delta = target_center - origin;
    let distance = delta.length();
    if !delta.is_finite() || !distance.is_finite() || distance > reach + target_radius {
        return false;
    }
    if distance <= f32::EPSILON {
        return true;
    }
    let half_angle = (angle_degrees.to_radians() / 2.0).clamp(0.0, std::f32::consts::PI);
    let angular_padding = (target_radius / distance).clamp(0.0, 1.0).asin();
    let difference = (delta.y.atan2(delta.x) - facing + std::f32::consts::PI)
        .rem_euclid(std::f32::consts::TAU)
        - std::f32::consts::PI;
    difference.abs() <= half_angle + angular_padding
}

#[must_use]
pub fn repaired_landing_point(
    launch: Vec2,
    desired: Vec2,
    minimum_distance: f32,
    mut is_clear: impl FnMut(Vec2) -> bool,
) -> Option<Vec2> {
    let ray = desired - launch;
    let distance = ray.length();
    if !distance.is_finite() || distance <= f32::EPSILON {
        return is_clear(launch).then_some(launch);
    }
    let direction = ray / distance;
    let minimum_distance = minimum_distance.clamp(0.0, distance);
    let mut furthest_clear = None;
    let mut blocked = distance;
    let mut sample = distance;
    for _ in 0..128 {
        let point = launch + direction * sample;
        if is_clear(point) {
            furthest_clear = Some(sample);
            break;
        }
        blocked = sample;
        if sample <= minimum_distance {
            break;
        }
        sample = (sample - 5.0).max(minimum_distance);
    }
    let mut clear = furthest_clear?;
    for _ in 0..8 {
        let middle = clear.midpoint(blocked);
        if is_clear(launch + direction * middle) {
            clear = middle;
        } else {
            blocked = middle;
        }
    }
    Some(launch + direction * clear)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn arc_height_peaks_at_half_progress() {
        assert!((lob_height(0.5, 140.0) - 140.0).abs() < 0.001);
    }
    #[test]
    fn sector_includes_tangent_target_radius() {
        assert!(sector_contains(
            Vec2::ZERO,
            0.0,
            100.0,
            60.0,
            Vec2::new(90.0, 20.0),
            20.0
        ));
    }
    #[test]
    fn landing_repair_returns_furthest_clear_point() {
        let point =
            repaired_landing_point(Vec2::ZERO, Vec2::X * 20.0, 0.0, |p| p.x < 12.0).unwrap();
        assert!(point.x < 12.1 && point.x > 6.0);
    }
}
