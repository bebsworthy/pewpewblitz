# Outcome

Player names stay readable in overhead plates and the HUD roster, whatever characters the player chose.

# Context

- BRL-0145 moved all in-match HUD and overhead text to Lilita One and added a glyph-coverage test for the code-owned string templates.
- Display names are validated by `src/lobby.rs::normalize_proposed_display_name` (NFC, grapheme/byte bounds, control-character rejection) and may contain any script. Lilita One covers Latin only, so names in other scripts render missing-glyph boxes.

# Scope

Choose between a bundled fallback font chain for names and a validation-time character restriction. Then apply it to overhead names and roster rows.

# Acceptance criteria

- A name containing characters outside Lilita One renders without missing-glyph boxes, or is rejected at validation with clear feedback.
- A focused test covers the chosen rule.


# Research (2026-11-14)

- Lilita One (`assets/brawler/ui/fonts/lilita-one-regular.ttf`, 26.8 KB) maps only 225 code points: ASCII plus 94/96 of Latin-1. Coverage is 10/128 Latin Extended-A (no `ł`, `ő`, `č`), 0 Greek, 0 Cyrillic, 0 Vietnamese, 0 CJK/kana/Hangul, and no emoji. So the gap is not only "other scripts": most European names outside Western Europe box today.
- Bevy 0.19.1 text uses parley 0.9 / fontique 0.9 (not cosmic-text). `TextFont { font: FontSource::Handle(..) }` resolves to one named family (the asset alias). For each character cluster that family does not cover, parley's `FontSelector` asks fontique for the fallback families of the cluster's script (`Query::set_fallbacks(FallbackKey(script, locale))`, always plus the `Hani` list for common punctuation) and, for emoji clusters, the generic `Emoji` family. So per-glyph fallback already exists. Without system fonts, those fallback lists are empty and the glyph becomes `.notdef` (the box).
- The fallback lists come from either `Collection::append_fallbacks` (fonts the app registers itself) or, with fontique's `system` feature, the platform backend: CoreText on macOS, `/system/etc/fonts.xml` plus `/system/fonts` on Android, DirectWrite, fontconfig. Bevy exposes that as its `system_font_discovery` cargo feature (`bevy_text/system_font_discovery` → `parley/system`). The repo does not enable it.
- macOS measurement (scratch fontique 0.9.0 program, release): system collection built in 255 ms cold and 21 ms warm. Script fallbacks resolved: Cyrl/Latn → Helvetica, Grek → Athelas, Hani → Heiti SC, Hira → Hiragino Sans, Hang → Apple SD Gothic Neo, Arab → Geeza Pro, Thai → Thonburi, Emoji → Apple Color Emoji. Bevy rasterizes color outlines and color bitmaps (`font_atlas.rs` uses swash `ColorOutline`/`ColorBitmap`/`Outline` sources), so emoji can render.
- Android: the Pixel/AOSP system image ships Noto for every major script plus Noto Color Emoji, and fontique's Android backend reads them. It is not verified on a device here (no device attached; see Deferred).
- Size of a bundled alternative: a Noto Sans subset for Latin Extended + Greek + Cyrillic + Vietnamese is roughly 0.3-0.6 MB per weight. CJK needs a Noto Sans CJK region subset of about 5-16 MB, and color emoji about 10 MB. Full coverage would add tens of MB to the APK that BRL-0202 is trying to shrink.

# Decision (2026-11-14)

- Use the platform's own fonts as the fallback chain: add `bevy/system_font_discovery` to the `bevy-client` feature. Names keep Lilita One for every glyph it has. Any other glyph falls back per script to the OS font for that script, through Bevy/parley's built-in fallback. No font file is bundled (0 bytes of assets). The new compiled dependencies are small (fontique's CoreText/Android/DirectWrite/fontconfig backend crates).
- Why not a bundled font: covering CJK and emoji the same way would cost tens of MB of APK (against BRL-0202). A Latin/Greek/Cyrillic-only bundle would still box CJK and emoji, so it would need this mechanism anyway.
- Why not validation-time restriction: Latin Extended alone would have to be rejected (Polish, Czech, Hungarian, Vietnamese, Turkish names). That is a worse player outcome, and it is a server name-rule change the ticket does not need.
- No per-name font selection: parley already chooses a font per cluster, so a second name-specific mechanism would duplicate it.
- Scope of effect: the feature is process-wide on the client, so every text node gains OS fallback (Dashboard, roster, overhead plates, kill feed, scoreboard, results). No name call site needs a code change, because they already use the Lilita One handle. The server/headless graph has no `bevy_text` and is unchanged (`scripts/check-server-features.sh`).
- Known limitation: a character that no installed OS font covers (private-use or unassigned code points, or a rare script missing on a device) can still box. Validation-time rejection of those is filed as a follow-up, not done here.
- Test: a client unit test beside the theme font builds Bevy's default `FontCx` (so it fails if the feature is dropped), registers the shipped Lilita One bytes, lays out representative names (accented/extended Latin, Greek, Cyrillic, Vietnamese, CJK, Hangul, emoji) through parley exactly as `bevy_text` does, and asserts that no glyph resolves to `.notdef` (glyph id 0) and that ASCII runs keep Lilita One. It needs `parley` as a dev-dependency pinned to the locked `=0.9.0`, the same pattern as `skrifa`.
