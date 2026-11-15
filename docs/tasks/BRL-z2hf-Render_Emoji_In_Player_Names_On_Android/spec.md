Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec refreshed. Not fixed. A source check makes lead (a), COLRv1 not rasterized, the likelier cause. Lead (b) looks covered by parley/fontique.

# Found
BRL-5e2s device check 2026-11-15, Pixel 8a (Android 17), HEAD 6a6d6599. No text or font code has changed since then.

# Repro
An opponent proposed the display name `Жора 李雷 مرحبا 🎮` (valid under `normalize_proposed_display_name`). It was played in a 1v1 against the phone.
- The overhead plate, the kill feed and the Results roster on the phone show `Жора 李雷 مرحبا`. Cyrillic, CJK and Arabic render through system Noto fallback, as BRL-5e2s intended.
- 🎮 is absent. There is no box and no blank cell: the glyph produces nothing. macOS renders the same name with the emoji (Apple Color Emoji, sbix).

# Leads (checked against the pinned crates)
- Device font: `/system/fonts/NotoColorEmoji.ttf` is 2,992,136 bytes, which points to the COLRv1 build (the CBDT build is ~10 MB). `/system/etc/fonts.xml` lists it as `<family lang="und-Zsye">`.
- (b) Font selection, probably fine. parley 0.9.0 `src/shape/mod.rs` L557-567 queries `GenericFamily::Emoji` for every emoji cluster. fontique 0.9.0 `src/backend/android.rs` L37 maps `Emoji` → "Noto Color Emoji", and L132-137 also registers `und-Zsye` as a script fallback. Confirm on device with one log line naming the font parley picked for the 🎮 cluster.
- (a) Rasterization, likely the cause. bevy_text 0.19.1 `src/font_atlas.rs` L190-193 tries swash `Source::ColorOutline(0)` (COLRv0), `ColorBitmap(BestFit)` (CBDT/sbix) and `Outline`. swash 0.2.10 has no COLRv1 paint-graph support, and COLRv1 emoji glyphs usually have no plain outline, so the glyph rasterizes empty.
- Client code: system fallback comes from `bevy/system_font_discovery` (root `Cargo.toml` `bevy-client`). The fallback test `packages/brawler-client/src/ui/theme.rs` `names_outside_the_display_font_fall_back_to_real_glyphs` (L97) includes `Pew🎮🔥` but is `#[cfg(target_os = "macos")]`.

# Fix options (owner decision if cost > S)
1. Bundle a small CBDT or COLRv0 emoji font (for example a Noto Emoji COLRv0 or a subset). Register it as the `Emoji` generic family on Android only, through `FontCx` / the `TextFont` fallback path. Watch the BRL-5k0g APK size goal and record the added MB. The font licence must allow bundling (OFL is fine).
2. A monochrome fallback: register `NotoEmoji` (outline) so emoji show as glyphs rather than nothing.
3. A COLRv1-capable rasterizer (skrifa 0.44 has COLR paint traversal) in a Bevy text atlas hook. Largest option; upstream territory.

# Acceptance
- A host test: a layout over the chosen emoji font produces a non-empty rasterized glyph for 🎮 through the same swash sources Bevy uses (run on macOS/CI with the bundled font file).
- `cargo test -p brawler-client theme` passes.
- **Pixel 8a device check:** an emoji in a player name renders on the overhead plate, the kill feed and Results.

Evidence: /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/dc/r04.png (full frame), r04-plate-crop.png, r04-feed-crop.png, r05.jpg (Results).
