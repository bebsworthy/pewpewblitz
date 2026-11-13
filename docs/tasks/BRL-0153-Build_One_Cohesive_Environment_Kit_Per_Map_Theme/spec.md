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


# Design decisions (2026-11-13, implementation pass 1)

## Phased split

The full scope (three themes x walls/cover/foliage/water edge/barrels/chests/pickups/decorations)
is too large for one reviewable change. This ticket delivers:

1. The theme-kit mechanism (client-only catalog schema, resolution, validation, tests).
2. Theme 1 **Crossroads Facility** kit, complete: every built-in theme-1 map (Crossroads Facility,
   Barrel Yard, Feature Yard Wipeout/Hot Zone/Heist, Zombie Yard) renders imported pieces only
   from this kit.
3. Foliage sway, kit provenance, Map Maker palette previews resolving the theme kit.

Theme 2 (Tidal Garden) and theme 3 (Ashen Court) kits move to two `todo` follow-up tickets
(related BRL-0153, parent BRL-0171). The last of them removes the then-unused Kenney/KayKit
environment files, their manifest rows/licenses, and the `tint` field.

## Theme kit resolution (client-only, outside the gameplay fingerprint)

- `assets/catalogs/map_asset_visuals.ron` schema 5 -> 6 adds `theme_kits: [(theme, directory,
  visuals: [<profile>])]`. A kit visual overrides the default profile with the same
  `MapVisualProfileId` for maps of that theme only. `content/` (asset IDs, visual profile IDs,
  recipes, footprints, gameplay profiles) is untouched, so the content fingerprint is unchanged.
- Validation: kit theme exists; each kit visual id is a known shared visual id, unique per kit;
  its `fallback` equals the default profile's fallback (same primitive degradation); every
  imported kit path starts with the kit `directory`. Imported pieces may now also use the
  `Vegetation` and `Barrier` fallbacks.
- Lookup `MapVisualCatalog::resolve(id, theme)` replaces theme-blind lookups at every presentation
  site that knows the theme: static and dynamic map assets, Heist safe, restoration pickups (via
  `Presented3dMap.theme_id`), Map Maker markers, and the Map Maker palette preview (its host
  already carries the document theme, so palette thumbnails show the theme's kit).
- Imported scenes are loaded and admitted per GLB path (not per profile id), so one GLB serves
  any profile/theme that names it; footprint admission still checks every asset that uses it.
- `tint` becomes optional (default white). Kit visuals carry no tint: colour is authored in the
  kit's vertex palette. Legacy default profiles keep their tints until their theme's kit lands.

## Kit authoring (procedural Blender, BRL-0154 precedent)

- One builder `asset_src/environment/crossroads-facility/build_kit.py`, run headless by
  `just facility-kit` (Blender 5.2.3, `--factory-startup`). It writes `kit.blend`, one GLB per piece
  under `assets/brawler/models/original/environment/crossroads-facility/`, review previews, and a
  `KIT_BUILD_REPORT` JSON line; it fails if an export breaks the contract. Rebuilds are
  byte-identical. Provenance lives in the source README (original work, no third-party data),
  referenced from `assets/licenses/` per the acceptance criterion.
- Shared kit rules: 1 Blender unit = 1 map cell, +Z up, ground-centred origin, cell footprint
  `-0.5..0.5`; one shared chamfer (0.045 cell) on every block edge; one material using a flat
  per-face vertex palette in `COLOR_0`, no normals/UVs/textures (Bevy rebuilds flat normals, toon
  material shades it); top-face highlight = upward faces lifted ~18% in value; baked AO = per-vertex
  height gradient (base ~0.62 -> 1.0 by 0.35 cell) applied in the palette. Budget < 2,500 tris
  per piece.
- Pieces and readability roles (gameplay meaning must stay obvious):
  - walls (`wall-arena`, `wall-dungeon` visuals 1/2): cool slate-blue concrete blocks with a pale
    top cap - hue-contrasts the warm orange floor; two face variants of the same block.
  - destructible cover (24): honey-wood crate with dark frame and X-braced top - lighter and warmer
    than walls, reads "breakable".
  - concealment foliage (33): chunky multi-lobe bush, two greens, low dark base; sways.
  - breakable barrier (34): steel frame with yellow/black hazard planks.
  - oil barrel (37) red drum with yellow band; barrel debris (38) flat red/wood scraps.
  - treasure chest (40), Heist safe (39, vault with dial), restoration pickup (42, green canister
    with white cross), weapon rack (11), target (12), pumpkin (13), floor hatch (14, flat).
  - Water surface, rubble, effect tiles, and the arena border stay generated from the theme palette;
    the water edge gets a kit-coloured curb via a new theme `water_edge` colour (theme schema 6).
- Destructible states are unchanged: cover is present/removed, barrels replace with the kit debris,
  chests are removed; object health bars are untouched.

## Foliage sway

Kit visuals may set `sway: true` (validated: Vegetation fallback only). The client inserts a pivot
at the cell's ground centre and tilts it a few degrees with a per-cell phase each frame (a whole-bush
bend about the base). This is a transform sway rather than a vertex-shader sway: the shared toon
material has no vertex stage, and adding one for every toon mesh is not justified for one prop;
at gameplay scale the bush is one chunky silhouette, so the result reads the same. Sway is off under
reduced effects and in primitive-world fallback (no imported scene). Revisit only if playtest asks.

## Unchanged

Collision footprints, recipe IDs, asset/visual profile IDs, gameplay profiles, fitting validation
(`fit_scene_to_footprint`), `BRAWLER_FORCE_PRIMITIVE_WORLD` fallbacks, contact shadows and `flat`.


# Implementation (pass 1, 2026-11-13)

- Mechanism: `assets/catalogs/map_asset_visuals.ron` schema 6 `theme_kits`; `MapVisualCatalog::resolve`
  / `all_visuals`; `validate_theme_kits` (known theme, shared id, unique, same fallback, path inside
  the kit directory, `sway` only on Vegetation). Imported GLBs are handled and admitted per path.
  Theme schema 6: `ground_accent` (only used by the water curb) renamed to `water_edge`; theme 1
  now uses a slate curb (0.50, 0.57, 0.65), themes 2/3 keep their previous values.
- Callers resolved through the theme: static and dynamic map assets, Heist safe and restoration
  pickups (theme of the accepted map, default profile when none), Map Maker markers and palette
  preview (the host's document theme).
- Sway: `environment_assets/sway.rs` (`FoliageSway` pivot, `sway_foliage` in
  `WorldPresentationSet::Animate`, still under reduced effects).
- Kit: `asset_src/environment/crossroads-facility/build_kit.py`, `just facility-kit`, README,
  `kit.blend`, `previews/kit-sheet.png` (re-imports every exported GLB). 14 GLBs under
  `assets/brawler/models/original/environment/crossroads-facility/`, 200 KB total
  (wall-block 5.9 KB, wall-course 9.2, crate 22.5, bush 13.1, barrier 20.6, barrel 13.6,
  barrel-debris 8.5, chest 7.8, vault 14.6, pickup 11.5, weapon-rack 12.0, target 12.8,
  pumpkin 10.7, floor-hatch 11.6); 188-836 triangles each. Two consecutive builds are
  byte-identical (14/14 GLB SHA-1 match).
- Palette iteration from native captures: the pale wall cap read as white blocks and honey crates
  merged with the orange floor, so walls became saturated slate blue (cap #5A7898) and crates pale
  wood with a dark frame; the chest got its own warm wood so it does not read as a crate.
- Provenance: kit README plus `assets/licenses/pewpew-blitz-original-environment.txt`; the asset
  test lets `brawler/models/original/` paths skip the third-party manifest but still requires the
  files. docs/11 updated (kit resolution, foliage sway wording, provenance, ticket table).
- Follow-ups: BRL-0187 (Tidal Garden kit, theme 2), BRL-0188 (Ashen Court kit, theme 3, and
  retiring the Kenney/KayKit environment packs, manifest rows, licenses, and `tint`).

# Verification (pass 1)

- Focused: `cargo test --features client --lib -- environment_assets` 17 passed, including new
  `crossroads_facility_maps_render_only_their_theme_kit` (all 6 theme-1 built-ins, including
  destruction replacements, Heist safe, and pickups), `theme_kit_validation_rejects_...`,
  `original_kit_scenes_fit_every_owned_map_footprint` (GLB bounds through the production
  fitting/admission path for all 14 pieces), and the two sway tests. Existing footprint, contact
  shadow, and Kenney/KayKit profile tests pass unchanged.
- `just lint` (real ripgrep first on PATH): pass.
- `just test`: pass (all suites, 0 failed, incl. performance gates).
- `just e2e 2`: pass (1v1 roster reached Active).
- Native render evidence (`scripts/v3-render-evidence.sh`), all `result=pass`, no panics, no
  `slab_allocator` errors, no rejected scenes; `data/server/profiles.sqlite3` unchanged.
  p50/p95/p99 ms vs BRL-0154 r2:
  - theme 1 Feature Yard combat: 16.665/17.685/18.907 -> 16.660/17.580/19.465 (vsync 60 Hz)
  - theme 1 Feature Yard Heist 2v2 Practice: 8.328/8.595/8.720
  - theme 1 Feature Yard Hot Zone 2v2 Practice: 8.335/8.583/8.714
  - theme 2 Powderline Vault Heist 3v3 Practice: 8.332/8.540/8.662 -> 8.329/8.537/8.687
  - theme 3 Verdant Crossfire Practice: 8.333/8.559/8.666 -> 8.335/8.572/8.674
  - primitive world theme 1 combat 16.673/17.114/17.924 and Hot Zone 8.333/8.582/8.693
  - mesh-entity high water theme 1 combat 329 -> 327.
- Evidence: `target/brl0153/r1-*.captures` (final), `a5-theme1-hotzone-practice` frame 1500 (bushes
  in game; bush unchanged since), `a1-*` (first pass, before palette fixes). Before:
  `target/brl0154/r2-*.captures`. Kit review: `asset_src/environment/crossroads-facility/previews/kit-sheet.png`.
- Capture review: walls (slate blue, pale-blue top), crates (pale wood, dark frame), bushes
  (chunky green lobes), barriers (yellow/black), barrels (red), vault, chest, water curb are
  mutually distinct on the orange floor; themes 2/3 unchanged; primitive world keeps the generated
  fallbacks.
- Not natively captured: Map Maker palette previews (covered by the shared resolve path), sway in
  motion (stills only; unit-tested), pickups and barrel debris in game (no harness selector).
  A local copy of the evidence script with configurable move axis/capture interval was used for
  the Hot Zone runs (scratch only; the checked-in script is unchanged).

# Playtest request

Run `just run 2` (or `just server` + `just client`) and play Practice on Feature Yard (Wipeout,
Hot Zone, Heist), Barrel Yard, Crossroads Facility, and Zombie Yard. Check: walls vs crates vs bushes
vs barriers vs water read instantly at gameplay scale; bushes feel chunky and the sway is gentle (and
stops with reduced effects); barrels look explosive and their debris lies flat; chest and vault are
recognisable; the pickup canister reads as health; crates disappear cleanly when destroyed; Map Maker
palette on a theme-1 document shows the kit. Known limits: themes 2/3 still use third-party pieces
(BRL-0187, BRL-0188); sway is a whole-bush bend rather than per-vertex.

# Learn from errors

- The first review sheet hid the bush behind the vault: the source objects stayed at the origin in
  the render. Cause: the export collection was not excluded from the review render. Prevention: the
  sheet now re-imports the exported GLBs (also a round-trip check) with the sources hidden.
- Workbench previews under-predicted in-game brightness (HDR, grading, warm key light): the pale
  wall cap rendered white and honey wood merged with the orange floor. Prevention: judge palette
  only in native captures against the theme floor; keep prop values clearly apart from the floor hue.
- Theme 3 evidence first ran on Feature Yard because the script defaults Wipeout to 1v1 regardless
  of players per team. Prevention: always pass `BRAWLER_RENDER_GAME_TYPE` for 3v3 maps and check
  `presentation_theme_id` in each report.

# Playtest feedback (2026-11-13)
- User played native sessions (`just run 1`, several 3v3 Practice matches across themes) at e03db203 and accepted the result ("ok for me"). Open look choices in this spec are accepted as implemented. Disposition: accepted; deferred items stay in their linked tickets.
