# V8 Milestone 04 — Legacy eradication, hardening, and closeout

## Status

`User playtest`

Implementation began after the original cleanup specification was approved. On 2026-08-23 the
user identified that the broader production content tree was still organized by historical
version folders. M04 returned to specification review for this material scope correction before
moving or deleting active content. The user approved the amended cleanup plan on 2026-08-23.

## Outcome

M04 removes the superseded V4 map and 8-unit terrain systems after all shipped maps have already
cut over to the sparse 32-unit map-asset format. The final product has one authored catalog, one
resolver, one authoritative map runtime, one replicated snapshot and dynamic-state contract, one
client presenter, and one set of fixtures and diagnostics.

This milestone does not redesign the accepted maps. Crossroads Facility, Crossroads Hot Zone,
Ashen Court, and Tidal Garden retain their current cells, collision, spawn topology, themes, and
presentation. Destruction continues to remove or replace complete 32-unit map-asset cells; M04
must not reintroduce an 8-unit occupancy grid or collision specks. Concealing bushes remain the
future `V8-CONCEALMENT` slice.

## Entry evidence

M03 completed on 2026-08-23 after the user accepted the converted-map handoff by directing work to
proceed to M04. Its canonical checks, routed 1v1/2v2/3v3 scenarios, and imported/primitive Ashen and
Hot Zone evidence passed. Production map selection is already V8-only. Six deliberately dormant
`V8-MIGRATION(M04)` seams remain, so M04 deletes a known compatibility boundary rather than
performing another content migration.

## Research

### Local sources inspected

- `src/map/mod.rs`, `model.rs`, `grid.rs`, `grid_server.rs`, `server.rs`, and `client.rs` for the
  current dual model, plugin composition, install/teardown, resolution, recovery, and presentation
  acceptance paths;
- `src/map/definitions/**` and `src/map/objects.rs` for the complete old parser, object/variant,
  layout-requirement, and region-to-terrain ownership;
- all of `src/terrain/**` for the retired 8-unit chunk grid, brush transaction, collider rebuild,
  recovery, presentation, telemetry, and its 2,474-line focused test suite;
- `src/protocol.rs`, `src/content.rs`, `build.rs`, `src/server/admission.rs`, and
  `src/server/lobby/catalog.rs` for registry, content identity, generated input, admission, and map
  pool dependencies;
- `src/client/prediction.rs`, `src/client/presentation_3d/**`, `src/combat/**`, `src/movement/**`,
  `src/matchplay/**`, `src/server/verification.rs`, and `src/diagnostics/**` for consumers currently
  fed by the compatibility projection or old terrain vocabulary;
- `tests/network/map.rs`, `tests/network/terrain.rs`, `tests/network/harness.rs`, and
  `tests/performance.rs` for coverage that must be converted rather than silently lost;
- `content/v4/**`, `content/v8/**`, both environment catalog files, `assets/manifest.ron`, current
  documentation, and current scripts for source, asset, documentation, and automation traces;
- `references/bevy/examples/README.md` and `references/bevy/examples/app/plugin.rs` for the local
  Bevy snapshot's plugin ownership pattern;
- `references/lightyear/book/src/SUMMARY.md`,
  `references/lightyear/book/src/concepts/bevy_integration/system_order.md`,
  `references/lightyear/book/src/concepts/replication/protocol.md`, and
  `references/lightyear/book/src/concepts/replication/replicate.md` for receive/send ordering,
  shared registry requirements, channels, and replicated-component behavior.

The checked-in Bevy source is 0.20-dev while Brawler uses Bevy 0.19, so it is used only for plugin
structure. The checked-in Lightyear 0.29 book matches Brawler's Bevy version and supplies the
needed ordering and registration contracts. No internet research was necessary.

### Findings

1. V8 resolution still builds `ResolvedGridMap`, then constructs a complete old `ResolvedMap` as
   `compatibility`. Authoritative installation calls the old installer first. Camera, prediction,
   match capacity, spawning, objective setup, presentation, diagnostics, and several tests still
   read that projection.
2. Every production server still installs `AuthoritativeTerrainPlugin`. It notices `GridMapRoot`,
   tears down or bypasses its old chunks, and consumes no V8 destruction facts only because the V8
   system runs first. Every client still installs `ClientTerrainPlugin`, although client playability
   is now gated by map readiness instead. This is dormant runtime machinery, not merely dead data.
3. The protocol still registers the old snapshot plus four terrain messages and `TerrainChannel`.
   Each map root carries old and new snapshots, two root markers, and the new dynamic state.
4. `build.rs` still generates one source table for `content/v4` and another for `content/v8`.
   `gameplay_content_fingerprint` still accepts the old catalog, whose fingerprint method wraps
   both old and new catalogs.
5. The renderer loads both visual catalogs. V8 imported scenes are converted from
   `MapVisualProfileId` into the old `MapVisualVariantId` solely to reuse old handle/readiness
   resources. Its main materializer still has a V4 branch, and outer dressing still carries old
   variant IDs even though the V8 path does not use it.
6. The old terrain suite contains valuable behaviors—gap recovery, forged-request rejection,
   restart generation safety, projectile blocking, single-cue outcomes, and capacity ceilings.
   Deleting it without equivalent map-dynamic tests would reduce authority and recovery coverage.
7. V8 recovery currently validates active sessions and exact generations, but it has no retained
   counters and no per-link response limiter. The old terrain path did. M04 must preserve the
   security/evidence property in the canonical map owner before deleting the old implementation.
8. Ten GLBs are referenced only by the old environment catalog and have no other production
   consumer: Graveyard border pillar and iron-fence border; Mini Arena block, border corner,
   border straight, and column; Mini Dungeon barrel; Mini Forest fence, rocks-low, and tree. The
   four pack textures are shared by retained V8 assets and must remain.
9. Old technical vocabulary also survives outside the old modules: collision-layer names,
   `terrain_occlusion`, combat policy limits, dash telemetry, closeout reports, process-only terrain
   assertion flags, HUD tests, preview collision, verification scripts, and repository guidance.
10. The old map and terrain source tree is roughly 9,500 lines before dependent tests and consumer
    branches. The highest-risk work is therefore the consumer cutover and test replacement, not
    file deletion.

## Scope decisions

### One canonical model, without aliases

The V8 shapes become the ordinary map API. M04 does not leave `Grid*`/old pairs or deprecated type
aliases. Names that describe the authoring coordinate system may retain “grid” (`MapCell`,
`MapGridVertex`), but “Grid” is removed when it exists only to distinguish V8 from V4.

| Migration name | Final canonical name/disposition |
|---|---|
| `GridMapContentCatalog` / `GridMapCatalogResource` | `MapContentCatalog` / `MapCatalogResource` |
| `GridMapRecipe`, `GridMapPreset` | `MapRecipe`, `MapPreset` |
| `GridMapAssetPlacement`, `GridFilledRect` | `MapAssetPlacement`, `MapFilledRect` |
| `GridModeAnchorKind`, `GridModeAnchorPlacement` | `MapModeAnchorKind`, `MapModeAnchorPlacement` |
| `ResolvedGridMapSnapshot`, `ResolvedGridMap` | `ResolvedMapSnapshot`, `ResolvedMap` using only the sparse map-asset shape |
| `GridMapDynamicState`, `GridMapRoot` | `MapDynamicState`; delete the second root marker and use only `MapRoot` |
| `GridMapRuntimeSet`, `GridMapClientPlugin` | `MapRuntimeSet`, one `ClientMapPlugin` |
| `GridDestructibleCollider` | `DestructibleMapCollider` |
| old `ResolvedMap*`, recipe/object/region placement shapes | delete; no forwarding constructors or aliases |

Stable generic identities and derived runtime facts remain when they still have a real V8 owner:
`MapPresetId`, `MapRecipeId`, `MapInstanceId`, `MapRecipeFingerprint`, `MapPlacementId`,
`MapPresentationThemeId`, `ModeDefinitionId`, `ModeAnchorId`, `SpawnPointId`,
`ResolvedMapIdentity`, `AxisAlignedMapRect`, `MapShape`, `NormalizedArea`, `TeamSpawnPoint`,
`PlayableBounds`, `SpawnPointCatalog`, `MapRoot`, `MapInstanceMember`, and `SpawnAssignment`.
Floating `TeamSpawnPoint` values are derived runtime indexes from spawn marker cells; they are not
an authored floating-point spawn format.

The following IDs and shapes have no V8 owner and are deleted:
`MapPresentationProfileId`, `MapObjectDefinitionId`, `MapVisualVariantId`, `CollisionProfileId`,
`RegionProfileId`, `EntityDefinitionId`, `ModeAnchorDefinitionId`, `RegionId`,
`MapObjectPlacement`, `GeometryPlacement`, `VisualPlacement`, `MapEntityPlacement`,
`MapRegionPlacement`, `TeamSpawnArea`, old `ModeAnchorPlacement`, old `MapRecipe`, and every old
object/variant/binding/layout/terrain definition.

### Final source ownership

The large migration-era `grid.rs` is split because it currently combines wire model, authored
schema, catalog parsing, pure resolution, runtime indexing, and tests with different owners:

```text
src/map/
  mod.rs          composition and intentional crate-facing re-exports
  model.rs        stable IDs, cells/dimensions, gameplay properties, resolved wire/dynamic shapes
  catalog.rs      embedded V8 catalog/index loading, validation, canonical content fingerprint
  recipe.rs       sparse authored recipe and deterministic pure resolution/derived indexes
  server.rs       authoritative install/teardown, colliders, destruction, reset, recovery, telemetry
  client.rs       replicated-state convergence, validation, readiness, and presentation handoff
  tests.rs        focused composition/lifecycle tests not owned by the modules above
```

This is an ownership split, not one file per type. There is no `src/terrain` module and no second
map runtime. Client-only visual loading/materialization remains under `src/client/presentation_3d`,
with `environment_assets.rs` renamed to `map_assets.rs` and its retained resources expressed in
`MapVisualProfileId`.

### Canonical resolved runtime

`ResolvedMapSnapshot` is the sole replicated authored-resolution result. It contains the current
identity, schema versions, theme, mode, dimensions, default surface, canonical asset placements,
and typed mode anchors. It does not carry old geometry, entity, region, spawn-area, or visual-
variant arrays.

The server-only `ResolvedMap` resource carries that snapshot plus derived facts needed at runtime:

- spawn points grouped by team;
- merged static rectangle descriptors and individual circular descriptors;
- player-only surface rectangles;
- destructible/replacement placements;
- the optional resolved Hot Zone objective.

These facts are derived once by the canonical resolver and are not a compatibility projection.
Client prediction derives the equivalent blocking shapes from the replicated snapshot plus the
fingerprinted embedded catalog. Client snapshot acceptance still reconstructs the selected preset
and requires exact canonical equality before opening `ClientPlayableGate`.

### One authoritative lifecycle

`AuthoritativeMapPlugin` initializes one catalog and selection, allocates one nonzero instance ID,
resolves one selected preset, and installs one root containing:

- `MapRoot`, `MapInstanceId`, and `ResolvedMapIdentity`;
- `ResolvedMapSnapshot` and `MapDynamicState`;
- one `Replicate::to_clients(NetworkTarget::All)` owner.

Installation creates perimeter, merged static, circular, player-only surface, and dynamic
colliders directly from V8 facts; installs `PlayableBounds`, `SpawnPointCatalog`, `ResolvedMap`,
and the optional `ResolvedObjectiveZone`; and never calls a legacy installer. Teardown removes the
exact root and `MapInstanceMember` entities, clears map-owned resources/outboxes/telemetry, and has
no terrain cleanup handoff.

Whole-placement destruction remains a server-owned fixed-post transaction. Final order is:

```text
combat Damage / ability outcome observation
  -> MapRuntimeSet::ApplyDestruction
  -> collider/state commit
  -> MapRuntimeSet::Publish
  -> MatchSet::ModeRules
  -> Lightyear PostUpdate send
```

The current `TerrainSet` dependency is removed. Match restart uses the existing environment-reset
registration to advance generation, restore every dynamic placement and collider, clear terminal
state, and publish one reset. Removal/replacement cannot embed a fighter because every shipped
transition removes blocking volume or replaces it with nonblocking rubble; M04 does not add a
general fighter-repair phase without an authored transition that needs one.

### Dynamic recovery and telemetry

The current V8 mutation/reset/recovery messages remain the semantic protocol. The server accepts a
recovery request only from an active, connected session for the exact live map generation. M04 adds
a bounded per-link cooldown/one-pending-response rule so repeated valid requests cannot amplify
traffic, and records saturating aggregate counters for:

- destruction facts observed, no-op facts, committed mutation events, and placement transitions;
- deferred/dropped work;
- resets;
- recovery requests, responses, and rejections;
- maximum mutation-event and recovery-snapshot bytes.

This `MapDynamicTelemetry` is owned by `src/map/server.rs`; it does not recreate the old chunk
telemetry framework. It supplies current closeout evidence and focused forgery/recovery assertions.

Client convergence continues to accept only sorted, legal, previously unseen transitions for the
current generation. A gap or newer generation requests recovery once, an exact snapshot replaces
local state, stale traffic is ignored after reset, and map replacement clears the pending request.

### Client presentation and prediction

One `ClientMapPlugin` owns dynamic convergence, snapshot reconciliation, validation, readiness, and
the following update order after Lightyear's `PreUpdate` receive/replication work:

```text
MapClientSet::Converge
  -> MapClientSet::Reconcile
  -> MapPresentationSet::Materialize3d
  -> MapClientSet::Readiness
  -> Lightyear PostUpdate send
```

The 3D materializer becomes V8-only. It reads `ResolvedMapSnapshot`, `MapDynamicState`, the shared
map catalog, and one client `MapAssetVisualCatalog`. Handles, imported scenes, fallbacks, readiness,
themes, diagnostics, and tests use `MapVisualProfileId` directly. The old branch, old
`EnvironmentVisual*` types, object catalog, imported-border variants, and random old-theme outer
dressing are deleted. The existing generated V8 perimeter stays unchanged.

Weapon previews and client static-arena repair read effective V8 asset collision. A terminal
placement is absent or replaced according to `MapDynamicState`, so the preview never tests old
geometry/chunk data. This is presentation/prediction guidance only; Avian server collision remains
authoritative.

### Combat and movement vocabulary

The old terrain grid is also embedded in otherwise-current names. M04 performs these semantic
renames while preserving gameplay:

- `INDESTRUCTIBLE_TERRAIN_LAYER` / `DESTRUCTIBLE_TERRAIN_LAYER` become static/blocking-map and
  destructible-map layers;
- `terrain_collision_layers`, `destructible_terrain_collision_layers`, terrain muzzle/contact and
  targeting variables become map-collision names;
- `ArenaWall` becomes a general `MapCollider` marker;
- `terrain_occlusion` becomes `map_occlusion` in the weapon catalog and resolved definitions;
- `max_terrain_brush_radius` becomes `max_map_destruction_radius`;
- dash terrain truncation telemetry becomes map-collision truncation telemetry.

`DestroyMap(radius)` remains the correct gameplay effect. Its radius is validated as finite,
positive, and within the catalog/engine map-destruction ceiling. The retired 8-unit minimum and
4-unit half-cell rounding rule are removed because only the server computes circle-versus-32-unit-
cell overlap and clients receive exact placement transitions. The shipped Arc Launcher remains at
radius 48, so accepted player behavior does not change.

### Wire and content compatibility

M04 is an intentional hard compatibility break:

- register only `MapRoot`, the canonical `ResolvedMapSnapshot`, and `MapDynamicState` for map state;
- retain only `MapDynamicChannel` and its mutation/reset/recovery messages;
- delete `TerrainChannel`, all `Terrain*` messages, the old snapshot registration, and their
  protocol tests;
- bump `SUPPORTED_PROTOCOL_VERSION` from `20` to `21`;
- bump `GAMEPLAY_CONTENT_ENVELOPE_VERSION` from `12` to `13`;
- let the protocol-registry and gameplay-content fingerprints change from the one current registry
  and one current map catalog;
- keep V8 map catalog/recipe/fingerprint schema value `3` and the four map admission revisions
  unchanged because no recipe semantics or authored map changes;
- retain `NETWORK_PROTOCOL_ID` as the Brawler protocol-family identity.

There is no V4 decoder, terrain-message decoder, alias component, dual registration, or mixed-peer
mode.

### Content, build inputs, and shipped assets

Production content is organized by responsibility, not by the version or milestone that first
introduced it. Versioned directories are appropriate for historical implementation records, but
they falsely suggest that the runtime supports parallel content generations. Schema and
compatibility versions already have explicit ownership inside the RON documents and Rust protocol
constants.

The canonical production tree is:

```text
content/
  catalogs/
    builds.ron
    weapons.ron
    weapon_parts.ron
    map_assets.ron
    map_gameplay_profiles.ron
    map_presentation_themes.ron
  maps/
    index.ron
    builtin/
      ashen-court.ron
      crossroads-facility-hot-zone.ron
      crossroads-facility.ron
      tidal-garden.ron

assets/catalogs/
  map_asset_visuals.ron
  map_presentation_themes.ron
```

`content/catalogs` contains build-embedded, headless-safe gameplay definitions.
`content/maps` contains authored map documents and their index. `assets/catalogs` remains the
client-only visual/path/transform boundary. Operator-selected game-type configuration remains in
`config/server`; it is neither authored gameplay content nor a visual asset catalog.

The exact source-to-destination migration is:

| Current source | Canonical destination |
|---|---|
| `content/v1/builds.ron` | `content/catalogs/builds.ron` |
| `content/v1/weapons.ron` | `content/catalogs/weapons.ron` |
| `content/v7/weapon-parts.ron` | `content/catalogs/weapon_parts.ron` |
| `content/v8/map_assets.ron` | `content/catalogs/map_assets.ron` |
| `content/v8/map_gameplay_profiles.ron` | `content/catalogs/map_gameplay_profiles.ron` |
| `content/v8/map_presentation_themes.ron` | `content/catalogs/map_presentation_themes.ron` |
| `content/v8/maps/index.ron` | `content/maps/index.ron` |
| `content/v8/maps/builtin/*.ron` | `content/maps/builtin/*.ron` |

The move preserves every active file byte-for-byte. Catalog schema versions, recipe schema
versions, recipe fingerprints, preset IDs, admission revisions, and gameplay values do not change.
Loaders switch directly to the canonical path; there is no fallback search or compatibility
decoder. `build.rs` emits one `embedded_builtin_maps.rs` table from `content/maps/builtin`, and its
stable logical keys remain `builtin/<map-key>.ron`.

#### Content-layout execution sequence

1. Record the current bytes and resolved identities for all six active catalogs and four maps.
   This provides an explicit invariant, not a new migration format.
2. Create `content/catalogs` and `content/maps`, move the active files without rewriting their RON,
   and switch all production `include_str!` calls, generated map input, focused fixtures, and
   current documentation in the same change.
3. Collapse `build.rs` to one source directory, one generated output, and one generated constant.
   Verify client, server, network-test, and Balance Lab feature graphs from a fresh target directory
   before removing the old locations.
4. Delete `content/v1`, `content/v4`, `content/v7`, `content/v8`, and `content/.DS_Store`. No old
   directory remains as an empty namespace, symlink, fallback, or test fixture.
5. Run the removal audit, catalog/resolver/admission tests, full canonical matrix, and a second
   empty-target regeneration proof. Compare the recorded catalog, recipe, and admission identities
   to demonstrate that organization changed while gameplay content did not.

At every intermediate buildable state there is exactly one loader for each catalog. The move does
not combine client visual paths with server-safe gameplay data: that execution-role boundary is
more important than putting every map-related file in one physical directory.

Delete:

- all of `content/v1`, `content/v4`, `content/v7`, and `content/v8` after the active files above
  have moved and all loaders use their canonical destinations;
- `content/.DS_Store`;
- `assets/catalogs/environment_visuals.ron`;
- the V4 generated-source call/table and the migration inventory markers;
- the ten GLBs proven to be old-catalog-only:
  `graveyard/{border-pillar,iron-fence-border}.glb`,
  `mini-arena/{block,border-corner,border-straight,column}.glb`,
  `mini-dungeon/barrel.glb`, and
  `mini-forest/{fence,rocks-low,tree}.glb`;
- those ten manifest rows.

Keep every shared texture and every GLB referenced by `map_asset_visuals.ron`. Replace the old
manifest test with one that extracts every imported path from the active map-asset visual catalog,
requires a manifest entry and file, and validates referenced GLB dependencies. `build.rs` generates
one canonical `embedded_builtin_maps.rs` table from `content/maps/builtin` into a clean target.

### Diagnostics, scripts, and documentation

Closeout and native evidence replace inactive `terrain_*` fields with map-dynamic generation,
revision, terminal-transition, collider, mutation, reset, recovery, byte, deferred, and dropped
metrics. Process diagnostics read `MapDynamicTelemetry`. Obsolete direct-UDP terrain-assertion env
variables, dummy weapon path, report writer, and branches are removed from `scripts/network.sh` and
server verification; equivalent behavior is covered through canonical V8 separate-App/routed
tests and actual Arc Launcher scenarios.

Current documentation is reconciled in `README.md`, `AGENTS.md`, `docs/README.md`,
`docs/04-maps-and-game-modes.md`, `docs/08-network-architecture.md`,
`docs/09-environment-gameplay.md`, `docs/11-art-and-presentation-direction.md`,
`docs/16-grid-map-asset-system.md`, and `docs/backlog.md` where relevant. Current documents teach
only the completed map-asset system. Historical implementation documents under
`docs/implementation/v1` through `v7` and completed V8 milestone records remain unchanged as
evidence.

## Explicit non-goals

- no map layout, theme, balance, collision, spawn, objective, or destruction-granularity redesign;
- no concealment, bush privacy, teleport, chest, interaction framework, editor, upload, or
  procedural map work;
- no V4 runtime migration or compatibility layer;
- no renderer choice or 2D gameplay-world fallback;
- no deletion of shared asset dependencies merely because an old catalog also referenced them;
- no broad cleanup of unrelated uses of “legacy,” including the intentionally retained direct-UDP
  comparison baseline and separate combat cue compatibility work.

## Legacy-removal inventory

| Owner | Exact final condition |
|---|---|
| Production content layout | no `content/v*` directory exists; active gameplay catalogs live under `content/catalogs` and maps under `content/maps` |
| V4 authored data | retired V4 files do not exist and `build.rs` has no V4 input |
| Old visual data | `environment_visuals.ron`, `EnvironmentVisual*`, `MapVisualVariantId`, and old-only GLBs are gone |
| Old parser/resolver | `src/map/definitions/**`, `objects.rs`, `MapObject*`, layout policies, region resolver, and dual resolution are gone |
| Compatibility runtime | no `compatibility` field/function, second snapshot, `GridMapRoot`, legacy override, or old installer remains |
| Old terrain | `src/terrain/**`, terrain plugins/sets/chunks/brushes/wire/recovery/presentation/telemetry are gone |
| Consumer vocabulary | collision layers, combat catalog fields, previews, HUD, diagnostics, logs, and scripts use map-asset/map-dynamic terms |
| Tests | no old fixture, terrain suite, legacy override, or old maximum-policy map builder remains; equivalent V8 behavior is covered |
| Current docs | no current architecture/layout claims point at `content/v4` or teach regions/destructible reservations |
| Migration scaffolding | no `V8-MIGRATION` marker, compatibility alias, dead branch, or generated stale-input dependency remains |

## Removal audit

Add one repository script and `just` gate that fails on:

1. existence of any `content/v*` directory, `content/.DS_Store`,
   `assets/catalogs/environment_visuals.ron`, `src/terrain`, or any of the ten old-only GLBs;
2. production/current-source matches for the retired IDs, object/region/terrain types, old schema
   keys, old catalog path, `LegacyMapTestOverride`, `GridMapRoot`, `compatibility_runtime_map`,
   `TerrainChannel`, `V8-MIGRATION`, or old user-facing V4/terrain-grid messages;
3. an imported active map visual absent from the asset manifest or disk;
4. more than one embedded map source directory/table or more than one replicated map snapshot/root
   contract.

The audit searches `src`, `tests`, `content`, `assets/catalogs`, `config`, `scripts`, `build.rs`,
`README.md`, `AGENTS.md`, and current top-level `docs/*.md`. It deliberately excludes `target`,
`.git`, `references`, `external_assets`, and `docs/implementation/**`, where historical evidence is
allowed to name the deleted system. The M04 file itself is historical implementation evidence once
complete.

A second proof builds from a newly created empty `CARGO_TARGET_DIR`; passing an incremental build
is insufficient because a stale generated V4 table could mask a broken `build.rs` cutover.

## Implementation plan

### Implementation progress — 2026-08-23

The hard cutover is implemented and has entered verification:

- `src/map/{model,catalog,runtime,server,client}.rs` is the only production map model, resolver,
  authority, recovery, and client-convergence path;
- `MapAssetId` placements directly derive bounds, spawns, objective state, static/dynamic/player-
  only colliders, prediction, preview collision, and presentation;
- `MapDynamicTelemetry` records destruction and recovery outcomes, and each active link is limited
  to four recovery responses per exact map generation;
- all active authored gameplay definitions now live in `content/catalogs`, built-in recipes in
  `content/maps`, and client map visuals in `assets/catalogs`;
- the retired map schemas, compatibility projection, second root/snapshot, terrain module and wire,
  old environment catalog, and ten old-only GLBs have been deleted;
- protocol version `21` and gameplay-content envelope `13` are active, with routed allocation
  revisions updated to the canonical map admission revisions;
- `scripts/check-v8-map-cleanup.sh` is part of `just lint`, and a package-clean regeneration rebuilt
  both client and server without a stale generated map table.

Verification evidence:

- `just check`, `just lint`, and `just test` pass from the canonical repository entry points;
- clean client/server role rebuilds pass after `cargo clean -p brawler` removed package artifacts;
- client unit suite: 344 passed; server unit suite: 266 passed; Balance Lab unit suite: 276
  passed; routing package: 100 tests across its unit/process suites passed;
- the seven map network scenarios pass, including connected/late-join destruction convergence,
  atomic rubble replacement, root replacement, content rejection, and no client authority;
- the full separate-App network suite passes all 78 scenarios, including its restart and reconnect
  soaks;
- retained performance suite: 11 passed; maximum 512-transition dynamic state is 1,415 bytes with
  7.750 microseconds measured serialization p95 on the verification host;
- client, server, and Balance Lab Clippy gates pass with warnings denied;
- server feature isolation, retired 2D renderer, and map-cleanup audits pass;
- `just e2e 2`, `just e2e 4`, and `just e2e 6` pass, and explicit `first-blood`,
  `wipeout-2v2`, `tidal-garden-2v2`, `hot-zone-2v2`, and `hot-zone-3v3` routed runs all
  reach `Active` with their exact rosters;
- all eight release-client native records pass their locked thresholds: imported and forced-
  primitive Crossroads Wipeout, Crossroads Hot Zone, Ashen Court, and Tidal Garden. Each run
  requested a 1280x720 logical window; macOS/Metal reported its Retina 2560x1440 backing size.
  Reports are under ignored `target/v8-m04-evidence/` and therefore remain local evidence rather
  than production content.

The Crossroads and Hot Zone native logs can emit a deferred-command warning when presentation
teardown tries to despawn an entity already removed in the same frame. The locked reports pass,
authoritative workers stop cleanly, and entity high-water/terminal bounds remain valid. It is a
pre-existing presentation teardown diagnostic rather than evidence of a map-state or collision
failure; the user playtest should report any visible symptom if one exists.

### Wave 1 — Canonical V8 ownership and consumer cutover

- [x] Split the current grid model/catalog/recipe responsibilities into the final map modules.
- [x] Make the V8 resolved snapshot/runtime the canonical names with no aliases.
- [x] Derive spawns, bounds, objective, static/circular/player-only/dynamic colliders directly.
- [x] Convert match capacity, spawning, practice, prediction, camera, combat preview, lobby,
  admission, Balance Lab, diagnostics, and test harness consumers.
- [x] Keep the selected map recipes and their fingerprints/admission revisions stable.

### Wave 2 — One authority, client, and presentation path

- [x] Merge the authoritative installers into one root/resource/collider lifecycle.
- [x] Remove the compatibility projection and legacy test override.
- [x] Merge client convergence/readiness into `ClientMapPlugin` with explicit sets.
- [x] Make the 3D presenter and asset readiness V8-only and keyed by `MapVisualProfileId`.
- [x] Convert objective rendering, preview collision, root replacement, and teardown tests.

### Wave 3 — Retire terrain and harden map dynamics

- [x] Add bounded per-link recovery admission and `MapDynamicTelemetry`.
- [x] Port the valuable terrain recovery/security/restart/projectile/cue scenarios to map dynamics.
- [x] Remove both terrain plugins, `TerrainSet`, the entire `src/terrain` module, terrain wire, and
  client terrain readiness/presentation.
- [x] Rename collision, occlusion, destruction-limit, dash, report, log, and script vocabulary.
- [x] Replace old performance ceilings with current 512-placement/128x96/dynamic-state ceilings.

### Wave 4 — Delete sources/assets and close every trace

- [x] Move every active gameplay catalog and map byte-for-byte into `content/catalogs` and
  `content/maps`; switch every `include_str!`, test fixture, generated input, and current-doc link
  directly to those paths.
- [x] Delete all `content/v*` directories, old map modules/types/tests, the old environment
  catalog, ten old-only GLBs, `.DS_Store`, and their manifest rows.
- [x] Collapse `build.rs` and gameplay content fingerprinting to one canonical map catalog and one
  `content/maps/builtin` generated table.
- [x] Apply protocol/content compatibility bumps and update locked expectations.
- [x] Reconcile current documentation and repository orientation.
- [x] Add and pass the removal audit and empty-target regeneration proof.

### Wave 5 — Verification, playtest, and closeout

- [x] Run the complete automated matrix below.
- [x] Record imported and forced-primitive native evidence for all four maps.
- [x] Hand off a concise all-map playtest.
- [ ] Triage every playtest item.
- [ ] Re-run affected checks, complete the learn-from-errors review, and mark M04/V8 complete only
  after user acceptance.

## Verification plan

### Focused catalog and pure tests

- all four V8 documents parse, validate, resolve, and fingerprint in stable preset order;
- cell/world conversion, rotated multi-cell footprints, filled rectangles, canonical ordering,
  conflicts, bounds, default surfaces, anchors, spawn safety, reachability, and collision shapes;
- exact active visual/theme coverage and asset-manifest/dependency closure;
- maximum 128x96 dimensions, 512 resolved placements, 96 KiB recipe, 64 KiB snapshot, dynamic
  transition, event, and recovery bounds;
- no old catalog or generated input is required from a clean target.

### ECS/authority and client tests

- one root, one snapshot, one dynamic state, exact generic resources, and exact collider ownership
  on install;
- rectangle merging, circles, water, indestructible assets, destructible assets, and replacement
  assets produce the expected player/projectile layers;
- Arc destruction removes/replaces whole placements, commits state/collider together, produces one
  revision, and preserves the accepted coarse cell result;
- restart advances generation and restores colliders; map replacement and teardown remove all
  prior roots, members, outboxes, recovery admission, telemetry, and presentation meshes;
- client accepts only exact embedded content, handles duplicate/gap/stale/new-generation traffic,
  requests bounded recovery, and closes its gate on invalid state;
- preview/static repair follows effective current asset collision without authority mutation;
- repeated install/replacement/restart cycles have stable entity/collider/resource high-water marks.

### Network and product tests

- two clients receive identical canonical snapshot/dynamic state and no authoritative colliders;
- mutation convergence, adverse impairment gap recovery, late join, reconnect, and root replacement;
- forged foreign/stale/disconnected/repeated recovery requests cannot mutate state or amplify
  responses;
- restart ignores stale pre-reset mutation/recovery traffic;
- straight shots stop at destructible cover until removal, and one Arc landing produces one landed
  cue while changing every overlapping whole placement once;
- Crossroads Wipeout, Crossroads Hot Zone, Ashen Court, and Tidal Garden remain admissible only for
  their declared modes/revisions;
- routed First Blood 1v1, Wipeout/Hot Zone 2v2, and Wipeout/Hot Zone 3v3 complete and requeue.

### Canonical commands and role isolation

- `just check`;
- `just lint`, including server feature isolation and the removal audit;
- `just test`;
- `just e2e 2`, `just e2e 4`, and `just e2e 6`;
- explicit routed product runs for `first-blood`, `wipeout-2v2`, `tidal-garden-2v2`,
  `hot-zone-2v2`, and `hot-zone-3v3`;
- empty-target client, server, network-test, and Balance Lab regeneration/checks.

### Performance and lifecycle campaign

- fixed-post p95 for a maximum legal placement map and simultaneous maximum admitted destruction
  facts;
- mutation and recovery byte maxima for the largest shipped and synthetic legal states;
- static, circular, player-only, and dynamic collider counts for the largest map;
- repeated Ashen destruction/restart and Hot Zone completion/requeue/reconnect loops;
- client entity, generated mesh, imported scene, material, cache, queue, and server memory high-water
  marks remain bounded with no cycle-over-cycle growth.

### Native evidence and user playtest

Record 1280x720 imported and `BRAWLER_FORCE_PRIMITIVE_WORLD=1` evidence for:

1. Crossroads Facility Wipeout (`wipeout-2v2`);
2. Crossroads Facility Hot Zone (`hot-zone-2v2`, plus routed 3v3);
3. Ashen Court (`first-blood`);
4. Tidal Garden (`tidal-garden-2v2`).

The playtest asks the user to confirm that cleanup caused no visual/collision/topology regression,
that whole-cell destruction still leaves no blocking specks, that water and non-concealing grass
remain honest, that Hot Zone is readable, and that imported/fallback paths communicate the same
gameplay. M04 does not ask the user to reapprove a new artistic direction.

Playtest handoff:

1. Run `just run 2`, select **First Blood**, and inspect Ashen Court.
2. Run `just run 4`; in all four clients select the same game in turn: **Wipeout 2v2**,
   **Hot Zone 2v2**, and **Tidal Garden 2v2**.
3. Repeat the relevant run with `BRAWLER_FORCE_PRIMITIVE_WORLD=1` to compare the fallback.
4. In Crossroads or Tidal Garden, use Arc Launcher destruction and verify that each affected
   placement disappears or becomes its complete rubble replacement, with no tiny blocking speck.
5. In Tidal Garden, verify that water blocks players but not shots and that tall grass does not
   imply concealment. In Hot Zone, verify that the objective remains readable while fighting.

Requested response: report each map as pass or name the visible/collision/topology difference,
including whether it occurs in imported assets, primitive fallback, or both.

## Exit criteria

- the user approves this specification before implementation;
- one canonical map catalog, resolver, root, snapshot, dynamic state, authority plugin, client
  plugin, visual catalog, and generated input remain;
- the removal inventory and audit reach zero without aliases, decoders, dormant branches, or test-
  only legacy composition;
- `content/v4`, the old visual catalog, `src/terrain`, and old-only assets are deleted;
- converted behavioral/security/performance coverage passes before old tests are removed;
- all canonical, empty-target, routed, native, lifecycle, and role-isolation gates pass;
- current docs and repository guidance teach only the completed sparse map-asset system;
- every playtest item is triaged, affected verification is rerun, and the learn-from-errors review
  is complete;
- the user accepts the closeout before M04 and V8 become `Complete`.

## Feedback review

Pending user playtest feedback. Specification approval, implementation, automated verification,
routed E2E, and imported/primitive native evidence are complete.

## Learn-from-errors review

Pending user playtest and feedback review; closeout learning will be recorded before M04/V8 become
`Complete`.
