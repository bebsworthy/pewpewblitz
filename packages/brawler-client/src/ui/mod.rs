//! Shared client-only shell presentation.
//! Screens own actions and focus ownership; this module owns reusable visual policy.

pub(crate) mod theme;

pub(crate) mod controls;
pub(crate) mod feedback;
pub(crate) mod focus_visibility;
pub(crate) mod fonts;
#[cfg(feature = "automation")]
pub(crate) mod gallery;
pub(crate) mod icons;
#[cfg(test)]
pub(crate) mod layout_harness;

pub(crate) mod motifs;

pub(crate) mod press;

pub(crate) mod scroll;

pub(crate) mod slider;

pub(crate) mod surface;

pub(crate) mod touch_target;

pub(crate) mod viewport;
