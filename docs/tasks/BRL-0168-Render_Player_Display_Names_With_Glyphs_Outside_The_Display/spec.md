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


# Implementation (2026-11-14)

- `Cargo.toml`: `bevy/system_font_discovery` is added to `bevy-client`, so the macOS client and the Android player (`brawler-android` uses `client`) both get it. The `server`/`bevy-server` graph is unchanged. `parley = "=0.9.0"` (std, no defaults) is a dev-dependency for the test. `Cargo.lock` gains `objc2-core-text`, `roxmltree` and `yeslogic-fontconfig-sys` (the fontique platform backends; only the target's own backend is compiled).
- `src/client/ui/theme.rs`: adds the test `names_outside_the_display_font_fall_back_to_real_glyphs` (macOS-gated, because it depends on installed OS fonts). No name call site changed: overhead plates, the Dashboard, the roster, the kill feed and the results screen already use the Lilita One handle, and parley falls back per cluster.
- `docs/11-art-and-presentation-direction.md`: the typography bullet records the fallback rule.
- No font file, `assets/manifest.ron` row, license file or `THIRD_PARTY.md` row is added, because nothing new is shipped. System fonts are used in place, not redistributed.

# Size impact

- Assets: 0 bytes.
- Code: measured with a scratch fontique 0.9.0 release binary. Stripped size with the `system` backend is 517,816 B, without it 394,344 B. That is about +121 KB on macOS. The Android backend (fonts.xml via roxmltree) is a comparable small code path; it was not built here. Against the 113.9 MB `libbrawler_android.so` (BRL-0202), the change is about 0.1%.

# Verification (2026-11-14)

- Focused test: `cargo test --locked --lib names_outside_the_display_font` passes. A negative check temporarily replaced the collection with `system_fonts: false`; the test then failed with "Łukasz Żółć rendered a missing-glyph box", which proves it detects the regression. The change was reverted.
- `just lint` (VS Code's ripgrep first on PATH): exit 0, including `check-server-features.sh` and `check-client-features.sh`.
- `just test`: exit 0. Totals include 960 lib (client), 583, 533, 112 network, and 85, with 0 failures.
- `just e2e 2`: exit 0 ("1 exact 1v1 roster(s) reached Active").
- Native macOS render evidence (release `client,automation`, 1280x720 at 2x Retina, routed supervisor). Names were set through `BRAWLER_CLIENT_DATA_DIR`/`connections.ron` `preferred_display_name` in the interactive product shell. The canonical `v3-render-evidence.sh` `--auto-connect` path always uses the generated `Brawler-XXXX` name, so it cannot show a custom name. That run is in `practice-cjk.txt.captures` and passed.
  - "李小雷 さくら 🎮🔥": overhead plate in Practice Wipeout 3v3, `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0168/shell-cjk/brawler-000600.png`, crop `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0168/overhead-cjk-crop.png`. Dashboard identity strip: `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0168/dashboard-cjk/brawler-000270.png`. CJK (Heiti SC), kana (Hiragino Sans) and color emoji (Apple Color Emoji) all render. No boxes.
  - "Łukasz Жора Αλέξης": overhead plate `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0168/shell-cyr/brawler-000600.png`. Dashboard `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0168/dashboard-cyr/brawler-000150.png`, crop `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0168/dashboard-cyr-crop.png`. "ukasz" stays Lilita One, Ł and Cyrillic use Helvetica, Greek uses Athelas. No boxes.
  - Both render reports passed their locked thresholds: `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0168/shell-cjk.txt` and `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0168/shell-cyr.txt` show `result=pass`.
- Observations: fallback glyphs are regular weight and look thinner than Lilita One on plates. Greek falls back to a serif (BRL-0215). The wide Cyrillic/Greek name runs past the 120 px plate (BRL-0216). Japanese text logs a harmless `ICU4X data error: No segmentation model for language: ja` from parley's line breaker. Glyphs still render; parley simply has no Japanese word-break model.
- Not verified: Android. No device was attached (`adb devices` was empty).

# Playtest request

- macOS: `just run`, then Menu → change the display name to something like `Łukasz Жора 李雷 🎮` and reconnect. Check the Dashboard identity strip. Start Practice → Verdant Crossfire Wipeout 3v3 and check your overhead plate, the kill feed, the scoreboard and the results screen. Expected: no boxes anywhere. Fallback glyphs are thinner than Lilita One; judge whether that is acceptable for now (BRL-0215).
- Android (Pixel 8a): `just android-install`, set the same name, and check the Dashboard and the overhead plate. Also note whether cold start feels slower. fontique scans `/system/fonts` once at startup; that took 255 ms cold and 21 ms warm on macOS, and Android is unmeasured. This is the remaining required evidence and keeps the ticket in `doing`.

# Deferred

- BRL-0214: reject private-use, noncharacter and unassigned code points at name validation (a server rule change; needs owner confirmation).
- BRL-0215: heavier, sans-serif fallback for names (explicit fallback families or a small OFL subset).
- BRL-0216: fit long or full-width names to the overhead plate width used by overlap resolution.

# Learn from errors

- Assumption corrected by measurement: the ticket context said Lilita One "covers Latin only". Its cmap check showed only ASCII + Latin-1, so Polish, Czech, Turkish and Vietnamese names also boxed. Lesson: measure a font's cmap before scoping a glyph fix.
- The prompt's hints pointed at cosmic-text and a bundled font. Bevy 0.19.1 actually uses parley/fontique, which already does per-script fallback and only lacked a font source. Lesson: read the exact locked dependency source before designing; the cheapest fix was a Bevy feature flag.
- Evidence tooling gap: `--auto-connect` evidence runs cannot carry a custom display name. I used the interactive shell with a seeded `connections.ron` instead. If name rendering needs repeat evidence, add a display-name override to the automation path (not done; no second use yet).
- The evidence runs' default supervisor wrote to the tracked `data/server/profiles.sqlite3`. I restored it with `git checkout`. Prevention: give ad-hoc supervisors a scratch data path, or restore that file after native runs.
