//! Pure vertical-scroll geometry shared by flow screens.

use bevy::input::mouse::{MouseScrollUnit, MouseWheel};
use std::ops::Range;

pub(super) fn normalized_wheel_delta<'a>(
    events: impl IntoIterator<Item = &'a MouseWheel>,
    line_multiplier: f32,
) -> f32 {
    events
        .into_iter()
        .map(|event| match event.unit {
            MouseScrollUnit::Line => event.y * line_multiplier,
            MouseScrollUnit::Pixel => event.y,
        })
        .sum()
}

pub(super) fn clamp_scroll_offset(
    offset: f32,
    content_extent: f32,
    viewport_extent: f32,
    inverse_scale_factor: f32,
) -> f32 {
    if content_extent <= 0.0 || viewport_extent <= 0.0 {
        return offset.max(0.0);
    }
    let maximum = ((content_extent - viewport_extent) * inverse_scale_factor).max(0.0);
    offset.clamp(0.0, maximum)
}

pub(super) fn offset_keeping_interval_visible(
    offset: f32,
    viewport: Range<f32>,
    focused: Range<f32>,
    inverse_scale_factor: f32,
) -> f32 {
    let viewport_extent = ((viewport.end - viewport.start) * inverse_scale_factor).max(0.0);
    let focused_start = offset + (focused.start - viewport.start) * inverse_scale_factor;
    let focused_end = offset + (focused.end - viewport.start) * inverse_scale_factor;
    let minimum = focused_end - viewport_extent;
    let maximum = focused_start;

    if minimum <= maximum {
        offset.clamp(minimum, maximum)
    } else if focused.start < viewport.start {
        maximum
    } else if focused.end > viewport.end {
        minimum
    } else {
        offset
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::{ecs::entity::Entity, input::touch::TouchPhase};

    fn wheel(unit: MouseScrollUnit, y: f32) -> MouseWheel {
        MouseWheel {
            unit,
            x: 0.0,
            y,
            window: Entity::PLACEHOLDER,
            phase: TouchPhase::Moved,
        }
    }

    fn assert_close(actual: f32, expected: f32) {
        assert!(
            (actual - expected).abs() <= f32::EPSILON,
            "expected {expected}, got {actual}"
        );
    }

    #[test]
    fn wheel_delta_normalizes_lines_and_preserves_pixels() {
        let events = [
            wheel(MouseScrollUnit::Line, 2.0),
            wheel(MouseScrollUnit::Pixel, 3.5),
        ];

        assert_close(normalized_wheel_delta(&events, 24.0), 51.5);
    }

    #[test]
    fn scroll_offset_is_bounded_by_scaled_overflow() {
        assert_close(clamp_scroll_offset(-5.0, 300.0, 100.0, 0.5), 0.0);
        assert_close(clamp_scroll_offset(40.0, 300.0, 100.0, 0.5), 40.0);
        assert_close(clamp_scroll_offset(140.0, 300.0, 100.0, 0.5), 100.0);
        assert_close(clamp_scroll_offset(10.0, 80.0, 100.0, 1.0), 0.0);
        assert_close(clamp_scroll_offset(10.0, 0.0, 0.0, 1.0), 10.0);
    }

    #[test]
    fn focused_interval_moves_only_when_outside_viewport() {
        let viewport = 100.0..200.0;

        assert_close(
            offset_keeping_interval_visible(40.0, viewport.clone(), 120.0..150.0, 1.0),
            40.0,
        );
        assert_close(
            offset_keeping_interval_visible(40.0, viewport.clone(), 80.0..110.0, 1.0),
            20.0,
        );
        assert_close(
            offset_keeping_interval_visible(40.0, viewport.clone(), 190.0..230.0, 1.0),
            70.0,
        );
        assert_close(
            offset_keeping_interval_visible(40.0, viewport, 80.0..110.0, 0.5),
            30.0,
        );
    }
}
