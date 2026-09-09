//! Visual state shared by image-backed shell controls, independent of screen actions.

use bevy::prelude::*;

#[derive(Component)]
pub(crate) struct ImageButtonArt {
    pub(crate) normal: Handle<Image>,
    pub(crate) focus: Handle<Image>,
}

/// Callers supply whether their screen owns input; inactive surfaces never show focus/hover.
#[derive(Clone, Copy)]
pub(crate) struct ImageButtonState {
    pub(crate) active: bool,
    pub(crate) disabled: bool,
    pub(crate) focused: bool,
    pub(crate) interaction: Interaction,
}

impl ImageButtonState {
    pub(crate) fn shows_focus(self) -> bool {
        self.active && !self.disabled && self.focused
    }

    pub(crate) fn tint(self) -> Color {
        if self.disabled {
            return Color::srgba(0.6, 0.6, 0.6, 0.7);
        }
        match self.interaction {
            Interaction::Pressed if self.active => Color::srgb(0.8, 0.8, 0.8),
            Interaction::Hovered if self.active => Color::srgb(1.08, 1.08, 1.08),
            _ => Color::WHITE,
        }
    }

    pub(crate) fn apply(self, image: &mut ImageNode, art: &ImageButtonArt) {
        image.image = if self.shows_focus() {
            art.focus.clone()
        } else {
            art.normal.clone()
        };
        image.color = self.tint();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn losing_screen_ownership_restores_normal_art() {
        let mut images = Assets::<Image>::default();
        let art = ImageButtonArt {
            normal: images.add(Image::default()),
            focus: images.add(Image::default()),
        };
        let mut image = ImageNode::default();
        let focused = ImageButtonState {
            active: true,
            disabled: false,
            focused: true,
            interaction: Interaction::Hovered,
        };
        focused.apply(&mut image, &art);
        assert_eq!(image.image, art.focus);
        ImageButtonState {
            active: false,
            ..focused
        }
        .apply(&mut image, &art);
        assert_eq!(image.image, art.normal);
        assert_eq!(image.color, Color::WHITE);
    }

    #[test]
    fn inactive_surface_suppresses_stale_focus_and_pointer_state() {
        for interaction in [
            Interaction::Pressed,
            Interaction::Hovered,
            Interaction::None,
        ] {
            let state = ImageButtonState {
                active: false,
                disabled: false,
                focused: true,
                interaction,
            };
            assert!(!state.shows_focus());
            assert_eq!(state.tint(), Color::WHITE);
        }
    }

    #[test]
    fn disabled_beats_focus_and_press() {
        let state = ImageButtonState {
            active: true,
            disabled: true,
            focused: true,
            interaction: Interaction::Pressed,
        };
        assert!(!state.shows_focus());
        assert_eq!(state.tint(), Color::srgba(0.6, 0.6, 0.6, 0.7));
    }

    #[test]
    fn focus_art_survives_pointer_press_and_release() {
        let state = ImageButtonState {
            active: true,
            disabled: false,
            focused: true,
            interaction: Interaction::Pressed,
        };
        assert!(state.shows_focus());
        assert_ne!(state.tint(), Color::WHITE);
        let released = ImageButtonState {
            interaction: Interaction::None,
            ..state
        };
        assert!(released.shows_focus());
        assert_eq!(released.tint(), Color::WHITE);
    }
}
