//! Fallback faces for display-type glyphs outside Lilita One (BRL-cgcf).
//!
//! Lilita One covers ASCII and Latin-1 only; player names may contain any script, so the rest
//! falls back to OS fonts that fontique picks per script. Two levers keep those fallbacks close
//! to the heavy sans display face without bundling fonts:
//! - every display `TextFont` requests [`DISPLAY_WEIGHT`], so fontique picks each fallback
//!   family's heavy face;
//! - [`SCRIPT_FALLBACKS`] replaces platform defaults that are serif or have no heavy face.
//!
//! Emoji are the exception (BRL-z2hf): Android's system Noto Color Emoji is `COLRv1`, which the
//! swash rasterizer behind `bevy_text` cannot paint, so emoji rendered as nothing. Android loads
//! the bundled `COLRv0` [`EMOJI_FONT_PATH`] and puts it first in the `Emoji` generic family that
//! parley queries for every emoji cluster. Desktop keeps its system emoji (Apple Color Emoji is
//! sbix, which swash paints).

use bevy::prelude::*;
use parley::fontique::{Collection, Script};

/// Weight every display-type `TextFont` requests. Lilita One ships one static face, which
/// fontique selects at any weight, and `bevy_text` ignores parley's synthetic-bold suggestion
/// (it rasterizes with variation coordinates only), so Lilita glyphs are unchanged; the weight
/// only selects the heavy face of each fallback family.
pub(crate) const DISPLAY_WEIGHT: FontWeight = FontWeight::BOLD;

/// Per-script fallback families, in preference order, for platforms whose defaults are wrong
/// for display type. On macOS fontique asks CoreText per script, which returns serif Athelas for
/// Greek and Heiti SC (no face heavier than 400) for Han.
#[cfg(target_os = "macos")]
const SCRIPT_FALLBACKS: &[(&[u8; 4], &[&str])] = &[
    (b"Latn", &["Helvetica Neue"]),
    (b"Grek", &["Helvetica Neue"]),
    (b"Cyrl", &["Helvetica Neue"]),
    (b"Hani", &["Hiragino Sans", "Hiragino Sans GB", "Heiti SC"]),
    (b"Hira", &["Hiragino Sans"]),
    (b"Kana", &["Hiragino Sans"]),
    (b"Hang", &["Apple SD Gothic Neo"]),
];

/// Android keeps the `fonts.xml` fallbacks fontique already reads: they are sans Noto/Roboto
/// families, and [`DISPLAY_WEIGHT`] selects their heavier faces where the device ships them.
#[cfg(not(target_os = "macos"))]
const SCRIPT_FALLBACKS: &[(&[u8; 4], &[&str])] = &[];

/// Twemoji Mozilla 0.7.0 (`COLRv0/CPAL`, CC BY 4.0; see `assets/licenses/THIRD_PARTY.md`). Every
/// client ships the file, but only Android loads it.
#[cfg(any(target_os = "android", test))]
pub(crate) const EMOJI_FONT_PATH: &str = "brawler/ui/fonts/twemoji-mozilla.ttf";

/// Family name embedded in [`EMOJI_FONT_PATH`]; `bevy_text` registers every `Font` asset under it.
#[cfg(any(target_os = "android", test))]
const EMOJI_FAMILY: &str = "Twemoji Mozilla";

/// Installs the display fallback configuration once at startup.
pub(crate) fn add_fallback_fonts(app: &mut App) {
    app.add_systems(Startup, configure_fallback_fonts);
    #[cfg(target_os = "android")]
    app.add_systems(Startup, load_bundled_emoji_font)
        .add_systems(Update, install_bundled_emoji_font);
}

/// Keeps the bundled emoji `Font` asset alive; `bevy_text` rebuilds the collection without it
/// if the asset is ever dropped.
#[cfg(target_os = "android")]
#[derive(Resource)]
struct BundledEmojiFont {
    _handle: Handle<Font>,
    installed: bool,
}

#[cfg(target_os = "android")]
#[allow(
    clippy::needless_pass_by_value,
    reason = "every parameter is a Bevy system parameter owned by the scheduling runtime"
)]
fn load_bundled_emoji_font(mut commands: Commands, asset_server: Res<AssetServer>) {
    commands.insert_resource(BundledEmojiFont {
        _handle: asset_server.load(EMOJI_FONT_PATH),
        installed: false,
    });
}

/// Once `bevy_text` has registered the loaded font, prefers it for emoji and relays out every
/// text already shown (its emoji clusters were shaped before the font existed).
#[cfg(target_os = "android")]
fn install_bundled_emoji_font(
    mut emoji: ResMut<BundledEmojiFont>,
    mut font_cx: ResMut<bevy::text::FontCx>,
    mut texts: Query<&mut TextFont>,
) {
    if emoji.installed || !prefer_bundled_emoji(&mut font_cx.collection) {
        return;
    }
    emoji.installed = true;
    info!(family = EMOJI_FAMILY, "bundled emoji font installed");
    for mut text in &mut texts {
        text.set_changed();
    }
}

/// Puts [`EMOJI_FAMILY`] ahead of the platform's `Emoji` generic families, which stay as
/// fallbacks for emoji newer than the bundled set. Returns false until the family is registered.
#[cfg(any(target_os = "android", test))]
pub(crate) fn prefer_bundled_emoji(collection: &mut Collection) -> bool {
    let Some(bundled) = collection.family_id(EMOJI_FAMILY) else {
        return false;
    };
    let platform: Vec<_> = collection
        .generic_families(parley::fontique::GenericFamily::Emoji)
        .filter(|family| *family != bundled)
        .collect();
    collection.set_generic_families(
        parley::fontique::GenericFamily::Emoji,
        std::iter::once(bundled).chain(platform),
    );
    true
}

/// `bevy_text` clears these only when a `Font` asset is removed, which the client never does.
fn configure_fallback_fonts(mut font_cx: ResMut<bevy::text::FontCx>) {
    apply_script_fallbacks(&mut font_cx.collection);
}

/// Replaces each listed script's fallback with the families this system has; scripts with none
/// keep the platform default.
pub(crate) fn apply_script_fallbacks(collection: &mut Collection) {
    for (script, names) in SCRIPT_FALLBACKS {
        let families = names
            .iter()
            .filter_map(|name| collection.family_id(name))
            .collect::<Vec<_>>();
        if families.is_empty() {
            debug!(script = ?Script::from_bytes(**script), "no display fallback family installed");
            continue;
        }
        collection.set_fallbacks(Script::from_bytes(**script), families.into_iter());
    }
}

#[cfg(test)]
mod tests {
    use bevy::text::{FontCx, FontSmoothing, ScaleCx, get_outlined_glyph_texture};
    use parley::{
        FontFamily, LayoutContext, PositionedLayoutItem, StyleProperty,
        fontique::{Blob, FontInfoOverride},
    };

    const DISPLAY_ALIAS: &str = "brl-z2hf-display";
    const SIZE: f32 = 32.0;

    fn shipped_font(path: &str) -> Blob<u8> {
        let path = std::path::Path::new(crate::REPOSITORY_ROOT)
            .join("assets")
            .join(path);
        Blob::from(std::fs::read(&path).unwrap_or_else(|error| panic!("{path:?}: {error}")))
    }

    /// BRL-z2hf: Android's `COLRv1` Noto Color Emoji rasterizes to nothing through swash. With the
    /// bundled `COLRv0` font preferred for `Emoji`, a display-font name's 🎮 cluster shapes in that
    /// font and Bevy's own glyph rasterizer (`ColorOutline(0)`, `ColorBitmap`, `Outline`) paints
    /// a non-empty colour glyph. Runs on any host: the bundled font leads the generic family
    /// whatever system emoji font is installed.
    #[test]
    fn bundled_emoji_font_paints_a_colour_glyph_for_emoji_in_a_display_name() {
        let emoji = shipped_font(super::EMOJI_FONT_PATH);
        let display = shipped_font(crate::ui::theme::FONT_PATH);
        let mut font_cx = FontCx::default();
        assert!(
            !super::prefer_bundled_emoji(&mut font_cx.collection),
            "nothing to prefer before the font asset is registered"
        );
        // As `bevy_text` registers a loaded `Font` asset: under its embedded family name (the
        // emoji font) or an asset alias (the display handle).
        font_cx.collection.register_fonts(emoji.clone(), None);
        font_cx.collection.register_fonts(
            display,
            Some(FontInfoOverride {
                family_name: Some(DISPLAY_ALIAS),
                ..Default::default()
            }),
        );
        assert!(super::prefer_bundled_emoji(&mut font_cx.collection));

        let text = "Pew 🎮";
        let mut layout_cx = LayoutContext::<()>::new();
        let mut builder = layout_cx.ranged_builder(&mut font_cx.context, text, 1.0, true);
        builder.push_default(StyleProperty::FontFamily(FontFamily::named(DISPLAY_ALIAS)));
        builder.push_default(StyleProperty::FontSize(SIZE));
        builder.push_default(StyleProperty::FontWeight(super::DISPLAY_WEIGHT.into()));
        let mut layout = builder.build(text);
        layout.break_all_lines(None);

        let mut scale_cx = ScaleCx::default();
        let mut emoji_glyphs = 0;
        for line in layout.lines() {
            for item in line.items() {
                let PositionedLayoutItem::GlyphRun(run) = item else {
                    continue;
                };
                let font = run.run().font();
                if font.data.id() != emoji.id() {
                    continue;
                }
                let face = swash::FontRef::from_index(font.data.as_ref(), font.index as usize)
                    .expect("bundled emoji font parses");
                let mut scaler = scale_cx.0.builder(face).size(SIZE).build();
                for glyph in run.glyphs() {
                    let glyph_id = u16::try_from(glyph.id).expect("16-bit glyph id");
                    assert_ne!(glyph_id, 0, "🎮 mapped to .notdef");
                    let (image, _, alpha_mask) = get_outlined_glyph_texture(
                        &mut scaler,
                        glyph_id,
                        FontSmoothing::AntiAliased,
                    )
                    .expect("bevy_text rasterizes the emoji glyph");
                    assert!(!alpha_mask, "the COLRv0 layers paint in colour");
                    let pixels = image.data.as_deref().expect("glyph pixels");
                    let painted = pixels.chunks_exact(4).filter(|rgba| rgba[3] > 0).count();
                    assert!(painted > 0, "🎮 rasterized empty");
                    emoji_glyphs += 1;
                }
            }
        }
        assert_eq!(emoji_glyphs, 1, "🎮 shapes in the bundled emoji font");
    }
}
