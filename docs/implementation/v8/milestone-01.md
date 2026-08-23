# V8 Milestone 01 — New catalog/grid vertical slice and first Crossroads conversion

## Status

`Complete`

Research and specification preparation were authorized on 2026-08-22. V7 completed and the user
approved this specification for implementation on 2026-08-23.

Implementation and automated verification completed on 2026-08-23. The native multi-window render
harness did not emit client reports in the Codex execution environment, but the user completed the
native playtest, accepted the converted map on 2026-08-23, and specifically preferred the 32-unit
destruction granularity for readability and reliable collision clearance.

## Player/developer-visible outcome

Crossroads Facility Wipeout is authored as integer dimensions, one default surface, sparse
`MapAssetId` placements, parameterized spawn markers, and no floating object/region/spawn recipe
arrays. A routed match resolves, installs authoritative collision, destroys placed center-cover
cells, presents imported or primitive-fallback visuals, restarts, reconnects, recovers, replaces or
tears down, and completes through the new map pipeline.

M01 proves the smallest complete player-visible slice without regressing the selected map:
walkable default ground, blocking walls, destructible blocking cover, inert decorations, player
spawns, generated perimeter presentation, and current routed map identity. It does not claim water,
vegetation, concealment, teleports, chests, replacement-state visuals, or a general interaction
framework. Those remain M02 or later behavior work.

## Research questions

1. Which current shared types, catalogs, build-generated inputs, protocols, admission manifests,
   runtime owners, presenters, diagnostics, and tests are required for Crossroads Wipeout?
2. Can a 32-world-unit cell and integer multi-cell footprint reproduce Crossroads bounds and
   collision exactly, and how should even-sized footprints derive world centers?
3. What is the smallest shared catalog representation that joins author-facing map assets to
   server gameplay profiles and stable client visual IDs without placing paths in the server graph?
4. Which Bevy ownership and schedule boundaries let one resolved grid install merged static Avian
   colliders while clients independently materialize tiled/imported visuals?
5. What map snapshot must cross Lightyear, and which data should be derived from the shared catalog
   rather than duplicated on the wire?
6. How can the build embed one V8 map index/document set deterministically while V4 remains a
   temporary implementation dependency for unconverted maps only?
7. Which canonical commands and focused tests prove the first vertical slice without prematurely
   implementing M02 dynamic state?
8. What exact temporary legacy inventory must M01 hand to M03/M04 so coexistence cannot become a
   permanent compatibility layer?

## Research findings

### Current production cutover surface

The current system is not only a recipe parser. The cutover crosses these owners:

- `build.rs` scans `content/v4/maps/builtin/*.ron`, sorts the filenames, and generates the embedded
  built-in byte table. V8 needs a second, explicitly named input during M01 and one V8-only input at
  closeout; stale generated output must not keep deleted V4 files alive.
- `src/content.rs` includes the canonical map-catalog material in
  `GameplayContentFingerprint`. A V8 catalog or recipe change therefore changes routed process
  identity as intended.
- `src/map/model.rs`, `src/map/objects.rs`, and `src/map/definitions/**` own the current floating
  recipe, role/binding catalogs, resolution, fingerprints, indexes, and validation.
- `src/map/server.rs` installs the replicated map root, perimeter and static Avian colliders;
  `src/map/client.rs` accepts the snapshot and controls map readiness.
- `src/terrain/**` separately owns the 8-unit destructible occupancy grid, brush transaction,
  collider rebuild, fighter repair, generation/revision recovery, client convergence, and chunk
  presentation.
- `src/client/presentation_3d/map.rs`, `border.rs`, and `environment_assets.rs` materialize current
  ground, rectangular/circular objects, borders, dressing, decorations, GLBs, and fallbacks from
  `assets/catalogs/environment_visuals.ron`.
- `src/protocol.rs` registers the map snapshot once and separately registers terrain destruction,
  reset, request, and recovery messages plus their channel.
- `src/server/lobby/catalog.rs`, `src/server/admission.rs`, `packages/brawler-routing`, and routed
  tests carry stable `map_preset` and `map_revision` through listing, allocation, worker manifests,
  and admission. These stable admission fields remain useful; their content changes, not their
  purpose.
- `src/matchplay/spawns.rs`, Wipeout/Hot Zone rules, diagnostics, network tests, performance tests,
  native automation, and render evidence consume resolved map facts and therefore require explicit
  migration ownership.

The exact M01 production inventory is:

| Concern | M01 action | Later deletion owner |
|---|---|---|
| `content/v4/maps/builtin/crossroads-facility.ron` | stop selecting it; add one V8 replacement with the same preset identity and a bumped admission revision | M04 deletes the unused source |
| remaining `content/v4/**` maps/catalogs | keep runnable through the isolated old resolver | M03 converts; M04 deletes |
| `assets/catalogs/environment_visuals.ron` | retain for old maps; add a V8 client catalog containing only Crossroads references | M03 converts remaining profiles; M04 deletes old catalog |
| old `ResolvedMapSnapshot` protocol component | retain only for unconverted maps | M04 deletes after M03 removes all producers |
| new `ResolvedGridMapSnapshot` | register once and use only for V8 maps | permanent; final name may become `ResolvedMapSnapshot` during M04 cleanup |
| `src/terrain/**` and terrain wire messages | retain only for unconverted maps | M04 deletes after new map runtime owns all dynamic maps |
| V8 map runtime/events/recovery | implement directly for Crossroads; do not adapt a V8 recipe into a `RegionProfileId` or terrain snapshot | permanent |
| combined map selection for lobby/admission | bounded temporary dispatcher by preset ID, never a schema decoder | M03 removes old entries; M04 collapses dispatcher |

Every temporary reference introduced or touched by M01 must contain `V8-MIGRATION(M03)` or
`V8-MIGRATION(M04)` and appear in a focused removal-audit test/list. This is bookkeeping, not a
runtime compatibility format.

### Grid fit and Crossroads conversion

Crossroads Facility bounds are `1792 x 1152`, exactly `56 x 36` 32-unit cells. Every current wall
edge and the center destructible region edge lies on this grid:

| Current placement | V8 minimum cell | Footprint |
|---|---:|---:|
| south horizontal wall | `(23, 9)` | `10 x 2` |
| north horizontal wall | `(23, 25)` | `10 x 2` |
| west inner vertical wall | `(15, 14)` | `2 x 8` |
| east inner vertical wall | `(39, 14)` | `2 x 8` |
| west outer vertical wall | `(9, 15)` | `2 x 6` |
| east outer vertical wall | `(45, 15)` | `2 x 6` |
| center destructible cover | `(25, 15)` through `(30, 20)` | 36 one-cell assets |

The initial wall and center-cover collision envelope can therefore remain exact. The current center
region becomes 36 explicit `DESTRUCTIBLE_COVER` placements, not one tagged rectangle and not 576
authored 8-unit terrain cells.

Point positions reveal one deliberate conversion cost: on an even-sized centered map, the old
coordinates are grid intersections while tile placements resolve to cell centers. V8 does not add
floating offsets or a second node coordinate system to preserve ornamental coordinates. Crossroads
is re-authored symmetrically to the nearest inward cell centers:

- team spawn `x` becomes `-752` / `752`; `y` becomes `-272`, `-80`, `80`, `272`;
- decorations move from `(±288, ±480)` to `(±272, ±464)`;
- facing becomes exact east/west quarter-turns.

This is at most 16 units on either axis. Spawn clearance, reachability, symmetry, camera framing,
combat timing, and visual composition must be tested and playtested; the conversion is not called
lossless for point placements.

The old Arc Launcher destroys radius-48 areas on an 8-unit internal grid. V8 lowers the same
accepted world-effect center and radius to sorted 32-unit authoring cells using circle-versus-cell
overlap, then removes touched destructible placements atomically. Initial cover extent is exact,
but destruction becomes intentionally tile-sized. This is the behavior the M01 playtest must judge.

### Pinned engine and network findings

- The project pins Bevy `0.19.1`, Lightyear `0.29.0`, and Avian 2D `0.7.0`. The checked-in Bevy
  reference is `0.20-dev` and the Avian source reference is newer than the pinned crate, so exact
  released APIs were checked against primary versioned documentation rather than copied from the
  snapshots.
- Bevy documents `FixedUpdate` as the schedule for gameplay, physics, networking, and game rules,
  while `FixedPostUpdate` reacts after fixed gameplay. That supports retaining Brawler's explicit
  fixed-post map-mutation transaction rather than moving it into render-frame `Update`.
- Lightyear 0.29 applies received replication in `PreUpdate`, sends replication in `PostUpdate`,
  and supports components whose insertion/removal is replicated once. The immutable resolved map
  remains a once-replicated component; mutable destruction remains ordered messages with explicit
  gap recovery rather than repeatedly replacing the full map component.
- Avian 0.7 supports static rectangle and compound colliders. M01 does not need a general collider
  optimizer: one collider per six wall placement, one per live destructible cell, and four perimeter
  colliders are bounded and preserve simple dynamic removal. A later capacity result may justify
  merging immutable adjacent placements.

Local sources inspected:

- `references/bevy/examples/README.md`, `app/headless.rs`, `app/plugin.rs`, and
  `app/plugin_group.rs`;
- `references/lightyear/examples/README.md`, `examples/simple_setup/README.md`, and
  `references/lightyear/book/src/concepts/{replication/protocol.md,replication/replicate.md,bevy_integration/system_order.md,bevy_integration/shared_plugin.md}`;
- `references/avian/crates/avian2d/examples` plus the production Avian usage in
  `src/map/server.rs` and `src/terrain/**`.

Primary released references:

- [Bevy 0.19.1 `FixedUpdate`](https://docs.rs/bevy/0.19.1/bevy/app/struct.FixedUpdate.html)
- [Lightyear 0.29 `ComponentRegistry`](https://docs.rs/lightyear/0.29.0/lightyear/prelude/struct.ComponentRegistry.html)
- [Avian 2D 0.7 `Collider`](https://docs.rs/avian2d/0.7.0/avian2d/collision/collider/struct.Collider.html)
- [Avian 2D 0.7 `RigidBody`](https://docs.rs/avian2d/0.7.0/avian2d/dynamics/rigid_body/enum.RigidBody.html)

### Alternatives rejected for M01

- A finer public authoring grid would merely preserve old internals and would make the intended
  tile vocabulary harder to author; 32 units already fits every structural edge.
- Node coordinates or per-placement visual/world offsets would create two coordinate grammars and
  weaken validation for a 16-unit point-placement difference.
- Converting the V8 center cover back into an old region would leave `DestructibleTerrain` as a
  disguised kind and fail to prove the new ownership.
- Deferring destruction would regress Crossroads and leave the first vertical slice unable to
  complete a representative match with its current combat behavior.
- A generic property bag, reflected component list, or scriptable behavior catalog would allow
  unsupported combinations and add server/runtime complexity without a demonstrated need.
- A single wire enum wrapping old and new recipes would become a compatibility format. Two
  temporarily distinct snapshot components make the migration boundary visible and deletable.
- General collider merging, autotiling, water, bush behavior, and interactions are not necessary to
  prove this slice.

## Decisions from research

1. M01 keeps the fixed 32-unit, origin-centered grid and minimum-occupied-cell placement rule from
   the version specification.
2. Crossroads Facility Wipeout keeps preset ID `1`; its admission revision increments from `4` to
   `5`. Recipe/catalog/schema and global gameplay-content fingerprints change normally.
3. The M01 shared asset set is exactly `GROUND`, two visually distinct immutable wall assets,
   `DESTRUCTIBLE_COVER`, four inert decoration assets, and `PLAYER_SPAWN`. Visual distinction is an
   asset/catalog choice, not a placement-level visual override.
4. `DESTRUCTIBLE_COVER` is a feature whose gameplay profile blocks players and projectiles and is
   removed by the implemented map-destruction world effect. Destructibility is a profile property,
   never a slot or kind.
5. Each center-cover cell is a separate placement with a stable ID. The source may use a bounded
   filled-span convenience form, but resolution expands it before validation, fingerprinting,
   networking, and runtime installation.
6. M01 implements the V8 dynamic map transaction and recovery directly. It does not publish
   `TerrainChunkId`, `TerrainBits`, `RegionId`, or 8-unit coordinates for the new map.
7. Point placements are manually shifted to symmetric nearest inward cell centers. No automatic V4
   converter ships and no arbitrary offset enters the schema.
8. Immutable resolved map state is once-replicated. Ordered dynamic outcomes carry generation and
   revision; a gap or generation mismatch requests a bounded current-state snapshot.
9. Stable routed preset/revision fields remain. Old and new content loaders coexist only as
   separately typed, annotated migration paths until all maps convert.
10. M01 uses direct bounded colliders rather than building a general merging subsystem.

## Technical specification

### Authored files and schemas

M01 adds:

```text
content/v8/map_gameplay_profiles.ron
content/v8/map_assets.ron
content/v8/map_presentation_themes.ron
content/v8/maps/index.ron
content/v8/maps/builtin/crossroads-facility.ron
assets/catalogs/map_asset_visuals.ron
assets/catalogs/map_presentation_themes.ron
```

All documents use `serde(deny_unknown_fields)`, an exact nonzero schema version, bounded byte and
entry counts, unique stable IDs/keys, and failure-before-install parsing. Shared files contain no
path, handle, mesh, material, color, or renderer type. Client catalogs contain no gameplay fact.

The M01 recipe's semantic shape is:

```rust
struct GridMapRecipe {
    recipe_id: MapRecipeId,
    revision: u16,
    schema_version: u16,
    mode_definition_id: ModeDefinitionId,
    presentation_theme_id: MapPresentationThemeId,
    dimensions: MapDimensions,              // 56 x 36
    default_surface_asset_id: MapAssetId,   // GROUND
    placements: Vec<MapAssetPlacement>,
    mode_anchors: Vec<GridModeAnchor>,       // empty for Wipeout
}

struct MapAssetPlacement {
    placement_id: MapPlacementId,
    cell: MapCell,                           // minimum occupied cell
    asset_id: MapAssetId,
    quarter_turns: u8,                       // 0..=3, checked by asset mask
    parameters: MapPlacementParameters,
}

enum MapPlacementParameters {
    None,
    PlayerSpawn { team_slot: u8, ordinal: u8, facing: QuarterTurn },
}
```

`MapCell`, dimensions, footprints, and intermediate multiplication use checked integers. World
bounds and centers are derived only after validation. Canonical order is `(y, x, slot, asset_id,
placement_id)`; source order never changes fingerprints or runtime outcomes.

The optional authoring convenience `FilledRect` contains a minimum cell, positive dimensions,
asset ID, rotation, and a starting placement ID. It is accepted only for a one-cell-footprint asset,
expands row-major with checked consecutive IDs, and disappears from the canonical recipe. It is
used for the 6-by-6 cover, not as a region or runtime shape.

### Shared catalog

The shared definitions follow `docs/16-grid-map-asset-system.md` with bounded enums:

```text
slot: Surface | Feature | Decoration | Marker
player collision: Pass | Block
projectile collision: Pass | BlockAndConsume
concealment: None
destruction: Indestructible | RemoveOnMapDestruction
interaction: None | PlayerSpawn
```

M01 does not serialize unsupported enum values for concealment, replacement, teleport, or chest
behavior. The Rust model may use the already accepted bounded `Destructible { ... outcome }`
contract, but production validation accepts only the implemented M01 subset.

Required consistency rules include:

- `Surface` is pass/pass, non-concealing, indestructible, and non-interactive;
- `Decoration` has the same non-gameplay profile and a one-cell visual envelope;
- a blocking feature cannot share its covered cells with another feature;
- `DESTRUCTIBLE_COVER` has a one-cell footprint, accepts only the default ground surface, and uses
  remove-on-map-destruction;
- `PLAYER_SPAWN` is a one-cell marker, requires exact spawn parameters, lies on passable surface,
  has fighter-radius clearance, and cannot overlap a blocking feature;
- all placed assets reference an existing shared visual ID; a build/test cross-catalog audit proves
  that each visual ID also exists in the client catalog without loading that catalog on a server.

### Resolution and derived indexes

One pure resolver parses, canonicalizes, expands conveniences, and validates the entire map before
returning `ResolvedGridMap`. It derives:

- origin-centered bounds, each placement's rotated footprint and world AABB/center;
- effective surface, feature, decoration, and marker occupancy per cell;
- player/projectile blocking facts;
- sorted static collider descriptors and sorted destructible placement descriptors;
- the spawn index consumed by `matchplay`;
- canonical fingerprint material and serialized-size checks;
- initial map-dynamic generation material.

The resolver caps dimensions, total cells, source constructs, expanded placements, generated visual
instances, colliders, spawn count, dynamic placements, snapshot bytes, event bytes, and recovery
bytes. Initial constants should be the smallest values covering all existing built-ins plus a
measured margin; M01 records actual Crossroads counts and fails tests at each ceiling. Arbitrary
large maps are not accepted merely because integers can represent them.

Spawn reachability uses the current representative navigation/grid test adapted to effective V8
player blocking. Spawn areas are not synthesized. The eight explicit markers alone provide 1v1,
2v2, and 3v3 capacity.

### ECS ownership and composition

M01 adds cohesive V8 owners under `src/map/` rather than another top-level subsystem:

```text
src/map/grid.rs              coordinates, footprints, occupancy and pure overlap rules
src/map/catalog/             shared definitions, parsing, validation and fingerprints
src/map/recipe/              recipe parsing, expansion, canonical resolution and indexes
src/map/runtime/             authoritative dynamic state, transaction and recovery
```

Existing files may remain while old maps run, but new types use `GridMap*` names only where a
temporary collision with old public names requires it. M04 removes transitional qualifiers rather
than preserving forwarding modules.

Server-owned entities/resources are:

- one `GridMapRoot` with map instance, recipe/catalog fingerprints, and once-replicated snapshot;
- one immutable collider entity per wall placement and four perimeter colliders;
- one `MapDynamicRoot` with exact generation/revision;
- one runtime entity/index entry and collider per live destructible placement;
- bounded pending effects, transaction scratch, outbox, recovery cache, and telemetry.

Decorations and spawn markers do not become authoritative colliders or one server ECS entity per
cell. Presentation entities are client-owned children of one client map root and are removed as a
unit on replacement/teardown.

### Authority and fixed-tick order

The existing `CombatWorldEffectFact` remains the combat-to-environment fact but the public effect
is renamed from terrain destruction to map destruction during M01. The combat definition radius
limit references the new map API; old terrain consumes the same fact temporarily for unconverted
maps.

For a V8 map, the authoritative fixed-post chain is:

```text
combat damage/outcome observation
  -> collect accepted map-destruction facts
  -> sort by simulation tick, attack ID and delivery index
  -> derive circle-overlapping candidate map cells
  -> stage unique live destructible placements
  -> build prospective colliders/state/events
  -> commit state and collider commands atomically
  -> ApplyDeferred
  -> repair any embedded fighters using the current bounded deterministic rule
  -> publish revisioned outcomes
  -> mode rules
```

The chain runs only when a matching V8 root exists. Old terrain runs only for an old map root.
Newest excess requests are rejected/deferred according to explicit ceilings and telemetry; a
partial brush never commits. A brush that touches no destructible placement produces no revision.
Projectile blocking remains ordinary Avian collision; only the explicit accepted world effect
changes cover.

Restart restores every authored destructible placement, rebuilds colliders, clears pending work,
advances generation, resets revision to zero, and publishes a reset. Replacement and teardown clear
the root, indexes, colliders, queues, outbox, recovery cache, client buffers, visuals, and effects.

### Wire contract and recovery

`ResolvedGridMapSnapshot` carries only stable shared facts:

- map instance, preset, recipe revision and fingerprints;
- schema/catalog identities, dimensions, default surface and canonical placements;
- mode anchors and derived spawn facts needed by shared consumers;
- initial dynamic generation identity.

It is registered as a once-replicated component. It never carries paths, transforms, Bevy handles,
entities, resolved colliders, per-cell duplicated defaults, or old region/terrain types.

The V8 dynamic protocol uses one ordered-reliable map channel and these bounded shapes:

```text
MapDynamicGeneration { map_instance_id, match_id, initial_fingerprint }
MapMutationEvent { generation, revision, source_attack_id, source_delivery_index, outcomes }
MapPlacementOutcome { placement_id, current_asset_id: Option<MapAssetId> }
MapDynamicResetEvent { previous_generation, next_generation }
MapDynamicRecoveryRequest { generation }
MapDynamicRecoverySnapshot { generation, revision, non_initial_outcomes }
```

M01 outcomes only remove authored `DESTRUCTIBLE_COVER`, but the state shape also represents the
already accepted replacement outcome without exposing a second protocol later. Outcomes and
recovery entries are sorted by placement ID and unique. Clients ignore duplicates, buffer a bounded
number of future events, request recovery on a revision gap or generation mismatch, and accept only
an exact-generation bounded snapshot. Late join/reconnect receives the immutable map plus current
dynamic snapshot before readiness.

The protocol registry and global content envelope are bumped once as required by the current
compatibility policy. There is no per-message version and no decoder for V4 bytes.

### Client acceptance and presentation

The client first validates the shared snapshot/fingerprint against its embedded V8 catalog, then
loads the theme and every referenced normal visual. Map readiness requires the authoritative
snapshot, matching dynamic generation/current state, and either each imported visual or its
declared primitive fallback.

Presentation derives:

- one generated ground plane from dimensions/default `GROUND`;
- current modular imported/fallback walls from the six wall placements;
- one generated or existing destructible-cover mesh per live cell, removed on committed outcomes;
- four imported/fallback decorations at their new cell centers;
- generated perimeter and outer dressing from derived bounds;
- no visible spawn-marker object.

Visual paths, scale, yaw correction, vertical offset, tint, fallback, and fitting policy live only
in the client catalogs. A visual cannot alter collider dimensions. Primitive and imported paths use
the same authoritative placement transforms and dynamic state.

### Admission, identity and lifecycle

The built-in V8 index retains preset ID `1` and key `crossroads-facility`; only one catalog may own
a preset/key. Operator catalog resolution and worker admission query the temporary combined preset
registry, validate Wipeout requirements through the selected resolver, and compare revision `5`.
The supervisor, lobby worker, match worker, and client all compute the same changed global content
fingerprint from canonical V8 material.

Map install is all-or-nothing. A failed resolver, catalog, snapshot, collider, visual-coverage, or
capacity check prevents readiness/admission rather than silently selecting the old Crossroads map.
Restart does not replace immutable map identity. A new selected map instance tears down the prior
old or new owner before installing exactly one next owner.

## Implementation checklist

- [x] Add exact V8 shared IDs, grid coordinates, checked conversions, slot occupancy, footprint,
  overlap, and canonical-order helpers.
- [x] Add bounded V8 shared gameplay/map-asset/theme catalogs and complete contradiction/reference
  validation.
- [x] Add the separate client visual/theme catalogs and a cross-role coverage audit that keeps
  client paths out of the server graph.
- [x] Add V8 recipe parsing, filled-rectangle expansion, canonical resolution, limits,
  fingerprints, collision descriptors, spawn index, and Wipeout layout validation.
- [x] Author the `56 x 36` Crossroads recipe with six wall placements, 36 destructible-cover
  placements, four decorations, and eight spawn markers at the reviewed cells.
- [x] Extend `build.rs` deterministically for V8 inputs and prove clean regeneration.
- [x] Add the temporary unique-preset dispatcher and annotate every retained legacy reference with
  its M03/M04 owner.
- [x] Add the V8 server root, static/perimeter colliders, dynamic placement state/index, install,
  replacement, restart, and teardown.
- [x] Rename the combat-facing terrain effect to map destruction and connect V8 collection,
  deterministic overlap, atomic commit, collider removal, fighter repair, and telemetry.
- [x] Add once-replicated V8 snapshot plus mutation/reset/recovery messages, channel, registry
  fingerprint update, publisher, receiver, gap recovery, and late-join bootstrap.
- [x] Add client catalog loading/readiness, ground/wall/cover/decoration/perimeter materialization,
  imported and primitive fallbacks, dynamic removal/recovery, and complete cleanup.
- [x] Update lobby selection, admission, routing fixtures, map revision, content identity,
  diagnostics, match spawn consumers, performance fixtures, native automation, and current docs
  required by the converted preset.
- [x] Add and maintain the exact migration inventory/removal assertions; do not create a V4 recipe
  decoder, V8-to-region adapter, forwarding `terrain` facade, or placement-level visual override.

## Verification plan

### Pure/catalog tests

- `56 x 36` bounds, cell min/center, world-to-cell overlap, even footprints, quarter-turns, and all
  six exact wall AABBs;
- deterministic nearest-cell Crossroads spawn/decor positions and east/west facing;
- filled-rectangle expansion to exactly 36 unique row-major placements;
- canonical source-order independence and stable recipe/catalog/content fingerprints;
- out-of-bounds/overflow, zero/duplicate IDs, bad rotation/parameters, unknown references,
  slot conflicts, disallowed surfaces, contradictory profiles, missing visual coverage, excessive
  dimensions/placements/bytes/colliders/recovery, and unsupported schema rejection;
- eight spawn markers provide the required 1v1/2v2/3v3 capacity, clearance, symmetry and
  reachability.

### ECS/authority tests

- one atomic install creates exactly six wall, 36 cover, and four perimeter colliders with no
  authoritative decoration/spawn entities;
- player and projectile collision agree with profiles at wall, cover, open ground and perimeter;
- representative radius-48 brushes remove the exact sorted cell placements, duplicates do nothing,
  and over-capacity batches never partially commit;
- collider removal completes before fighter repair and mode rules; current physics observes the
  committed state;
- restart restores all 36 placements under a new generation; replacement/teardown leaves no root,
  collider, index, queue, cache, or outbox;
- an old map cannot activate V8 runtime and a V8 map cannot activate terrain runtime.

### Separate-App/network/routed tests

- snapshot insertion and client catalog acceptance converge before map readiness;
- ordered mutation, duplicate, gap, recovery, stale generation, reset, late join, reconnect and
  replacement converge exactly;
- client intent cannot author a mutation, revision, collider, spawn, score, or recovery response;
- routed 1v1, 2v2 and 3v3 Crossroads Wipeout use preset `1` revision `5`, enter gameplay, destroy
  cover, complete, return, and requeue without identity mismatch;
- supervisor/lobby/match worker identities match and a stale revision/content/protocol fingerprint
  fails closed;
- server-only check confirms no render/window/audio/device-input/client-asset dependency enters the
  feature graph.

### Canonical commands and capacity evidence

Run the repository-owned commands, not substitutes:

```text
just check
just lint
just test
just e2e 2
just e2e 4
just e2e 6
just v3-render-evidence
```

The milestone records resolved recipe bytes, replicated snapshot bytes, maximum live event bytes,
full recovery bytes, collider/entity counts, fixed-post transaction time, client map entity/mesh
counts, load-to-readiness time, and repeated restart/reconnect memory behavior. Thresholds are set
before implementation from current gates and measured baseline; exceeding one fails the milestone
rather than being waived as future optimization.

## Visual and playtest handoff

The handoff supplies the canonical native run path and asks the user to play Crossroads Wipeout in
both imported and `BRAWLER_FORCE_PRIMITIVE_WORLD` presentation. The focused scenario is:

```text
# Imported presentation: select Wipeout 2v2 in the Dashboard and press Play in all four clients.
just run 4

# Primitive fallback presentation: repeat the same selection and flow.
BRAWLER_FORCE_PRIMITIVE_WORLD=1 just run 4

# Optional bounded two-window report rerun for this converted preset.
BRAWLER_RENDER_GAME_TYPE=wipeout-2v2 BRAWLER_RENDER_PLAYERS_PER_TEAM=2 \
  just v3-render-evidence target/v8-m01-user-render.txt
```

1. inspect symmetry, six walls, four decorations, perimeter, camera framing and spawn positions;
2. move and fire around each wall and live center-cover edge;
3. use the Arc Launcher at cover centers, edges and corners and judge the new 32-unit destruction
   granularity and feedback;
4. restart and confirm all cover returns;
5. disconnect/reconnect after partial destruction and confirm the same open cells appear;
6. complete and requeue a match and confirm no stale map or visual remains.

Requested observations are combat readability, whether the 16-unit inward spawn/decor shift is
noticeable or harmful, whether tile-sized destruction feels too coarse, collision/presentation
agreement, imported/fallback parity, and any stale or flickering state. Feedback that would change
grid size, destruction granularity, or the shared schema returns the milestone to specification
review before implementation continues.

## Implementation evidence

- `content/v8/` is now the only authored source selected for Crossroads Facility Wipeout preset
  `1`, admission revision `5`. It resolves a `56 x 36` map containing six structural wall
  placements, 36 independently destructible cover cells, four decorations, and eight spawn
  markers. The V4 Crossroads source remains unselected and is assigned to M04 deletion.
- The shared catalog binds author-facing `MapAssetId` values to explicit gameplay profiles and
  stable visual profile IDs. Client-only paths and transforms live in the two new catalogs under
  `assets/catalogs/`; the dedicated-server feature graph does not include them.
- Installation owns one grid root, six merged structural colliders, four perimeter colliders, and
  36 per-cell dynamic cover colliders: 46 authoritative map members in total. Decorations and spawn
  markers do not create authoritative entities.
- Map destruction is collected from the renamed `DestroyMap` combat effect, overlaps whole
  32-unit cells deterministically, commits sorted placement IDs atomically, and publishes ordered
  mutation facts. Restart restores all cover under a new generation; gap recovery returns the
  complete sorted removed-placement state.
- Protocol version is `18` and gameplay-content envelope version is `10`. Crossroads source size is
  3,262 bytes, its resolved replicated snapshot is 932 bytes, and the maximum current 36-cell live
  mutation and full recovery payloads are each 48 bytes.
- The production presenter selects Crossroads walls, cover, decorations, ground, and perimeter
  directly through V8 asset/visual IDs. It does not query the old semantic object-role catalog for
  converted placements. A neutral runtime projection temporarily supplies existing camera and
  match consumers, but contains no V4 recipe, region, or terrain authoring data and is assigned to
  M04 deletion.
- The eight explicit `V8-MIGRATION(M03/M04)` seams are enumerated by a focused removal-audit test.
  Retained terrain network fixtures opt into an explicitly test-only legacy map; production and the
  default network harness select V8 Crossroads.

The initially proposed directory tree was kept cohesive as `src/map/grid.rs` and
`src/map/grid_server.rs` for this first slice. Splitting catalog, recipe, runtime, and client into
additional subdirectories before a second converted map demonstrated distinct ownership would add
structure without changing lifecycle or feature boundaries.

## Verification evidence

Completed on 2026-08-23:

- `just check` passed for routing, client, server, network-test, and Balance Lab targets;
- `just lint` passed formatting, all Clippy targets with warnings denied, server feature isolation,
  and the sole-world-presentation audit;
- client unit suite passed `426/426`, the server unit suite passed `333/333`, the subsequently added
  bounded-payload server test passed separately, Balance Lab passed `343/343`, and the routing
  package suites passed;
- network integration passed `83/83`, including V8 default-map lifecycle and explicit retained
  legacy-terrain fixtures;
- performance passed `14/14`; the worst existing fixed-tick case measured p95 `6.676583 ms`, the
  combined case p95 `2.96775 ms`, maximum terrain p95 `2.621667 ms`, and recovery remained 32,530
  bytes with p50 serialization `27.289 us` and mesh/chunk work `181.506 us`;
- `just e2e 2`, `just e2e 4`, and `just e2e 6` passed. The first 2v2 attempt timed out; an immediate
  diagnostic rerun passed with both clients and the match reaching `Active`, and the subsequent
  campaign stayed green;
- `cargo fmt --all`, `git diff --check`, and the focused serialized-size test passed.

`just v3-render-evidence` was attempted three times for V8 Crossroads, including a TTY run with a
60-second bound. The supervisor and worker shut down cleanly, but all windowed client logs remained
empty and neither render report was created. Because routed headless 2v2 passes, this is recorded as
a local native-window harness limitation rather than evidence of visual correctness. Visual
correctness is deliberately not claimed; the user playtest below must close this gate.

## Exit criteria

- the user has validated this M01 specification and V7 has completed before status changes to
  `Implementing`;
- Crossroads Facility Wipeout has one V8 source and never resolves through an old recipe, region,
  object-role, or terrain-authoring adapter;
- initial structural collision is exact; reviewed point shifts and 32-unit destruction behavior
  are documented and accepted through native playtest;
- catalog, recipe, snapshot, runtime, recovery, presentation, admission and lifecycle contracts
  above pass their focused and canonical evidence;
- the V8 server graph contains no client catalog/path/render dependency and clients cannot mutate
  authoritative state;
- every temporary legacy dependency is annotated and assigned to M03/M04, with no new compatibility
  decoder or facade;
- the user accepts the playable first vertical slice and every feedback item is triaged;
- affected verification is rerun, the learn-from-errors review is recorded, and only then M01 is
  marked `Complete`.

## Feedback review

Completed on 2026-08-23:

- The converted map looks acceptable. No visual, collision, spawn, camera, reconnect, or lifecycle
  correction was requested.
- Keep whole 32-unit asset-cell destruction. The coarser result is more readable and avoids tiny
  surviving collision specks that can still block the player. This is accepted now and recorded in
  the durable grid-map specification; it is not deferred.
- No affected production code changed after the accepted playtest, so the already-green automated
  verification remains applicable. Documentation checks pass during the M01-to-M02 transition.

## Learn-from-errors review

Completed on 2026-08-23:

- Switching the default production map exposed terrain integration tests that implicitly depended
  on the default preset. Those fixtures now request a clearly named test-only legacy terrain map;
  future conversion work must make fixture map ownership explicit before changing defaults.
- The first spawn configuration assertion still encoded the old floating coordinate. It now edits
  and verifies the V8 marker cell and quarter-turn instead, which tests the authored contract rather
  than a compatibility projection.
- A single routed 2v2 timeout followed by a diagnostic pass showed why one failed process run must
  be investigated and rerun, not silently waived. The failed attempt and successful rerun are both
  retained in this evidence.
- Native render automation can fail without emitting per-client diagnostics. M01 leaves this as a
  visible recorded limitation; a later milestone should improve early client-start/connect
  diagnostics if the failure reproduces outside the Codex execution environment. Direct user
  playtesting supplied the missing native acceptance evidence for M01.
- The accepted coarse destruction is not merely an implementation compromise. Tying collision
  removal to visible 32-unit asset cells produces a clearer invariant: if the cell is visibly gone,
  its blocker is gone. Future optimization or presentation work must not reintroduce sub-cell
  collision remnants.
