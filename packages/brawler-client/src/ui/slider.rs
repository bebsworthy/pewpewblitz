//! Pointer adjustment of shared slider tracks, including the touch gesture (BRL-0201).
//!
//! A mouse slider jumps to the pointer while the left button is held. Touch cannot do that: a
//! vertical scroll that starts on a slider row must not edit it. A finger that goes down on a
//! slider track therefore waits for the shared drag distance. A mostly sideways drag then follows
//! the finger, a vertical drag is left to `drag_scroll_by_touch`, and an undragged tap sets the
//! tapped position when the finger lifts. Owners keep range, step, units and persistence.

use super::controls::{SLIDER_KNOB_RADIUS, SliderTrack, slider_fraction};
use super::press::{DRAG_CANCEL_DISTANCE, is_sideways};
use bevy::{
    ecs::system::SystemParam,
    input::touch::Touches,
    prelude::*,
    ui::{CalculatedClip, RelativeCursorPosition},
    window::PrimaryWindow,
};

/// One pointer adjustment of a slider this frame. Owners map `fraction` (0..=1 along the track)
/// to their range and defer saving while `held`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct SliderAdjust {
    pub(crate) owner: Entity,
    pub(crate) fraction: f32,
    pub(crate) held: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Phase {
    /// Within the drag distance: still a possible tap.
    Pending,
    /// A sideways drag: the value follows the finger.
    Dragging,
    /// A vertical drag owned by scrolling: the value never changes.
    Scrolling,
}

/// The finger on a slider: its row, and the track's horizontal extent in logical window pixels
/// captured at touch-down so a live UI-scale change cannot feed back into the mapping.
#[derive(Clone, Copy, Debug)]
struct TouchSlide {
    id: u64,
    owner: Entity,
    origin: Vec2,
    last: Vec2,
    track_left: f32,
    track_width: f32,
    phase: Phase,
}

impl TouchSlide {
    fn adjust(&self, held: bool) -> SliderAdjust {
        SliderAdjust {
            owner: self.owner,
            fraction: ((self.last.x - self.track_left) / self.track_width).clamp(0.0, 1.0),
            held,
        }
    }
}

/// The touch slider gesture and this frame's adjustment from it.
#[derive(Resource, Default, Debug)]
pub(crate) struct SliderTouch {
    slide: Option<TouchSlide>,
    adjust: Option<SliderAdjust>,
}

impl SliderTouch {
    /// True while a finger that started on a slider track is still down.
    pub(crate) const fn held(&self) -> bool {
        self.slide.is_some()
    }
}

/// The pointer inputs every slider owner reads: mouse samples and the touch gesture.
#[derive(SystemParam)]
pub(crate) struct SliderPointer<'w, 's> {
    mouse: Res<'w, ButtonInput<MouseButton>>,
    touch: Res<'w, SliderTouch>,
    tracks: Query<'w, 's, (&'static SliderTrack, &'static RelativeCursorPosition)>,
}

impl SliderPointer<'_, '_> {
    pub(crate) fn adjustments(&self) -> Vec<SliderAdjust> {
        slider_adjustments(&self.mouse, self.tracks.iter(), &self.touch)
    }
}

/// This frame's slider adjustments: every track under a pressed, held or just-released left mouse
/// button (the unchanged desktop path), plus at most one touch gesture.
fn slider_adjustments<'a>(
    mouse: &ButtonInput<MouseButton>,
    tracks: impl IntoIterator<Item = (&'a SliderTrack, &'a RelativeCursorPosition)>,
    touch: &SliderTouch,
) -> Vec<SliderAdjust> {
    let mut adjustments = Vec::new();
    if mouse.pressed(MouseButton::Left)
        || mouse.just_pressed(MouseButton::Left)
        || mouse.just_released(MouseButton::Left)
    {
        adjustments.extend(tracks.into_iter().filter_map(|(track, cursor)| {
            slider_fraction(cursor).map(|fraction| SliderAdjust {
                owner: track.0,
                fraction,
                held: mouse.pressed(MouseButton::Left),
            })
        }));
    }
    adjustments.extend(touch.adjust);
    adjustments
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(crate) fn track_touch_sliders(
    touches: Option<Res<Touches>>,
    windows: Query<&Window, With<PrimaryWindow>>,
    tracks: Query<(&SliderTrack, &ComputedNode, &UiGlobalTransform)>,
    rows: Query<(
        &ComputedNode,
        &UiGlobalTransform,
        &InheritedVisibility,
        Option<&CalculatedClip>,
    )>,
    mut state: ResMut<SliderTouch>,
) {
    state.adjust = None;
    let Some(touches) = touches else {
        state.slide = None;
        return;
    };
    if let Some(mut slide) = state.slide {
        let Some(touch) = touches.get_pressed(slide.id) else {
            // A lifted finger finishes a tap or drag; a cancelled one changes nothing.
            state.slide = None;
            if touches.just_released(slide.id) && slide.phase != Phase::Scrolling {
                state.adjust = Some(slide.adjust(false));
            }
            return;
        };
        slide.last = touch.position();
        let travelled = slide.last - slide.origin;
        if slide.phase == Phase::Pending && travelled.length() > DRAG_CANCEL_DISTANCE {
            slide.phase = if is_sideways(travelled) {
                Phase::Dragging
            } else {
                Phase::Scrolling
            };
        }
        if slide.phase == Phase::Dragging {
            state.adjust = Some(slide.adjust(true));
        }
        state.slide = Some(slide);
        return;
    }
    let Some(touch) = touches.iter_just_pressed().next() else {
        return;
    };
    let scale_factor = windows.iter().next().map_or(1.0, Window::scale_factor);
    let position = touch.position();
    let physical = position * scale_factor;
    // Hit-tested from layout rather than `Interaction`: a quick tap can start and end within one
    // frame, before UI focus ever sees the finger held.
    let slide = tracks.iter().find_map(|(track, computed, transform)| {
        let (row, row_transform, visible, clip) = rows.get(track.0).ok()?;
        if !visible.get()
            || computed.is_empty()
            || !row.contains_point(*row_transform, physical)
            || clip.is_some_and(|clip| !clip.clip.contains(physical))
        {
            return None;
        }
        let (_, _, center) = transform.to_scale_angle_translation();
        let track_width = computed.size().x / scale_factor;
        let track_left = center.x / scale_factor - track_width / 2.0;
        let overhang = SLIDER_KNOB_RADIUS / computed.inverse_scale_factor() / scale_factor;
        (position.x >= track_left - overhang && position.x <= track_left + track_width + overhang)
            .then_some(TouchSlide {
                id: touch.id(),
                owner: track.0,
                origin: position,
                last: position,
                track_left,
                track_width,
                phase: Phase::Pending,
            })
    });
    match slide {
        // The finger already lifted this frame: a tap.
        Some(slide) if touches.get_pressed(slide.id).is_none() => {
            state.adjust = touches.just_released(slide.id).then(|| slide.adjust(false));
        }
        slide => state.slide = slide,
    }
}

#[cfg(test)]
mod tests {
    use super::super::{
        controls::{ControlValue, SLIDER_ROW_MIN_HEIGHT, spawn_control},
        layout_harness::{self, node_rect},
        press::add_release_activation,
        viewport::viewport_fit,
    };
    use super::*;
    use bevy::{
        input::touch::{TouchInput, TouchPhase},
        ui::UiScale,
    };

    struct Rig {
        app: App,
        area: Entity,
        row: Entity,
        track: Rect,
    }

    /// A slider row near the top of a scrolling column on an 868x411 phone viewport.
    fn rig(ui_scale: f32) -> Rig {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        layout_harness::add_layout(&mut app, 868, 411);
        add_release_activation(&mut app);
        app.world_mut().resource_mut::<UiScale>().0 = ui_scale;
        let area = app
            .world_mut()
            .spawn((
                Node {
                    width: percent(100),
                    height: percent(100),
                    flex_direction: FlexDirection::Column,
                    padding: UiRect::all(px(24)),
                    overflow: Overflow::scroll_y(),
                    ..default()
                },
                ScrollPosition::default(),
            ))
            .id();
        let mut row = Entity::PLACEHOLDER;
        app.world_mut()
            .commands()
            .entity(area)
            .with_children(|column| {
                row = spawn_control(
                    column,
                    &TextFont::default(),
                    "Master volume",
                    ControlValue::Slider {
                        fraction: 0.0,
                        label: String::new(),
                    },
                );
                column.spawn(Node {
                    height: px(2000),
                    flex_shrink: 0.0,
                    ..default()
                });
            });
        app.world_mut().flush();
        for _ in 0..3 {
            app.update();
        }
        let track = app
            .world_mut()
            .query::<(Entity, &SliderTrack)>()
            .iter(app.world())
            .find(|(_, track)| track.0 == row)
            .map(|(entity, _)| entity)
            .expect("slider track");
        let track = node_rect(app.world(), track).expect("laid-out track");
        Rig {
            app,
            area,
            row,
            track,
        }
    }

    impl Rig {
        /// Logical window point `fraction` along the track, at the track's vertical center.
        fn along(&self, fraction: f32) -> Vec2 {
            Vec2::new(
                self.track.min.x + self.track.width() * fraction,
                self.track.center().y,
            )
        }

        fn touch(&mut self, phase: TouchPhase, position: Vec2) -> Option<SliderAdjust> {
            layout_harness::touch(&mut self.app, phase, position);
            self.app.world().resource::<SliderTouch>().adjust
        }

        fn scroll(&self) -> f32 {
            self.app
                .world()
                .get::<ScrollPosition>(self.area)
                .unwrap()
                .0
                .y
        }
    }

    fn assert_fraction(adjust: Option<SliderAdjust>, expected: f32, held: bool) {
        let adjust = adjust.expect("an adjustment");
        assert!(
            (adjust.fraction - expected).abs() < 0.01,
            "expected {expected}, got {}",
            adjust.fraction
        );
        assert_eq!(adjust.held, held);
    }

    #[test]
    fn sideways_drag_follows_the_finger_and_finishes_on_release() {
        let fit = viewport_fit(411.0);
        let mut rig = rig(fit);
        let start = rig.along(0.2);
        assert_eq!(rig.touch(TouchPhase::Started, start), None);
        assert_eq!(
            rig.touch(TouchPhase::Moved, start + Vec2::new(6.0, 1.0)),
            None,
            "within the drag distance the gesture may still be a tap or a scroll"
        );
        let adjust = rig.touch(TouchPhase::Moved, rig.along(0.5));
        assert_eq!(adjust.map(|adjust| adjust.owner), Some(rig.row));
        assert_fraction(adjust, 0.5, true);
        assert!(rig.app.world().resource::<SliderTouch>().held());
        // Drifting off the row vertically keeps adjusting, and the ends clamp.
        let past_end = rig.along(1.3) + Vec2::new(0.0, 40.0);
        assert_fraction(rig.touch(TouchPhase::Moved, past_end), 1.0, true);
        assert_fraction(rig.touch(TouchPhase::Moved, rig.along(0.8)), 0.8, true);
        assert_fraction(rig.touch(TouchPhase::Ended, rig.along(0.8)), 0.8, false);
        assert!(!rig.app.world().resource::<SliderTouch>().held());
        assert_eq!(rig.touch(TouchPhase::Ended, rig.along(0.8)), None);
        assert!(
            rig.scroll().abs() < f32::EPSILON,
            "sideways drags never scroll"
        );
    }

    #[test]
    fn tap_sets_the_tapped_position_on_release_only() {
        let mut rig = rig(viewport_fit(411.0));
        assert_eq!(rig.touch(TouchPhase::Started, rig.along(0.75)), None);
        assert_eq!(
            rig.touch(TouchPhase::Moved, rig.along(0.75) + Vec2::splat(3.0)),
            None
        );
        let adjust = rig.touch(TouchPhase::Ended, rig.along(0.75));
        assert_fraction(adjust, 0.75 + 3.0 / rig.track.width(), false);

        // A tap on the label part of the row focuses it but never edits the value.
        let label = Vec2::new(rig.track.min.x - 120.0, rig.track.center().y);
        assert_eq!(rig.touch(TouchPhase::Started, label), None);
        assert_eq!(rig.touch(TouchPhase::Ended, label), None);
    }

    #[test]
    fn a_tap_that_starts_and_ends_within_one_frame_still_sets_the_value() {
        let mut rig = rig(viewport_fit(411.0));
        let at = rig.along(0.6);
        let window = rig
            .app
            .world_mut()
            .query_filtered::<Entity, With<PrimaryWindow>>()
            .single(rig.app.world())
            .unwrap();
        for phase in [TouchPhase::Started, TouchPhase::Ended] {
            rig.app.world_mut().write_message(TouchInput {
                phase,
                position: at,
                window,
                force: None,
                id: 3,
            });
        }
        rig.app.update();
        assert_fraction(rig.app.world().resource::<SliderTouch>().adjust, 0.6, false);
        assert!(!rig.app.world().resource::<SliderTouch>().held());
    }

    #[test]
    fn rows_clipped_by_their_scroll_area_ignore_touches_outside_the_clip() {
        let mut rig = rig(viewport_fit(411.0));
        rig.app
            .world_mut()
            .get_mut::<ScrollPosition>(rig.area)
            .unwrap()
            .0
            .y = 40.0;
        rig.app.update();
        rig.app.update();
        let row = node_rect(rig.app.world(), rig.row).unwrap();
        let x = rig.along(0.5).x;
        // The area's top padding clips the scrolled row; only its visible part is touchable.
        let hidden = Vec2::new(x, row.min.y + 2.0);
        assert!(hidden.y < 24.0 * viewport_fit(411.0));
        rig.touch(TouchPhase::Started, hidden);
        assert_eq!(rig.touch(TouchPhase::Ended, hidden), None);
        let visible = Vec2::new(x, row.max.y - 4.0);
        rig.touch(TouchPhase::Started, visible);
        assert!(rig.touch(TouchPhase::Ended, visible).is_some());
    }

    #[test]
    fn vertical_drag_on_a_slider_scrolls_without_changing_the_value() {
        let mut rig = rig(viewport_fit(411.0));
        let start = rig.along(0.4);
        let mut adjustments = vec![rig.touch(TouchPhase::Started, start)];
        for dy in [-10.0, -40.0, -80.0, -120.0] {
            adjustments.push(rig.touch(TouchPhase::Moved, start + Vec2::new(4.0, dy)));
        }
        adjustments.push(rig.touch(TouchPhase::Ended, start + Vec2::new(4.0, -120.0)));
        adjustments.push(rig.touch(TouchPhase::Ended, start + Vec2::new(4.0, -120.0)));
        assert!(adjustments.iter().all(Option::is_none), "{adjustments:?}");
        assert!(
            rig.scroll() > 100.0,
            "the column scrolled: {}",
            rig.scroll()
        );
    }

    #[test]
    fn finger_maps_through_the_laid_out_track_at_every_ui_scale() {
        for ui_scale in [1.0, viewport_fit(411.0), 0.8 * viewport_fit(411.0)] {
            let mut rig = rig(ui_scale);
            rig.touch(TouchPhase::Started, rig.along(0.25));
            assert_fraction(rig.touch(TouchPhase::Ended, rig.along(0.25)), 0.25, false);
            // The knob overhang past the left end still lands on the minimum.
            let overhang = SLIDER_KNOB_RADIUS * ui_scale * 0.8;
            let before = rig.along(0.0) - Vec2::new(overhang, 0.0);
            rig.touch(TouchPhase::Started, before);
            assert_fraction(rig.touch(TouchPhase::Ended, before), 0.0, false);
        }
    }

    #[test]
    fn slider_touch_target_is_at_least_48_dp_at_phone_scale() {
        let rig = rig(viewport_fit(411.0));
        // The harness window has a scale factor of 1, so logical pixels are dp.
        let row = node_rect(rig.app.world(), rig.row).unwrap();
        assert!(row.height() >= 48.0, "row is {} dp tall", row.height());
        assert!(
            rig.track.width() >= 48.0,
            "track is {} dp wide",
            rig.track.width()
        );
        assert!(SLIDER_ROW_MIN_HEIGHT * viewport_fit(411.0) >= 48.0);
    }

    #[test]
    fn mouse_samples_are_unchanged_and_merge_with_touch() {
        let owner = Entity::from_raw_u32(7).unwrap();
        let track = SliderTrack(owner);
        let cursor = RelativeCursorPosition {
            cursor_over: true,
            normalized: Some(Vec2::new(0.25, 0.0)),
        };
        let mut mouse = ButtonInput::<MouseButton>::default();
        let idle = SliderTouch::default();
        assert!(slider_adjustments(&mouse, [(&track, &cursor)], &idle).is_empty());
        mouse.press(MouseButton::Left);
        assert_eq!(
            slider_adjustments(&mouse, [(&track, &cursor)], &idle),
            [SliderAdjust {
                owner,
                fraction: 0.75,
                held: true
            }]
        );
        let touch = SliderTouch {
            slide: None,
            adjust: Some(SliderAdjust {
                owner,
                fraction: 0.1,
                held: false,
            }),
        };
        mouse.reset_all();
        assert_eq!(
            slider_adjustments(&mouse, [(&track, &cursor)], &touch),
            touch.adjust.into_iter().collect::<Vec<_>>()
        );
    }
}
