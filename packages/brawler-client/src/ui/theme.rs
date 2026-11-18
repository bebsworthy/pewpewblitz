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

pub(crate) fn font(assets: Option<&crate::ClientAssetHandles>, size: f32) -> TextFont {
    assets.map_or_else(
        || TextFont::from_font_size(size).with_font_weight(super::fonts::DISPLAY_WEIGHT),
        |assets| TextFont {
            font: assets.dashboard_font.clone().into(),
            font_size: FontSize::Px(size),
            weight: super::fonts::DISPLAY_WEIGHT,
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
            weight: super::fonts::DISPLAY_WEIGHT,
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
        "/../../assets/brawler/ui/fonts/lilita-one-regular.ttf"
    ));
    skrifa::FontRef::new(FONT)
        .expect("shipped display font parses")
        .charmap()
        .map(character)
        .is_some()
}

#[cfg(test)]
mod tests {
    use parley::{
        FontFamily, LayoutContext, PositionedLayoutItem, StyleProperty,
        fontique::{Blob, FontInfoOverride},
    };
    use skrifa::{MetadataProvider as _, string::StringId};

    /// BRL-0168: player names are free-form Unicode, but Lilita One covers ASCII and Latin-1
    /// only. Every other glyph must fall back to an OS font (Bevy's `system_font_discovery`)
    /// rather than render `.notdef` boxes. BRL-cgcf: those fallbacks are heavy (weight >= 600)
    /// and Greek is sans, not the serif CoreText default. Fallback depends on the platform's
    /// installed fonts, so this checks the macOS development/CI target.
    #[test]
    #[cfg(target_os = "macos")]
    fn names_outside_the_display_font_fall_back_to_heavy_sans_glyphs() {
        // (name, whether its fallback glyphs must come from a heavy face; color emoji has one).
        const NAMES: &[(&str, bool)] = &[
            ("Łukasz Żółć", true),
            ("Nguyễn Văn An", true),
            ("Αλέξης", true),
            ("Жора Кот", true),
            ("李小雷", true),
            ("さくらちゃん", true),
            ("김민준", true),
            ("Pew🎮🔥", false),
        ];
        const ALIAS: &str = "brl-0168-display";
        let display_font = Blob::from(
            include_bytes!(concat!(
                env!("CARGO_MANIFEST_DIR"),
                "/../../assets/brawler/ui/fonts/lilita-one-regular.ttf"
            ))
            .to_vec(),
        );
        // Bevy's own default context: it only discovers system fonts when the client enables
        // `bevy/system_font_discovery`, so this fails if that feature is dropped.
        let mut font_cx = bevy::text::FontCx::default();
        crate::ui::fonts::apply_script_fallbacks(&mut font_cx.collection);
        // Registered under an alias, as `bevy_text` does for a `Handle<Font>`.
        font_cx.collection.register_fonts(
            display_font.clone(),
            Some(FontInfoOverride {
                family_name: Some(ALIAS),
                ..Default::default()
            }),
        );
        let mut layout_cx = LayoutContext::<()>::new();
        let weight = super::hud_text(&bevy::asset::Handle::default(), 24.0)
            .0
            .weight;
        let mut layout = |text: &str, weight: bevy::text::FontWeight| {
            let mut builder = layout_cx.ranged_builder(&mut font_cx.context, text, 1.0, true);
            builder.push_default(StyleProperty::FontFamily(FontFamily::named(ALIAS)));
            builder.push_default(StyleProperty::FontSize(24.0));
            builder.push_default(StyleProperty::FontWeight(weight.into()));
            let mut layout = builder.build(text);
            layout.break_all_lines(None);
            layout
        };

        // The display weight selects the single Lilita One face unchanged (no faux bold).
        let display_width = layout("Bot", weight).full_width();
        assert!(
            (display_width - layout("Bot", bevy::text::FontWeight::NORMAL).full_width()).abs()
                < 1e-3,
            "Lilita One metrics do not depend on the requested weight"
        );

        for &(name, heavy) in NAMES {
            assert!(
                brawler::lobby::normalize_proposed_display_name(name).is_ok(),
                "{name} is a valid display name"
            );
            assert!(
                !name.chars().all(super::display_font_has_glyph),
                "{name} needs a glyph outside Lilita One"
            );
            let text = format!("Bot {name}");
            let layout = layout(&text, weight);

            let mut display_font_glyphs = 0;
            for line in layout.lines() {
                for item in line.items() {
                    let PositionedLayoutItem::GlyphRun(run) = item else {
                        continue;
                    };
                    let font = run.run().font();
                    let from_display_font = font.data.id() == display_font.id();
                    for glyph in run.glyphs() {
                        assert_ne!(glyph.id, 0, "{name} rendered a missing-glyph box");
                        display_font_glyphs += usize::from(from_display_font);
                    }
                    if !from_display_font {
                        assert_heavy_sans_fallback(name, heavy, font);
                    }
                }
            }
            assert!(
                display_font_glyphs >= "Bot".len(),
                "{name}: Latin text keeps Lilita One"
            );
        }
    }

    /// A fallback face is heavy (unless the name's script has a single-weight face, such as
    /// color emoji) and Greek uses the configured sans family.
    #[cfg(target_os = "macos")]
    fn assert_heavy_sans_fallback(name: &str, heavy: bool, font: &parley::FontData) {
        let face = skrifa::FontRef::from_index(font.data.as_ref(), font.index)
            .expect("fallback face parses");
        let face_name = face
            .localized_strings(StringId::FULL_NAME)
            .english_or_first()
            .map(|name| name.to_string())
            .unwrap_or_default();
        if heavy {
            assert!(
                face.attributes().weight.value() >= 600.0,
                "{name}: fallback {face_name} is heavy"
            );
        }
        if name.chars().any(|c| ('\u{370}'..='\u{3FF}').contains(&c)) {
            assert!(
                face_name.starts_with("Helvetica Neue"),
                "{name}: Greek falls back to sans Helvetica Neue, not {face_name}"
            );
        }
    }
}
