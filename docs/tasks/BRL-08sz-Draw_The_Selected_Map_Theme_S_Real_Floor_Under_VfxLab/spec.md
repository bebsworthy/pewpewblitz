# Outcome

Effects in VfxLab are judged against the real floor of the selected map theme. Today VfxLab borrows the theme's lighting and grading (BRL-1cnh) but draws a fixed dark blue-grey slab, sRGB (0.075, 0.085, 0.105) (`packages/brawler-client/src/vfx_lab.rs:822-831`). The production theme 1 floor is warm brown, tones (0.52, 0.38, 0.24) / (0.54, 0.40, 0.25) (`assets/catalogs/map_presentation_themes.ron`). VfxLab captures therefore misjudge surface contrast, which was the original complaint behind this ticket.

# Descoped (owner decision, 2026-11-20)

The earlier spec (a full production-map viewer with a map selector, map loading, placement and camera controls, and fighter, cover, and objective fixtures) is dropped. Most of its goal (real maps, real scale, real clutter, the production path) is already met by Practice matches captured with `just v3-render-evidence`, which is the actual game rather than a copy of it. The full viewer belongs to BRL-pzkk (the VFX Workbench idea) if it is ever needed.

# Scope

- Replace the neutral slab with the selected theme's floor, built through the production floor path: `floor_tiles::floor_tile_mesh` with the theme's `FloorTiles` and `floor` material, plus the theme's `outer_ground` around it. It uses the same `PLAYABLE_FLOOR_Y` height and the same coordinate conventions (`presentation_3d/map.rs::spawn_ground_surfaces`).
- Size the lab floor as a small synthetic open dimension covering the lab camera footprint, with no occluder cells. The floor decals, walls, perimeter, water, and props are out of scope.
- Expose only what VfxLab needs, through one narrow `pub(crate)` helper in `presentation_3d` (for example "spawn a theme floor for these dimensions"). Do not widen the `floor_tiles` internals, and do not build a second floor implementation.
- Keep `--theme N` as the selector. Its default stays 1.
- Add `--floor neutral` to keep the old slab as a diagnostic option. The default becomes the themed floor.
- Show the floor mode in the inspector panel ("Theme N floor + lighting" or "neutral floor") and in the evidence snapshot JSON.

# Boundaries

- VfxLab stays read-only and client-only. There is no map loading, gameplay, networking, map selector, or placement tooling.
- The server and headless feature graphs are unchanged (`scripts/check-client-features.sh`).

# Acceptance criteria

1. With the default flags, VfxLab draws the theme 1 production floor tiles and outer ground under the effect, matching a Practice capture of the same theme in tone.
2. `--theme 2` and `--theme 3` switch both the floor and the lighting.
3. `--floor neutral` restores the old slab.
4. The inspector and the evidence snapshot report the floor mode and theme.
5. The normal, reduced-effects, and primitive modes still work.
6. No new floor or material code is duplicated from `presentation_3d`.

# Verification

- `just lint`, plus a focused test that the lab floor uses the theme's `FloorTiles` and material handles, and that `--floor neutral` keeps the slab.
- Off-screen captures (`target/debug/brawler-vfx-lab --offscreen --capture-dir …`) of the `field` and `cone` fixtures on themes 1, 2, and 3, plus `--floor neutral`. Attach them as a contact sheet.
- One paired comparison with a Practice render-evidence frame on the same theme, with notes on any remaining contrast difference.

# Follow-ups

BRL-nhs7 (enemy areas and cones) should use the themed floor to judge its fix. The block is no longer formal: BRL-nhs7 can proceed without this ticket.
