//! Pure vertical-scroll geometry shared by flow screens.

use bevy::{
    input::{
        mouse::{MouseScrollUnit, MouseWheel},
        touch::Touches,
    },
    prelude::*,
    ui::{OverflowAxis, ScrollPosition},
    window::PrimaryWindow,
};
use std::ops::Range;

use super::press::{DRAG_CANCEL_DISTANCE, is_sideways};

/// One finger held on the shell: where it started, the scroll area under it, and whether it has
/// travelled far enough to become a scroll drag (which also cancels release activation).
#[derive(Clone, Copy, Debug)]
struct TouchDrag {
    id: u64,
    origin: Vec2,
    /// Vertical travel already applied to the target; `None` until the drag threshold passes.
    applied: Option<f32>,
    target: Option<Entity>,
}

/// Touch drag-scrolling for every vertical shell scroll area (BRL-0196).
///
/// Mouse wheels keep their per-screen handlers; touch has no wheel, so a single generic system
/// scrolls the innermost overflowing `Overflow::scroll_y` node under the finger once the finger
/// passes the same drag distance that cancels a press.
#[derive(Resource, Default, Debug)]
pub(crate) struct TouchDragScroll(Option<TouchDrag>);

/// Innermost (smallest) visible vertical scroll area containing `point` that can scroll.
fn scroll_area_at(
    point_physical: Vec2,
    areas: &Query<(
        Entity,
        &Node,
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
        &mut ScrollPosition,
    )>,
) -> Option<Entity> {
    areas
        .iter()
        .filter(|(_, node, computed, transform, visible, _)| {
            node.overflow.y == OverflowAxis::Scroll
                && visible.get()
                && !computed.is_empty()
                && computed.content_size().y > computed.size().y + 0.5
                && computed.contains_point(**transform, point_physical)
        })
        .min_by(|a, b| {
            let area = |c: &ComputedNode| c.size().x * c.size().y;
            area(a.2).total_cmp(&area(b.2))
        })
        .map(|(entity, ..)| entity)
}

/// Scroll offset after a finger moved `drag_logical` window pixels since its last applied step.
pub(crate) fn dragged_scroll_offset(
    offset: f32,
    drag_logical: f32,
    window_scale_factor: f32,
    computed: &ComputedNode,
) -> f32 {
    let delta = drag_logical * window_scale_factor * computed.inverse_scale_factor();
    clamp_scroll_offset(
        offset - delta,
        computed.content_size().y,
        computed.size().y,
        computed.inverse_scale_factor(),
    )
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(crate) fn drag_scroll_by_touch(
    touches: Option<Res<Touches>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    mut state: ResMut<TouchDragScroll>,
    mut areas: Query<(
        Entity,
        &Node,
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
        &mut ScrollPosition,
    )>,
) {
    let Some(touches) = touches else {
        state.0 = None;
        return;
    };
    let scale_factor = windows.iter().next().map_or(1.0, Window::scale_factor);
    let held = state
        .0
        .and_then(|drag| touches.get_pressed(drag.id).map(|t| (drag, t.position())));
    let Some((mut drag, position)) = held else {
        state.0 = touches.iter().next().map(|touch| TouchDrag {
            id: touch.id(),
            origin: touch.position(),
            applied: None,
            target: scroll_area_at(touch.position() * scale_factor, &areas),
        });
        return;
    };
    let travelled = position - drag.origin;
    if drag.applied.is_none() && travelled.length() <= DRAG_CANCEL_DISTANCE {
        return;
    }
    if drag.applied.is_none() && is_sideways(travelled) {
        // A sideways gesture (slider drag, carousel swipe) never scrolls vertically.
        drag.target = None;
    }
    let step = travelled.y - drag.applied.unwrap_or(0.0);
    drag.applied = Some(travelled.y);
    state.0 = Some(drag);
    if let Some(target) = drag.target
        && let Ok((_, _, computed, _, _, mut scroll)) = areas.get_mut(target)
    {
        let next = dragged_scroll_offset(scroll.0.y, step, scale_factor, computed);
        if (next - scroll.0.y).abs() > f32::EPSILON {
            scroll.0.y = next;
        }
    }
}

pub(crate) fn normalized_wheel_delta<'a>(
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

pub(crate) fn clamp_scroll_offset(
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

pub(crate) fn offset_keeping_interval_visible(
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

    mod touch {
        use super::super::super::{layout_harness, press::ReleaseActivation};
        use bevy::{
            input::touch::{TouchInput, TouchPhase},
            prelude::*,
            ui::ScrollPosition,
        };

        fn scroll_app() -> (App, Entity, Entity) {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins);
            layout_harness::add_layout(&mut app, 914, 411);
            super::super::super::press::add_release_activation(&mut app);
            let area = app
                .world_mut()
                .spawn((
                    Node {
                        width: percent(100),
                        height: percent(100),
                        flex_direction: FlexDirection::Column,
                        overflow: Overflow::scroll_y(),
                        ..default()
                    },
                    ScrollPosition::default(),
                ))
                .id();
            let button = app
                .world_mut()
                .spawn((
                    Button,
                    ReleaseActivation::default(),
                    Node {
                        width: px(400),
                        height: px(300),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    ChildOf(area),
                ))
                .id();
            app.world_mut().spawn((
                Node {
                    height: px(900),
                    flex_shrink: 0.0,
                    ..default()
                },
                ChildOf(area),
            ));
            for _ in 0..3 {
                app.update();
            }
            (app, area, button)
        }

        fn touch(app: &mut App, phase: TouchPhase, position: Vec2) -> bool {
            let window = app
                .world_mut()
                .query_filtered::<Entity, With<Window>>()
                .single(app.world())
                .unwrap();
            app.world_mut().write_message(TouchInput {
                phase,
                position,
                window,
                force: None,
                id: 7,
            });
            app.update();
            let button = app
                .world_mut()
                .query::<&ReleaseActivation>()
                .single(app.world())
                .unwrap();
            button.activated()
        }

        fn scroll(app: &App, area: Entity) -> f32 {
            app.world().get::<ScrollPosition>(area).unwrap().0.y
        }

        #[test]
        fn touch_drag_scrolls_the_area_and_cancels_the_press() {
            let (mut app, area, _) = scroll_app();
            let mut activated = touch(&mut app, TouchPhase::Started, Vec2::new(200.0, 200.0));
            for y in [190.0, 160.0, 130.0, 100.0] {
                activated |= touch(&mut app, TouchPhase::Moved, Vec2::new(200.0, y));
            }
            activated |= touch(&mut app, TouchPhase::Ended, Vec2::new(200.0, 100.0));
            activated |= touch(&mut app, TouchPhase::Ended, Vec2::new(200.0, 100.0));
            assert!(
                !activated,
                "a scroll drag must not activate the control under it"
            );
            assert!(
                (scroll(&app, area) - 100.0).abs() < 1.0,
                "{}",
                scroll(&app, area)
            );

            // Dragging back down past the top clamps at zero.
            touch(&mut app, TouchPhase::Started, Vec2::new(200.0, 100.0));
            touch(&mut app, TouchPhase::Moved, Vec2::new(200.0, 400.0));
            touch(&mut app, TouchPhase::Ended, Vec2::new(200.0, 400.0));
            assert!(scroll(&app, area).abs() < f32::EPSILON);
        }

        #[test]
        fn tap_activates_without_scrolling() {
            let (mut app, area, _) = scroll_app();
            let mut activated = touch(&mut app, TouchPhase::Started, Vec2::new(200.0, 200.0));
            activated |= touch(&mut app, TouchPhase::Moved, Vec2::new(203.0, 196.0));
            activated |= touch(&mut app, TouchPhase::Ended, Vec2::new(203.0, 196.0));
            activated |= touch(&mut app, TouchPhase::Ended, Vec2::new(203.0, 196.0));
            assert!(activated);
            assert!(scroll(&app, area).abs() < f32::EPSILON);
        }
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
