//! Viewport fit policy for small logical windows (BRL-0196).
//!
//! Shell and HUD layouts are authored for desktop windows at least 640 logical pixels tall (the
//! Dashboard switches to its stacked compact layout below that).
//! A landscape phone (Pixel 8a: about 914x411 logical) is far shorter, so the effective UI scale
//! shrinks with the logical height until the authored layout fits again. Taller windows keep the
//! player's chosen scale unchanged, so desktop layouts at 1280x720 and above do not move.

/// Smallest logical height at which authored shell layouts fit without a fit reduction.
pub(crate) const FIT_REFERENCE_HEIGHT: f32 = 640.0;

/// The fit reduction never shrinks the UI below this factor, keeping text legible.
const MIN_FIT: f32 = 0.5;

/// Factor applied on top of the player's UI-scale setting for a window `logical_height` tall.
pub(crate) fn viewport_fit(logical_height: f32) -> f32 {
    if !logical_height.is_finite() || logical_height <= 0.0 {
        return 1.0;
    }
    (logical_height / FIT_REFERENCE_HEIGHT).clamp(MIN_FIT, 1.0)
}

/// Converts a camera viewport position or size (logical window pixels) to UI node pixels, which
/// Bevy multiplies by `UiScale`. World-anchored UI (overheads, floating numbers, object health)
/// must divide by the scale or it drifts toward the top-left whenever the scale is not 1.
pub(crate) fn viewport_to_ui(
    point: bevy::math::Vec2,
    ui_scale: Option<&bevy::ui::UiScale>,
) -> bevy::math::Vec2 {
    point / ui_scale.map_or(1.0, |scale| scale.0).max(0.01)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn desktop_heights_keep_the_authored_scale() {
        for height in [640.0, 720.0, 1080.0, 1440.0] {
            assert!((viewport_fit(height) - 1.0).abs() < f32::EPSILON);
        }
    }

    #[test]
    fn landscape_phone_scales_down_to_the_reference_height() {
        let fit = viewport_fit(411.0);
        assert!((fit - 411.0 / 640.0).abs() < 1e-6);
        assert!((411.0 / fit - FIT_REFERENCE_HEIGHT).abs() < 1e-3);
    }

    #[test]
    fn viewport_points_map_into_scaled_ui_space() {
        let scale = bevy::ui::UiScale(0.5);
        let point = viewport_to_ui(bevy::math::Vec2::new(400.0, 100.0), Some(&scale));
        assert_eq!(point, bevy::math::Vec2::new(800.0, 200.0));
        assert_eq!(viewport_to_ui(point, None), point);
    }

    #[test]
    fn degenerate_and_tiny_windows_are_bounded() {
        assert!((viewport_fit(0.0) - 1.0).abs() < f32::EPSILON);
        assert!((viewport_fit(f32::NAN) - 1.0).abs() < f32::EPSILON);
        assert!((viewport_fit(100.0) - MIN_FIT).abs() < f32::EPSILON);
    }
}
