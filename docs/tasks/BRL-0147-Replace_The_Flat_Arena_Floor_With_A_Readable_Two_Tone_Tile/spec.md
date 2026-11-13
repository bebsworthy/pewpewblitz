# Outcome

The floor gives the arena structure and scale cues while staying quiet behind fighters, hazards, and objectives.

# Context

`assets/catalogs/map_presentation_themes.ron` gives each theme a flat ground color plus organic blobs. The floor fills most of each frame (about 70% of the August capture; slate blue in the 2026-10-23 capture).

# Scope

- A ground material/shader with two close tones alternating per authoritative grid cell, subtle per-cell value jitter, and darkening near walls/edges.
- Per-theme parameters in the presentation theme catalog.
- Optional sparse decals near walls.
- Effect tiles, Hot Zone regions, concealment fields, and objective markers must stay readable on top.

# Out of scope

Environment model kits (separate ticket) and gameplay or map recipe changes.

# Acceptance criteria

- Tiles align exactly to authoritative cells on every built-in map.
- A readability review passes for effect tiles, zones, and concealment boundaries on every theme.
- The floor is configured per theme in the catalog, with no hard-coded colors.

# Verification

- A focused alignment test, if feasible.
- Native captures per map.
- `just test`.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.


# Research and design (2026-11-13)

- Today: `map::spawn_ground_surfaces` draws one flat matte `Cuboid` (`playable_ground`) per map
  plus 18 organic `ground_accent` blobs placed by fractions of the bounds (not grid-aligned).
  The floor top sits at y = 0.25; everything else (effect tiles, water, Hot Zone ring,
  concealment fields, rings) is already layered above that height, so layering is unchanged.
- Authoritative grid: `MapDimensions::bounds()` is centred on the origin, `cell_min(cell) =
  bounds.min + cell * MAP_CELL_SIZE_WORLD` (32). Built-in maps are at most 64 x 40 cells; config
  allows up to 512 x 512 for authored maps.
- Decision: no custom shader. The floor becomes one generated mesh with one quad per authoritative
  cell, built by a pure function from `MapDimensions`, and drawn with a white matte
  `StandardMaterial` that multiplies `ATTRIBUTE_COLOR` (linear vertex colours). Per-cell tone is
  constant over the quad; edge darkening is per shared cell corner, so it reads as a smooth
  gradient across tile boundaries. This keeps `BRAWLER_FORCE_PRIMITIVE_WORLD`, the HDR/bloom
  pipeline, shadows, and the generated-mesh lifecycle (`GeneratedMapMesh`) unchanged and makes
  alignment directly testable. Cost: 4 vertices per cell (10 240 for the largest built-in map;
  u32 indices only above 16 383 cells).
- Tone: checkerboard `(x + y) % 2` picks `tone_a`/`tone_b`; a deterministic per-cell hash adds a
  value jitter of +/- `value_jitter`.
- Edge darkening: occluders are out-of-bounds cells (arena edge) and static (indestructible)
  Feature placements that block players (walls). Destructible cover/barrels are excluded because
  the mesh is built once per map and would leave darkening after destruction; vegetation and
  surfaces (water, effect tiles) are excluded. A corner touching an occluder gets full darkening,
  the next corner ring half, then none (code-owned 2-corner falloff); strength is per theme.
- Catalog: `assets/catalogs/map_presentation_themes.ron` schema 4 -> 5 replaces
  `playable_ground` with `floor: (tone_a, tone_b, value_jitter, edge_darkening)`. Validation
  enforces the art direction: tones differ by at most 8% HSV value, `value_jitter` in [0, 0.04],
  `edge_darkening` in [0, 0.4]. `ground_accent` stays (water shore edges still use it).
- The organic accent blobs are removed: they are not grid-aligned and fight the tile pattern.
- Retune the three themes' floor tones lighter and less saturated than the props (the art
  direction), in particular theme 3 whose slate floor graded to a dark navy after BRL-0146.
- Optional sparse wall decals: not done here (environment dressing belongs with the per-theme kit,
  BRL-0153); a backlog ticket records it.
- Tests: pure alignment test over every built-in map (each quad's corners equal the authoritative
  cell bounds; checkerboard tone assignment; occluder darkening), catalog validation (schema,
  8% value rule, ranges), and the BRL-0146 bloom-knee test extended to the floor tones.


# Implementation (2026-11-13)

- New `src/client/presentation_3d/floor_tiles.rs` (+ `floor_tiles/tests.rs`): `FloorTiles` catalog
  type with validation (8% value rule, jitter <= 0.04, edge darkening <= 0.4),
  `floor_occluder_cells` (static player-blocking Feature placements), `floor_tile_mesh` (one quad per
  cell, linear vertex colours, u16 indices up to 16 383 cells, u32 above).
- `map.rs`: `spawn_ground_surfaces` takes the snapshot, shared map catalog, and theme floor; the
  flat floor cuboid and the 18 organic accent patches (and their mesh/tests) are removed. The tile
  surface sits at the old floor top (y = 0.25).
- `environment_assets`: theme catalog schema 4 -> 5, `playable_ground` replaced by `floor`; the
  theme floor material is white matte (tones come from vertex colours). `ground_accent` is kept for
  water shore edges.
- `assets/catalogs/map_presentation_themes.ron` final floors (tone_a / tone_b / jitter / edge):
  theme 1 (0.52,0.38,0.24) / (0.54,0.40,0.25) / 0.025 / 0.22; theme 2 (0.72,0.57,0.38) /
  (0.745,0.595,0.40) / 0.02 / 0.2; theme 3 (0.44,0.43,0.41) / (0.46,0.45,0.43) / 0.025 / 0.3.
  A first pass with ~5-6% tone difference read as a busy checkerboard after grading contrast;
  reduced to ~3.5-4.5%. Theme 3 moved from dark slate (graded to navy) to a lighter warm grey.
- `world_look/tests.rs`: the bloom-knee test also checks every theme's brightest jittered floor tone.
- `docs/11-art-and-presentation-direction.md`: floor paragraph; adoption table marks BRL-0147
  adopted.

# Verification (2026-11-13)

- Focused: `cargo test --lib -- floor_tiles world_look environment_assets` pass (alignment over every
  built-in map, upward winding, checkerboard tones, deterministic bounded jitter, wall/edge
  darkening, occluders exclude destructible/pass-through placements, catalog validation, bloom knee).
- `just lint` (real ripgrep first on PATH) pass (fixed: test-only helper `#[cfg(test)]`, one
  `checked_conversions`).
- `just test` pass: 2138 passed, 0 failed (incl. performance gates).
- `just e2e 2` pass (1v1 roster reached Active).
- Render evidence (`just v3-render-evidence`, `BRAWLER_RENDER_COMBAT=1`; frames 300/480/660),
  p50/p95/p99 ms, all `result=pass`, no client panics:
  - theme 1 Feature Yard 1v1: before 16.655/18.271/19.219 -> after 16.656/17.444/19.116 (60 Hz vsync)
  - theme 2 Powderline Vault Heist Practice 3v3 (`BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_MODE=heist
    BRAWLER_RENDER_PLAYERS_PER_TEAM=3 BRAWLER_RENDER_GAME_TYPE=heist-3v3`): before 8.331/8.535/8.664
    -> after 8.333/8.546/8.638
  - theme 3 Verdant Crossfire Practice 3v3 (`BRAWLER_RENDER_GAME_TYPE=wipeout-3v3`): before
    8.356/16.734/17.743 -> after 8.333/8.559/8.666
  - theme 3 Switchback Basin Hot Zone Practice 3v3 after 8.332/8.557/8.667
  - theme 1 primitive world (`BRAWLER_FORCE_PRIMITIVE_WORLD=1`) after 16.666/17.127/17.799,
    `fallback=primitive`
- Evidence directories (PNG + JSON): before
  /Users/boyd/wip/brawler/target/brl0147/before-theme{1-combat,2-practice,3-practice}.txt.captures;
  after /Users/boyd/wip/brawler/target/brl0147/after-theme{1-combat,2-practice,3-practice,3-hotzone,1-primitive}.txt.captures;
  first tuning pass /Users/boyd/wip/brawler/target/brl0147/a1-theme{1,2,3}-*.txt.captures.
- Readability review of the captures: tile seams coincide with effect tiles, water, cover, and wall
  blocks in all themes; red damage / blue speed / pale slow effect tiles, water, local green, ally
  blue rings, projectiles, and aim lanes stay clearly separated from the floor; concealment bushes
  (dark teal / green) read against the floor; walls and the arena edge show a soft darkening. The
  floor is now lighter and less saturated than the props on every theme. Limitations: no enemy red
  ring and no Hot Zone circle were in frame in the automated captures (Switchback's zone is outside
  the spawn view), so those two need the playtest.

# Playtest request

`just run 2`, then Practice: Feature Yard (theme 1), Powderline Vault Heist 3v3 (theme 2), Verdant
Crossfire and Switchback Basin Hot Zone 3v3 (theme 3). Look at: whether the checkerboard reads as
quiet structure rather than busy pattern; that tile seams line up with effect tiles, water, and
walls; enemy red rings, ally blue, local green, projectiles, effect tiles, the Hot Zone circle, and
bush concealment against the floor; the darkening along walls and the arena edge (too strong or too
subtle?); and the overall theme 3 colour (no longer navy). Also open Map Maker and edit a map to see
the floor rebuild under the editor preview.

# Learn from errors

- The first tone pair (~5-6% value difference) looked fine as numbers but the graded output
  (contrast 1.05, saturation 1.15) made the checkerboard busy; judge floor contrast on graded native
  captures, not catalog values.
- A first darkening test compared corners of different tone parity and corners still inside the
  falloff ring; tests of shading must isolate one factor (same parity, clearly open reference cell).
- Baseline captures were taken before any source edit, as BRL-0146 learned (the evidence script
  rebuilds from the working tree).

# Deferred

- BRL-0178 (todo): sparse wall-adjacent floor decals per theme (optional scope here; belongs with
  the per-theme kit, BRL-0153).

# Playtest feedback (2026-11-13)
- User ran native sessions (`just run 2`, `just run 1`) with several 3v3 Practice matches and accepted the result ("seems fine"). No corrections requested; the open tuning choices in this spec are accepted as implemented. No induced-latency run was reported separately.
- Observed in the session logs, not user-visible: recurring `bevy_render::slab_allocator: Use-after-free` errors at client start and on match exit; tracked separately (see BRL-0171 children). Disposition: accepted; no open feedback items.
