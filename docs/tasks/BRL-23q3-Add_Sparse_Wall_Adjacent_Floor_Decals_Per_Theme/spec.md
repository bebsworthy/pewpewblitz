# Outcome

Each theme's tile floor gets a small number of decals near walls that add place character while
the floor stays quiet behind fighters, effect tiles, zones, and concealment.

# Context

BRL-005q replaced the flat floor and organic blobs with a grid-aligned two-tone tile surface
(`src/client/presentation_3d/floor_tiles.rs`, per-theme `floor` in
`assets/catalogs/map_presentation_themes.ron`). Its spec listed "optional sparse decals near walls";
they were deferred because dressing should match the per-theme environment kit (BRL-k4n1).

# Scope

- Per-theme decal set and density in the client theme catalog (no hard-coded colors).
- Deterministic placement on cells adjacent to static walls, never on surfaces, effect tiles,
  objective areas, or spawn cells.
- Presentation-only; no gameplay content or fingerprint change.

# Acceptance criteria

- Decals are deterministic per map and never overlap effect tiles, water, Hot Zone, or safe areas.
- A readability review passes on every theme.

# Verification

- Focused placement test; native captures per theme; `just test`.



# Research and decisions (2026-11-14)

- Inputs: BRL-005q (one generated floor mesh at y = 0.25, static player-blocking Feature placements
  are the wall occluders, destructible cover is excluded), BRL-k4n1 (per-theme kits, client-only
  theme catalog), BRL-farz (non-glow layers must stay under the bloom knee; lit albedo <= 1 is
  already safe because the theme's lit white is under the knee), BRL-ynt5 (tiers in
  `render_quality.ron`).
- Decision: decals are one generated mesh per map (one quad per decal, `u16` indices) drawn with one
  lit, alpha-blended toon `StandardMaterial` over one generated atlas. Tint and opacity are linear
  vertex colours, so the three themes share one material and one texture: one draw call per map.
  No Bevy `ForwardDecal` (needs a depth prepass) and no custom shader.
- Decision: placement is a pure function `place_floor_decals(snapshot, catalog, occluders, decals)`
  seeded by `MapRecipeId` and the cell (splitmix64), so a map always gets the same decals in every
  match instance and in the Map Maker preview. Candidates are in-bounds cells that are not
  occluders and have a 4-neighbour that is a static wall or the arena edge. Excluded: every
  placement footprint of any slot (effect tiles, water, cover, foliage, spawns, markers), the Hot
  Zone circle plus half a cell, Heist safes plus their two-cell attack sectors, and Zombie ingresses
  plus two cells. Accepted decals keep Chebyshev distance >= 2 (never touch). Code-owned cap 160 per
  map (bounded state). Each decal is a square inside its own cell (0.78-0.98 of the cell, slid
  along the wall), its sprite top edge turned toward the wall, optionally mirrored, so it can never
  overlap a neighbouring surface.
- Decision: catalog `assets/catalogs/map_presentation_themes.ron` schema 6 -> 7 adds per-theme
  `floor_decals: (density, opacity, kinds: [(sprite, tint, weight)])`. Validation: density <= 0.4,
  opacity <= 0.6, 1-6 kinds, unique sprites, weight 1-10, sRGB tints with HSV value at most 1.2x
  the theme's brightest floor tone (a drift reads as sand, not a light spot).
- Decision (bloom): the material is lit (never unlit), has black emissive, and tints are <= 1, so
  decals never exceed the lit floor; the BRL-m10f knee test now also checks every decal tint.
- Decision (layering): 0.15 above the floor (under contact shadows at 0.6 and overlays at 1.0) and
  `depth_bias = -16`, so the decal layer sorts before every other transparent layer (they use 0 or
  higher); fields, contact shadows, rings, and zones always draw over decals. `NotShadowCaster`.
- Decision (tiers): `render_quality.ron` schema 1 -> 2 adds `floor_decals`; High and Medium keep
  them, Low drops them (a system toggles the layer's `Visibility`, so a live tier change applies
  without rebuilding the map).
- Decision (art): `asset_src/environment/floor-decals/build_decals.py` (Python 3 standard library,
  deterministic, byte-identical rebuilds) writes a 384x256 RGBA atlas, 3x2 cells of 128 px: crack,
  scuff, leaves, sand-drift, stain, pebbles. Near-white texels, alpha carries the shape, white
  transparent texels, transparent border. Loaded with a linear sampler (the app default is
  nearest). No mipmaps; at today's framing a cell spans about 60-140 physical pixels, so the
  sprite is magnified or barely minified (mipmaps deferred to BRL-dq06).
- Theme sets (final): theme 1 Crossroads Facility: crack, scuff, stain (dark umber), density 0.3,
  opacity 0.6. Theme 2 Tidal Garden: sand drift (light sand), olive leaves, crack, pebbles, density
  0.3, opacity 0.6. Theme 3 Ashen Court: charcoal crack, dry brown leaves, scuff, pebbles, density
  0.3, opacity 0.6.

# Implementation (2026-11-14)

- New `packages/brawler-client/src/presentation_3d/floor_decals.rs` (+ `floor_decals/tests.rs`):
  `FloorDecals` catalog type and validation, `excluded_decal_cells`, `place_floor_decals`,
  `decal_corners`, `floor_decal_mesh`, `floor_decal_material`, `spawn_floor_decals`,
  `FloorDecalLayer` and `apply_floor_decal_quality` (in `WorldPresentationSet::Animate`).
- `presentation_3d/mod.rs`: module, quality system, and the map materialization spawns decals right
  after the ground surfaces (same `GeneratedMapMesh` lifecycle, despawned with the map generation).
  `map.rs`: `PLAYABLE_FLOOR_Y` is `pub(super)`.
- `environment_assets`: theme schema 7 with `floor_decals` (validated against the theme floor);
  one shared `floor_decals` toon material in `EnvironmentThemeMaterials`; the atlas is loaded with a
  linear sampler in `load_environment_assets`.
- `render_quality.rs` / `render_quality.ron`: schema 2, `floor_decals` knob (Low off), logged.
- `world_look/tests.rs`: decal tints checked against the bloom knee.
- Art and provenance: `asset_src/environment/floor-decals/{build_decals.py,README.md,previews/decal-sheet.png}`,
  `assets/brawler/textures/original/environment/floor-decals.png`,
  `assets/licenses/pewpew-blitz-original-decals.txt` (original, script-generated).
- Docs: `docs/11-art-and-presentation-direction.md` (decal paragraph, quality tiers, adoption table).
- Unchanged: gameplay content, `content/`, the content fingerprint, protocol, server, map recipes.

# Verification (2026-11-14, reference Mac, scratch CARGO_TARGET_DIR)

- Focused: `cargo test -p brawler-client --lib -- floor_decals floor_tiles world_look render_quality
  environment_assets`: 53 passed (determinism per map, wall adjacency, exclusions of placements,
  Hot Zone, Heist safes and sectors, spawns, spacing, cap, corners inside the cell, seed dependence
  and instance independence, upward winding and atlas UVs, catalog validation, atlas layout, lit
  non-emissive blended material with negative depth bias, Low tier hides the layer, bloom knee).
  Every built-in map gets 23-69 decals.
- `just check`: pass. `just lint` (VS Code ripgrep first on PATH): pass after two Clippy fixes
  (`similar_names`, and `too_many_arguments` on `spawn_ground_surfaces`, fixed by moving decal
  spawning into its own `spawn_floor_decals`). `just test`: pass, 2357 passed, 0 failed (including the performance gates).
- Render evidence: patched scratch copy of `v3-render-evidence.sh`
  (/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0178-render-evidence.sh:
  scratch-target binaries, supervisor `--data-directory` under the scratchpad, bind 127.0.0.1:5188).
  The user's server on UDP 5000 was not touched and `data/server/` was not written. Practice runs,
  frames 300/480/660, all `result=pass`, no client panics. Client p50/p95/p99 ms (120 Hz):
  - theme 1 Feature Yard Wipeout 1v1: before 8.336/8.568/8.712, after 8.334/8.557/8.665
  - theme 2 Powderline Vault Heist 3v3: before 8.332/8.571/8.689, after 8.331/8.570/8.681
  - theme 3 Verdant Crossfire Wipeout 3v3: before 8.332/8.561/8.662, after 8.335/8.554/8.664
  - theme 3 Switchback Basin Hot Zone 3v3: before 8.305/10.211/12.969, after 8.333/8.562/8.672
  - theme 1 Feature Yard Heist 1v1 / Hot Zone 1v1 (after only): 8.331/8.539/8.650, 8.330/8.558/8.639
- Captures (PNG + JSON per frame), under
  /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0178-evidence/ :
  - before (taken before any source edit): `before/{theme1-practice,theme2-heist,theme3-practice,theme3-hotzone}.txt.captures`
  - after, final tuning: `after/{theme1-practice,theme2-heist,theme3-practice,theme3-hotzone}.txt.captures`
    and `after-t1/{theme1-heist,theme1-hotzone}.txt.captures`
  - after, first tuning pass (too faint): `after-pass1/...`
  - Best views: after/theme3-hotzone.txt.captures/brawler-000660.png (cracks, leaves, scuff,
    pebbles at walls and the edge), after/theme3-practice.txt.captures/brawler-000480.png,
    after/theme2-heist.txt.captures/brawler-000300.png (sand drifts and pebbles at walls),
    after-t1/theme1-hotzone.txt.captures/brawler-000660.png (scuff at a wall).
  - Spawn side is chosen per run, so before/after frames are not pixel-paired.
- Readability review: decals appear only in open cells against walls and the arena edge, never on
  effect tiles, water, bushes, cover, spawn rows, the Heist safe area, or inside the Hot Zone. They
  stay darker or close to the floor value, so fighters, rings (local green, ally blue), names,
  health bars, cover, and walls read exactly as before. Frame times are unchanged. Theme 1 has few
  interior walls in the Feature Yard spawn views, so only one scuff is in frame; Crossroads
  Facility and Barrel Yard have more walls but no Practice game type, so they need the playtest.

# Playtest request

Restart the client only; the server is unaffected (presentation-only, no fingerprint change). Run
`just run 2` → Practice on Feature Yard (Wipeout/Hot Zone/Heist, theme 1), Powderline Vault Heist
3v3 (theme 2), Verdant Crossfire and Switchback Basin (theme 3). Also open Map Maker on a theme-1
map with walls (Crossroads Facility or Barrel Yard) to see more theme-1 decals, and edit a wall to
see decals follow it. Look at:
- Do the decals make the floor read as lived-in, or are they too faint (theme 1 especially) or too
  busy? Density and opacity are per theme in `assets/catalogs/map_presentation_themes.ron`.
- Any decal that could be mistaken for a gameplay element (olive leaves vs. restoration green, sand
  drifts vs. a light patch)?
- Fire/poison/restoration fields, contact shadows, and the Hot Zone ring draw over decals.
- Settings → Display → Graphics quality LOW hides decals; MEDIUM/HIGH show them.
Known limitations: one shared atlas tinted per theme (no per-theme sprite shapes); no mipmaps
(BRL-dq06).

# Learn from errors

- The first tuning pass looked fine in the review sheet but was almost invisible in game: the
  sprite alphas (0.4-0.8) multiplied by the theme opacity (0.5) left 0.2-0.4 coverage, and tints
  close to the floor value vanished under grading. Lesson (again, as in BRL-005q): judge decal
  contrast on native captures, and keep sprite alpha near 1 so the catalog opacity is the one knob.
- I assumed a cell spans 25-45 screen pixels and sized the atlas at 64 px; the capture showed about
  140 physical pixels on the Retina reference Mac, so the atlas went to 128 px cells. Measure the
  on-screen cell size from a capture before choosing texture resolution.
- `sips -c` ignores `--cropOffset` without a matching order and silently produced centre crops;
  use `ffmpeg -vf crop=w:h:x:y` for evidence crops.
- Baseline captures were taken before any compiled source edit, and new files were written only as
  unreferenced modules until the baseline builds finished (the evidence script rebuilds from the
  working tree), following the BRL-m10f/0147/0176 lesson.
