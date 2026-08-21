# V4 Milestone 01 — Reusable environment library and first themed arena

| Field | Value |
|---|---|
| Status | Feedback review |
| Depends on | V3 complete |
| Outcome | Crossroads reads as a rich 3D arena with themed ground, recognizable modular cover, a wide decorated perimeter, and a reusable game-object taxonomy whose placements may mix compatible visual styles |

## Research question

What is the smallest production-reusable change that materially improves the current map, proves a
scalable environment-asset library, and prepares independently stored map documents without
changing planar authority?

## Current implementation findings

### Why the map looks flat

The initial issue was composition plus an overly flat projection:

- `Camera3d` now uses a restrained 27-degree perspective field of view with 55-degree elevation,
  zero horizontal azimuth, real depth, lighting, shadows, and ground-plane ray projection;
- the floor is 504 repeated 64×64 cuboids at `Y = -0.5`, all with one near-black material;
- permanent rectangular cover repeats only `block.glb` (source dimensions 1×0.5×1);
- the perimeter is four generated cuboids only 24 units thick and 72 units high;
- all placed map entities share one neutral 24-unit cube fallback;
- environment handles are hard-coded for one arena block rather than catalog-driven.

The references gain depth from perspective plus layered edge geometry, visible wall tops/fronts, large off-arena
dressing, varied silhouettes, calm but nonempty ground, coherent palettes, and separation between
playable space and the surrounding world.

### Camera finding

The camera is an oblique top-down shooter view, not classic 45-degree grid isometric. This matches
the references' screen-aligned horizontal/vertical geometry. A 45-degree yaw would show two faces
but rotate arena axes and require camera-relative keyboard/controller movement, aim conversion, and
yaw-aware clamping. Retain yaw 0 as the lead candidate and compare:

| Candidate | Elevation | Azimuth | Purpose |
|---|---:|---:|---|
| A | 55° | 0° | Accepted fixed-perspective baseline |
| B | 50° | 0° | More visible wall faces, with higher occlusion risk |
| C | 60° | 0° | More top-down readability, with flatter-looking walls |
| D | 55° | 30° | Diagnostic diagonal view; adopt only if input/framing cost is justified |

Compare candidates on the same improved map at 16:9 plus one narrow/wide viewport. Do not select a
camera from an empty greybox comparison.

## External asset audit

`external_assets/` contains seven CC0 Kenney distributions and 303 GLBs:

| Pack | GLBs | Approx. size | M01 disposition |
|---|---:|---:|---|
| Mini Arena 1.1 | 22 | 2.4 MB | Primary training-arena kit |
| Mini Forest 1.0 | 22 | 3.4 MB | Trees, fence, target, rocks, ground details |
| Mini Dungeon 2.0 | 30 | 4.9 MB | Barrel and compact props |
| Graveyard Kit 5.0 | 91 | 13 MB | Pumpkin variants; later themed props |
| Pirate Kit 2.1 | 72 | 9.8 MB | Later theme; separate source-scale profile required |
| Mini Characters 1.0 | 26 | 14 MB | Existing fighter family |
| Blaster Kit 2.1 | 40 | 4.8 MB | Existing weapon; future target/crates candidates |

The mini packs and Graveyard have convenient near-one-unit modules. Pirate models are much larger
in source units (`rocks-a.glb` is about 5.1×2.9×4.4), proving one global Kenney scale is unsafe.
Every inspected model references its pack-local `Textures/colormap.png`; packs cannot be flattened.

### Candidate object families

| Family | Candidate source | Source dimensions X×Y×Z | Initial role |
|---|---|---:|---|
| floor | Mini Arena `floor.glb` / generated plane | 1×0×1 | prefer one generated themed surface |
| floor detail | Mini Arena `floor-detail.glb` | 1×0.07×1 | sparse decoration |
| wall | Mini Arena `wall.glb` | 1×1×0.6 | modular blocker |
| block | Mini Arena `block.glb` | 1×0.5×1 | low square blocker; already shipped |
| straight border | Mini Arena `border-straight.glb` | 1×0.4×0.6 | presentation-only edge module |
| corner border | Mini Arena `border-corner.glb` | 0.8×0.4×0.8 | presentation-only corner |
| weapon rack | Mini Arena `weapon-rack.glb` | 0.78×0.47×0.45 | decoration or validated low blocker |
| column | Mini Arena `column.glb` | 0.60×1×0.60 | circular/square blocker |
| tree | Mini Forest `tree.glb` | 0.93×1.68×0.88 | round blocker inside; decoration outside |
| barrel | Mini Dungeon `barrel.glb` | 0.52×0.48×0.52 | small blocker or decoration |
| rock | Mini Forest `rocks-low.glb` | 1×0.52×1 | conservative blocker or decoration |
| target | Mini Forest `target.glb` | 0.55×0.58×0.47 | training decoration initially |
| fence | Mini Forest `fence.glb` | 1.10×0.4×0.25 | narrow blocker or outer decoration |
| pumpkin | Graveyard `pumpkin.glb` | 0.38×0.32×0.38 | decoration initially |

Catalog/test every requested family. Crossroads may deliberately mix Mini Arena, Mini Forest, Mini
Dungeon, and Graveyard variants where the user wants them; the theme is only a starting default.
Readability and footprint agreement remain required, but visual coherence is an authoring choice,
not a validator rule. Pumpkins may therefore be placed in the first map if desired.

### Asset-promotion policy

Source distributions stay ignored. Copy only selected GLBs, each pack colormap, and one license per
promoted pack into `assets/`. Preserve exact source path, pack/version, date, license, URL, fallback,
and runtime path in `assets/manifest.ron`.

The reusable library is **game-object first**, not asset-file first:

```text
GameObjectDefinition
  -> authoritative role/footprint + bounded placement/display metadata
  -> compatible VisualVariantDefinition IDs

ThemeDefinition
  -> default ground/edge/lighting and default variant per object kind

MapObjectPlacement
  -> GameObjectDefinition ID
  -> optional explicit VisualVariantDefinition ID override

VisualVariantDefinition
  -> client EnvironmentVisualProfile
     -> GLB scene / generated mesh / primitive fallback
```

The initial taxonomy is role-first so selecting an asset cannot accidentally select gameplay:

| Taxonomy branch | Meaning | Example objects |
|---|---|---|
| `surface.*` | nonblocking map-wide or shaped ground | `surface.ground`, `surface.outer_ground` |
| `boundary.*` | presentation-only playable-edge composition | `boundary.edge.straight`, `boundary.edge.corner` |
| `obstacle.indestructible.*` | authoritative blocker with no destruction lifecycle | `wall.straight`, `wall.corner`, `block`, `column`, optionally tree/rock/fence variants |
| `obstacle.destructible.*` | authoritative discrete blocker with damage/destruction lifecycle | destructible wall, tree, rock, barrel, fence definitions |
| `decoration.*` | explicit visual with no gameplay effect | `tree`, `rock`, `barrel`, `weapon_rack`, `target`, `pumpkin`, `floor_detail` |
| `terrain.destructible.*` | authoritative quantized/chunked terrain field | existing generated terrain reservation; distinct from a discrete obstacle entity |
| `marker.*` | spawn/objective/debug aid | generated Hot Zone and diagnostic overlays, not implied by environment art |

This intentionally allows one source asset to appear under more than one object role. For example,
Mini Forest `tree.glb` can be a decoration with no collision or the visual variant of
`obstacle.indestructible.tree.round` or `obstacle.destructible.tree.round`; the map placement
chooses the object first, so those meanings are never inferred from the model.

The current runtime already supports indestructible placed geometry and chunked destructible
terrain, but it does **not** yet have a general discrete destructible-obstacle lifecycle. M01 keeps
gameplay unchanged, so Crossroads blockers initially use `obstacle.indestructible.*`. Before any
future content exposes `obstacle.destructible.*`, a dedicated milestone specification must own authoritative
health/damage eligibility, collision removal, replication, effects, restart/recovery, and whether
destroyed objects return. Catalog taxonomy does not pretend that runtime exists early.

Examples:

- `obstacle.indestructible.wall.straight` and `obstacle.destructible.wall.straight` are separate
  gameplay definitions compatible with Mini Arena wall, Mini
  Dungeon wall, Graveyard stone/brick wall, and future original variants. Adjacent placements may
  explicitly choose different variants in the same map, and the two gameplay definitions may share
  those variants;
- indestructible and destructible round-tree objects may both use `tree.glb`;
- `decor.tree` may use the same GLB but has no collision and is legal only in decorative layers;
- `edge.straight` and `edge.corner` are presentation-only boundary objects generated from bounds;
- `decor.target`, `decor.weapon-rack`, and `decor.pumpkin` are explicit noninteractive objects;
- `obstacle.destructible.barrel.small` and `obstacle.destructible.fence.narrow` can gain runtime
  support without changing visual asset identity.

M01 introduces the minimum shared `GameObjectDefinition` and `VisualVariantDefinition` metadata
needed by Crossroads and the requested families. Each object owns a stable numeric ID/key, taxonomy
path/category, semantic role, authoritative footprint or explicit `Decoration` role, rotation/snap
rules, display label/tags, and compatible visual-variant IDs. Each wire-safe variant definition owns
only stable identity, object compatibility, native footprint envelope, and fitting policy. The
headless server loads these catalogs but no asset paths.

The client-only environment visual catalog maps stable visual-variant IDs to assets. Each entry owns:

- stable presentation asset ID/key and runtime GLB scene or primitive kind;
- source-to-world scale, yaw, vertical offset, optional pivot correction;
- shadow policy, fallback, footprint-debug metadata, theme/category tags, optional thumbnail.

It never owns collision, gameplay, replicated identity, or mode compatibility. A theme selects
defaults and optional deterministic outside-dressing weights, while explicit placements may
override those defaults. The server validates the override is compatible with the selected object
and fitting policy. For blockers, fitting is explicit: exact footprint, bounded modular stretch, or
visual-contained-within-collision; changing art never silently changes collision. M02 expands map
documents around this model rather than creating a second object model.

## Floor decision

Do not use one baked full-map image as the normal solution:

1. Spawn one generated plane for playable bounds and a larger plane under the outer band.
2. Give the theme a calm material; repeatable textures may be added when suitable art exists.
3. Add sparse `floor-detail.glb` instances with bounded explicit/deterministic placement.
4. Keep objectives, previews, zones, terrain, and gameplay surfaces independent.

This removes hundreds of identical floor entities, avoids stretching a large illustration, supports
all legal map sizes, and gives each map a simple theme default. A future theme texture
should tile or use world-space UVs, not encode object placement into one bitmap.

## Perimeter and outer-world decision

The first theme uses three layers:

```text
playable arena
  -> modular straight/corner edge kit at the visual boundary
  -> 192–320 world-unit non-playable dressing band
  -> outer ground plane beyond the maximum camera footprint
```

- Repeat validated edge modules and dedicated corners.
- Decorate outside with bounded clusters of trees, rocks, columns, fences, racks, barrels, and
  floor details.
- Use edge/corner exclusion zones so silhouettes never intrude into combat.
- Seed dressing from accepted map identity/theme so clients and reconnects agree without replication.
- Keep containment authoritative; border scenes create no server colliders.
- Inside blockers remain explicit geometry. A tree may be decorative outside and a round blocker
  inside, but those are distinct uses.

Reject a single whole-border model as the foundation: it couples one mesh to one map size and
conflicts with variable map bounds. Themes may still add large landmarks to modular borders.

## ECS ownership and composition

Extract map-world presentation rather than extending the broad `reconcile_3d_map` system:

```text
src/client/presentation_3d/
  map.rs                 map generation reconciliation/cleanup
  environment_assets.rs client catalog, GLB readiness, profile resolution
  border.rs              pure module layout and dressing plan
  camera.rs              framing and ground projection
```

- `MapObjectCatalog` is shared server/client-neutral semantic data and contains no render handles.
- `EnvironmentVisualCatalog` retains client profiles and shared handles.
- `MapThemeCatalog` supplies default variants without prohibiting compatible placement overrides.
- `EnvironmentAssetReadiness` tracks loaded/degraded families with fallbacks.
- `Presented3dMap` owns accepted generation/theme identity.
- every map entity carries `MapPresentationMember` plus a focused visual-role marker;
- generated meshes carry explicit cleanup ownership;
- dressing planning is a pure bounded function of bounds, theme, fingerprint/seed, and limits.

Keep the schedule recognizable:

```text
MapPresentationSet::Reconcile
  -> MapPresentationSet::Materialize3d
     -> imported-scene readiness/reconciliation
        -> Transform propagation
```

No visual system runs in `FixedUpdate` or writes collision, replicated pose, or map authority.

## Implementation tasks

### Catalog and assets

- [x] Record accepted floor, camera, border, and source-layout decisions here.
- [x] Add and validate shared game-object and wire-safe visual-variant catalogs, including
      footprints, placement metadata, compatibility, and fitting policies.
- [x] Add/validate the environment visual catalog.
- [x] Promote selected assets with exact provenance/licenses; do not bulk-import.
- [x] Add an import audit for missing dependencies, duplicate IDs/paths, and unmanifested files.

### Renderer and loading

- [x] Extract map/environment/border modules without changing combat ownership/order.
- [x] Replace hard-coded environment handles with catalog-driven loading/profile lookup.
- [x] Preserve automatic fallback and `BRAWLER_FORCE_PRIMITIVE_WORLD=1`.
- [ ] Verify scale, yaw, pivot, readiness, and fallback in pinned Bevy 0.19.1.

### Current map

- [x] Replace repeated dark floor cuboids with playable/outer ground surfaces.
- [x] Add sparse floor details.
- [x] Present blockers through wall/block modules with exact fallbacks for partial/odd shapes.
- [x] Build modular edges/corners and a wide outside dressing band.
- [x] Exercise mixed-style wall/prop choices in one map without making style coherence a validator.
- [ ] Preserve Hot Zone and destructible-terrain readability.

### Camera and lifecycle

- [ ] Capture candidates A–D at 16:9 and narrow/wide viewports.
- [ ] A nonzero azimuth must first prove camera-relative movement/aim and rotated clamping.
- [x] Extend ground/dressing beyond every legal camera footprint.
- [ ] Select one camera after user review and remove comparison switches.
- [x] Bound object counts and prove install/restart/reconnect cleanup.
- [x] Measure native performance against accepted V3 thresholds before considering advanced rendering.

## Implementation evidence — 2026-08-20

Implemented:

- shared, server-safe object/variant/theme catalog in `content/v4/map_objects.ron`;
- client-only visual profiles in `assets/catalogs/environment_visuals.ron`;
- curated Mini Arena, Mini Dungeon, Mini Forest, and Graveyard promotion with pack-local
  colormaps, CC0 licenses, and manifest provenance;
- explicit mixed-style Crossroads wall and decoration placements;
- catalog-driven asynchronous GLB loading, readiness, and primitive fallback;
- one playable ground surface, one camera-covering outer surface, 96 modular edge/corner modules,
  and 64 deterministic outside-only dressing placements for the built-in arena;
- focused `map.rs`, `environment_assets.rs`, and `border.rs` presentation ownership;
- generation-owned cleanup and bounded object/mesh counts.

Verification passed:

- `just check`;
- `just lint`, including dedicated-server feature isolation;
- `just test`: 377 client tests, 304 server tests, 82 serialized network scenarios, 14 performance
  gates, plus routing tests;
- focused current-code client Clippy and 29 presentation tests after the final outer-ground change;
- native imported-asset render evidence at `target/v4-m01-render-evidence.txt`: Metal/Apple M3,
  1,801 samples, 16.675 ms p50, 17.005 ms p95, 17.152 ms p99, no frame over 25 ms, result
  `pass`.

Automated evidence confirms loading, lifecycle, bounds, authority isolation, and performance. The
remaining unchecked items require the user visual pass: model scale/yaw/pivots, Hot Zone and
terrain contrast, and selection of the final camera/floor/perimeter treatment.

## User playtest handoff

Run `just run 1`, choose Wipeout and Hot Zone through Practice, and inspect the imported path. Then
run `BRAWLER_FORCE_PRIMITIVE_WORLD=1 just run 1` once to confirm the fallback remains legible.

Please evaluate:

1. whether the two wall styles align with authoritative cover and expose useful front/top faces;
2. whether the modular edge reads as a raised arena boundary rather than a flat line;
3. whether the outer trees/rocks/props feel rich without obscuring the playable edge;
4. whether the dark green outer surface and dark playable floor are an acceptable first palette;
5. whether the 27-degree-perspective, 55-degree-elevation, zero-azimuth camera provides enough
   depth while preserving shooter readability;
6. Hot Zone, destructible terrain, targeting, projectiles, and overhead UI contrast.

## Feedback review — first visual pass

The first screenshot showed that GLBs loaded and authoritative cover alignment remained intact, but
the result was not visually acceptable:

- **Implemented now — framing:** camera clamping kept the viewport entirely inside authoritative
  camera bounds, clipping the modular border against the screen and hiding the generated outer
  environment. Follow framing now permits a bounded 224-unit presentation margin without changing
  gameplay containment or aim projection.
- **Implemented now — palette:** the near-black playable surface still read as the V3 grey field.
  The training theme now uses a warm earth floor, green outer surface, and green destructible
  terrain for clearer surface separation.
- **Implemented now — dressing density:** equal random selection produced isolated tiny pumpkins
  and floor details instead of a surrounding environment. Dressing is increased from 40 to 64
  bounded placements and deterministically favors trees, rocks, columns, and fences while retaining
  smaller accents.
- **Awaiting second screenshot — border/asset fit:** wall brightness, mixed wall silhouette,
  border height, model pivots, and final palette need another visual pass after the framing fix.

Affected verification passed after these changes: client Clippy with warnings denied and all 30
focused 3D-presentation tests, including the new presentation-margin and revised dressing-count
checks.

## Feedback review — second visual pass

The second screenshot confirmed the bounded camera margin and surface separation now work: the
playable edge and outer ground are visible while combat remains framed. It also exposed the next
presentation defects:

- **Implemented now — ground composition:** twelve broad, low-contrast generated accent patches
  break up the single brown plane without returning to hundreds of floor entities or baking a map
  image.
- **Implemented now — outer clusters:** the 64 dressing placements are reorganized into sixteen
  four-object clusters. Every cluster has a deterministic tree, rock, or column anchor with nearby
  secondary props; isolated rope/fence scatter is no longer treated as a primary silhouette.
- **Implemented now — border depth:** a dark raised generated foundation now supports the modular
  edge kit, and imported/fallback edge modules are lifted onto it instead of reading as a thin pale
  line on the ground.
- **Awaiting third screenshot:** imported wall brightness, accent strength, cluster density, border
  height, and destructible-terrain treatment remain visual-review items.

Affected verification passed: client Clippy with warnings denied and all 30 focused 3D
presentation tests, including deterministic outside-only cluster placement and bounded repeated-map
cleanup with the new generated meshes.

## Feedback review — third visual pass

The third screenshot confirms that the clustered outer dressing and raised border foundation are
working. The generated ground accents were absent because their horizontal meshes were placed
below the playable ground cuboid's top face.

- **Implemented now — accent depth:** ground and accent elevations now use explicit constants, and
  accent meshes sit slightly above the playable surface. A focused regression test enforces that
  ordering.
- **Still under visual review:** imported walls remain very pale and destructible terrain remains a
  blocky green volume. These are separate material and terrain-presentation decisions; they are not
  hidden by the accent-depth correction.

Affected verification passed: client Clippy with warnings denied and all 31 focused 3D
presentation tests, including the new ground-surface ordering regression.

## Feedback review — fourth visual pass

The fourth screenshot confirms that the accents now render, but the twelve large, bright circles
read as regularly painted blobs instead of natural floor variation.

- **Implemented now — organic floor detail:** replace the circle primitive with one bounded
  irregular mesh, distribute eighteen much smaller deterministic marks, and bring the accent color
  closer to the base floor. This keeps the floor cheap and generation-owned while removing the
  dominant repeated ellipse pattern.
- **Still under visual review:** accent subtlety, pale imported walls, and blocky destructible
  terrain.

Affected verification passed: client Clippy with warnings denied and all 32 focused 3D
presentation tests, including bounded irregular accent geometry and surface ordering.

## Feedback review — fifth visual pass

The corrected screenshot confirms that the smaller irregular floor marks now provide restrained
variation. The remaining dominant placeholder is the destructible terrain's single bright, flat
green top.

- **Implemented now — faceted destructible canopy:** retain one bounded mesh per terrain chunk and
  the unchanged occupancy/collision field, but give every occupied cell a deterministic raised
  center, world-stable corner-height variation, and independently lit triangular faces. Darken the
  terrain material so the resulting canopy reads as dense foliage rather than a luminous block.
- **Still under visual review:** terrain silhouette and contrast, plus pale imported walls.

Affected verification passed: client Clippy with warnings denied, all three focused terrain-mesh
tests, all 32 focused 3D-presentation tests, and the maximum-layout terrain performance gate. The
faceted mesh rebuilt in 158.612 microseconds per chunk against the 1.5-millisecond ceiling.

## Feedback review — sixth visual pass

The sixth screenshot confirms the darker faceted terrain reads as a dense, quantized destructible
canopy. Its geometry and gameplay meaning are accepted for M01. Imported wall and perimeter
materials remain much brighter than every other combat element.

- **Implemented now — variant-owned material tint:** environment visual profiles now provide a
  per-variant RGB tint. When an imported environment instance becomes ready, the client clones and
  caches each source material/tint combination before assigning it to that instance. Warm arena
  stone, cool dungeon stone, and the perimeter can therefore be balanced independently without
  mutating shared fighter, weapon, or decoration materials.
- **Still under visual review:** mixed-cover color balance and perimeter brightness.

Affected verification passed: client Clippy with warnings denied, both focused environment-catalog
and tint tests, and all 33 focused 3D-presentation tests.

## Feedback review — seventh visual pass

The seventh screenshot confirms variant tinting works: warm arena walls and cool dungeon walls are
distinct, terrain remains readable, and fighter materials are unaffected. The warm perimeter is
still too bright and orange, while warm cover remains close to the floor value.

- **Implemented now — final warm-palette balance:** darken only the arena wall/block/column and
  perimeter tint values. Preserve the accepted dungeon wall, terrain, floor, lighting, fighter,
  weapon, and decoration treatment.
- **Awaiting final visual acceptance:** warm cover separation and perimeter prominence.

Affected verification passed: client Clippy with warnings denied and both focused
environment-catalog/material-tint tests.

## Feedback review — perspective correction

The user rejected the map's lack of perspective after the seventh visual pass. The fixed
orthographic projection and zero azimuth preserved alignment but removed distance scaling and
vanishing-point depth, leaving the 3D arena visibly flat.

- **Implemented now — restrained perspective camera:** replace the orthographic projection with a
  fixed perspective field of view at the existing 55-degree elevation and zero azimuth. This
  preserves screen-aligned movement while introducing depth and distance scaling.
- **Implemented now — perspective framing:** derive the asymmetric near/far ground footprint from
  camera distance, elevation, field of view, and viewport aspect. Clamp the follow target against
  that trapezoid rather than the former orthographic rectangle.
- **Preserved:** ground-plane cursor ray casting, planar authority, collision, protocol, isolated UI
  camera, and fixed non-orbiting gameplay view.
- **Awaiting visual acceptance:** perspective strength, near-object scale, outer-border coverage,
  and combat readability at map edges.

Affected verification passed: client Clippy with warnings denied and all 34 focused 3D-presentation
tests, including perspective footprint asymmetry, bounded aspect handling, camera clamping, cursor
coordinate conventions, and map lifecycle coverage.

## Feedback review — perspective lens tuning

The first perspective screenshot proves depth and vanishing-point scale now work, but the
35-degree field of view at 1,200 units is visibly too wide-angle: near cover grows sharply, far
cover shrinks aggressively, and the border recedes too strongly.

- **Implemented now — longer gameplay lens:** narrow vertical FOV to 27 degrees and move the camera
  from 1,200 to 1,600 units. The derived ground footprint remains near the accepted combat framing,
  while perspective distortion is reduced.
- **Implemented now — clip coverage:** extend the far clip plane to 4,000 units for the more distant
  camera and decorated outer band.
- **Awaiting visual acceptance:** depth strength, near/far fighter scale, border recession, Hot Zone
  readability, and edge framing.

Affected verification passed: client Clippy with warnings denied and all 34 focused 3D-presentation
tests with the revised lens, derived footprint, clip range, and camera clamping.

## Feedback review — long-lens visual pass

The two long-lens screenshots show that the 27-degree FOV at 1,600 units meets the intended camera
criteria: perspective remains clear, near/far scale is controlled, central and edge framing retain
the raised border, and map axes remain screen-aligned.

- **Accepted for the first pass — camera:** the user accepted the current perspective lens,
  elevation, azimuth, derived clamp, and clip range on 2026-08-21.
- **Corrected review error — overhead labels:** the reviewer misread labels near the viewport edge
  as clipped. The user supplied a clearer screenshot showing complete names, values, and bars. No
  overhead-visibility change is warranted, and the unverified change was reverted.
- **Accepted for the first pass:** current camera treatment and general combat readability. Further
  art and palette refinement remains normal later polish rather than an M01 blocker.

## Verification plan

Automated coverage:

- object catalog duplicate/key/taxonomy/footprint/role/presentation-slot rejection;
- visual catalog duplicate/path/transform/fallback/provenance rejection;
- resolution accepts explicit compatible variants from different styles in one map and rejects
  incompatible object/variant pairs or invalid footprint fitting;
- recursive GLB dependencies and pack-local colormaps;
- border side/corner coverage at minimum/current/maximum bounds;
- deterministic, bounded, outside-only dressing with exclusions;
- ground coverage across clamp/aspect fixtures;
- footprint parity for imported and forced-primitive paths;
- generation teardown for entities/generated meshes;
- Bevy schedule initialization/query safety and server feature isolation;
- existing map, mode, terrain, routing, network, and performance suites.

Visual coverage:

- Wipeout and Hot Zone at 16:9 and supported narrow/wide cases;
- camera candidates from identical positions;
- fighters before/behind each blocker family;
- border at every camera edge without void, clipping, or intrusion;
- imported and forced-primitive paths;
- Hot Zone, previews, bullets, overhead UI, terrain, defeat/respawn, and relation markers against
  the new palette;
- restart, map replacement, reconnect, and result-to-title cleanup.

## Exit criteria

M01 enters playtest when:

1. Crossroads has themed ground, recognizable cover, modular edges/corners, and a wide decorated
   outside band;
2. map placements resolve game-object definitions through a theme to data-driven visual profiles
   with exact provenance and fallbacks;
3. requested families have validated catalog entries and the map can mix compatible styles;
4. both modes preserve authority/readability;
5. lifecycle, isolation, deterministic layout, and native performance checks pass;
6. the user accepts one floor, perimeter, and camera direction.

Complete only after feedback classification, reverification, and learning review. M02 does not
begin before closeout.

## Research references

Local:

- `src/client/presentation_3d/{camera.rs,mod.rs}`
- `src/client/assets.rs`
- `src/map/{model.rs,client.rs,definitions/mod.rs,definitions/resolver.rs}`
- `content/v1/maps.ron`, `assets/manifest.ron`
- `external_assets/*/{License.txt,Sample.png,Overview.html}` and GLBs
- `references/bevy/examples/3d/visibility_range.rs`
- `references/bevy/examples/tools/scene_viewer/`

The checked-in Bevy tree is 0.20-dev, so exact APIs remain verified against pinned Bevy 0.19.1 and
the compiling V3 code.

Primary external:

- [Bevy — Load glTF](https://bevy.org/examples/gltf/load-gltf/)
- [Bevy 0.19 `GltfAssetLabel`](https://docs.rs/bevy/0.19.0/bevy/gltf/enum.GltfAssetLabel.html)
- [Bevy examples and scene viewer](https://bevy.org/examples/)

The official API supports labeled scene/mesh/primitive/material sub-assets. Continue with shared
scene handles first; extract sub-assets only if repeated static families show a real hierarchy or
performance cost.
