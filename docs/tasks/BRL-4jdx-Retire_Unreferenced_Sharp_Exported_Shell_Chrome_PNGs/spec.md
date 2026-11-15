Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec updated — re-verified references; panel-identity-focus.png is also unused, and render-map-overviews.cjs is a fourth Sharp exporter.

# Outcome
`assets/brawler/ui/shell/` holds only artwork a screen loads; the remaining shell exporters no longer need Sharp.

# Context
Found during BRL-1wms. Re-checked 2026-11-15 (`rg` of each basename over `packages/`, `src/`, `tests/`, `assets/catalogs/`): runtime loads only
- `logo.png`, `panel-identity.png`, `underline.png` (`packages/brawler-client/src/assets.rs:65,68,79`),
- `game-selection-background.png` (12 screens, e.g. `flow/screens/game_select.rs:228`),
- the seven `*-overview.png` via `format!("brawler/ui/shell/{}-overview.png", p.key)` (`flow/screens/practice_maps.rs:360`).

Unreferenced (19 files): `panel-{mode,play,practice,build,menu}{,-focus}.png`, `panel-identity-focus.png`, `game-card{,-selected}{,-focus}.png`, `practice-card{,-selected}{,-focus}.png`, plus their SVGs in `asset_src/player-shell/`.

Sharp exporters (`require('sharp')`): `generate-chrome.cjs` (panels + underline), `generate-game-selection.cjs` (background + game cards), `generate-practice-cards.cjs`, and `render-map-overviews.cjs` (overview SVG → PNG; SVGs come from `generate-map-overviews.py`). Sharp is not installed; `game-selection-background.png` was already re-rasterized by a pure-Python writer (README, BRL-0156 note).

# Plan
1. Delete the 19 PNGs, their SVGs, `generate-practice-cards.cjs`, the panel/card sections of `generate-chrome.cjs` and `generate-game-selection.cjs`.
2. Replace the survivors' exporters with one stdlib-Python builder (`asset_src/player-shell/build_shell_art.py`, like `icons/build_icons.py`) for `panel-identity`, `underline`, `game-selection-background`; extend `generate-map-overviews.py` to write PNGs directly, or document that `render-map-overviews.cjs` keeps Sharp. Rebuilt PNGs must match the shipped ones visually (pixel diff small).
3. Update `asset_src/player-shell/README.md` (BRL-0114/0116/practice-card paragraphs).
4. Guard test next to the retired-icon test in `packages/brawler-client/src/ui/icons.rs:~165-193`: every PNG in `assets/brawler/ui/shell/` is either a literal path in client source or `<practice key>-overview.png`.

# Acceptance
- No unreferenced PNG remains in `assets/brawler/ui/shell/`; the guard test fails if one is added.
- Credits drift tests pass (`cargo test -p brawler-client --lib shell::credits`).
- `just check`, `just test`, `just lint`, `just ui-gallery`, and a native Dashboard + Practice capture showing no visual change.
- No device check needed (asset removal only; optionally confirm APK shrinks).

# Related
BRL-ztpa's shell-art license note should list the files left after this ticket; do this first or update the note after.
