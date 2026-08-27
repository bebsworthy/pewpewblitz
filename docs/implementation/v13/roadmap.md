# Version 13 implementation roadmap

## Purpose and scope

V13 establishes Brawler's first original, Blender-authored environment kit and a reproducible path
from source master to Bevy-ready GLB. The version covers the square obstacle, crate, and concealing
vegetation families requested by the user, integrates accepted assets into real gameplay maps, and
begins replacing temporary third-party or generated environment presentation without changing
server-owned gameplay behavior.

KayKit Block Bits 1.0 FREE is a retained CC0 style reference. V13 borrows its useful production
principles--chunky one-cell silhouettes, restrained detail, softened edges, compact palettes, and
interchangeable families--but authors fresh Brawler geometry, face patterns, materials, and foliage.
KayKit meshes, UVs, texture atlas, and decorative motifs are not the source masters for the new kit.

V13 starts after V12 closes. It does not modify or reinterpret V12's completed specification,
asset IDs, recipes, or evidence. At V13 implementation time, new stable IDs are allocated after the
completed V12 catalog rather than reserving numeric ranges in advance.

## Version status

| Field | Value |
|---|---|
| Status | Specification review |
| Current milestone | M01 -- original environment-kit pipeline proof |
| Entry gate | Satisfied: V12 completed and was accepted on 2026-08-27 |
| Completion gate | The required original wall, crate, and tall-grass families are accepted at gameplay scale, reproducibly exported, integrated into owned maps, verified in normal and fallback presentation, and documented with a learning review |

## Requested and desired asset inventory

The version-level inventory separates committed families from candidates so the first pipeline proof
does not silently expand into an unreviewed art pack.

### Required by V13 completion

| Family | Required variants | Shared gameplay intent |
|---|---|---|
| Solid wall/block | earth, brick, wood, metal, stone, ice | existing indestructible one-cell blocking wall behavior |
| Crate | wood, metal, alien | existing one-cell cover behavior selected in the owning milestone; no new durability rule is inferred from appearance |
| Tall grass | green, dry brown, alien | existing passable `HideOccupants` terrain behavior |

### Variety candidates

- one neutral ornate block with restrained coral, cobalt, jade, and amber presentation colorways;
- one dark arcane/alien block with a small violet/cyan emissive accent;
- one multicolor mosaic block whose color does not resemble a team or objective marker;
- concrete, ceramic, bone, and opaque crystal wall families when an owned map needs them;
- golden/autumn grass, marsh reeds, frost grass, and crimson/coral alien grass;
- visual edge, corner, end-cap, or shape variants only after the existing adjacency masks demonstrate
  that one rotated asset is insufficient.

Hazard stripes, explosive markings, fire, poison, healing, team emblems, and objective colors are not
neutral variety. They are reserved for matching gameplay affordances and cannot be added as harmless
decoration merely to increase color count.

## Milestone overview

| Milestone | Status | Player-visible deliverable |
|---|---|---|
| 01 | Specification review | A reproducible Blender-to-Brawler pipeline proven by an original stone wall, wooden crate, green tall grass, and arcane block, with at least one accepted asset family visible in an advertised gameplay map |
| 02 | Not started | The required earth/brick/wood/metal/stone/ice wall set, wood/metal/alien crate set, and accepted colored-block variants, specified from M01 evidence and integrated where maps own them |
| 03 | Not started | Green/brown/alien tall-grass completion, accepted additional foliage variety, broader map rollout, redundant temporary-presentation retirement, and V13 closeout |

M02 and M03 receive milestone files only when they become next. Their exact model list, map
classification, budgets, and retirement targets incorporate M01 native readability and performance
evidence rather than being pre-authored now.

## Version boundaries

- V13 changes client presentation and authored content; it does not add collision, hit points,
  destruction, hazards, pickups, concealment rules, scoring, or objective behavior.
- A wall, crate, or grass appearance never decides gameplay. Shared map assets continue to reference
  explicit existing gameplay profiles, and the server remains authoritative.
- V13 does not add a map editor, general material framework, procedural modeling framework, custom
  renderer, custom shader pipeline, LOD system, or instancing path without measured need.
- Source masters and export tooling are tracked outside runtime `assets/`; only accepted runtime GLBs
  and their owned metadata are promoted into the client asset tree.
- V12's completed content may remain available as a fallback or historical family. V13 retirement
  removes only assets proven unused after map, catalog, manifest, and fallback audits.
- The primitive-world override remains a degradation and verification path inside the sole 3D
  renderer, not a competing content mode.

## Ordering

M01 proves the authoring and runtime contract with four deliberately different cases: exact solid
geometry, a contained prop, double-sided foliage geometry, and restrained emissive presentation.
M02 expands solid obstacles and crates only after that proof is accepted. M03 then expands vegetation
and rolls the coherent kit through maps, because grass density, occlusion, repetition, and shadow
cost need native evidence before multiplying variants.
