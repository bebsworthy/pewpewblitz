# Outcome

Mode pennant badges share the BRL-0157 construction: ink outline and offset lip, two tones plus a highlight, BRL-0140 palette.

# Context

BRL-0157 replaced every shell, part-picker, ability and Map Maker icon with the generated set in `asset_src/player-shell/icons/`. The pennant badges (`assets/brawler/ui/shell/{wipeout,hot-zone,heist,zombie}.png` and `game-badge-*.png`, sources `generate-chrome.cjs`, `generate-game-selection.cjs`, `export-badges.cjs`) were kept out of scope as mode emblems; they still use the earlier cream/orange/teal flat style and need Sharp to re-export.

# Scope

- Add the pennants and their emblems to `build_icons.py` (or a sibling using the same renderer), keeping the hanging-pennant silhouette that distinguishes modes.
- Retire the Sharp badge exports they replace.

# Acceptance criteria

- All mode badges come from the generated set with SVG sources and README provenance.
- Owner visual acceptance on Dashboard, game selection and match loading.

# Verification

- `just ui-gallery` capture, native Dashboard capture, focused asset tests.
