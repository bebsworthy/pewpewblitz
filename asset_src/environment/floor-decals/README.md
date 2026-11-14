# Floor decals

The sparse wall-adjacent floor decal atlas (BRL-0178): cracks, scuffs, leaves, sand drifts, stains,
and pebbles that make the tile floor read as a lived-in place. The atlas is original project work
generated entirely by the checked-in script; no third-party, commissioned, AI-generated, or
hand-edited image data is involved.

## Files

- `build_decals.py` — the sprite definitions (signed-distance strokes, ellipses, seeded value
  noise) and the PNG writer (Python 3 standard library only).
- `previews/decal-sheet.png` — generated review sheet: every sprite tinted per theme over that
  theme's floor tone. Its tints are illustrative; the theme catalog owns the shipped values.
- Runtime derivative (not in this directory):
  `assets/brawler/textures/original/environment/floor-decals.png`.

## Rebuild

From the repository root:

```sh
python3 asset_src/environment/floor-decals/build_decals.py             # atlas + review sheet
python3 asset_src/environment/floor-decals/build_decals.py --no-sheet  # atlas only
python3 asset_src/environment/floor-decals/build_decals.py --list      # inventory
```

Built with Python 3.15. Rendering is deterministic (seeded xorshift, no platform randomness), so
two builds write byte-identical PNGs. Never hand-edit the PNG: change the script and rebuild.

## Atlas contract

Mirrored by `packages/brawler-client/src/presentation_3d/floor_decals.rs`:

- 384 x 256 RGBA, 3 columns x 2 rows of 128 px cells, row-major in the order crack, scuff, leaves,
  sand-drift, stain, pebbles (`FloorDecalSprite`).
- The top edge (v = 0) of each sprite is the wall side; the client turns each decal so that edge
  faces the adjacent wall or arena edge.
- Colour is near-white (0.70–1.00) and alpha carries the shape. Each theme's tint and opacity in
  `assets/catalogs/map_presentation_themes.ron` (`floor_decals`) set the hue, so one atlas serves
  every theme. Fully transparent texels are white and every sprite keeps a transparent border, so
  linear filtering never darkens edges or bleeds between cells.
- Shapes are soft and low-frequency because the texture has no mipmaps: a cell spans roughly
  60–140 physical pixels on desktop and phone screens, so a 128 px sprite is never minified
  far enough to shimmer.

## Inventory

| Index | Sprite | Used by |
|---|---|---|
| 0 | crack — branching fissure starting at the wall | themes 1, 2, 3 |
| 1 | scuff — curved skid marks | themes 1, 3 |
| 2 | leaves — a few fallen leaves gathered at the wall | themes 2, 3 |
| 3 | sand-drift — a rippled mound banked against the wall | theme 2 |
| 4 | stain — a dried puddle or oil ring | theme 1 |
| 5 | pebbles — a few small stones with contact shadows | themes 2, 3 |

Provenance: `assets/licenses/pewpew-blitz-original-decals.txt`.
