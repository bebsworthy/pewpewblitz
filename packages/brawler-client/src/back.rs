//! Platform "back" navigation input shared by the shell and (BRL-0197) the in-match menu.
//!
//! Escape stays its own key with its own consumers. `BackInput` adds the platform Back request:
//! a keyboard Back key (winit reports Android `KEYCODE_BACK` as logical `BrowserBack` where the
//! platform still dispatches it) or a platform adapter's [`BackInput::request`] (the Android
//! predictive-back callback). Consumers OR [`BackInput::just_pressed`] with their Escape check.

use bevy::{
    input::{
        ButtonState, InputSystems,
        keyboard::{Key, KeyboardInput},
    },
    prelude::*,
};

/// One-frame platform Back state plus the shell's "Back here leaves the app" flag.
#[derive(Resource, Default, Debug)]
pub struct BackInput {
    requested: bool,
    just_pressed: bool,
    leaves_app: bool,
}

impl BackInput {
    /// Queues a Back press from a platform adapter; it is reported on the next input update.
    pub fn request(&mut self) {
        self.requested = true;
    }

    /// True during the one frame in which Back was pressed.
    #[must_use]
    pub fn just_pressed(&self) -> bool {
        self.just_pressed
    }

    /// True while the shell has no in-app destination for Back (a root screen). A platform shell
    /// then handles Back itself; Android moves the task to the background.
    #[must_use]
    pub fn leaves_app(&self) -> bool {
        self.leaves_app
    }

    pub(crate) fn set_leaves_app(&mut self, leaves_app: bool) {
        self.leaves_app = leaves_app;
    }
}

/// Updates [`BackInput`] in `PreUpdate`; platform adapters call `request()` before this set.
#[derive(SystemSet, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct BackInputSystems;

struct BackInputPlugin;

impl Plugin for BackInputPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BackInput>()
            .add_message::<KeyboardInput>()
            .configure_sets(PreUpdate, BackInputSystems.after(InputSystems))
            .add_systems(PreUpdate, update_back_input.in_set(BackInputSystems));
    }
}

/// Installs the Back input once for every plugin that consumes it.
pub(crate) fn add_back_input(app: &mut App) {
    if !app.is_plugin_added::<BackInputPlugin>() {
        app.add_plugins(BackInputPlugin);
    }
}

fn update_back_input(mut back: ResMut<BackInput>, mut keyboard: MessageReader<KeyboardInput>) {
    let key_pressed = keyboard.read().any(|event| {
        event.state == ButtonState::Pressed
            && !event.repeat
            && (event.logical_key == Key::BrowserBack || event.key_code == KeyCode::BrowserBack)
    });
    back.just_pressed = std::mem::take(&mut back.requested) || key_pressed;
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy::input::keyboard::NativeKey;

    fn app() -> App {
        let mut app = App::new();
        app.add_plugins(MinimalPlugins);
        add_back_input(&mut app);
        app
    }

    fn key(app: &mut App, logical_key: Key, key_code: KeyCode) {
        app.world_mut().write_message(KeyboardInput {
            key_code,
            logical_key,
            state: ButtonState::Pressed,
            text: None,
            repeat: false,
            window: Entity::PLACEHOLDER,
        });
    }

    fn pressed(app: &App) -> bool {
        app.world().resource::<BackInput>().just_pressed()
    }

    #[test]
    fn platform_request_and_back_key_each_press_back_for_one_frame() {
        let mut app = app();
        app.world_mut().resource_mut::<BackInput>().request();
        app.update();
        assert!(pressed(&app));
        app.update();
        assert!(!pressed(&app));

        // Android KEYCODE_BACK: logical BrowserBack with an unidentified physical key.
        key(
            &mut app,
            Key::BrowserBack,
            KeyCode::Unidentified(bevy::input::keyboard::NativeKeyCode::Unidentified),
        );
        app.update();
        assert!(pressed(&app));
        app.update();
        assert!(!pressed(&app));
    }

    #[test]
    fn escape_is_not_platform_back() {
        let mut app = app();
        key(&mut app, Key::Escape, KeyCode::Escape);
        key(
            &mut app,
            Key::Unidentified(NativeKey::Unidentified),
            KeyCode::KeyB,
        );
        app.update();
        assert!(!pressed(&app));
    }
}
