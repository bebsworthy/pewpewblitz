use super::super::{
    ClientOverlay, InputCaptureConsumed, InputSettingsDraft, InputSettingsSelection,
    NavigationDirty, ShellButton, ShellControlId, settings_screen::SettingsPage,
};
use super::model::{Capture, ControlsState, InputControl};
use crate::client::settings::binding_edit::{self, BindingTarget, BindingValue};
use bevy::prelude::*;

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "One owner coordinates physical input capture before shell navigation"
)]
pub(in crate::client::shell) fn capture_input(
    keyboard: Res<ButtonInput<KeyCode>>,
    mouse: Res<ButtonInput<MouseButton>>,
    gamepads: Query<&Gamepad>,
    buttons: Query<(&Interaction, &ShellButton)>,
    mut state: ResMut<ControlsState>,
    mut selection: ResMut<InputSettingsSelection>,
    mut consumed: ResMut<InputCaptureConsumed>,
    draft: Option<ResMut<InputSettingsDraft>>,
    mut page: ResMut<SettingsPage>,
    mut dirty: ResMut<NavigationDirty>,
    overlay: Res<ClientOverlay>,
) {
    if *overlay != ClientOverlay::Settings {
        state.cancel_transient();
        selection.listening = false;
        return;
    }
    selection.listening = state.capture.waiting();
    let (id, device, armed) = match state.capture {
        Capture::Arming(id, device) => (id, device, false),
        Capture::Listening(id, device) => (id, device, true),
        _ => return,
    };
    consumed.0 = true;
    let cancel_clicked = buttons.iter().any(|(interaction, button)| {
        *interaction == Interaction::Pressed
            && matches!(
                button.id,
                ShellControlId::Cancel | ShellControlId::Input(InputControl::CancelCapture)
            )
    });
    let target = id.target().expect("capture only starts for a binding row");
    let disconnected = matches!(target, BindingTarget::Gamepad(_))
        && device.is_none_or(|entity| gamepads.get(entity).is_err());
    if keyboard.just_pressed(KeyCode::Escape) || cancel_clicked || disconnected {
        state.capture = Capture::Idle;
        selection.listening = false;
        page.rebuild = true;
        if disconnected {
            state.notice = "Controller disconnected. The binding was not changed.".into();
        }
        dirty.0 = Some(ShellControlId::Input(id));
        return;
    }
    let pad = device.and_then(|entity| gamepads.get(entity).ok());
    if !armed {
        if keyboard.get_pressed().next().is_none()
            && mouse.get_pressed().next().is_none()
            && pad.is_none_or(|pad| pad.get_pressed().next().is_none())
        {
            state.capture = Capture::Listening(id, device);
        }
        return;
    }
    let value = match target {
        BindingTarget::Keyboard(_) => keyboard
            .get_just_pressed()
            .next()
            .copied()
            .map(BindingValue::Key),
        BindingTarget::Mouse => mouse
            .get_just_pressed()
            .next()
            .copied()
            .map(BindingValue::Mouse),
        BindingTarget::Gamepad(_) => pad
            .and_then(|pad| pad.get_just_pressed().next().copied())
            .map(BindingValue::Gamepad),
    };
    let (Some(value), Some(mut draft)) = (value, draft) else {
        return;
    };
    if value == target.value(draft.0) {
        state.capture = Capture::Idle;
    } else {
        match binding_edit::propose(draft.0, target, value) {
            Ok(proposal) if proposal.other.is_some() => {
                state.capture = Capture::Conflict(proposal);
                dirty.0 = Some(ShellControlId::Input(InputControl::Swap));
            }
            Ok(proposal) => {
                draft.0 = proposal.settings;
                state.capture = Capture::Idle;
            }
            Err(error) => {
                state.notice = error;
                return;
            }
        }
    }
    selection.listening = false;
    page.rebuild = true;
    if !matches!(state.capture, Capture::Conflict(_)) {
        dirty.0 = Some(ShellControlId::Input(id));
    }
}
