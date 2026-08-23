# Version 8 implementation roadmap

## Purpose and scope

V8 replaces the prior continuous-placement and special-region map system with the sparse grid and
unified map-asset system specified in
[Grid map-asset system specification](../../16-grid-map-asset-system.md). A map becomes bounded
integer dimensions, a default surface and presentation theme, a sparse list of stable map-asset
placements, and typed mode anchors. One author-facing `MapAssetId` joins placement rules, a
server-known gameplay profile, normal presentation, and optional destroyed presentation while the
dedicated server remains free of client asset paths and rendering dependencies.

V8 is a hard production cutover. It converts every built-in map, shared map catalog, client visual
catalog, map runtime, network snapshot, test fixture, and current specification, then deletes the
superseded schema and implementation. Historical implementation records remain evidence; there is
no runtime V4 decoder, compatibility loader, dual authoring path, or dormant old catalog after
closeout.

V8 also proves why the replacement exists with one small original reference-inspired environment
layout using irregular water, vegetation, walls/corners, default ground, decorations, spawns, and a
mode anchor where applicable. The ignored drawings under `external_assets/map_images/` inform the
grammar but are not shipped, traced, or copied as authored maps.

## Version status

| Field | Value |
|---|---|
| Status | Complete |
| Current milestone | V8 complete; V9 M01 is next |
| Entry gate | Satisfied: M03 completed and the user authorized M04 implementation on 2026-08-23 |
| Completion gate | Every built-in and proof map uses the one sparse-grid `MapAssetId` pipeline; required surface/feature/destruction behavior and any shipped concealment are authoritative and recoverable; all canonical and native checks pass; the user accepts the converted maps; the superseded production system has zero remaining source/content/runtime references |

Allowed statuses are `Not started`, `Researching`, `Specification review`, `Implementing`,
`Verifying`, `User playtest`, `Feedback review`, `Complete`, and `Blocked`.

## Specification-review decisions

The version-level proposal is:

1. Use one fixed 32-world-unit authoring grid. Recipes store integer dimensions and cells; world
   bounds and transforms are derived.
2. Give each cell one effective surface, at most one feature, at most one inert decoration, and a
   bounded set of validated markers. A bush and wall cannot coexist in the same cell.
3. Make `MapAssetId` the only author-facing placeable identity. A shared definition references one
   placement contract, one bounded gameplay profile, one normal visual profile, and an optional
   destroyed visual profile.
4. Keep client asset paths, transforms, materials, generated presentation, and fallbacks in a
   client-only catalog keyed by stable visual ID. Preserve `assets/manifest.ron` as provenance, not
   gameplay or placement data.
5. Express player collision, projectile collision, concealment, destruction/replacement, and
   implemented interaction through explicit server-known properties. Do not retain category roles
   such as `ObstacleIndestructible` or `TerrainDestructible`.
6. Derive walkability from player collision. Reject contradictory catalog definitions instead of
   accumulating independent booleans.
7. Represent player spawns as parameterized marker assets. Keep mode-owned area anchors typed and
   separate because the selected mode owns scoring and required layout semantics.
8. Require complete server-owned privacy/reveal behavior before a concealing bush ships. A client
   opacity trick is not accepted concealment.
9. Convert built-ins directly and bump the global current schema/protocol/content identity. Do not
   implement a legacy map decoder or runtime migration.
10. Retain historical milestone documentation unchanged while removing every old-system production
    type, file, loader, path, test fixture, diagnostic term, and current-specification claim.

M01 research and specification preparation were authorized on 2026-08-22. V7 completed and the
user approved M01 implementation on 2026-08-23. M02 completed after the user accepted Tidal Garden,
the coarse whole-cell destruction rule, and honestly non-concealing tall grass on 2026-08-23.
M04 and V8 completed on 2026-08-23 after the user accepted the final hard-cutover closeout with no
additional map, collision, topology, imported/fallback, or presentation change requested.

## Milestone overview

| Milestone | Status | Player/developer-visible deliverable | Plan |
|---|---|---|---|
| 01 | Complete | One routed Crossroads Wipeout match authored, resolved, collided, destroyed, rendered, restarted, and recovered exclusively through the new sparse-grid map and unified map-asset catalogs | [milestone-01.md](./milestone-01.md) |
| 02 | Complete | Original Tidal Garden 2v2 proof with shaped water, honestly non-concealing vegetation, adjacency presentation, multi-cell barriers, and deterministic rubble replacement | [milestone-02.md](./milestone-02.md) |
| 03 | Complete | All existing Wipeout and Hot Zone built-ins converted with accepted gameplay/presentation parity through one new map pool | [milestone-03.md](./milestone-03.md) |
| 04 | Complete | V8 hard cutover and closeout: domain-organized production content, no old production map system, and accepted converted/proof maps | [milestone-04.md](./milestone-04.md) |

## Ordering rationale and milestone gates

### M01 — New catalog/grid vertical slice and first Crossroads conversion

Deliver one playable end-to-end result before converting the library. Introduce the exact integer
grid contract, stable IDs, shared map-asset and gameplay-profile catalog, client visual/theme
catalog, recipe/index loader, canonical resolver, resolved snapshot, static and destructible server
installation, dynamic recovery, client materialization, and map lifecycle. Convert Crossroads
Wipeout into the new source format and route an actual match through it.

M01 owns only behaviors required by the first slice: default walkable surface, blocking player-and-
projectile features, destructible blocking center cover, inert decorations, player spawn markers,
perimeter, dynamic recovery, and existing visual fallbacks. It does not claim bush concealment,
water, replacement-state visuals, teleport, chest rewards, or a generic interaction framework
before those behaviors exist.

Temporary coexistence is allowed only inside the implementation window because Ashen Court and Hot
Zone still require a runnable baseline. The new resolver never accepts an old recipe and the old
resolver never accepts a new recipe. Every temporary old-path reference is listed in the milestone
with its M03/M04 deletion owner; no compatibility abstraction is built to make coexistence
permanent.

Gate:

- Crossroads Wipeout source contains integer dimensions, default surface, sparse `MapAssetId`
  placements, and spawn marker parameters only;
- server and client derive identical bounds, cell transforms, footprints, fingerprints, and visual
  references without client assets entering the server feature graph;
- player and projectile collision agree with the selected gameplay profiles;
- radius-based map destruction removes deterministic 32-unit cover cells, repairs collision,
  resets, and recovers without exposing the old region/terrain wire model;
- imported and primitive-fallback rendering preserve accepted Crossroads readability;
- map install, restart, recovery bootstrap, replacement, teardown, and routed admission work for
  the new snapshot;
- focused coordinate/conflict/catalog tests and representative separate-App/routed tests pass;
- the user accepts the first converted map before M02 expands behavior.

### M02 — Surface, vegetation, adjacency, and replacement proof

Prove the system fits the supplied map grammar. Add explicit surface overrides, non-walkable
projectile-pass-through water, walkable vegetation, deterministic replacement outcomes,
normal/destroyed presentation, and adjacency-aware or generated surface/feature rendering on top of
M01's dynamic generation/revision, bounded live outcomes, snapshot recovery, and restart behavior.

M02 research found that per-observer fighter visibility alone is insufficient: projectiles,
deployables, combat cues, audio, spectators, and reconnect can still leak a hidden subject. M02
therefore uses honestly named, non-concealing `TALL_GRASS`; `BUSH/HideOccupants` does not ship.
Complete concealment is deferred to `V8-CONCEALMENT` as a dedicated observer-privacy slice. The
catalog may not promise a property the runtime does not enforce.

The proof map is original and deliberately small. It exercises irregular holes and boundaries,
water, vegetation, walls/corners, multi-cell features, a default surface, decorations, spawns, and
explicit Wipeout spawns. M03 owns the real Hot Zone anchor conversion. Spawn parameters already
prove the typed parameter seam, so teleport remains absent from the production catalog.

Gate:

- irregular fields are authored as sparse assets rather than rectangles, circles, region profiles,
  or imported map images;
- water, vegetation, walls, and default ground have distinct catalog identities and explicit legal
  cell combinations;
- authoritative player/projectile collision matches the profile matrix;
- destruction applies deterministically, publishes bounded exact state, repairs collision, resets,
  and recovers after gaps/reconnect;
- normal, destroyed/replacement, imported, generated, and fallback presentation remain subordinate
  to authoritative state;
- no concealment identity or behavior ships; tall grass remains honestly non-concealing;
- fixed-tick, collider, event, recovery-byte, render, and memory ceilings pass;
- the user accepts that the proof demonstrates the target map grammar.

### M03 — Complete built-in and catalog conversion

Convert Crossroads Hot Zone and Ashen Court. Re-author every existing map placement on integer
cells, convert existing destructible reservations to explicit assets, convert team spawns to marker
assets, convert area objectives to grid-owned typed anchors, and preserve mode/topology requirements.
Replace the current shared object/variant/theme catalogs and client `environment_visuals.ron` with
the new shared map-asset/gameplay catalog and client visual/theme catalog. Update the shipped asset
manifest only where paths or promoted files actually change.

M03 also updates the built-in map index, catalog and recipe fingerprints, routed lobby/worker
manifest revisions, client acceptance/readiness, diagnostics, map selection metadata, performance
fixtures, integration harnesses, native automation, root README/current specs, and any Balance Lab
or profile surface that consumes map identity. Exact current consumers must be re-audited during
M03 research rather than inferred from this roadmap.

Gate:

- every built-in is a new-schema document and resolves only through the new catalog;
- Wipeout and Hot Zone preserve legal 1v1, 2v2, and 3v3 topology, safe spawns, reachability, objective
  access, scoring, results, and requeue behavior;
- both current themes and every retained environment asset resolve through stable new visual IDs;
- no map identity branch changes gameplay or presentation;
- imported/fallback native comparisons are accepted for Crossroads and Ashen Court;
- map pools, admission, manifests, snapshots, recovery, and reconnect use only V8 content identity;
- no production source document remains in the old schema, although old implementation code awaits
  M04 deletion until the converted suite is green.

### M04 — Legacy eradication, hardening, and closeout

Delete the superseded system rather than deprecating it. Remove its source modules/types, content
files, asset catalog, loaders, serializers, schema constants, feature gates, fixtures, test helpers,
diagnostics, logs, docs vocabulary, re-exports, generated build inputs, and fallback branches.
Rename or relocate any reusable algorithm into its actual V8 owner and test it only through the new
contract.

The removal audit must explicitly search for retired symbols and paths named in the canonical
specification. `content/v4` must no longer be a production input; `assets/catalogs/environment_visuals.ron`
must be gone; the old `MapObject*`, `MapVisualVariant*`, region-profile, collision-profile,
entity-definition, destructible-reservation, floating placement, and dual-resolver vocabulary must
have zero production matches. Clean builds must regenerate successfully after old generated
artifacts are absent. Historical V4 documents remain unchanged and are excluded from the
production-zero-match assertion.

Gate:

- the legacy-removal inventory reaches zero without compatibility aliases or dead code;
- all role-specific check/test/Clippy commands and canonical routed 1v1/2v2/3v3 E2E pass;
- repeated install/replacement/restart/reconnect/completion/requeue campaigns show bounded entity,
  mesh, collider, queue, cache, and memory ownership;
- current maps plus the proof map pass primitive and imported native render/performance evidence;
- the user playtests every converted built-in and the proof grammar, and every feedback item is
  implemented, deferred, rejected with rationale, or marked as needing evidence;
- current specifications, repository orientation, asset/catalog documentation, and commands teach
  only the V8 system;
- the learn-from-errors review records migration mistakes, their causes and prevention, and any
  genuinely reusable skill improvement;
- V8 is marked `Complete` only after the user accepts the closeout.

## Target production layout

The exact module split remains subject to milestone evidence, but V8 closes around one ownership
model:

```text
content/
  catalogs/
    builds.ron                   shared build definitions
    weapons.ron                  shared weapon definitions
    weapon_parts.ron             shared weapon-part definitions
    map_assets.ron               shared placeable definitions and gameplay-profile references
    map_gameplay_profiles.ron    bounded headless-safe map properties
    map_presentation_themes.ron  stable shared map-theme identities
  maps/
    index.ron
    builtin/<map-key>.ron        sparse grid recipes

assets/catalogs/
  map_asset_visuals.ron          client-only scenes/generated visuals/transforms/fallbacks
  map_presentation_themes.ron    client-only lighting/material/outer-world profiles

src/map/
  model/catalog/recipe/server/runtime/client ownership described by the V8 specification

src/client/presentation_3d/map/
  client catalog loading, materialization, generated/adjacent meshes, and lifecycle
```

`assets/manifest.ron` remains the authoritative shipped-file provenance inventory. It does not
become a map catalog and does not contain gameplay properties.

## Migration inventory

M01 must turn this table into an exact path/symbol checklist and keep it current through M04.

| Current concern | V8 owner | Final disposition |
|---|---|---|
| `content/v4/map_objects.ron` | shared `map_assets.ron` plus gameplay profiles | Convert referenced definitions, then delete |
| `content/v4/map_visual_variants.ron` | shared visual references plus client visual profiles | Convert referenced compatibility/fitting facts, then delete |
| `content/v4/map_themes.ron` | shared/client V8 theme catalogs | Convert both themes, then delete |
| `content/v4/map_definitions.ron` | V8 catalog policy and mode layout validation | Convert supported definitions/policies, then delete |
| `content/v4/maps/**` | `content/maps/**` | Re-author all recipes; no runtime decoder |
| active `content/v1`, `content/v7`, and `content/v8` files | `content/catalogs/**` and `content/maps/**` | Move byte-for-byte, switch direct loaders, then delete every production version directory |
| `assets/catalogs/environment_visuals.ron` | V8 client visual/theme catalogs | Convert active profiles, then delete |
| map object role/binding resolver | map-asset placement/gameplay resolver | Replace and delete |
| region/destructible reservation path | explicit placed assets and map runtime state | Replace and delete |
| generated playable ground | default/override surface assets | Replace and delete as an authoring substitute |
| separate spawn areas/points | parameterized spawn marker assets and derived indexes | Convert and delete recipe shapes |
| floating object placements/bounds | integer dimensions/cells/quarter-turns | Re-author and delete wire/source shapes |
| terrain convergence/recovery | V8 dynamic map-asset generation/revision recovery | Re-express through new IDs/state; no forwarding facade |
| environment visual loader/presenter | V8 client map presentation owner | Replace names/types/catalog, then delete old owner |
| map tests/network/performance fixtures | V8 recipe/catalog/runtime fixtures | Convert; remove old helpers and snapshots |
| current map/environment/art docs | V8 canonical specifications | Reconcile after accepted implementation |

## Verification matrix

Every milestone selects the proportional subset; M04 runs the complete matrix using canonical
commands from the root `justfile` and README.

| Boundary | Required evidence |
|---|---|
| Catalog/schema | deterministic parsing, references, profile contradictions, visual coverage, unknown/duplicate/oversized rejection |
| Grid | coordinate/world conversion, rotated multi-cell footprints, bounds, canonical order, conflicts, defaults, surface compatibility |
| Authority | player/projectile collision, destruction/replacement, spawn safety, interaction behavior actually shipped |
| Networking | current global compatibility, snapshot identity, live revision order, duplicate/gap recovery, late join, restart, reconnect |
| Modes | Wipeout and Hot Zone topology, objective access/occupancy, results, replay/requeue |
| Roles | server feature graph excludes window/render/audio/device input/client assets; client derives presentation only |
| Presentation | imported/generated/fallback surfaces/features, adjacency, normal/destroyed state, objective readability, cleanup |
| Capacity | recipe/snapshot/recovery bytes, cell/placement/collider limits, fixed tick, loading/render time, memory |
| Lifecycle | repeated map install/replacement, restart, reconnect, match exit/completion, worker shutdown |
| Removal | repository search, deleted old files, no adapters/aliases, clean regeneration/build without stale outputs |

## Explicitly outside V8

- a player-facing editor and its undo/selection/save UX;
- custom-map upload, distribution, server provisioning, caching, publishing, discovery, moderation,
  ratings, or monetization;
- arbitrary user assets, behavior definitions, scripts, shaders, or mode rules;
- a complete inventory/reward implementation for chests;
- teleports, launch pads, healing pads, hazards, or other interactions unless one is explicitly
  accepted into M02 after its focused technical specification;
- procedural map generation or automatic conversion of reference images into maps;
- vertical traversal or 3D authoritative physics;
- broad original-art replacement beyond assets needed to prove the V8 grammar.

## Initial V8 backlog

| ID | Item | Disposition |
|---|---|---|
| V8-EDITOR | Player-facing grid editor over the accepted recipe/catalog | Deferred to `CAND-MAP-BUILDER`; V8 establishes its one storage/runtime target only |
| V8-PROVISIONING | Server-selected custom map bundle delivery/caching | Deferred to `CAND-MAP-PROVISIONING` |
| V8-INTERACTIONS | Teleport, chest, healing, launcher, and hazard families | Promote individually only with complete authoritative behavior; optional one-interaction M02 proof requires specification review |
| V8-AUTOTILE | General author-controlled autotile rules | V8 may implement bounded client adjacency for demonstrated walls/vegetation; no general rule language |
| V8-CONCEALMENT | Server-owned `BUSH/HideOccupants` observer privacy | Deferred by M02 research: promote only as a dedicated slice covering fighter/projectile/deployable/cue/audio/spectator visibility, reveal transitions, reconnect, and leakage evidence; visual-only hiding is rejected |
| V8-IMAGE-IMPORT | Convert a drawing or bitmap into authoritative cells | Rejected as a runtime format; a future editor-side assisted tracing tool may emit an ordinary validated recipe |
| V8-COMPAT | Load or migrate V4 recipes at runtime | Rejected; built-ins are hard-converted and the current schema has no compatibility decoder |
