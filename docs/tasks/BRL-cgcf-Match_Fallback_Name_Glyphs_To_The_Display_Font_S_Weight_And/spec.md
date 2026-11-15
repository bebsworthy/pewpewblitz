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
