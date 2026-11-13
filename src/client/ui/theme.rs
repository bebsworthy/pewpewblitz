//! Approved player-shell palette and bundled typography.

use bevy::prelude::*;

pub(crate) const FONT_PATH: &str = "brawler/ui/fonts/lilita-one-regular.ttf";

// BRL-0140 player-shell tokens; `docs/design/player-shell/README.md` owns the values.
pub(crate) const SHELL_BG_TOP: Color = Color::srgb_u8(0x2E, 0x7B, 0xFF);
pub(crate) const SHELL_BG_BOTTOM: Color = Color::srgb_u8(0x1B, 0x3F, 0xA8);
pub(crate) const PANEL: Color = Color::srgb_u8(0x24, 0x30, 0x8F);
pub(crate) const PANEL_RAISED: Color = Color::srgb_u8(0x3B, 0x5B, 0xDB);
/// Outlines, text stroke, bottom lips, and dark identity strips.
pub(crate) const INK: Color = Color::srgb_u8(0x0E, 0x16, 0x38);
pub(crate) const PLAY: Color = Color::srgb_u8(0xFF, 0x8A, 0x1F);
pub(crate) const PLAY_HIGHLIGHT: Color = Color::srgb_u8(0xFF, 0xC2, 0x1A);
pub(crate) const ACCENT: Color = Color::srgb_u8(0x38, 0xD6, 0xFF);
pub(crate) const TEXT: Color = Color::srgb_u8(0xFF, 0xFF, 0xFF);

/// The shared full-screen shell background: the `shell-bg` vertical gradient over its bottom
/// color, which remains the fallback fill.
pub(crate) fn shell_background() -> (BackgroundColor, BackgroundGradient) {
    (
        BackgroundColor(SHELL_BG_BOTTOM),
        BackgroundGradient::from(LinearGradient::to_bottom(vec![
            ColorStop::auto(SHELL_BG_TOP),
            ColorStop::auto(SHELL_BG_BOTTOM),
        ])),
    )
}

/// White display type keeps an `ink` stroke, approximated by a size-scaled shadow because Bevy
/// UI text has no outline stroke.
pub(crate) fn type_stroke(size: f32) -> TextShadow {
    TextShadow {
        offset: Vec2::splat((size * 0.07).max(1.5)),
        color: INK,
    }
}

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
        type_stroke(size),
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
