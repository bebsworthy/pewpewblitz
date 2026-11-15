Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec updated — the two Credits entries still point at `asset_src/` (credits.rs:57,74); no notes exist yet. Added the branding PNGs and the logo's provenance facts.

# Context
Found by BRL-wpr8 (d73d51fb). `ORIGINAL` in `packages/brawler-client/src/shell/credits.rs:56-80` lists original PewPew Blitz work; five entries point at `assets/licenses/pewpew-blitz-original-{weapons,environment,decals,icons,audio}.txt`, two still at `asset_src/` directories:
- `("Riff fighter", "asset_src/characters/riff")` (`:57`) — ships `assets/brawler/models/original/characters/riff/chr_riff.glb`; provenance in `asset_src/characters/riff/README.md` (procedural Blender builder `build_riff.py`, frozen `versions/v1`,`v2`).
- `("branding and menu art", "asset_src/player-shell")` (`:74`) — ships `assets/brawler/ui/branding/pewpew-blitz-{lockup,wordmark}.png` (loaded at `assets.rs:26,64`) and `assets/brawler/ui/shell/*.png`; provenance in `asset_src/player-shell/README.md` (code-authored SVG exporters, `generate-map-overviews.py`; `logo.png` is a recorded AI-assisted refinement of the original branding). The branding PNGs have no source/builder recorded anywhere — the note should say so factually ("original project branding, source files not in the repository").

Tests: `every_license_file_is_credited` (`credits.rs:164-189`) requires every `assets/licenses/*` file to be credited and every `ORIGINAL` path to exist; `credits_text_names_only_shipped_work` (`:191`) checks labels appear in the text.

# Plan
1. Add `assets/licenses/pewpew-blitz-original-characters.txt` (Files: `brawler/models/original/characters/*/*.glb`; builder `asset_src/characters/riff/build_riff.py`, Blender, BRL-0154/BRL-q29d) and `assets/licenses/pewpew-blitz-original-shell-art.txt` (Files: `brawler/ui/branding/*.png`, `brawler/ui/shell/*.png`; per-file provenance as in the README), in the same format as `pewpew-blitz-original-icons.txt`.
2. Point the two `ORIGINAL` entries at the new notes; update the doc comment at `credits.rs:53-55` (no more `asset_src/` fallback) and optionally assert every `ORIGINAL` path is under `assets/licenses/`.
3. If BRL-4jdx lands first, list only the files left after it.

# Acceptance
- Every shipped original asset family has a note; the `ORIGINAL` table has no `asset_src/` path.
- `cargo test -p brawler-client --lib shell::credits`, `just lint`.
- No device check needed (Credits text change only; optional Credits screen glance on Pixel 8a).
