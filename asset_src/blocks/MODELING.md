# Brawler block modeling guide

This is the canonical source-side guide for original Brawler walls, crates, vegetation bases, and
decorative one-cell blocks. The target is a friendly, chunky low-poly family that sits comfortably
beside the local KayKit BlockBits references while using original geometry, layouts, palettes, and
details.

## Accepted visual language

- Build from a few large readable forms. Give the silhouette, primary construction, and one accent
  different visual weights instead of distributing detail evenly.
- Prefer iconic abstraction over miniature realism. Once the material and object type read from the
  primary forms, stop; do not add literal construction pieces merely because a real object has them.
- Control perceived complexity through grouping, not part count alone. Several close-value boards can
  read as one quiet material field, while a blank panel plus disconnected accents can feel busier.
- Prefer full-volume construction. Stones, timbers, braces, posts, and plates should look structural,
  not like thin decorations glued to a generic cube.
- Use recessed negative space to explain assembly: mortar cores, plank gaps, panel seams, and sockets.
  Keep the recess visible but subordinate to the main material.
- Use broad low-poly bevels to catch the game's three-quarter lighting. Vary piece size, split
  position, course height, or bevel width slightly so the result feels authored rather than tiled.
- Use a small material family: normally two or three close values for the primary substance plus one
  recess/accent material. Variation should clarify pieces without becoming a checkerboard.
- Make the top readable. Brawler's camera exposes top faces, so they need intentional construction,
  not a forgotten flat lid or a copy of a side face.
- Judge assets at gameplay scale as well as in the source preview. Tiny texture-like geometry that
  disappears at distance is cost without readability value.

## Lessons from `wall-stone`

The accepted proof uses eight full-depth rounded ashlar stones in two offset courses around a
smaller dark mortar core. Its outer faces are flush with the cell boundary; the pieces are the wall,
not plates covering another wall. Different planar splits between the two courses create a useful
top pattern and prevent mechanical repetition.

Avoid these demonstrated failure modes:

- Raised face plates read as ceramic tiles attached to a cube.
- Perfectly straight Boolean grooves on large flat faces remain sterile even when technically inset.
- Many equal courses create noise and weaken the silhouette.
- Decorative panels and diamond-shaped chips can shift stone toward a sci-fi container vocabulary.
- Flat pale color without value grouping makes construction difficult to read.

These are project learnings, not universal prohibitions. A metal or alien block may intentionally use
panels, but its panels must express that material and construction rather than compensate for an
unresolved base shape.

## Source and game contract

- Keep one editable source file per asset at `asset_src/blocks/<asset-id>.blend`. Never add or save a
  second block into an existing block's source file.
- A block source may contain its one owned `mesh_<asset_id_with_underscores>` object, its matching
  `EXPORT__<asset-id>` collection, and the reusable `JIG_*` review objects. It must not contain another
  block mesh or export collection.
- `brawler-blocks.blend` is retained only as the restored wall-only compatibility file from the first
  proof; `wall-stone.blend` is the canonical named source. New assets always use their asset ID.
- Before every save, verify that the filepath, owned `mesh_*` object, and `EXPORT__*` collection all
  identify the same asset. Stop rather than overwriting a different block's source.
- Export one joined mesh named `mesh_<asset_id_with_underscores>` with a ground-centered origin,
  identity rotation, and identity scale.
- Use Blender `+Z` as up. One-cell exact walls occupy `x/y = -0.5..0.5`; keep their top just below one
  unit when practical (`wall-stone` uses `0.995`). Contained props must stay comfortably inside that
  footprint so adjacent assets do not visually collide.
- Keep ordinary blocks opaque, texture-independent where practical, and within four materials. Use
  more only when a reviewed visual requirement outweighs batching and maintenance costs.
- Aim below 2,500 exported triangles for an ordinary one-cell block. Spend geometry on silhouette,
  bevels, and construction gaps before surface speckle.
- Appearance never changes authoritative collision or gameplay semantics. The existing map fitting
  and collider profile remain the authority.

## Review and export loop

1. Compare the intended material/construction against the local reference contact sheets, then make
   an original arrangement rather than copying a reference mesh or texture.
2. Review a neutral three-quarter render with the one-cell jig visible. Check the top, two sides,
   silhouette, recess depth, and neighboring-cell clearance.
3. Reject the pass early if its material identity depends on explanation.
4. Join the export geometry, deduplicate material slots, set the origin and transforms, and record
   asset metadata on the object.
5. Export only the selected asset to `asset_src/blocks/exports/<asset-id>.glb` and render
   `asset_src/blocks/previews/<asset-id>.png`.
6. Re-import the GLB transiently and verify one mesh, expected materials and triangle count, identity-
   compatible bounds, and no cameras, lights, animation, skins, or unrelated source objects.
7. Remove the transient import and save a clean source file. Promotion into runtime `assets/` and
   catalogs happens only after visual acceptance and owned V13 integration work.

## Family-specific starting points

- Stone and brick: offset courses, full-depth blocks or genuinely carved faces, recessed mortar, and
  restrained value variation.
- Wood crates and walls: use a continuous chunky frame, one dominant brace rhythm, and a simple top.
  A few recessed boards may form one quiet light-value field; keep braces connected visually to the
  frame and recess lid boards inside its rim. Add fasteners only when the asset fails to read without
  them, and avoid turning every real construction joint into a separate visual event.
- Metal: frame, panels, seams, and fasteners may be explicit, but keep one dominant frame/panel
  hierarchy and avoid covering every face with equal greebles.
- Alien: begin with a strong asymmetry or repeated non-human motif, then use emission as an accent;
  emission must not be the only feature that identifies the asset.
- Ice: use a strong faceted silhouette, cool value separation, and limited translucent/emissive
  accents only after confirming the current Bevy material path supports them safely.
