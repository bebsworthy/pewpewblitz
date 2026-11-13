# Outcome

Every built-in theme-2 map (Tidal Garden, Powderline Vault) draws its imported environment pieces
only from one original Tidal Garden kit.

# Context

BRL-0153 delivered the theme-kit mechanism (`theme_kits` in `assets/catalogs/map_asset_visuals.ron`,
schema 6, resolved per map theme), foliage sway, the theme `water_edge` colour, and the theme-1
Crossroads Facility kit built by `asset_src/environment/crossroads-facility/build_kit.py`
(`just facility-kit`). Theme 2 still draws Kenney mini-arena/graveyard and KayKit BlockBits pieces
with per-asset tints.

# Scope

- A `build_kit.py` builder under `asset_src/environment/tidal-garden/` with the same kit contract
  (shared bevel, top-face highlight, height AO in a vertex palette, one material, byte-identical
  rebuilds, self-validation, review sheet) and a `just` recipe.
- Theme-2 kit visuals for every visual a theme-2 built-in map draws: garden wall (32), red brick
  wall (45), metal wall (46), wood wall (47), yellow striped cover (48), cactus (43), seashell (36,
  generated today), concealment bush (33, sway), breakable barrier (34), Heist safe (39), pickup (42).
- Readability: walls vs destructible cover vs bushes vs water stay distinct at gameplay scale.
- Extend the kit coverage test to theme 2; provenance in the kit README and
  `assets/licenses/pewpew-blitz-original-environment.txt`.

# Out of scope

Gameplay geometry, map content, content fingerprint; removing the default third-party profiles
(BRL-0188).

# Acceptance criteria

- Tidal Garden and Powderline Vault render only theme-2 kit imports; footprint/visual-bounds tests
  pass; Map Maker palette previews show the kit for theme-2 documents.

# Verification

Focused tests, `just lint`, `just test`, `just e2e 2`, native captures of both maps (practice,
combat) plus primitive world.


# Design decisions (2026-11-13, implementation pass 1)

## Coverage

Theme-2 built-ins draw these non-surface visuals (placements, destruction replacements, Heist
anchors, restoration pickups): 32 garden wall, 45 red brick wall, 46 metal wall, 47 wood wall
(Exact walls); 48 yellow striped cover and 43 cactus (destructible cover); 33 tall grass
(concealment, sways); 34 breakable barrier (2x1 footprint); 36 seashell (decoration, generated
today); 39 Heist safe (3x2 anchor); 42 restoration pickup. Ground (15/30), water (31), rubble (35),
effect tiles, and the arena border stay generated from the theme palette, as for theme 1. The kit
adds exactly these 11 `theme_kits` visuals for `MapPresentationThemeId(2)`; no other theme-2 visual
is added (Map Maker documents of theme 2 that use non-built-in assets keep the defaults until
BRL-0188 retires them).

## Shared builder code

The theme-1 builder is split: the kit contract (Piece parts, one bevel, flat per-face palette,
top-face highlight, height AO bake, single material, GLB export, GLB self-validation, review sheet,
build report, deterministic save) moves to `asset_src/environment/kit_common.py`, parameterised by a
small `KitSpec` (name, ticket, palette, output directory, pieces). Each kit's `build_kit.py` keeps
only its palette and piece geometry. The theme-1 refactor must rebuild byte-identical GLBs (checked
with SHA-1 against the committed files). `just tidal-kit` runs the theme-2 builder the same way as
`just facility-kit` (Blender headless, `--factory-startup`). Bytecode writing is disabled so no
`__pycache__` lands in `asset_src/`.

## Tidal Garden identity and readability (warm sand floor)

Coastal walled garden: sea-worn stone, coral brick, verdigris tide gates, driftwood, beach-hut
paint, seagrass, shells. Rules derived from BRL-0153 capture lessons (judge in native captures
against the floor; keep props clearly apart from the floor hue; no pale caps that read white):

- Walls (all four share the dark stone plinth, the sea-stone cap, full height and full cell, so
  they read as one wall family): 32 sea-stone courses (cool blue-grey), 45 coral-red brick courses,
  46 iron tide-gate panels (dark blue steel with verdigris rivet bands), 47 driftwood palisade
  (blue-grey vertical planks with rope bands). All are darker and cooler (or, for brick, much
  darker) than the sand.
- Destructible cover is lower than walls and painted, never stone: 48 beach-hut crate with
  turquoise and white painted stripes inside a dark frame (turquoise is the complement of the sand,
  replacing today's yellow block that merges with the floor); 43 sage cactus with pink flowers in a
  terracotta pot (columnar silhouette, lighter than the bushes).
- Concealment 33: chunky seagrass bush (deep sea greens, lobes plus a few upright fronds), sways
  via BRL-0153 `sway: true`.
- Breakable barrier 34: a 2-cell lifeguard fence (driftwood posts and rails, red and white
  planks, rope), distinct from both walls and the turquoise cover.
- Seashell 36: low cream/pink scallop with radiating ribs (decoration, small).
- Heist safe 39: barnacled iron sea-chest vault with a brass porthole dial and turquoise trim
  (same 3:2 proportions as the theme-1 vault).
- Pickup 42: the same health language as theme 1 (green canister, white cross) with rope rings,
  so restoration reads identically across themes.
- `water_edge` and other theme colours stay unchanged unless native captures show a conflict.

## Unchanged

Content, recipes, footprints, gameplay profiles, fingerprint; kit mechanism and validation; sway,
contact shadows, toon material, outlines, destructible states, primitive fallbacks (kit visuals
keep the default fallback kinds). Default profiles and `tint` stay until BRL-0188.

## Native evidence plan

Powderline Vault is reachable through `heist-3v3` (Practice and combat). Tidal Garden is in no
game type, so its native capture uses a local, uncommitted `config/server/game-types.ron` edit
(`wipeout-3v3` maps -> `tidal-garden`), reverted with `git checkout` right after the run.


# Implementation (pass 1, 2026-11-13)

- Shared contract: `asset_src/environment/kit_common.py` (`KitSpec`, `KitPiece`, `Piece` parts,
  palette bake with top-face highlight and height AO, one material, GLB export, self-validation
  now also rejecting zero-triangle pieces, review sheet, build report). `Piece.cylinder` gained an
  optional `top_radius` (default unchanged). The theme-1 builder keeps only its palette and
  geometry; its rebuild is byte-identical (14/14 GLBs unchanged in git). `kit.blend` and the sheet
  PNG are not byte-stable between runs (timestamps), so the theme-1 ones were restored to HEAD.
- Theme 2 kit: `asset_src/environment/tidal-garden/build_kit.py`, `README.md`, `kit.blend`,
  `previews/kit-sheet.png`; `just tidal-kit`. 11 GLBs under
  `assets/brawler/models/original/environment/tidal-garden/`, 213 KB total: sea-stone-wall 13.8 KB
  (496 tris), coral-brick-wall 16.0 (584), tide-gate-wall 10.4 (364), driftwood-wall 12.6 (452),
  beach-crate 41.7 (1,584), cactus 23.6 (1,024), seagrass-bush 18.3 (780), lifeguard-fence 20.2
  (748, 1.92 x 0.8 cell for the 2x1 barrier), scallop-shell 10.1 (352), sea-vault 22.4 (916),
  pickup 13.5 (544). Two consecutive builds: 11/11 GLB SHA-1 identical.
- Catalog: `theme_kits` entry for `MapPresentationThemeId(2)` with 32, 33 (sway), 34, 36, 39, 42,
  43, 45, 46, 47, 48; walls Exact, everything else Contained; no tint.
- Palette iteration from native captures (HDR brightens the Workbench preview a lot): wall tops all
  looked identical, so the shared cap rim got a material inlay per wall; sea-stone and brick
  inlays first read pale/orange against sand, so stone, cap, brick, and driftwood were darkened and
  inlays use the base (not light) tone; the tide-gate inlay moved off verdigris so wall tops never
  echo the turquoise cover; the crate lid lost its white plank; the barrier was thickened from
  0.44 to 0.8 cell so its visual matches its full-cell collision.
- Presentation fix (in scope, needed for the kit): static and dynamic map visuals were fitted to
  the *rotated* footprint and then rotated again by their root, so a rotated non-square placement
  rendered at the wrong size/orientation (Tidal Garden's two quarter-turned 2x1 barriers rendered
  one cell long; the primitive fallback rendered them crosswise). Both sites now size children in
  the unrotated footprint (the contact shadow already did). Only rotated non-square placements
  change; built-ins have exactly those two. docs/11 records the rule; test
  `rotated_rectangular_dynamic_visual_matches_its_rotated_authoritative_footprint`.
- Tests: the kit coverage test is now `assert_theme_maps_render_only_their_kit(theme)`, used by
  `crossroads_facility_maps_render_only_their_theme_kit` (6 maps) and new
  `tidal_garden_maps_render_only_their_theme_kit` (2 maps). It is stricter: every non-surface
  visual must be a kit import (only generated ground/water/rubble/effect tiles allowed), and
  destruction replacements are derived from the assets actually placed on each map (the old helper
  added every damage profile's replacement to every map). `original_kit_scenes_fit_every_owned_map_footprint`
  now checks 25 GLBs.
- Provenance: tidal-garden README, `assets/licenses/pewpew-blitz-original-environment.txt` lists the
  kit and the shared module; crossroads README points to `kit_common.py`; docs/11 notes the shared
  kit module and the rotated-footprint rule.

# Verification (pass 1)

- Focused: `cargo test --features client --lib -- environment_assets` 18 passed;
  `-- presentation_3d` 206 passed.
- `just lint` (real ripgrep first on PATH, after `cargo fmt`): pass.
- `just test`: pass (all suites, 0 failed, including performance gates).
- `just e2e 2`: pass (1 exact 1v1 roster reached Active).
- Native render evidence, all `result=pass`, no panics, no `slab_allocator` errors, no rejected
  scenes; `data/server/profiles.sqlite3` restored with `git checkout` after the runs.
  p50/p95/p99 ms:
  - theme 2 Powderline Vault Heist 3v3 Practice: 8.329/8.534/8.672 (BRL-0153 r1: 8.329/8.537/8.687)
  - theme 2 Powderline Vault Heist 3v3 combat: 16.670/17.524/18.991 (vsync 60 Hz)
  - theme 2 Tidal Garden 3v3 Practice: 8.335/8.577/8.680
  - theme 2 Tidal Garden 1v1 combat: 16.663/17.449/18.163
  - theme 2 Tidal Garden primitive world Practice: 8.335/8.568/8.670 (`fallback=primitive`)
  - theme 1 Feature Yard combat regression: 16.661/17.236/17.912, mesh-entity high water 327
    (unchanged from BRL-0153)
  - theme 3 Verdant Crossfire Practice regression: 8.336/8.570/8.696
- Tidal Garden is in no shipped game type: its runs used a local `config/server/game-types.ron`
  edit (wipeout-1v1 and wipeout-3v3 -> `tidal-garden`), reverted with `git checkout` before the
  Powderline/theme 1/theme 3 runs. Practice runs that needed later frames or other movement used a
  scratch copy of the evidence script with configurable move axis and capture schedule (checked-in
  script unchanged).
- Evidence (final `target/brl0187/r1-*`): `r1-theme2-vault-practice`, `r1-theme2-vault-combat`,
  `r1-theme2-tidal-practice`, `r1b-theme2-tidal-practice` (move axis 0,1: rotated lifeguard
  barricade, shell, bushes, walls, water), `r1-theme2-tidal-combat` (horizontal barricade),
  `r1-theme2-tidal-primitive`, `r1-theme1-combat`, `r1-theme3-practice`; iterations `a1`-`a6`.
  Before: `target/brl0153/r1-theme2-practice.txt.captures`. Kit sheet:
  `asset_src/environment/tidal-garden/previews/kit-sheet.png`.
- Capture review: sea-stone (teal-slate), coral brick (dark red with darker inlay), and driftwood
  walls read as one wall family and stay darker/cooler than the sand; turquoise striped crates,
  green seagrass bushes, red/white barricade, cream shell, iron sea vault, and blue water are
  mutually distinct; primitive world keeps the generated fallbacks; themes 1 and 3 unchanged.
- Not natively captured: tide-gate wall and cactus (Powderline areas the harness camera did not
  reach), restoration pickup (no selector), Map Maker palette on a theme-2 document (shared
  resolve path, unit-covered), sway in motion (unit-tested in BRL-0153).

# Playtest request

Run `just run 2` (or `just server` + `just client`) and play Practice Heist 3v3 on Powderline Vault.
To reach Tidal Garden, temporarily point `wipeout-3v3` in `config/server/game-types.ron` at
`tidal-garden` (BRL-0117 owns map choice in Practice) and revert afterwards. Check: the four wall
types read as walls and as one set; turquoise crates and the cactus read as breakable cover;
seagrass bushes read as concealment and sway gently (still under reduced effects); the red/white
barricade reads as breakable and is two cells long in both orientations; water stays obvious;
the iron sea vault reads as the Heist objective; destroyed crates/barricades disappear or become
rubble cleanly; Map Maker palette on a theme-2 document shows the kit. Known limits: third-party
default profiles and `tint` remain for theme 3 until BRL-0188.

# Learn from errors

- The first coverage test for theme 2 failed on barrel debris that no theme-2 map can produce:
  the helper added every damage profile's replacement to every map. Prevention: derive runtime
  replacements from the placed assets' gameplay profiles.
- A non-square kit piece exposed a latent double rotation (fit to the rotated footprint, then rotate
  the root). Theme 1 hid it with square barriers. Prevention: fit children in the root's local
  (unrotated) footprint; check rotated non-square placements in captures whenever a piece is not
  square.
- Workbench previews again under-predicted in-game brightness; the first wall caps and brick
  inlays merged toward the sand. Prevention (reconfirmed): iterate palette only in native captures.
- A narrow barrier silhouette (0.44 cell) under-represented a full-cell collider. Prevention:
  keep contained props near their collision footprint in both axes.
- Practice countdown and random spawn made early fixed-frame captures miss props; captures after
  the countdown with an explicit move axis were needed.
