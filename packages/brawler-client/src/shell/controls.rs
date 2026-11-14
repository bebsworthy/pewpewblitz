//! Settings-owned input editing. Widgets remain presentation-only.
mod calibration_view;
mod capture;
mod model;
mod view;

pub(super) use calibration_view::present_plots;
pub(super) use capture::capture_input;
pub(super) use model::{
    ControlsState, InputControl, actions, allows, pointer_sliders, sample_sticks, slider_actions,
};
pub(super) use view::{detail_text, present_values, spawn_details, spawn_options};
#[cfg(test)]
mod tests;
