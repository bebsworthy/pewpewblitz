# Tidal Garden environment kit

The environment kit for map presentation theme 2 (Tidal Garden; BRL-0187). Every built-in theme-2
map (Tidal Garden, Powderline Vault) draws its imported environment pieces only from this kit. The
kit is original project work built entirely by the checked-in procedural Blender script; no
third-party, commissioned, generated, or hand-edited model data is involved.

## Files

- `build_kit.py` — the theme-2 palette and piece geometry. The kit contract shared with every theme
  kit (parts, bevel, palette bake, material, export, validation, review sheet) lives in
  `../kit_common.py`.
- `kit.blend` — the generated source master: the `EXPORT__tidal_garden` collection with one joined
  `mesh_<piece>` object per piece.
- `previews/kit-sheet.png` — generated review sheet. It re-imports every exported GLB (a round-trip
  check) and renders it with vertex colours from a 55-degree gameplay-like elevation. Workbench
  under-predicts in-game brightness; judge palette in native captures against the sand floor.
- Runtime derivatives (not in this directory):
  `assets/brawler/models/original/environment/tidal-garden/<piece>.glb`.

## Rebuild

From the repository root:

```sh
just tidal-kit
# equivalent to
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  -P asset_src/environment/tidal-garden/build_kit.py
```

Built and verified with Blender 5.2.3 LTS. Pass `-- --no-renders` after the script to skip the
review sheet. The run prints a `KIT_BUILD_REPORT` JSON line (bytes, triangles, bounds per piece)
and fails if any export breaks the kit contract. Two consecutive builds produce byte-identical GLBs
(`kit.blend` and the sheet PNG carry timestamps and are not byte-stable). Never hand-edit a GLB:
change the builder and rebuild, then update the matching `theme_kits` entry in
`assets/catalogs/map_asset_visuals.ron` if a bound changed.

## Kit contract

The same contract as every theme kit (see `../kit_common.py` and the Crossroads Facility README):
1 Blender unit = 1 map cell, `+Z` up, ground-centred origin, front faces `-Y`; one shared chamfer
(0.045 cell); one material with a flat per-face palette in `COLOR_0` and no normals, UVs, or
textures; top-face highlight and height AO baked into the palette; at most 2,500 triangles per
piece; Exact pieces fill exactly one cell.

## Readability on the sand floor

- Walls share a dark stone plinth and a sea-stone cap rim; the cap inlay and the faces show the
  material. All are darker and cooler (brick: much darker) than the sand.
- Destructible cover is lower and painted: a turquoise beach-hut crate (the complement of the sand)
  and a sage cactus in a terracotta pot.
- Concealment is deep sea green with upright fronds and sways in game.
- The breakable barrier is a two-cell red and white lifeguard barricade.

## Pieces

| GLB | Visual profile | Use | Fitting | Triangles |
|---|---|---|---|---|
| `sea-stone-wall.glb` | 32 (garden wall) | blue-grey stone courses, seaweed band | Exact | 496 |
| `coral-brick-wall.glb` | 45 (red brick wall) | coral-red brick courses | Exact | 584 |
| `tide-gate-wall.glb` | 46 (metal wall) | iron panels, verdigris straps | Exact | 364 |
| `driftwood-wall.glb` | 47 (wood wall) | driftwood palisade, rope bands | Exact | 452 |
| `beach-crate.glb` | 48 (striped cover) | turquoise and white striped crate | Contained 0.91 | 1,584 |
| `cactus.glb` | 43 (cactus) | sage cactus, pink flowers, terracotta pot | Contained 0.84 | 1,024 |
| `seagrass-bush.glb` | 33 (tall grass) | chunky seagrass bush; sways in game | Contained 0.96 | 780 |
| `lifeguard-fence.glb` | 34 (breakable barrier, 2x1) | red and white barricade, driftwood posts | Contained 0.95 | 748 |
| `scallop-shell.glb` | 36 (seashell) | cream and pink scallop | Contained 0.62 | 352 |
| `sea-vault.glb` | 39 (Heist safe) | iron sea vault, brass porthole dial | Contained 0.96 | 916 |
| `pickup.glb` | 42 (restoration pickup) | green canister, white cross, rope ring | Contained 0.55 | 544 |

Water, rubble, effect tiles, and the arena border stay generated from the theme palette; the
water curb uses the theme's `water_edge` colour.

## Provenance

Original work of the PewPew Blitz project, so it is not a row in the third-party
`assets/manifest.ron`. The source of truth is this directory and `../kit_common.py`: the builder
reproduces the runtime GLBs with the command above.
`assets/licenses/pewpew-blitz-original-environment.txt` points here.
