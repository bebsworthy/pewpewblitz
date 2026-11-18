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


# Result (2026-11-18)
Re-verified references at HEAD c1626efc (rg of each basename over packages/, src/, tests/, assets/catalogs/, scripts/): unchanged from Context.

- Deleted 19 PNGs and their 19 SVGs: `panel-{mode,play,practice,build,menu}{,-focus}`, `panel-identity-focus`, `game-card{,-selected}{,-focus}`, `practice-card{,-selected}{,-focus}`. Also deleted all four Sharp exporters: `generate-chrome.cjs`, `generate-game-selection.cjs`, `generate-practice-cards.cjs`, `render-map-overviews.cjs`.
- Added `asset_src/player-shell/build_shell_art.py`. It uses the stdlib only, with an anti-aliased SDF and exact-area rasterizer (`Canvas`). It writes the SVG and PNG for `panel-identity`, `underline` and `game-selection-background`, and the regenerated SVGs are byte-identical to the old ones. `generate-map-overviews.py` now rasterizes the overview PNGs through `Canvas`, fit inside 1200×800 with per-axis scale; its SVGs are also byte-identical. Sharp is no longer needed.
- Pixel diff of each rebuilt PNG against the shipped one, per channel with RGB premultiplied: panel-identity max 28, mean 0.18, 0.18% px >8, 0 px >32. underline max 42, 0.23% >8, 1 px >32. game-selection-background max 2. feature-yard-* overviews max 18, 0.009% >8. The 541×800 and 1067×800 overviews have max 58, about 1% >8 at cell edges. Their >32 pixels are exactly the two edge columns, which Sharp's fit-inside export left semi-transparent (alpha 197); the rebuild is opaque there. Evidence: `target/4jdx/asset-pixel-diff.txt`, `target/4jdx/diff-*.png`, originals in `target/4jdx/shipped/`.
- Updated the README (Shell chrome and Practice map overviews sections) and `assets/licenses/pewpew-blitz-original-shell-art.txt` so they list only the surviving files and generators.
- Added the guard test `ui::icons::tests::shell_directory_holds_only_loaded_artwork`. It requires every shell PNG to be either a literal `"brawler/ui/shell/<file>"` in client source or `<key>-overview.png` for a map named in `config/server/game-types.ron`, and every such map to have an overview. Adding a stray PNG makes it fail ("no screen loads brawler/ui/shell/stray-test.png").

Verification:
- `just check`, `just lint` (after `cargo fmt`) and `just test` all pass; logs are in `target/4jdx/{check,lint,test}.log`. The credits tests pass.
- I ran `brawler-ui-gallery` (the `just ui-gallery` binary) for 20 s and it showed no errors (`target/4jdx/ui-gallery.log`).
- I compared `just shell-capture` before (`target/4jdx/before/`) and after (`target/4jdx/after/`) for all 19 routes at both sizes; numbers are in `target/4jdx/capture-pixel-diff.txt`. At 1280x720 and 914x411 respectively: game-select ≤0.003% px >8, practice-maps 0.12% and 0.05% >8 with max 33 and 32, dashboard 0.63% and 0.55% >8. Run-to-run noise between two after-runs is larger (dashboard 3.7%, credits 3.7%; `target/4jdx/capture-noise.txt`). It comes from the animated fighter, Play pulse and drifting decorations, plus the random brawler name. A crop of the identity plate (`target/4jdx/capture-diff/dashboard-topbar-before-after.png`) looks identical apart from the name.
- Not done: device or APK size check (optional). The shell PNGs went from about 600 KB to about 135 KB.
