# Milestone 02 — Default 3D arena, map, terrain, camera, and input cutover

## Tracking

| Field | Value |
|---|---|
| Status | Complete |
| Prepared | 2026-08-20 after the user accepted M01 and directed M02 to begin |
| Objective | Make the accepted 3D arena/map/terrain/camera/input composition the supported client default and remove the replaced 2D map/terrain paths |
| Entry dependency | Satisfied — M01 closed with a positive feasibility decision, accepted camera direction, server isolation, reusable 3D foundations, and explicit transferred verification |
| Scope authority | Validated by the user's explicit direction to implement M02 on 2026-08-20 |

## Player-visible outcome

The normal product client enters Wipeout and Hot Zone directly into the accepted fixed-camera 3D
arena. The ground, playable perimeter, permanent cover, Hot Zone, and destructible terrain are real
depth-buffered 3D geometry. The first environment theme uses exact modular Mini Arena cover where
the authored footprint matches; every imported family has an exact procedural fallback. Mouse and
controller aim remain planar, camera containment works at supported aspect ratios, and map/terrain
visuals rebuild and disappear with their authoritative generation.

The old 2D map and terrain renderers and the `2d`/`3d` selector no longer exist after this cutover.
Screen-space Bevy UI still uses its dedicated UI camera. M02 intentionally retains the M01 3D
fighter, weapon, projectile, and objective proof until M03 replaces the remaining combat-world
presentation systematically.

## Scope boundary

### In scope

- the default orthographic `Camera3d`, lighting, dedicated UI camera, camera follow/clamp, and
  ground-plane mouse aiming;
- renderer-neutral map snapshot validation/readiness separated from 3D generation spawning;
- 3D floor, perimeter, rectangular and circular permanent geometry, resolved visual instances,
  placed map entities, product-visible mode anchors, and exact procedural fallbacks;
- one coherent `facility-greybox` environment profile using selected Mini Arena assets;
- 3D destructible-terrain chunk meshes and bounded debris driven by convergence dirty state;
- map replacement, terrain reset/recovery, disconnect, degraded-asset, and teardown behavior;
- removal of `WorldPresentationMode`, `--world-renderer`, `BRAWLER_WORLD_RENDERER`, the 2D
  map/terrain spawners, and their tests/assets where no other client feature owns them;
- unchanged server authority, Avian 2D collision, fixed-tick rules, and network protocol.

### Outside M02

- complete fighter variants, weapon attachment polish, animation transitions, sentries, combat
  previews, cue/effect conversion, status markers, world health, and final projectile coverage
  (M03);
- final deletion of residual 2D combat presentation and client render dependencies still used by
  M03 scope (M04);
- final release-profile performance and occlusion tuning (M04);
- additional Dungeon, Forest, Pirate, or Graveyard themes without a selected authored map;
- Avian 3D, replicated height, vertical gameplay, perspective/orbit cameras, decals, or original
  production art.

## Research conclusion

M02 is feasible without a gameplay, physics, content-schema, or wire-format migration. The
resolved map snapshot already contains stable profile IDs, exact authoritative shapes, bounded
expanded floor instances, generation identity, and all camera bounds. Terrain convergence already
owns authoritative client occupancy, dirty chunks, applied brushes, and reset/recovery state. The
work is therefore a client presentation ownership change, not a second map or terrain model.

The main implementation risk is duplicate consumption. Today `MapPresentationPlugin` combines
snapshot validation with 2D spawning; `ClientTerrainPlugin` always consumes dirty chunks and brush
feedback for sprites; and the M01 proof separately scans the same state for 3D. M02 resolves this
by giving each renderer-neutral lifecycle one owner and each 3D presentation stream exactly one
consumer before deleting the legacy systems.

No research finding requires 3D authority, a replicated `Vec3`, a new crate, a generic asset
framework, custom rendering, GPU instancing, or a permanent renderer abstraction.

## Current source and content inventory

### Ownership that remains renderer-neutral

| Owner | Retained responsibility |
|---|---|
| `src/map/client.rs` | select the newest `ResolvedMapSnapshot`, validate schema/count/profile bounds, own `PresentedMap` and `ClientMapReadiness`, close the playable gate on invalid/missing generations |
| `src/terrain/client/recovery.rs` | derive expected terrain, apply incremental events/recovery snapshots, own convergence phase and committed occupancy |
| `src/terrain/client/mod.rs` | own `ExpectedClientTerrainSlot`, `ClientTerrainReadiness`, recovery composition, and terrain input gating |
| `src/client/assets.rs` | retain client-only asset handles, evaluate root plus recursive dependency state, and publish deterministic degraded readiness |
| `src/client/presentation_3d/coordinates.rs` | map simulation positions/directions/rotations/extents to the render X/Z ground plane |
| authority/protocol/map model | remain unchanged and unaware of meshes, materials, transforms, scene roots, or presentation height |

### Legacy work removed by M02

- `spawn_snapshot_visuals`, 2D objective tinting, `Sprite`, `Mesh2d`, `ColorMaterial`, and 2D map
  geometry helpers in `src/map/client.rs`;
- `src/terrain/client/presentation.rs` image painting, chunk `Image`/`Sprite` ownership, and sprite
  debris, replaced by the 3D terrain module and tests;
- `reconcile_3d_map` and terrain lifecycle ownership from the broad M01 feasibility module after
  equivalent systems move to their domain owners;
- the configuration enum/field/parser/validation, CLI flag, environment variable, conditional
  plugin branch, documentation, and automation assumptions for renderer selection;
- the old facility tileset only if the post-M02 source audit proves no remaining UI or combat use.

`Sprite`, `Mesh2d`, `ColorMaterial`, and `Camera2d` are not banned globally in M02: remaining combat
work still owns some of them until M03/M04, and the screen-space UI camera is intentional. The M02
audit is scoped specifically to arena, map, and terrain presentation.

### Built-in map facts

Both built-in recipes use the `facility-greybox` theme and bounds from `(-896, -576)` to
`(896, 576)`. Each expands one 28×18 floor placement into 504 stable 64×64 visual instances. Each
contains six rectangular permanent-cover placements whose dimensions are integral multiples of
64, one central destructible-terrain reservation, two spawn areas, and eight spawn points. Hot Zone
adds one circular area anchor of radius 160. There are currently no placed map entities.

The following visibility policy is selected:

| Snapshot family | M02 presentation |
|---|---|
| floor visual instances | calm shared procedural 64×64 floor tile geometry; consume all 504 resolved instances directly |
| permanent rectangular geometry | one exact 64×64 module per authored cell; imported Mini Arena block when ready, exact cuboid fallback otherwise |
| permanent circular geometry | procedural cylinder with the authoritative radius; covered by a synthetic fixture because built-in maps have none |
| playable perimeter | four procedural cuboids outside the authoritative playable edge; never used for collision |
| destruction-reservation region | no separate visual; committed destructible terrain is its player-visible representation |
| spawn areas and spawn points | no product-world visual; they are server placement metadata, and the old debug/planning markers are removed |
| placed inert entity | procedural marker fallback for the existing profile contract; covered by a synthetic fixture because built-in maps have none |
| Hot Zone area anchor | procedural fill disc and annulus on the ground; the annulus centerline follows the authoritative radius |
| point mode anchor | no generic marker; the owning gameplay entity/effect presents it, as with the practice dummy |

The serialized enum name `VisualPlacementKind::Sprite` remains for compatibility. It describes an
authored single visual placement, not the active renderer, and renaming it would create unrelated
content/protocol churn.

## Environment asset decision

### Selected first theme

M02 uses only Kenney Mini Arena for the current `facility-greybox` map. Mixing Mini Dungeon or Mini
Forest into the same authored theme would reduce visual coherence and create unused profile
machinery. Those packs remain staged source material under `external_assets/` until a real theme
and map select them.

The candidate GLBs were inspected for accessor bounds, nodes, texture URI, and preview scale:

| Candidate | Source-space bounds | Decision |
|---|---:|---|
| Mini Arena `floor.glb` | 1×0×1 | compatible, but not selected: 504 scene hierarchies add no first-slice value over shared procedural floor tiles |
| Mini Arena `floor-detail.glb` | 1×0.07×1 | defer until an authored sparse-detail placement exists |
| Mini Arena `block.glb` | 1×0.5×1 | select for permanent 64×64 cover; uniform scale 64 gives an exact footprint and height 32 |
| Mini Arena `wall.glb` | 1×1×0.6 | retire from map use: its 64×38.4 footprint needs a hidden plinth and is less exact than `block.glb` |
| Mini Dungeon `wall.glb` | 1×1.1×1 | exact footprint but belongs to a different unselected theme |
| Mini Dungeon `wall-half.glb` | 1×1×1 | exact footprint but belongs to a different unselected theme |
| Mini Forest ground/rock modules | approximately 1×height×1 | defer until a forest map owns their visual role |

Implementation copies only `block.glb`, its relative `Textures/colormap.png`, and the pack license
into the namespaced runtime asset tree and records exact provenance in `assets/manifest.ron`. The
M01 `wall.glb` runtime entry is removed if no remaining code consumes it. `external_assets/`
remains read-only source material.

### Imported-module lifecycle

Each permanent-cover cell first receives an exact cuboid presentation. The optional Mini Arena
block is loaded once as a retained `Handle<Gltf>` and evaluated with root plus recursive dependency
state. When the GLB and colormap are ready, the system replaces that cell's fallback child with a
`WorldAssetRoot` for scene zero, scaled uniformly by 64. A failed root or dependency keeps the
cuboid; it never blocks map or terrain readiness and never leaves a partial imported hierarchy.

Only 24 cover modules exist in each current map, so cached scene cloning is the smallest clear
solution. M02 does not extract glTF primitives, add a static-model registry, or add custom
instancing. Bevy's normal render batching may combine compatible procedural tiles; measured
optimization remains an M04 decision.

### Deterministic failure proof

Add a client-local `ImportedWorldFallbackPolicy` resource with `Auto` and `ForcePrimitive`
variants. Production composition inserts `Auto`; tests and the existing windowed verification
configuration can insert `ForcePrimitive`. The policy affects presentation selection only—it does
not alter asset paths, readiness, map state, protocol state, or server behavior. It is not a
renderer selector and is not exposed in the product shell.

The forced path must prove character, blaster, and environment fallbacks together because M01
transferred all three dependency-failure cases. A failed optional audio asset continues to degrade
to silence under the existing asset policy.

## Technical specification

### Module and plugin ownership

Use responsibility boundaries rather than retaining one broad feasibility module:

```text
src/client/presentation_3d/
  mod.rs          3D world plugin composition, shared primitive/material resources,
                  lighting, and temporary M01 actor/projectile presentation
  coordinates.rs simulation-to-render conversion contract
  camera.rs       arena Camera3d, UI camera, follow/clamp, and viewport ground ray
src/map/
  client.rs       renderer-neutral snapshot reconciliation/readiness only
  presentation_3d.rs  generation-owned floor/perimeter/geometry/entity/anchor presentation
src/terrain/client/
  mod.rs          convergence/readiness composition
  presentation_3d.rs  dirty chunk meshes, exposed faces, debris, and teardown
```

`Feasibility3dPresentationPlugin` becomes `WorldPresentationPlugin` and is installed unconditionally
for a windowed client. `MapPresentationPlugin` remains the renderer-neutral map lifecycle and adds
its own 3D presentation systems only in the client render feature graph. `ClientTerrainPlugin`
replaces its 2D presentation chain with the 3D consumer. No new public crate API is needed.

### Resources and components

| Item | Owner and purpose |
|---|---|
| `PresentedMap` | renderer-neutral accepted source root, instance ID, fingerprint, playable bounds, and camera bounds |
| `MapPresentationMember` | every 3D map-owned entity; generation-tagged for exact teardown |
| `Map3dGeneration` | client-local record of the one materialized map instance and whether optional imported modules were upgraded |
| `WorldPrimitiveAssets` | one cached mesh handle per repeated primitive family |
| `WorldMaterialAssets` | bounded cached materials for floor, walls, perimeter, terrain, fallback props, teams, and objective states |
| `ImportedWorldAssets` | retained optional GLB handles and evaluated family readiness; extends the existing client asset owner rather than duplicating loading |
| `ImportedWorldFallbackPolicy` | deterministic Auto/ForcePrimitive presentation decision |
| `TerrainChunkVisual3d` | chunk ID, complete `TerrainGeneration`, and one mutable mesh handle |
| `TerrainDebris3d` | complete terrain generation and client-time expiry; no collider or replication |
| `ArenaCamera` | exactly one world `Camera3d`; the existing screen-space UI camera remains separately marked |

Materials are shared. Hot Zone uses a bounded palette of fill/boundary material pairs for Empty,
Contested, Team 0, and Team 1; tinting swaps handles rather than mutating a material shared by
unrelated generations. Terrain owns one shared matte material while each live chunk owns only its
geometry mesh, which must change with occupancy.

### Schedule and deferred-command boundaries

The selected ordering is:

```text
Update
  MapPresentationSet::Reconcile
      validate newest snapshot; install/remove PresentedMap and readiness
  MapPresentationSet::Materialize3d (after Reconcile)
      observe accepted snapshot; replace one map generation; upgrade optional modules

  TerrainClientSet::Derive
      derive expected generation from accepted map + match
  TerrainClientSet::Converge (after Derive)
      apply events/recovery or clear on disconnect
  TerrainClientSet::Present3d (after Converge)
      consume dirty chunks and applied brushes exactly once
  TerrainClientSet::Readiness (after Converge)
      publish terrain readiness

PostUpdate
  actor/projectile pose writes
      after Lightyear interpolation and Avian writeback
  ArenaCameraSet::Follow
      after pose writes, before TransformSystems::Propagate
  terrain/map systems do not run here
```

Normal deferred commands may make a newly accepted map materialize on the following frame. That is
allowed: `PresentedMap` and readiness become the lifecycle truth first, and the playable gate also
waits for terrain convergence. Do not insert an `ApplyDeferred` solely to save one presentation
frame. System sets and ordering stay visible at their composition points.

### Map reconciliation and generation lifecycle

1. Select the highest map instance ID exactly as today.
2. If no snapshot exists, despawn the current `MapPresentationMember` generation, remove
   `PresentedMap`/`Map3dGeneration`, set map readiness to `WaitingForSnapshot`, center the camera at
   the neutral origin, and close the playable gate.
3. Validate schema version, serialized size, bounded counts, mode requirements, and known profile
   IDs before creating any presentation.
4. On invalid data, remove the previous generation, publish the exact invalid state, close the
   gate, and spawn nothing.
5. On a new valid instance, tear down every entity tagged with the previous instance, install
   `PresentedMap`, and publish Ready.
6. The 3D materializer consumes the accepted snapshot once, creates all exact procedural content,
   and records `Map3dGeneration`.
7. Optional imported modules upgrade only matching fallback children and retain the same generation
   tag. Asset success/failure does not recreate the authoritative map or change readiness.
8. Reconnect, match restart on the same map, map replacement, and disconnect may not leave old map
   members, scene roots, materials unique to an old generation, or stale camera bounds.

### Shape and profile mapping

- `simulation_position(Vec2)` remains the only position conversion to `(x, 0, z)`.
- Simulation rotation remains converted by the tested coordinate API; no call site copies sign or
  axis corrections.
- A resolved 64×64 floor instance spawns a thin shared cuboid or plane at ground level. All 504
  instances share one mesh and one material; only transforms/entities differ.
- Rectangle geometry whose full extents are integral 64-unit cells expands deterministically in
  local placement space and then applies authored rotation/translation. Each cell has exactly the
  authoritative 64×64 footprint.
- Non-integral rectangles use one exact procedural cuboid matching full authoritative extents.
- Circles use one cylinder whose diameter is `2 * radius`; height is presentation-only.
- Perimeter cuboids sit outside, not inside, the playable boundary so they never imply a smaller
  gameplay arena.
- The synthetic inert-decoration profile uses a small cuboid fallback at the exact resolved
  position/rotation. A real model is deferred until a built-in map owns such an entity.
- The central destruction reservation spawns nothing; its actual committed terrain chunks are the
  one representation.
- Spawn areas/points spawn nothing. Team/facing feedback comes from live fighters and HUD, not
  authoring markers.
- Hot Zone uses planar mesh geometry raised only enough to avoid z-fighting. It has no collider and
  does not determine occupancy. Decals are not introduced.

### Terrain mesh lifecycle

`ClientTerrainConvergence` remains the only committed occupancy owner. The 3D presentation system
is the only caller that consumes `take_dirty()` and `take_applied_brushes()` in a windowed client.

For a ready expected generation:

1. On first observation or generation change, rebuild every expected committed chunk.
2. On incremental updates, start from `take_dirty()` and add each allocated orthogonal neighbor,
   because a changed seam can expose or remove the neighbor's side faces.
3. Build top faces only for occupied subcells and side faces only where the adjacent subcell,
   including cross-chunk neighbors, is empty or unallocated. Bottom faces are unnecessary.
4. Reuse the chunk's `Handle<Mesh>` and replace its mesh data for dirty rebuilds. Spawn one handle
   for a newly non-empty chunk and despawn an empty chunk visual.
5. Remove visuals not present in the expected chunk set or whose full generation differs.
6. When convergence waits, becomes invalid, resets, loses its map, or disconnects, despawn all
   affected chunk visuals and debris immediately.
7. Brush feedback consumes applied brushes once, keeps the newest effects within
   `MAX_TERRAIN_DEBRIS_EFFECTS`, and spawns short-lived non-colliding cuboids at render-ground
   height. Debris is tagged with the full terrain generation and expires on time or mismatch.

This replaces M01's compare-all observer and the legacy image painter. It preserves bounded state
and makes seam cost proportional to the changed chunks plus at most four neighbors.

### Camera and input cutover

- Spawn exactly one fixed-azimuth, fixed-elevation orthographic world camera with the M01 accepted
  vertical span, clip range, tone mapping, ambient light, and directional light.
- Retain one higher-order `Camera2d` marked as the default UI camera, with a transparent clear and
  UI render layer. It must not render world entities.
- Follow the latest interpolated controlled fighter. With an accepted map but no controlled
  fighter, target the camera-bounds center. With no accepted map, target the neutral origin.
- Clamp the target against `PresentedMap.camera_bounds` using the projected ground footprint of the
  accepted axis-aligned orthographic camera. Horizontal ground half-span is the orthographic
  half-height times aspect ratio; simulation-Y half-span is half-height divided by the sine of the
  fixed elevation. If an arena axis is smaller than the footprint, center that axis.
- Derive aspect from the active viewport/window and handle zero or unavailable height without NaN
  by retaining the last valid/default aspect.
- `cursor_ground_point` uses `Camera::viewport_to_world` and intersects only the render ground
  plane `Y = 0`. Remove the legacy `Z = 0` fallback.
- Convert the intersection through `ground_point` and feed the existing normalized aim-intent
  path. Mouse/controller inputs still send a planar direction, never a point, height, or hit.
- Input remains suppressed until map, assets, join, and terrain readiness are satisfied.

### Selector and cutover sequence

The implementation order must keep every intermediate commit buildable:

1. Extract the accepted camera/coordinate foundation and install the 3D map/terrain owners behind
   the existing M01 selector.
2. Make the 3D composition unconditional for windowed clients and update affected tests/automation.
3. Remove the conditional 2D map and terrain systems and confirm their state streams have one
   consumer.
4. Delete `WorldPresentationMode`, `ClientNetworkConfig::world_presentation`,
   `--world-renderer`, `BRAWLER_WORLD_RENDERER`, validation/parser tests, and selector docs.
5. Remove unreferenced 2D map/terrain helpers, assets, imports, and tests; retain screen UI and
   M03-owned combat presentation.
6. Run a targeted source audit plus the complete verification matrix before declaring the cutover.

Headless clients do not spawn cameras/assets/presentation and continue to run convergence and
readiness logic. The default headless configuration therefore needs no replacement renderer value.

## Bevy 0.19 API decisions

- Load GLBs as retained `Handle<Gltf>` values and spawn the selected scene with
  `WorldAssetRoot`; do not assume that loading a root alone means its texture dependencies are
  ready.
- Determine imported-family readiness from both root `LoadState` and recursive dependency state.
  Bevy exposes separate root and recursive dependency queries, so a missing colormap must select
  the same fallback as a failed GLB.
- Use `Camera::viewport_to_world` for orthographic mouse rays. For an orthographic camera the ray
  direction is constant; the ray origin varies with viewport position, which is exactly the
  ground-plane aiming requirement.
- Use `OrthographicProjection` with `ScalingMode::FixedVertical`; Bevy updates its projection area
  as the viewport changes. Camera containment still uses the explicit accepted span and observed
  aspect so it can be tested without render-world internals.
- Reuse normal Bevy mesh/material asset handles. No manual instancing or custom render pipeline is
  justified at the current bounded map counts.

## Research sources

### Local primary sources

- `references/bevy/examples/README.md` and
  `references/bevy/examples/3d/orthographic.rs` — official snapshot patterns and the local warning
  that the snapshot is 0.20-dev rather than the pinned application release.
- Cargo-registry `bevy_gltf-0.19.1/src/{assets,material,loader}.rs` — exact `Gltf`, `GltfMesh`,
  `GltfPrimitive`, material, scene, and dependency structures for the pinned release.
- Cargo-registry Bevy 0.19.1 camera and asset sources — exact viewport-ray, orthographic projection,
  root load-state, and recursive dependency-state APIs.
- `src/client/{presentation,presentation_3d,input,assets}.rs`, `src/map/{client,model}.rs`,
  `src/map/definitions/{resolver,terrain}.rs`, `src/terrain/client/`, and `content/v1/maps.ron` —
  current ownership, schedules, bounded content, and every migration/removal seam.
- Mini Arena, Mini Dungeon, and Mini Forest GLBs under `external_assets/` — accessor bounds, nodes,
  texture URIs, previews, and CC0 license files inspected in place.

### Current official references

- [Bevy — Load glTF example](https://bevy.org/examples/gltf/load-gltf/) — scene-label loading with a
  `WorldAssetRoot` and client lighting.
- [Bevy 0.19 `GltfPrimitive`](https://docs.rs/bevy/0.19.0/bevy/gltf/struct.GltfPrimitive.html) and
  [Bevy 0.19 `GltfMesh`](https://docs.rs/bevy/0.19.0/bevy/gltf/struct.GltfMesh.html) — loaded mesh,
  material, and primitive subasset structure; reviewed to reject unnecessary extraction for this
  milestone's 24 static modules.
- [Bevy 0.19 `Camera`](https://docs.rs/bevy/0.19.0/bevy/camera/struct.Camera.html) —
  `viewport_to_world` orthographic-ray behavior.
- [Bevy 0.19 `OrthographicProjection`](https://docs.rs/bevy/0.19.0/bevy/camera/prelude/struct.OrthographicProjection.html)
  — fixed projection behavior, scale, viewport origin, and resize-updated area.
- [Bevy 0.19 `AssetServer`](https://docs.rs/bevy/0.19.0/bevy/asset/struct.AssetServer.html) — distinct
  root, dependency, and recursive dependency load states.

The local pinned sources govern exact implementation. Current official examples are architectural
confirmation only where their code may target a newer Bevy release.

## Implementation checklist

### Composition and assets

- [x] Rename/decompose the M01 feasibility plugin into the default `WorldPresentationPlugin`.
- [x] Move camera/input helpers to the focused camera module without duplicating conversion rules.
- [x] Curate Mini Arena `block.glb`, colormap, and license; update the retained handles and manifest.
- [x] Add `ImportedWorldFallbackPolicy` and family-level deterministic readiness decisions.
- [x] Cache all repeated primitive meshes and material palettes once.

### Map cutover

- [x] Separate renderer-neutral reconciliation/readiness from presentation spawning.
- [x] Implement generation-owned 3D floor instances, perimeter, rectangle/circle geometry,
  synthetic placed-entity fallback, and Hot Zone meshes.
- [x] Implement exact 64-unit Mini Arena block expansion with cuboid fallback and clean upgrades.
- [x] Remove product-world spawn/region debug markers under the selected visibility policy.
- [x] Prove map loss, invalid data, replacement, and reconnect teardown.
- [x] Delete the 2D map spawner and tint path after the 3D owner passes its tests.

### Terrain cutover

- [x] Make the 3D terrain system the sole dirty-chunk and applied-brush consumer.
- [x] Rebuild dirty chunks and orthogonal seam neighbors through retained mesh handles.
- [x] Handle empty/non-empty transitions, generation changes, recovery, reset, and disconnect.
- [x] Replace sprite debris with bounded generation-owned 3D debris.
- [x] Delete image/sprite terrain presentation and its asset ownership after equivalent tests pass.

### Camera, input, and removal

- [x] Install one default orthographic world camera plus the isolated UI camera.
- [x] Implement projected-footprint clamping and map-center/no-map lifecycle behavior.
- [x] Restrict viewport aiming to the render `Y = 0` ground plane.
- [x] Remove selector configuration, CLI/environment parsing, validation, branches, and docs.
- [x] Update README, AGENTS.md, source inventory, commands, and visual workflow for the default 3D
  client.
- [x] Audit that no arena/map/terrain `Sprite`, `Mesh2d`, `ColorMaterial`, or 2D fallback remains.

## Verification plan

### Focused pure and ECS tests

- coordinate position/direction/rotation/extents round trips remain exact;
- 64-unit rectangle module expansion covers each authoritative footprint without gaps/overhang;
- non-integral rectangle and circle fallback dimensions match the authoritative `MapShape`;
- the built-in snapshot produces 504 floor instances, 24 cover modules, one perimeter, no spawn
  debug markers, and the expected Hot Zone count by mode;
- an accepted instance replaces and fully removes the prior `MapPresentationMember` generation;
- invalid/missing snapshots spawn nothing, close readiness, and remove camera bounds;
- optional block ready/failed/forced states select exactly one imported or fallback child;
- forced character, blaster, block, and colormap failure retains a playable primitive scene;
- objective material handles follow Empty/Contested/Team 0/Team 1 without cross-generation
  mutation;
- dirty terrain rebuilds the touched chunk plus allocated orthogonal seam neighbors only;
- an occupied-to-empty chunk despawns, an empty-to-occupied chunk spawns, and retained handles mutate
  in place;
- terrain reset, map replacement, invalidation, and disconnect remove chunks and debris of the old
  full generation;
- debris keeps the newest bounded effects and expires by virtual client time;
- camera clamping covers 16:9, 4:3, 21:9, arena-smaller-than-footprint, and missing/zero viewport
  cases without NaN;
- viewport center and four corner rays intersect `Y = 0` inside the analytically expected ground
  footprint at 16:9, 4:3, and 21:9;
- mouse ray conversion feeds the same normalized planar intent contract as controller aim;
- the UI camera is isolated and exactly one world `Camera3d` exists;
- headless composition has no camera, light, mesh, material, GLB, image, or window dependency.

Time-dependent tests advance Bevy virtual/fixed time explicitly; none use wall-clock sleeps.

### Integration and regression

- both embedded Wipeout and Hot Zone maps resolve and reconstruct through normal client/server
  replication;
- a separate-App network test covers initial map, authoritative terrain mutation, recovery snapshot,
  restart generation, map replacement, and disconnect cleanup;
- existing routed product-match and direct-UDP baseline tests continue to pass unchanged at the
  protocol/authority boundary;
- movement, collision, attacks, scoring, respawn, build selection, map convergence, and terrain
  fingerprints remain identical because no server/wire shape changes;
- role-specific builds prove the server feature graph does not acquire rendering/window/audio/input
  dependencies;
- `just lint` and the complete `just test` suite pass.

### Visual and manual matrix

Run the canonical windowed network workflow in release mode where practical and record screenshots
plus observations for:

| Scenario | Required observations |
|---|---|
| Wipeout at 1280×720 | complete floor/perimeter/cover, readable depth, no old tile sprites or spawn markers |
| Hot Zone at 1280×720 | exact disc/ring, all four objective tint states, fighters/projectiles visible around cover |
| 960×540 (16:9) | camera containment, UI separation, cursor center/corners |
| 1024×768 (4:3) | conservative vertical ground footprint and corner aiming |
| 1680×720 (21:9) | conservative horizontal clamp and corner aiming |
| forced primitive fallback | no partial scene roots, missing texture artifacts, or readiness deadlock |
| terrain destruction at a chunk seam | both sides rebuild, crater remains exact after recovery |
| restart/map replacement/disconnect | no old floor, wall, zone, terrain, or debris generation remains |
| mouse and controller | aim direction agrees with the visible ground plane while moving and near camera bounds |

M02 records visible entity/mesh/material counts as a regression baseline, but final frame-time and
occlusion acceptance remain M04 gates.

## Risks and bounded responses

| Risk | Prevention / response |
|---|---|
| duplicate dirty/brush consumption | install the 3D consumer before deleting 2D, then source-audit for one call site of each `take_*` API |
| imported block footprint drifts from collision | use inspected 1×0.5×1 `block.glb` at uniform scale 64 and retain an exact fallback test |
| colormap fails after GLB root loads | gate upgrade on recursive dependency readiness and keep the fallback until the complete family is ready |
| 504 floor entities regress rendering | share one mesh/material and rely on normal batching first; record counts and defer optimization until measured |
| camera exposes outside arena on unusual aspect | analytic projected-footprint clamp plus 4:3/16:9/21:9 corner tests |
| old generations survive deferred despawn | full instance/generation markers on every root and explicit replacement/disconnect ECS tests |
| M02 absorbs combat conversion | keep temporary M01 actor/projectile systems and track complete replacement in M03 |
| source audit mistakes UI `Camera2d` for world fallback | audit by world ownership/components and document the dedicated UI exception |

## Exit criteria

M02 may move to `User playtest` only when:

- the normal windowed client has no renderer choice and starts directly in the 3D world;
- Wipeout and Hot Zone reconstruct every selected product-visible map family in 3D;
- exact Mini Arena cover and forced procedural fallback both work without readiness deadlock;
- 3D terrain is the sole presentation consumer and passes mutation, seam, recovery, reset,
  replacement, and disconnect tests;
- mouse/controller aiming and camera containment pass the supported aspect matrix;
- a targeted audit finds no 2D arena/map/terrain spawner or `WorldPresentationMode` branch;
- screen-space UI remains functional and the M01 combat proof remains usable pending M03;
- authority/protocol shapes and fingerprints are unchanged, server isolation passes, and all
  canonical lint/test/network checks pass;
- implementation evidence, known limitations, and a clear user playtest scenario are recorded in
  this file.

M02 becomes `Complete` only after user feedback is triaged, affected checks are rerun, and the
learn-from-errors review is recorded. Final combat-world deletion, occlusion tuning, and release
performance are not M02 completion requirements.

## Implementation and verification evidence

Implementation completed on 2026-08-20:

- `WorldPresentationPlugin` is now the only windowed gameplay-world composition. The renderer
  enum, client configuration field, CLI switch, environment selector, conditional plugin branch,
  and legacy map/terrain presentation systems were removed.
- `src/map/client.rs` owns only validated generation reconciliation and readiness. The 3D
  materializer produces 504 shared-mesh floor cells, 24 exact 64-unit cover modules, four
  out-of-bounds perimeter pieces, optional circle/placed-entity fallbacks, and generation-owned Hot
  Zone geometry for the built-in content.
- Mini Arena `block.glb` and its colormap are curated under `assets/brawler/models/kenney/` with CC0
  provenance in the asset manifest. Arena readiness is independent from character/weapon
  readiness. `BRAWLER_FORCE_PRIMITIVE_WORLD=1` provides a verification-only deterministic fallback
  path without changing packaged assets.
- `src/terrain/client/presentation.rs` is the sole dirty-chunk and brush-feedback consumer. It owns
  exposed top/side mesh generation, cross-chunk seam rebuilding, retained mesh handles, full
  generation teardown, bounded cuboid debris, and virtual-time expiry. The former image/sprite
  painter no longer exists.
- Camera framing and clamping are isolated in `src/client/presentation_3d/camera.rs`; cursor aiming
  intersects only `Y = 0`. With no controlled fighter the camera uses the accepted map center, and
  with no map it safely returns to the origin.

Automated evidence recorded on Apple Silicon macOS:

- `just check` passed every routing, client, server, and network-test feature composition.
- `just lint` passed format, routing/client/server Clippy with warnings denied, and the server
  feature-graph isolation check.
- `just test` passed all suites: 357 client-library tests at the canonical run, 301 server-library
  tests, 82 separate-App network scenarios, and 14 performance gates. After adding the explicit
  fallback override test, the complete client-library suite passed again at 358 tests. The terrain
  recovery benchmark rebuilt one exposed mesh chunk in `98.349 µs`; the 100-reset terrain soak
  retained its entity/mesh bounds.
- Focused map materialization tests prove the 504/24/4 built-in counts and invalid-generation
  rejection. Focused terrain tests prove exposed faces, seam ownership, empty transitions, debris
  bounds, recovery, and reset reuse. Camera tests cover 16:9, 4:3, 21:9, zero, and non-finite
  viewports without NaN.
- A source audit found no `Sprite`, `Mesh2d`, or `ColorMaterial` owner in `src/map` or `src/terrain`,
  no `WorldPresentationMode`/renderer branch, and one production consumer each for terrain dirty
  chunks and applied brushes. The dedicated Bevy UI `Camera2d` remains intentional.

Windowed direct-UDP captures exercised the normal replicated path rather than a standalone render
fixture. Wipeout and Hot Zone both reconstructed with imported Mini Arena blocks, depth, UI, actors,
projectiles, and the Hot Zone disc/ring. A second Hot Zone capture with
`BRAWLER_FORCE_PRIMITIVE_WORLD=1` showed exact cuboid cover plus sphere/cuboid actor fallbacks with
no partial imported hierarchy or readiness deadlock. Each bounded visual run reached its requested
screenshot update with server and both clients still live, then the harness intentionally stopped
them at its 18-second verification timeout.

## Known limitations at playtest handoff

- M02 deliberately retains the M01 fighter, weapon, projectile, and basic objective proof. M03
  owns sentries, previews, cue/effect coverage, status/world HUD, animation polish, and systematic
  replacement of the remaining combat-world presentation code.
- The first environment is a functional Mini Arena/primitive composition, not final lighting,
  materials, occlusion, or art direction. M04 owns measured native frame-time and readability
  tuning after the complete 3D world exists.
- Floor cells remain individual shared-mesh entities. The measured tests do not justify custom
  instancing yet.

## User playtest handoff

Run `just run 2`, connect both clients, and play one Wipeout and one Hot Zone match. Please check:

1. cover footprints and collision agree, including movement close to wall corners;
2. mouse and controller aim agree with the visible ground plane near every camera edge;
3. bullets start at the fighter/weapon and remain readable while moving;
4. Hot Zone containment, tint, and readability remain clear around the central terrain block;
5. Arc Launcher terrain damage produces stable 3D craters without stale faces at chunk seams;
6. restart and return-to-lobby leave no old floor, wall, zone, terrain, or debris visible.

For degraded presentation, launch a client with `BRAWLER_FORCE_PRIMITIVE_WORLD=1 just client` and
confirm the match remains fully playable. Visual polish observations are welcome, but M02 feedback
should distinguish functional cutover defects from the planned M03/M04 art/readability work.

## Specification validation

Research and specification completed on 2026-08-20. The user validated the specification and
directed implementation on 2026-08-20. Implementation and repository verification completed the
same day. The first user feedback item entered review on 2026-08-20.

## Feedback review

| Date | Feedback | Decision | Result |
|---|---|---|---|
| 2026-08-20 | Straight bullets visually align with the character only near simulation Y=0; displacement grows when the character moves along Y. | Implement now — this is an M02 3D coordinate/scheduling defect. | Final projectile pose conversion now runs in `PostUpdate` after Lightyear interpolation and Avian writeback, matching fighter pose ownership. A regression test starts from the overwritten legacy XY transform at nonzero Y and verifies the final X/Z-ground transform. The complete 359-test client library suite, client Clippy with warnings denied, formatting, and `git diff --check` pass. |
| 2026-08-20 | Remove the old XY presentation convention rather than relying on 3D systems to repair its `Transform` afterward. | Accepted with staged ownership: M02 removes the obsolete projectile sprite/XY synchronization path because its 3D replacement already exists; M03 moves every remaining gameplay-world visual to dedicated 3D visual entities; M04 verifies complete retirement. The planar server, wire `Position`/`Rotation`, and Lightyear interpolation remain unchanged. | Removed `ensure_projectile_visuals` and `sync_projectile_visuals` from the combat plugin, deleted their sprite/XY implementation and obsolete tests, and retained the 3D nonzero-Y regression. Headless combat evidence continues to read gameplay state rather than render transforms. |

The user confirmed the corrected result and directed M02 closeout on 2026-08-20. Both accepted
feedback items are implemented, affected verification is recorded below, and M02 is complete.

## Feedback verification

- The complete canonical `just check`, `just lint`, and `just test` closeout passed after removing
  the obsolete projectile sprite/XY systems.
- The client library contains 358 passing tests, including the nonzero-Y post-interpolation 3D
  projectile regression and the retained first-observation muzzle catch-up test.
- Headless client/server/network roles remain render-independent, and `git diff --check` passes.

## Learn-from-errors review

1. **The replacement renderer did not become the sole owner soon enough.** M02 installed a 3D
   projectile writer but retained the old sprite/XY writer in the combat plugin. Both mutated the
   same gameplay entity `Transform`, so correctness depended on schedule order. When a migration
   replacement becomes functional, remove the replaced writer in the same milestone or make the
   temporary dual ownership explicit and test its ordering.
2. **A gameplay entity `Transform` carried two incompatible meanings.** The legacy client treated
   it as `(simulation x, simulation y, painter layer)`, while V3 treated it as `(render x, height,
   render z)`. M03 therefore gives dedicated render-only entities ownership of 3D transforms;
   replicated gameplay roots expose interpolated planar `Position`/`Rotation` only.
3. **Centered visual evidence hid an axis-dependent defect.** The initial projectile capture was
   near simulation Y=0, where the stale XY transform looked plausible. Coordinate-migration tests
   and captures must exercise nonzero positive and negative values on both planar axes, not only
   origins and center lanes.
4. **A tactical overwrite fixed the symptom but exposed the ownership flaw.** Moving the 3D write
   after interpolation made the shot correct, but the user correctly challenged why the obsolete
   writer existed at all. Future feedback fixes include an ownership audit before accepting an
   ordering workaround as the final design.

No new general Codex skill was created. These lessons are specific applications of the repository's
existing sole-owner, scheduling, and milestone-closeout rules and are carried into the M03 roadmap
gate.
