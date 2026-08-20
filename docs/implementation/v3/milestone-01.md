# Milestone 01 — 3D presentation feasibility and foundation

## Tracking

| Field | Value |
|---|---|
| Status | Complete |
| Prepared | 2026-08-20 after the user reopened the validated 2D art direction and selected V3 for a complete 3D gameplay-world presentation migration |
| Implementation started | 2026-08-20 after explicit user approval |
| Completed | 2026-08-20 after user acceptance, feedback fixes, final regression checks, explicit deferrals, and learning review |
| Objective | Prove a readable, performant, client-only 3D presentation over the unchanged 2D authoritative simulation and leave production-reusable foundations for the cutover |
| Entry dependency | Satisfied — V2 closeout and explicit M01 specification approval were received on 2026-08-20 |
| Scope authority | The user selected primitives for the initial migration, supplied CC0 Kenney environment, character, and weapon packs, accepted the resulting technical design, and directed M01 implementation to begin |

## Outcome statement

M01 must answer one question with playable evidence:

> Can the current authoritative Brawler match be presented as a fixed-camera 3D arena without
> changing its 2D gameplay model, network protocol, or server feature boundary?

The expected answer is yes. The milestone proves that answer through the real client/server path,
not through a separate Bevy demo. Its code is a reusable foundation, but it does not make an
incomplete 3D renderer the permanent product default. M02 owns the default cutover and systematic
removal of replaced 2D arena/map/terrain paths.

If M01 shows that acceptable aiming, camera containment, objective readability, terrain updates,
or occlusion requires authoritative height or 3D physics, implementation stops and the
specification returns for review. M01 must not silently broaden the gameplay model.

## Selected first-release visual language

| Gameplay/presentation family | M01 representation | Reason |
|---|---|---|
| Fighter | One curated Mini Characters GLB at validated scale; sphere with authoritative body diameter plus direction marker as fallback | The supplied rig is suitable for first-release presentation, while the primitive preserves deterministic degraded behavior and shape debugging |
| Controlled fighter identity | Thin ground ring plus existing screen-space HUD/team treatment | Keeps “you” and team identity readable without making model shape authoritative |
| Rectangular permanent wall | Repeated curated 1-unit Mini Arena/Dungeon wall or block module where it exactly tiles the resolved footprint; cuboid fallback | Uses the available style without stretching one decorative mesh across arbitrary geometry or weakening collision readability |
| Circular permanent geometry | Vertical cylinder with the exact authoritative radius | A cuboid would falsely imply blocked corners |
| Ground/floor | One planar rectangle or bounded map mesh on Y=0 | The gameplay floor is flat and non-authoritative presentation needs no tessellation |
| Straight projectile | Short horizontal cylinder or capsule aligned to planar travel, with diameter derived from authoritative projectile radius | More readable direction and speed than a sphere or cube; extra length remains presentation-only |
| Lobbed projectile | Sphere with planar X/Z from existing flight state and Y from the existing `lob_height` visual curve | Makes the already-authored arc genuinely vertical without changing collision or landing |
| Equipped weapon | One curated Blaster Kit GLB attached to the character's `arm-right` descendant through a per-weapon local transform profile | The character has holding/shoot clips and stable arm node names, but neither pack supplies a shared weapon socket or guaranteed grip transform |
| Sentry sample, if needed by the playtest | Cylinder base plus cuboid/cylinder barrel | Exposes facing with primitives and reuses cached shapes |
| Hot Zone circle | Procedural fill disc plus annulus at a small positive ground offset | Exact boundary, tintable state, portable on macOS, no decal pipeline |
| Rectangular area | Procedural fill quad plus border strips/outline mesh at the same offset | Exact parity with `MapShape::Rectangle` |
| Destructible terrain sample | One combined extruded mesh per visual chunk, rebuilt from committed occupancy | Proves real terrain depth and reuses current dirty-chunk ownership without one entity per cell |

Primitive colors preserve the current readability language: cool and warm team colors, neutral dark
floor, strongly separated cover, gold-neutral empty objective, explicit contested color, and team
color only while controlled. Materials start matte and low-metallic. One restrained directional
light plus ambient fill is preferred over multiple dynamic lights.

### Hot Zone: mesh, not decal

Hot Zone is a gameplay area with an exact server-owned shape, not surface decoration. Its M01
visual is procedural geometry:

- the fill disc ends at the authoritative radius;
- the annulus boundary is centered on that radius, so the rule edge is visually unambiguous;
- the mesh sits at a small named Y offset above the floor to avoid z-fighting;
- material color/emissive state derives from replicated Hot Zone status;
- the mesh has no collider and never participates in occupancy;
- reduced-effects mode may reduce animation but never remove the boundary.

Bevy 0.19 clustered decals require bindless support and are disabled on macOS/iOS. Forward decals
are available but require a depth prepass and solve projection across irregular surfaces, which the
flat objective does not need. A thin real mesh is smaller, exact, testable, and portable. Future
cosmetic markings may revisit decals independently.

## Available Kenney asset evidence

All seven staged packs declare CC0 in their local `License.txt`. They provide parallel GLB, FBX,
and OBJ exports; V3 selects GLB only. The overview counts and direct GLB inspection produced this
inventory:

| Pack | GLBs | Animation clips across files | Useful first-release role | M01 disposition |
|---|---:|---:|---|---|
| Mini Arena | 22 | 25 | 1-unit floor/wall/block modules, arena props, animated soldier | Import one modular wall/block candidate for scale and tiling proof |
| Mini Characters | 26 | 384 | Twelve animated character variants plus accessibility props | Import one character and its required colormap; primary attachment/animation proof |
| Mini Dungeon | 30 | 73 | 1-unit floor/wall modules, small props, human/orc rigs | Compare one wall against Mini Arena; no complete theme import |
| Mini Forest 1.0 | 22 | 32 | Trees, rocks, structures, ground patches, animated archer | Import at most one sparse prop if needed for occlusion/scale evidence |
| Blaster Kit 2.1 | 40 | 9 | Eighteen blasters, bullets, grenades, accessories, animated crates | Import one representative compact blaster; prove hand attachment |
| Pirate Kit 2.1 | 72 | 3 | Large tropical/fortress/ship theme | Research-only in M01 because its source dimensions differ substantially from the Mini 1-unit modules |
| Graveyard Kit 5.0 | 91 | 166 | Graveyard walls/props plus animated creatures | Research-only in M01; candidate later theme/character source |

The totals count clips repeated in independently exported character files; they are not 689 unique
gameplay motions. Mini Characters models share the same seven named bones (`root`, legs, `torso`,
arms, and `head`) and 32 named clips. Direct inspection of `character-male-a.glb` found two skinned
meshes, two skins over the same seven joints, and useful clips including `idle`, `walk`, `sprint`,
`die`, `holding-right`, `holding-both`, and matching shoot clips.

The packs are stylistically compatible but not universally scale-identical. Mini character bounds
are roughly 0.78 units wide and 0.67–0.84 units high; Mini Arena/Dungeon modular floors and walls
use approximately 1-unit footprints. Pirate pieces are often several source units across/high.
Every selected presentation profile therefore declares a reviewed source bound, uniform world
scale, forward axis, ground offset, and allowed placement policy. V3 does not infer gameplay
collision from imported bounds.

### GLB dependency and namespace finding

The files are GLB containers, but their textures are external rather than embedded. Every inspected
model refers to `Textures/colormap.png`; each pack's 512×512 colormap has a different content hash.
Many packs also reuse model basenames such as `wall.glb`, `floor.glb`, `tree.glb`, `rocks.glb`, and
`weapon-sword.glb`.

Runtime import must therefore preserve a structure such as:

```text
assets/brawler/models/kenney/<pack-key>/
  <selected-model>.glb
  Textures/colormap.png
```

`external_assets/` remains the unmodified upstream source. FBX, OBJ, previews, samples, and unused
GLBs do not enter the shipped asset root. The generic Kenney isometric-tile instructions do not
apply to V3's world renderer, and these local packs contain 64×64 per-model preview thumbnails
rather than a runtime 512×512 isometric tile set. A future map-builder catalog may intentionally
reuse curated thumbnails, but they never replace GLB geometry or authoritative map shapes.

## Scope

### In scope

1. Add only the client-side Bevy features required for primitive and selected GLB rendering,
   expected to include `bevy_pbr`, `bevy_gltf`, and `gltf_animation` plus their transitive
   mesh/light/world-serialization/animation support. Do not add clustered decals or unrelated scene
   features without an M01-owned use.
2. Establish one tested simulation-to-render coordinate contract for positions, directions,
   rotations, extents, and ground bounds.
3. Add a fixed orthographic `Camera3d`, retained screen-space UI composition, camera following, and
   ground-footprint clamping.
4. Replace 2D cursor conversion in the selected 3D composition with `viewport_to_world` followed by
   ray intersection against the Y=0 gameplay plane.
5. Cache primitive meshes and shared materials in client-owned resources.
6. Import a namespaced, manifest-recorded subset containing one Mini Characters fighter, one
   Mini Arena/Dungeon environment module, one Blaster Kit weapon, each required pack colormap, and
   copied CC0 license/provenance records. Retain primitives as fallbacks.
7. Load static and animated GLB scenes with dependency-aware readiness, validate their expected
   scene/node/skin/animation contract, and attach the weapon after scene instantiation.
8. Prove a deliberately small animation path: idle/walk locomotion plus one holding and shoot clip.
   M01 records whether an upper-body mask/blend is necessary; it does not build a general animator.
9. Present a representative real match with floor, permanent rectangle/circle cover, imported and
   fallback fighters/walls, an equipped blaster, straight projectiles, one lobbed projectile, Hot
   Zone, and one destructible-terrain section.
10. Read replicated/interpolated 2D positions and rotations without changing protocol registration
   or server components.
11. Use the existing match/cue/terrain lifecycle so disconnect, map replacement, terrain reset, and
   match restart remove their 3D presentation members.
12. Compare two bounded camera compositions, select one, and record its constants:
   - an axis-aligned angled top-down view that keeps map X horizontal on screen;
   - a diagonal isometric candidate with approximately 45-degree azimuth.
   Elevation, orthographic span, wall height, and shadow strength may be tuned within those two
   compositions. M01 does not create a general camera settings system.
13. Observe north/south wall occlusion, projectile visibility, fighter overlap, objective boundary
    readability, and shadow clarity under representative play.
14. Measure a release-profile native macOS run and record hardware, resolution, present/render
    profile, scene counts, frame-time distribution, and fixed-tick behavior.
15. Preserve the supported 2D product composition until the user accepts the feasibility result.
    The M01 selector is development-only and is removed with the M02 default cutover; it is not a
    permanent user setting or a generalized renderer interface.

### Out of scope

- Avian 3D, `Vec3` gameplay positions, vertical collision, jumping, ramps, walkable elevations, or
  height-aware projectiles;
- protocol changes, new compatibility schema, replicated render transforms, or model/mesh IDs;
- bulk import of all 303 GLBs, alternate FBX/OBJ exports, complete environment themes, all character
  variants, all blasters, skin selection/replication, original model authoring, or a general asset
  conversion framework;
- perspective projection, orbit/zoom controls, cinematic cameras, split screen, or camera shake;
- a general decal system, clustered-decal feature, custom render pipeline, deferred rendering,
  postprocessing stack, GPU particles, LOD, or instancing framework;
- final conversion of every map, terrain, combat, preview, cue, status, and world-HUD visual;
- deleting the 2D renderer before feasibility acceptance;
- changing balance, geometry, terrain resolution, movement, combat timing, or match rules;
- unrelated V2 backlog work.

## Current seams and ownership

### Preserved authoritative seams

- `src/protocol.rs` registers Avian 2D `Position` and `Rotation`; these remain the wire pose.
- `src/movement/` owns fixed-tick movement, the 24-unit baseline fighter radius, collision layers,
  and camera span constants that currently also influence presentation.
- `src/map/model.rs` owns `MapShape`, resolved geometry, mode anchors, camera bounds, and stable
  presentation profile IDs.
- `src/map/server.rs` and `src/terrain/` own authoritative permanent/destructible collision and
  committed occupancy. M01 observes them only.
- `src/combat/delivery.rs::lob_height` already expresses the presentation arc independently of
  collision and remains the source of vertical lob display.
- `src/client/assets.rs` owns retained handles, dependency readiness, deterministic fallbacks, and
  CC0 manifest validation; the GLB subset extends this ownership rather than adding another loader.
- Lightyear interpolation continues to produce the pose that client presentation consumes.

### Client ownership after M01

- A focused client presentation module owns coordinate conversion, selected camera constants,
  ground-plane ray conversion, cached primitive handles, shared materials, and lights.
- Client asset ownership retains the selected `Handle<Gltf>`/scene/animation handles, gates on
  `AssetServer::is_loaded_with_dependencies`, and maps stable presentation profile keys to curated
  asset paths and local transforms.
- Existing concern owners remain responsible for their visuals:
  - map presentation owns floor, walls, anchors, and Hot Zone meshes;
  - terrain presentation owns occupancy-derived chunk meshes and dirty rebuilds;
  - movement/client presentation owns the camera and fighter visual attachment;
  - combat client presentation owns projectile and lob visuals.
- Shared helpers never discover map/combat lifecycle on their own and never mutate authoritative
  components.
- Visual geometry should normally be attached as render-only children of replicated roots. This
  lets the root retain its canonical 2D/interpolated pose while the child receives X/Z conversion,
  height, model-forward correction, and presentation animation.

The exact file split is chosen during implementation from these ownership boundaries. M01 must not
turn the already-large `src/client/presentation.rs` or any `mod.rs` into a dumping ground.

## Curated asset ingestion contract

- Import from the GLB-format directory only. Do not convert through FBX/OBJ or modify the upstream
  files merely to make the first slice load.
- Copy only selected GLBs plus the exact pack-local `Textures/colormap.png` they reference. Preserve
  pack namespaces and relative case-sensitive paths.
- Record every selected model and dependent texture in `assets/manifest.ron`, with pack/version,
  original relative path, Kenney, CC0-1.0, official pack URL, import date, required/degraded policy,
  and primitive fallback. Copy an appropriate pack license record into `assets/licenses/`.
- Update manifest validation to accept the new truthful import date rather than preserving its
  current single hard-coded 2026-08-15 value. Keep date syntax and required visual fallback checks
  strict.
- Retain one `Handle<Gltf>` or typed scene/animation handle set per selected asset in the existing
  client asset resource. Readiness includes dependencies because the colormap loads separately.
- A failed or incompatible imported model degrades to its primitive fallback and reports the stable
  manifest/profile ID. It must not prevent the authoritative match from running indefinitely.
- Validate expected scene count, node names, skins, named animation clips, material/texture
  dependency, finite source bounds, and source generator in a focused import contract. Do not rely
  on numerical animation indices without also validating the expected names/order for this exact
  file revision.
- Static repeated pieces share loaded mesh/material assets. Do not load or clone the GLB file per
  placement, create a material per wall segment, or spawn a full scene hierarchy for a four-vertex
  floor tile when a shared mesh/procedural ground is clearer.
- Imported source bounds are presentation metadata only. Authoritative `MapShape`, fighter radius,
  projectile radius, and terrain occupancy remain the only gameplay footprints.

## Character animation and weapon attachment contract

Direct GLB inspection supports combining Mini Characters with Blaster Kit, with one caveat: the
character scenes expose named `arm-left`/`arm-right` bones and holding/shoot animations but no
explicit weapon socket, while blasters are separate unskinned scenes with their own origins.

M01 therefore uses this small adapter:

```text
WeaponPresentationProfile
  model_asset_id
  attachment_bone_name = "arm-right"
  local_translation
  local_rotation
  uniform_scale
  muzzle_local_position
  holding_clip = "holding-right" | "holding-both"
  shoot_clip = "holding-right-shoot" | "holding-both-shoot"
```

After `WorldAssetRoot` reports `WorldInstanceReady`, the owning fighter presentation searches only
that scene instance's descendants for the validated attachment bone and `AnimationPlayer`, attaches
one weapon visual, and installs the small animation graph. It never queries an arbitrary global
entity named `arm-right`.

The first profile selects one compact blaster and tunes its transform by inspection. That proves
compatibility; it does not claim every Blaster Kit model shares a grip origin. Each later weapon
profile must be individually reviewed. The weapon remains a visual child: recoil, muzzle position,
and animation never decide authoritative firing origin, aim, hit geometry, or timing.

M01 uses supplied named clips rather than retargeting. It tests `idle`, `walk`, one holding pose, and
one shoot clip. If locomotion plus holding requires concurrent graph nodes or an upper-body
animation mask, M01 implements only the smallest graph/mask for these shared seven bones and records
the result for M03. Jump/fall/crouch/wheelchair/emote/melee completeness remains outside M01.

## Coordinate and transform contract

M01 defines a small pure API rather than scattering `Vec2::extend`, sign changes, and yaw formulas:

```text
ground_position(Vec2) -> Vec3
ground_direction(Vec2) -> Vec3
ground_rotation(Rotation) -> Quat
ground_point(Vec3) -> Vec2
ground_extents(Vec2) -> Vec3
```

The selected convention uses Bevy Y as render height and the X/Z plane as gameplay ground. The
adapter preserves the simulation's handedness and makes the presentation root's declared forward
axis explicit. Pure tests must prove:

- position round trips on representative positive, negative, boundary, and origin values;
- simulation X/Y basis directions map to the intended render-ground directions;
- zero, quarter-turn, half-turn, and arbitrary finite angles point the visual marker in the same
  planar direction as authoritative aim;
- rectangle dimensions and rotations preserve their authoritative ground footprint;
- presentation-only Y never survives conversion back to gameplay coordinates.

Root pose writeback must have one schedule owner and remain after Lightyear interpolation and
Avian writeback but before transform propagation, preserving the current ordering contract.

## Camera and input contract

### Projection and framing

The baseline uses `Camera3d` plus `Projection::Orthographic` and
`OrthographicProjection::default_3d()` with fixed vertical framing. Orthographic projection is
selected because it preserves apparent fighter/projectile size across the arena and keeps aiming
and collision silhouettes legible.

The camera follows a ground target by applying one fixed offset and looking at that target. It does
not copy the fighter's height or rotation. Camera containment can no longer assume that viewport
width/height directly equal an axis-aligned ground rectangle: M01 projects viewport corners onto
the Y=0 plane and derives or validates the ground footprint used for clamping. Unsupported/invalid
viewport states retain the last valid center rather than producing a non-finite pose.

M01 records the chosen:

- elevation and azimuth;
- orthographic fixed span/scale;
- target offset if the controlled fighter is not centered;
- near/far planes sufficient for the complete bounded scene;
- wall and terrain height;
- shadow direction, strength, and bias;
- camera bounds behavior on maps smaller than the projected footprint.

### Mouse aim

For the 3D composition:

1. get the cursor position from the primary window;
2. get the arena `Camera3d` and its `GlobalTransform`;
3. call `Camera::viewport_to_world`;
4. intersect the returned ray with `InfinitePlane3d::new(Vec3::Y)` at `Vec3::ZERO`;
5. convert the intersection X/Z to simulation `Vec2`;
6. subtract the authoritative/interpolated controlled fighter position and retain the current
   finite/nonzero validation, normalization, and aim-distance behavior.

Rendered walls, fighters, and effects do not participate in cursor picking. Aim always intersects
the gameplay plane, so a tall wall cannot redirect intent.

Controller/right-stick aim remains planar input and should need no semantic change.

## Presentation lifecycle and materials

- Primitive mesh handles are created once per shape/resolution and shared.
- Imported GLB/scene/animation handles are loaded once by the retained client asset resource and
  scene descendants are reconciled only after `WorldInstanceReady`.
- Material handles are shared by semantic role/team/status. A team or objective-state change swaps
  handles or updates an intentionally owned shared palette entry; it does not allocate every frame.
- M01 uses simple `StandardMaterial` values, low metallic response, restrained roughness, and an
  unlit/emissive option for boundary-critical ground indicators if lighting obscures them.
- Transparent ground fills remain bounded and are tested from the selected fixed camera. The
  objective boundary remains visible if the fill's alpha sorting is imperfect.
- Directional light/shadow entities belong to the 3D world generation and do not survive leaving
  the gameplay scene. Ambient fill is a client presentation resource.
- Every spawned visual carries the existing map/terrain/match generation marker or an equivalent
  concern-owned marker so reset and replacement are deterministic.
- Missing render resources in a headless/separate-App test cause presentation to skip safely; they
  never block gameplay readiness on the server.

## Destructible terrain feasibility contract

M01 does not replace the complete terrain renderer, but it must prove the hard representation:

- committed occupancy bits remain the sole input;
- one visual chunk owns one combined mesh rather than up to 1,024 cell entities;
- occupied cells emit a top at the selected terrain height and only externally visible side faces;
- orthogonal neighbor information closes or opens faces at chunk seams;
- the existing dirty chunk plus neighbor invalidation causes bounded mesh rebuilds after a brush;
- mesh replacement updates or invalidates culling bounds as required by Bevy;
- the mesh has no 3D collider and never becomes authoritative;
- generation replacement and convergence invalidation despawn it exactly as the current chunk
  image lifecycle does.

The first implementation may use flat colors and does not need UVs when its selected material does
not sample a texture. Normals and winding must still be correct for lighting and back-face culling.

## Schedule and plugin composition

The accepted implementation keeps these phases explicit:

```text
asset dependency load / WorldAssetRoot scene instantiation
    -> instance-local bone, AnimationPlayer, and weapon attachment setup
Lightyear interpolation / Avian presentation pose
    -> convert replicated/interpolated 2D roots to 3D render transforms
    -> reconcile concern-owned render children, animation state, and material changes
    -> update render-time lob height/transient visuals
    -> follow and clamp Camera3d
    -> Bevy transform propagation / rendering
```

Input remains sampled before the fixed-tick bridge through the established client input schedule.
The ground-ray helper replaces only the world-coordinate calculation for the selected 3D camera.
No 3D system is added to `FixedUpdate` unless it merely observes fixed-tick facts; visual animation
uses render time in `Update`/`PostUpdate`.

Server and headless app composition must not register the 3D plugin or initialize PBR assets.
Asynchronous scene readiness is an observer/lifecycle boundary, not a reason to poll or rebuild the
fighter hierarchy every frame.

## Implementation slices

1. **Dependency and isolation proof**
   - enable the minimum Bevy client 3D/GLB/animation features;
   - compose a `Camera3d`, ground mesh, cached primitive/material resources, and one light;
   - prove server-only check and feature audit remain clean.
2. **Curated Kenney import contract**
   - select one character, compact blaster, and modular environment piece;
   - copy only their GLBs, distinct pack colormaps, and license records under pack namespaces;
   - extend manifest/readiness validation and prove dependency-aware load plus primitive fallback;
   - inspect and test scene, node, bone, clip, material, bounds, and forward-axis assumptions.
3. **Coordinate, camera, and input foundation**
   - implement/test conversion helpers;
   - implement both bounded camera candidates;
   - add ground-footprint clamp and cursor-ray tests;
   - select the final constants after screenshots and hands-on aiming.
4. **Map and objective representative slice**
   - floor, repeated imported wall/block module with cuboid fallback, rectangle/circle cover,
     perimeter, Hot Zone fill/boundary and status tint;
   - generation-owned spawn/despawn and exact shape assertions.
5. **Fighter, weapon, animation, and projectile representative slice**
   - imported character plus sphere fallback, direction marker, and controlled/team distinction;
   - dependency-ready scene discovery, animation graph, selected arm attachment, blaster profile,
     muzzle marker, idle/walk/holding/shoot proof;
   - straight cylinder/capsule aligned to travel;
   - lob sphere with real render height;
   - replicated/interpolated pose and cleanup tests.
6. **Terrain depth slice**
   - combined exposed-face mesh for a representative chunk;
   - dirty update, seam, mutation, reset, and culling-bound verification.
7. **Networked feasibility and measurement**
   - real Wipeout and Hot Zone paths with two clients where practical;
   - release-profile native capture and frame/fixed-tick evidence;
   - user playtest of selected camera, imported/fallback readability, animation/weapon fit,
     occlusion, aim, scale, projectiles, and objective edge.
8. **Decision closeout**
   - record accepted constants, misses, fixes/deferments, and learning;
   - either approve M02 cutover preparation or return V3 to specification review.

## Implementation progress

### 2026-08-20 — Active-version handoff and dependency isolation

- Promoted the V3 roadmap and M01 from `Specification review` to `Implementing` after explicit
  user approval and the already-accepted V2 closeout.
- Updated `AGENTS.md`, the root README, the documentation index, and the historical sprite
  inventory so V3 M01 is consistently identified as the current implementation contract.
- Enabled Bevy 0.19.1 `bevy_pbr`, `bevy_gltf`, and `gltf_animation` only through the existing
  `bevy-client` feature. Cargo resolved the expected PBR, glTF, animation, and world-serialization
  dependencies without adding them to `bevy-server`.
- Verified `cargo check --no-default-features --features client`,
  `cargo check --no-default-features --features server`, and
  `scripts/check-server-features.sh`; all passed.
- Added the explicit `--world-renderer 3d` / `BRAWLER_WORLD_RENDERER=3d` development selector. The
  normal product default remains `2d`, and a headless process rejects the 3D composition.
- Added the tested simulation `(x, y)` to render `(x, 0, -y)` adapter, fixed 55-degree
  axis-aligned orthographic camera, explicit 0.1/3,000 clip range, conservative ground-footprint
  clamp, and camera-ray/Y=0 mouse aiming.
- Added shared primitive/material resources; 3D floor, perimeter, rectangle/circle cover,
  fighter fallback/facing cue, straight and lobbed projectiles, procedural Hot Zone geometry, and
  occupancy-derived exposed-face terrain chunk meshes.
- Curated and manifested `character-male-a.glb`, `blaster-a.glb`, and Mini Arena `wall.glb` with
  their separate colormaps and CC0 license copies. Dependency-aware load state is reported, the
  four selected character clips build one animation graph, and the blaster attaches beneath the
  owning character's `arm-right` descendant.
- Added Bevy's client-only scene/reflection features required by 0.19 `WorldAsset` GLB spawning.
  The PBR/scene/glTF/animation graph remains absent from the dedicated-server feature graph.
- In 3D mode, map validation/readiness remains shared but no legacy map `Mesh2d`/sprite entities
  are spawned. The 2D renderer remains the default supported path until M02.
- Extended the bounded direct-UDP visual harness with first-client screenshot scheduling and made
  `--combat-demo` auto-ready both visual clients so active-match evidence is reproducible.

### 2026-08-20 — First live 3D evidence and corrections

- The first attempted environment-selected run exposed that the CLI parser overwrote the
  environment-derived renderer with `2d`; the parser now preserves the environment value unless
  an explicit CLI value replaces it.
- The first true 3D run exposed missing reflected scene registration and a camera distance beyond
  the default far plane. Adding `bevy_scene`/`reflect_auto_register`, disabling unavailable
  LUT-dependent tonemapping, and recording the explicit clip range removed both failures.
- Active two-client Wipeout and Hot Zone captures completed without a client panic or stale 2D
  world. Wipeout showed the imported character/blaster, repeated imported walls, projectile cue,
  permanent cover, perimeter, and terrain depth. Hot Zone additionally showed the procedural fill
  and exact annulus boundary around the authoritative central area.
- Current visual evidence is deliberately feasibility-grade: Kenney wall brightness, fighter
  scale/edge framing, ground material contrast, and general presentation polish remain user
  evaluation inputs, not M01 release-art claims.
- The initial user visual review accepted the overall feasibility result for later tuning and
  identified one immediate readability defect: straight shots appeared above and detached from
  the fighter muzzle. This was accepted for M01. The Kenney character and blaster native forward
  axes are now normalized to the fighter root's +X facing, while straight projectiles use only a
  radius-sized ground clearance instead of the former 20-unit lift. Lobbed projectiles retain
  their launch height and authored vertical arc. A follow-up capture showed the remaining temporal
  gap: by the first replicated frame, a 900-unit/second shot had already travelled well beyond its
  muzzle. Straight-shot presentation now starts from replicated `StraightFlight.origin` and
  catches up to the current replicated position at a bounded 3x visual speed without overshoot. A
  repeat live run completes with the blaster, facing cue, and straight shot on the same apparent
  firing line and a visible client-only launch, without changing authoritative pose, collision,
  hit timing, or protocol state.
- `just lint` passes, including all-target Clippy and server feature isolation. The final
  `just test` rerun initially hit the pre-existing
  `malformed_source_is_suppressed_without_allocating_workers_or_replies` routing test at 30/32,
  then that exact test passed immediately on retry. A complete retry passed: 83 routing unit tests
  plus routing process suites, 362 client tests, 302 server tests, 82 serial network integration
  tests, and 14 fixed-tick performance gates.
- After both accepted projectile-origin corrections, the closeout reran the canonical paths on the
  final code: `just lint` passed formatting, all-target client/server/routing Clippy, and server
  feature isolation; `just test` passed 83 routing unit tests and all routing process suites, 363
  client tests, 302 server tests, 82 serial network integration tests, and 14 fixed-tick
  performance gates. The performance gates measure headless fixed-tick work, not native 3D render
  frame time; render profiling remains explicitly transferred to M04.

## Verification plan

### Pure and ECS tests

- coordinate position/direction/rotation round trips and basis directions;
- X/Z ground footprint and exact rectangle/circle dimensions;
- render height cannot alter simulation position;
- projectile cylinder/capsule orientation follows finite planar travel direction;
- lobbed sphere uses the existing parabola and returns to ground presentation height at both ends;
- Hot Zone fill and annulus derive from the exact replicated radius and preserve status colors;
- camera ground-footprint/clamp behavior at representative aspect ratios and small/large bounds;
- cursor center/corners produce finite expected plane intersections for the selected camera;
- mesh/material resources are shared rather than growing per spawned entity or frame;
- curated GLBs preserve pack-relative colormap dependencies and load with dependencies;
- imported contract validation finds the expected scene, finite bounds, seven character bones,
  two skins, selected named clips, material, source revision/hash, and forward-axis metadata;
- `WorldInstanceReady` setup finds `AnimationPlayer` and `arm-right` only beneath the owning fighter,
  installs the graph once, and attaches exactly one weapon; missing/duplicate contract nodes select
  the primitive fallback without cross-attaching between fighters;
- idle/walk/holding/shoot transitions remain render-time presentation and clean up with the fighter;
- repeated wall/model placements share handles and keep the authoritative resolved footprint
  visually covered without non-uniformly stretching decorative models;
- map/match/terrain generation replacement removes every owned 3D entity;
- terrain chunk mesh emits only required exposed faces, observes seam neighbors, rebuilds dirty
  chunks, and refreshes culling bounds;
- plugin/schedule tests preserve interpolation -> pose -> camera -> propagation ordering;
- a headless client/separate `App` without render assets skips presentation safely.

### Build and regression checks

- canonical formatting and lint paths;
- client tests with the selected 3D features;
- server-only check and the repository's forbidden client/render dependency audit;
- existing server, network integration, and performance tests sufficient to prove protocol and
  simulation behavior are unchanged;
- targeted source/protocol diff confirming no `Vec3`, `Transform`, height, mesh, or material was
  registered for replication.
- asset/provenance tests confirming only selected runtime GLBs/textures/licenses enter `assets/`,
  paths remain unique/namespaced, and every selected model declares a primitive fallback.

Canonical commands come from the then-current `justfile` and root README. M01 updates those
commands only if the accepted 3D development selector needs a documented invocation; it does not
invent parallel build/test workflows.

### Network and lifecycle scenarios

- one real routed or current canonical two-client Wipeout reaches Active, fires straight
  projectiles, completes, and returns/cleans up with the 3D presentation enabled;
- one real Hot Zone run shows exact empty/contested/controlled transitions and cleanup;
- one terrain mutation visibly rebuilds the affected 3D chunk and its needed seam neighbor without
  changing the server result;
- disconnect/reconnect, match restart, and map generation replacement retain no stale 3D entities,
  lights, materials, or camera target;
- different render profiles do not change fixed-tick authority or replicated outcomes.

### Visual and hands-on matrix

At minimum inspect native 16:9 and one non-16:9 supported layout:

- both bounded camera candidates before selecting one;
- controlled fighter at center and every camera-bound edge;
- allies/enemies north and south of walls;
- imported wall modules repeated across every current resolved wall length, including corners/end
  treatment or an honest cuboid fallback where a module does not fit;
- overlapping fighter silhouettes and team/facing distinction;
- imported character at idle and walk, holding/shooting the selected blaster; inspect grip,
  handedness, muzzle position, clipping, recoil, model forward axis, scale, and team marker;
- forced missing/failed character, weapon, environment, or colormap load showing the correct
  primitive fallback without blocking the match;
- fast straight projectile against floor, wall, and fighter colors;
- lob launch, apex, telegraph/landing, and impact;
- empty, contested, and each-team Hot Zone state, with the authoritative boundary understandable;
- intact and damaged destructible terrain including a chunk seam;
- reduced-effects setting and current UI/HUD layered over the 3D world;
- pause/menu/scoreboard visibility without depth interference.

Requested user observations:

1. Does the fixed camera preserve effortless movement and aim, or does its azimuth make controls
   feel rotated?
2. Are fighters, aim direction, and straight/lobbed projectiles readable at combat speed?
3. Do walls obscure actionable information often enough to require fading or outlines?
4. Is the Hot Zone boundary exact and readable without looking like a raised obstacle?
5. Do the imported models and their primitive fallbacks share a coherent scale, especially the
   48-unit fighter beside cover and terrain?
6. Does the 3D result clearly justify retiring the existing 2D world renderer?
7. Does the Kenney character/blaster combination look intentional enough for the first release,
   or does the hand pose/weapon scale require another selected model or a simple source adjustment?

## Performance and feasibility evidence

The native release-profile run records rather than guesses:

- Mac model, CPU/GPU, operating system, window logical/physical resolution, and render profile;
- camera constants and shadow configuration;
- visible fighters, projectiles, wall meshes, terrain chunks/triangles, objective meshes, lights,
  transient entities, instantiated GLB scene entities, skinned meshes, and active animation players;
- warmup duration, sample duration, median/p95/p99 render-frame time, slow-frame count, and fixed-tick
  lag/miss evidence available through current diagnostics;
- mesh/material asset counts before gameplay, at representative load, and after teardown;
- comparison with the same supported match under the existing 2D renderer where the harness can
  make the samples comparable.

Feasibility requires the representative imported/fallback 3D scene to sustain the existing 60 Hz
fixed simulation and a 60 FPS presentation target at the representative native resolution on the
named development Mac.
Use a p95 frame-time target of at most 16.67 ms after warmup. A miss is not hidden by reducing
gameplay counts or authority work: record the cause, try only a bounded obvious correction, and
return the milestone to specification review if the accepted representative scene still misses.

This is a development-machine feasibility gate, not a claim about all future supported hardware.
Broader device tiers belong to the release/platform roadmap once those targets are selected.

## Risks and bounded responses

| Risk | Detection | Bounded response |
|---|---|---|
| Camera azimuth makes movement/aim feel rotated | Side-by-side hands-on camera candidates | Select axis-aligned angled top-down view; do not rotate input semantics to compensate invisibly |
| Walls hide fighters/objectives | North/south occlusion matrix | Tune elevation/wall height first; record selective fade/outline for M02–M04 only if still required |
| Imported fighter pose or fallback sphere hides facing | Combat-speed inspection | Retain a simple ground/facing marker and validate the model's forward-axis correction |
| Straight projectile is too small or directionless | Fast projectile playtest | Use short capsule/cylinder length and emissive/high-contrast material while preserving authoritative diameter |
| Transparent zone sorts poorly or z-fights | Camera/aspect/state matrix | Named ground offset, boundary-first design, unlit/emissive boundary, or opaque patterned fill; do not adopt decals automatically |
| Tilted camera clamp exposes outside-map space | Corner-ray footprint tests and edge play | Derive conservative ground footprint from ray intersections and clamp against resolved camera bounds |
| Terrain mesh rebuild allocates or stalls | Mutation measurements and asset counts | Reuse per-chunk mesh handles/buffers where Bevy permits and rebuild only dirty chunks/neighbors; no cell entities |
| GLB loads but its colormap does not | Dependency-aware readiness and forced missing-texture case | Preserve pack namespace and relative `Textures/colormap.png`; fall back by stable profile ID |
| Duplicate basenames bind the wrong pack asset | Manifest/path uniqueness and visual inspection | Never flatten packs; use explicit pack-key paths and distinct colormaps |
| Character and blaster do not share a socket | Instance node inspection and grip/muzzle matrix | Attach to validated `arm-right` with one reviewed local transform profile; select another blaster or retain primitive weapon if fit remains poor |
| Holding pose conflicts with locomotion | Walk-while-aiming and shoot inspection | Use the smallest animation graph/mask over the shared bones; defer general blending and do not let animation drive movement |
| Imported wall module misrepresents collider footprint | Resolved footprint overlay/test | Repeat only modules with compatible nominal bounds; use exact cuboid/cylinder fallback for remainder or incompatible shapes |
| Asset import bloats the first release | Runtime asset inventory and package size | Copy only M01-selected GLBs, their colormaps, and licenses; exclude FBX/OBJ/previews/unused packs |
| PBR dependencies leak into server | server-only feature audit | Keep all features under `bevy-client` and types inside client-gated modules |
| 3D transform starts influencing authority | protocol/source audit and outcome comparison | Keep render children and conversion one-way; reject replicated height/`Transform` |
| Representative imported/fallback scene misses 60 FPS | native release profile measurement | Remove unnecessary shadows/transparency/draw calls with one bounded pass; return to review if the representative scene still misses |

## Closeout criteria

- [x] The real client/server path proves the accepted fixed orthographic 3D presentation without
      changing planar authority, Avian 2D collision, replicated height, or protocol registration.
- [x] Coordinate conversion, camera, ground-ray input, shared primitive/material ownership, GLB
      dependency loading, animation hierarchy discovery, weapon attachment, exact Hot Zone mesh,
      terrain mesh generation, and straight/lob projectile foundations exist in production code.
- [x] The selected client dependencies remain absent from the dedicated-server feature graph.
- [x] Wipeout and Hot Zone reach Active in live 3D smoke runs, and the final `just lint` and `just
      test` regression paths pass after the accepted projectile fixes.
- [x] The user accepted the feasibility result, requested only later visual tuning, reviewed both
      projectile-origin defects, accepted their corrections, and explicitly directed M01 closeout
      and M02 start on 2026-08-20.
- [x] Every uncompleted production-hardening item from the original broad feasibility matrix is
      named in the V3 backlog with a receiving milestone rather than reported as passed.
- [x] Feedback disposition and learn-from-errors review are recorded below.

### Transferred verification

- M02 owns forced GLB/colormap failure fallback, camera-ray corner coverage, projected camera
  footprint coverage, sole-owner map/terrain lifecycle, live terrain mutation/seam/reset/teardown,
  and objective state-transition verification before the 3D map renderer becomes default.
- M03 owns the complete character locomotion/holding/shoot, grip/clipping, fighter scale, and
  combat-world replacement matrix.
- M04 owns native release-profile p95/p99 frame-time, representative entity/asset counts, cleanup
  counts, and final occlusion/readability comparison. M01 makes no 60 FPS evidence claim.

## Research log

### Repository and exact local sources

- `docs/00-product-direction.md` — combat readability and network-first simulation pillars.
- `docs/09-environment-and-tile-ideas.md` — enduring separation between authoritative geometry,
  regions, terrain occupancy, and replaceable client visuals.
- `docs/11-art-and-presentation-direction.md` — superseded 2D production-art proposal and the
  presentation/authority constraints retained by V3.
- `docs/08-network-architecture.md` — authority and protocol-evolution boundaries.
- `docs/implementation/v2/roadmap.md` — current version status and explicit production-art deferral.
- `Cargo.toml` — Bevy 0.19.1, Avian 2D 0.7, Lightyear 0.29, client/server feature isolation.
- `src/client/presentation.rs` and `src/client/input.rs` — current `Camera2d`, interpolation pose,
  follow/clamp, UI camera, and `viewport_to_world_2d` seams.
- `src/client/assets.rs` and `assets/manifest.ron` — retained handle/readiness ownership, current
  single-date CC0 manifest validation, and primitive/silence fallback contract to extend.
- `src/map/client.rs` and `src/map/model.rs` — current `Sprite`/`Mesh2d` reconstruction and resolved
  shape/presentation contracts.
- `src/terrain/client/presentation.rs` — committed occupancy, dirty chunks, neighbor seams, current
  per-chunk image lifecycle, and bounded debris.
- `src/combat/client/{world,cues,effects,hud,preview}.rs` — current sprite-owned combat presentation
  families requiring later replacement.
- `references/bevy/examples/3d/orthographic.rs` — checked-in `Camera3d` orthographic architecture
  example; the repository warns this snapshot is 0.20-dev, so exact APIs were checked below.
- Cargo registry `bevy-0.19.1/examples/3d/{orthographic,3d_viewport_to_world,decal,clustered_decals}.rs`
  — exact pinned camera, ground-ray, forward decal, and clustered-decal APIs.
- Cargo registry `bevy-0.19.1/examples/gltf/load_gltf.rs` and
  `examples/animation/{animated_mesh,animated_mesh_control,animation_masks}.rs` — exact pinned
  `WorldAssetRoot`, `GltfAssetLabel`, dependency readiness, `WorldInstanceReady`, named animation,
  graph/transition, and optional mask composition.
- `external_assets/kenney_{mini-arena,mini-characters,mini-dungeon,mini-forest_1.0,blaster-kit_2.1,pirate-kit,graveyard-kit_5.0}/`
  — user-supplied upstream packs, local CC0 licenses, overviews, GLB/FBX/OBJ exports, previews, and
  pack-local textures. Direct GLB JSON inspection recorded node/skin/animation names, source bounds,
  external image URIs, extensions, and shared/different asset conventions without modifying them.
- Cargo registry `bevy_light-0.19.1/src/cluster/mod.rs` — exact pinned clustered-decal platform
  limitations.
- Cargo registry `bevy_math-0.19.1/src/ray.rs` and `bevy_camera-0.19.1/src/camera.rs` — exact pinned
  ray-plane and viewport-to-world APIs.

### Current primary sources

- [Bevy orthographic 3D example](https://bevy.org/examples/3d-rendering/orthographic/) — official
  fixed orthographic `Camera3d`, `Mesh3d`, ground plane, cuboid, and light composition.
- [Bevy 0.19 orthographic projection API](https://docs.rs/bevy/0.19.1/bevy/camera/prelude/struct.OrthographicProjection.html)
  — `default_3d`, fixed scaling, near/far, and resize behavior.
- [Bevy 0.19 mesh API](https://docs.rs/bevy/0.19.1/bevy/mesh/struct.Mesh.html) — `Mesh3d`, required
  normals/UV/winding behavior, culling bounds, and mutation caveat.
- [Bevy 0.19 clustered-decal availability](https://docs.rs/bevy/0.19.1/bevy/pbr/decal/clustered/fn.clustered_decals_are_usable.html)
  — official macOS/iOS disablement, which disqualifies clustered decals as Brawler's objective
  foundation.
- [Bevy glTF loading example](https://bevy.org/examples/gltf/load-gltf/) — official scene loading
  through `WorldAssetRoot` and `GltfAssetLabel::Scene`.
- [Kenney Mini Characters](https://kenney.nl/assets/mini-characters),
  [Mini Arena](https://kenney.nl/assets/mini-arena),
  [Mini Dungeon](https://kenney.nl/assets/mini-dungeon),
  [Mini Forest](https://kenney.nl/assets/mini-forest),
  [Blaster Kit](https://kenney.nl/assets/blaster-kit),
  [Pirate Kit](https://kenney.nl/assets/pirate-kit), and
  [Graveyard Kit](https://kenney.nl/assets/graveyard-kit) — official pack identity, category,
  version notes, animation/variation availability, and CC0 license confirmation.

The local pinned source was sufficient for exact M01 APIs. Internet research confirmed the current
official orthographic, GLB scene-loading, decal, pack, and license guidance; no unrelated engine or
general Rust architecture pattern was needed.

## Specification validation

Accepted on 2026-08-20 when the user explicitly directed V3 M01 implementation to begin. V2 had
already completed and its closeout had been accepted, so the milestone moved to `Implementing`.

## Feedback review

| Feedback | Disposition |
|---|---|
| The initial 3D result “seems ok” and can be tuned later | Accepted as the M01 feasibility decision; environment brightness, scale, framing, and broader polish remain scheduled work rather than closeout blockers |
| Straight bullets appeared vertically detached from the fighter | Implemented in M01 by correcting Kenney character/blaster forward axes and reducing straight-shot render height from 20 to radius-sized clearance; authority was unchanged |
| Corrected bullets still first appeared far from the muzzle | Implemented in M01 with client-only launch catch-up from replicated `StraightFlight.origin` to the current replicated pose at bounded 3x speed without overshoot; collision and hit timing were unchanged |
| Close M01 and move to M02 | Accepted on 2026-08-20; M01 is Complete and M02 is Researching |

## Learn-from-errors review

1. **The renderer decision preceded a representative feasibility comparison.** The earlier art
   research optimized a faux-depth 2D pipeline before testing whether the intended arena target was
   fundamentally 3D. Future foundational renderer/art decisions require one small, real-client
   comparison covering camera, one character, one obstacle, one projectile, one objective, input,
   and dependency isolation before a production direction is validated.
2. **Configuration had two sources of truth.** The first apparent 3D run was actually 2D because
   CLI initialization overwrote the environment-selected renderer. Parser defaults must preserve
   already-resolved configuration; selector tests must cover environment-only, CLI-only, and CLI
   precedence before visual evidence is accepted.
3. **A compiling 3D scene was not a runnable 3D scene.** Missing reflected scene registration,
   unavailable LUT-dependent tonemapping, and a camera beyond the default far plane appeared only
   in the first live GLB run. A 3D foundation check must include one imported scene, explicit
   projection near/far, selected tonemapping/features, and a native capture—not only unit tests.
4. **Projectile correctness has spatial and temporal presentation dimensions.** The authoritative
   muzzle and coordinate mapping were correct, but artificial height caused camera parallax and
   replication delay caused the first visible frame to appear in mid-flight. Fast replicated
   visuals must be checked both for world-line alignment and first-observation continuity. Any
   catch-up remains bounded client presentation and cannot alter authority or collision.
5. **M01 mixed feasibility and final-production gates.** Forced failure matrices, complete terrain
   lifecycle, full animation polish, and release performance are valid requirements, but the M01
   contract bundled them into a feasibility answer that the user could accept before those systems
   became sole-owner/default. Future milestones separate the smallest decision gate from the
   production cutover gate and explicitly transfer remaining work at closeout.

These lessons are recorded in the project roadmap and M02 entry work. No new general Codex skill
was created: the reusable practice is already covered by the repository's milestone and
verification rules, while the concrete failures are specific to Brawler's renderer migration.
