use bevy::{prelude::*, ui::InteractionDisabled};

use crate::{
    audio::{
        registry::{AudioProducerRegistration, AudioProducerRegistrationAppExt},
        request::{AudioCueKey, AudioRequest, cue_keys},
    },
    ui::{
        feedback::{UiFeedback, add_ui_feedback},
        press::ReleaseActivation,
        surface::{Surface, SurfaceState},
    },
};

use super::AudioProducerSequence;

const UI_RANK: u16 = 90;
const UI_KEYS: &[AudioCueKey] = &[
    cue_keys::UI_CLICK,
    cue_keys::UI_CONFIRM,
    cue_keys::UI_BACK,
    cue_keys::UI_MOVE,
];

/// A keyboard/gamepad press within this window of another UI press is the same gesture seen
/// twice (for example by two screen owners); only the first sounds.
const PRESS_SPACING_SECONDS: f64 = 0.05;
/// Focus and slider ticks are at most this often, so a quick run of arrow presses, a stick sweep
/// or a slider drag reads as a soft ticking rather than a buzz.
const MOVE_SPACING_SECONDS: f64 = 0.07;

pub(super) fn register(app: &mut App) {
    add_ui_feedback(app);
    app.try_register_audio_producer(AudioProducerRegistration {
        id: "ui",
        rank: UI_RANK,
        cue_keys: UI_KEYS,
    })
    .expect("UI audio producer registration must be valid");
}

/// Presentation-only memory for UI sound spacing (real time in seconds); never input truth.
#[derive(Default, Debug)]
pub(super) struct UiSoundSpacing {
    last_press: Option<f64>,
    last_move: Option<f64>,
}

impl UiSoundSpacing {
    /// A pointer release always sounds (each is a distinct physical press) and opens the window.
    fn pointer_press(&mut self, now: f64) {
        self.last_press = Some(now);
    }

    /// A keyboard/gamepad activation or Back sounds unless a UI press just sounded.
    fn admit_press(&mut self, now: f64) -> bool {
        if self
            .last_press
            .is_some_and(|last| now - last < PRESS_SPACING_SECONDS)
        {
            return false;
        }
        self.last_press = Some(now);
        true
    }

    /// A focus/slider tick sounds unless a press or another tick just sounded.
    fn admit_move(&mut self, now: f64) -> bool {
        if self
            .last_press
            .is_some_and(|last| now - last < PRESS_SPACING_SECONDS)
            || self
                .last_move
                .is_some_and(|last| now - last < MOVE_SPACING_SECONDS)
        {
            return false;
        }
        self.last_move = Some(now);
        true
    }
}

/// One sound per control activated this frame through the shared BRL-0156 press model (an
/// undragged pointer/touch release), plus the keyboard/gamepad [`UiFeedback`] facts screens
/// publish (BRL-ek8p). Screens keep owning what the activation means; this only reads the
/// control's semantic surface role.
#[allow(
    clippy::type_complexity,
    clippy::needless_pass_by_value,
    reason = "the query declares the shared press model and surface role inputs"
)]
pub(super) fn produce_ui_audio_requests(
    controls: Query<(
        &ReleaseActivation,
        Option<&Surface>,
        Option<&SurfaceState>,
        Has<InteractionDisabled>,
    )>,
    mut feedback: MessageReader<UiFeedback>,
    time: Option<Res<Time<Real>>>,
    mut requests: MessageWriter<AudioRequest>,
    mut sequence: Local<AudioProducerSequence>,
    mut spacing: Local<UiSoundSpacing>,
) {
    let now = time.map_or(0.0, |time| time.elapsed_secs_f64());
    for (activation, surface, state, disabled) in &controls {
        if activation.activated() && !disabled && !state.is_some_and(|state| state.disabled) {
            spacing.pointer_press(now);
            requests.write(AudioRequest::once(
                ui_cue(surface.copied()),
                sequence.next(UI_RANK),
            ));
        }
    }
    for fact in feedback.read() {
        let cue = match *fact {
            UiFeedback::Activated(surface) => spacing.admit_press(now).then(|| ui_cue(surface)),
            UiFeedback::Back => spacing.admit_press(now).then_some(cue_keys::UI_BACK),
            UiFeedback::Moved | UiFeedback::Stepped => {
                spacing.admit_move(now).then_some(cue_keys::UI_MOVE)
            }
        };
        if let Some(cue) = cue {
            requests.write(AudioRequest::once(cue, sequence.next(UI_RANK)));
        }
    }
}

const fn ui_cue(surface: Option<Surface>) -> AudioCueKey {
    match surface {
        Some(Surface::Primary) => cue_keys::UI_CONFIRM,
        Some(Surface::Back) => cue_keys::UI_BACK,
        _ => cue_keys::UI_CLICK,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn surface_roles_select_confirm_back_or_click() {
        assert_eq!(ui_cue(Some(Surface::Primary)), cue_keys::UI_CONFIRM);
        assert_eq!(ui_cue(Some(Surface::Back)), cue_keys::UI_BACK);
        assert_eq!(ui_cue(Some(Surface::Action)), cue_keys::UI_CLICK);
        assert_eq!(ui_cue(Some(Surface::Danger)), cue_keys::UI_CLICK);
        assert_eq!(ui_cue(None), cue_keys::UI_CLICK);
    }

    #[test]
    fn spacing_drops_repeated_presses_and_bounds_ticks() {
        let mut spacing = UiSoundSpacing::default();
        assert!(spacing.admit_press(1.0));
        assert!(
            !spacing.admit_press(1.0),
            "the same press seen twice sounds once"
        );
        assert!(!spacing.admit_move(1.02), "no tick right after a press");
        assert!(spacing.admit_press(1.2));
        assert!(spacing.admit_move(1.3));
        assert!(
            !spacing.admit_move(1.33),
            "held or rapid moves are rate limited"
        );
        assert!(spacing.admit_move(1.38));
        spacing.pointer_press(2.0);
        assert!(
            !spacing.admit_press(2.01),
            "a key press right after a pointer press is silent"
        );
        assert!(!spacing.admit_move(2.01));
        assert!(spacing.admit_press(2.1));
    }

    #[test]
    fn a_held_arrow_sweep_ticks_at_most_once_per_spacing_window() {
        let mut spacing = UiSoundSpacing::default();
        // Thirty frames at 120 Hz with a move fact every frame: 0.25 s of ticks.
        let ticks = (0..30)
            .filter(|frame| spacing.admit_move(f64::from(*frame) / 120.0))
            .count();
        assert_eq!(ticks, 4);
    }
}
