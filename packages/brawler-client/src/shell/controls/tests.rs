use super::super::{
    tests::{request_settings, shell_test_app},
    *,
};
use super::model::{Capture, ControlsState, InputControl};
use bevy::time::TimeUpdateStrategy;
use std::time::Duration;

fn app_for(name: &str) -> (App, std::path::PathBuf) {
    let dir = std::env::temp_dir().join(format!("brawler-controls-{name}-{}", std::process::id()));
    let mut app = shell_test_app(dir.join("settings.ron"));
    app.update();
    request_settings(&mut app);
    action(&mut app, ShellAction::ControlsCategory);
    (app, dir)
}
fn action(app: &mut App, action: ShellAction) {
    app.world_mut()
        .resource_mut::<PendingActions>()
        .0
        .push(action);
    app.update();
}
fn keyboard(app: &mut App, key: KeyCode) {
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(key);
    app.update();
}

#[test]
fn capture_waits_for_release_and_conflict_requires_explicit_swap() {
    let (mut app, dir) = app_for("swap");
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .press(KeyCode::Enter);
    action(&mut app, ShellAction::Input(InputControl::Keyboard(5)));
    app.update();
    assert!(matches!(
        app.world().resource::<ControlsState>().capture,
        Capture::Arming(..)
    ));
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    app.update();
    keyboard(&mut app, KeyCode::KeyQ);
    assert!(matches!(
        app.world().resource::<ControlsState>().capture,
        Capture::Conflict(_)
    ));
    assert_eq!(
        app.world()
            .resource::<ClientInputSettings>()
            .keyboard
            .ultimate,
        KeyCode::KeyE
    );
    app.world_mut()
        .resource_mut::<ButtonInput<KeyCode>>()
        .reset_all();
    action(&mut app, ShellAction::Input(InputControl::Swap));
    let settings = app.world().resource::<ClientInputSettings>();
    assert_eq!(settings.keyboard.ultimate, KeyCode::KeyQ);
    assert_eq!(settings.keyboard.active_item, KeyCode::KeyE);
    assert!(settings.keyboard_conflicts().is_empty());
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::Settings
    );
    assert!(dir.join("settings.ron").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn pointer_back_cancels_capture_without_closing_settings() {
    let (mut app, _) = app_for("capture-back");
    action(&mut app, ShellAction::Input(InputControl::Mouse));
    app.update();
    let mut buttons = app.world_mut().query::<(Entity, &ShellButton)>();
    let back = buttons
        .iter(app.world())
        .find(|(_, button)| button.id == ShellControlId::Cancel)
        .unwrap()
        .0;
    app.world_mut()
        .entity_mut(back)
        .insert(Interaction::Pressed);
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert!(matches!(
        app.world().resource::<ControlsState>().capture,
        Capture::Idle
    ));
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::Settings
    );
    assert_eq!(
        app.world().resource::<ClientInputSettings>().mouse_primary,
        MouseButton::Left
    );
}

#[test]
fn gamepad_capture_ignores_other_controllers_and_cancels_on_disconnect() {
    let (mut app, dir) = app_for("device");
    let chosen = app.world_mut().spawn(Gamepad::default()).id();
    let other = app.world_mut().spawn(Gamepad::default()).id();
    app.world_mut().resource_mut::<ControlsState>().device = Some(chosen);
    action(&mut app, ShellAction::Input(InputControl::GamepadTab));
    assert_eq!(app.world().resource::<ControlsState>().device, Some(chosen));
    action(&mut app, ShellAction::Input(InputControl::Gamepad(2)));
    app.update();
    app.world_mut()
        .get_mut::<Gamepad>(other)
        .unwrap()
        .digital_mut()
        .press(GamepadButton::West);
    app.update();
    assert!(matches!(
        app.world().resource::<ControlsState>().capture,
        Capture::Listening(..)
    ));
    assert_eq!(
        app.world()
            .resource::<ClientInputSettings>()
            .gamepad
            .ultimate,
        GamepadButton::RightTrigger
    );
    app.world_mut()
        .get_mut::<Gamepad>(chosen)
        .unwrap()
        .digital_mut()
        .press(GamepadButton::West);
    app.update();
    assert_eq!(
        app.world()
            .resource::<ClientInputSettings>()
            .gamepad
            .ultimate,
        GamepadButton::West
    );
    app.world_mut()
        .get_mut::<Gamepad>(chosen)
        .unwrap()
        .digital_mut()
        .reset_all();
    action(&mut app, ShellAction::Input(InputControl::Gamepad(2)));
    app.update();
    app.world_mut().despawn(chosen);
    app.update();
    assert!(matches!(
        app.world().resource::<ControlsState>().capture,
        Capture::Idle
    ));
    assert_eq!(
        app.world()
            .resource::<ClientInputSettings>()
            .gamepad
            .ultimate,
        GamepadButton::West
    );
    assert_eq!(
        *app.world().resource::<ClientOverlay>(),
        ClientOverlay::Settings
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn measured_recommendation_previews_without_saving_until_accepted() {
    let (mut app, dir) = app_for("measure");
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_millis(
        50,
    )));
    let mut pad = Gamepad::default();
    pad.analog_mut().set(GamepadAxis::LeftStickX, 0.02);
    pad.analog_mut().set(GamepadAxis::RightStickX, 0.04);
    app.world_mut().spawn(pad);
    action(&mut app, ShellAction::Input(InputControl::GamepadTab));
    action(&mut app, ShellAction::Input(InputControl::Calibrate));
    action(&mut app, ShellAction::Input(InputControl::Measure));
    for _ in 0..65 {
        app.update();
    }
    assert!(
        app.world()
            .resource::<ControlsState>()
            .recommendation
            .is_some()
    );
    assert!(
        app.world()
            .resource::<ClientInputSettings>()
            .shape_targeting_axis(Vec2::X * 0.04)
            .is_some()
    );
    assert!(!dir.join("settings.ron").exists());
    action(&mut app, ShellAction::Input(InputControl::UseValues));
    assert!(
        app.world()
            .resource::<ClientInputSettings>()
            .shape_targeting_axis(Vec2::X * 0.04)
            .is_none()
    );
    assert!(dir.join("settings.ron").exists());
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn slider_changes_are_active_while_held_and_saved_on_release() {
    let (mut app, dir) = app_for("coalesce");
    app.world_mut()
        .resource_mut::<ShellSettingsDraft>()
        .0
        .master_volume = 50;
    app.world_mut()
        .resource_mut::<settings_screen::SettingsPage>()
        .defer_save = true;
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .press(MouseButton::Left);
    app.update();
    assert_eq!(
        app.world().resource::<ClientShellSettings>().master_volume,
        50
    );
    assert!(!dir.join("settings.ron").exists());
    app.world_mut()
        .resource_mut::<ButtonInput<MouseButton>>()
        .release(MouseButton::Left);
    app.update();
    assert_eq!(
        load_settings(&dir.join("settings.ron"))
            .unwrap()
            .unwrap()
            .1
            .master_volume,
        50
    );
    std::fs::remove_dir_all(dir).unwrap();
}

#[test]
fn display_preview_reverts_on_timeout_without_persisting_the_preview() {
    let (mut app, dir) = app_for("display");
    app.insert_resource(TimeUpdateStrategy::ManualDuration(Duration::from_secs(16)));
    action(&mut app, ShellAction::ToggleFullscreen);
    assert!(app.world().resource::<ShellSettingsDraft>().0.fullscreen);
    assert!(!app.world().resource::<ClientShellSettings>().fullscreen);
    app.update();
    assert!(!app.world().resource::<ShellSettingsDraft>().0.fullscreen);
    assert!(!dir.join("settings.ron").exists());
}

#[test]
fn category_reset_and_undo_leave_other_preferences_unchanged() {
    let (mut app, dir) = app_for("reset");
    app.world_mut()
        .resource_mut::<ShellSettingsDraft>()
        .0
        .master_volume = 50;
    action(&mut app, ShellAction::AccessibilityCategory);
    action(&mut app, ShellAction::ToggleReducedMotion);
    action(&mut app, ShellAction::ResetPreferences);
    assert!(!app.world().resource::<ClientShellSettings>().reduced_motion);
    assert_eq!(
        app.world().resource::<ClientShellSettings>().master_volume,
        50
    );
    action(&mut app, ShellAction::UndoPreferences);
    assert!(app.world().resource::<ClientShellSettings>().reduced_motion);
    assert_eq!(
        app.world().resource::<ClientShellSettings>().master_volume,
        50
    );
    std::fs::remove_dir_all(dir).unwrap();
}
