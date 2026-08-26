# Riff V2

Riff V2 is the orthographic-correction pass derived from the frozen V1 technical foundation.
Nothing in `../v1/` is modified by this build.

## Assets

- `riff-v2.blend` — canonical V2 Blender source
- `chr_riff-v2.glb` — V2 game-facing export
- `build_riff_v2.py` — deterministic V2 builder/exporter
- `previews/` — front, three-quarter, gameplay, and animated run-pose checks
- `comparison/` — exact front, side, and back orthographic renders and reference comparison boards
- `comparison/reference-proportion-landmarks.png` — user-supplied horizontal landmark analysis used
  for the first vertical-proportion correction
- `comparison/reference-proportion-review-02.png` — follow-up landmark review used to redistribute
  height without moving the planted feet

## Verified contract

- Source geometry: 72 separated `GEO_*` objects
- Source triangles: 9,688
- GLB round-trip triangles: 9,780
- Armature: one, 38 bones
- Required sockets: all seven present after GLB round-trip
- Animation actions: all eleven present after GLB round-trip
- GLB size: 1,222,588 bytes
- Visible source height: 2.1373 units (`Z = 0.04` through `Z = 2.1773`)
- Up/front axes: `+Z` / `-Y`
- Weapon mesh: intentionally absent

## Orthographic corrections from V1

- Replaced the rectangular head with a faceted, rounded, tapered volume.
- Rebuilt the ears with asymmetrical front silhouettes and different fore-aft paths.
- Added stronger ear depth so the profile no longer collapses into a single thin slab.
- Replaced the mask-like muzzle with a smaller integrated lower-face plane.
- Enlarged the eyes, angled the brows, and rebuilt the mouth as a playful `w` shape.
- Added restrained cheek marks and smaller side tufts.
- Rebuilt the jacket as a short wraparound orange volume with a complete back.
- Added a large rear scarf bow, central knot, and two tails.
- Lowered the relaxed arms, reduced the hands, and made the palms more angular.
- Rebuilt the shorts as a unified waist and flared leg silhouette.
- Added heel, tongue, lace, toe, upper, and sole grouping to the sneakers.
- Reduced, recolored, and tucked the tail against the shorts.
- Corrected authored sRGB colors to Blender linear values for stronger game-lighting separation.
- Used the user's horizontal landmark analysis to compress both shoe-to-belt and chin-to-belt spans.
- Used the follow-up landmark review to move the ears, goggles, head, and shoulders downward while
  preserving the foot contact plane.
- Re-measured the front, side, and back character bounds from the orthographic presentation and
  rebuilt the silhouette around a shared ground plane and character height.
- Rebuilt the stance with outward-splayed short legs and widely separated oversized sneakers.
- Replaced the shallow face plane with a compact projected muzzle constrained by the side view.
- Rebuilt the jacket front as open polygonal panels and side lapels rather than attached boxes.
- Reshaped the ears with broad lower-middle sections, asymmetric lean, and tapered faceted tips.
- Matched the darker orange, purple, and gold presentation palette more closely.
- Changed comparison generation to crop both subjects, normalize them to one height, and align
  their ground planes. This prevents camera margins from disguising proportion mismatches.
- Moved the skeleton pivots and runtime sockets with their visible parts rather than scaling only
  the render meshes.

## Remaining visual differences

V2 is an original game-ready reconstruction from raster presentation views, not the unavailable
source mesh. The aligned neutral landmarks and silhouette now closely track the supplied front,
side, and back views. Exact polygon facets, jacket folds, lens highlights, and shoe panel topology
remain simplified to preserve the low-poly game asset contract.
