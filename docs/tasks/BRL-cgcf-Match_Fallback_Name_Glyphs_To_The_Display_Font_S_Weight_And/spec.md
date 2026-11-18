Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec updated — no runtime fallback configuration exists yet; added file refs and the shared fallback hook with BRL-z2hf.

# Context
After BRL-5e2s, characters outside Lilita One (`assets/brawler/ui/fonts/lilita-one-regular.ttf`, ASCII + Latin-1 only) fall back to the OS font fontique picks per script; the client configures nothing (`rg set_fallbacks` finds no runtime code; only the macOS test `names_outside_the_display_font_fall_back_to_real_glyphs` in `packages/brawler-client/src/ui/theme.rs:95-163` uses a local `bevy::text::FontCx`). Name text uses `ui::theme::hud_text` (`theme.rs:57-66`, regular `TextFont`) on the overhead (`presentation_3d/combat/fighter_ui.rs:348-385`) and Dashboard.

The 2026-11-14 macOS captures (evidence in the BRL-5e2s spec) show regular-weight fallbacks: Greek → Athelas (serif), CJK/kana → Heiti SC / Hiragino Sans W3 — readable but thin on grass/floor tiles. Android falls back to system Noto (regular) per BRL-z2hf's device check.

# Options
- A (preferred, zero APK cost): at startup, a system with `ResMut<bevy::text::FontCx>` calls fontique `collection.set_fallbacks(FallbackKey(script, locale), families)` (fontique 0.9, `collection/mod.rs:174`) per platform: macOS Grek/Cyrl/Latn → "Helvetica Neue" Bold or "SF Pro" heavy; Hani/Kana/Hira → "Hiragino Sans W6"; Hang → "Apple SD Gothic Neo" Bold. Android → "Noto Sans" / "Noto Sans CJK" at weight 700 if the family resolves by weight (verify; fontique fallback lists are families, weight comes from the query — may need `FontWeight` on a per-span basis, see next bullet).
- B: bundle a small OFL heavy subset (e.g. Noto Sans Black, Latin Ext + Greek + Cyrillic, ~0.3–0.6 MB) registered as a fallback family. Measure against BRL-5k0g's APK budget first.
- Do not set `FontWeight::BOLD` on the name `TextFont`: Lilita One has one weight and would be faux-bolded.

# Plan
1. Spike: log which family/weight parley resolves for Greek/CJK clusters on macOS and Pixel 8a (same logging BRL-z2hf needs for emoji).
2. Implement A in one `ui::fonts` startup system shared with BRL-z2hf's Android emoji fallback; fall back to B only if A cannot select a heavier weight.
3. Record APK/size impact in the ticket (0 for A).

# Acceptance
- macOS test (extend `theme.rs` test): Greek resolves to a sans family, and Greek/CJK fallback faces have weight >= 600.
- Native macOS overhead + Dashboard captures of accented Latin, Greek, Cyrillic and CJK names: fallback glyphs at visibly similar weight, no serif Greek.
- Pixel 8a device check: same names on overhead and Dashboard.
- `just lint`, `cargo test -p brawler-client --lib ui::theme`. Size impact recorded.

# Related / overlap
BRL-z2hf (emoji on Android) — both touch the same startup fallback configuration; implement together or z2hf first. BRL-5k0g (APK size) constrains option B.


# Implementation (2026-11-18)

## Research findings
- fontique 0.9 matches weight per family from the query attributes (`family.match_index(width, style, weight)`); fallback lists are families. Families come from the typographic family name (name ID 16), so per-weight family names such as "Hiragino Sans W6" do not exist as families on macOS (Hiragino Sans is one family with W0–W9).
- Probe (temporary test, removed) of what parley resolves for the display font plus system fallbacks on macOS, weight 400 → 700: Latin-Ext/Cyrillic Helvetica → Helvetica Bold; Greek Athelas Regular (serif) → Athelas Bold (still serif); Han Heiti SC Medium → Heiti SC Medium (family has no face above 400); kana Hiragino Sans W4 → W7; Hangul Apple SD Gothic Neo Regular → Bold.
- Lilita One at weight 700: fontique selects its only face and reports `synthesis.embolden = true`, but `bevy_text` 0.19.1 never reads parley's `Synthesis` (glyphs are rasterized with `normalized_coords` only; no embolden/skew anywhere in `bevy_text`), so Lilita glyphs and advances are unchanged. The spec's earlier "FontWeight::BOLD would faux-bold Lilita One" is not true for Bevy 0.19.1.

## Decision (option A, zero APK cost)
- New `packages/brawler-client/src/ui/fonts.rs` (the shared startup place BRL-z2hf can extend):
  - `DISPLAY_WEIGHT = FontWeight::BOLD`, requested by `ui::theme::font` and `ui::theme::hud_text` (all display text, not just names; Lilita renders identically, only fallback faces change).
  - `SCRIPT_FALLBACKS` + `apply_script_fallbacks(&mut Collection)` + `configure_fallback_fonts` Startup system installed by `ui::fonts::add_fallback_fonts` from `ClientPresentationPlugin`. macOS: Latn/Grek/Cyrl → Helvetica Neue; Hani → Hiragino Sans, Hiragino Sans GB, Heiti SC; Hira/Kana → Hiragino Sans; Hang → Apple SD Gothic Neo. Families missing on a system are skipped; a script with none keeps the platform default.
  - Android: no overrides. fontique's Android backend already uses `fonts.xml` script/locale fallbacks (sans Noto/Roboto); the bold query picks heavier faces where the device ships them (static Bold faces, or variable `wght` axes through parley's normalized coords).
- Overhead name fitting (`fighter_ui::measure_text_width`) now passes the label's `TextFont::weight`, so measured widths match the bold fallback layout.
- `bevy_text` clears fallbacks only when a `Font` asset is removed (`load_font_assets_into_font_collection`); the client never removes its font, so a Startup system is enough.
- Option B not needed; nothing bundled. Size impact: 0 bytes of assets/APK; a few hundred bytes of code.
- Durable doc: `docs/11-art-and-presentation-direction.md` (Product shell and HUD) records the fallback policy.

## Verification
- `cargo test -p brawler-client --lib ui::theme`: `names_outside_the_display_font_fall_back_to_heavy_sans_glyphs` (macOS) applies the production fallback config and the `hud_text` weight; asserts no `.notdef`, Latin stays Lilita One, Lilita One width is identical at NORMAL and DISPLAY_WEIGHT, every non-emoji fallback face has weight >= 600, and Greek resolves to Helvetica Neue (sans). Confirmed it fails without the fallback config ("Greek falls back to sans Helvetica Neue, not Athelas Bold").
- `cargo test -p brawler-client --lib fighter_ui`: 16 passed (fitting test now uses the production fallback config via a `world_with_display_font` helper).
- `just lint`: pass.
- `just test`: pass (2530 passed, 0 failed). `just lint` and the focused `ui::theme` + `fighter_ui` tests (17) re-run green after the final test cleanup.

## Native macOS captures (release automation client, 1280x720 @2x, own supervisor 127.0.0.1:5094, data under target/cgcf/)
Before = HEAD source, after = working tree; both builds carried two temporary capture hooks, now removed (`git diff` shows no change to `session/admission.rs` or `flow.rs`): an env-gated proposed display name in `session/admission.rs`, and a frame-driven flow driver (create brawler → brawler list → Dashboard, screenshot) like the BRL-y3hb scratch driver.
- Overhead, Wipeout 3v3 Verdant Crossfire, local `Łukasz Nguyễn Αλέ`, allies `Αλέ Жора 李さ김` (truncated `…`):
  - before `target/cgcf/before-match/captures/brawler-000750.png`, crop `target/cgcf/before-match/crop-000750.png`: Greek in serif Athelas, Cyrillic/CJK/Hangul regular-weight and thin next to Lilita.
  - after `target/cgcf/after-match/captures/brawler-000750.png`, crop `target/cgcf/after-match/crop-000750.png`: Greek/Cyrillic in Helvetica Neue Bold, 李 in Hiragino Sans Bold, さ W7, 김 Apple SD Gothic Neo Bold; weight visibly close to Lilita One; no serif.
- Dashboard identity plate `Łukasz Αλέξης Жора 李さ김`:
  - before `target/cgcf/before-dashboard/captures/dashboard.png`, crop `target/cgcf/before-dashboard/crop-identity.png` (serif thin Greek, thin Cyrillic/CJK).
  - after `target/cgcf/after-dashboard/captures/dashboard.png`, crop `target/cgcf/after-dashboard/crop-identity.png` (bold sans throughout). Lilita text elsewhere is unchanged.
- Judged by eye from the PNGs.

## Not verified / remaining
- Pixel 8a device check (acceptance) not done: no device use in this run. What to check: overhead and Dashboard with the same names; expected Roboto/Noto Sans Bold for Greek/Cyrillic/Latin-Ext and Noto Sans CJK at a heavier weight if the device's CJK font is variable or has a Bold face. If the CJK fallback stays regular on device (a regular-only `NotoSansCJK-Regular.ttc`), the remaining lever is option B (bundled heavy subset), which needs an owner decision on APK size (BRL-5k0g). Name entry on the phone still needs the automation display-name path (BRL-06pz).
- Emoji keeps its single-weight face (BRL-z2hf extends `ui::fonts`).

## Mistakes / lessons
- First capture batch: the dashboard branch of the scratch capture script hit an unbound array under `set -u`, skipped stopping its supervisor, and the next run's supervisor could not bind 127.0.0.1:5094, so that after-match run produced blank frames. Fixed the scratch script (sentinel array) and reran. Lesson: guard cleanup in capture scripts with a trap so a script error still stops the supervisor.
- `--auto-connect` without a smoke flag does not show the product shell; the Dashboard capture needs the interactive shell plus a driver that creates a brawler first.
