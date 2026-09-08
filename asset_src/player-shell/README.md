# Player shell artwork

BRL-0114, 2026-09-07. Original project UI artwork derived from the user-approved Dashboard rev10 composition.

- `assets/brawler/ui/shell/logo.png`: isolated AI-generated refinement, OpenAI image generation, original retained at `/Users/boyd/.codex/generated_images/01a07a6d-94f7-7f21-a36f-405709f6b7cd/exec-23b41955-8aa8-4d74-a70d-71dcd7076f66.png`. Prompt: isolate the approved stacked PEWPEW BLITZ cream/orange/navy logo, teal/cream blaster and lightning, transparent background, no grunge or other UI. Original production branding files are preserved. Clean refinement replaces the earlier mottled export; generated original retained.
- `wipeout.svg`, `hot-zone.svg`, `heist.svg`, `zombie.svg`: editable original vector emblems on a consistent orange hanging pennant. Runtime PNGs exported at 300×372 with `export-badges.cjs` using Sharp. No external symbol pack used.
- Background: native WGSL geometric planes, no raster texture.
- Typography and models: existing shipped Lilita One and actual client models, with their existing manifest provenance.

These assets are project artwork, not third-party CC0 imports. Like the original project branding, their provenance is maintained here, outside the imported-asset manifest (whose contract only admits CC0/OFL third-party assets). Retained client handles still report runtime loading/failure for each asset. Approved full-screen PNGs under `inspiration/player-shell/` are design evidence, never runtime interface textures.

BRL-0114 visual correction: `generate-chrome.cjs` authors the six angled panel silhouettes, their shaped focus outlines, semantic icons, underline and refined Wipeout badge as SVG plus 2× PNG exports. Run with Node and Sharp (`NODE_PATH` may point to the bundled runtime). The SVGs contain only decoration, never interface text. PNGs are runtime exports; native Bevy labels, button semantics and flow ownership remain intact. `export-badges.cjs` can export the other existing mode badge sources.
