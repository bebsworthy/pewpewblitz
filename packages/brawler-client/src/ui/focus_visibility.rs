//! Focus visible by input modality (BRL-fe7t). A touch press hides the focus frame; the next
//! keyboard or gamepad navigation event, or a mouse press, shows it again. Only the presentation
//! changes: navigation keeps its focused control, so a gamepad resumes exactly where the touch
//! left it, and current-choice and press feedback stay visible.

use bevy::{
    input::{
        ButtonState,
        gamepad::GamepadButtonStateChangedEvent,
        keyboard::KeyboardInput,
        mouse::MouseButtonInput,
        touch::{TouchInput, TouchPhase},
    },
    prelude::*,
};

/// Whether surfaces draw their navigation focus frame and pointer.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct FocusVisibility {
    pub(crate) visible: bool,
}

impl Default for FocusVisibility {
    fn default() -> Self {
        Self { visible: true }
    }
}

/// Keys that move focus. Text keys and Enter stay out: the Android on-screen keyboard sends them
/// while the player types after a tap.
fn navigates(key: KeyCode) -> bool {
    matches!(
        key,
        KeyCode::ArrowUp
            | KeyCode::ArrowDown
            | KeyCode::ArrowLeft
            | KeyCode::ArrowRight
            | KeyCode::Tab
    )
}

/// Applies this frame's input in order of the modalities that hide or show focus. A touch press
/// wins over anything else in the same frame.
#[allow(
    clippy::needless_pass_by_value,
    reason = "Bevy system parameters are runtime-owned"
)]
pub(crate) fn track_focus_visibility(
    mut visibility: ResMut<FocusVisibility>,
    mut touches: MessageReader<TouchInput>,
    mut keys: MessageReader<KeyboardInput>,
    mut mouse: MessageReader<MouseButtonInput>,
    mut pads: MessageReader<GamepadButtonStateChangedEvent>,
) {
    let hidden = touches
        .read()
        .any(|touch| touch.phase == TouchPhase::Started);
    let navigated = keys
        .read()
        .any(|key| key.state == ButtonState::Pressed && navigates(key.key_code))
        | mouse
            .read()
            .any(|button| button.state == ButtonState::Pressed)
        | pads.read().any(|pad| pad.state == ButtonState::Pressed);
    let next = if hidden {
        false
    } else if navigated {
        true
    } else {
        visibility.visible
    };
    if visibility.visible != next {
        visibility.visible = next;
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::{gamepad::GamepadButton, keyboard::Key, mouse::MouseButton};

    fn app() -> App {
        let mut app = App::new();
        app.add_message::<TouchInput>()
            .add_message::<KeyboardInput>()
            .add_message::<MouseButtonInput>()
            .add_message::<GamepadButtonStateChangedEvent>()
            .init_resource::<FocusVisibility>()
            .add_systems(PreUpdate, track_focus_visibility);
        app.update();
        app
    }

    fn visible(app: &App) -> bool {
        app.world().resource::<FocusVisibility>().visible
    }

    fn touch(app: &mut App, phase: TouchPhase) {
        app.world_mut().write_message(TouchInput {
            phase,
            position: Vec2::ZERO,
            window: Entity::PLACEHOLDER,
            force: None,
            id: 1,
        });
        app.update();
    }

    fn key(app: &mut App, key_code: KeyCode, logical_key: Key) {
        app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
        app.update();
    }

    #[test]
    fn touch_hides_focus_until_keyboard_gamepad_or_mouse_navigation() {
        let mut app = app();
        assert!(
            visible(&app),
            "keyboard, gamepad and mouse players start with focus"
        );

        touch(&mut app, TouchPhase::Started);
        assert!(!visible(&app));
        touch(&mut app, TouchPhase::Ended);
        assert!(!visible(&app), "lifting the finger keeps focus hidden");
        // Typing on the on-screen keyboard after a tap does not bring the frame back.
        key(&mut app, KeyCode::KeyA, Key::Character("a".into()));
        key(&mut app, KeyCode::Enter, Key::Enter);
        assert!(!visible(&app));

        key(&mut app, KeyCode::ArrowDown, Key::ArrowDown);
        assert!(visible(&app), "keyboard navigation shows focus again");

        touch(&mut app, TouchPhase::Started);
        app.world_mut()
            .write_message(GamepadButtonStateChangedEvent::new(
                Entity::PLACEHOLDER,
                GamepadButton::DPadRight,
                ButtonState::Pressed,
            ));
        app.update();
        assert!(visible(&app), "gamepad navigation shows focus again");

        touch(&mut app, TouchPhase::Started);
        app.world_mut().write_message(MouseButtonInput {
            button: MouseButton::Left,
            state: ButtonState::Pressed,
            window: Entity::PLACEHOLDER,
        });
        app.update();
        assert!(visible(&app), "a mouse press keeps today's visible focus");
    }
}
