# Outcome

Each theme's tile floor gets a small number of decals near walls that add place character while
the floor stays quiet behind fighters, effect tiles, zones, and concealment.

# Context

BRL-0147 replaced the flat floor and organic blobs with a grid-aligned two-tone tile surface
(`src/client/presentation_3d/floor_tiles.rs`, per-theme `floor` in
`assets/catalogs/map_presentation_themes.ron`). Its spec listed "optional sparse decals near walls";
they were deferred because dressing should match the per-theme environment kit (BRL-0153).

# Scope

- Per-theme decal set and density in the client theme catalog (no hard-coded colors).
- Deterministic placement on cells adjacent to static walls, never on surfaces, effect tiles,
  objective areas, or spawn cells.
- Presentation-only; no gameplay content or fingerprint change.

# Acceptance criteria

- Decals are deterministic per map and never overlap effect tiles, water, Hot Zone, or safe areas.
- A readability review passes on every theme.

# Verification

- Focused placement test; native captures per theme; `just test`.
