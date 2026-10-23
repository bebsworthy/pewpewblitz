//! Approved player-shell palette and bundled typography.

use bevy::prelude::*;

pub(crate) const FONT_PATH: &str = "brawler/ui/fonts/lilita-one-regular.ttf";

// Representative dark navy sampled from the approved mockup (#081932).
pub(crate) const INK: Color = Color::srgb(8.0 / 255.0, 25.0 / 255.0, 50.0 / 255.0);
pub(crate) const CREAM: Color = Color::srgb(0.98, 0.92, 0.81);
pub(crate) const TEAL: Color = Color::srgb(0.32, 0.64, 0.62);
pub(crate) const CYAN: Color = Color::srgb(0.05, 0.92, 0.88);
pub(crate) const ORANGE: Color = Color::srgb(1.0, 0.40, 0.025);
// Approved display-type stroke navy (#0E1638, BRL-0140 art direction).
pub(crate) const TYPE_STROKE: Color = Color::srgb(14.0 / 255.0, 22.0 / 255.0, 56.0 / 255.0);

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

/// The shipped display font, or Bevy's default font in headless schedule tests without assets.
pub(crate) fn display_font_handle(asset_server: Option<&AssetServer>) -> Handle<Font> {
    asset_server.map_or_else(Handle::default, |server| server.load(FONT_PATH))
}

/// In-match display type: Lilita One with the navy stroke approximated by a size-scaled drop
/// shadow, because Bevy UI text has no outline stroke.
pub(crate) fn hud_text(font: &Handle<Font>, size: f32) -> (TextFont, TextShadow) {
    (
        TextFont {
            font: font.clone().into(),
            font_size: FontSize::Px(size),
            ..default()
        },
        TextShadow {
            offset: Vec2::splat((size * 0.07).max(1.5)),
            color: TYPE_STROKE,
        },
    )
}

/// Whether the shipped display font maps `character` to a glyph.
#[cfg(test)]
pub(crate) fn display_font_has_glyph(character: char) -> bool {
    use skrifa::MetadataProvider as _;
    static FONT: &[u8] = include_bytes!(concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/assets/brawler/ui/fonts/lilita-one-regular.ttf"
    ));
    skrifa::FontRef::new(FONT)
        .expect("shipped display font parses")
        .charmap()
        .map(character)
        .is_some()
}
