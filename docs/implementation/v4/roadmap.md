# Version 4 implementation roadmap

## Purpose and scope

V4 turns V3's working 3D renderer into a reusable map-presentation and map-content foundation.
It improves the current arena, promotes a curated environment library from `external_assets/`,
splits built-in maps into scalable documents, and proves reuse with a second map/theme.

V4 does not change the simulation model:

```text
server-owned map document + stable object definitions
                         |
                         v
validated/resolved planar map snapshot
                         |
             +-----------+-----------+
             |                       |
             v                       v
authoritative geometry/regions   client theme + visual catalog
Avian 2D collision/game rules    GLB scenes/generated surfaces/border dressing
```

Models remain replaceable presentation; the headless server resolves stable definitions into
bounded planar geometry, spawn data, regions, and mode anchors. Player-facing map editing and
custom-map launch workflows are deferred to the root backlog and are not V4 deliverables.

## Accepted product decisions

1. **A map background is a theme, not a baked screenshot.** Normal ground should be one or a few
   generated planes with a calm theme material and repeatable UVs, plus sparse detail props. A
   full-map illustration may be an optional future surface, but should not be the default format:
   it scales poorly across map bounds, content changes, aspect ratios, and texture resolutions.
2. **The playable edge is modular and the outer world is decorative.** A theme supplies straight
   edge modules, corner modules, an outside surface, and bounded dressing rules. The client builds
   a wide visual band outside authoritative bounds; containment never depends on those models.
3. **Use a restrained fixed-perspective shooter camera.** The accepted correction keeps zero
   horizontal azimuth so map axes and movement remain screen-aligned, but replaces orthographic
   flattening with a restrained long-lens perspective field of view. A classic 45-degree isometric yaw remains
   outside the default because it also rotates apparent movement and aim axes.
4. **Themes provide defaults, not style locks.** A theme supplies default ground, edge, lighting,
   palette, and object-variant choices. Any explicit placement may select another compatible stable
   visual variant, so Mini Arena, Mini Dungeon, Graveyard, Forest, Pirate, and future original
   styles may be mixed in one map. Outside dressing may derive from theme defaults, map fingerprint,
   bounds, and a bounded seed; inside-arena objects are explicit, fingerprinted placements.
5. **Separate source packs, shipped assets, visual profiles, and gameplay definitions.** A source
   GLB filename never becomes a wire contract or map-file reference.
6. **Use one map document per map.** Shared definitions and indexes remain catalogs; complete map
   recipes do not remain in one ever-growing `maps.ron`.
7. **Obstacle behavior is explicit.** `obstacle.indestructible.*` and
   `obstacle.destructible.*` are different game-object definitions even when they share the same
   wall, tree, rock, fence, or barrel visual variant. The asset never decides destructibility.

The user accepted these decisions during M01 specification review on 2026-08-20.

## Version status

| Field | Value |
|---|---|
| Status | Feedback review |
| Current milestone | M01 — reusable environment library and first themed arena |
| Entry gate | V3 complete; user requested V4 map presentation, reusable assets, and scalable map storage on 2026-08-20 |
| Completion gate | Two distinct map documents prove reusable object/theme composition; current-map presentation, scalable storage, lifecycle, readability, and performance checks are accepted |

Allowed statuses are `Not started`, `Researching`, `Specification review`, `Implementing`,
`Verifying`, `User playtest`, `Feedback review`, `Complete`, and `Blocked`.

## Milestone overview

| Milestone | Status | Player-visible deliverable | Plan |
|---|---|---|---|
| 01 | Feedback review | First richly presented Crossroads theme, wide 3D perimeter, game-object taxonomy mapped to reusable visual assets, and accepted camera treatment | [milestone-01.md](./milestone-01.md) |
| 02 | Not started | Scalable one-document-per-map storage and expanded object-placement format | Create after M01 feedback |
| 03 | Not started | Second map/theme proof, usability/performance hardening, and V4 closeout | Create after M02 feedback |

## Ordering rationale and milestone gates

### M01 — Reusable environment library and first themed arena

Solve the visible problem first while proving the risky asset and composition seams. Establish a
minimal game-object taxonomy first, map each object kind to compatible data-driven visual variants,
promote the GLBs needed by the first arena and catalog comparison,
replace the dark tiled-floor impression, replace the thin cuboid perimeter with modular 3D edges
and a wide decorated outer band, and compare bounded camera candidates. Redesign Crossroads without
altering authoritative collision or mode layouts.

Gate: imported and primitive-fallback paths both present Wipeout and Hot Zone; floor, cover, edge,
corners, and requested prop families read clearly; no model changes collision; map replacement and
restart release owned entities; server feature isolation passes; the user accepts one camera,
perimeter, and floor direction.

### M02 — Scalable map documents and reusable object definitions

Split the monolithic catalog into shared definition catalogs, a deterministic built-in map index,
and one recipe per map. Expand M01's semantic objects into a bounded placement format while
preserving stable IDs, theme defaults, and per-placement visual overrides. Preserve
canonical fingerprints, bounded validation, resolved snapshots, routed admission, and existing map
identities. Recipes never contain arbitrary paths.

Gate: built-ins and non-preset fixtures round-trip independently; adding a map does not require
editing a monolithic recipe array or Rust source; duplicate IDs/keys and unsafe paths fail; content
fingerprints are deterministic across file enumeration order; both modes pass network tests.

### M03 — Second map/theme proof and V4 closeout

Build a materially different map grammar and visual theme from the same definitions, proving the
library is reusable rather than Crossroads-specific. Add product-facing built-in map selection only
as needed to exercise both maps/modes. Promote new assets only when the second theme owns them.
Complete usability, lifecycle, performance, feedback, documentation, and learning review.

Gate: two maps and two distinct theme defaults use the same resolver/presenter, and at least one map
mixes compatible wall/prop variants from different styles; no code branches on map identity;
fallback, lifecycle, native performance, readability, canonical checks, E2E tests, feedback triage,
and learning review pass.

## Asset organization target

```text
external_assets/                         ignored upstream workspace material
  vendor/kenney/<pack>/<version>/        complete unmodified distributions

assets/                                  shipped client files only
  brawler/models/kenney/<pack>/          selected GLBs + pack-local Textures/
  licenses/kenney-<pack>.txt             one retained license per promoted pack
  manifest.ron                           exact provenance/redistribution inventory
  catalogs/environment_visuals.ron       client path/scale/yaw/pivot/fallback profiles

content/v4/                              server/client-neutral authored data
  map_objects.ron                        stable game-object taxonomy, behavior and footprints
  map_visual_variants.ron                stable object/variant compatibility and fit policy
  map_themes.ron                         stable theme/presentation references
  maps/index.ron                         built-in metadata and deterministic file list
  maps/builtin/<map-key>.ron             one complete recipe per file
```

V4 may normalize ignored `external_assets/` paths when it adds an import/check script, but it must
not bulk-commit or ship those distributions. Runtime files retain pack namespaces because every
pack has its own `Textures/colormap.png` and common filenames collide.

## Cross-version policies

- Authoritative footprints, regions, containment, spawns, objectives, and runtime terrain remain
  planar and server-owned.
- A visual entry may define asset ID, scene/primitive kind, scale, yaw, vertical/pivot correction,
  shadow policy, fallback, and thumbnail; it never defines gameplay.
- A semantic map object is the primary reusable authoring unit. It may define taxonomy, bounded
  collision/region role, footprint, placement rules, display metadata, and compatible visual-variant
  IDs. A theme chooses defaults while a placement may choose any compatible variant; the object and
  map never store GLB paths or Bevy handles.
- Indestructible geometry, discrete destructible obstacles, and chunked destructible terrain have
  different authoritative lifecycles. They remain separate object roles even when presentation
  variants overlap.
- Object-to-variant compatibility is many-to-many where useful: one wall object supports several
  art styles, and one tree asset may present both a blocking tree object and a decoration object.
- Imported silhouettes must agree with authoritative footprints; shape-critical blockers retain
  primitive/generated fallbacks.
- Repeated GLBs share loaded assets. Map-owned entities and generated meshes have explicit
  generation ownership and cleanup.
- The dedicated server and routing packages remain free of rendering, scenes, images, and client
  asset paths.
- Thumbnails are catalog aids, not runtime tiles or collision sources.

## Outside V4

- player-facing map editor, custom-map save/load/launch, and authoring UI;
- user-uploaded assets, scripts, shaders, or arbitrary filesystem content;
- internet publishing, discovery, ratings, moderation, cloud persistence, or map monetization;
- collaborative editing or arbitrary custom game modes;
- 3D gameplay physics, vertical traversal, jumping, or replicated height;
- a general procedural generator for playable layouts;
- classic 45-degree isometric conversion unless later evidence and feedback select it;
- bulk promotion of every available Kenney model.
