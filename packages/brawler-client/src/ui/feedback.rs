//! Presentation-only feedback facts for keyboard and gamepad shell input (BRL-ek8p).
//!
//! Pointer and touch activation is observed directly from [`super::press::ReleaseActivation`].
//! Keyboard, gamepad and platform-Back input is interpreted by each owning screen, so those
//! screens write one [`UiFeedback`] where their input actually activated a control, went back,
//! moved focus or stepped a slider. Consumers (the UI audio producer) only present it; screens keep
//! owning what the input means.

use bevy::prelude::*;

use super::surface::Surface;

/// One non-pointer shell input outcome this frame.
#[derive(Message, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum UiFeedback {
    /// The focused control was activated by Enter/Space or controller South. The control's
    /// surface role, read by the screen at activation time, selects confirm/back/click.
    Activated(Option<Surface>),
    /// Escape, controller East or the platform Back request backed out of a screen or overlay.
    Back,
    /// Keyboard/gamepad focus moved to another control.
    Moved,
    /// A slider value changed by one or more steps (keyboard, gamepad, mouse or touch).
    Stepped,
}

/// Registers the message once for every emitter and consumer.
pub(crate) fn add_ui_feedback(app: &mut App) {
    app.add_message::<UiFeedback>();
}
