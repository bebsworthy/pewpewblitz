# PewPew Blitz shell icon set

BRL-0157, 2026-11-13. One original, cohesive icon family for the player shell: Dashboard
navigation, brawler details and creation, confirmations, ultimate/passive badges, weapon part
illustrations and the Map Maker. No third-party, purchased, AI-generated or hand-edited pixels
are used; every icon is code-authored geometry.

Build (Python 3 standard library only, no Pillow/NumPy/Sharp), from the repository root:

    python3 asset_src/player-shell/icons/build_icons.py            # whole set
    python3 asset_src/player-shell/icons/build_icons.py fire scan  # selected icons

The script owns the set. For each icon it writes the editable SVG source to `svg/<name>.svg`
(explicit viewBox, real paths, masks for the tone bands, no filters or external links) and
rasterizes the runtime PNG to `assets/brawler/ui/icons/<name>.png` with an exact signed-distance
renderer from the same geometry. Output is deterministic; a full build takes a few seconds. A full
build also removes files in those two directories that are no longer in the set.

## Style contract

- Every part: navy `ink` (#0E1638) outline (4.5 units), darker bottom-lip shade band, base tone,
  light top highlight band — outline, two tones and a highlight. The silhouette sits on a
  3.5-unit ink offset lip, echoing the shell's pressable surfaces.
- Tones follow the BRL-0140 palette (`play` orange/yellow, `accent` cyan, `panel-raised` indigo)
  plus semantic accents: coral (danger/pulse), green (restore/test), lime (poison), violet
  (concealment), ice (cold), steel (secondary pieces).
- Canvases: square icons 100 units → 128 px; ultimate/passive badges 120×100 units → 240×200 px
  on a shared indigo hexagon emblem; weapon part illustrations 120×100 units → 288×240 px.
- Icons are full color. Load sites use a white (no-op) `ImageNode` tint; do not tint them.

## Inventory

Navigation/actions: chevron-left/right/up/down, lightning (Play), crosshair (Practice), menu,
player, arena, skull (View Brawlers), edit, trash, empty (unfilled slot).
Map Maker: close, check (saved/ready), warning, play (test map), place, select, erase, grid,
symmetry, fit, search, rotate, duplicate.
Ultimate/passive families: dash, sentry, cloak, scan, field, blast, cold, fire, poison, heal,
blob, feather, shield, pulse, target, cycle.
Weapon part families: part-magazine, part-ammunition, part-barrel, part-chamber, part-element.

`packages/brawler-client/src/ui/icons.rs` lists the shipped names and its tests check each PNG, size and SVG
source. Add an icon by defining it in `ICONS` in the script, rebuilding, and adding its name to
`icons.rs`.

Retired by this set: the Pillow `details/build_art.py` pictograms, the Sharp-exported shell
`icon-*` and `part-*` artwork, and six unused Kenney Game Icons. Mode pennant badges, the logo
and panel chrome are separate artwork (see `../README.md`).
