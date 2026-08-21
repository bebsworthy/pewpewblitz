# V4 Milestone 03 — Second map/theme proof and V4 closeout

| Field | Value |
|---|---|
| Status | Complete |
| Depends on | V4 M02 complete |
| Outcome | A materially different Ashen Court arena and theme prove that built-in map documents, semantic objects, theme defaults, routed admission, and the sole 3D presenter are reusable; V4 then closes with accepted usability, lifecycle, performance, documentation, feedback, and learning evidence |

## Research question

What is the smallest player-visible second map and theme that proves V4 is reusable rather than
Crossroads-specific, while preserving planar server authority, the M02 independent-document and
admission model, the accepted fixed camera, deterministic fallback, readable combat, and bounded
native performance?

## Current implementation findings

M01 and M02 provide most of the required seams, but the second-theme proof exposes several
Crossroads-specific presentation assumptions:

- `content/v4/maps/index.ron` and the generated source table already admit another independently
  validated document without Rust discovery changes.
- lobby game types already advertise stable map IDs and names, queue reservations already rotate
  bounded map pools, and match-worker admission installs the manifest-selected preset. No routing
  or gameplay protocol shape needs to change.
- the Game Select screen already resolves every advertised map ID to its built-in display name.
  Practice starts the selected game type immediately, so assigning the second map to a dedicated
  existing game type is sufficient for a deterministic player-facing exercise path.
- `MapThemeDefinition` currently chooses object defaults and one outside-dressing variant list,
  while `EnvironmentVisualCatalog` owns client paths, scale, yaw, pivot correction, tint, and
  fallback. This separation is correct.
- the playable floor, floor accents, outer ground, primitive wall/perimeter, terrain, ambient
  light, and directional light are still one global hard-coded palette in
  `WorldPresentationPlugin`. A second theme would otherwise rearrange objects over the same
  Crossroads treatment.
- arena edge presentation still selects visual variant IDs 9 and 10 directly instead of resolving
  the theme defaults for `boundary.edge.straight` and `boundary.edge.corner`.
- outside dressing treats numeric variant IDs 4 through 6 as prominent cluster anchors. That
  heuristic happens to fit the first catalog but cannot express a second coherent theme without
  identity-aware code.
- map and terrain lifecycle ownership is already explicit. `Presented3dMap` and
  `MapPresentationMember` own map visuals and generated mesh cleanup; terrain chunks own their
  mutable meshes. Theme reconciliation must extend those owners rather than create a second world
  lifecycle.
- `PresentedMap` and `Presented3dMap` currently treat `MapInstanceId` alone as their reconciliation
  key. A replacement snapshot that deliberately reuses an instance ID but changes recipe
  fingerprint or theme would be ignored; M03 must make the accepted presentation key complete.
- combat, objective, preview, and relationship colors are readability semantics, not theme
  colors. A theme may vary environment materials and restrained lighting but must not recolor the
  controlled/ally/enemy language, Hot Zone boundary, blocked previews, or required combat cues.

The M02 learn-from-errors review also applies directly: new tests should assert resolved behavior,
theme selection, and lifecycle rather than freezing incidental entity counts or authored source
ordering.

## Source and asset findings

The ignored source packs provide three plausible directions:

| Candidate | Useful source material | Result |
|---|---|---|
| Mini Forest 1.0 | tree, rocks, fence, tent, plant, dirt/grass patches | Too close to Crossroads' existing outer trees, rocks, fence, and target to prove a materially distinct second theme without promoting larger structures |
| Pirate Kit 2.1 | fortress wall, palms, sand patches, rocks, docks, boats, towers | Visually distinct, but it creates expectations for water, shore/dock composition, and a separate source-scale pass that are unnecessary for V4 closeout |
| Graveyard Kit 5.0 | modular stone/iron edges, pines, gravestones, coffins, lanterns, pillars, rocks | **Selected**: distinct silhouettes and a cool stone palette fit the existing planar arena and generated-ground model without new surface or gameplay primitives |

The selected Graveyard source is already present under
`external_assets/kenney_graveyard-kit_5.0/`. Its current upstream page identifies version 5.0,
90 files, and CC0. Brawler already ships the pack-local colormap, license, and one pumpkin model;
M03 promotes only the additional GLBs used by Ashen Court.

Source OBJ bounds confirm that the candidate models fit the existing per-variant profile approach:

| Model | Source bounds `(x, y, z)` | Intended role |
|---|---:|---|
| `stone-wall` | `1.000 × 0.725 × 0.200` | modular authoritative wall default at scale 64 |
| `iron-fence-border` | `1.000 × 0.879 × 0.150` | straight decorative arena edge |
| `border-pillar` | `0.222 × 0.760 × 0.222` | edge corner/pillar |
| `pine` | `1.178 × 2.302 × 1.335`, minimum Y `-0.100` | outside anchor and explicit decoration; profile corrects its low pivot |
| `gravestone-wide` | `0.850 × 0.570 × 0.300` | explicit and outside decoration |
| `coffin` | `0.574 × 0.325 × 0.838` | explicit and outside decoration |
| `lantern-glass` | `0.240 × 0.388 × 0.240` | small outside/detail decoration; no dynamic light |

Exact footprint-critical blockers retain generated cuboid/cylinder fallbacks. Decorative models do
not create collision, and edge models remain outside authority.

## Alternatives considered

| Option | Result | Decision |
|---|---|---|
| Add only another recipe using the current theme | Proves document count but not theme reuse; hard-coded palette, edge IDs, and dressing anchors remain hidden | Reject |
| Put palette, lights, and GLB paths in the map recipe | Makes server-neutral authored gameplay content depend on renderer details and asset packaging | Reject |
| Branch in the renderer on Ashen Court's map ID | Fast but violates the milestone gate and makes every future map a code path | Reject |
| Make every presentation value a general authored rule | Over-models counts, cluster algorithms, shadows, and render features without a second demonstrated use | Reject |
| Add a client-only theme profile keyed by stable theme ID | Separates renderer values from server content, supports two real themes, preserves one presenter, and needs no wire changes | **Selected** |
| Build a new per-map selection protocol/screen | Duplicates the existing game-type selection and expands product/network scope | Reject |
| Add both maps to one random/round-robin pool only | Exercises routing but makes the new map awkward to select in Practice and manual playtests | Keep supported, but not the primary exercise path |
| Assign Ashen Court to the existing First Blood game type | Reuses the current product flow and render/E2E automation while leaving Crossroads selectable in Wipeout and Hot Zone | **Selected** |

## Selected player-visible slice

M03 adds one new built-in document:

```text
MapPresetId(3) / MapRecipeId(3)
key: ashen-court
display name: Ashen Court
mode: Wipeout
theme: MapPresentationThemeId(2) / ashen-court
```

Ashen Court is a compact, 180-degree rotationally symmetric courtyard with a different grammar
from Crossroads:

- Crossroads keeps long orthogonal lanes and six large axial wall runs;
- Ashen Court uses a central open dueling court, staggered short stone-wall pairs, circular
  pillar/tree blockers, two asymmetric-looking but rotationally paired destructible groves, and
  looping side approaches;
- left/right team spawns remain safe and face inward;
- at least four spawn points per team retain 1v1 through 3v3 capacity even though the initial
  product exercise is First Blood;
- exact positions and footprints are authored in the independent document and validated through
  the existing Wipeout layout requirements; no procedural playable-layout generator is added.

The map uses theme defaults for its Graveyard wall and edge families. Crossroads continues to prove
explicit mixed Mini Arena/Mini Dungeon/Forest/Graveyard overrides; Ashen Court may use an explicit
compatible non-Graveyard variant only when the visual composition benefits, never to satisfy a
test artificially.

`config/server/game-types.ron` changes the existing `first-blood` map from Crossroads to Ashen
Court and increments that game type's configuration revision. This provides deterministic access
through both Practice and multiplayer without a new UI state or request. The other Wipeout and Hot
Zone rows retain Crossroads, so both map families remain directly selectable.

## Theme and catalog specification

### Shared server-safe definitions

`MapPresentationThemeId(2)` is added to `map_definitions.ron` and `map_themes.ron`. The existing
theme key is reconciled to the accepted `training-arena-mixed` name in both catalogs so one stable
ID does not carry two labels.

The shared object/variant catalogs add only demonstrated Ashen Court roles:

- Graveyard stone wall compatible with `obstacle.indestructible.wall.straight`;
- Graveyard pine compatible with blocking and decorative tree objects;
- Graveyard rocks compatible with blocking and decorative rock objects;
- Graveyard iron edge and pillar compatible with generated straight/corner boundary objects;
- focused inert `decoration.gravestone`, `decoration.coffin`, and `decoration.lantern` objects and
  their visual variants.

The new theme supplies a default for every object it uses. Compatible explicit overrides remain
legal across themes. No theme or variant defines collision, health, damage, terrain occupancy,
spawn behavior, or asset paths.

### Outside dressing roles

Replace the numeric prominence heuristic with two bounded theme-owned lists:

```ron
outside_dressing_anchor_variants: [...],
outside_dressing_detail_variants: [...],
```

Every four-item deterministic cluster selects its first item from the anchor list and the
remaining items from the detail list. Both lists are nonempty, sorted, reference known variants,
and may overlap when a theme deliberately permits that. Placement count, band, cluster geometry,
seed, and outside-only containment remain the existing bounded client algorithm; M03 does not add
a general procedural environment language.

The training theme classifies its current trees/rocks/columns as anchors and its smaller props as
details. Ashen Court uses pines and larger stone silhouettes as anchors, with gravestones, coffins,
lanterns, rocks, and pumpkins as details. No code checks a theme, map, or numeric variant range.

This source-shape change advances the map-object catalog schema, map fingerprint format, and global
gameplay-content envelope. The map recipe schema and resolved snapshot wire shape do not change.
Canonical material includes the new definitions and both dressing lists in stable order.

### Client-only theme profiles

Extend `assets/catalogs/environment_visuals.ron` with a client-only theme profile list keyed by
`MapPresentationThemeId`. Each profile owns the small set of renderer values that currently vary
as one global palette:

```text
playable ground color       ground accent color
outer ground color          fallback wall color
fallback perimeter color   terrain/debris color
ambient color/brightness    directional color/illuminance
```

Colors are finite normalized RGB values. Brightness and illuminance are finite, positive, and
bounded to conservative project-owned ranges. Roughness, metallic value, shadow policy, light
direction, camera, accent mesh/count, border height, and dressing geometry remain renderer
constants because M03 has no demonstrated need to author them.

Training Arena retains its accepted warm earth/green palette and current lighting. Ashen Court
uses a cool desaturated stone floor, darker blue-green outer surface, subdued stone fallbacks, and
a restrained cool ambient/key-light balance. It must remain bright enough for red/blue/green
relationship markers, projectiles, terrain, previews, and overhead UI to keep their accepted
contrast.

The environment catalog transaction validates exact one-to-one theme coverage against the shared
theme catalog. A missing, duplicate, unknown, or invalid profile fails client startup. These
profiles remain absent from the server feature graph and gameplay fingerprint; stable theme
selection is shared, while exact render colors are packaged client presentation.

### Theme-resolved presentation

- create/cache one small environment material set per client theme; do not allocate a material per
  entity or per map generation;
- leave combat, objective, status, preview, and relationship handles in the existing invariant
  material resource;
- resolve straight/corner edge variants through theme defaults for boundary object IDs instead of
  literal variant IDs;
- map ground, accents, circular/primitive cover, fallback edges, and decoration fallbacks consume
  the selected environment theme materials;
- terrain chunk and debris presentation resolves the current replicated snapshot's theme. A chunk
  records or reconciles its theme so same-generation theme replacement updates material handles
  without rebuilding unchanged occupancy meshes;
- extend the renderer-neutral accepted-map state with the recipe fingerprint and presentation
  theme, and key `Presented3dMap` reconciliation on instance ID plus fingerprint plus theme rather
  than instance ID alone;
- ambient and directional light values reconcile once when accepted map instance/theme changes;
  the single existing light entity and camera remain lifecycle owners;
- removing/replacing a map releases generation-owned meshes/entities and leaves only the bounded
  cached per-theme materials and shared imported handles.

## Asset promotion and readiness

Promote only the selected Graveyard GLBs into the existing pack namespace and add exact entries to
`assets/manifest.ron`. Reuse the already shipped pack-local `Textures/colormap.png` and retained
license; do not duplicate them.

Every new variant receives a measured scale, yaw, vertical/pivot correction, tint, and fallback in
the environment visual catalog. Shape-critical walls use exact cuboid fallback. Boundaries use the
existing boundary fallback. Pines, rocks, graves, coffins, and lanterns use neutral readable
decoration fallbacks.

Environment readiness remains one catalog-wide asynchronous transaction. Missing or failed new
GLBs degrade only to the existing primitive path; they never delay map authority or alter
collision. `BRAWLER_FORCE_PRIMITIVE_WORLD=1` must present both themes distinctly through their
theme palettes and fallback silhouettes.

## ECS ownership, lifecycle, and schedule

```text
shared MapObjectCatalog + resolved snapshot theme ID
                         |
              +----------+----------+
              |                     |
              v                     v
server authority/lobby        client EnvironmentVisualCatalog
(no renderer change)          + cached theme materials/scenes
                                      |
                                      v
MapPresentationSet::Reconcile -> Materialize3d -> readiness/reconcile
                                      |
                         terrain presentation reads same theme
```

- `MapContentPlugin` continues to own immutable shared map/object/theme definitions.
- lobby configuration owns which product game type selects Ashen Court; reservation and worker
  admission keep their M02 authority.
- `AuthoritativeMapPlugin` resolves, installs, replicates, resets, and tears down the new document
  through the unchanged planar snapshot.
- `WorldPresentationPlugin` owns client-only theme profiles, material caches, the single ambient
  resource/light entity, imported scenes, map visuals, and fallbacks.
- `ClientTerrainPlugin` retains terrain convergence and mesh ownership while selecting the
  environment material directly from the same accepted snapshot/theme cache. It does not depend
  on a separately deferred mutable "current theme" resource.
- no theme system runs in `FixedUpdate`, writes a replicated component, changes collision, or
  creates a second map/presentation lifecycle.
- existing `MapPresentationSet::Reconcile -> Materialize3d -> Readiness` ordering remains explicit;
  schedule-initialization tests cover any changed query/resource access, and deferred commands are
  not assumed visible across unordered systems.

## Network and product behavior

No new network message, replicated component, protocol registration, per-message version, or
compatibility decoder is introduced.

1. the lobby resolves `ashen-court` by stable key from the embedded catalog;
2. the existing First Blood advertisement includes `MapPresetId(3)` and its display name;
3. Practice and multiplayer send the same existing game-type selection intent;
4. reservation/manifest construction carries preset 3 and its indexed admission revision;
5. the worker validates mode, revision, fingerprint, and topology before startup;
6. clients receive the authoritative resolved snapshot and select presentation by its stable theme
   ID;
7. unknown IDs, mismatched content, wrong mode/revision, or insufficient map capacity continue to
   fail before authority starts.

The global gameplay fingerprint intentionally changes with the new shared definitions, theme, and
map. Client-only palette numbers and GLB paths remain outside gameplay compatibility, as they do
not affect server authority.

## Implementation plan

### Phase 1 — Theme data and client presentation seam

- [x] Add the second shared theme and replace numeric dressing prominence with theme-owned anchor
      and detail lists.
- [x] Add client-only theme profiles and validate exact shared/client theme coverage and bounded
      colors/light values.
- [x] Cache per-theme environment material handles while preserving invariant combat/readability
      materials.
- [x] Resolve borders, ground, fallback cover/perimeter, lighting, terrain, and debris from the
      accepted snapshot theme without map-ID branches.
- [x] Advance the object/fingerprint/content versions and update intentional golden identities.

### Phase 2 — Ashen Court content and assets

- [x] Promote only the selected Graveyard GLBs with exact provenance and reuse the existing
      texture/license files.
- [x] Add focused semantic objects/variants and measured client profiles with deterministic
      fallback.
- [x] Author `content/v4/maps/builtin/ashen-court.ron`, add its sorted index entry, and validate
      Wipeout plus 1v1–3v3 spawn capacity.
- [x] Change First Blood to Ashen Court, increment its configuration revision, and preserve the
      other Crossroads game types.
- [x] Confirm no Rust discovery table, map identity branch, protocol shape, or gameplay authority
      rule is added.

### Phase 3 — Usability, lifecycle, and performance hardening

- [x] Verify imported model scale, pivot, yaw, footprint agreement, border joins, and outside-only
      dressing at supported viewport aspects.
- [x] Verify map replacement/restart/reconnect and same-generation theme replacement cleanly
      reconcile map entities, generated meshes, terrain materials, lighting, and readiness.
- [x] Tune only the selected map layout, palette, lighting, dressing mix, and model profiles needed
      for combat readability; record broader polish in the backlog.
- [x] Run Ashen Court imported and forced-primitive native render evidence against the accepted
      release thresholds and compare high-water/terminal counts with Crossroads.

### Phase 4 — V4 closeout

- [x] Run canonical format, role, lint, unit, network, performance, routed E2E, and automated
      native-render gates.
- [x] Reconcile V4 roadmap, product/presentation/asset documentation, root README, and backlog from
      actual implementation evidence.
- [x] Deliver a deterministic user playtest path for Crossroads Wipeout, Crossroads Hot Zone, and
      Ashen Court First Blood.
- [x] Classify every feedback item, rerun affected checks, perform the learn-from-errors review,
      and close V4 only after user acceptance.

## Verification plan

### Catalog, theme, and asset tests

- parse and validate both shared themes and both client profiles;
- reject missing/duplicate/unknown theme profiles, invalid colors/light values, empty or unknown
  dressing role lists, incompatible defaults, and unsorted/duplicate stable IDs;
- prove both theme defaults resolve straight/corner boundaries and shared object families without
  map identity checks;
- prove explicit compatible overrides work under either theme and incompatible variants fail;
- audit every environment visual path against `assets/manifest.ron`, shipped files, pack-local
  dependencies, provenance, and license inventory;
- forced failure and `BRAWLER_FORCE_PRIMITIVE_WORLD=1` retain distinct, readable theme palettes.

### Map and authority tests

- independently parse, RON-round-trip, normalize, fingerprint, and resolve Ashen Court;
- assert its bounds, blockers, terrain reservations, spawn areas/points, and Wipeout mode contract
  are materially different from Crossroads and valid for 1v1, 2v2, and 3v3;
- keep authority parity: explicit/default visual variants lower to the same collision geometry,
  decorations remain collider-free, and clients cannot choose geometry/theme/map identity outside
  existing game-type intent;
- operator catalog advertises First Blood with preset 3 and rejects wrong mode, unknown map,
  duplicate map, wrong admission revision, or insufficient capacity;
- separate-App and routed-process coverage proves preset 3 reaches the worker, match summary,
  diagnostics, and clients with matching resolved fingerprint.

### Presentation and lifecycle tests

- materialize both themes through the same system and assert selected ground, edge, fallback,
  terrain, and lighting profiles;
- dressing is deterministic for a fingerprint, uses each theme's anchor/detail roles, stays outside
  playable bounds, and remains bounded;
- map instance replacement across theme 1 → 2 → 1 removes old members/generated meshes, updates
  light and terrain handles, and does not grow terminal/high-water ownership unexpectedly;
- a replacement snapshot that reuses `MapInstanceId` but changes recipe fingerprint/theme still
  reconciles; an identical snapshot remains idempotent;
- late imported-scene readiness upgrades either theme without duplicate map members;
- missing/invalid presentation profile degrades visibly without affecting map/terrain readiness or
  authority;
- camera framing, aim projection, Hot Zone objective, relation markers, projectiles, previews,
  overhead UI, and terrain mutation retain accepted contrast at 16:9 plus narrow/wide fixtures.

### Canonical commands and native matrix

- `just fmt`, `just check`, `just lint`, and `just test`;
- `just e2e 2`, `just e2e 4`, and `just e2e 6` through the existing routed product path;
- release native render evidence for Crossroads Hot Zone and Ashen Court First Blood with imported
  assets;
- the same two paths with `BRAWLER_FORCE_PRIMITIVE_WORLD=1`;
- manual Practice pass at supported narrow/16:9/wide windows covering both map families, both
  modes, movement/aim, combat density, terrain mutation, defeat/respawn, and restart.

The accepted V3/M01 render thresholds remain locked unless measured evidence proves they are
invalid. Debug builds are not performance evidence. Reports record adapter/hardware, resolution,
sample count, frame percentiles, over-budget frames, visible/high-water/terminal entity and mesh
counts, fixed-tick behavior, map ID, theme ID, and fallback policy.

## Implementation and verification evidence

M03 implementation preserves one renderer and one authority path while adding:

- independently embedded `ashen-court` recipe/preset 3, selected deterministically by First Blood;
- shared theme 2 and Graveyard semantic object/variant families, with theme-owned outside-dressing
  anchor/detail roles rather than numeric variant heuristics;
- two validated client-only environment profiles and cached per-theme materials for ground,
  perimeter, fallback cover, lighting, terrain, and debris;
- complete map presentation reconciliation by instance, recipe fingerprint, and theme, including
  same-generation terrain rematerialization;
- eight promoted Graveyard GLBs with manifest provenance and deterministic primitive degradation;
- render-report schema 2 map recipe/theme identity and an exact game-type automation selector. The
  selector was necessary because roster size alone selected the first matching 2v2 game rather
  than proving Crossroads Hot Zone.

Automated verification on 2026-08-21:

- `just lint`: passed formatting, routing/client/server Clippy, server feature isolation, and the
  retired-renderer source guard;
- `just test`: 83 routing library tests plus routing process suites, 391 client tests, 311 server
  tests, 82 serialized network tests, and 14 performance gates passed. The heaviest current fixed
  tick sample was the 24-fighter/24-seam-brush gate at p95 12.829 ms, inside budget;
- `just e2e 2`, `just e2e 4`, and `just e2e 6`: exact 1v1, 2v2, and 3v3 product rosters each
  reached `Active` through the routed supervisor/lobby/match-worker topology;
- lifecycle coverage includes same-instance theme replacement, terrain rematerialization without
  occupancy rebuild, repeated map replacement, restart/reconnect, deterministic dressing, asset
  audit, and authority/fingerprint/admission rejection paths.

Release native evidence used a 10-second warm-up and 30-second measurement on Apple M3/Metal. All
four primary reports had stable high-water/terminal entity and asset counts and passed the locked
thresholds:

| Scenario | Presentation | p95 / p99 | >25 ms | Entities high/terminal | Recipe / mode / theme |
|---|---|---:|---:|---:|---|
| Ashen Court First Blood | Imported | 17.892 / 19.864 ms | 5 | 1321 / 1321 | 3 / 2 / 2 |
| Ashen Court First Blood | Primitive | 17.329 / 19.518 ms | 7 | 716 / 716 | 3 / 2 / 2 |
| Crossroads Hot Zone | Imported | 17.609 / 18.853 ms | 4 | 1427 / 1427 | 2 / 3 / 1 |
| Crossroads Hot Zone | Primitive | 17.919 / 19.763 ms | 8 | 758 / 758 | 2 / 3 / 1 |

The remaining viewport/model-fit/readability checklist is visual acceptance work, not an automated
performance claim. It remains open for the playtest below.

## User playtest handoff

For the visual acceptance pass:

1. run `just run 1`;
2. choose Wipeout or Hot Zone in Practice to inspect the accepted Crossroads theme;
3. choose First Blood in Practice to inspect Ashen Court deterministically;
4. repeat First Blood once with `BRAWLER_FORCE_PRIMITIVE_WORLD=1 just run 1`.

Requested observations will cover layout flow and spawn safety; floor/edge/dressing coherence;
wall and prop footprint agreement; fighter/projectile/terrain readability; primitive degradation;
and whether the two maps feel materially distinct while clearly belonging to the same game.

## Feedback review — detached fighter overhead UI

The user supplied three screenshots showing live remote-player/bot name and health blocks detached
from their fighter, including a repeatable-looking default placement in the top-left corner. This
is classified **implemented now** because it is current combat-readability presentation and the
failure was exposed during the M03 visual pass.

The first correction was incomplete. It ordered projection after camera viewport recomputation and
transform propagation but before Bevy UI preparation/layout, hid roots when the viewport was
unavailable, and rejected elevated-label intersections whose fighter body anchor was off-screen.
Those are valid edge/resize fixes, but a follow-up screenshot proved a separate default-position
path remained.

Root cause and final correction:

- overhead state/text reconciliation and screen projection both wrote root `Visibility`;
- state reconciliation could unhide a newly created live root while its absolute `Node` still had
  the default `(0, 0)` position;
- projection used replicated fighter `Position` instead of the propagated transform of the actual
  independent fighter visual root;
- projection is now the sole owner that may make an overhead root visible, and it does so only
  after writing a valid node position anchored to the matching propagated `V3FighterVisual` root;
- missing visual roots, invalid cameras/viewports, defeated fighters, failed projections, and
  off-screen fighter anchors all keep the overhead hidden.

Affected verification passed: four focused overhead tests, all 393 client tests, canonical lint
and role-isolation checks, and a fresh two-client imported native report with p95 17.073 ms, p99
17.267 ms, zero frames over 25 ms, stable 1321/1321 entity high-water/terminal counts, and result
`pass`. The generalized render-evidence cleanup also received a Bash 3.2-safe empty PID sentinel
after this pass exposed the shell's `set -u` empty-array behavior.

The user accepted the corrected First Blood result and directed V4 closeout on 2026-08-21. The
detached-overhead report is therefore **implemented and accepted**, with no remaining M03 feedback
awaiting evidence.

## Learn-from-errors review

The M03 closeout produced these reusable lessons:

- When two Bevy systems mutate the same presentation component, schedule order alone does not
  establish semantic ownership. The first overhead correction improved projection timing but
  missed the independent state system that could reveal an unpositioned root. For projected UI,
  enumerate every writer during diagnosis and keep one system as the sole reveal owner; other
  systems may only force safe hidden states.
- Project client presentation from the propagated transform of the actual visual root, not merely
  from a parallel replicated gameplay position. This preserves attachment across interpolation,
  visual offsets, deferred spawning, camera updates, and UI layout preparation.
- A screenshot showing an exact viewport corner is strong evidence of an uninitialized absolute UI
  coordinate. Treat default `(0, 0)` placement as a lifecycle/visibility clue before tuning
  projection arithmetic.
- Evidence automation must be exercised through every roster branch on the repository's initial
  macOS shell. Bash 3.2 plus `set -u` treats an empty array differently from newer shells; the 1v1
  cleanup path now retains a harmless sentinel, while expanded rosters use the same bounded cleanup.
- Visual acceptance remains necessary after automated projection and lifecycle tests. The user's
  follow-up screenshot disproved the first partial diagnosis and prevented an incomplete closeout.

These lessons are recorded beside the owning presentation and evidence contracts. They refine
existing Bevy ECS and repository practices rather than establishing a new reusable project skill.

## Closeout

All M03 and V4 exit criteria are satisfied as of 2026-08-21. Two independently embedded maps and
two distinct themes use the same authoritative resolver and sole client presenter; imported and
primitive paths, routed admission, lifecycle cleanup, role isolation, canonical tests, native
performance, and readability evidence pass. The only playtest defect was classified, corrected,
reverified, and accepted. Deferred editor, provisioning, additional-theme, and release-polish work
remains in the root backlog. M03 and V4 are `Complete`.

## Risks and containment

- **Theme becomes a second renderer:** keep one materializer, one camera/light lifecycle, one
  imported-scene path, and one primitive path. Profiles supply values only.
- **Presentation data leaks into authority:** stable theme/variant IDs remain shared; paths, colors,
  light values, handles, and model transforms remain client-only.
- **Dark theme harms competitive readability:** preserve invariant combat/relation/objective colors,
  bound light values, test forced fallback, and require visual acceptance before closeout.
- **New models disagree with collision:** use measured profiles, modular/contained fit validation,
  and exact primitive fallback; decoration never adds collision.
- **Terrain keeps the previous theme material:** include theme identity in terrain presentation
  reconciliation and test theme replacement without an occupancy generation change.
- **Dressing remains secretly first-theme-specific:** replace numeric ID prominence with validated
  anchor/detail lists and test both themes through the same pure planner.
- **Manual access to the new map is unreliable:** assign it to First Blood rather than relying on
  round-robin selection or adding a new protocol/UI control.
- **Asset/package growth:** promote only the selected GLBs and reuse the already shipped Graveyard
  colormap/license.
- **Closeout expands into general polish:** fix only evidence-backed usability/readability failures;
  record original art, editor, additional themes, advanced rendering, and unrelated release polish
  in their owning backlogs.

## Deferred and out of scope

- player-facing map editor, custom-map persistence/launch, remote transfer, publishing, or asset
  upload;
- user-selected visual themes independent of map recipes, skins, cosmetics, or entitlements;
- a general procedural playable-layout or outside-world generator;
- destructible discrete barrels/fences and their health/removal/replication lifecycle;
- water, shorelines, docks, vertical traversal, 3D physics, or pirate-map surface behavior;
- dynamic local lights on lanterns, day/night cycles, weather, fog, post-processing, outlines,
  decals, LOD, instancing, or custom render pipelines;
- arbitrary theme-authored camera, shadow, cluster algorithm, material shader, or effect settings;
- original replacement art, additional built-in maps/themes, balance changes, and general release
  polish not caused by M03.

## Exit criteria

M03 may enter user playtest when:

1. Ashen Court is an independently validated built-in Wipeout document with a materially different
   layout and a deterministic product-flow exercise path;
2. two shared themes and two client theme profiles use the same resolver, presenter, asset
   readiness, fallback, terrain, lighting, and lifecycle owners;
3. no production code branches on map identity or numeric visual-variant ranges;
4. server authority, protocol shapes, planar collision, stable wire identity, and client intent
   boundaries remain unchanged;
5. imported and forced-primitive paths keep combat, objectives, terrain, relation markers, and UI
   readable;
6. theme/map replacement, restart, reconnect, delayed asset readiness, and terrain mutation clean
   up and converge without unbounded growth;
7. canonical checks, routed E2E, native performance, role isolation, source audit, and documentation
   reconciliation pass.

M03 and V4 become `Complete` only after user feedback is classified, accepted changes are
reverified, the learning review is recorded, all V4 docs describe actual final behavior, and the
user accepts the closeout result.

## Research references

### Local project and pinned references

- `docs/implementation/v4/{roadmap,milestone-01,milestone-02}.md` — accepted theme, asset,
  independent-document, admission, verification, and feedback decisions;
- `docs/{00-product-direction,08-network-architecture,11-art-and-presentation-direction,12-sprite-inventory,13-player-ux,14-multiplayer-server-architecture}.md`
  — product, authority, readability, asset, player-flow, and routed-process constraints;
- `content/v4/{map_definitions,map_objects,map_visual_variants,map_themes}.ron` and
  `content/v4/maps/` — current shared catalogs, theme 1, built-in index, and independent recipes;
- `src/map/{model,objects,client}.rs` and `src/map/definitions/` — stable theme identity,
  validation/fingerprints, resolved snapshot, and presentation lifecycle;
- `src/client/presentation_3d/{mod,map,border,environment_assets}.rs` and
  `src/terrain/client/presentation.rs` — current hard-coded palette/light/edge assumptions,
  deterministic dressing, imported readiness, generated mesh ownership, and terrain material use;
- `src/server/lobby/{catalog,queue}.rs`, `src/server/admission.rs`, and
  `config/server/game-types.ron` — advertised maps, deterministic reservation selection,
  match-worker admission, and the existing deterministic First Blood exercise path;
- `external_assets/kenney_{mini-forest_1.0,graveyard-kit_5.0,pirate-kit}/` — ignored complete source
  packs, licenses, previews, GLBs, and OBJ bounds used for the theme comparison;
- `references/bevy/examples/README.md` — warns that development examples can differ from released
  APIs and directs version-specific verification;
- `references/bevy/examples/3d/{3d_scene,lighting}.rs` and
  `references/bevy/examples/gltf/gltf_skinned_mesh.rs` — official local examples for shared
  `StandardMaterial` assets, ambient/directional lighting resources/components, and GLB scene
  loading. Brawler's compiling 0.19.1 path remains the exact API authority.

### Primary external sources

- [Bevy 3D Scene example](https://bevy.org/examples/3d-rendering/3d-scene/) — official minimal
  `Mesh3d`, `StandardMaterial`, light, and camera composition;
- [Bevy Lighting example](https://bevy.org/examples/3d-rendering/lighting/) — official material,
  ambient-light, directional-light, and shadow composition; exact APIs are checked against the
  pinned build because the public example tracks current Bevy;
- [Kenney Graveyard Kit](https://kenney.nl/assets/graveyard-kit) — current primary pack page,
  version 5.0, 90 files, CC0;
- [Kenney Mini Forest](https://kenney.nl/assets/mini-forest) — current primary pack page, version
  1.0, 20 files, CC0;
- [Kenney Pirate Kit](https://kenney.nl/assets/pirate-kit) — current primary pack page, version 2.1,
  70 files, CC0.

The checked-in Bevy source is 0.20-dev while Brawler pins Bevy 0.19.1. M03 introduces no novel
render API: it extends the already compiling 0.19.1 `Assets<StandardMaterial>`,
`GlobalAmbientLight`, `DirectionalLight`, `AssetServer`, and GLB readiness paths. Local exact-version
compilation and tests take precedence over copying newer example syntax.
