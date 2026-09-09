//! Bounded single-binding edits. A conflicting assignment is staged as an atomic swap.
use super::{ClientInputSettings, GamepadAction, KeyboardAction};
use bevy::prelude::{GamepadButton, KeyCode, MouseButton};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BindingTarget {
    Keyboard(KeyboardAction),
    Mouse,
    Gamepad(GamepadAction),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum BindingValue {
    Key(KeyCode),
    Mouse(MouseButton),
    Gamepad(GamepadButton),
}

impl BindingTarget {
    pub(crate) fn name(self) -> &'static str {
        match self {
            Self::Keyboard(action) => action.name(),
            Self::Mouse => "Primary fire",
            Self::Gamepad(action) => action.name(),
        }
    }

    pub(crate) fn value(self, settings: ClientInputSettings) -> BindingValue {
        match self {
            Self::Keyboard(action) => BindingValue::Key(settings.keyboard.key(action)),
            Self::Mouse => BindingValue::Mouse(settings.mouse_primary),
            Self::Gamepad(action) => BindingValue::Gamepad(settings.gamepad.button(action)),
        }
    }

    fn assign(self, settings: &mut ClientInputSettings, value: BindingValue) -> Result<(), String> {
        match (self, value) {
            (Self::Keyboard(action), BindingValue::Key(key)) => settings.rebind(action, key)?,
            (Self::Mouse, BindingValue::Mouse(button)) => settings.rebind_mouse(button),
            (Self::Gamepad(action), BindingValue::Gamepad(button)) => {
                settings.rebind_gamepad(action, button);
            }
            _ => return Err("Choose an input from the selected device.".into()),
        }
        Ok(())
    }
}

impl BindingValue {
    pub(crate) fn label(self) -> String {
        match self {
            Self::Key(key) => {
                let name = format!("{key:?}");
                name.strip_prefix("Key")
                    .unwrap_or(&name)
                    .replace("Arrow", "")
            }
            Self::Mouse(button) => match button {
                MouseButton::Left => "Left mouse".into(),
                MouseButton::Right => "Right mouse".into(),
                MouseButton::Middle => "Middle mouse".into(),
                MouseButton::Back => "Mouse back".into(),
                MouseButton::Forward => "Mouse forward".into(),
                MouseButton::Other(index) => format!("Mouse {index}"),
            },
            Self::Gamepad(button) => match button {
                GamepadButton::South => "South".into(),
                GamepadButton::East => "East".into(),
                GamepadButton::North => "North".into(),
                GamepadButton::West => "West".into(),
                GamepadButton::LeftTrigger => "Left bumper".into(),
                GamepadButton::RightTrigger => "Right bumper".into(),
                GamepadButton::LeftTrigger2 => "Left trigger".into(),
                GamepadButton::RightTrigger2 => "Right trigger".into(),
                GamepadButton::LeftThumb => "Left stick press".into(),
                GamepadButton::RightThumb => "Right stick press".into(),
                GamepadButton::DPadUp => "D-pad up".into(),
                GamepadButton::DPadDown => "D-pad down".into(),
                GamepadButton::DPadLeft => "D-pad left".into(),
                GamepadButton::DPadRight => "D-pad right".into(),
                _ => format!("{button:?}"),
            },
        }
    }
}

#[derive(Clone, Debug)]
pub(crate) struct BindingProposal {
    pub(crate) settings: ClientInputSettings,
    pub(crate) target: BindingTarget,
    pub(crate) other: Option<BindingTarget>,
    pub(crate) previous: BindingValue,
    pub(crate) replacement: BindingValue,
}

pub(crate) fn propose(
    settings: ClientInputSettings,
    target: BindingTarget,
    replacement: BindingValue,
) -> Result<BindingProposal, String> {
    let previous = target.value(settings);
    let targets: Vec<_> = match target {
        BindingTarget::Keyboard(_) => KeyboardAction::ALL
            .into_iter()
            .map(BindingTarget::Keyboard)
            .collect(),
        BindingTarget::Gamepad(_) => settings
            .gamepad
            .rows()
            .into_iter()
            .map(|(action, _)| BindingTarget::Gamepad(action))
            .collect(),
        BindingTarget::Mouse => vec![BindingTarget::Mouse],
    };
    let conflicts: Vec<_> = targets
        .iter()
        .copied()
        .filter(|other| *other != target && other.value(settings) == replacement)
        .collect();
    if conflicts.len() > 1 {
        return Err(
            "This input already has several assignments. Choose another input first.".into(),
        );
    }
    let other = conflicts.first().copied();
    if other.is_some()
        && targets.iter().any(|candidate| {
            *candidate != target
                && Some(*candidate) != other
                && candidate.value(settings) == previous
        })
    {
        return Err(
            "The old input has another assignment. Choose an unused input before swapping.".into(),
        );
    }
    let mut changed = settings;
    target.assign(&mut changed, replacement)?;
    if let Some(other) = other {
        other.assign(&mut changed, previous)?;
    }
    changed.validate()?;
    Ok(BindingProposal {
        settings: changed,
        target,
        other,
        previous,
        replacement,
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn conflicting_key_is_a_staged_atomic_swap() {
        let original = ClientInputSettings::default();
        let proposal = propose(
            original,
            BindingTarget::Keyboard(KeyboardAction::Ultimate),
            BindingValue::Key(KeyCode::KeyQ),
        )
        .unwrap();
        assert_eq!(
            proposal.other,
            Some(BindingTarget::Keyboard(KeyboardAction::ActiveItem))
        );
        assert_eq!(original.keyboard.ultimate, KeyCode::KeyE);
        assert_eq!(proposal.settings.keyboard.ultimate, KeyCode::KeyQ);
        assert_eq!(proposal.settings.keyboard.active_item, KeyCode::KeyE);
        assert!(proposal.settings.keyboard_conflicts().is_empty());
    }
    #[test]
    fn gamepad_swap_preserves_both_actions_and_rejects_wrong_device() {
        let original = ClientInputSettings::default();
        let proposal = propose(
            original,
            BindingTarget::Gamepad(GamepadAction::Ultimate),
            BindingValue::Gamepad(GamepadButton::South),
        )
        .unwrap();
        assert_eq!(proposal.settings.gamepad.ultimate, GamepadButton::South);
        assert_eq!(
            proposal.settings.gamepad.interact,
            original.gamepad.ultimate
        );
        assert!(proposal.settings.gamepad_conflicts().is_empty());
        assert!(
            propose(
                original,
                BindingTarget::Mouse,
                BindingValue::Key(KeyCode::KeyQ)
            )
            .is_err()
        );
    }
    #[test]
    fn ambiguous_legacy_duplicates_are_not_silently_overwritten() {
        let mut original = ClientInputSettings::default();
        original.keyboard.interact = KeyCode::KeyQ;
        assert!(
            propose(
                original,
                BindingTarget::Keyboard(KeyboardAction::Ultimate),
                BindingValue::Key(KeyCode::KeyQ)
            )
            .is_err()
        );
    }
}
