use bevy::{prelude::*, ui::InteractionDisabled};

use crate::{
    audio::{
        registry::{AudioProducerRegistration, AudioProducerRegistrationAppExt},
        request::{AudioCueKey, AudioRequest, cue_keys},
    },
    ui::{
        press::ReleaseActivation,
        surface::{Surface, SurfaceState},
    },
};

use super::AudioProducerSequence;

const UI_RANK: u16 = 90;
const UI_KEYS: &[AudioCueKey] = &[cue_keys::UI_CLICK, cue_keys::UI_CONFIRM, cue_keys::UI_BACK];

pub(super) fn register(app: &mut App) {
    app.try_register_audio_producer(AudioProducerRegistration {
        id: "ui",
        rank: UI_RANK,
        cue_keys: UI_KEYS,
    })
    .expect("UI audio producer registration must be valid");
}

/// One sound per control activated this frame through the shared BRL-0156 press model (an
/// undragged pointer/touch release). Screens keep owning what the activation means; this only
/// reads the control's semantic surface role.
#[allow(
    clippy::type_complexity,
    reason = "the query declares the shared press model and surface role inputs"
)]
pub(super) fn produce_ui_audio_requests(
    controls: Query<(
        &ReleaseActivation,
        Option<&Surface>,
        Option<&SurfaceState>,
        Has<InteractionDisabled>,
    )>,
    mut requests: MessageWriter<AudioRequest>,
    mut sequence: Local<AudioProducerSequence>,
) {
    for (activation, surface, state, disabled) in &controls {
        if activation.activated() && !disabled && !state.is_some_and(|state| state.disabled) {
            requests.write(AudioRequest::once(
                ui_cue(surface.copied()),
                sequence.next(UI_RANK),
            ));
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
}
