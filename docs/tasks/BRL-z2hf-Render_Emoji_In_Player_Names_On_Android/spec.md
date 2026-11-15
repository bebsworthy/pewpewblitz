# Found
BRL-5e2s device check 2026-11-15, Pixel 8a (Android 17), HEAD 6a6d6599.

# Repro
An opponent proposed the display name `Жора 李雷 مرحبا 🎮` (valid under `normalize_proposed_display_name`). It was played in a 1v1 against the phone.
- The overhead plate, the kill feed and the Results roster on the phone show `Жора 李雷 مرحبا`. Cyrillic, CJK and Arabic render through system Noto fallback, as BRL-5e2s intended.
- 🎮 is absent. There is no box and no blank cell: the glyph produces nothing. macOS renders the same name with the emoji (Apple Color Emoji, sbix).

# Leads
- The device's `/system/fonts/NotoColorEmoji.ttf` is 2,992,136 bytes. That size points to the COLRv1 build (the CBDT bitmap build is about 10 MB). `/system/etc/fonts.xml` lists it as `<family lang="und-Zsye">`. Possible causes:
  (a) Bevy 0.19's `font_atlas.rs` rasterizes swash `ColorOutline` (COLRv0) and `ColorBitmap`, but not COLRv1 paint graphs, so the glyph rasterizes empty.
  (b) fontique's Android backend does not map `und-Zsye` to the Emoji generic family.
  Check (b) first: log the font that parley picks for the emoji cluster on device.
- Fixes to weigh: a COLRv1-capable rasterizer path, bundling a small CBDT/COLRv0 emoji subset, or an explicit Android fallback to `NotoColorEmoji`. Respect BRL-5k0g's APK size goal.

# Acceptance
- On the Pixel 8a an emoji in a player name renders on the overhead plate, the kill feed and Results.

Evidence: /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/dc/r04.png (full frame), r04-plate-crop.png, r04-feed-crop.png, r05.jpg (Results).
