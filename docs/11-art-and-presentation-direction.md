# Art and presentation direction

## Status

V3 completed the gameplay-world migration from sprites and `Mesh2d` to a fixed-camera 3D Bevy
scene. This document is the current art/presentation contract. The former pixel-art, atlas,
paper-doll, y-sort, and tile-renderer proposal is retired; its enduring lessons survive here as
readability, bounded effects, replaceable presentation, and server-isolation rules.

## Presentation model

Brawler is a 3D-presented game with planar authoritative gameplay:

```text
server-owned gameplay                  client-owned presentation
Vec2 position / planar rotation   ->   X/Z ground transform + render-only height
Avian 2D collision                ->   Mesh3d / imported GLB scenes
resolved 2D map shapes            ->   exact-footprint 3D/procedural geometry
replicated combat state/cues      ->   animation, effects, audio, projected UI
```

The camera is fixed, tilted, and uses restrained perspective. There is no player orbit,
authoritative height, jumping, or walkable wall top. The depth buffer replaces 2D painter
ordering; render height and animation never feed back into simulation.

Screen-space menus, HUD, settings, and overlays remain Bevy UI on the UI camera. Fighter overhead
information is also Bevy UI, projected from a 3D point above each fighter so names and values stay
crisp without reintroducing `Text2d` into the gameplay world.

## Visual character

The first-release direction is stylized, toy-like, clean, and readable at match scale:

- simple silhouettes and restrained surface detail;
- matte materials rather than photorealistic metal/roughness variation;
- calm floors and subordinate neutral cover;
- strong team/relationship accents independent of model or skin color;
- shallow, restrained shadows that provide grounding without hiding footprints;
- exact objective boundaries and unmistakable blocked/invalid previews;
- short, bounded combat effects that communicate outcomes without obscuring actors.

Kenney Mini Characters, Blaster Kit, and Mini Arena provide the current CC0 foundation. They are
replaceable presentation, not a claim of final visual identity. Original models, materials,
animations, and VFX should preserve the same proportions, anchors, relation language, and
gameplay-shape agreement.

## Readability language

### Fighter identity

- the controlled fighter uses green ground-ring, facing, and name accents;
- allies use blue;
- enemies use red;
- the ring is a flat, unlit, non-shadow-receiving annulus slightly above the ground;
- a small arrowhead integrated into the ring communicates facing without protruding from the body;
- skins and imported materials may not override these relation cues.

The imported character's model-space forward direction is corrected once at its visual root. The
weapon attaches inside that corrected hierarchy, and animation-specific transforms stay below it.
A sphere at the authoritative body radius remains the deterministic fallback and geometry aid.

### Overhead information

Each live fighter has a camera-projected overhead cluster:

- player name centered above the health bar and colored by relation;
- current health centered in white, slightly overlapping the rounded bar;
- green health fill for the controlled fighter and allies, red for enemies;
- a compact segmented ammunition row only for the controlled fighter, with one segment per
  authoritative shot capacity;
- no reserved ammunition-row height for allies or enemies;
- defeated and off-screen fighters hide their cluster.

This UI reads replicated state only. It does not own health, ammunition, defeat, names, or team
assignment.

### Combat and objectives

- straight projectiles are compact travel-aligned cylinders; lobbed projectiles are spheres with a
  presentation-only vertical arc;
- previews and telegraphs use shape-exact procedural meshes and distinguish allowed from blocked;
- Hot Zone uses a procedural fill plus boundary annulus, not a decal or authored floor model;
- slow, knockback, dash, sentry, impact, damage, defeat, and reset feedback remain visibly distinct;
- reduced-effects mode decreases redundant debris/effect size, lifetime, and count but never hides
  required state such as objectives, previews, projectiles, status, or relation identity.

## Environment and geometry

Visual footprints must agree with authoritative 2D shapes:

- rectangles become cuboids with presentation-only height;
- circles become cylinders or generated circular geometry;
- floors use calm cached tiles or generated surfaces;
- destructible terrain builds chunk-owned meshes from replicated occupied cells and updates dirty
  chunks in place;
- arbitrary/dynamic shapes use primitives or generated meshes instead of forcing an approximate
  imported prop;
- imported environment GLBs are used only where their pivot, orientation, scale, and footprint are
  validated against a real presentation profile.

Occlusion should first be solved with camera elevation, wall height, material value, and map
composition. Selective fade, outlines, x-ray rendering, decals, and custom pipelines require a
future evidence-backed specification; none is part of the V3 foundation.

## Asset and animation pipeline

GLB is the preferred shipped model format because it carries mesh, material, hierarchy, skin, and
animation data in one Bevy-supported asset. Runtime files retain a pack namespace and relative
texture dependencies. Only selected assets are copied from `external_assets/` into `assets/` and
entered in the provenance manifest.

The current Mini Character contract expects named idle, walk, holding, shoot, and die clips. Bevy
animation clips may omit channels; when leaving defeat, the client restores the imported bind pose
before starting the live loop so root/leg transforms cannot remain in the death pose. Weapon grip
and model-forward corrections belong to presentation constants/profiles, not gameplay rotation.

Primitive and generated mesh assets are cached and shared. Generation-owned meshes are explicitly
removed on map replacement; terrain chunks mutate their owned mesh rather than allocating on every
revision. Do not create a unique mesh or material asset per fighter, projectile, tile, or effect.

## Rendering architecture

`WorldPresentationPlugin` owns the sole gameplay-world renderer. Its responsibilities are:

- one `Camera3d`, fixed perspective projection, ambient light, directional light, and selected MSAA;
- the tested simulation-XY to render-XZ conversion API;
- map, terrain, fighter, projectile, sentry, objective, preview, status, debris, and cue visuals;
- GLB readiness, imported-scene promotion, deterministic primitive fallback, animation, and weapon
  attachment;
- render-only entity ownership, reconciliation, cleanup, and final pose writes after interpolation;
- camera projection of fighter overhead Bevy UI;
- optional bounded native render diagnostics.

Replicated gameplay entities remain presentation-neutral. Render entities carry an owner link and
are reconciled idempotently. Final transforms are written after Lightyear interpolation and Avian
writeback, before transform propagation; projected UI runs after propagation. Complex mutable
queries must encode disjoint marker filters or use a `ParamSet`, and schedule-initialization tests
must validate query access—not only compile it.

## Server, protocol, and asset boundaries

- the server and routed workers contain no cameras, meshes, materials, images, models, animation,
  audio, windowing, or device-input dependencies;
- protocol state remains planar and never carries `Transform`, `Vec3`, entity IDs local to a
  process, mesh/material handles, model-node names, or render height;
- gameplay emits stable state and cues; presentation resolves them to client assets and effects;
- every shipped third-party asset has exact provenance in `assets/manifest.ron` and retained
  license text where required;
- `BRAWLER_FORCE_PRIMITIVE_WORLD=1` is a degradation/verification path inside the 3D renderer, not
  a renderer choice and not a return to 2D.

## Performance and verification

The baseline strategy is deliberately simple: cached meshes/materials, chunked terrain, bounded
transients, four-sample MSAA, one directional light, restrained shadows, and Bevy's normal culling.
Instancing, LOD, GPU particles, bloom, deferred rendering, and custom pipelines require measured
need.

Automated verification covers coordinate mapping, exact footprints, projectile origins, lifecycle
cleanup, animation recovery, marker relation colors, overhead layout/query safety, reduced-effects
bounds, source retirement, server feature isolation, and fixed-tick capacity. Native visual checks
cover both modes, supported aspect ratios, imported and primitive fallback paths, defeat/respawn,
terrain mutation, objectives, and representative combat density.

## Deferred art work

- original environment, fighter, weapon, material, animation, VFX, icon, and branding production;
- additional map-theme defaults promoted from the available Kenney packs; themes guide ground,
  edge, lighting, palette, and default object variants but do not prohibit mixed styles;
- skin selection, replication, entitlement, and accessibility validation;
- game-object and model/weapon variant catalogs with per-placement compatible visual overrides;
- advanced occlusion treatments or rendering features justified by later playtests.
