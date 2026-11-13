# Outcome

Every built-in theme-3 map (Ashen Court, Switchback Basin, Verdant Crossfire) draws its imported
environment pieces only from one original Ashen Court kit, and no shipped map uses a third-party
environment pack or a per-asset tint multiplier.

# Context

BRL-0153 delivered the theme-kit mechanism and the theme-1 Crossroads Facility kit; BRL-0187 does
theme 2. Theme 3 still draws Kenney graveyard pieces (stone wall 16, pine 19, rocks 20, gravestone
21, coffin 22, lantern 23) and KayKit BlockBits (44, 45, 48, 49) with tints.

# Scope

- `asset_src/environment/ashen-court/build_kit.py` with the shared kit contract and a `just` recipe.
- Theme-3 kit visuals for every visual a theme-3 map draws, including destructible cover (24),
  concealment bush (33, sway), and decorations.
- After all three kits exist: make kit coverage mandatory for every theme, retire the default
  third-party profiles in favour of kit visuals, remove unused files under
  `assets/brawler/models/kenney/` and `kaykit/`, their `assets/manifest.ron` rows and
  `assets/licenses/` texts, and the `tint` field from `map_asset_visuals.ron` and code.

# Out of scope

Gameplay geometry, map content, content fingerprint.

# Acceptance criteria

- All built-in maps render only their theme's kit; footprint tests pass; no environment tint field;
  manifest and licenses list only shipped files; Map Maker previews show each theme's kit.

# Verification

Focused tests, `just lint`, `just test`, `just e2e 2`, native captures of every built-in map plus
primitive world.



# Design decisions (2026-11-13, implementation pass 1)

## Coverage

Theme-3 built-ins draw these non-surface visuals (placements, filled rects, destruction
replacements, restoration pickups; no Heist anchors): Ashen Court 16 stone wall, 19 pine (2x2,
circle collider), 20 rocks (2x2, circle collider), 21 gravestone, 22 coffin, 23 lantern (decorations),
24 destructible cover; Switchback Basin 45 red brick wall, 49 green striped cover, 33 tall grass;
Verdant Crossfire 44 green symbol wall, 48 yellow striped cover, 33 tall grass; all maps 42
restoration pickup. Ground (15/30), water, rubble, effect tiles, and the arena border stay
generated from the theme palette. The kit adds exactly these 13 `theme_kits` visuals for
`MapPresentationThemeId(3)`: 16, 19, 20, 21, 22, 23, 24, 33 (sway), 42, 44, 45, 48, 49.

## Ashen Court identity and readability (neutral mid-grey stone floor, cool moonlight)

An overgrown moonlit court: dark basalt masonry, soot brick, carved rune stones, ash-dusted pines,
moss, weathered oak and iron, lantern light. Roles are separated by hue and value against the grey
floor, and wall/cover colours stay clear of the effect-tile colours (speed cyan, slow violet, damage
red: no saturated red, cyan, or violet on walls, cover, or their tops):

- Walls (16, 44, 45) share one dark basalt plinth, a slate cap rim, full height and full cell
  (Exact), and are clearly darker than the floor: 16 blue-slate ashlar courses; 44 dark rune stone
  with a carved pale-lichen roundel and moss trim (replaces the bright green KayKit symbol block);
  45 soot brick, desaturated umber-brown courses with dark mortar and a charcoal top inlay (not red).
- Destructible cover (24, 48, 49) is lower than walls and always warm amber oak with a dark iron
  frame, so "breakable" reads the same on all three maps: 24 X-braced reliquary crate, 48 crate with
  ochre/cream painted bands, 49 crate bound with copper straps. Warm amber is the complement of the
  cool floor and light, and is far from damage red.
- Concealment 33: chunky moss-green fern bush (lobes plus upright fronds), lighter and more
  saturated than the pine canopy, sways via `sway: true`.
- Blockers 19 and 20 (2x2, circle collider radius 28 of a 64-unit footprint): 19 a tiered dark
  teal pine with ash-pale tips on a stone collar; 20 a cluster of blue-grey boulders with moss caps.
  Both stay near their circular collision footprint (~0.9 of the 2x2 cell span).
- Decorations stay low and quiet: 21 lichen headstone on a moss mound, 22 dark oak coffin with
  brass trim, 23 iron post lantern with warm amber glass (vertex colour only, no emission).
- Pickup 42: the cross-theme health language (green canister, white cross) with iron rings.
- `water_edge` and other theme colours stay unchanged unless native captures show a conflict.
- Build: `asset_src/environment/ashen-court/build_kit.py` (palette and geometry only, shared
  `kit_common.py`), `just ashen-kit`, README, kit sheet, provenance line in
  `assets/licenses/pewpew-blitz-original-environment.txt`.

## Retiring the third-party defaults and `tint`

- Default profiles (`visuals`) still exist for every shared visual id, because a Map Maker document
  of any theme may place any asset. Each previously third-party imported default now names the
  original kit piece of its native theme (theme 1: 1, 2, 11, 12, 13, 14, 37, 38, 39, 40, 42;
  theme 2: 32, 43, 46, 47; theme 3: 16, 19, 20, 21, 22, 23, 44, 45, 48, 49) with the same scale and
  fitting as that kit entry. Generated defaults (24, 33, 34, 36, ground, water, rubble, effect
  tiles, hidden marker) stay generated; they are not third-party.
- Validation: every theme in the client theme catalog must have a kit (kit coverage is mandatory),
  and every imported default must import from a theme kit directory, so no third-party environment
  path can return through the defaults.
- `tint` is removed from the schema (`deny_unknown_fields` rejects it), from `MapVisualProfile`,
  and the `EnvironmentMaterialTint` component; environment roots are marked with the existing
  `ToonScene` instead, and the toon conversion cache is keyed by source material only. Effect-tile
  colours were never read from `tint` (their materials are code-owned), so nothing visual changes.
  Map visual schema 6 -> 7.
- Files removed after proving them unreferenced (catalogs, code, tests, Map Maker, VfxLab,
  dashboard, zombies, scripts, tools): every GLB and colormap under `assets/brawler/models/kenney/`
  `mini-arena/`, `mini-dungeon/`, `mini-forest/`, `graveyard/`, all of `assets/brawler/models/kaykit/`,
  the unreferenced `kenney/blaster-kit/crate-wide.glb`, their `assets/manifest.ron` rows, and the
  licenses `kenney-mini-arena.txt`, `kenney-mini-dungeon.txt`, `kenney-mini-forest.txt`,
  `kenney-graveyard-kit.txt`, `kaykit-block-bits.txt`. Kept: Kenney mini-characters (zombie NPC),
  blaster-kit `blaster-a.glb` and its colormap (fighters), icons and sounds, and their licenses.

## Theme-2 coral wall tops (reviewer note)

Agreed: the coral-brick wall's top inlay used the brick red and, lifted by the top-face highlight
and HDR, reads as a saturated red square from the gameplay camera, close to the red damage effect
tile. Change: the coral-brick wall's cap inlay uses a new darker, desaturated `brick_top` tone; the
brick courses on the faces stay coral so the wall keeps its identity. Only `coral-brick-wall.glb`
changes; the other 10 theme-2 GLBs must rebuild byte-identical.

## Unchanged

Content, recipes, footprints, gameplay profiles, fingerprint; kit mechanism; sway, contact
shadows, toon material, outlines, destructible states, primitive fallbacks (kit visuals keep the
default fallback kinds), Map Maker palette resolution.

## Native evidence plan

Verdant Crossfire (`wipeout-3v3`) and Switchback Basin (`hot-zone-3v3`) are reachable in Practice;
Ashen Court is in no game type, so its capture uses a local, uncommitted
`config/server/game-types.ron` edit (`wipeout-3v3` maps -> `ashen-court`), reverted with
`git checkout`. Plus theme 1 and theme 2 regression captures (Powderline Vault for the coral-wall
change) and a primitive-world run.



# Implementation (pass 1, 2026-11-13)

- Theme 3 kit: `asset_src/environment/ashen-court/build_kit.py`, `README.md`, `kit.blend`,
  `previews/kit-sheet.png`; `just ashen-kit`. 13 GLBs under
  `assets/brawler/models/original/environment/ashen-court/`, 216,236 bytes total: ashlar-wall
  12.6 KB (452 tris), rune-wall 16.9 (672), soot-brick-wall 16.4 (596), reliquary-crate 28.9
  (1,100), banded-crate 22.5 (836), strapped-crate 25.3 (968), fern-bush 18.3 (780), ash-pine 16.0
  (688, 2x2), boulders 13.6 (560, 2x2), headstone 11.3 (428), coffin 7.8 (264), lantern 13.2
  (484), pickup 13.5 (544). Two consecutive builds: 13/13 GLB SHA-1 identical. Theme-1 rebuild
  14/14 GLBs unchanged in git (its `kit.blend`/sheet timestamps restored to HEAD).
- Catalog: map visual schema 6 -> 7; `theme_kits` entry for `MapPresentationThemeId(3)` (16, 19,
  20, 21, 22, 23, 24, 33 sway, 42, 44, 45, 48, 49); the 25 formerly third-party imported defaults
  now name their native theme's kit piece with the same scale/fitting; `tint` removed everywhere.
- Validation (`catalog.rs`): every client theme must have a non-empty kit; imported defaults must
  import from a kit directory. Code: `tint`/`tint_rgb`/`EnvironmentMaterialTint` removed; imported
  environment roots carry `ToonScene`; `ToonSceneMaterials` is keyed by source material only;
  Heist idol assets no longer carry an unused profile.
- Theme-2 coral wall top: new `brick_top` (#4B3A38) inlay; only `coral-brick-wall.glb` changed
  (16,024 bytes, same size), the other 10 theme-2 GLBs rebuild byte-identical.
- Palette iteration from native captures: first-pass walls read as saturated blue under the cool
  moonlight and HDR grading, and the rune-wall lichen discs read white, so ashlar/rune/cap were
  desaturated and the lichen darkened; the boulder cluster was rounded out towards its circular
  collider (fill 0.90 -> 0.94); the lantern roof hid the amber glass from the gameplay camera, so
  the roof shrank and the lantern grew (0.60 -> 0.66).
- Retired (proved unreferenced by `git grep` over catalogs, `src/`, tests, tools, scripts, and the
  Map Maker/VfxLab/dashboard/zombie code paths; only `assets/manifest.ron` and the tests that
  asserted them referenced these): 34 files, 934,003 bytes — all of
  `assets/brawler/models/kenney/{mini-arena,mini-dungeon,mini-forest,graveyard}/` (GLBs and
  colormaps), all of `assets/brawler/models/kaykit/`, `kenney/blaster-kit/crate-wide.glb`, licenses
  `kenney-mini-arena.txt`, `kenney-mini-dungeon.txt`, `kenney-mini-forest.txt`,
  `kenney-graveyard-kit.txt`, `kaykit-block-bits.txt`; 29 `assets/manifest.ron` rows. Kept: Kenney
  mini-characters (zombie NPC), `blaster-a.glb` and the blaster colormap, icons, sounds, and their
  licenses.
- Tests: `ashen_court_maps_render_only_their_theme_kit` (3 maps), new
  `every_builtin_map_renders_only_its_theme_kit` (all 11 built-ins),
  `imported_defaults_are_their_native_theme_kit_pieces` (25 defaults), kit validation now covers a
  missing kit and an empty kit, `original_kit_scenes_fit_every_owned_map_footprint` checks 38 GLBs,
  the asset test asserts every environment visual is an original kit piece, no retired
  environment rows remain in the manifest, and the retired licenses are gone. Kenney/KayKit
  path/scale assertions and the tint unit test were removed with the feature; the toon conversion
  test now checks cached untinted conversion. `src/map/tests.rs` test renamed (no "kaykit").
- Docs: docs/11 (mandatory kits, defaults name kit pieces, no tint, effect-tile hue rule);
  `pewpew-blitz-original-environment.txt` lists the ashen-court kit; tidal-garden README notes the
  brick top.

# Verification (pass 1)

- Focused: `cargo test --features client --lib -- environment_assets toon presentation_3d` 215
  passed; `-- client::assets map::tests` 39 passed.
- `just lint` (real ripgrep first on PATH via a scratch wrapper, after `cargo fmt`): pass.
- `just test`: pass (all suites, 0 failed, including performance gates).
- `just e2e 2`: pass (1 exact 1v1 roster reached Active).
- Native render evidence (scratch copy of `scripts/v3-render-evidence.sh` with configurable move
  axis and capture schedule; checked-in script unchanged), all `result=pass`, no panics, no
  `slab_allocator` errors, no asset load failures or rejected scenes in client logs;
  `config/server/game-types.ron` restored with `git checkout` after each Ashen Court run;
  `data/server/profiles.sqlite3` unchanged. p50/p95/p99 ms:
  - theme 3 Verdant Crossfire Practice: 8.332/8.551/8.653 (BRL-0187 r1: 8.336/8.570/8.696)
  - theme 3 Switchback Basin Hot Zone 3v3 Practice: 8.333/8.560/8.669
  - theme 3 Ashen Court Practice (local wipeout-3v3 -> ashen-court): 8.333/8.570/8.701
  - theme 3 Verdant Crossfire primitive world Practice: 8.331/8.539/8.636 (`fallback=primitive`)
  - theme 2 Powderline Vault Heist 3v3 Practice (coral top check): 8.335/8.570/8.698
  - theme 1 Feature Yard combat regression: 16.672/17.345/18.399, mesh-entity high water 327
    (unchanged from BRL-0153/0187)
- Evidence (final `target/brl0188/r1-*`): `r1-theme3-verdant-practice`, `r1-theme3-switchback-practice`,
  `r1-theme3-ashen-practice` (8 captures; crates, headstone, lantern, boulders, walls),
  `r1-theme3-verdant-primitive`, `r1-theme2-vault-practice`, `r1-theme1-combat`; iterations
  `a1`-`a6` (a2/a5 1300-frame boulders, a6 coffin). Before: `target/brl0187/r1-theme3-practice.txt.captures`.
  Kit sheet: `asset_src/environment/ashen-court/previews/kit-sheet.png`.
- Capture review: ashlar, rune, and soot-brick walls read as one dark wall family above the grey
  floor; amber oak crates, moss-green fern bushes, blue-grey boulders, and decorations are mutually
  distinct; no wall top, cover, or prop uses a saturated red, cyan, or violet; theme-2 coral wall
  tops are now dark brown, not red; primitive world keeps generated fallbacks; theme 1 unchanged.
- Not natively captured: the 2x2 ash pine (the Practice automation strafes around the dummy and
  never reached the court centre in six runs; covered by the kit sheet and the footprint test),
  restoration pickup (no selector), Map Maker palette on a theme-3 document (shared resolve path,
  unit-covered), sway in motion (unit-tested in BRL-0153).

# Playtest request

Run `just run 2` and play Practice Wipeout 3v3 (Verdant Crossfire) and Hot Zone 3v3 (Switchback
Basin). For Ashen Court, temporarily point `wipeout-3v3` in `config/server/game-types.ron` at
`ashen-court` and revert afterwards. Check: the three wall types read as walls and as one set
against the grey floor (are they too blue under the moonlight?); amber crates read as breakable
cover on all three maps and vanish cleanly; fern bushes read as concealment and sway; the ash pines
and boulder clusters read as solid 2x2 blockers whose size matches their collision; headstones,
coffins, and lanterns stay quiet decoration; Powderline Vault's coral walls no longer show red
tops; Map Maker palette on a theme-3 document shows the kit. Known limits: Credits text is stale
(BRL-0189).

# Learn from errors

- The first rune-wall export failed the Exact contract (depth 1.03): a face roundel protruded past
  the cell. Prevention: keep face details inside the +-0.5 cell bounds; the builder's self-check
  caught it before any runtime use.
- Removing `EnvironmentMaterialTint` left four `if let (Some(profile), Some(fitted))` sites with an
  unused profile, and a blanket rewrite briefly broke the one site that still needs it. Prevention:
  change call sites individually after the compiler lists them.
- The first scratch ripgrep wrapper pointed at itself (a `find` for `rg` found the wrapper). The
  shell's `rg` is a function wrapping the Claude Code binary (`ARGV0=rg`); the working wrapper
  runs `exec -a rg <binary>`.
- Native captures again showed the theme light shifting greys strongly (cool moonlight made slate
  read saturated blue); palette was judged and fixed only in captures (reconfirmed lesson).
- Practice automation cannot be steered to an arbitrary prop; captures of central props need a
  scripted camera or spawn selection (not added; noted as a limit).
