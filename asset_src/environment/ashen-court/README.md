# Ashen Court environment kit

The environment kit for map presentation theme 3 (Ashen Court; BRL-0188). Every built-in theme-3
map (Ashen Court, Switchback Basin, Verdant Crossfire) draws its imported environment pieces only
from this kit. The kit is original project work built entirely by the checked-in procedural Blender
script; no third-party, commissioned, generated, or hand-edited model data is involved.

## Files

- `build_kit.py` — the theme-3 palette and piece geometry. The kit contract shared with every theme
  kit (parts, bevel, palette bake, material, export, validation, review sheet) lives in
  `../kit_common.py`.
- `kit.blend` — the generated source master: the `EXPORT__ashen_court` collection with one joined
  `mesh_<piece>` object per piece.
- `previews/kit-sheet.png` — generated review sheet. It re-imports every exported GLB (a round-trip
  check) and renders it with vertex colours from a 55-degree gameplay-like elevation. Workbench
  under-predicts in-game brightness and the cool theme light pushes greys towards blue; judge
  palette in native captures against the grey stone floor.
- Runtime derivatives (not in this directory):
  `assets/brawler/models/original/environment/ashen-court/<piece>.glb`.

## Rebuild

From the repository root:

```sh
just ashen-kit
# equivalent to
/Applications/Blender.app/Contents/MacOS/Blender -b --factory-startup \
  -P asset_src/environment/ashen-court/build_kit.py
```

Built and verified with Blender 5.2.3 LTS. Pass `-- --no-renders` after the script to skip the
review sheet. The run prints a `KIT_BUILD_REPORT` JSON line (bytes, triangles, bounds per piece)
and fails if any export breaks the kit contract. Two consecutive builds produce byte-identical GLBs
(`kit.blend` and the sheet PNG carry timestamps and are not byte-stable). Never hand-edit a GLB:
change the builder and rebuild, then update the matching `theme_kits` entry (and, for a piece that
is also a default, the default entry) in `assets/catalogs/map_asset_visuals.ron` if a bound changed.

## Kit contract

The same contract as every theme kit (see `../kit_common.py`): 1 Blender unit = 1 map cell, `+Z`
up, ground-centred origin, front faces `-Y`; one shared chamfer (0.045 cell); one material with a
flat per-face palette in `COLOR_0` and no normals, UVs, or textures; top-face highlight and height
AO baked into the palette; at most 2,500 triangles per piece; Exact pieces fill exactly one cell.
The 2x2 pine and boulders are modelled in a two-cell span and fitted with `Contained`.

## Readability on the grey stone floor

- Walls share a dark basalt plinth and a slate cap rim; the cap inlay and the faces show the
  material (ashlar courses, rune stone with a lichen roundel and moss band, soot brick with a dark
  charcoal-umber top). All are clearly darker than the floor.
- Destructible cover is lower and always warm amber oak in a dark iron frame (X-braced, painted
  bands, or copper straps), so "breakable" reads the same on every theme-3 map.
- Concealment is a moss-green fern bush with arching fronds and sways in game.
- The 2x2 blockers (pine, boulders) stay near their circular colliders.
- No wall, cover, or wall top uses the effect-tile hues (speed cyan, slow violet, damage red).

## Pieces

| GLB | Visual profile | Use | Fitting | Triangles |
|---|---|---|---|---|
| `ashlar-wall.glb` | 16 (Ashen stone wall) | basalt ashlar courses | Exact | 452 |
| `rune-wall.glb` | 44 (green symbol wall) | rune stone, lichen roundel, moss band | Exact | 672 |
| `soot-brick-wall.glb` | 45 (red brick wall) | soot brick courses, charcoal-umber top | Exact | 596 |
| `reliquary-crate.glb` | 24 (destructible cover) | amber oak crate, iron X-braces | Contained 0.91 | 1,100 |
| `banded-crate.glb` | 48 (yellow striped cover) | amber oak crate, ochre and cream bands | Contained 0.91 | 836 |
| `strapped-crate.glb` | 49 (green striped cover) | amber oak crate, copper straps | Contained 0.91 | 968 |
| `fern-bush.glb` | 33 (tall grass) | chunky fern bush; sways in game | Contained 0.96 | 780 |
| `ash-pine.glb` | 19 (Ashen pine, 2x2) | tiered teal pine, ash-pale rims, stone collar | Contained 0.92 | 688 |
| `boulders.glb` | 20 (Ashen rock, 2x2) | blue-grey boulder cluster, moss caps | Contained 0.94 | 560 |
| `headstone.glb` | 21 (gravestone) | lichen headstone on a moss mound | Contained 0.62 | 428 |
| `coffin.glb` | 22 (coffin) | dark oak coffin, brass cross | Contained 0.72 | 264 |
| `lantern.glb` | 23 (lantern) | iron post lantern, amber glass | Contained 0.66 | 484 |
| `pickup.glb` | 42 (restoration pickup) | green canister, white cross, iron ring | Contained 0.55 | 544 |

Ground, water, rubble, effect tiles, and the arena border stay generated from the theme palette.

## Provenance

Original work of the PewPew Blitz project, so it is not a row in the third-party
`assets/manifest.ron`. The source of truth is this directory and `../kit_common.py`: the builder
reproduces the runtime GLBs with the command above.
`assets/licenses/pewpew-blitz-original-environment.txt` points here.
