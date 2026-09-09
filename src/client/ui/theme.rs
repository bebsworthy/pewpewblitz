//! Approved player-shell palette and bundled typography.

use bevy::prelude::*;

pub(crate) const FONT_PATH: &str = "brawler/ui/fonts/lilita-one-regular.ttf";

pub(crate) const INK: Color = Color::srgb(0.025, 0.10, 0.145);
pub(crate) const CREAM: Color = Color::srgb(0.98, 0.92, 0.81);
pub(crate) const TEAL: Color = Color::srgb(0.32, 0.64, 0.62);
pub(crate) const ORANGE: Color = Color::srgb(1.0, 0.40, 0.025);

pub(crate) fn font(assets: Option<&crate::client::ClientAssetHandles>, size: f32) -> TextFont {
    assets.map_or_else(
        || TextFont::from_font_size(size),
        |assets| TextFont {
            font: assets.dashboard_font.clone().into(),
            font_size: FontSize::Px(size),
            ..default()
        },
    )
}
