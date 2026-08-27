# V13 Milestone 01 -- original environment-kit pipeline proof

## Status

`Specification review`

Research completed on 2026-08-26. Production modeling, catalog changes, and map integration must not
begin until the user validates this specification and V12 has completed.

## Player-visible outcome

Brawler has a small original environment family that reads cleanly from its fixed gameplay camera:
one stone wall fills one authoritative blocking cell, one wooden crate reads as contained cover, one
green tall-grass cluster communicates the full concealing cell without becoming an opaque cube, and
one arcane block demonstrates safe colored detail. The accepted family appears in at least one
advertised gameplay map; a development gallery alone is not the completion claim.

The same checked-in Blender source and command can reproduce the shipped GLBs. Missing or invalid
models still degrade to footprint-correct primitive/generated presentation without changing
gameplay, networking, admission, or match readiness.

## Research inputs and findings

### Local product and runtime contracts

- `docs/11-art-and-presentation-direction.md` requires stylized toy-like forms, recognizable
  silhouettes, matte graphic materials, restrained detail, exact authoritative footprints, optional
  client assets, deterministic fallbacks, and preserved source masters for original replacements.
- `docs/16-grid-map-asset-system.md` keeps gameplay profiles, stable map-asset identities, visual
  profiles, sparse recipes, and client-only source paths separate. Adjacency is derived from canonical
  four-neighbor masks rather than authored into recipes.
- `docs/17-concealment.md` makes tall grass real server-owned `HideOccupants` terrain. A foliage model
  can present that public boundary but cannot grant concealment.
- The authoritative cell is 32 by 32 world units. The gameplay camera targets about 14 cells
  vertically at a 27-degree vertical field of view and 55-degree elevation.
- The current client uses Bevy 0.19.1 `Gltf` assets and their default `WorldAsset` scenes. Imported
  environment assets are optional and resolve through `assets/catalogs/map_asset_visuals.ron`; the
  dedicated server does not load that catalog.
- Existing generated tall grass is one rotated primitive per cell. It does not provide a useful
  material family or convincing foliage silhouette, and its presentation name still says
  "non-concealing" despite the completed concealment gameplay contract.

### KayKit Block Bits reference audit

Reference root:
`external_assets/KayKit_BlockBits_1.0_FREE/`

The retained pack is CC0 and includes 40 glTF source models plus FBX/OBJ alternatives, a shared
1024-square texture atlas, an overview, and a sample scene. The glTF files use one default scene, one
mesh, one material, and centered geometry. Most solid blocks have exact bounds from -1 to +1 on each
axis. The audited range is approximately 108 to 2,076 triangles per model; the ornate colored block
is about 1,824 triangles, brick about 844, metal about 672, wood about 1,168, and the pack's grass
block about 520.

Useful reference principles:

- a clear square plan and almost cubic mass;
- softened/chamfered outer edges that catch the top light;
- large calm surfaces with one or two readable motifs;
- face rhythm from bricks, planks, plates, stones, bands, or inset panels;
- compact saturated accents over a mostly matte base;
- consistent normalized bounds and pivot discipline across the family.

V13 deliberately does not copy the exact meshes, UV islands, atlas, colored-ring motif, brick layout,
or other face decoration. The pack's grass/tree models are solid terrain cubes and are not accepted as
the visual model for Brawler's passable tall grass.

### Blender and exporter compatibility

The connected authoring application is Blender 5.2.0 LTS. Its glTF exporter is available, the scene
uses metric units at scale 1.0, and the installed exporter exposes the required GLB, selected-object,
modifier-application, +Y-up, normals, materials, camera/light, animation, skin, morph, and Draco
controls.

Exact Bevy 0.19.1 source confirms that the glTF loader exposes a default scene and supports ordinary
PBR base color, metallic/roughness, opaque/alpha modes, double-sided materials, and emissive factors.
M01 intentionally uses the smallest compatible subset: opaque PBR materials, double-sided opaque
grass, and one restrained emissive accent. It does not require transmission, alpha blending, normal
maps, tangents, Draco, animation, skins, morphs, cameras, or lights.

## M01 proof asset set

| Source key | Visual purpose | Geometry/fitting | Gameplay reuse |
|---|---|---|---|
| `wall-stone` | irregular chunky masonry, slate base with lighter chipped edges | exact one-cell solid | existing indestructible one-cell wall profile |
| `crate-wood` | plank faces, diagonal brace, corner blocks, large top silhouette | contained one-cell prop | existing cover profile selected after V12; appearance adds no hit points |
| `tall-grass-green` | several broad low-poly blade fans with gaps and a readable perimeter | contained one-cell foliage cluster | existing passable `HideOccupants` profile |
| `block-arcane` | dark violet block with inset asymmetric chevron/ring language and small cyan emissive seams | exact one-cell solid | existing indestructible one-cell wall profile |

The development gallery shows all four together, but only assets with an owned map placement are
promoted to production catalogs. The map selected for M01 is chosen from the completed V12 advertised
set so this plan does not retroactively edit V12's completed recipes.

### Source-only stone prototype -- 2026-08-26

Status: **awaiting visual feedback; not promoted to production**.

The user requested one trial block before V13 implementation. Blender MCP authored an original
masonry block in `asset_src/blocks/wall-stone.blend`, rendered
`asset_src/blocks/previews/wall-stone.png`, and exported the source-side derivative
`asset_src/blocks/exports/wall-stone.glb`. The first visual pass used raised bevelled face plates;
the user rejected that construction because it read as a cube plated with tiles rather than a
stone wall. A second pass carved regular masonry grooves into a monolithic cube, but the user found
that result visually sterile. Direct topology inspection of the local `bricks_B.gltf` reference
showed that its appealing rounded masonry is assembled from many disconnected full-volume pieces,
not a flat decorated shell. The third pass therefore uses an original arrangement of eight
full-depth, rounded ashlar stones in two offset courses around a smaller recessed mortar core. The
outer stone faces remain flush with the exact cell boundary; none are thin plates attached to a
cube. Four restrained gray/sage tones, varied stone proportions, broad bevels, the horizontal bed
joint, and different planar splits between courses provide the visual hierarchy.

The current source prototype has an identity transform, ground-centered origin, exact 1.0 by 1.0
planar bounds, height `0.995`, 504 vertices, 972 triangles, and four opaque matte materials. Its GLB
is 54,680 bytes. A transient Blender re-import reproduced one mesh, all four materials, 972
triangles, and dimensions exactly `1.0 x 1.0 x 0.995`.

This probe does not allocate map or visual IDs, modify catalogs or recipes, enter runtime `assets/`,
or change V12. Its geometry, palette, and face rhythm remain provisional until user review.

### Source-only wooden crate prototype -- 2026-08-26

Status: **awaiting visual feedback; not promoted to production**.

The second proof applies the recorded block language to a different material and construction. Its
first assembly used a recessed body, four full-height posts, four gapped boards on every side, frame
rails, one diagonal brace per side, four lid boards, and six iron fasteners. The user rejected that
pass because the many literal construction details tried too hard to be realistic.

The corrected `crate-wood` reduces the idea to one broad rounded body, four posts, two frame bands,
one large brace per side, and a two-piece lid. It removes the wall-plank repetition, nails, secondary
trim, and fine lid assembly. Three wood values establish body/frame/accent hierarchy without
textures. The construction now reads as a toy-like game block in one glance rather than a miniature
real crate.

The source object lives in its independent `asset_src/blocks/crate-wood.blend`; its review render is
`asset_src/blocks/previews/crate-wood.png` and its source-side derivative is
`asset_src/blocks/exports/crate-wood.glb`. Before the simplification feedback, the detailed joined
mesh had contained dimensions `0.8982 x 0.8982 x 0.76`, 1,258 vertices, 2,344 triangles, four opaque
materials, and a 107,752-byte GLB.

After the simplification correction, the intermediate joined mesh had a ground-centered origin, identity
transform, contained dimensions `0.859 x 0.859 x 0.72`, 456 vertices, 836 triangles, and three opaque
materials. The replacement GLB is 51,800 bytes. A transient re-import reproduced one mesh, all three
materials, the same dimensions, and the same triangle count. Like the stone proof, this crate remains
source-only until visual acceptance and owned V13 promotion.

A subsequent comparison against a user-supplied crate image showed that the simplified blank panels
had removed useful material identity. The reference's apparent simplicity came from grouping its
vertical boards into one pale field beneath a continuous soft frame, not from eliminating boards.
It also integrated long braces into the frame and recessed its lid beneath a rim. The image was used
only to identify these broad principles; the replacement keeps original geometry, proportions,
palette, and layout.

The comparison-driven pass now uses three recessed boards on each side and lid, a continuous rounded
frame whose rails terminate into its posts, one long brace per side meeting both frame bands, and a
strong pale-board/medium-frame value split. The current joined mesh has a ground-centered origin,
identity transform, contained dimensions `0.86 x 0.86 x 0.72`, 1,280 vertices, 2,432 triangles, and
three opaque materials. Its 137,444-byte GLB re-imported as one mesh with all three materials, the
same dimensions, and the same triangle count. It remains source-only pending visual feedback.

### Reusable modeling knowledge -- 2026-08-26

`asset_src/blocks/MODELING.md` is the canonical project guide for the accepted visual language,
demonstrated failure modes, source/game contract, review/export loop, and family-specific starting
points. The discoverable personal Codex skill `brawler-block-modeling` routes future Brawler block
tasks to that guide and preserves the V13/V12 boundary. Its scaffold and metadata passed the bundled
skill validator in an isolated `uv` environment.

The initial source-management assumption incorrectly placed both proofs in one shared
`brawler-blocks.blend`. User feedback established one `.blend` per block as the required ownership
boundary. The preserved wall and crate objects were split into `wall-stone.blend` and
`crate-wood.blend`, each with only its owned export object/collection plus the review jig.
`brawler-blocks.blend` was restored to wall-only compatibility, and an explicit
`brawler-blocks-recovery.blend` snapshot preserves the pre-split state until the user accepts cleanup.
The guide and reusable skill now require filepath/object/export-collection agreement before saving.

## Original visual language

### Shape language

- Keep the cell-readable mass first: walls remain square from above, crates remain obviously box-like,
  and grass remains visibly passable foliage rather than a solid terrain voxel.
- Use 3-6% normalized bevel widths, one or two bevel segments, weighted/controlled normals, and flat
  or deliberately softened shading. Micro-bevel noise is not a substitute for silhouette.
- Prefer a few large modeled seams, chips, braces, plates, or blade fans. Do not add photoreal grain,
  dense surface noise, tiny bolts, thin wires, or details that vanish at the 14-cell viewport.
- Each face may vary, but rotation must not change the gameplay reading or expose an unfinished side.
- The original recurring motif is an offset chevron/diamond cut with one asymmetric notch. It can
  appear sparingly on ornate or alien assets; it is not stamped onto natural stone, earth, or grass.

### Color and material language

- Environment colors stay below the saturation and brightness of local/allied/enemy relationship
  rings, health cues, previews, projectiles, and objectives.
- Reserve pure saturated red, blue, and green for gameplay cues. Colored environment variants use
  quieter coral, cobalt, jade, amber, violet, cyan, and earth tones.
- Default surfaces are matte: roughness normally 0.65-0.9 and metallic 0.0. Metal may use metallic
  0.65-0.85 with roughness 0.45-0.7. Ice remains opaque pale cyan/white with roughness 0.25-0.45;
  transparency is not part of the core set.
- Emissive color is confined to small alien/arcane insets and remains subordinate to combat effects.
  No harmless asset uses warning stripes, fire, poison green, healing crosses, team marks, or an
  objective halo.
- M01 uses material colors rather than a texture atlas. One asset may use at most four material
  slots. UVs, image textures, and normal maps require a demonstrated visual need in a later milestone.

## Canonical Blender asset standard

### Units, axes, origin, and bounds

- Blender unit system: Metric, scale length 1.0.
- Author in normalized map units: one Blender meter across X and Y represents one Brawler cell for an
  exact asset; Blender Z is presentation height. The glTF exporter converts to +Y-up.
- Every exported collection has one root at world origin with identity rotation and scale.
- Solid exact assets have X/Y bounds of 1.0 by 1.0 within a 0.1% tolerance, bottom Z at 0, and height
  between 0.92 and 1.08 unless native occlusion evidence changes the family standard.
- The contained crate uses at most 0.94 by 0.94 of the cell and 0.72-0.84 cell height.
- Tall grass uses 0.86-0.94 of the cell, 0.52-0.68 cell height, no solid plinth, and visible gaps
  between blade groups. Geometry must not overflow its cell after any allowed quarter-turn.
- Apply object rotation and scale before export. Negative scale, non-finite values, zero-area bounds,
  unapplied mirror transforms, and hidden collision proxy meshes fail validation.

### Topology and budgets

| Asset class | Target triangles | Hard M01 ceiling | Material slots | Notes |
|---|---:|---:|---:|---|
| wall/block | 500-1,800 | 2,500 | 4 | no interior faces; top silhouette must remain calm |
| crate | 400-1,400 | 2,000 | 4 | braces and corner blocks share the same exported root |
| tall grass | 250-650 | 900 | 3 | modeled blades/fans, opaque and double-sided; no alpha cards |

Source meshes may retain editable quads and non-destructive modifiers. Export applies modifiers and
triangulates deterministically. Degenerate triangles, loose unintended geometry, duplicate coplanar
faces, invalid normals, and unexpected extra scenes fail the export check. Non-manifold blade edges
are allowed only for the explicitly tagged grass collection.

### Source organization and naming

The user selected the existing `asset_src/blocks/` source root. The source-owned layout is:

```text
asset_src/blocks/
  wall-stone.blend
  crate-wood.blend
  tall-grass-green.blend
  block-arcane.blend
  MODELING.md
  previews/
tools/blender/
  export_environment_kit.py
  validate_environment_kit.py
assets/brawler/models/original/environment/
  walls/wall-stone.glb
  crates/crate-wood.glb
  vegetation/tall-grass-green.glb
  blocks/block-arcane.glb
```

The source file uses one `EXPORT__<asset-key>` collection per runtime GLB. `WORK__`, `JIG__`, and
`REFERENCE__` collections are never exported. Mesh objects use `mesh_<asset_key>_<part>` and
materials use `mat_<family>_<role>`. Preview cameras, lights, grid cells, fighter scale references,
text labels, and KayKit reference geometry never enter an export collection.

`asset_src/blocks/` owns editable `.blend` masters, block-specific notes, and review renders; none of
those files are runtime-scanned. Shared Blender automation remains under `tools/blender/` so a later
original asset family can reuse proven export checks without placing tooling inside runtime assets.
The exact files are created only after specification approval. If V12 establishes a conflicting
export-tool convention first, V13 follows that completed convention rather than creating a second
one.

## Reproducible Blender-to-Brawler pipeline

1. **Brief.** Record asset key, owned map use, gameplay profile to reuse, footprint, fitting policy,
   palette, silhouette sketch, triangle/material budget, and fallback class.
2. **Blockout.** Model inside a one-cell jig with a fighter-height/readability reference. Validate top,
   gameplay-camera, and side silhouettes before surface detail.
3. **Detail and materials.** Add only gameplay-scale seams/chips/braces/blades, apply the approved
   matte palette, and keep cue-reserved colors out of the environment.
4. **Interactive review.** Use Blender MCP for bounded object inspection, scripted edits, and viewport
   screenshots. Save every accepted result in the source `.blend`; the MCP transcript is not an
   artifact or source of truth.
5. **Source validation.** Run the checked-in Blender validator in background mode. It checks collection
   naming, bounds, pivot, transforms, visible mesh ownership, triangle count, material count/types,
   forbidden data blocks, and class-specific manifold exceptions.
6. **Export.** A checked-in script exports one collection at a time with Blender 5.2 LTS using GLB,
   selected objects, applied modifiers, +Y-up, normals, materials, no cameras/lights, no animations,
   no skins/morphs, no tangents, no custom extras, and no Draco compression.
7. **Derivative validation.** Re-open or inspect every GLB, require one usable default scene and finite
   bounds, compare its class/footprint to the brief, and load it through the exact Bevy 0.19.1 client
   path. Repeated export must reproduce the same asset list and semantic bounds; byte-for-byte hash
   stability becomes a gate only if Blender proves deterministic in practice.
8. **Promotion.** Add only accepted derivatives to the original environment namespace, record the
   source master and export command/version as project-owned provenance, and add client visual
   profiles plus deterministic fallbacks. Source masters do not enter runtime `assets/`.
9. **Map integration.** Allocate stable IDs after the completed V12 catalog, reuse explicit gameplay
   profiles, revise only maps that own the variants, advance their admission revisions, and prove
   that collision, concealment, destruction, navigation, objectives, and spawns are unchanged.
10. **Native acceptance.** Review a generated contact sheet, the development gallery, and a real match
    at the accepted camera. Adjust the source master, re-export, and rerun affected validation rather
    than hand-editing GLBs.

The canonical operator surface should be a small pair of `just` recipes wrapping the pinned Blender
commands, for example an export command and a read-only validation command. Exact recipe names are
chosen during implementation to avoid conflicting with commands added by V12.

## Runtime and ownership contract

- Source `.blend` files, exporter scripts, preview scenes, and contact sheets are development-only.
- Runtime GLBs, handles, materials, intrinsic bounds, and scene entities remain client-owned under
  `WorldPresentationPlugin` and the focused environment-asset modules.
- Shared `MapAssetId`, `MapVisualProfileId`, footprints, and gameplay-profile references remain stable
  content identities; source paths and Blender names do not enter the protocol.
- Exact wall/block scenes fill the authoritative planar footprint without non-uniform distortion.
  Contained crate and grass scenes stay inside it and remain grounded from intrinsic bounds.
- Missing, late, empty, invalid, or overflowing scenes emit bounded diagnostics and select the existing
  deterministic fallback. They cannot block loading, authority, matchmaking, or recovery.
- One asset handle and its materials are reused across placements. M01 does not allocate one unique
  mesh or material per cell and does not add LOD or instancing without measurements.
- Grass presentation may use placement quarter-turns for repetition control. New random seeds,
  authored adjacency variants, or recipe fields are out of scope for the proof.

## Network and gameplay behavior

M01 is presentation-only. Clients still receive resolved map placements and observer-permitted
gameplay state; they never send model choice, collision, destruction, concealment, or map edits.
Walls reuse the existing blocking profile, tall grass reuses `HideOccupants`, and the crate uses one
existing cover profile selected after V12. The visual catalog and GLBs remain absent from server-only
builds. No application-protocol change is expected because no wire shape or meaning changes.

If the requested crate behavior does not match any completed gameplay profile after V12, M01 stops at
specification review for that item rather than inventing durability from its appearance.

## Implementation sequence

1. After V12 completes, reconcile this plan against its final fitting implementation, ID allocation,
   visual schema, source conventions, and accepted maps; update V13 only.
2. Create the per-asset source layout, copy the one-cell/fighter/camera review jig into each source,
   and establish the shared material palette.
3. Implement the smallest Blender validation/export scripts and wrap them in canonical `just` recipes.
4. Model and review `wall-stone`; prove exact bounds and default-scene loading end to end.
5. Model `crate-wood`, `tall-grass-green`, and `block-arcane`; exercise contained, double-sided, and
   emissive cases without expanding renderer architecture.
6. Produce GLBs and a contact sheet, add original provenance and client catalog entries, then add the
   smallest shared map-asset variants required by the selected map.
7. Integrate the accepted subset into a development gallery and at least one advertised map while
   preserving all gameplay profiles and recipe topology except visual asset classification.
8. Run automated, routed, fallback, native readability, and representative-density checks; triage
   feedback and perform the milestone learning review.

## Implementation checklist

- [ ] Reconcile against completed V12 without modifying V12 history or reusing IDs.
- [ ] Create the independent per-asset source masters and excluded preview/reference collection
      structure; verify no source contains another block's mesh or export collection.
- [ ] Add the one-cell, fighter-scale, and accepted-camera review jig.
- [ ] Add source and derivative validation with focused failure fixtures.
- [ ] Add one collection-per-GLB export and canonical `just` wrappers.
- [ ] Model, export, and accept stone wall, wooden crate, green tall grass, and arcane block.
- [ ] Record project-owned provenance, source master, Blender/export version, output hashes, and
  deterministic fallback for every promoted derivative.
- [ ] Add client-only visual profiles and only the shared asset variants owned by selected maps.
- [ ] Prove server feature isolation and unchanged gameplay-profile assignment.
- [ ] Integrate the accepted family into a gallery and at least one advertised map.
- [ ] Run automated, routed, native, fallback, and performance evidence.
- [ ] Triage user feedback, rerun affected checks, and complete the learning review.

## Verification plan

### Automated source/export checks

- exact wall/block X/Y bounds, grounded minimum Z, identity root transform, and quarter-turn safety;
- contained crate/grass bounds, grass-only non-manifold allowance, and no hidden export helpers;
- triangle/material budgets, finite vertices/normals, no degenerate triangles, and no unexpected
  cameras, lights, animations, skins, morphs, images, or extra scenes;
- one default GLB scene, expected mesh/material count, class-consistent bounds, and Bevy 0.19.1 load;
- repeated export preserves the expected file list, names, semantic bounds, material factors, and
  source-to-output ownership.

### Catalog and gameplay checks

- every new shared visual identity resolves exactly once client-side and every shipped path appears
  in the asset inventory with original provenance;
- walls keep the existing blocking/projectile behavior, grass keeps pass/pass plus
  `HideOccupants`, and the crate exactly matches its selected existing cover profile;
- map occupancy, navigation, concealment volumes, destruction/recovery, objectives, spawns, recipe
  fingerprints, admission revisions, and bot behavior remain valid;
- primitive fallback preserves footprint and gameplay class when each new GLB is unavailable;
- server, client, and network-test role checks pass, and the server feature graph contains no Blender,
  GLB scene, material, image, rendering, windowing, audio, or device-input dependency.

### Native visual and performance checks

- contact sheet: neutral three-quarter, top, and side views with one-cell and fighter references;
- normal gameplay camera: square blockers cover their cells, crate containment does not imply a
  walkable gap, grass perimeter and passability are legible, and fighters/projectiles/UI remain the
  visual priority;
- all accepted themes: matte response, bevel highlights, stone/wood/grass class recognition, and
  arcane emissive restraint remain readable without team-color confusion;
- adjacent rotations: no unfinished face, pivot jump, seam overflow, collider/mesh gap, or obvious
  repeating artifact at representative patch size;
- normal and `BRAWLER_FORCE_PRIMITIVE_WORLD=1` paths survive map load, restart, replacement,
  reconnect, conceal/reveal transitions, and teardown;
- representative dense wall and grass patches keep bounded entity, mesh, material, image, and frame
  diagnostics. The 512-square extreme remains an authoring bound, not an M01 performance claim.

## Playtest handoff

The handoff supplies the canonical export/validate commands, Blender source location, contact sheet,
selected map and game type, controls, normal/fallback launch paths, and these requested observations:

1. Can wall, crate, grass, and arcane block be identified immediately at match scale?
2. Does every blocker visually agree with its cell and collider from all approach directions?
3. Does the crate look blocking without masquerading as an indestructible wall?
4. Does grass communicate concealment and passability without hiding too much combat action?
5. Do bevels, material values, and color accents feel like one original family rather than four
   unrelated test assets?
6. Does the arcane color treatment remain subordinate to team, objective, projectile, and status cues?

## Exit criteria

- the user accepts the original visual language and all four proof assets at gameplay scale;
- the checked-in source master and canonical commands reproducibly produce validated Bevy-ready GLBs;
- at least one advertised map uses the accepted family without changing authoritative behavior;
- normal and primitive fallback paths pass lifecycle, routed, server-isolation, readability, and
  representative-density checks;
- every feedback item is implemented, deferred to the V13 roadmap, rejected with rationale, or held
  for more evidence; and
- the learning review records modeling, export, integration, and playtest corrections before M01 is
  marked complete and M02 is specified.
