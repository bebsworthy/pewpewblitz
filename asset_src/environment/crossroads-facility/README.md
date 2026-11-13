# Crossroads Facility environment kit

The environment kit for map presentation theme 1 (Crossroads Facility; BRL-0153). Every built-in
theme-1 map (Crossroads Facility, Barrel Yard, Feature Yard Wipeout/Hot Zone/Heist, Zombie Yard)
draws its imported environment pieces only from this kit. The kit is original project work built
entirely by the checked-in procedural Blender script; no third-party, commissioned, generated, or
hand-edited model data is involved.

## Files

- `build_kit.py` — the theme-1 palette and piece geometry. The kit contract shared with every
  theme kit (parts, bevel, palette bake, material, export, validation, review sheet) lives in
  `../kit_common.py` (BRL-0187).
- `kit.blend` — the generated source master: the `EXPORT__crossroads_facility` collection with one
  joined `mesh_<piece>` object per piece.
- `previews/kit-sheet.png` — generated review sheet. It re-imports every exported GLB (a round-trip
  check) and renders it with vertex colours from a 55-degree gameplay-like elevation.
- Runtime derivatives (not in this directory):
  `assets/brawler/models/original/environment/crossroads-facility/<piece>.glb`.

## Rebuild

From the repository root:

```sh
just facility-kit
# equivalent to
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  -P asset_src/environment/crossroads-facility/build_kit.py
```

Built and verified with Blender 5.2.3 LTS. Pass `-- --no-renders` after the script to skip the
review sheet. The run prints a `KIT_BUILD_REPORT` JSON line (bytes, triangles, bounds, suggested
`Contained` fill per piece) and fails if any export breaks the contract below. Two consecutive
builds produce byte-identical GLBs. Never hand-edit a GLB: change the builder and rebuild, then
update the matching `theme_kits` entry in `assets/catalogs/map_asset_visuals.ron` if a bound changed.

## Kit contract

- 1 Blender unit = 1 map cell, `+Z` up, ground-centred origin, cell footprint `-0.5..0.5`; the
  front of a piece faces `-Y` (towards the gameplay camera after glTF export).
- One shared chamfer (`BEVEL` = 0.045 cell) on every block edge and cylinder cap.
- One material per piece; colour is a flat per-face palette in `COLOR_0`. No normals (Bevy
  rebuilds flat normals), UVs, or textures; the client's toon material shades it.
- Baked shading lives in the palette: upward faces are lifted by `TOP_HIGHLIGHT` (top-face
  highlight) and every vertex is darkened towards the ground by a height gradient (baked AO).
- Budget: at most 2,500 triangles per piece (the largest, the crate, is 836).
- Exact pieces (walls) fill exactly one cell; `flat` pieces are under 0.1 cell tall.

## Pieces

| GLB | Visual profile | Use | Fitting |
|---|---|---|---|
| `wall-block.glb` | 1 (`wall-arena`) | slate concrete wall, two-block course, pale cap | Exact |
| `wall-course.glb` | 2 (`wall-dungeon`) | slate wall, offset courses, teal band | Exact |
| `crate.glb` | 24 (destructible cover) | honey-wood crate, dark frame, braced top | Contained 0.91 |
| `bush.glb` | 33 (tall grass) | chunky multi-lobe concealment bush; sways in game | Contained 0.96 |
| `barrier.glb` | 34 (breakable barrier) | steel posts, hazard-striped block | Contained 0.92 |
| `barrel.glb` | 37 (oil barrel) | red drum with hazard band | Contained 0.73 |
| `barrel-debris.glb` | 38 (barrel debris) | flat staves and lid | Contained 0.77, flat |
| `chest.glb` | 40 (treasure chest) | wood chest with gold straps | Contained 0.84 |
| `vault.glb` | 39 (Heist safe) | steel vault with gold dial | Contained 0.96 |
| `pickup.glb` | 42 (restoration pickup) | green canister with white cross | Contained 0.55 |
| `weapon-rack.glb` | 11 | wooden rack with three blasters | Contained 0.88 |
| `target.glb` | 12 | red and white target on a stand | Contained 0.72 |
| `pumpkin.glb` | 13 | chunky pumpkin | Contained 0.64 |
| `floor-hatch.glb` | 14 (floor detail) | steel floor grate | Contained 0.94, flat |

Water, rubble, effect tiles, and the arena border stay generated from the theme palette; the
water curb uses the theme's `water_edge` colour.

## Provenance

Original work of the PewPew Blitz project, so it is not a row in the third-party
`assets/manifest.ron`. The source of truth is this directory: the builder script reproduces the
runtime GLBs with the command above. `assets/licenses/pewpew-blitz-original-environment.txt`
points here.
