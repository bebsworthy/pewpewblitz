//! Pointer and touch activation on release, with drag-cancel.
//!
//! Bevy's `Interaction` reports `Pressed` for as long as a pointer is held, which conflicts with
//! touch drag-scrolling. Shell controls instead act once when a press ends without dragging.
//! Keyboard and gamepad activation stay with their owning screens.

use bevy::{input::touch::Touches, prelude::*, ui::UiSystems, window::PrimaryWindow};

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

    /// Advances the gesture by one observed frame.
    fn observe(&mut self, interaction: Interaction, pointer: Option<Vec2>, touch: bool) {
        self.activated = false;
        let pressed = interaction == Interaction::Pressed;
        match (&mut self.held, pressed) {
            (None, true) => {
                self.held = Some(HeldPress {
                    touch,
                    cancelled: false,
                    origin: pointer,
                });
            }
            (Some(held), true) => {
                if let (Some(origin), Some(pointer)) = (held.origin, pointer)
                    && origin.distance(pointer) > DRAG_CANCEL_DISTANCE
                {
                    held.cancelled = true;
                }
            }
            (Some(held), false) => {
                // A mouse release over the control leaves it hovered; touch has no position
                // after release, so its drag-cancel check is the only containment evidence.
                self.activated =
                    !held.cancelled && (interaction == Interaction::Hovered || held.touch);
                self.held = None;
            }
            (None, false) => {}
        }
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

#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
fn track_release_activation(
    windows: Query<&Window, With<PrimaryWindow>>,
    touches: Option<Res<Touches>>,
    mut controls: Query<(&Interaction, &mut ReleaseActivation)>,
) {
    let touch_position = touches.as_deref().and_then(Touches::first_pressed_position);
    let pointer = touch_position.or_else(|| windows.iter().next()?.cursor_position());
    let touch = touch_position.is_some();
    for (interaction, mut activation) in &mut controls {
        // Owners poll `activated()` each frame; change detection would only add noise.
        activation
            .bypass_change_detection()
            .observe(*interaction, pointer, touch);
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
                activation.observe(*interaction, *pointer, *touch);
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
}
