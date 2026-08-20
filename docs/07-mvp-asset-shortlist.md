# Runtime asset selection and provenance

## Status

V3 replaced the original 2D stand-in shortlist with a small shipped 3D selection. This document
records the current asset policy and the packs already proven in the client. It is not permission to
bulk-copy `external_assets/` into the runtime tree.

## Current shipped selection

The supported client loads three optional CC0 Kenney model families:

| Runtime asset | Source pack | Use | Deterministic fallback |
|---|---|---|---|
| `character-male-a.glb` plus its colormap | Mini Characters | Fighter model and named idle, walk, hold, shoot, and defeat animation clips | Team-colored sphere and facing marker |
| `blaster-a.glb` plus its colormap | Blaster Kit | Weapon attached to the imported character hierarchy | Small cuboid weapon |
| `block.glb` plus its colormap | Mini Arena | Repeated permanent-cover presentation where the authored footprint matches | Exact 64×64 cuboid cover |

These assets live under `assets/brawler/models/kenney/<pack>/`. Their exact original paths,
source URLs, authors, licenses, import dates, and fallbacks are recorded in
`assets/manifest.ron`. Runtime audio remains under `assets/brawler/audio/` and follows the same
manifest policy.

GLB is the runtime model format. Each selected model keeps the relative texture layout expected by
the file, including pack-local `Textures/colormap.png`; common filenames must not be flattened
across packs. FBX, OBJ, preview renders, source archives, and unused GLBs remain source material in
`external_assets/` and never enter the shipped asset scan merely because they are available.

## Asset roles

Not every visible object should become an authored model:

- imported GLB scenes own recognizable static props, animated characters, and weapons;
- cached Bevy primitive meshes own exact or highly dynamic shapes such as floors, arbitrary cover,
  projectiles, sentries, debris, fallback fighters, and transient combat effects;
- generated meshes own resolved circular/perimeter geometry and terrain chunks whose topology is
  derived from authoritative state;
- procedural planar meshes own Hot Zone fill/boundary and fighter ground markers;
- camera-projected Bevy UI owns fighter names, health values/bars, and local ammunition;
- normal screen-space Bevy UI owns menus, product flow, HUD, overlays, and settings.

The server loads none of these client assets. Gameplay definitions carry stable presentation IDs
or authoritative shapes, never GLB paths, mesh handles, materials, scene-node names, or textures.

## Import acceptance checklist

A runtime model is accepted only when all of the following are true:

1. A current map, fighter, weapon, or presentation profile owns the model.
2. Its license permits redistribution and its provenance is entered in `assets/manifest.ron`.
3. The GLB and every relative texture dependency are copied under one pack namespace.
4. Orientation, scale, pivot, footprint, material response, and animation names are verified in
   Bevy 0.19.1 rather than inferred from an isometric preview.
5. An exact primitive/generated fallback exists when the asset is optional or shape-critical.
6. Readiness degradation is bounded and cannot block authority or crash the client.
7. Repeated spawn, map replacement, restart, and reconnect release owned entities and generated
   mesh assets.
8. The dedicated-server feature graph remains free of render, image, scene, animation, and asset
   dependencies.

## External pack disposition

`external_assets/` contains broader Kenney packs for future evaluation, including Mini Dungeon,
Mini Forest, Pirate, Graveyard, and additional Mini Arena/Character/Blaster variants. They are not
runtime content and are not all visually compatible with one another. Promote a new family only
when a real theme or gameplay object owns it; choose one coherent visual language per map rather
than mixing packs as a catalog demonstration.

The supplied 512×512 isometric renders and 128×64 tile guidance are preview/Tiled-authoring
metadata. Brawler renders the GLBs through its fixed orthographic `Camera3d`; those PNG dimensions,
drawing offsets, and tile sizes do not define runtime scale, collision, or camera projection.

## Original-art direction

Kenney assets are a first-release foundation, not Brawler's permanent identity. Original models,
materials, animation, VFX, and UI art remain a later product-art slice. Replacements must preserve
the stable gameplay/presentation boundary and current readability language, so art iteration does
not alter hitboxes, objective geometry, team relation colors, or authoritative timing.
