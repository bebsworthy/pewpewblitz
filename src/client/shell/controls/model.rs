use super::super::{
    ClientOverlay, InputSettingsDraft, InputSettingsSelection, NavigationDirty, PendingActions,
    ShellAction, ShellButton, ShellControlId,
    settings_screen::{Category, SettingsPage},
};
use crate::client::settings::ClientInputSettings;
use crate::client::settings::{
    CalibrationField, KeyboardAction,
    binding_edit::{BindingProposal, BindingTarget},
    calibration::{NeutralMeasurement, Recommendation},
};
use crate::client::ui::controls::{SliderTrack, slider_fraction};
use bevy::prelude::*;
use bevy::ui::RelativeCursorPosition;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(in crate::client::shell) enum InputPage {
    #[default]
    Keyboard,
    Gamepad,
    Calibration,
    Advanced,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(in crate::client::shell) enum InputControl {
    KeyboardTab,
    GamepadTab,
    Device,
    Keyboard(usize),
    Mouse,
    Gamepad(usize),
    InvertMove,
    InvertAim,
    Calibrate,
    Advanced,
    MoveDeadzone,
    AimDeadzone,
    AimCommit,
    TriggerPress,
    TriggerRelease,
    Measure,
    UseValues,
    KeepCurrent,
    CancelCapture,
    Swap,
    Reset,
    Undo,
}
impl InputControl {
    pub(super) fn target(self) -> Option<BindingTarget> {
        match self {
            Self::Keyboard(index) => KeyboardAction::ALL
                .get(index)
                .copied()
                .map(BindingTarget::Keyboard),
            Self::Mouse => Some(BindingTarget::Mouse),
            Self::Gamepad(index) => ClientInputSettings::default()
                .gamepad
                .rows()
                .get(index)
                .map(|(action, _)| BindingTarget::Gamepad(*action)),
            _ => None,
        }
    }
    pub(super) fn calibration(self) -> Option<CalibrationField> {
        match self {
            Self::MoveDeadzone => Some(CalibrationField::MoveDeadzone),
            Self::AimDeadzone => Some(CalibrationField::AimDeadzone),
            Self::AimCommit => Some(CalibrationField::AimCommitThreshold),
            Self::TriggerPress => Some(CalibrationField::TriggerPress),
            Self::TriggerRelease => Some(CalibrationField::TriggerRelease),
            _ => None,
        }
    }
}

#[derive(Clone, Default, Debug)]
pub(super) enum Capture {
    #[default]
    Idle,
    Arming(InputControl, Option<Entity>),
    Listening(InputControl, Option<Entity>),
    Conflict(BindingProposal),
}
impl Capture {
    pub(super) fn waiting(&self) -> bool {
        matches!(self, Self::Arming(..) | Self::Listening(..))
    }
    pub(super) fn control(&self) -> Option<InputControl> {
        match self {
            Self::Arming(id, _) | Self::Listening(id, _) => Some(*id),
            _ => None,
        }
    }
}

#[derive(Resource, Clone, Default, Debug)]
pub(in crate::client::shell) struct ControlsState {
    pub(super) page: InputPage,
    pub(super) device: Option<Entity>,
    pub(super) device_name: String,
    pub(super) capture: Capture,
    pub(super) measurement: Option<NeutralMeasurement>,
    pub(super) recommendation: Option<Recommendation>,
    pub(super) raw: [Vec2; 2],
    pub(super) notice: String,
    pub(super) undo: Option<ClientInputSettings>,
}
impl ControlsState {
    pub(in crate::client::shell) fn child_page(&self) -> bool {
        matches!(self.page, InputPage::Calibration | InputPage::Advanced)
    }
    pub(in crate::client::shell) fn testing_sticks(&self) -> bool {
        self.page == InputPage::Calibration
    }
    pub(in crate::client::shell) fn cancel_transient(&mut self) {
        self.capture = Capture::Idle;
        self.measurement = None;
        self.recommendation = None;
        self.undo = None;
    }
}

pub(in crate::client::shell) fn allows(id: ShellControlId, state: &ControlsState) -> bool {
    match state.capture {
        Capture::Arming(..) | Capture::Listening(..) => matches!(
            id,
            ShellControlId::Cancel | ShellControlId::Input(InputControl::CancelCapture)
        ),
        Capture::Conflict(_) => matches!(
            id,
            ShellControlId::Cancel
                | ShellControlId::Input(InputControl::CancelCapture | InputControl::Swap)
        ),
        Capture::Idle => {
            if state.measurement.is_some() {
                return matches!(
                    id,
                    ShellControlId::Cancel | ShellControlId::Input(InputControl::KeepCurrent)
                );
            }
            !(state.device.is_none()
                && matches!(
                    id,
                    ShellControlId::Input(InputControl::Gamepad(_) | InputControl::Measure)
                ))
        }
    }
}

pub(in crate::client::shell) fn slider_actions(
    id: InputControl,
) -> Option<(ShellAction, ShellAction)> {
    id.calibration().map(|_| {
        (
            ShellAction::InputAdjust(id, -1),
            ShellAction::InputAdjust(id, 1),
        )
    })
}

#[allow(
    clippy::needless_pass_by_value,
    clippy::too_many_arguments,
    reason = "Settings input action boundary"
)]
pub(in crate::client::shell) fn actions(
    mut pending: ResMut<PendingActions>,
    mut state: ResMut<ControlsState>,
    mut page: ResMut<SettingsPage>,
    mut draft: Option<ResMut<InputSettingsDraft>>,
    mut selection: ResMut<InputSettingsSelection>,
    mut dirty: ResMut<NavigationDirty>,
    gamepads: Query<(Entity, &Gamepad)>,
    overlay: Res<ClientOverlay>,
) {
    if *overlay != ClientOverlay::Settings {
        return;
    }
    pending.0.retain(|action| {
        if *action == ShellAction::Back && page.category == Category::Controls {
            if !matches!(state.capture, Capture::Idle)
                || state.measurement.is_some()
                || state.recommendation.is_some()
            {
                state.cancel_transient();
                selection.listening = false;
                page.rebuild = true;
                return false;
            }
            if matches!(state.page, InputPage::Calibration | InputPage::Advanced) {
                state.cancel_transient();
                state.page = InputPage::Gamepad;
                page.rebuild = true;
                dirty.0 = Some(ShellControlId::Input(InputControl::Calibrate));
                return false;
            }
        }
        let Some(draft) = draft.as_deref_mut() else {
            return true;
        };
        match *action {
            ShellAction::Input(id) => {
                activate(id, &mut state, &mut draft.0, &gamepads);
                selection.listening = state.capture.waiting();
                page.rebuild = true;
                dirty.0 = Some(ShellControlId::Input(match id {
                    InputControl::Calibrate => InputControl::MoveDeadzone,
                    InputControl::Advanced => InputControl::AimCommit,
                    InputControl::Measure => InputControl::KeepCurrent,
                    _ => id,
                }));
                false
            }
            ShellAction::InputAdjust(id, direction) => {
                if let Some(field) = id.calibration() {
                    draft
                        .0
                        .adjust_calibration(field, f32::from(direction) * 0.01);
                }
                page.rebuild |= state.undo.take().is_some() | state.recommendation.take().is_some();
                false
            }
            _ => true,
        }
    });
}

fn activate(
    id: InputControl,
    state: &mut ControlsState,
    settings: &mut ClientInputSettings,
    gamepads: &Query<(Entity, &Gamepad)>,
) {
    state.notice.clear();
    if !matches!(id, InputControl::Reset | InputControl::Undo) {
        state.undo = None;
    }
    match id {
        InputControl::KeyboardTab | InputControl::GamepadTab => {
            state.cancel_transient();
            state.undo = None;
            state.page = if id == InputControl::KeyboardTab {
                InputPage::Keyboard
            } else {
                InputPage::Gamepad
            };
        }
        InputControl::Device => {
            let mut connected: Vec<_> = gamepads.iter().map(|(entity, _)| entity).collect();
            connected.sort();
            state.device = connected
                .iter()
                .position(|entity| Some(*entity) == state.device)
                .and_then(|index| connected.get((index + 1) % connected.len()).copied())
                .or_else(|| connected.first().copied());
        }
        InputControl::InvertMove => settings.toggle_inversion(false),
        InputControl::InvertAim => settings.toggle_inversion(true),
        InputControl::Calibrate | InputControl::Advanced => {
            state.page = if id == InputControl::Calibrate {
                InputPage::Calibration
            } else {
                InputPage::Advanced
            };
            state.undo = None;
        }
        InputControl::CancelCapture => state.capture = Capture::Idle,
        InputControl::Swap => {
            if let Capture::Conflict(proposal) = core::mem::take(&mut state.capture) {
                *settings = proposal.settings;
            }
        }
        InputControl::Measure => {
            if state.device.is_some() {
                state.measurement = Some(NeutralMeasurement::default());
                state.recommendation = None;
            }
        }
        InputControl::UseValues => {
            if let Some(recommendation) = state.recommendation.take() {
                recommendation.apply(settings);
            }
        }
        InputControl::KeepCurrent => {
            state.measurement = None;
            state.recommendation = None;
        }
        InputControl::Reset => {
            state.undo = Some(*settings);
            reset_page(state.page, settings);
        }
        InputControl::Undo => {
            if let Some(previous) = state.undo.take() {
                let revision = settings.revision.saturating_add(1);
                *settings = previous;
                settings.revision = revision;
            }
        }
        _ => {
            if let Some(target) = id.target() {
                if matches!(target, BindingTarget::Gamepad(_)) && state.device.is_none() {
                    state.notice = "Connect a controller to change its bindings.".into();
                } else {
                    state.capture = Capture::Arming(id, state.device);
                    state.undo = None;
                }
            }
        }
    }
}

fn reset_page(page: InputPage, settings: &mut ClientInputSettings) {
    let defaults = ClientInputSettings::default();
    match page {
        InputPage::Keyboard => {
            settings.keyboard = defaults.keyboard;
            settings.mouse_primary = defaults.mouse_primary;
        }
        InputPage::Gamepad => {
            settings.gamepad = defaults.gamepad;
            settings.invert_move_y = false;
            settings.invert_aim_y = false;
        }
        InputPage::Calibration => {
            settings.move_deadzone = defaults.move_deadzone;
            settings.aim_deadzone = defaults.aim_deadzone;
        }
        InputPage::Advanced => {
            settings.aim_commit_threshold =
                defaults.aim_commit_threshold.max(settings.aim_deadzone);
            settings.trigger_press = defaults.trigger_press;
            settings.trigger_release = defaults.trigger_release;
        }
    }
    settings.revision = settings.revision.saturating_add(1);
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Settings stick sampling uses the production Gamepad samples"
)]
pub(in crate::client::shell) fn sample_sticks(
    gamepads: Query<(Entity, &Gamepad, Option<&Name>)>,
    mut state: ResMut<ControlsState>,
    mut page: ResMut<SettingsPage>,
    time: Res<Time<Real>>,
    overlay: Res<ClientOverlay>,
    mut dirty: ResMut<NavigationDirty>,
) {
    if *overlay != ClientOverlay::Settings {
        return;
    }
    let previous_device = state.device;
    let previous_name = state.device_name.clone();
    let lost = state
        .device
        .is_some_and(|device| gamepads.get(device).is_err());
    if lost {
        state.cancel_transient();
        state.device = None;
        state.notice = "Controller disconnected. Existing settings were kept.".into();
        page.rebuild = true;
    }
    if state.device.is_none() && !lost {
        state.device = gamepads.iter().map(|(entity, _, _)| entity).min();
    }
    if let Some((_, pad, name)) = state.device.and_then(|device| gamepads.get(device).ok()) {
        state.device_name = name.map_or_else(|| "Controller".into(), |name| name.as_str().into());
        state.raw = [pad.left_stick(), pad.right_stick()];
        let [movement, aim] = state.raw;
        if let Some(measurement) = &mut state.measurement
            && let Some(result) = measurement.sample(time.delta_secs(), movement, aim)
        {
            state.measurement = None;
            match result {
                Ok(recommendation) => {
                    state.recommendation = Some(recommendation);
                    dirty.0 = Some(ShellControlId::Input(InputControl::UseValues));
                }
                Err(error) => {
                    state.notice = error.into();
                    dirty.0 = Some(ShellControlId::Input(InputControl::Measure));
                }
            }
            page.rebuild = true;
        }
    } else {
        state.raw = [Vec2::ZERO; 2];
        state.device_name = "No controller connected".into();
    }
    if previous_device != state.device || previous_name != state.device_name {
        page.rebuild = true;
    }
}

#[allow(
    clippy::needless_pass_by_value,
    reason = "Settings analog slider input"
)]
pub(in crate::client::shell) fn pointer_sliders(
    mouse: Res<ButtonInput<MouseButton>>,
    overlay: Res<ClientOverlay>,
    tracks: Query<(&SliderTrack, &RelativeCursorPosition)>,
    buttons: Query<&ShellButton>,
    draft: Option<ResMut<InputSettingsDraft>>,
    mut page: ResMut<SettingsPage>,
    mut state: ResMut<ControlsState>,
) {
    if *overlay != ClientOverlay::Settings
        || !(mouse.pressed(MouseButton::Left)
            || mouse.just_pressed(MouseButton::Left)
            || mouse.just_released(MouseButton::Left))
    {
        return;
    }
    let Some(mut draft) = draft else {
        return;
    };
    for (track, cursor) in &tracks {
        let Some(fraction) = slider_fraction(cursor) else {
            continue;
        };
        let Ok(button) = buttons.get(track.0) else {
            continue;
        };
        if !allows(button.id, &state) {
            continue;
        }
        if let ShellControlId::Input(id) = button.id
            && let Some(field) = id.calibration()
        {
            let maximum = if matches!(
                field,
                CalibrationField::MoveDeadzone | CalibrationField::AimDeadzone
            ) {
                0.5
            } else {
                1.0
            };
            let value = (fraction * maximum * 100.0).round() / 100.0;
            let current = calibration_value(field, draft.0);
            if (value - current).abs() > f32::EPSILON {
                draft.0.adjust_calibration(field, value - current);
                page.rebuild |= state.recommendation.take().is_some() | state.undo.take().is_some();
            }
            page.defer_save = mouse.pressed(MouseButton::Left);
        }
    }
}

pub(super) fn calibration_value(field: CalibrationField, settings: ClientInputSettings) -> f32 {
    match field {
        CalibrationField::MoveDeadzone => settings.move_deadzone,
        CalibrationField::AimDeadzone => settings.aim_deadzone,
        CalibrationField::AimCommitThreshold => settings.aim_commit_threshold,
        CalibrationField::TriggerPress => settings.trigger_press,
        CalibrationField::TriggerRelease => settings.trigger_release,
    }
}
