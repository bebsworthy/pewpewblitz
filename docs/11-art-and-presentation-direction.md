# Art, presentation, and asset specification

## Purpose

This document defines Brawler's durable visual direction, presentation ownership, readability
language, and runtime-asset policy. It describes the established 3D presentation foundation and the
constraints that future art must preserve; versioned implementation documents record when and how
individual parts were delivered.

Exact redistributed third-party files, licenses, and provenance belong to `assets/manifest.ron`,
not to this document. Every advertised built-in resolves stable map visual identities through
`assets/catalogs/map_asset_visuals.ron` and `assets/catalogs/map_presentation_themes.ron`, paired
with the shared definitions in `content/catalogs/` and authored recipes in `content/maps/`. These
machine-readable sources may evolve without turning this specification into a duplicate inventory.

## Visual direction

Brawler is a stylized, toy-like, fixed-camera arena shooter designed to remain clean and readable at
match scale. Its visual identity favors:

- recognizable silhouettes and restrained surface detail;
- matte, graphic materials rather than photorealistic surface variation;
- calm floors and subordinate environment dressing around the combat space;
- strong relationship and gameplay-state cues independent of model, theme, or skin color;
- shallow, restrained shadows that ground objects without hiding their footprints;
- exact objective boundaries and unmistakable allowed or blocked previews;
- short, bounded combat effects that communicate outcomes without obscuring actors;
- a quiet product shell that gives the selected brawler and current action clear hierarchy.

Curated third-party models, audio, fonts, icons, and source art provide the established production
foundation. They are replaceable presentation, not Brawler's permanent identity. Future original
art should build a coherent PewPew Blitz identity while preserving gameplay proportions, anchors,
relation semantics, authoritative-shape agreement, accessibility, and degraded fallbacks.

## Presentation model

Brawler presents authoritative planar gameplay through a client-owned 3D scene:

```text
server-owned gameplay                  client-owned presentation
Vec2 position / planar rotation   ->   X/Z ground transform + render-only height
Avian 2D collision                ->   Mesh3d / imported GLB scenes
resolved 2D map shapes            ->   exact-footprint 3D/procedural geometry
replicated combat state/cues      ->   animation, effects, audio, projected UI
```

The gameplay camera is fixed, tilted, and uses restrained perspective. There is no player orbit,
authoritative height, jumping, or walkable wall top. The depth buffer owns visual occlusion; render
height, animation, particles, audio, and UI never feed back into simulation.

Presentation reads authoritative or replicated facts and resolves stable presentation identities.
Gameplay definitions and protocol state never carry source paths, scene-node names, mesh or
material handles, client-local entity identities, render height, or other renderer details.

## Presentation surfaces and ownership

### Gameplay world

`WorldPresentationPlugin` owns the sole gameplay-world renderer:

- the gameplay `Camera3d`, fixed projection, lighting, and selected MSAA;
- the tested simulation-XY to render-XZ conversion;
- map, terrain, fighter, projectile, sentry, objective, preview, status, debris, and cue visuals;
- imported-scene readiness, deterministic primitive fallback, animation, and weapon attachment;
- render-only entity reconciliation, cleanup, and final pose writes after interpolation;
- projection anchors for fighter overhead Bevy UI;
- optional bounded native render diagnostics.

Replicated gameplay entities remain presentation-neutral. Render entities carry explicit owner
links and reconcile idempotently across spawn, replacement, restart, reconnect, and late asset
readiness. Final transforms are written after Lightyear interpolation and Avian writeback, before
transform propagation; projected UI follows propagation.

V8 grid-map presentation derives water shores, vegetation edges, and wall joins from canonical
four-neighbor cell masks; recipes do not author visual variants or adjacency masks. V9 promoted
tall grass to explicit concealing terrain while retaining its public geometry, and whole-placement
barriers use normal or rubble replacement visuals. Imported and primitive fallback paths must
communicate the same walkability, concealment boundary, and destruction state. The accepted 32-unit
destruction granularity must never leave a smaller visual or collision speck.

V10's Feature Yard family reuses one normalized geometry across Wipeout, Hot Zone, and Heist while
presenting only the active mode's objective. Damageable barrels and chests reconcile replicated
health and terminal state; a destroyed chest disappears and its public restoration pickup uses a
distinct potion silhouette and glow. Heist presents each typed safe objective as a team idol and
pedestal fitted inside the authoritative footprint. Imported and primitive paths preserve object
class, collision footprint, team ownership, damaged-only versus persistent health treatment, and
terminal/pickup readability.

### Player Dashboard preview

The Dashboard owns a separate client-only brawler preview, its camera, lighting, entities, and
lifecycle. It may reuse the supported fighter model, attached weapon, idle animation, presentation
profile, and primitive fallback, but it does not instantiate a gameplay fighter, decide build
legality, or share entity ownership with the gameplay-world renderer.

### Bevy UI

Screen-space product flow, Dashboard, selection surfaces, HUD, results, settings, accessibility,
and overlays remain Bevy UI on UI-owned cameras. Fighter names, health, and local ammunition are
also Bevy UI projected from propagated gameplay-world anchors; they are not world-space text and do
not own the facts they display.

UI branding, fonts, icons, and shaders are optional client assets with bounded fallback behavior.
Navigation, focus, saving, networking, recovery, and shutdown must remain functional when a visual
asset or custom shader is unavailable.

### Audio

Client audio presents bounded gameplay and product cues. Missing or late audio degrades to silence
and cannot delay authority, navigation, match readiness, or cleanup. Playback limits prevent cue
bursts from becoming an unbounded entity or channel lifecycle.

## Readability language

### Fighter identity

- the controlled fighter uses green ground-ring, facing, and name accents;
- allies use blue;
- enemies use red;
- the ring is a flat, unlit, non-shadow-receiving annulus slightly above the ground;
- a small arrowhead integrated into the ring communicates facing without protruding from the body;
- imported materials, environment themes, and future skins may not override these relation cues.

The imported character's model-space forward direction is corrected once at its visual root. The
weapon attaches inside that corrected hierarchy, and animation-specific transforms remain below
it. A sphere matching the authoritative body radius is the deterministic fighter fallback and a
geometry-verification aid.

### Overhead information

Each live fighter has a camera-projected overhead cluster:

- player name centered above the health bar and colored by relation;
- current health centered in white over the rounded bar;
- green health fill for the controlled fighter and allies, red for enemies;
- a compact segmented ammunition row only for the controlled fighter, with one segment per
  authoritative shot capacity;
- no reserved ammunition-row height for allies or enemies;
- no cluster for defeated or off-screen fighters.

The cluster reads replicated state only. It never owns health, ammunition, defeat, names, or team
assignment.

### Combat and objectives

- straight projectiles use compact travel-aligned geometry;
- lobbed projectiles may use a presentation-only vertical arc while retaining authoritative ground
  position;
- previews and telegraphs use shape-exact procedural geometry and distinguish allowed from blocked;
- objective boundaries remain precise and cannot depend on a decorative model or decal;
- Heist idols show persistent public team/objective health, while neutral barrels and chests show
  compact health only when damaged; the idol, chest, barrel, and restoration pickup remain
  unmistakable at gameplay scale;
- destroyed chests disappear, pickups remain visibly available until authoritative collection or
  expiry, and primitive fallbacks preserve the same footprint and state distinctions;
- slow, knockback, dash, deployable, impact, damage, defeat, and reset feedback remain distinct;
- reduced-effects mode decreases redundant effect size, lifetime, and count but never hides required
  objectives, previews, projectiles, statuses, or relationship identity.

## Environments, themes, and geometry

Visual footprints must agree with authoritative planar shapes:

- rectangles become cuboids or validated imported models with presentation-only height;
- circles become cylinders or generated circular geometry;
- floors use calm generated or cached surfaces;
- destructible map assets reconcile whole-placement normal, removed, or replacement visuals from
  replicated terminal outcomes;
- arbitrary or dynamic shapes use primitives/generated meshes rather than an approximate prop;
- imported environment scenes require a validated pivot, orientation, scale, footprint, and
  compatible visual profile.

A map theme provides client-owned ground, edge, lighting, palette, material, and default visual
choices. It is not a baked map image or style lock. An authored placement resolves one stable
visual profile, but neither the theme nor the model decides collision, destructibility,
replacement, or other gameplay behavior.

The client resolves shared stable theme and visual-profile IDs through its map presentation
catalogs.
Asset paths, tints, transforms, and fallbacks remain client-only and do not contribute renderer
details to the gameplay protocol. Presentation reconciliation keys include the accepted map
instance, recipe fingerprint, and theme so replacement cannot retain stale geometry or materials.

Occlusion should first be solved through camera elevation, wall height, material value, and map
composition. Selective fade, outlines, x-ray rendering, decals, or custom gameplay-world pipelines
require evidence that these simpler controls are insufficient.

## Asset forms

Choose the smallest representation that preserves readability and authoritative-shape agreement:

- imported GLB scenes for recognizable props, animated characters, and attached weapons;
- cached Bevy primitives for exact, repeated, or highly dynamic shapes;
- generated meshes for resolved perimeter geometry, terrain chunks, procedural rings, and other
  topology derived from authoritative state;
- Bevy UI for product flow, HUD, overlays, and projected fighter information;
- image, font, audio, and shader assets for bounded client presentation with explicit degradation.

GLB is the preferred shipped model format because it carries meshes, materials, hierarchy, skins,
and animation in one Bevy-supported asset. Each imported family retains its pack namespace and
relative texture layout. Common texture filenames must not be flattened across packs.

Animation clips may omit channels. Leaving defeat restores the imported bind pose before starting
a live loop so root or limb transforms cannot remain in a defeat pose. Grip, forward-axis, scale,
and anchor corrections belong to presentation profiles/constants rather than gameplay rotation.

Primitive meshes and materials are cached and shared. Generation-owned meshes have explicit
removal ownership; terrain chunks mutate owned meshes instead of allocating on every revision. A
unique mesh or material per fighter, projectile, tile, prop, or effect is not an accepted default.

## Asset sources and provenance

The asset boundary has four distinct sources:

| Source | Responsibility |
|---|---|
| `external_assets/` and `inspiration/` | Source material and retained concepts; not scanned or packaged as runtime content |
| `assets/manifest.ron` | Exact shipped third-party file inventory, provenance, license, requiredness, and fallback |
| `assets/catalogs/` | Client-owned paths, transforms, material values, theme profiles, and degradation mappings |
| `content/` | Stable server/client-neutral gameplay, map-asset, gameplay-profile, and theme identities |

Only assets owned by a current gameplay, map, product-shell, accessibility, or presentation use are
promoted into `assets/`. Availability in a source pack is not sufficient. Preview renders, source
archives, FBX/OBJ alternatives, overview images, and authoring metadata remain source-only unless a
specific runtime use is accepted.

Every redistributed third-party file must have exact provenance in the manifest and retained
license text where required. Original replacements should also preserve their source masters and
document the export pipeline needed to reproduce runtime derivatives.

## Asset admission checklist

A presentation asset or family is admitted only when:

1. a current gameplay, map, UI, audio, branding, or presentation owner requires it;
2. any third-party source permits redistribution and the manifest records its exact provenance;
3. its runtime format and dependency paths preserve a stable pack or product namespace;
4. orientation, scale, pivot, footprint, material response, animation, and real-size readability
   are verified in the supported client rather than inferred from a preview;
5. optional or shape-critical content has a deterministic, usable fallback;
6. missing and late readiness cannot block authority, navigation, or crash the client;
7. repeated spawn, screen transition, map replacement, restart, and reconnect release owned
   entities and generated assets;
8. loading and handles remain client-owned, while shared definitions reference stable IDs only;
9. the dedicated-server feature graph remains free of rendering, windowing, image, scene,
   animation, audio, device-input, shader, and client-asset dependencies;
10. tests and native checks are proportional to the lifecycle, readability, and performance risk.

## Server, protocol, and degradation boundaries

- dedicated servers and routed workers contain no cameras, meshes, materials, images, models,
  animation, audio, windowing, shaders, device input, or client assets;
- protocol state remains planar and never exposes process-local ECS entity identity;
- gameplay emits stable state and cues; client presentation resolves them to assets and effects;
- optional presentation failure cannot alter authoritative timing, collision, damage, status,
  scoring, admission, or results;
- `BRAWLER_FORCE_PRIMITIVE_WORLD=1` is a degradation and verification path within the sole 3D
  gameplay renderer, not a renderer choice or parallel content mode;
- primitive degradation preserves gameplay footprints, relationship cues, objectives, and distinct
  theme readability even when imported scenes are unavailable.

## Performance and verification

The baseline remains deliberately simple: cached meshes and materials, bounded map-asset visuals,
transients, restrained shadows, limited lighting, and Bevy's normal culling. Instancing, LOD, GPU
particles, bloom, deferred rendering, or custom gameplay-world pipelines require measured need.

Automated verification should cover coordinate conversion, exact footprints, projectile origins,
asset/catalog validation, lifecycle cleanup, animation recovery, relationship colors, projected-UI
query safety, reduced-effects bounds, degraded fallbacks, and server feature isolation. Native
checks should exercise supported modes, maps/themes, aspect ratios and UI scales, imported and
fallback paths, Dashboard preview, defeat/respawn, map destruction, objectives, and representative
combat density.

Imported map-object scale is validated from intrinsic asset bounds after the complete parent/child
transform chain, not from a profile number in isolation. A nearby authored tile provides a native
size reference when footprint readability matters. Static materialization and dynamic replacement
must share one runtime classification so an HP-bearing object cannot leave an obscuring static
duplicate. Imported and primitive-fallback transforms receive separate focused regressions when
their fitting rules differ.

## Envisioned direction

- a coherent original PewPew Blitz environment, fighter, weapon, animation, VFX, UI, icon,
  branding, and audio language that can replace or extend the current licensed foundation;
- additional themes and compatible visual families promoted only for concrete map needs;
- skin selection and replication with entitlement, contrast, silhouette, and relationship-cue
  validation;
- additional presentation profiles for genuinely supported fighter and weapon forms;
- advanced occlusion or rendering treatments justified by playtest and performance evidence.

These extensions must preserve the same authority, stable-identity, fallback, lifecycle,
readability, and server-isolation contracts. They do not justify a second gameplay renderer or a
generic asset framework in advance of a demonstrated use.
