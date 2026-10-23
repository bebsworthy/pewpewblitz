# Outcome

Each map looks like one place built from one consistent set of pieces.

# Context

- `assets/catalogs/map_asset_visuals.ron` mixes Kenney mini-arena, mini-dungeon, graveyard, blaster-kit, mini-forest, and KayKit BlockBits pieces, each with its own palette, multiplied by per-asset tints.
- Captures show sci-fi crates beside yellow blocks beside cube "bushes", and pyramid-textured grass.
- `asset_src/blocks/brawler-blocks.blend` is the start of an owned kit.

# Scope

- For each current theme: walls, destructible cover, concealment foliage, water edge, barrels, chests, pickups, and decorations from one kit with a shared bevel size, top-face highlight, and baked AO.
- Foliage gets a chunky readable form with vertex sway.
- Remove per-asset tint multipliers.
- Keep collision footprints and map recipe IDs unchanged (presentation-only).
- Record provenance and licenses.

# Out of scope

Gameplay geometry changes and new map content.

# Acceptance criteria

- Every built-in map renders using only its theme's kit.
- Footprint/visual-bounds tests pass unchanged.
- Map Maker palette thumbnails are updated.
- Provenance is recorded in `assets/licenses/`.

# Verification

- Existing map presentation tests.
- Native captures of every built-in map.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
