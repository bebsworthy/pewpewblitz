use super::super::{InputSettingsDraft, ShellAction, ShellControlId, settings_screen};
use super::{
    calibration_view,
    model::{Capture, ControlsState, InputControl, InputPage, calibration_value},
};
use crate::client::{
    settings::{ClientInputSettings, KeyboardAction},
    ui::{controls::ControlValue, theme},
};
use bevy::prelude::*;

pub(in crate::client::shell) fn spawn_options(
    parent: &mut ChildSpawnerCommands,
    font: &TextFont,
    state: &ControlsState,
) {
    parent
        .spawn(Node {
            width: percent(100),
            column_gap: px(10),
            ..default()
        })
        .with_children(|tabs| {
            for (label, id) in [
                ("Keyboard & Mouse", InputControl::KeyboardTab),
                ("Gamepad", InputControl::GamepadTab),
            ] {
                tabs.spawn(Node {
                    width: percent(50),
                    ..default()
                })
                .with_children(|cell| {
                    row(cell, font, label, id, ControlValue::Tab);
                });
            }
        });
    if state.page != InputPage::Keyboard {
        row(
            parent,
            font,
            &state.device_name,
            InputControl::Device,
            ControlValue::Action,
        );
    }
    match state.page {
        InputPage::Keyboard => keyboard_rows(parent, font),
        InputPage::Gamepad => gamepad_rows(parent, font),
        InputPage::Calibration => {
            heading(parent, font, "STICK CALIBRATION");
            row(
                parent,
                font,
                "Movement deadzone",
                InputControl::MoveDeadzone,
                slider(),
            );
            row(
                parent,
                font,
                "Aim deadzone",
                InputControl::AimDeadzone,
                slider(),
            );
            if state.measurement.is_none() && state.recommendation.is_none() {
                row(
                    parent,
                    font,
                    "Measure stick drift",
                    InputControl::Measure,
                    ControlValue::Action,
                );
            }
        }
        InputPage::Advanced => {
            heading(parent, font, "ADVANCED ANALOG CONTROLS");
            for (label, id) in [
                ("Facing threshold", InputControl::AimCommit),
                ("Trigger press", InputControl::TriggerPress),
                ("Trigger release", InputControl::TriggerRelease),
            ] {
                row(parent, font, label, id, slider());
            }
        }
    }
    if !state.capture.waiting()
        && !matches!(state.capture, Capture::Conflict(_))
        && state.measurement.is_none()
        && state.recommendation.is_none()
    {
        row(
            parent,
            font,
            "Reset this page",
            InputControl::Reset,
            ControlValue::Action,
        );
        if state.undo.is_some() {
            row(
                parent,
                font,
                "Undo reset",
                InputControl::Undo,
                ControlValue::Action,
            );
        }
    }
}

fn keyboard_rows(parent: &mut ChildSpawnerCommands, font: &TextFont) {
    for (index, action) in KeyboardAction::ALL.into_iter().enumerate() {
        if index == 0 {
            heading(parent, font, "MOVEMENT");
        }
        if index == 4 {
            heading(parent, font, "COMBAT");
            row(
                parent,
                font,
                "Primary fire",
                InputControl::Mouse,
                ControlValue::Binding(String::new()),
            );
        }
        if index == 7 {
            heading(parent, font, "SYSTEM");
        }
        row(
            parent,
            font,
            action.name(),
            InputControl::Keyboard(index),
            ControlValue::Binding(String::new()),
        );
    }
}
fn gamepad_rows(parent: &mut ChildSpawnerCommands, font: &TextFont) {
    row(
        parent,
        font,
        "Calibrate sticks",
        InputControl::Calibrate,
        ControlValue::Action,
    );
    heading(parent, font, "ACTIONS");
    for (index, (action, _)) in ClientInputSettings::default()
        .gamepad
        .rows()
        .into_iter()
        .enumerate()
    {
        row(
            parent,
            font,
            action.name(),
            InputControl::Gamepad(index),
            ControlValue::Binding(String::new()),
        );
    }
    heading(parent, font, "STICKS");
    row(
        parent,
        font,
        "Invert movement Y",
        InputControl::InvertMove,
        ControlValue::Toggle(false),
    );
    row(
        parent,
        font,
        "Invert aim Y",
        InputControl::InvertAim,
        ControlValue::Toggle(false),
    );
    row(
        parent,
        font,
        "Advanced analog controls",
        InputControl::Advanced,
        ControlValue::Action,
    );
}
fn slider() -> ControlValue {
    ControlValue::Slider {
        fraction: 0.0,
        label: String::new(),
    }
}
fn heading(parent: &mut ChildSpawnerCommands, font: &TextFont, label: &str) {
    parent.spawn((
        Text::new(label),
        TextFont {
            font_size: 20.0.into(),
            ..font.clone()
        },
        TextColor(theme::TEAL),
        Node {
            margin: UiRect::top(px(12)),
            ..default()
        },
    ));
}
pub(super) fn row(
    parent: &mut ChildSpawnerCommands,
    font: &TextFont,
    label: &str,
    id: InputControl,
    value: ControlValue,
) {
    settings_screen::control(
        parent,
        font,
        label,
        ShellControlId::Input(id),
        ShellAction::Input(id),
        value,
    );
}

pub(in crate::client::shell) fn spawn_details(
    parent: &mut ChildSpawnerCommands,
    font: &TextFont,
    state: &ControlsState,
) {
    if state.capture.waiting() {
        row(
            parent,
            font,
            "Cancel capture",
            InputControl::CancelCapture,
            ControlValue::Action,
        );
    } else if matches!(state.capture, Capture::Conflict(_)) {
        row(
            parent,
            font,
            "Swap bindings",
            InputControl::Swap,
            ControlValue::Action,
        );
        row(
            parent,
            font,
            "Cancel",
            InputControl::CancelCapture,
            ControlValue::Action,
        );
    } else if state.page == InputPage::Calibration {
        calibration_view::spawn(parent, font);
        if state.recommendation.is_some() {
            row(
                parent,
                font,
                "Use these values",
                InputControl::UseValues,
                ControlValue::Action,
            );
            row(
                parent,
                font,
                "Measure again",
                InputControl::Measure,
                ControlValue::Action,
            );
            row(
                parent,
                font,
                "Keep current",
                InputControl::KeepCurrent,
                ControlValue::Action,
            );
        } else if state.measurement.is_some() {
            row(
                parent,
                font,
                "Cancel measurement",
                InputControl::KeepCurrent,
                ControlValue::Action,
            );
        }
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Controls values project the current input draft"
)]
pub(in crate::client::shell) fn present_values(
    state: Res<ControlsState>,
    draft: Option<Res<InputSettingsDraft>>,
    mut controls: Query<(
        &super::super::ShellButton,
        &mut ControlValue,
        &mut crate::client::ui::controls::ControlState,
    )>,
) {
    let Some(draft) = draft else {
        return;
    };
    for (button, mut value, mut chrome) in &mut controls {
        let ShellControlId::Input(id) = button.id else {
            continue;
        };
        chrome.selected = matches!(
            (id, state.page),
            (InputControl::KeyboardTab, InputPage::Keyboard)
        ) || (id == InputControl::GamepadTab
            && state.page != InputPage::Keyboard);
        if let Some(target) = id.target() {
            *value = ControlValue::Binding(if state.capture.control() == Some(id) {
                if matches!(state.capture, Capture::Arming(..)) {
                    "Release input…"
                } else {
                    "Press input…"
                }
                .into()
            } else {
                target.value(draft.0).label()
            });
        } else if let Some(field) = id.calibration() {
            let current = calibration_value(field, draft.0);
            let maximum = if matches!(id, InputControl::MoveDeadzone | InputControl::AimDeadzone) {
                0.5
            } else {
                1.0
            };
            *value = ControlValue::Slider {
                fraction: current / maximum,
                label: format!("{:.0}%", current * 100.0),
            };
        } else {
            match id {
                InputControl::InvertMove => *value = ControlValue::Toggle(draft.0.invert_move_y),
                InputControl::InvertAim => *value = ControlValue::Toggle(draft.0.invert_aim_y),
                _ => {}
            }
        }
    }
}

pub(in crate::client::shell) fn detail_text(
    state: &ControlsState,
    id: Option<InputControl>,
    settings: ClientInputSettings,
) -> String {
    if let Capture::Conflict(proposal) = &state.capture {
        let other = proposal.other.expect("conflict contains another action");
        return format!(
            "SWAP BINDINGS\n\n{}: {} → {}\n\n{}: {} → {}",
            proposal.target.name(),
            proposal.previous.label(),
            proposal.replacement.label(),
            other.name(),
            proposal.replacement.label(),
            proposal.previous.label()
        );
    }
    if let Some(target) = state.capture.control().and_then(InputControl::target) {
        return format!(
            "{}\n\nPress the new input after releasing the activation button.\n\nCurrent: {}\nPress the current input to keep it, or use Escape / Cancel capture.\n\n{}",
            target.name().to_uppercase(),
            target.value(settings).label(),
            state.notice
        );
    }
    if state.page == InputPage::Calibration {
        if let Some(suggestion) = state.recommendation {
            return format!(
                "TEST RECOMMENDED VALUES\n\nMovement {:.0}% · Aim {:.0}%\nThe plots preview these values. Use them when the sticks feel right.",
                suggestion.movement * 100.0,
                suggestion.aim * 100.0
            );
        }
        if let Some(measurement) = &state.measurement {
            return format!(
                "MEASURING REST DRIFT\n\nRelease both sticks. {:.0}%\n",
                measurement.progress() * 100.0
            );
        }
        return format!(
            "STICK RESPONSE\n\nThe shaded area is ignored. Outside it, input smoothly uses the full range. Calibration is shared across controllers.\n\n{}",
            state.notice
        );
    }
    let explanation = match id {
        Some(InputControl::InvertMove) => "Reverse the left stick's vertical movement.",
        Some(InputControl::InvertAim) => "Reverse the right stick's vertical aim.",
        Some(InputControl::Calibrate) => {
            "Inspect live stick input, adjust deadzones, and measure neutral drift."
        }
        Some(InputControl::Device) => {
            "Choose the controller used for binding capture and calibration. Binding names describe physical positions so they work across controller brands."
        }
        Some(InputControl::AimCommit) => {
            "Minimum processed aim magnitude that updates facing. Placement targeting still uses every nonzero calibrated magnitude."
        }
        Some(InputControl::TriggerPress) => {
            "Trigger magnitude required to start firing. Kept above the release threshold to prevent flicker."
        }
        Some(InputControl::TriggerRelease) => {
            "Release firing below this trigger magnitude. Kept below the press threshold."
        }
        Some(id) if id.target().is_some() => {
            "Select the binding to assign a new input. Conflicting assignments are previewed as a swap before anything changes."
        }
        _ => "Choose an action to change its binding. Changes save automatically.",
    };
    format!("{explanation}\n\n{}", state.notice)
}
