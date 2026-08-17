//! Session-local input bindings and device calibration owned entirely by the client.
//!
//! The server validates ownership, tick windows, history, rate, bit masks, and normalized
//! magnitude of the quantized `FighterInput`; it never sees physical devices. These settings
//! shape device input *before* quantization, so the default calibration is the identity for
//! movement (the server remains the sole default move-shaping authority) and mirrors the
//! authoritative aim thresholds so default facing behavior is unchanged.

use bevy::input::keyboard::KeyCode;
use bevy::prelude::{GamepadButton, MouseButton, Resource, Vec2};

/// Upper bound for adjustable analog calibration values.
pub const MAX_CALIBRATION: f32 = 0.5;

/// One rebindable keyboard action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum KeyboardAction {
    MoveUp,
    MoveDown,
    MoveLeft,
    MoveRight,
    ActiveItem,
    Ultimate,
    Interact,
    Pause,
    Scoreboard,
}

impl KeyboardAction {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::MoveUp => "Move up",
            Self::MoveDown => "Move down",
            Self::MoveLeft => "Move left",
            Self::MoveRight => "Move right",
            Self::ActiveItem => "Active item",
            Self::Ultimate => "Ultimate",
            Self::Interact => "Interact",
            Self::Pause => "Pause",
            Self::Scoreboard => "Scoreboard",
        }
    }

    /// Every rebindable keyboard action in stable display order.
    pub const ALL: [KeyboardAction; 9] = [
        KeyboardAction::MoveUp,
        KeyboardAction::MoveDown,
        KeyboardAction::MoveLeft,
        KeyboardAction::MoveRight,
        KeyboardAction::ActiveItem,
        KeyboardAction::Ultimate,
        KeyboardAction::Interact,
        KeyboardAction::Pause,
        KeyboardAction::Scoreboard,
    ];
}

/// Keyboard bindings for the implemented gameplay actions.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct KeyboardBindings {
    pub move_up: KeyCode,
    pub move_down: KeyCode,
    pub move_left: KeyCode,
    pub move_right: KeyCode,
    pub active_item: KeyCode,
    pub ultimate: KeyCode,
    pub interact: KeyCode,
    pub pause: KeyCode,
    pub scoreboard: KeyCode,
}

impl Default for KeyboardBindings {
    fn default() -> Self {
        Self {
            move_up: KeyCode::KeyW,
            move_down: KeyCode::KeyS,
            move_left: KeyCode::KeyA,
            move_right: KeyCode::KeyD,
            active_item: KeyCode::KeyQ,
            ultimate: KeyCode::KeyE,
            interact: KeyCode::Space,
            pause: KeyCode::Escape,
            scoreboard: KeyCode::Tab,
        }
    }
}

impl KeyboardBindings {
    #[must_use]
    pub fn key(self, action: KeyboardAction) -> KeyCode {
        match action {
            KeyboardAction::MoveUp => self.move_up,
            KeyboardAction::MoveDown => self.move_down,
            KeyboardAction::MoveLeft => self.move_left,
            KeyboardAction::MoveRight => self.move_right,
            KeyboardAction::ActiveItem => self.active_item,
            KeyboardAction::Ultimate => self.ultimate,
            KeyboardAction::Interact => self.interact,
            KeyboardAction::Pause => self.pause,
            KeyboardAction::Scoreboard => self.scoreboard,
        }
    }

    /// Rebind exactly one action, rejecting modifier and unknown keys that cannot form a
    /// gameplay binding.
    pub fn rebind(&mut self, action: KeyboardAction, key: KeyCode) -> Result<(), String> {
        if matches!(
            key,
            KeyCode::ControlLeft
                | KeyCode::ControlRight
                | KeyCode::AltLeft
                | KeyCode::AltRight
                | KeyCode::SuperLeft
                | KeyCode::SuperRight
                | KeyCode::ShiftLeft
                | KeyCode::ShiftRight
                | KeyCode::Fn
                | KeyCode::FnLock
                | KeyCode::Meta
        ) {
            return Err(format!("{key:?} cannot be bound to an action"));
        }
        match action {
            KeyboardAction::MoveUp => self.move_up = key,
            KeyboardAction::MoveDown => self.move_down = key,
            KeyboardAction::MoveLeft => self.move_left = key,
            KeyboardAction::MoveRight => self.move_right = key,
            KeyboardAction::ActiveItem => self.active_item = key,
            KeyboardAction::Ultimate => self.ultimate = key,
            KeyboardAction::Interact => self.interact = key,
            KeyboardAction::Pause => self.pause = key,
            KeyboardAction::Scoreboard => self.scoreboard = key,
        }
        Ok(())
    }
}

/// The letter produced by a default binding, for logical keyboard-layout fallback.
/// Non-letter bindings (arrows, space, tab) have no character fallback.
#[must_use]
pub fn key_code_letter(code: KeyCode) -> Option<char> {
    let name = match code {
        KeyCode::KeyA => "a",
        KeyCode::KeyB => "b",
        KeyCode::KeyC => "c",
        KeyCode::KeyD => "d",
        KeyCode::KeyE => "e",
        KeyCode::KeyF => "f",
        KeyCode::KeyG => "g",
        KeyCode::KeyH => "h",
        KeyCode::KeyI => "i",
        KeyCode::KeyJ => "j",
        KeyCode::KeyK => "k",
        KeyCode::KeyL => "l",
        KeyCode::KeyM => "m",
        KeyCode::KeyN => "n",
        KeyCode::KeyO => "o",
        KeyCode::KeyP => "p",
        KeyCode::KeyQ => "q",
        KeyCode::KeyR => "r",
        KeyCode::KeyS => "s",
        KeyCode::KeyT => "t",
        KeyCode::KeyU => "u",
        KeyCode::KeyV => "v",
        KeyCode::KeyW => "w",
        KeyCode::KeyX => "x",
        KeyCode::KeyY => "y",
        KeyCode::KeyZ => "z",
        KeyCode::ArrowUp => "up",
        KeyCode::ArrowDown => "down",
        KeyCode::ArrowLeft => "left",
        KeyCode::ArrowRight => "right",
        KeyCode::Space => "space",
        KeyCode::Tab => "tab",
        KeyCode::Escape => "escape",
        KeyCode::Enter => "enter",
        _ => return None,
    };
    name.chars().next()
}

/// Controller button bindings for the implemented gameplay actions. The primary action is
/// read as an analog value with hysteresis, so digital buttons report 0.0/1.0 uniformly.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct GamepadBindings {
    pub primary: GamepadButton,
    pub active_item: GamepadButton,
    pub ultimate: GamepadButton,
    pub interact: GamepadButton,
    pub pause: GamepadButton,
    pub cancel: GamepadButton,
    pub scoreboard: GamepadButton,
}

impl Default for GamepadBindings {
    fn default() -> Self {
        Self {
            primary: GamepadButton::RightTrigger2,
            active_item: GamepadButton::LeftTrigger,
            ultimate: GamepadButton::RightTrigger,
            interact: GamepadButton::South,
            pause: GamepadButton::Start,
            cancel: GamepadButton::East,
            scoreboard: GamepadButton::Select,
        }
    }
}

impl GamepadBindings {
    #[must_use]
    pub fn button(self, action: GamepadAction) -> GamepadButton {
        match action {
            GamepadAction::Primary => self.primary,
            GamepadAction::ActiveItem => self.active_item,
            GamepadAction::Ultimate => self.ultimate,
            GamepadAction::Interact => self.interact,
            GamepadAction::Pause => self.pause,
            GamepadAction::Cancel => self.cancel,
            GamepadAction::Scoreboard => self.scoreboard,
        }
    }

    /// The full binding set as (action, button) rows for overlay rendering.
    #[must_use]
    pub fn rows(self) -> [(GamepadAction, GamepadButton); 7] {
        [
            (GamepadAction::Primary, self.primary),
            (GamepadAction::ActiveItem, self.active_item),
            (GamepadAction::Ultimate, self.ultimate),
            (GamepadAction::Interact, self.interact),
            (GamepadAction::Pause, self.pause),
            (GamepadAction::Cancel, self.cancel),
            (GamepadAction::Scoreboard, self.scoreboard),
        ]
    }
}

/// One rebindable controller action.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum GamepadAction {
    Primary,
    ActiveItem,
    Ultimate,
    Interact,
    Pause,
    Cancel,
    Scoreboard,
}

impl GamepadAction {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Primary => "Primary fire",
            Self::ActiveItem => "Active item",
            Self::Ultimate => "Ultimate",
            Self::Interact => "Interact",
            Self::Pause => "Pause",
            Self::Cancel => "Cancel",
            Self::Scoreboard => "Scoreboard",
        }
    }
}

/// Client-owned device calibration and bindings. Session-local; never serialized to the
/// wire and never read by the server.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ClientInputSettings {
    pub keyboard: KeyboardBindings,
    pub gamepad: GamepadBindings,
    pub mouse_primary: MouseButton,
    /// Movement deadzone applied before quantization. The default 0.0 is the identity so the
    /// authoritative server path is unchanged until the user opts into local shaping.
    pub move_deadzone: f32,
    pub aim_deadzone: f32,
    pub aim_commit_threshold: f32,
    pub trigger_press: f32,
    pub trigger_release: f32,
    pub invert_move_y: bool,
    pub invert_aim_y: bool,
    /// Bumped by every mutation so consumers can clear held/latched state on changes.
    pub revision: u32,
}

impl Default for ClientInputSettings {
    fn default() -> Self {
        Self {
            keyboard: KeyboardBindings::default(),
            gamepad: GamepadBindings::default(),
            mouse_primary: MouseButton::Left,
            move_deadzone: 0.0,
            aim_deadzone: 0.25,
            aim_commit_threshold: 0.35,
            trigger_press: 0.55,
            trigger_release: 0.45,
            invert_move_y: false,
            invert_aim_y: false,
            revision: 0,
        }
    }
}

/// A calibration field selectable in the pause settings overlay.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum CalibrationField {
    MoveDeadzone,
    AimDeadzone,
    AimCommitThreshold,
}

impl CalibrationField {
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::MoveDeadzone => "Move deadzone",
            Self::AimDeadzone => "Aim deadzone",
            Self::AimCommitThreshold => "Aim commit threshold",
        }
    }
}

impl ClientInputSettings {
    /// Validate bounds and hysteresis ordering.
    pub fn validate(&self) -> Result<(), String> {
        for (label, value) in [
            ("move deadzone", self.move_deadzone),
            ("aim deadzone", self.aim_deadzone),
            ("aim commit threshold", self.aim_commit_threshold),
            ("trigger press", self.trigger_press),
            ("trigger release", self.trigger_release),
        ] {
            if !value.is_finite() || !(0.0..=MAX_CALIBRATION + 0.5).contains(&value) {
                return Err(format!("client input {label} is out of range: {value}"));
            }
        }
        if self.move_deadzone < 0.0 || self.move_deadzone > MAX_CALIBRATION {
            return Err("client move deadzone must be between 0.0 and 0.5".to_string());
        }
        if self.aim_deadzone < 0.0 || self.aim_deadzone > MAX_CALIBRATION {
            return Err("client aim deadzone must be between 0.0 and 0.5".to_string());
        }
        if self.aim_commit_threshold < 0.0 || self.aim_commit_threshold > 1.0 {
            return Err("client aim commit threshold must be between 0.0 and 1.0".to_string());
        }
        if self.aim_commit_threshold < self.aim_deadzone {
            return Err("client aim commit threshold cannot be below the aim deadzone".to_string());
        }
        if self.trigger_release >= self.trigger_press {
            return Err("client trigger release must be strictly below press".to_string());
        }
        if self.trigger_press > 1.0 || self.trigger_release < 0.0 {
            return Err("client trigger thresholds must be within [0, 1]".to_string());
        }
        Ok(())
    }

    /// Report actions that share one physical key with another action.
    #[must_use]
    pub fn keyboard_conflicts(&self) -> Vec<KeyboardAction> {
        let mut conflicts = Vec::new();
        for (index, action) in KeyboardAction::ALL.iter().enumerate() {
            let key = self.keyboard.key(*action);
            if KeyboardAction::ALL
                .iter()
                .enumerate()
                .any(|(other, other_action)| {
                    other != index && self.keyboard.key(*other_action) == key
                })
                && !conflicts.contains(action)
            {
                conflicts.push(*action);
            }
        }
        conflicts
    }

    /// Report controller actions that share one physical button with another action.
    #[must_use]
    pub fn gamepad_conflicts(&self) -> Vec<GamepadAction> {
        let rows = self.gamepad.rows();
        let mut conflicts = Vec::new();
        for (index, (action, button)) in rows.iter().enumerate() {
            if rows
                .iter()
                .enumerate()
                .any(|(other, (other_action, other_button))| {
                    other != index && other_button == button && other_action != action
                })
                && !conflicts.contains(action)
            {
                conflicts.push(*action);
            }
        }
        conflicts
    }

    /// Reset to validated defaults and bump the revision.
    pub fn reset_to_default(&mut self) {
        *self = Self::default();
        self.revision = 1;
    }

    /// Adjust one calibration field by a clamped step and bump the revision.
    pub fn adjust_calibration(&mut self, field: CalibrationField, step: f32) {
        let (limit_min, limit_max) = match field {
            CalibrationField::MoveDeadzone | CalibrationField::AimDeadzone => {
                (0.0, MAX_CALIBRATION)
            }
            CalibrationField::AimCommitThreshold => (0.0, 1.0),
        };
        let target = match field {
            CalibrationField::MoveDeadzone => &mut self.move_deadzone,
            CalibrationField::AimDeadzone => &mut self.aim_deadzone,
            CalibrationField::AimCommitThreshold => &mut self.aim_commit_threshold,
        };
        *target = (*target + step).clamp(limit_min, limit_max);
        if field == CalibrationField::AimCommitThreshold {
            self.aim_commit_threshold = self.aim_commit_threshold.max(self.aim_deadzone);
        }
        self.revision = self.revision.saturating_add(1);
    }

    /// Toggle one Y-axis inversion and bump the revision.
    pub fn toggle_inversion(&mut self, aim: bool) {
        if aim {
            self.invert_aim_y = !self.invert_aim_y;
        } else {
            self.invert_move_y = !self.invert_move_y;
        }
        self.revision = self.revision.saturating_add(1);
    }

    /// Rebind one keyboard action and bump the revision.
    pub fn rebind(&mut self, action: KeyboardAction, key: KeyCode) -> Result<(), String> {
        self.keyboard.rebind(action, key)?;
        self.revision = self.revision.saturating_add(1);
        Ok(())
    }

    /// Shape a physical movement axis into abstract intent before quantization.
    ///
    /// With defaults this is the exact identity: no inversion, and the zero deadzone
    /// short-circuits before the radial remap so no floating-point round trip perturbs the
    /// axis. The authoritative server continues to apply its own fixed movement deadzone to
    /// the dequantized value.
    #[must_use]
    pub fn shape_move(&self, axis: Vec2) -> Vec2 {
        let axis = if self.invert_move_y {
            Vec2::new(axis.x, -axis.y)
        } else {
            axis
        };
        if self.move_deadzone <= 0.0 {
            return axis;
        }
        crate::movement::radial_deadzone(axis, self.move_deadzone)
    }

    /// Shape an aim axis and decide whether it commits a facing update.
    ///
    /// The defaults mirror the authoritative aim shaping so default facing behavior is
    /// unchanged; deviations are the user's local calibration choice.
    #[must_use]
    pub fn shape_aim(&self, axis: Vec2) -> Option<Vec2> {
        let axis = if self.invert_aim_y {
            Vec2::new(axis.x, -axis.y)
        } else {
            axis
        };
        let remapped = crate::movement::radial_deadzone(axis, self.aim_deadzone);
        (remapped.length() >= self.aim_commit_threshold).then(|| remapped.normalize_or_zero())
    }

    /// Post-calibration aim magnitude mapped onto the controlled lob range.
    #[must_use]
    pub fn shape_aim_distance(&self, axis: Vec2, range: f32) -> Option<f32> {
        self.shape_aim(axis)
            .is_some()
            .then(|| crate::movement::radial_deadzone(axis, self.aim_deadzone).length() * range)
    }

    /// Analog trigger hysteresis for the held primary-fire button.
    #[must_use]
    pub fn trigger_is_pressed(&self, previous: bool, value: f32) -> bool {
        let value = if value.is_finite() { value } else { 0.0 };
        if previous {
            value >= self.trigger_release
        } else {
            value >= self.trigger_press
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn defaults_validate_and_have_no_conflicts() {
        let settings = ClientInputSettings::default();
        assert!(settings.validate().is_ok());
        assert!(settings.keyboard_conflicts().is_empty());
    }

    #[test]
    fn default_move_shaping_is_the_identity() {
        let settings = ClientInputSettings::default();
        for axis in [
            Vec2::ZERO,
            Vec2::X,
            Vec2::NEG_Y,
            Vec2::new(0.5, -0.5),
            Vec2::new(0.2, 0.98),
        ] {
            assert_eq!(settings.shape_move(axis), axis);
        }
    }

    #[test]
    fn default_aim_shaping_matches_authoritative_commit_decisions() {
        let settings = ClientInputSettings::default();
        let tuning = crate::movement::InputTuning::default();
        for axis in [
            Vec2::ZERO,
            Vec2::new(0.1, 0.0),
            Vec2::new(0.3, 0.0),
            Vec2::new(0.34, 0.0),
            Vec2::new(0.36, 0.0),
            Vec2::new(-0.8, 0.2),
            Vec2::splat(core::f32::consts::FRAC_1_SQRT_2),
        ] {
            let client_commits = settings.shape_aim(axis);
            let server_commits = crate::movement::committed_aim(axis, tuning);
            // The commit decision must agree exactly; committed directions agree within
            // float noise because both paths normalize a positive scalar multiple.
            assert_eq!(
                client_commits.is_some(),
                server_commits.is_some(),
                "axis {axis:?}"
            );
            if let (Some(client), Some(server)) = (client_commits, server_commits) {
                assert!(
                    (client - server).length() < 1e-5,
                    "axis {axis:?} diverged: {client:?} vs {server:?}"
                );
            }
        }
    }

    #[test]
    fn default_golden_matrix_matches_wire_inputs_within_one_encoded_unit() {
        let settings = ClientInputSettings::default();
        let tuning = crate::movement::InputTuning::default();
        let magnitudes = [
            0.0_f32,
            0.1,
            0.2,
            0.25,
            0.35,
            0.5,
            core::f32::consts::FRAC_1_SQRT_2,
            0.9,
            1.0,
        ];
        for magnitude in magnitudes {
            for direction in [
                Vec2::new(1.0, 0.0),
                Vec2::new(0.0, 1.0),
                Vec2::new(-1.0, 0.0),
                Vec2::new(0.0, -1.0),
                Vec2::new(1.0, 1.0).normalize(),
                Vec2::new(-1.0, 1.0).normalize(),
            ] {
                let axis = direction * magnitude;
                // Calibrated client path: shape locally, then quantize.
                let calibrated =
                    crate::protocol::FighterInput::from_axes(settings.shape_move(axis), None, 0);
                // Current authoritative path: quantize the raw axis, then decode.
                let authoritative = crate::protocol::FighterInput::from_axes(axis, None, 0);
                let calibrated_decoded =
                    crate::movement::decoded_move(calibrated.move_axis, tuning);
                let authoritative_decoded =
                    crate::movement::decoded_move(authoritative.move_axis, tuning);
                let delta = (calibrated_decoded - authoritative_decoded).length();
                assert!(
                    delta <= 2.0 / f32::from(crate::protocol::QuantizedAxis2::MAX),
                    "magnitude {magnitude} direction {direction:?} diverged by {delta}"
                );
            }
        }
    }

    #[test]
    fn nondefault_calibration_shapes_before_quantization() {
        let mut settings = ClientInputSettings {
            invert_move_y: true,
            ..ClientInputSettings::default()
        };
        assert_eq!(
            settings.shape_move(Vec2::new(0.3, 0.4)),
            Vec2::new(0.3, -0.4)
        );

        settings.invert_aim_y = true;
        assert_eq!(
            settings.shape_aim(Vec2::new(0.6, 0.0)).map(|aim| aim.y),
            Some(0.0)
        );

        settings.move_deadzone = 0.2;
        let shaped = settings.shape_move(Vec2::new(0.5, 0.0));
        assert!((shaped.x - 0.375).abs() < 1e-6, "shaped {shaped:?}");
    }

    #[test]
    fn validation_rejects_crossed_hysteresis_and_out_of_range_calibration() {
        let mut settings = ClientInputSettings {
            trigger_release: 0.55,
            ..ClientInputSettings::default()
        };
        assert!(settings.validate().is_err());

        settings.trigger_release = 0.45;
        settings.move_deadzone = 0.9;
        assert!(settings.validate().is_err());

        settings.move_deadzone = 0.2;
        settings.aim_commit_threshold = 0.1;
        assert!(settings.validate().is_err());
    }

    #[test]
    fn conflicts_are_reported_and_reset_restores_defaults() {
        let mut settings = ClientInputSettings::default();
        settings
            .rebind(KeyboardAction::Ultimate, KeyCode::KeyQ)
            .expect("rebind applies");
        assert_eq!(
            settings.keyboard_conflicts(),
            vec![KeyboardAction::ActiveItem, KeyboardAction::Ultimate]
        );

        settings.reset_to_default();
        assert_eq!(settings.keyboard, KeyboardBindings::default());
        assert_eq!(settings.gamepad, GamepadBindings::default());
        assert!(!settings.invert_move_y && !settings.invert_aim_y);
        assert!(settings.revision > 0);
        assert!(settings.keyboard_conflicts().is_empty());
    }

    #[test]
    fn mutations_bump_the_revision_and_calibration_clamps() {
        let mut settings = ClientInputSettings::default();
        let before = settings.revision;
        settings.adjust_calibration(CalibrationField::MoveDeadzone, 0.3);
        assert!((settings.move_deadzone - 0.3).abs() < 1e-6);
        assert_eq!(settings.revision, before + 1);
        settings.adjust_calibration(CalibrationField::MoveDeadzone, 5.0);
        assert!((settings.move_deadzone - MAX_CALIBRATION).abs() < 1e-6);
        settings.adjust_calibration(CalibrationField::AimCommitThreshold, -5.0);
        assert!((settings.aim_commit_threshold - settings.aim_deadzone).abs() < 1e-6);
    }

    #[test]
    fn rebinding_rejects_modifier_keys() {
        let mut settings = ClientInputSettings::default();
        assert!(
            settings
                .rebind(KeyboardAction::Pause, KeyCode::ShiftLeft)
                .is_err()
        );
        assert!(
            settings
                .rebind(KeyboardAction::Pause, KeyCode::KeyP)
                .is_ok()
        );
        assert_eq!(settings.keyboard.pause, KeyCode::KeyP);
    }

    #[test]
    fn trigger_hysteresis_matches_default_authoritative_curve() {
        let settings = ClientInputSettings::default();
        let tuning = crate::movement::InputTuning::default();
        for previous in [false, true] {
            for value in [0.0_f32, 0.3, 0.44, 0.46, 0.54, 0.56, 1.0] {
                assert_eq!(
                    settings.trigger_is_pressed(previous, value),
                    crate::movement::trigger_pressed(previous, value, tuning),
                    "previous {previous} value {value}"
                );
            }
        }
    }
}
