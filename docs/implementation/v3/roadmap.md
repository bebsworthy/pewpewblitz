# Version 3 implementation roadmap

## Purpose and scope

V3 replaces Brawler's sprite- and `Mesh2d`-based **world presentation** with a fully 3D Bevy
scene viewed through a fixed, tilted orthographic camera. The authoritative game remains a 2D
arena shooter: movement, collision, targeting, map containment, terrain occupancy, combat, and
network replication continue to use the existing server-owned 2D simulation.

This is a presentation migration, not a conversion to 3D gameplay or physics. The selected model
is:

```text
authoritative simulation: Vec2 position + planar rotation + Avian 2D
                              |
                              v
client presentation adapter: X/Y plane -> X/Z ground plane + render-only height
                              |
                              v
rendered world: Camera3d + Mesh3d + materials + lights + depth buffer
```

V3 uses Bevy primitives and simple materials for exact dynamic geometry and deterministic
fallbacks, plus a curated subset of the CC0 Kenney GLB packs staged in `external_assets/` for the
first-release environment, fighter, and weapon presentation. The available packs demonstrate a
real asset use, so a small validated GLB/animation/attachment pipeline now belongs in the
migration. V3 does not bulk-import every model or build a general-purpose asset framework.

The player shell and HUD remain Bevy UI. A dedicated 2D/UI camera or screen-space UI is not part
of the world-renderer replacement and does not violate the V3 goal.

## Product and architecture decisions

- The game keeps its 2D gameplay plane and server-authoritative Avian 2D simulation.
- Network protocol positions, rotations, map shapes, terrain occupancy, and gameplay cues remain
  2D. V3 does not add replicated height or process-local 3D transforms.
- The client maps simulation `(x, y)` to render-ground `(x, 0, z)` through one tested conversion
  boundary. Render-only height is added after that conversion.
- Replicated gameplay entities retain interpolated planar `Position`/`Rotation` as their pose
  contract; client presentation must not encode that pose as the legacy
  `Transform(x, y, z-layer)` convention.
- Gameplay-world meshes, scenes, markers, telegraphs, and world HUD live on dedicated render-only
  entities linked to their gameplay owner. Their final 3D `Transform` is written after Lightyear
  interpolation and Avian writeback, before transform propagation. A gameplay entity may retain a
  structural `Transform` required by Bevy or Avian, but presentation never treats it as an XY pose
  carrier.
- The arena uses a fixed tilted `Camera3d` with an orthographic projection. M01 selects the exact
  elevation, azimuth, span, and wall height from bounded visual comparisons.
- Depth-buffered geometry replaces painter-order Y sorting and fake cliff faces.
- Rectangular permanent walls render as cuboids; circular geometry renders as cylinders so the
  visual footprint continues to match the authoritative shape.
- Fighters use a curated Mini Characters model with a small ground/facing marker; a team-colored
  sphere at the existing body radius remains the deterministic load-failure and shape-debugging
  fallback.
- Straight projectiles initially render as short horizontal cylinders or capsules aligned with
  travel. Their diameter derives from the authoritative projectile radius; their length is
  presentation-only. Lobbed projectiles render as spheres whose vertical position uses the
  existing visual arc.
- Ground gameplay indicators such as Hot Zone use procedural planar meshes, not decals or
  authored models. A circular zone is a fill disc plus an annulus whose centerline marks the exact
  authoritative boundary. Rectangle areas use equivalent planar quads/borders.
- Clustered decals are not a V3 foundation dependency. Bevy 0.19 disables them on macOS and iOS,
  and Brawler's first target is macOS. Forward decals also require a depth prepass and add no value
  for flat, shape-exact objective indicators.
- Primitive meshes and materials are cached and shared. Presentation systems do not allocate one
  new mesh or material asset per entity.
- Curated static props and modular environment pieces load from GLB. Animated characters load as
  scenes with their supplied skeleton and named clips. `bevy_gltf` and `gltf_animation` remain
  client-only features.
- Runtime assets preserve a pack namespace and the GLB's relative `Textures/colormap.png`
  dependency. Packs cannot be flattened: their colormaps differ and many reuse filenames such as
  `wall.glb`, `floor.glb`, and `tree.glb`.
- `external_assets/` is upstream source material, not the shipped asset root. Only models,
  textures, and licenses selected by an implementation milestone are copied into `assets/` and
  recorded in the provenance manifest.
- The supplied isometric PNG guidance does not define world rendering. V3 uses the GLBs; any
  isometric renders are optional future catalog thumbnails, not tiles or collision sources.
- The dedicated-server feature graph remains free of rendering, windowing, asset, PBR, scene, and
  glTF dependencies.

## Delivery rules

- Every milestone begins with R&D against the exact pinned Bevy 0.19.1 and current source tree.
- Only the next milestone receives a detailed file. Later roadmap entries specify player-visible
  outcomes, ordering, dependencies, and gates without pre-authoring their implementation.
- A milestone moves from `Researching` to `Specification review`; user validation is required
  before it moves to `Implementing`.
- M01 is a feasibility milestone, but its coordinate, camera, input, mesh, and material work must
  be production-reusable. It must not introduce a second simulation or disposable application.
- While M01 is under evaluation, the 3D composition may be development-selected so the supported
  product path remains usable. M02 makes 3D the default and removes replaced 2D world paths rather
  than maintaining a permanent renderer toggle.
- Each replacement preserves execution-role isolation, authoritative shape parity, lifecycle
  cleanup, bounded presentation state, and existing network behavior.
- Presentation height, mesh dimensions beyond the gameplay footprint, materials, lights, shadows,
  and animation never feed back into simulation or authority.
- Accepted scope changes update this roadmap and the active milestone before implementation.

Allowed statuses are `Not started`, `Researching`, `Specification review`, `Implementing`,
`Verifying`, `User playtest`, `Feedback review`, `Complete`, and `Blocked`.

## Version status

| Field | Value |
|---|---|
| Status | User playtest |
| Current milestone | M03 — Complete 3D combat, fighter, cue, preview, and world-HUD replacement |
| Entry gate | Satisfied — the user accepted the M01 feasibility result and directed M01 closeout and M02 start on 2026-08-20 |
| Completion gate | Every gameplay-world visual uses the 3D scene, no sprite/`Mesh2d` world fallback remains, the server and protocol stay 2D and authoritative, and representative networked play passes the accepted visual, performance, and lifecycle gates |

## Milestone overview

| Milestone | Status | Deliverable | Plan |
|---|---|---|---|
| 01 | Complete | 3D presentation feasibility and reusable foundation | [milestone-01.md](./milestone-01.md) |
| 02 | Complete | Default 3D arena, map, terrain, camera, and input cutover | [milestone-02.md](./milestone-02.md) |
| 03 | User playtest | Complete 3D combat, fighter, cue, preview, and world-HUD replacement | [milestone-03.md](./milestone-03.md) |
| 04 | Not started | 2D world-renderer retirement, readability, performance, and V3 closeout | Create after M03 completion |

## Ordering rationale and milestone gates

### M01 — 3D presentation feasibility and reusable foundation

Prove the risky cross-cutting seams before replacing the supported renderer: coordinate and angle
mapping, orthographic camera framing, ground-plane mouse aiming, primitive and imported-model
scale, GLB dependency/readiness behavior, character animation and weapon attachment, real depth,
lighting, shadows, objective geometry, projectile readability, lob height, terrain reconstruction,
occlusion, client-only dependency isolation, and networked interpolation.

The representative scene includes floor, permanent cover, one destructible-terrain section,
fighters, an equipped blaster, straight and lobbed projectiles, and Hot Zone. At least one fighter,
weapon, and environment family uses the curated Kenney GLBs while every imported family retains a
primitive fallback. The proof runs through the existing client and authoritative server; it is not
a separate gameplay demo.

Gate: [milestone-01.md](./milestone-01.md) passes, the user accepts one camera/readability candidate,
and no result requires 3D simulation or a protocol height component. Otherwise V3 returns to
specification review before a renderer cutover.

### M02 — Default 3D arena, map, terrain, camera, and input cutover

Make the 3D camera and world composition the supported default. Convert every map presentation
family: ground, playable perimeter, rectangular and circular permanent geometry, decorative visual
instances, placed map entities, authored regions, spawn markers where they remain product-visible,
mode anchors, and chunked destructible terrain. Preserve map-generation teardown and terrain dirty
chunk/seam behavior. Replace 2D viewport aiming and camera clamping with the accepted ground-plane
and projected-footprint implementations. Resolve the first environment theme through curated
Mini Arena, Mini Dungeon, and/or Mini Forest model profiles where their modular footprints match;
retain procedural meshes for arbitrary/dynamic shapes. Pirate Kit and Graveyard Kit enter only if
one selected theme demonstrates their different source scale and visual role.

M02 removes each 2D arena/map/terrain implementation when its 3D replacement lands. Its feedback
closeout also removes the obsolete 2D projectile creation/synchronization path now that the 3D
projectile owner exists; a 3D pose writer must not merely correct an XY `Transform` written earlier
in the frame. M02 does not retain a user-facing renderer choice.

Gate: both built-in modes and all supported map-size fixtures reconstruct, update, reset, and
teardown correctly in 3D; mouse/controller aiming and camera containment remain correct; terrain
mutation visibly converges; the default client has no 2D map or terrain fallback; and straight and
lobbed projectiles have no legacy sprite or XY-`Transform` writer.

### M03 — Complete 3D combat, fighter, cue, preview, and world-HUD replacement

Convert every remaining gameplay-world visual: fighters and facing markers, sentries, straight and
lobbed projectiles, dash trails, attack previews, impact/destruction cues, status markers, objective
feedback, world-space health/team identity, placement/landing telegraphs, and bounded debris. Keep
screen-space product HUD and menus in Bevy UI.

This milestone completes the presentation-pose ownership cut. Replicated gameplay entities expose
interpolated planar `Position`/`Rotation`; dedicated render-only visual entities reference their
owner and receive the converted X/Z-ground `Transform` in the post-interpolation presentation
phase. No combat or movement presentation system writes simulation `(x, y)` into a gameplay-world
`Transform`, and no 3D writer exists only to repair an earlier XY writer.

The selected Mini Characters rig and compatible Kenney character variants replace fighter spheres,
with spheres retained as load-failure fallbacks. Blaster Kit models attach through a validated
right-hand/both-hand presentation profile; supplied idle/walk/holding/shoot/die clips drive the
small render-only animation state. Primitive composition remains sufficient for projectiles,
telegraphs, and effects: spheres, cuboids, cylinders/capsules, cones, rings, lines, and bounded
particles.

Gate: every existing combat delivery, ability, status, preview, and cue has a readable 3D result;
no client gameplay-world system requires `Sprite`, `Text2d`, `Mesh2d`, or `ColorMaterial`; cues stay
bounded and presentation-only; network interpolation and authoritative outcome tests remain intact;
and a targeted source/schedule audit finds no legacy XY presentation-pose writer.

### M04 — 2D world-renderer retirement, readability, performance, and V3 closeout

Delete residual 2D world assets, renderer branches, pixel-density/y-sort assumptions, tests, and
dependencies that no longer serve UI. Resolve the observed occlusion policy with the smallest
accepted technique (camera/wall tuning first, selective fade or outline only if playtests require
it). Tune lighting, shadow strength, color language, reduced-effects behavior, and supported camera
framing. Measure native macOS rendering and fixed-tick stability with representative player,
projectile, terrain, and effect counts.

Gate: automated and supervised network play across Wipeout and Hot Zone proves readable action,
correct aiming, exact objective boundaries, stable terrain updates, clean lifecycle teardown, and
accepted native performance; server feature isolation passes; a targeted source audit finds no 2D
gameplay-world renderer, permanent migration toggle, or planar XY encoding in gameplay-world
`Transform`; feedback and learning review are complete.

## Cross-version technical policies

### Coordinate and authority boundary

- Gameplay owns `Vec2`, Avian `Position`/`Rotation`, authoritative colliders, and fixed-tick rules.
- One client-owned conversion API maps positions, directions, rotations, extents, and bounds into
  the X/Z render ground plane. Call sites do not reproduce axis/sign conventions ad hoc.
- Dedicated render-only visual entities consume the latest accepted replicated/interpolated pose
  from their gameplay owner. Their transforms add X/Z conversion, height, orientation correction,
  animation, and effects without mutating the replicated pose. Gameplay-world presentation never
  stores simulation `(x, y)` as render `Transform.translation.(x, y)`.
- A projectile's rendered length, fighter facing marker, wall height, shadow, and lob elevation are
  presentation facts. Hit testing continues to use the existing 2D shapes and ticks.
- Objective visuals derive from resolved authoritative `MapShape` data. They never determine
  occupancy or scoring.

### Execution roles and dependencies

- PBR, mesh, light, scene, glTF, and animation features belong inside `bevy-client` only.
- The dedicated server never creates meshes, materials, images, lights, cameras, or asset handles.
- Protocol registration does not replicate Bevy `Transform`, `Vec3`, mesh handles, material
  handles, model nodes, or render-only height.
- Map, terrain, combat, and client-presentation modules retain their existing state ownership.
  Shared coordinate/material utilities do not become a second owner of their lifecycles.

### Visual and performance policy

- Combat readability outranks physical realism. Materials remain matte, colors deliberate, floors
  calm, objectives unmistakable, and shadows restrained.
- The fixed camera has no player-controlled orbit. Zoom or perspective is not introduced unless a
  later validated requirement needs it.
- Shared meshes/materials, chunk meshes, bounded transient effects, frustum culling, and simple
  lighting are the starting performance strategy. Instancing, LOD, custom render pipelines, and GPU
  particles require measured evidence.
- Release-profile native measurements record hardware, resolution, render profile, visible entity
  counts, frame-time distribution, and fixed-tick behavior. A debug build is not a performance gate.
- Visual screenshots and manual play complement authority/lifecycle tests; they do not replace
  them.

## V3 backlog

| ID | Item | Disposition |
|---|---|---|
| V3-ORIGINAL-ASSETS | Replace or extend the first-release CC0 Kenney selection with original environment models, characters, weapons, materials, animations, and VFX | Deferred until the 3D renderer and content needs are proven; originality remains a later product-art goal |
| V3-ADDITIONAL-KENNEY-THEMES | Full Pirate, Graveyard, Dungeon, and Forest theme catalogs | Import only the first theme and model families exercised by V3; promote another pack when a real map/theme owns it |
| V3-SKINS | Character skin definitions, selection, replication, and entitlement policy | Deferred product/content slice |
| V3-ADVANCED-DECALS | Forward or clustered decals for markings across uneven surfaces | Not needed for planar gameplay indicators; clustered decals are unsuitable for the initial macOS target in Bevy 0.19 |
| V3-PERSPECTIVE-CAMERA | Perspective or custom projection | Rejected for the initial migration; revisit only if orthographic playtests fail a recorded product need |
| V3-3D-PHYSICS | Avian 3D gameplay, vertical collision, elevation, jumping, or walkable wall tops | Explicitly outside V3; requires a separate product and authority decision |
| V3-ADVANCED-RENDERING | LOD, GPU particles, environment maps, deferred rendering, dynamic time of day, or a custom render pipeline | Add only for a measured performance or art-direction requirement |
| V3-M01-FALLBACK-PROOF | Forced missing character, weapon, wall, and colormap dependencies with deterministic primitive fallback | Promoted to M02 because the default renderer must prove degraded play before removing the 2D map path |
| V3-M01-CAMERA-RAY-CORNERS | Focused non-16:9 ground-ray corner and projected camera-footprint tests | Promoted to M02 camera/input cutover verification |
| V3-M01-TERRAIN-LIFECYCLE | Live terrain mutation, seam rebuild, reset, map replacement, and disconnect cleanup in 3D | Promoted to M02, which becomes the sole map/terrain presentation owner |
| V3-M01-ANIMATION-POLISH | Full locomotion/holding/shoot transition, grip, clipping, and scale matrix | Deferred to M03 combat/fighter replacement |
| V3-M01-RENDER-PERFORMANCE | Native release-profile frame-time distribution and teardown asset counts | Deferred to the representative complete renderer gate in M04; M01 debug/native play showed no feasibility blocker but is not performance evidence |

## Explicitly outside V3

- changing Brawler into a vertically navigable game;
- replacing Avian 2D with a 3D physics engine;
- sending `Vec3`, height, mesh, model, or animation state as gameplay authority;
- original production character/environment art or bulk import of the complete external pack
  library beyond the curated first-release selection;
- a player-controlled orbit camera, free camera, or cinematic camera system;
- general-purpose rendering abstractions, model import frameworks, material editors, or visual
  scripting without an owned content use;
- unrelated v2 networking, matchmaking, hosting, account, bot-AI, or progression backlog work.
