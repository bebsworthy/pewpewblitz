# V12 Milestone 01 — proper 3v3 maps

## Status

`User playtest`

The user selected the map-first sequence, supplied one image reference for each supported mode,
directed manual conversion into existing Brawler assets, and approved server-configured map limits
of 20 through 512 cells per axis on 2026-08-26.

## Player-visible outcome

The advertised 3v3 Wipeout, Hot Zone, and Heist paths each use one deliberate mode-owned map rather
than the shared Feature Yard integration geometry. The maps preserve the recognizable lane, cover,
concealment, spawn, and objective structure of the supplied references while using Brawler's
authoritative assets, collision profiles, mode anchors, presentation, and original product naming.
Imported blockers and covers visibly agree with their authoritative cell footprints; deliberately
contained props remain grounded and cannot silently overflow their owned footprint.

## Reference inputs

- Wipeout: `external_assets/map_images/bounty/think_ahead.png`
- Hot Zone: `external_assets/map_images/hot-zone/back_shuffle.webp`
- Heist: `external_assets/map_images/heist/hot-potato.png`

The references are reviewed manually. No importer output or runtime image path enters production.

## Validated viewport and padding contract

The three references each encode a 23×35 logical grid. Production adds one symmetric empty cell on
every side, yielding a 25×37 recipe without scaling or rotating the supplied layout. Image rows are
flipped into Brawler's positive-Y map coordinates and translated by the one-cell inset.

The user identified the original mobile combat framing as approximately 14 visible tiles vertically.
At the existing 27-degree vertical field of view and 55-degree elevation, a camera distance of 743
world units yields approximately 23.82×14.00 visible cells at 16:9. This nearly fills the padded map
width while retaining vertical follow, so wider authoritative padding must not be used to compensate
for presentation framing.

## Validated dimension contract

`config/server/game-types.ron` owns one required `map_dimension_limits` record. The checked-in
operator policy is:

```text
minimum_width  = 20
minimum_height = 20
maximum_width  = 512
maximum_height = 512
```

Shared parsing retains a positive hard ceiling of 512 cells on either axis. Server startup rejects
invalid or inverted limits, a configured maximum above the engine ceiling, and every advertised
map outside the configured envelope. Spawn, reachability, mode-anchor, placement, concealment,
damageable-object, recipe-byte, snapshot-byte, collider, recovery, and bot-work bounds remain
independent gates.

The user rejected a style-specific concealment ceiling on 2026-08-26 and approved rounding the
maximum dimension to the power-of-two 512. Placement capacity now follows the four per-cell asset
slots, concealment may cover every cell, and the bounded resolved snapshot ceiling is 32 MiB. Thus
a 512×512 recipe may contain 262,144 concealing cells; rendering and lookup optimization remain
evidence-driven work rather than an authoring restriction.

## Validated objective-anchor precision

The padded 25×37 Hot Zone reference has its objective at the exact center of the middle cell and a
radius of approximately 3.5 cells. Recipe schema 5 therefore represents Hot Zone centers and radii
as unsigned integer half-cell units. `(2x + 1, 2y + 1)` is the center of cell `(x, y)`, so Back
Shuffle uses center `(25, 37)` and radius `7`; resolution converts those values to authoritative
world coordinates without floating-point authoring ambiguity.

This precision is reusable for geometric mode anchors, where sub-cell geometry has a demonstrated
need. It is deliberately not generalized to walls, grass, cover, spawns, safes, decorations, or
filled rectangles: those assets retain integer-cell placement, footprint, collision, canonical
ordering, and overlap rules. The existing Feature Yard Hot Zone recipe is migrated losslessly from
center `(32, 20)`, radius `5` cells to half-cell values `(64, 40)`, radius `10`.

Because `ResolvedMapSnapshot` replicates the anchor enum and the same three integers now carry
half-cell semantics, this is an incompatible meaning change even though its binary width is
unchanged. The one global application protocol therefore advances from 28 to 29; recipe schema and
canonical fingerprint formats advance from 4/7 to 5/8. There is no legacy decoder.

## Map-authoring contract

- Manually transcribe the references into ordinary sparse `MapRecipe` RON documents.
- Use existing sand/ground, wall, tall-grass, water, destructible-cover, barrier, spawn-marker,
  decoration, damageable-object, and objective-anchor gameplay capabilities only where the
  reference has a clear equivalent. Presentation-only wall and cover variants may share those
  existing gameplay profiles; they do not introduce another authority rule.
- Omit reference-mode objects with no Wipeout meaning rather than inventing rules.
- Record any visually distinct object that lacks a Brawler equivalent before adding or
  approximating it.
- Preserve exactly three safe spawns per team, mode compatibility, fighter-clearance navigation,
  and Heist safe attack sectors.
- Update the map index and only the 3v3 advertised game types; 1v1/2v2 remain unchanged unless the
  user later expands scope.

## Implemented map conversion

- Wipeout advertises **Verdant Crossfire** (`MapPresetId(10)`), derived from Think Ahead.
- Hot Zone advertises **Switchback Basin** (`MapPresetId(11)`), derived from Back Shuffle, with the
  exact half-cell objective center `(25, 37)` and radius `7`.
- Heist advertises **Powderline Vault** (`MapPresetId(12)`), derived from Hot Potato, with mirrored
  three-by-two safe anchors centered at world Y `-400` and `400`.

Each recipe uses one empty-cell perimeter around the transcribed 23×35 reference. Verdant blockers
use the KayKit green symbol block, Switchback blockers use red brick, and Powderline distinguishes
red brick, grey metal, and wood approximations of the reference's solid, metal-lane, and rope-fence
categories. Yellow or green striped blocks present destructible cover while preserving the prior
gameplay profile; tall grass preserves every reviewed concealment group. Powderline Vault's two
blocking cacti retain the dedicated destructible one-cell asset with the Kenney Graveyard Kit
`trunk.glb` visual and the same removal behavior as destructible cover. The Wipeout reference's
central Bounty star remains omitted because Wipeout has no corresponding pickup/scoring rule. No
missing gameplay or presentation primitive blocks the playtest.

## Implementation and verification

- [x] Move map minimum/maximum dimensions into validated server operator configuration.
- [x] Set the checked-in policy to 20×20 through 512×512 and extend hard shared safety to 512×512.
- [x] Keep Practice navigation representable at the configured maximum and add focused limit tests.
- [x] Set and test the accepted 14-cell vertical gameplay-camera target.
- [x] Replace fixed placement/concealment counts with 512×512 dimension-derived structural bounds.
- [x] Add bounded half-cell precision for Hot Zone centers and radii, migrate schema 5 recipes, and
  retain cell alignment for ordinary placements.
- [x] Author, index, resolve, and advertise the Wipeout 3v3 map.
- [x] Author, index, resolve, and advertise the Hot Zone 3v3 map with one typed capture anchor.
- [x] Author, index, resolve, and advertise the Heist 3v3 map with two typed team safes.
- [x] Enforce imported-scene `Exact` and `Contained` fitting from complete intrinsic bounds across
  static, dynamic, objective, and pickup presentation paths; invalid scenes use their fallback.
- [x] Promote the approved KayKit Block Bits subset, add visual-only wall/cover variants, and revise
  the three recipes without changing collision, destruction, concealment, or objective behavior.
- [ ] Run formatting, role checks, focused catalog/navigation/admission tests, canonical tests,
  routed 3v3 and Practice evidence, and native rendering/playtest.
- [ ] Triage gameplay feedback, rerun affected verification, reconcile durable documentation, and
  complete the learning review before closeout.

### Dimension-policy verification evidence — 2026-08-26

- `cargo fmt --all -- --check` passed.
- Server, client, network-test, and Balance Lab role checks passed.
- Server and client Clippy passed with warnings denied.
- The focused dimension-policy, operator-catalog, and 512×512 Practice-navigation tests passed.
- The complete server library suite passed serially: 319 passed, 0 failed. Its earlier parallel run
  exposed the existing global diagnostics/logger test interaction in
  `exit_frame_report_observes_terminal_counts_after_the_shutdown_chain`; that test passed both in
  isolation and in the serial suite.
- The server feature-isolation and V8 canonical-map-cleanup scripts passed.
- `git diff --check` passed.

The serial 319-test server suite above preceded the later power-of-two/capacity revision. After that
revision, the focused 512×512 dimension-policy, invalid-envelope, 1,048,576-slot capacity,
262,144-cell concealment, operator-catalog, and maximum-size Practice-navigation tests passed.
Server and client all-target role checks, formatting, and `git diff --check` also passed. The
14-cell camera-footprint test passed separately under the client role. Full closeout verification
remains pending until the three recipes are authored.

### Objective-anchor precision verification — 2026-08-26

- All 24 map-catalog unit tests passed under the client role, including exact odd-grid half-cell
  conversion, a `(25, 37)` center with radius `7`, lossless Feature Yard migration, embedded recipe
  parsing, canonical fingerprinting, and invalid-anchor rejection.
- All four Hot Zone-filtered authoritative, HUD, and 3D-presentation tests passed.
- All 14 protocol unit tests passed under the combined network-test role after advancing the global
  compatibility version to 29.
- Server and client all-target role checks, formatting, and `git diff --check` passed.

### Three-map implementation verification — 2026-08-26

- All 25 map-catalog unit tests passed under the client role. The new exact-topology test resolves
  all three 25×37 maps, checks three spawns per team, requires concealment, rejects mode-anchor
  drift, and proves the mirrored Heist safe centers at world Y `-400` and `400`.
- All 12 server admission tests passed, including exact preset/revision admission for the three new
  maps and real-schedule objective-bot coverage. The four operator-catalog tests passed with the
  revised golden advertisement and revision digest.
- The focused client 3D test passed for Switchback Basin's exact `3.5`-cell visual radius. The
  focused lobby Practice test proves Hot Zone 3v3 allocates Switchback Basin and five named bots.
- Server, client, and combined network-test all-target checks passed. Server and client Clippy
  passed with warnings denied. Formatting, canonical V8 map-cleanup, and `git diff --check` passed.
- Production-routed headless Practice reached Active with one human and five bots for
  `wipeout-3v3`, `hot-zone-3v3`, and `heist-3v3`; each supervisor/lobby/match-worker process tree
  shut down cleanly.
- Native visual/gameplay acceptance remains the active user-playtest gate.

### Feedback review — cactus visual variety — 2026-08-26

Disposition: **implemented now**. The user clarified that each cactus is a destructible wall used
for visual variety and selected Kenney Graveyard Kit `trunk.glb` as an acceptable source. The shared
catalog now owns stable cactus asset `28`, which is a one-cell blocking feature on sand and reuses
the exact destructible-cover gameplay profile. Client visual profile `43` uses the promoted trunk
model with a green tint and contained one-cell fitting. Powderline Vault recipe revision `2`
replaces the two former garden-wall approximations at cells `(9, 20)` and `(15, 17)`; its admission
revision advances to `2`, and the Heist 3v3 game-type revision advances to `5`.

Focused catalog tests prove both exact cells and gameplay-profile equality with destructible cover.
Client tests prove visual-catalog resolution, promoted-file presence, manifest provenance, and GLB
dependency coverage. Server/client all-target checks, exact manifest admission, the revised
operator-catalog golden, and production-routed Heist 3v3 Practice all passed. Native appearance and
scale remain part of the active user playtest.

## Feedback correction specification — grid-conformant imported assets

### Decision and research

Status: **approved for implementation on 2026-08-26**. The user reported that the environment
assets do not broadly read as occupying the same size and shape as their tiles, approved the
fitting/KayKit plan, and directed implementation.

The audit found that a Brawler cell is 32×32 world units, while imported static scenes currently
receive only a hand-authored scalar. `MapVisualFitting::{Exact, Tiled, Contained}` is parsed but does
not affect scene transforms. Imported dynamic assets also inherit the primitive fallback's
footprint scaling before their own profile scale; the cactus consequently renders at roughly
13×13 inside its 32×32 collision cell. The current arena/garden wall renders at roughly 32×19.2,
the Ashen wall at 32×6.4, and only the dungeon wall happens to cover 32×32. This is implementation
drift from `docs/11-art-and-presentation-direction.md`, which already requires complete intrinsic
bounds validation.

Research and compatibility evidence:

- exact Bevy 0.19.1 source at
  `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_gltf-0.19.1/src/assets.rs`
  exposes the loaded default `WorldAsset`, `GltfNode`, and `GltfMesh` assets;
- the exact loader at
  `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/bevy_gltf-0.19.1/src/loader/mod.rs`
  places the complete converted hierarchy, local transforms, `Mesh3d`, and accessor-derived `Aabb`
  components in that `WorldAsset`;
- `references/bevy/examples/tools/scene_viewer/main.rs` demonstrates unioning transformed mesh AABBs
  after load. Brawler will transform all eight corners rather than use its conservative sphere
  shortcut because footprint admission needs accurate planar extents;
- the local KayKit `License.txt` grants CC0 redistribution and commercial use. All 40 source models
  were inspected; the selected block meshes are centered 2×2×2 cubes with one shared texture and
  exact square planar aspect;
- the official glTF Transform CLI documents `copy` as the minimal glTF/GLB packing operation. The
  implementation will pin `@gltf-transform/cli@4.4.2`, record the exact command, retain source paths
  and hashes, and avoid optimization or geometry rewriting during promotion:
  <https://gltf-transform.dev/cli.html>.

No web API or newer Bevy documentation is needed for the renderer design: the installed Bevy
0.19.1 crate source is the exact production dependency and the checked-in asset pack and license
are the exact source inputs.

### Fitting contract

Imported geometry admission becomes a client-only extension of the existing visual catalog:

1. After a glTF and all dependencies load, walk its default `WorldAsset` hierarchy once. Compose
   every local transform from the scene root, transform all eight corners of every mesh `Aabb`, and
   union them into one finite, positive intrinsic scene bound.
2. Cache the default scene and bound together by `MapVisualProfileId`. No placement repeats this
   inspection, and no bound or asset path enters shared gameplay or the protocol.
3. Interpret imported-profile `scale` as a dimensionless fill factor in `(0, 1]`; advance the
   client visual-catalog schema from 3 to 4 and migrate every imported profile explicitly.
4. `Exact` uniformly scales and recenters the scene so its X/Z bound matches the authoritative
   footprint. Its source and target planar aspect ratios must agree within a small tested epsilon;
   a mismatch rejects the imported scene rather than distorting it.
5. `Contained` uniformly fits the scene inside the footprint and applies the profile fill factor.
   It can be smaller but can never overflow. Decorations, cactus, barrel/chest/debris, the Heist
   idol core, and the restoration pickup use this policy with their owned target box.
6. Grounding offsets the transformed intrinsic minimum Y to the presentation ground before adding
   the profile's deliberate `vertical_offset`; non-centered X/Z pivots are recentered from the same
   bound. Placement rotation remains on the owner root.
7. Imported `Tiled` remains unsupported in this pass because no selected recipe needs a multi-cell
   imported placement. Existing generated water and vegetation retain their current tiled paths.
8. A missing scene, empty/non-finite bound, exact-aspect mismatch, or contained overflow emits one
   bounded diagnostic and selects the deterministic primitive/generated fallback. It cannot hold
   the client in loading or alter authority.

Static placements, dynamic placements and replacements, the Heist safe core, and restoration
pickups call one pure fitting helper. Dynamic owner roots retain translation, placement rotation,
generation identity, visibility, and damage lifecycle at unit scale; imported children receive
only the fitted transform. Primitive children retain their separate footprint transforms. This
removes the cactus's inherited 0.5 scale and the current asset-ID-specific imported-scale
exceptions. Exact primitive wall fallbacks are grounded at their actual half-height instead of the
current unrelated `WALL_HEIGHT` translation.

The fitting work stays in client presentation. `environment_assets.rs` continues to own loading,
readiness, and catalog resolution; a focused `environment_assets/fitting.rs` owns bounds traversal,
fit calculation, and pure tests. Server map assets, colliders, runtime destruction, match rules,
and replication remain unchanged.

### KayKit Block Bits content slice

Promote only these six source models as self-contained GLBs under a retained
`assets/brawler/models/kaykit/block-bits/` namespace:

| Source model | M01 visual role | Gameplay behavior |
|---|---|---|
| `decorative_block_green` | Verdant Crossfire symbol wall | existing indestructible one-cell wall profile |
| `bricks_A` | Switchback Basin and Powderline Vault red/brown wall | existing indestructible one-cell wall profile |
| `metal` | Powderline Vault grey block lane | existing indestructible one-cell wall profile |
| `wood` | Powderline Vault wooden/rope-fence approximation | existing indestructible one-cell wall profile |
| `striped_block_yellow` | orange/yellow destructible boxes | existing destructible-cover profile |
| `striped_block_green` | green destructible boxes | existing destructible-cover profile |

The implementation allocates stable map-asset IDs 29–34 and visual-profile IDs 44–49. They are
presentation variants only: four reuse gameplay profile 2 and two reuse gameplay profile 3. Exact
IDs are recorded with the implementation and never reuse retired IDs. Ground, sand, generated
water, generated tall grass, mode objectives, damageable objects, and the cactus retain their
specialized presentations. Block Bits terrain cubes are not adopted as floors, water, or
concealment, and its foliage cube is not treated as a cactus.

Implemented recipe revisions:

- Verdant Crossfire `1 -> 2`: all green symbol blockers use the decorative green block; orange
  boxes use the yellow striped cover. Admission `1 -> 2`, Wipeout 3v3 configuration `3 -> 4`.
- Switchback Basin `1 -> 2`: red/pink blockers use the brick block; green boxes use the green
  striped cover. Admission `1 -> 2`, Hot Zone 3v3 configuration `4 -> 5`.
- Powderline Vault `2 -> 3`: red/brown blockers use brick, grey lanes use metal, wooden/rope rows
  use wood, orange boxes use yellow striped cover, and the two cacti remain cactus. Admission
  `2 -> 3`, Heist 3v3 configuration `5 -> 6`.

The Heist visual categories are re-transcribed directly from the supplied image and locked by
exact cell-set tests. Recipe dimensions, placements, occupancy, spawns, concealment, safe anchors,
navigation, and gameplay-profile assignment must otherwise remain identical. Map recipe/catalog
schemas and global protocol compatibility remain unchanged; operator catalog digests advance only
because admitted recipe and game-type revisions change.

### Implementation sequence

1. Add intrinsic `WorldAsset` bounds extraction, fit math, cache/error state, and focused pure tests.
2. Route static and dynamic map visuals through the shared helper; remove double scaling and fix
   exact primitive grounding. Then route safe and pickup imported children through the contained
   path and update their regressions.
3. Advance/migrate the client visual catalog and prove every existing imported profile either fits
   its target or deliberately degrades. Do not hide a mismatch by increasing its tolerance.
4. Convert and promote the six approved KayKit sources, record CC0 provenance, source/output hashes,
   conversion command/version, default-scene presence, and manifest dependency evidence.
5. Add the six visual-only map assets and revise the three recipes, admissions, game-type revisions,
   exact topology assertions, and operator golden digest.
6. Run automated verification, then return all three maps for one native gameplay/readability pass.
   Feedback changes only fill factors, model choice, or exact source-cell classification unless the
   user separately approves gameplay changes.

### Verification and playtest gate

Automated evidence must cover:

- hierarchy-aware bounds across translated, rotated, scaled, nested, off-center, empty, and invalid
  synthetic scenes;
- exact 2×2×2 KayKit bounds resolving to 32×32, bottom grounding, contained non-overflow, rotated
  footprints, and identical static/dynamic fitted dimensions;
- cactus no longer inheriting the primitive 0.5 footprint scale;
- all selected GLBs loading with one usable default scene and passing exact footprint admission;
- exact map cell sets for each visual variant, unchanged gameplay-profile IDs, resolved occupancy,
  navigation, objectives, concealment, spawns, recovery, destruction, and admission revisions;
- client/server/network-test checks, warnings-denied Clippy, formatting, manifest/provenance and
  canonical catalog tests, server feature isolation, and the three production-routed Practice
  paths.

Native playtest uses the existing 14-cell vertical viewport and checks all three modes for visible
32×32 blocker coverage, no collider/mesh gaps, no unintended overlap, grounded props, readable
concealment boundaries, cactus and cover removal, objective visibility, and stable frame pacing.
The 512×512 dense-rendering problem remains explicitly deferred until measured content approaches
it; this pass neither reinstates placement limits nor claims one-scene-per-cell performance at the
maximum.

### Grid-fitting and KayKit implementation evidence — 2026-08-26

The visual catalog advanced from schema 3 to 4. Every imported profile now uses a fill factor in
`(0, 1]`; complete `WorldAsset` hierarchy bounds are cached once, and the same fitting helper owns
static map assets, dynamic assets and replacements, Heist idols, and restoration pickups. Dynamic
owner roots remain at unit scale, which removes the cactus's inherited primitive half-scale.
Fallback wall and cover cuboids now use their actual 32-unit height and footprint.

The six source GLTFs were packed without optimization or geometry rewriting with:

```text
npx --yes @gltf-transform/cli@4.4.2 copy \
  external_assets/KayKit_BlockBits_1.0_FREE/Assets/gltf/<source>.gltf \
  assets/brawler/models/kaykit/block-bits/<output>.glb
```

Each output has one default scene, intrinsic bounds `(-1,-1,-1)` through `(1,1,1)` (within source
floating-point precision), and passed `gltf-transform validate` with zero errors and warnings.
The retained pack license is `assets/licenses/kaykit-block-bits.txt`.

| Source / output | Source SHA-256 | Output SHA-256 |
|---|---|---|
| `decorative_block_green.gltf` / `decorative-block-green.glb` | `643bf2a48a1462cb771392c2afc362475e06c765252e0ee86b1f667fdf97d37b` | `ab44d5048a4a1b7aa896ab1f243d87f7b662a98dabde3f51db229629fa5eebd4` |
| `bricks_A.gltf` / `bricks-a.glb` | `275377599f4423f57cbde85fc7b47c81aedd63fd7b4f6b8591b0cbc6437b4dfc` | `e433243ce84b797e6ccc51a7f6282409e3042fa09400cb757bcf52ccfe734f00` |
| `metal.gltf` / `metal.glb` | `d3a00cc6cc3a23f1a930fee433e74fee2515ce37d1748a9ef3f856f3ec1dd1e0` | `c887c4a720f1bcc9eab8cea1e77208d30a7e630a75f5faaf0bf954dfb7e33f76` |
| `wood.gltf` / `wood.glb` | `63ad9bdf917b243aac6ff5b39a3bca160a1abee731bedec9217fb3fa72b248b9` | `0c3c0b9cb45322dbf8868c7654bd2bb2579157c47d193350ecebaadb9b8aeecb` |
| `striped_block_yellow.gltf` / `striped-block-yellow.glb` | `f53b842f9e43404a8c4fa4115c5583193fe21ce9dabb5efbe12dea497fc565a8` | `7db4065ff7d5202550bd41e99b7c85b137ea9889f90ad191804313373a4fbffb` |
| `striped_block_green.gltf` / `striped-block-green.glb` | `a29c3e0da9c630222c5f6bb526dd2497dd8cb3bdc42e875a695b0827db3f5db7` | `4e8ffcb3797a24088ce1853629617ee73c02826e091fa30fcd8570424aecadae` |

Verification passed:

- `just check` for routing, client, server, network-test, Balance Lab, and the web UI;
- `just lint`, including warnings-denied Clippy and the server/renderer/map boundary scripts;
- `just test`: 406 client, 325 server, 335 Balance Lab, 88 network, and 12 performance tests,
  plus routing/process suites and the mixed Balance Lab/network compatibility test;
- seven pure fitting tests covering nested translation/scale/rotation, off-center bounds, exact
  and contained fit, grounding, profile yaw, empty scenes, non-finite input, mismatch, and imported
  tiling rejection;
- exact KayKit visual paths/fitting, retained-license/manifest coverage, exact Powderline metal and
  wood cell sets, unchanged gameplay-profile ownership, recipe/admission revisions, and operator
  catalog digest;
- production-routed `wipeout-3v3`, `hot-zone-3v3`, and `heist-3v3` Practice each reached Active
  with one human and five server bots and then shut down cleanly; and
- canonical native Wipeout 3v3 Practice rendering passed 1,801 samples at 2560×1440 on Apple M3:
  p95 `16.965 ms`, max `17.542 ms`, and zero frames over `25 ms`. All six KayKit scenes admitted;
  three legacy non-square `Exact` wall profiles intentionally selected their footprint-correct
  primitive fallback.

Automated implementation and rendering gates are complete. M01 now waits for the user's native
readability/gameplay pass across all three maps.

## Exit criteria

- all three maps resolve through the ordinary server-owned catalog and exact 3v3 admission path;
- their collision, concealment, spawn, objective, restart, recovery, and bot-navigation behavior
  is valid and readable;
- imported blockers agree with their cell footprints, contained props remain within them, invalid
  scenes degrade safely, and the selected KayKit family is accepted or explicitly revised from the
  native playtest;
- the configured dimension envelope fails closed and does not become a client-authored rule;
- the user can play each map and every feedback item receives an explicit disposition; and
- M02 remains unstarted and unspecified until the user defines the balancing-tool changes.
