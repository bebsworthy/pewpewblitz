//! Pointer and touch activation on release, with drag-cancel.
//!
//! Bevy's `Interaction` reports `Pressed` for as long as a pointer is held, which conflicts with
//! touch drag-scrolling. Shell controls instead act once when a press ends without dragging.
//! Keyboard and gamepad activation stay with their owning screens.
//!
//! A touch release that no control saw pressed (the finger landed before the control was laid out
//! where it is now, or the control was rebuilt mid-press) is hit-tested against the current layout
//! with Bevy UI's own picking rules, so the tap still activates the control under it (BRL-wr0s).

use bevy::{
    camera::Camera,
    ecs::system::SystemParam,
    input::touch::Touches,
    prelude::*,
    ui::{
        ComputedUiTargetCamera, FocusPolicy, OverrideClip, UiStack, UiSystems, clip_check_recursive,
    },
    window::PrimaryWindow,
};

/// Logical pixels a held pointer may travel before the press becomes a drag.
pub(crate) const DRAG_CANCEL_DISTANCE: f32 = 12.0;

/// A drag that has passed the cancel distance belongs to horizontal controls (sliders, carousels)
/// when it travelled mostly sideways, and to vertical scrolling otherwise.
pub(crate) fn is_sideways(travelled: Vec2) -> bool {
    travelled.x.abs() > travelled.y.abs()
}

/// A press in progress: how it started and whether it has become a drag.
#[derive(Clone, Copy, Debug)]
struct HeldPress {
    touch: bool,
    cancelled: bool,
    origin: Option<Vec2>,
}

/// Release-activation state for one pointer-activatable control.
#[derive(Component, Clone, Copy, Debug, Default)]
pub(crate) struct ReleaseActivation {
    held: Option<HeldPress>,
    activated: bool,
}

impl ReleaseActivation {
    /// True only on the frame a press ends over the control without dragging.
    pub(crate) const fn activated(&self) -> bool {
        self.activated
    }

    /// A control whose press ended over it this frame, for consumers' focused tests.
    #[cfg(test)]
    pub(crate) const fn activated_for_test() -> Self {
        Self {
            held: None,
            activated: true,
        }
    }

    /// Advances the gesture by one observed frame. `dragged` reports a touch released this frame
    /// after travelling past the cancel distance, which a frame's single pointer sample can miss.
    fn observe(
        &mut self,
        interaction: Interaction,
        pointer: Option<Vec2>,
        touch: bool,
        dragged: bool,
    ) {
        self.activated = false;
        let pressed = interaction == Interaction::Pressed;
        match (&mut self.held, pressed) {
            (None, true) => {
                self.held = Some(HeldPress {
                    touch,
                    cancelled: dragged,
                    origin: pointer,
                });
            }
            (Some(held), true) => {
                if dragged
                    || matches!((held.origin, pointer), (Some(origin), Some(pointer))
                        if origin.distance(pointer) > DRAG_CANCEL_DISTANCE)
                {
                    held.cancelled = true;
                }
            }
            (Some(held), false) => {
                // A mouse release over the control leaves it hovered; touch has no position
                // after release, so its drag-cancel check is the only containment evidence.
                self.activated = !held.cancelled
                    && !dragged
                    && (interaction == Interaction::Hovered || held.touch);
                self.held = None;
            }
            (None, false) => {}
        }
    }

    /// Activates for a touch release that UI focus never reported pressed on this control.
    fn activate_unseen_tap(&mut self) {
        self.activated = true;
        self.held = None;
    }
}

/// Installs the shared release tracker, touch drag-scrolling and touch slider gesture once,
/// whichever shell owner composes first; all three share the press/drag distance policy.
pub(crate) struct ReleaseActivationPlugin;

impl Plugin for ReleaseActivationPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<super::scroll::TouchDragScroll>()
            .init_resource::<super::slider::SliderTouch>()
            .add_systems(
                PreUpdate,
                (
                    track_release_activation,
                    super::scroll::drag_scroll_by_touch,
                    super::slider::track_touch_sliders,
                )
                    .after(UiSystems::Focus),
            );
    }
}

pub(crate) fn add_release_activation(app: &mut App) {
    if !app.is_plugin_added::<ReleaseActivationPlugin>() {
        app.add_plugins(ReleaseActivationPlugin);
    }
}

/// One node as UI picking reads it: geometry, target camera, visibility and focus policy.
type PickNode = (
    &'static ComputedNode,
    &'static UiGlobalTransform,
    &'static ComputedUiTargetCamera,
    Option<&'static InheritedVisibility>,
    Option<&'static FocusPolicy>,
);

/// Bevy UI's pointer picking (`ui_focus_system`) replayed for one position against the current
/// layout: top layer first, hidden and clipped nodes skipped, stopping at the first node that
/// blocks focus.
#[derive(SystemParam)]
struct UiPicker<'w, 's> {
    stack: Option<Res<'w, UiStack>>,
    cameras: Query<'w, 's, &'static Camera>,
    nodes: Query<'w, 's, PickNode>,
    clipping: Query<
        'w,
        's,
        (
            &'static ComputedNode,
            &'static UiGlobalTransform,
            &'static Node,
        ),
    >,
    parents: Query<'w, 's, &'static ChildOf, Without<OverrideClip>>,
}

impl UiPicker<'_, '_> {
    /// Nodes UI focus would press at a physical window position, topmost first.
    fn pressed_at(&self, physical: Vec2) -> Vec<Entity> {
        let Some(stack) = self.stack.as_deref() else {
            return Vec::new();
        };
        let mut picked = Vec::new();
        for slice in stack
            .partition
            .iter()
            .rev()
            .map(|range| &stack.uinodes[range.clone()])
        {
            let Some(camera) = self
                .nodes
                .get(slice[0])
                .ok()
                .and_then(|(_, _, target, ..)| target.get())
                .and_then(|camera| self.cameras.get(camera).ok())
            else {
                continue;
            };
            let point = physical
                - camera
                    .physical_viewport_rect()
                    .map(|rect| rect.min.as_vec2())
                    .unwrap_or_default();
            for &entity in slice.iter().rev() {
                let Ok((node, transform, _, visibility, policy)) = self.nodes.get(entity) else {
                    continue;
                };
                if !visibility.is_some_and(|visibility| visibility.get())
                    || !node.contains_point(*transform, point)
                    || !clip_check_recursive(point, entity, &self.clipping, &self.parents)
                {
                    continue;
                }
                picked.push(entity);
                if policy.is_none_or(|policy| *policy == FocusPolicy::Block) {
                    return picked;
                }
            }
        }
        picked
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn track_release_activation(
    windows: Query<&Window, With<PrimaryWindow>>,
    touches: Option<Res<Touches>>,
    mut controls: Query<(Entity, &Interaction, &mut ReleaseActivation)>,
    picker: UiPicker,
    mut held_touches: Local<Vec<u64>>,
) {
    let touches = touches.as_deref();
    let held_before = core::mem::replace(
        &mut *held_touches,
        touches.map_or_else(Vec::new, |touches| {
            touches.iter().map(bevy::input::touch::Touch::id).collect()
        }),
    );
    let touch_position = touches.and_then(Touches::first_pressed_position);
    let pointer = touch_position.or_else(|| windows.iter().next()?.cursor_position());
    let touch = touch_position.is_some();
    let travelled = |touch: &bevy::input::touch::Touch| {
        touch.start_position().distance(touch.position()) > DRAG_CANCEL_DISTANCE
    };
    let dragged = touches.is_some_and(|touches| touches.iter_just_released().any(travelled));
    // A release counts as unseen only when UI focus pressed nothing during the gesture: no control
    // holds a press from an earlier frame or was pressed by this frame's same-frame tap.
    let unseen = !controls.iter().any(|(_, interaction, activation)| {
        activation.held.is_some() || *interaction == Interaction::Pressed
    });
    for (_, interaction, mut activation) in &mut controls {
        // Owners poll `activated()` each frame; change detection would only add noise.
        activation
            .bypass_change_detection()
            .observe(*interaction, pointer, touch, dragged);
    }
    let Some(touches) = touches.filter(|_| unseen) else {
        return;
    };
    let scale_factor = windows.iter().next().map_or(1.0, Window::scale_factor);
    // Only a release that ends a real press (started this frame or held last frame) is a tap; a
    // stray `Ended` for a finger already lifted is not.
    let taps = touches.iter_just_released().filter(|released| {
        !travelled(released)
            && (touches.just_pressed(released.id()) || held_before.contains(&released.id()))
    });
    for released in taps {
        let at_start = picker.pressed_at(released.start_position() * scale_factor);
        let at_end = picker.pressed_at(released.position() * scale_factor);
        for entity in at_end
            .into_iter()
            .filter(|entity| at_start.contains(entity))
        {
            if let Ok((_, _, mut activation)) = controls.get_mut(entity) {
                activation.bypass_change_detection().activate_unseen_tap();
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn run(steps: &[(Interaction, Option<Vec2>, bool)]) -> Vec<bool> {
        let mut activation = ReleaseActivation::default();
        steps
            .iter()
            .map(|(interaction, pointer, touch)| {
                activation.observe(*interaction, *pointer, *touch, false);
                activation.activated()
            })
            .collect()
    }

    #[test]
    fn press_does_not_activate_and_release_over_target_activates_once() {
        let at = Some(Vec2::new(100.0, 100.0));
        assert_eq!(
            run(&[
                (Interaction::Hovered, at, false),
                (Interaction::Pressed, at, false),
                (Interaction::Pressed, at, false),
                (Interaction::Hovered, at, false),
                (Interaction::Hovered, at, false),
            ]),
            [false, false, false, true, false]
        );
    }

    #[test]
    fn drag_beyond_threshold_cancels_even_when_released_over_target() {
        let start = Vec2::new(100.0, 100.0);
        let dragged = start + Vec2::new(0.0, DRAG_CANCEL_DISTANCE + 1.0);
        assert_eq!(
            run(&[
                (Interaction::Pressed, Some(start), true),
                (Interaction::Pressed, Some(dragged), true),
                (Interaction::None, None, false),
            ]),
            [false, false, false]
        );
        assert_eq!(
            run(&[
                (Interaction::Pressed, Some(start), false),
                (Interaction::Pressed, Some(dragged), false),
                (Interaction::Pressed, Some(start), false),
                (Interaction::Hovered, Some(start), false),
            ]),
            [false, false, false, false]
        );
    }

    #[test]
    fn small_jitter_still_activates_and_touch_release_has_no_position() {
        let start = Vec2::new(100.0, 100.0);
        assert_eq!(
            run(&[
                (Interaction::Pressed, Some(start), true),
                (Interaction::Pressed, Some(start + Vec2::splat(4.0)), true),
                (Interaction::None, None, false),
            ]),
            [false, false, true]
        );
    }

    #[test]
    fn mouse_release_off_target_does_not_activate() {
        let at = Some(Vec2::new(10.0, 10.0));
        assert_eq!(
            run(&[
                (Interaction::Pressed, at, false),
                (Interaction::None, at, false),
            ]),
            [false, false]
        );
    }

    #[test]
    fn schedule_tracker_fires_on_the_release_frame_only() {
        let mut app = App::new();
        app.add_systems(Update, track_release_activation);
        let button = app
            .world_mut()
            .spawn((Interaction::Pressed, ReleaseActivation::default()))
            .id();
        app.update();
        assert!(
            !app.world()
                .get::<ReleaseActivation>(button)
                .unwrap()
                .activated()
        );
        *app.world_mut().get_mut::<Interaction>(button).unwrap() = Interaction::Hovered;
        app.update();
        assert!(
            app.world()
                .get::<ReleaseActivation>(button)
                .unwrap()
                .activated()
        );
        app.update();
        assert!(
            !app.world()
                .get::<ReleaseActivation>(button)
                .unwrap()
                .activated()
        );
    }

    /// BRL-wr0s: real touch input through Bevy UI focus on a laid-out 868x411 phone window.
    mod touch {
        use super::super::{ReleaseActivation, add_release_activation};
        use crate::ui::layout_harness::{self, node_rect};
        use bevy::{
            input::touch::{TouchInput, TouchPhase},
            prelude::*,
            ui::FocusPolicy,
            window::PrimaryWindow,
        };

        fn button(app: &mut App, left: f32, top: f32) -> Entity {
            app.world_mut()
                .spawn((
                    Button,
                    ReleaseActivation::default(),
                    Node {
                        position_type: PositionType::Absolute,
                        left: px(left),
                        top: px(top),
                        width: px(200),
                        height: px(80),
                        ..default()
                    },
                ))
                .id()
        }

        fn rig() -> (App, Entity) {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins);
            layout_harness::add_layout(&mut app, 868, 411);
            add_release_activation(&mut app);
            let button = button(&mut app, 100.0, 100.0);
            for _ in 0..3 {
                app.update();
            }
            (app, button)
        }

        fn center(app: &App, entity: Entity) -> Vec2 {
            node_rect(app.world(), entity).unwrap().center()
        }

        fn send(app: &mut App, phase: TouchPhase, position: Vec2) {
            let window = app
                .world_mut()
                .query_filtered::<Entity, With<PrimaryWindow>>()
                .single(app.world())
                .unwrap();
            app.world_mut().write_message(TouchInput {
                phase,
                position,
                window,
                force: None,
                id: 5,
            });
        }

        /// Runs one frame and returns the activated controls.
        fn frame(app: &mut App) -> Vec<Entity> {
            app.update();
            app.world_mut()
                .query::<(Entity, &ReleaseActivation)>()
                .iter(app.world())
                .filter(|(_, activation)| activation.activated())
                .map(|(entity, _)| entity)
                .collect()
        }

        /// Runs `frames` frames and returns every activation, frame by frame.
        fn activations(app: &mut App, frames: usize) -> Vec<Entity> {
            (0..frames).flat_map(|_| frame(app)).collect()
        }

        #[test]
        fn a_first_tap_that_starts_and_ends_within_one_frame_activates_once() {
            let (mut app, button) = rig();
            let at = center(&app, button);
            send(&mut app, TouchPhase::Started, at);
            send(&mut app, TouchPhase::Ended, at);
            assert_eq!(activations(&mut app, 4), [button]);
        }

        #[test]
        fn a_press_and_release_on_separate_frames_activates_once() {
            let (mut app, button) = rig();
            let at = center(&app, button);
            send(&mut app, TouchPhase::Started, at);
            let mut seen = frame(&mut app);
            send(&mut app, TouchPhase::Ended, at);
            seen.extend(activations(&mut app, 4));
            assert_eq!(seen, [button]);
        }

        #[test]
        fn a_control_rebuilt_between_press_and_release_activates_once() {
            let (mut app, old) = rig();
            let at = center(&app, old);
            send(&mut app, TouchPhase::Started, at);
            let mut seen = frame(&mut app);
            app.world_mut().despawn(old);
            let rebuilt = button(&mut app, 100.0, 100.0);
            seen.extend(frame(&mut app));
            send(&mut app, TouchPhase::Ended, at);
            seen.extend(activations(&mut app, 4));
            assert_eq!(seen, [rebuilt]);
        }

        #[test]
        fn a_same_frame_tap_that_drags_or_misses_does_not_activate() {
            let (mut app, button) = rig();
            let at = center(&app, button);
            send(&mut app, TouchPhase::Started, at);
            send(
                &mut app,
                TouchPhase::Moved,
                at + Vec2::new(0.0, super::super::DRAG_CANCEL_DISTANCE + 1.0),
            );
            send(&mut app, TouchPhase::Ended, at);
            assert!(activations(&mut app, 3).is_empty());
            let outside = node_rect(app.world(), button).unwrap().max + Vec2::splat(20.0);
            send(&mut app, TouchPhase::Started, outside);
            send(&mut app, TouchPhase::Ended, outside);
            assert!(activations(&mut app, 3).is_empty());
        }

        #[test]
        fn a_same_frame_tap_under_a_blocking_overlay_does_not_activate_the_control_below() {
            let (mut app, below) = rig();
            let at = center(&app, below);
            // A modal scrim on a higher layer, as shell and flow overlays draw over a screen.
            app.world_mut().spawn((
                Node {
                    position_type: PositionType::Absolute,
                    width: percent(100),
                    height: percent(100),
                    ..default()
                },
                FocusPolicy::Block,
                GlobalZIndex(500),
            ));
            for _ in 0..3 {
                app.update();
            }
            send(&mut app, TouchPhase::Started, at);
            send(&mut app, TouchPhase::Ended, at);
            assert!(activations(&mut app, 3).is_empty());
            // The overlay's own control on top still takes the tap.
            let top = button(&mut app, 100.0, 100.0);
            app.world_mut().entity_mut(top).insert(GlobalZIndex(600));
            for _ in 0..3 {
                app.update();
            }
            send(&mut app, TouchPhase::Started, at);
            send(&mut app, TouchPhase::Ended, at);
            assert_eq!(activations(&mut app, 3), [top]);
        }

        #[test]
        fn a_press_no_control_saw_hits_only_the_topmost_unblocked_control_on_release() {
            let mut app = App::new();
            app.add_plugins(MinimalPlugins);
            layout_harness::add_layout(&mut app, 868, 411);
            add_release_activation(&mut app);
            app.update();
            // The finger lands before anything is laid out under it, as on a slow launch frame.
            let at = Vec2::new(200.0, 140.0);
            send(&mut app, TouchPhase::Started, at);
            app.update();
            let below = button(&mut app, 100.0, 100.0);
            let scrim = app
                .world_mut()
                .spawn((
                    Node {
                        position_type: PositionType::Absolute,
                        width: percent(100),
                        height: percent(100),
                        ..default()
                    },
                    FocusPolicy::Block,
                    GlobalZIndex(500),
                ))
                .id();
            for _ in 0..3 {
                app.update();
            }
            send(&mut app, TouchPhase::Ended, at);
            assert!(activations(&mut app, 3).is_empty(), "the overlay blocks it");

            app.world_mut().despawn(scrim);
            send(&mut app, TouchPhase::Started, Vec2::new(800.0, 380.0));
            app.update();
            let top = button(&mut app, 120.0, 110.0);
            app.world_mut().entity_mut(top).insert(GlobalZIndex(600));
            for _ in 0..3 {
                app.update();
            }
            // Started outside both controls: a release over them is not a tap on them.
            send(&mut app, TouchPhase::Ended, at);
            assert!(activations(&mut app, 3).is_empty());
            // A finger that lands on the top control and lifts within one frame takes it alone.
            send(&mut app, TouchPhase::Started, at);
            send(&mut app, TouchPhase::Ended, at);
            assert_eq!(activations(&mut app, 3), [top]);
            assert_ne!(top, below);
        }

        #[test]
        fn a_same_frame_tap_on_a_hidden_or_clipped_control_does_not_activate() {
            let (mut app, button) = rig();
            let at = center(&app, button);
            app.world_mut()
                .entity_mut(button)
                .insert(Visibility::Hidden);
            for _ in 0..2 {
                app.update();
            }
            send(&mut app, TouchPhase::Started, at);
            send(&mut app, TouchPhase::Ended, at);
            assert!(activations(&mut app, 3).is_empty());

            // A control scrolled outside its clipping parent.
            let clip = app
                .world_mut()
                .spawn(Node {
                    position_type: PositionType::Absolute,
                    left: px(400),
                    top: px(100),
                    width: px(200),
                    height: px(40),
                    overflow: Overflow::clip(),
                    ..default()
                })
                .id();
            let clipped = app
                .world_mut()
                .spawn((
                    Button,
                    ReleaseActivation::default(),
                    Node {
                        width: px(200),
                        height: px(80),
                        flex_shrink: 0.0,
                        ..default()
                    },
                    ChildOf(clip),
                ))
                .id();
            for _ in 0..3 {
                app.update();
            }
            let hidden_part = node_rect(app.world(), clipped).unwrap().max - Vec2::splat(10.0);
            send(&mut app, TouchPhase::Started, hidden_part);
            send(&mut app, TouchPhase::Ended, hidden_part);
            assert!(activations(&mut app, 3).is_empty());
        }

        #[test]
        fn a_mouse_click_still_activates_once_through_ui_focus() {
            use bevy::input::{ButtonState, mouse::MouseButtonInput};
            let (mut app, button) = rig();
            let at = center(&app, button);
            let window = app
                .world_mut()
                .query_filtered::<Entity, With<PrimaryWindow>>()
                .single(app.world())
                .unwrap();
            app.world_mut()
                .get_mut::<Window>(window)
                .unwrap()
                .set_cursor_position(Some(at));
            let mut seen = Vec::new();
            for state in [ButtonState::Pressed, ButtonState::Released] {
                app.world_mut().write_message(MouseButtonInput {
                    button: MouseButton::Left,
                    state,
                    window,
                });
                seen.extend(frame(&mut app));
            }
            seen.extend(activations(&mut app, 2));
            assert_eq!(seen, [button]);
        }
    }
}
