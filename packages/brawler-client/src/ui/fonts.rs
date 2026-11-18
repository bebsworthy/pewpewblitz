//! Fallback faces for display-type glyphs outside Lilita One (BRL-cgcf).
//!
//! Lilita One covers ASCII and Latin-1 only; player names may contain any script, so the rest
//! falls back to OS fonts that fontique picks per script. Two levers keep those fallbacks close
//! to the heavy sans display face without bundling fonts:
//! - every display `TextFont` requests [`DISPLAY_WEIGHT`], so fontique picks each fallback
//!   family's heavy face;
//! - [`SCRIPT_FALLBACKS`] replaces platform defaults that are serif or have no heavy face.

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

/// Installs the display fallback configuration once at startup.
pub(crate) fn add_fallback_fonts(app: &mut App) {
    app.add_systems(Startup, configure_fallback_fonts);
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
