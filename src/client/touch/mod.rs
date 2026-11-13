//! Touch twin-stick controls: a floating movement stick, a drag-to-aim attack stick that fires on
//! release (tap = auto-aim), and a drag-aimed ultimate button.
//!
//! Touch is one more device sampler. It runs between `sample_local_input` and
//! `resolve_targeted_ultimate_input` and writes the same `PendingLocalActions` intent that
//! keyboard, mouse, and controller produce, so the wire `FighterInput` is unchanged. The
//! [`TouchControls`] resource is the presentation seam: the placeholder overlay here, and the
//! touch-first HUD later, read its layout and live stick state.

mod model;
mod overlay;
mod sample;
#[cfg(test)]
mod tests;

pub(crate) use model::TouchControls;
pub(super) use overlay::TouchControlsOverlayPlugin;
pub(super) use sample::apply_touch_controls;
