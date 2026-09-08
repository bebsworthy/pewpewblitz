# Player shell artwork

BRL-0114, 2026-09-07. Original project UI artwork derived from the user-approved Dashboard rev10 composition.

- `assets/brawler/ui/shell/logo.png`: isolated AI-generated refinement, OpenAI image generation, original retained at `/Users/boyd/.codex/generated_images/01a07a6d-94f7-7f21-a36f-405709f6b7cd/exec-23b41955-8aa8-4d74-a70d-71dcd7076f66.png`. Prompt: isolate the approved stacked PEWPEW BLITZ cream/orange/navy logo, teal/cream blaster and lightning, transparent background, no grunge or other UI. Original production branding files are preserved. Clean refinement replaces the earlier mottled export; generated original retained.
- `wipeout.svg`, `hot-zone.svg`, `heist.svg`, `zombie.svg`: editable original vector emblems on a consistent orange hanging pennant. Runtime PNGs exported at 300×372 with `export-badges.cjs` using Sharp. No external symbol pack used.
- Background: native WGSL geometric planes, no raster texture.
- Typography and models: existing shipped Lilita One and actual client models, with their existing manifest provenance.

These assets are project artwork, not third-party CC0 imports. Like the original project branding, their provenance is maintained here, outside the imported-asset manifest (whose contract only admits CC0/OFL third-party assets). Retained client handles still report runtime loading/failure for each asset. Approved full-screen PNGs under `inspiration/player-shell/` are design evidence, never runtime interface textures.

BRL-0114 visual correction: `generate-chrome.cjs` authors the six angled panel silhouettes, their shaped focus outlines, semantic icons, underline and refined Wipeout badge as SVG plus 2× PNG exports. Run with Node and Sharp (`NODE_PATH` may point to the bundled runtime). The SVGs contain only decoration, never interface text. PNGs are runtime exports; native Bevy labels, button semantics and flow ownership remain intact. `export-badges.cjs` can export the other existing mode badge sources.

BRL-0116 game selection: `generate-game-selection.cjs` exports the clean cream/teal/orange background, portrait card silhouettes with separate selected/focused states, semantic Hot Zone/Heist/Zombie badges. These are original code-authored SVG decorations with PNG runtime exports. Text, hit targets and choices remain native UI. The screen owns strong image handles while mounted; no generated full-screen mockup is used at runtime. Reuses the shipped logo, Wipeout badge, font and action panels. Run with Node and Sharp as above.

## Practice map overviews

Run `python3 asset_src/player-shell/generate-map-overviews.py`, then `node asset_src/player-shell/render-map-overviews.cjs` with Sharp available. The seven original schematic overviews encode actual authored map dimensions, placements, filled rectangles, rotated footprints and spawn points. They are overview diagrams, not gameplay screenshots. Regenerate when map geometry changes. SVG sources live in `map-overviews/`; runtime PNGs live in `assets/brawler/ui/shell/`.

Practice card frames: `node asset_src/player-shell/generate-practice-cards.cjs` (Sharp). Dedicated landscape proportions, orange choice fill, cream/navy focus frame and pointer; native text remains independent of the raster chrome.

Weapon part family illustrations are original SVG UI assets, exported by
`generate-parts.cjs` using Sharp. Magazine, ammunition, barrel, chamber and element
shapes follow the catalog's presentation families. They are symbolic illustrations;
the native weapon viewport uses the existing imported blaster and does not imply
that modifiers attach physical meshes.
