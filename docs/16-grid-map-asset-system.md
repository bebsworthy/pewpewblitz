# Grid map-asset system specification

## Decision status

`Implemented`

This document is the canonical specification for Brawler's sparse-grid map-asset system. It is
based on the five map drawings under `external_assets/map_images/` and the accepted requirement
that one author-facing map asset joins placement, gameplay, and presentation meaning without
making client assets authoritative.

The [V8 roadmap](./implementation/v8/roadmap.md) records implementation and verification history.
Historical milestone documents are evidence, not production content organization.

## Current repository ownership

- [`content/catalogs/`](../content/catalogs/) contains headless-safe build, weapon, map-asset,
  gameplay-profile, and presentation-theme definitions.
- [`content/maps/`](../content/maps/) contains the built-in index and one independent sparse recipe
  per built-in map.
- [`assets/catalogs/`](../assets/catalogs/) maps stable visual IDs to client-only scenes,
  transforms, and materials; shared gameplay content never contains client asset paths.
- [`src/map/catalog.rs`](../src/map/catalog.rs) parses, validates, fingerprints, and resolves map
  content. [`src/map/runtime.rs`](../src/map/runtime.rs) owns authoritative whole-asset destruction,
  recovery, and colliders. [`src/map/client.rs`](../src/map/client.rs) owns client convergence and
  readiness.
- Destructibility is a gameplay-profile property of a map asset. Destruction changes or removes
  the complete authored placement, preserving the accepted coarse 32-world-unit readability.
- [`external_assets/map_images/`](../external_assets/map_images/) remains ignored design-reference
  material and is not a runtime asset directory.

## Product outcome

A map author works with a bounded two-dimensional grid and a catalog of approved map assets:

```text
WALL
WALL_CORNER
GRASS
BUSH
WATER
SAND_FLOOR
CHEST
TELEPORT
DECOR_SEASHELL
PLAYER_SPAWN
```

The author chooses an asset and places it in a cell. The asset definition supplies its footprint,
legal rotation, placement slot, gameplay behavior, normal visual profile, and optional destroyed
visual profile. The recipe supplies only instance facts such as cell, rotation, team, facing, or
teleport channel. Neither a GLB path nor arbitrary executable behavior appears in a map recipe.

The target authoring model is:

```text
MapAssetDefinition
  = stable author-facing identity
  + placement contract
  + server-known gameplay profile
  + stable client visual references

MapRecipe
  = dimensions and metadata
  + default surface and presentation theme
  + sparse MapAssetPlacement values
  + mode identity and mode-owned anchors
```

Built-in maps and future player-authored maps use the same schema, catalog lookup, validation,
resolution, authoritative installation, client presentation, and fingerprint path.

## Goals

- Make a sparse grid of catalogued map assets the only production map-authoring representation.
- Make `GRASS`, `WATER`, `WALL`, and similar stable assets the nouns authors place.
- Model blocking, projectile interaction, concealment, destructibility, and interaction as explicit
  gameplay properties or bounded behavior references, not as competing object categories.
- Preserve one clear authoring concept while keeping the headless server independent from client
  files, Bevy rendering, scenes, materials, and asset handles.
- Support the irregular orthogonal water, vegetation, wall, decoration, spawn, and objective layouts
  demonstrated by the reference drawings without decomposing them into many rectangles.
- Preserve server authority, deterministic resolution, bounded work and memory, stable wire
  identity, match restart, reconnect recovery, map replacement, and routed admission.
- Convert every existing built-in map and promoted environment asset to the new representation.
- End V8 with no production fallback or decoder for the superseded representation.

## Non-goals

- A player-facing map editor, publishing, discovery, moderation, collaboration, or asset upload.
- Arbitrary scripts, user-defined gameplay properties, user-defined behavior profiles, or dynamic
  component reflection in map files.
- Procedural generation of playable layouts.
- Vertical gameplay, stacked floors, ramps, jumping, or 3D authoritative collision.
- Implementing every plausible asset named in this document. A catalog entry is legal only when its
  complete authoritative behavior and presentation lifecycle exist.
- Treating the reference drawings or imported images as collision, map, or runtime content.
- A compatibility decoder for V4 map documents. Built-ins are authored again in the new schema.

## Core vocabulary

### Map cell

One addressable authoring square. V8 uses a fixed `MAP_CELL_SIZE_WORLD` of 32 world units. A recipe
stores integer width and height, not floating-point playable bounds. The authoritative playable
rectangle is derived from those dimensions and centered on world origin:

```text
map_world_size = (width, height) * 32
map_min = -map_world_size / 2
cell_min(x, y) = map_min + (x, y) * 32
cell_center(x, y) = cell_min(x, y) + (16, 16)
```

`x` increases with gameplay world `x`; `y` increases with gameplay world `y`. Client 3D
presentation retains the established planar conversion to ground-plane `x/-z`. Editor screen-row
orientation is an editor concern and never changes stored coordinates.

Thirty-two units losslessly expresses current built-in bounds and rectangular placement edges.
Visual modules may span several cells. A future format revision may change the global constant only
through a deliberate migration and fingerprint/protocol review; individual maps do not choose
incompatible cell sizes.

It does not preserve every old floating point placement as a cell center. In particular, point
spawns and decorations on an even-sized map may currently sit on grid intersections. Conversion
re-authors those points to reviewed nearby cells and verifies clearance, symmetry and presentation;
V8 does not add arbitrary offsets or a second node-coordinate grammar to claim false losslessness.

### Map asset

A stable, server-known, author-facing thing that may be placed on a map. A map asset is not a GLB,
texture, or Bevy asset. It joins one placement contract, one gameplay profile, and stable visual
profile references.

Examples include a sand surface, blocking stone wall, walkable concealing bush, non-walkable water,
inert seashell, team spawn marker, or implemented teleporter.

### Gameplay profile

A developer-authored, shared, headless-safe definition of how an asset behaves. Profiles exist to
reuse exact behavior and keep properties bounded; map authors select a map asset and cannot modify
profile fields.

### Visual profile

A client-only definition mapping a stable visual ID to an imported scene or generated presentation,
including scale, yaw correction, vertical offset, tint/material policy, footprint agreement,
fallback, and optional animation or adjacency information. Visual readiness never changes gameplay.

### Placement

One instance of a `MapAssetId` at an integer cell with a legal quarter-turn rotation and, only when
the asset requires it, a small typed parameter value.

## Cell composition and conflicts

A cell is not an unrestricted stack. It has bounded slots:

```text
Cell
  surface: exactly one, usually inherited from the map default
  feature: zero or one physical/gameplay feature
  decoration: zero or one explicitly non-gameplay decoration
  markers: bounded mode/session annotations
```

The rules are:

- `Surface` assets replace the default surface in their covered cells. Examples: sand, stone floor,
  water.
- `Feature` assets occupy physical/gameplay space above a surface. Examples: wall, bush, chest,
  teleporter. Two features cannot occupy the same cell unless one future concrete asset explicitly
  owns a compound footprint; V8 does not allow arbitrary feature stacking.
- `Decoration` assets have no authoritative collision or gameplay effect. Their declared visual
  envelope must fit without obscuring required combat information. A decoration cannot be used to
  imply a wall, bush, pickup, or interactable.
- `Marker` assets expose server-known layout meaning such as a player spawn. Their typed parameters
  are validated, and they cannot occupy cells that make the marker unusable.
- An area objective remains a mode-owned anchor because it spans cells and its mode owns scoring.
  It uses grid coordinates and bounded cell/radius dimensions but is not disguised as a generic
  environment effect.

Examples:

```text
SAND_FLOOR + BUSH                 valid
SAND_FLOOR + WALL                 valid
SAND_FLOOR + WALL + BUSH          invalid
WATER + WALL                      invalid
WATER + BRIDGE                    invalid in V8; a future bridge needs an explicit compound rule
SAND_FLOOR + PLAYER_SPAWN         valid when fighter clearance passes
WATER + PLAYER_SPAWN              invalid
```

Meaningful combinations such as a flowering bush, torch wall, bridge, or destroyed wall are
catalogued assets or states with one validated lifecycle, not arbitrary stacks assembled by maps.

## Shared catalog contract

The shared catalog is compiled into every role and contains no client asset path. Conceptually:

```rust
struct MapAssetDefinition {
    id: MapAssetId,
    key: String,
    display_name: String,
    slot: MapAssetSlot,
    gameplay_profile_id: MapGameplayProfileId,
    visual_profile_id: MapVisualProfileId,
    footprint_cells: UVec2,
    allowed_quarter_turns: u8,
    allowed_surface_tags: Vec<MapSurfaceTagId>,
    parameter_kind: MapPlacementParameterKind,
}
```

`allowed_quarter_turns` is a four-bit mask for rotations `0`, `90`, `180`, and `270` degrees.
Rotation swaps the footprint axes. Placement uses the minimum occupied cell after rotation; runtime
world position and authoritative footprint are derived, never authored as floating-point values.

The first gameplay profile shape is explicit rather than a generic property map:

```rust
struct MapGameplayProfile {
    id: MapGameplayProfileId,
    player_collision: PlayerCollision,
    projectile_collision: ProjectileCollision,
    destruction: MapDestructionBehavior,
    interaction: MapInteractionBehavior,
    concealment: MapConcealmentBehavior,
}
```

The values are bounded enums with implemented semantics. V8 M02 deliberately omitted concealment;
completed V9 added `MapConcealmentBehavior` through the full authoritative concealment slice and
revised the schema/catalog/content identity atomically. Unknown variants fail catalog loading.
The catalog validator rejects contradictory combinations. In particular:

- walkability is derived from `player_collision`; there is no second `walkable` boolean that can
  disagree;
- an asset cannot hide occupants while blocking all player entry;
- a decoration profile cannot collide, conceal, destruct, spawn, teleport, or otherwise affect
  gameplay;
- a destructible asset declares exactly what authoritative state replaces it;
- an interaction requiring parameters declares their exact typed parameter kind;
- visual profiles cannot change any of these facts.

### Collision

Initial supported values are:

```text
PlayerCollision:     Pass | Block
ProjectileCollision: Pass | BlockAndConsume
```

Additional responses such as reflection or conditional team passage require their own gameplay
milestone and enum variant; maps cannot synthesize them from flags.

### Destruction

Initial supported values are:

```text
Indestructible
RemoveOnMapDestruction
ReplaceOnMapDestruction(MapAssetId)
```

Destruction is server-owned and M02 accepts only the existing bounded authoritative `DestroyMap`
world effect; it does not add durability or direct weapon damage to map assets. A replacement must
be catalog-compatible with every covered cell and
must not create a feature where one already exists. A cleared bush commonly uses `Remove` and
exposes the existing surface; a broken obstacle may use `Replace` with a nonblocking destroyed
feature. The replacement asset owns presentation after replacement commits.

V8 retains bounded area-destruction delivery by lowering accepted attacks into deterministic cell
candidate sets. It does not retain a second public 8-unit map-authoring grid. If sub-cell collision
work remains useful internally during migration, it is an implementation detail and cannot appear
in recipes, shared IDs, recovery schemas, or authoring terminology at V8 closeout.

The proposed V10 [damageable world objects and Heist specification](./18-damageable-world-objects-and-heist.md)
adds ordinary attack durability as a separate profile/runtime axis. It does not reinterpret these
V8 destruction variants: a V10 hit-point placement is immune to `DestroyMap`, carries durable
partial health, and commits one compatible terminal placement outcome only when its health reaches
zero.

### Concealment

Initial values are `None` and one fully specified `HideOccupants` behavior. `HideOccupants` means
the server owns whether an observer may receive or present a subject. Its specification must cover
allies, nearby enemies, attacks, damage, objective carrying, projectiles, audio cues, spectators,
reveal timing, reconnect, and transitions across adjacent cells. A client-only opacity effect is not
accepted as gameplay concealment.

V8 M02 selected the safe historical branch: it shipped non-concealing `TALL_GRASS` and omitted both
`BUSH` and `HideOccupants` because the full privacy boundary did not yet exist. V9 deliberately
changed `TALL_GRASS` to reference an explicit `HideOccupants` gameplay profile and implemented the
required per-observer Lightyear visibility, cue/audio filtering, sentry perception, lifecycle,
late-join, reconnect, and recovery rules. New concealing assets must use that contract; a schema
entry may never claim to hide players while ordinary replicated state leaks the subject.

### Interactions and parameters

Initial structural values are:

```text
None
PlayerSpawn
Teleporter
```

Only behavior completed by an accepted milestone may be present in the production catalog. Spawn placement
parameters contain team slot, stable spawn ordinal, and facing quarter-turn. Teleporter placement
parameters contain a bounded channel/link identity and exit facing policy. A teleporter behavior is
not complete until authoritative entry, destination choice, cooldown/re-entry protection, collision
repair, replication, reset, and presentation are specified and tested.

Chests, healing pads, launchers, pickups, and hazards follow the same rule: the asset model can host
them later, but names in an example do not create unsupported runtime behavior. V10 has now
selected one exact treasure-chest/restoration-pickup behavior, documented separately and still
unsupported until the gated [V10 roadmap](./implementation/v10/roadmap.md) is implemented and
accepted; this does not promote the other interaction families.

## Client visual catalog

The client catalog is keyed by `MapVisualProfileId` and owns presentation only:

```rust
struct MapVisualProfile {
    id: MapVisualProfileId,
    source: None | ImportedScene | GeneratedSurface | GeneratedFeature | GeneratedMarker,
    asset_path: Option<String>,
    scale: Vec3,
    yaw_degrees: f32,
    vertical_offset: f32,
    material: MapMaterialProfile,
    fitting: Exact | Tiled | Contained,
    fallback: MapVisualFallback,
    adjacency: Option<MapAdjacencyProfile>,
}
```

- Imported paths remain under `assets/` and appear in `assets/manifest.ron` with provenance and
  licensing.
- The headless server neither parses this catalog nor includes its files through a server feature.
- Every shared visual reference must exist in the client catalog; unused client profiles fail the
  closeout audit unless retained by another explicit presentation owner.
- A visual footprint must agree with or fit inside the authoritative cell footprint according to
  its fitting policy.
- Adjacency may select straight, inner-corner, outer-corner, end-cap, or isolated presentation for
  neighboring instances of the same adjacency group. It never changes collision or gameplay.
- Authors may still place an explicit corner asset when it has distinct semantics or footprint.
  Purely visual wall/grass edge selection should normally be derived by adjacency presentation.
- Missing or failed imported assets use a deterministic primitive/generated fallback with the same
  authoritative footprint and readable gameplay meaning.

Normal and destroyed visuals are stable references on the shared asset definition. Transient hit,
break, splash, conceal/reveal, and teleport effects are client presentation facts emitted from
authoritative outcomes; they are not map recipe fields.

## Theme and defaults

A map recipe selects:

```text
default_surface_asset_id
presentation_theme_id
```

The default surface supplies the gameplay and ordinary surface visual for every cell without an
explicit surface placement. The presentation theme supplies map-wide client choices such as:

- ambient and directional lighting;
- outer-world surface and perimeter presentation;
- calm palette/material adjustments;
- optional compatible visual defaults for deliberately theme-relative assets;
- bounded outside dressing.

A theme cannot change collision, projectile blocking, concealment, destruction, interaction,
footprints, or legal placement. Ground gameplay never comes from a color or shader.

## Map recipe schema

Conceptual source form:

```ron
(
    recipe_id: MapRecipeId(4),
    revision: 1,
    recipe_version: 1,
    name: "Acid Lakes",
    mode_definition_id: WIPEOUT,
    presentation_theme_id: DESERT_DAY,
    dimensions: (width: 60, height: 60),
    default_surface_asset_id: SAND_FLOOR,

    placements: [
        (placement_id: 1, cell: (4, 7), asset: WALL_CORNER, rotation: 0),
        (placement_id: 2, cell: (5, 7), asset: WALL, rotation: 0),
        (placement_id: 3, cell: (12, 9), asset: WATER, rotation: 0),
        (placement_id: 4, cell: (18, 14), asset: BUSH, rotation: 0),
        (placement_id: 5, cell: (22, 8), asset: CHEST, rotation: 0),
        (
            placement_id: 6,
            cell: (3, 20),
            asset: PLAYER_SPAWN,
            rotation: 0,
            parameters: PlayerSpawn(team: 0, ordinal: 1, facing: East),
        ),
    ],

    mode_anchors: [],
)
```

Actual RON uses stable numeric newtypes plus catalog keys for diagnostics. The serialized wire form
does not depend on Rust enum debug names.

Placements are canonicalized by occupied minimum cell, slot, stable asset ID, and placement ID.
Every placement has a globally unique nonzero ID. Source convenience syntax may support row spans
or filled rectangles, but the canonical resolved recipe expands them into bounded placements before
fingerprinting. There is one parser and canonical representation; a convenience construct does not
create another runtime path.

Mode anchors use integer cell coordinates and bounded cell dimensions. Team spawn areas disappear;
spawn capacity comes from placed spawn assets. An area mode anchor remains a dedicated typed layout
fact because the selected mode owns its meaning and scoring.

## Resolution and validation

The headless resolver performs one complete transaction and returns no partial map. Validation
covers:

- supported schema, catalog, mode, and layout versions;
- nonzero stable IDs, unique recipe/placement/marker identities, and bounded display metadata;
- dimension, total-cell, placement, footprint, generated-instance, collider, runtime-state,
  serialized-byte, and recovery-byte ceilings;
- in-bounds rotated footprints without integer overflow;
- exactly one effective surface per cell;
- feature, decoration, and marker slot conflicts across multi-cell footprints;
- allowed-surface constraints and replacement compatibility;
- exact typed placement parameters;
- collision, projectile, concealment, destruction, and interaction profile consistency;
- spawn count, team topology, capacity, clearance, and terrain-aware reachability;
- required mode anchors, objective access, and mode compatibility;
- deterministic canonical order and fingerprints;
- client visual coverage through a build/test-time cross-catalog audit, without loading client files
  in the server runtime.

Validation derives immutable per-cell facts and groups them for efficient runtime installation. The
runtime is not required to spawn one ECS entity per map cell. Contiguous static blockers may become
merged Avian colliders; repeated visuals may become instanced or generated chunk meshes; sparse
interactive assets retain stable runtime owners where their lifecycle needs them.

## Resolved snapshot and networking

The resolved map snapshot carries only stable shared facts:

```text
map identity and recipe fingerprint
catalog/layout schema and content fingerprints
dimensions and derived world bounds
default surface asset
canonical resolved placements
mode anchors and derived spawn index
initial dynamic map-asset generation identity
```

It carries no GLB path, material, mesh, scene, Bevy handle, or process-local entity. The V8 cutover
uses the one global application compatibility handshake and current schema; there is no per-message
version or legacy decoder.

Clients validate the snapshot and referenced shared catalog fingerprint before declaring map
readiness. Dynamic destructible or interactive state uses generation plus monotonic revision.
Ordered live outcomes update exact placements/cells. A gap or generation mismatch requests one
bounded authoritative current-state snapshot. Restart restores authored initial state under a new
match generation. Map replacement and teardown remove every old collider, runtime entity, queued
mutation, recovery cache, visual entity, generated mesh, and transient effect.

## ECS and schedule ownership

The final source organization should reflect the new responsibility rather than preserve old file
names:

```text
src/map/
  mod.rs                 composition, sets, public API
  model.rs               shared grid, IDs, recipe and resolved shapes
  catalog/               shared map-asset/gameplay definitions and validation
  recipe/                parsing, canonicalization, resolution and layout validation
  server/                authoritative installation, indexes, reset and teardown
  runtime/               dynamic asset state, destruction/interactions and recovery
  client/                snapshot acceptance and client convergence
  tests.rs

src/client/presentation_3d/map/
  catalog.rs             client visual/theme profiles and asset readiness
  materialize.rs         grid surfaces, features, decorations and markers
  generated.rs           adjacency/chunk/generated mesh ownership
  tests.rs
```

Exact filenames may change when implementation evidence demonstrates a clearer cohesive boundary.
The final design must not preserve `terrain/`, `regions`, or `objects` merely as forwarding layers
for deleted concepts.

Authoritative fixed-tick ordering remains explicit:

```text
input/combat intent
  -> combat delivery and accepted map effects
  -> bounded map-runtime transaction planning
  -> cell/placement state commit
  -> collider rebuild/reconciliation
  -> fighter embedding repair where required
  -> revision/outcome publication
  -> physics step observes committed state
```

Concealment observation and interactive behaviors receive their own explicit ordering in the
milestone that implements them; presentation never becomes an authority dependency.

## Existing content conversion

V8 converts, rather than compatibility-loads:

- Crossroads Facility Wipeout;
- Crossroads Facility Hot Zone;
- Ashen Court;
- every shared map object and visual actually referenced after conversion;
- both current presentation themes;
- the client map-asset visual catalogs and asset readiness path;
- generated ground, perimeter, decorations, obstacles, spawns, anchors, and destructible state;
- map fingerprints, routed admission revisions, network fixtures, performance fixtures, and native
  playtest commands.

Existing permanent rectangles become bounded cell footprints. Existing circular trees, rocks, and
columns become catalogued feature assets with cell placement and an authoritative collision shape
owned by their gameplay profile; the grid does not require every collider to fill a square.
Existing destructible reservations are repainted as explicit map-asset cells. Presentation should
preserve accepted map identity and readability unless a V8 milestone explicitly presents and gains
acceptance for a visual or gameplay correction.

Crossroads Facility is the first such deliberate correction: its initial center-cover envelope
remains exact, while destruction resolves at the public 32-unit asset-cell granularity instead of
the retired 8-unit terrain occupancy. M01 playtesting accepted this as the baseline because whole
asset-cell removal is easier to read in combat and cannot leave tiny collision specks that still
block a player. Later V8 work must preserve this 32-unit destruction granularity unless a new
specification review deliberately changes the public grid contract.

The ignored drawings under `external_assets/map_images/` are design references only. V8 uses at
least one deliberately small reference-inspired proof layout to demonstrate irregular water,
vegetation, corners, default ground, and sparse authoring; it does not copy the drawings as shipped
maps or claim their art/layout as original content.

## Removal invariant

The completed cutover enforces that the following production concepts remain absent:

- `MapObjectDefinitionId`, `MapVisualVariantId`, `CollisionProfileId`, `RegionProfileId`, and
  `EntityDefinitionId` where they exist only for the superseded map model;
- `MapObjectRole`, `MapObjectPlacementBinding`, `MapObjectPlacement`, `GeometryPlacement`,
  `MapEntityPlacement`, `MapRegionPlacement`, and `VisualPlacement`;
- the destructible-reservation profile and region-to-terrain rasterizer;
- authored floating-point map bounds and object positions;
- generated-only playable ground as a substitute for authored surfaces;
- separate spawn-area recipes and floating-point spawn-point recipes;
- `content/v4` production loading and its object, variant, theme, definition, and map recipe schemas;
- `assets/catalogs/environment_visuals.ron` and its `EnvironmentVisual*` production types;
- dual resolver, dual presenter, fallback-to-V4, migration-at-runtime, and V4 snapshot decoding;
- active docs, examples, tests, scripts, logs, diagnostics, and error messages that teach the old
  authoring vocabulary.

Reusable algorithms may be carried forward only after being expressed through new owners and new
tests. Historical V4 milestone documents are retained unchanged as implementation evidence and are
not production traces.

[`scripts/check-v8-map-cleanup.sh`](../scripts/check-v8-map-cleanup.sh) enforces the retired-path and
retired-symbol subset as part of `just lint`.

## Verification contract

V8 verification must include:

- pure coordinate, rotation, footprint, canonical ordering, conflict, surface, replacement, and
  fingerprint tests, including negative/overflow-adjacent conversion inputs where applicable;
- catalog cross-reference and client visual coverage tests;
- representative maps with irregular water and vegetation, holes, corners, multi-cell features,
  and sparse default ground;
- collision tests distinguishing player and projectile behavior;
- destruction transaction, replacement, restart, duplicate/gap, late join, and recovery tests;
- completed V9 concealment privacy, lifecycle, cue-filtering, and recovery tests for every hiding
  asset;
- interaction lifecycle tests before any interactive asset ships;
- spawn safety, reachability, objective accessibility, topology, and map-capacity tests;
- separate-App authority/replication tests and routed 1v1, 2v2, and 3v3 Wipeout/Hot Zone E2E;
- client/server role-specific compilation proving no rendering or client asset dependency enters the
  server feature graph;
- primitive fallback and imported-asset native rendering for every converted built-in map;
- repeated map replacement, restart, reconnect, match completion/requeue, and teardown leak checks;
- measured fixed-tick, collider-build, recovery-size, loading, render, and memory bounds;
- a final legacy-removal search and clean-build proof with no generated artifact masking a removed
  source dependency.

## Accepted decisions

The accepted decisions are:

1. the fixed 32-world-unit authoring cell;
2. the `surface + optional feature + optional decoration + bounded markers` composition;
3. stable map assets joining placement/gameplay and stable visual references while client paths stay
   in a separate role-owned catalog;
4. property/profile semantics rather than role names such as `TerrainDestructible`;
5. dedicated typed mode anchors but spawn represented as a map asset placement;
6. full server-owned concealment as the requirement fulfilled by V9 before shipping hiding grass
   or bush behavior;
7. hard conversion with no V4 decoder or production compatibility period;
8. retention of historical milestone evidence despite zero old-system production traces.
