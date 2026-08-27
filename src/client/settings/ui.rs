//! Pause-overlay input-settings UI: selectable calibration and rebind rows, text
//! composition, and rebind capture. Device-to-intent conversion stays in `client::input`;
//! this module owns presenting and editing session-local input settings.

use super::is_modifier_key;
use super::{CalibrationField, ClientInputSettings, GamepadAction, KeyboardAction};
use crate::client::{ClientInputContext, InputCaptureConsumed, InputSettingsText};
use bevy::input::keyboard::KeyCode;
use bevy::prelude::*;

/// One selectable row of the pause settings overlay: a calibration value or a rebindable
/// device binding.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum InputSettingsField {
    Calibration(CalibrationField),
    Keyboard(KeyboardAction),
    MousePrimary,
    Gamepad(GamepadAction),
}

impl InputSettingsField {
    /// Every overlay row in stable cycling order.
    pub const ALL: [InputSettingsField; 24] = [
        InputSettingsField::Calibration(CalibrationField::MoveDeadzone),
        InputSettingsField::Calibration(CalibrationField::AimDeadzone),
        InputSettingsField::Calibration(CalibrationField::AimCommitThreshold),
        InputSettingsField::Calibration(CalibrationField::TriggerPress),
        InputSettingsField::Calibration(CalibrationField::TriggerRelease),
        InputSettingsField::Keyboard(KeyboardAction::MoveUp),
        InputSettingsField::Keyboard(KeyboardAction::MoveDown),
        InputSettingsField::Keyboard(KeyboardAction::MoveLeft),
        InputSettingsField::Keyboard(KeyboardAction::MoveRight),
        InputSettingsField::Keyboard(KeyboardAction::ActiveItem),
        InputSettingsField::Keyboard(KeyboardAction::Ultimate),
        InputSettingsField::Keyboard(KeyboardAction::Interact),
        InputSettingsField::Keyboard(KeyboardAction::Pause),
        InputSettingsField::Keyboard(KeyboardAction::Scoreboard),
        InputSettingsField::Keyboard(KeyboardAction::Screenshot),
        InputSettingsField::MousePrimary,
        InputSettingsField::Gamepad(GamepadAction::Primary),
        InputSettingsField::Gamepad(GamepadAction::ActiveItem),
        InputSettingsField::Gamepad(GamepadAction::Ultimate),
        InputSettingsField::Gamepad(GamepadAction::Interact),
        InputSettingsField::Gamepad(GamepadAction::Pause),
        InputSettingsField::Gamepad(GamepadAction::Cancel),
        InputSettingsField::Gamepad(GamepadAction::Scoreboard),
        InputSettingsField::Gamepad(GamepadAction::Screenshot),
    ];

    #[must_use]
    pub fn name(self) -> &'static str {
        match self {
            Self::Calibration(field) => field.name(),
            Self::Keyboard(action) => action.name(),
            Self::MousePrimary => "Mouse primary",
            Self::Gamepad(action) => action.name(),
        }
    }

    #[must_use]
    pub fn next(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|field| *field == self)
            .unwrap_or(0);
        Self::ALL[(index + 1) % Self::ALL.len()]
    }

    #[must_use]
    pub fn previous(self) -> Self {
        let index = Self::ALL
            .iter()
            .position(|field| *field == self)
            .unwrap_or(0);
        Self::ALL[(index + Self::ALL.len() - 1) % Self::ALL.len()]
    }

    pub(crate) const fn is_rebindable(self) -> bool {
        !matches!(self, Self::Calibration(_))
    }
}

/// Selected field and rebind-listening state for the pause settings overlay. While
/// `listening` is set, the next physical key, mouse button, or controller button commits
/// the selected binding instead of acting as gameplay or pause input.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct InputSettingsSelection {
    pub field: InputSettingsField,
    pub listening: bool,
}

/// Product-shell draft. Its presence redirects the existing calibration and rebind controls away
/// from active gameplay settings until Apply is accepted.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub(crate) struct InputSettingsDraft(pub ClientInputSettings);

impl Default for InputSettingsSelection {
    fn default() -> Self {
        Self {
            field: InputSettingsField::Calibration(CalibrationField::MoveDeadzone),
            listening: false,
        }
    }
}

fn key_label(code: KeyCode) -> String {
    let name = format!("{code:?}");
    name.strip_prefix("Key").unwrap_or(&name).to_string()
}

fn marked(label: &str, selected: bool) -> String {
    if selected {
        format!("[{label}]")
    } else {
        label.to_string()
    }
}

fn calibration_line(settings: &ClientInputSettings, selected: InputSettingsField) -> String {
    let mark = |value: f32, field: CalibrationField| {
        marked(
            &format!("{value:.2}"),
            selected == InputSettingsField::Calibration(field),
        )
    };
    format!(
        "Cal move={} aim={} commit={} trigger={}/{}",
        mark(settings.move_deadzone, CalibrationField::MoveDeadzone),
        mark(settings.aim_deadzone, CalibrationField::AimDeadzone),
        mark(
            settings.aim_commit_threshold,
            CalibrationField::AimCommitThreshold
        ),
        mark(settings.trigger_press, CalibrationField::TriggerPress),
        mark(settings.trigger_release, CalibrationField::TriggerRelease),
    )
}

fn keyboard_line(settings: &ClientInputSettings, selected: InputSettingsField) -> String {
    let mark = |code: KeyCode, action: KeyboardAction| {
        marked(
            &key_label(code),
            selected == InputSettingsField::Keyboard(action),
        )
    };
    format!(
        "Keys up={} down={} left={} right={} item={} ult={} use={} pause={} score={} shot={}",
        mark(settings.keyboard.move_up, KeyboardAction::MoveUp),
        mark(settings.keyboard.move_down, KeyboardAction::MoveDown),
        mark(settings.keyboard.move_left, KeyboardAction::MoveLeft),
        mark(settings.keyboard.move_right, KeyboardAction::MoveRight),
        mark(settings.keyboard.active_item, KeyboardAction::ActiveItem),
        mark(settings.keyboard.ultimate, KeyboardAction::Ultimate),
        mark(settings.keyboard.interact, KeyboardAction::Interact),
        mark(settings.keyboard.pause, KeyboardAction::Pause),
        mark(settings.keyboard.scoreboard, KeyboardAction::Scoreboard),
        mark(settings.keyboard.screenshot, KeyboardAction::Screenshot),
    )
}

fn gamepad_line(settings: &ClientInputSettings, selected: InputSettingsField) -> String {
    let mark = |button: GamepadButton, action: GamepadAction| {
        marked(
            &format!("{button:?}"),
            selected == InputSettingsField::Gamepad(action),
        )
    };
    format!(
        "Pad fire={} item={} ult={} use={} pause={} cancel={} score={} shot={}",
        mark(settings.gamepad.primary, GamepadAction::Primary),
        mark(settings.gamepad.active_item, GamepadAction::ActiveItem),
        mark(settings.gamepad.ultimate, GamepadAction::Ultimate),
        mark(settings.gamepad.interact, GamepadAction::Interact),
        mark(settings.gamepad.pause, GamepadAction::Pause),
        mark(settings.gamepad.cancel, GamepadAction::Cancel),
        mark(settings.gamepad.scoreboard, GamepadAction::Scoreboard),
        mark(settings.gamepad.screenshot, GamepadAction::Screenshot),
    )
}

fn conflict_line(settings: &ClientInputSettings) -> String {
    let mut conflicts: Vec<String> = settings
        .keyboard_conflicts()
        .into_iter()
        .map(|action| KeyboardAction::name(action).to_string())
        .collect();
    conflicts.extend(
        settings
            .gamepad_conflicts()
            .into_iter()
            .map(|action| GamepadAction::name(action).to_string()),
    );
    if conflicts.is_empty() {
        "Bindings OK".to_string()
    } else {
        format!("Conflict: {}", conflicts.join(", "))
    }
}

fn hint_line(selection: InputSettingsSelection) -> String {
    if selection.listening {
        format!(
            "Rebind {}: press a key, mouse button, or pad button (Escape or pad East cancels)",
            selection.field.name()
        )
    } else {
        "Tab/D-pad: next  [ ]/D-pad: adjust  B/South: rebind  I/O: invert  R: reset".to_string()
    }
}

/// Compose the pause-overlay settings text from validated session-local state. The selected
/// row is bracketed so the rebind target is unambiguous.
#[must_use]
pub fn compose_input_settings_lines(
    settings: &ClientInputSettings,
    selection: InputSettingsSelection,
) -> Vec<String> {
    vec![
        calibration_line(settings, selection.field),
        format!(
            "Invert move-Y: {}   Invert aim-Y: {}",
            if settings.invert_move_y { "on" } else { "off" },
            if settings.invert_aim_y { "on" } else { "off" }
        ),
        keyboard_line(settings, selection.field),
        gamepad_line(settings, selection.field),
        format!(
            "Mouse {}",
            marked(
                &format!("{:?}", settings.mouse_primary),
                selection.field == InputSettingsField::MousePrimary
            )
        ),
        conflict_line(settings),
        hint_line(selection),
    ]
}

/// Adjust session-local input calibration from the pause context only. These keys never
/// enter `FighterInput`: the authoritative match is unaffected by local settings.
#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "the focused Bevy system receives the complete local capture boundary; input device resources remain independently optional"
)]
pub(crate) fn adjust_input_settings_from_pause_keys(
    context: Res<ClientInputContext>,
    keyboard: Option<Res<ButtonInput<KeyCode>>>,
    mouse_buttons: Option<Res<ButtonInput<MouseButton>>>,
    gamepads: Query<&Gamepad>,
    mut selection: ResMut<InputSettingsSelection>,
    mut settings: ResMut<ClientInputSettings>,
    draft: Option<ResMut<InputSettingsDraft>>,
    capture_consumed: Option<ResMut<InputCaptureConsumed>>,
) {
    let mut capture_consumed = capture_consumed;
    if let Some(consumed) = capture_consumed.as_deref_mut() {
        consumed.0 = false;
    }
    if !settings_context_owns_input(*context) {
        return;
    }
    let Some(keyboard) = keyboard else {
        return;
    };
    let pad_pressed = |button: GamepadButton| gamepads.iter().any(|pad| pad.just_pressed(button));
    let editing_draft = draft.is_some();
    let mut draft = draft;
    let settings = draft
        .as_deref_mut()
        .map_or(&mut *settings, |draft| &mut draft.0);

    if selection.listening {
        // Escape and controller East cancel before any accepted binding press can commit.
        if keyboard.just_pressed(KeyCode::Escape) || pad_pressed(GamepadButton::East) {
            selection.listening = false;
            if let Some(consumed) = capture_consumed.as_deref_mut() {
                consumed.0 = true;
            }
            return;
        }
        match selection.field {
            InputSettingsField::Keyboard(action) => {
                if let Some(key) = keyboard
                    .get_just_pressed()
                    .copied()
                    .find(|key| !is_modifier_key(*key))
                    && settings.rebind(action, key).is_ok()
                {
                    selection.listening = false;
                }
            }
            InputSettingsField::MousePrimary => {
                if let Some(button) = mouse_buttons
                    .as_deref()
                    .and_then(|buttons| buttons.get_just_pressed().copied().next())
                {
                    settings.rebind_mouse(button);
                    selection.listening = false;
                }
            }
            InputSettingsField::Gamepad(action) => {
                if let Some(button) = gamepads
                    .iter()
                    .find_map(|pad| pad.get_just_pressed().copied().next())
                {
                    settings.rebind_gamepad(action, button);
                    selection.listening = false;
                }
            }
            InputSettingsField::Calibration(_) => {
                selection.listening = false;
            }
        }
        return;
    }

    // Product-shell editing is driven by focusable buttons so every operation is reachable from
    // controller, keyboard, and pointer. Only rebind capture itself remains in this shared input
    // system while a draft exists.
    if editing_draft {
        return;
    }

    if keyboard.just_pressed(KeyCode::Tab) || pad_pressed(GamepadButton::DPadDown) {
        selection.field = selection.field.next();
    }
    if pad_pressed(GamepadButton::DPadUp) {
        selection.field = selection.field.previous();
    }
    let adjust = |settings: &mut ClientInputSettings, step: f32| {
        if let InputSettingsField::Calibration(field) = selection.field {
            settings.adjust_calibration(field, step);
        }
    };
    if keyboard.just_pressed(KeyCode::BracketLeft) || pad_pressed(GamepadButton::DPadLeft) {
        adjust(settings, -0.05);
    }
    if keyboard.just_pressed(KeyCode::BracketRight) || pad_pressed(GamepadButton::DPadRight) {
        adjust(settings, 0.05);
    }
    if keyboard.just_pressed(KeyCode::KeyI) {
        settings.toggle_inversion(false);
    }
    if keyboard.just_pressed(KeyCode::KeyO) {
        settings.toggle_inversion(true);
    }
    if keyboard.just_pressed(KeyCode::KeyR) {
        settings.reset_to_default();
        selection.listening = false;
    }
    if selection.field.is_rebindable()
        && (keyboard.just_pressed(KeyCode::KeyB)
            || (pad_pressed(GamepadButton::South)
                && matches!(selection.field, InputSettingsField::Gamepad(_))))
    {
        selection.listening = true;
    }
}

const fn settings_context_owns_input(context: ClientInputContext) -> bool {
    matches!(context, ClientInputContext::Shell)
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
pub(crate) fn update_input_settings_overlay(
    context: Res<ClientInputContext>,
    settings: Res<ClientInputSettings>,
    draft: Option<Res<InputSettingsDraft>>,
    selection: Res<InputSettingsSelection>,
    mut texts: Query<&mut Text, With<InputSettingsText>>,
) {
    if !settings_context_owns_input(*context) {
        return;
    }
    let editing_product_draft = draft.is_some();
    let settings = draft.as_deref().map_or(&*settings, |draft| &draft.0);
    let mut lines = compose_input_settings_lines(settings, *selection);
    if editing_product_draft
        && !selection.listening
        && let Some(hint) = lines.last_mut()
    {
        *hint = "Use the focused controls below to select, adjust, rebind, or reset.".to_string();
    }
    for mut text in &mut texts {
        text.0 = lines.join("\n");
    }
}
