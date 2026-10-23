# Outcome

The floor gives the arena structure and scale cues while staying quiet behind fighters, hazards, and objectives.

# Context

`assets/catalogs/map_presentation_themes.ron` gives each theme a flat ground color plus organic blobs. The floor fills most of each frame (about 70% of the August capture; slate blue in the 2026-10-23 capture).

# Scope

- A ground material/shader with two close tones alternating per authoritative grid cell, subtle per-cell value jitter, and darkening near walls/edges.
- Per-theme parameters in the presentation theme catalog.
- Optional sparse decals near walls.
- Effect tiles, Hot Zone regions, concealment fields, and objective markers must stay readable on top.

# Out of scope

Environment model kits (separate ticket) and gameplay or map recipe changes.

# Acceptance criteria

- Tiles align exactly to authoritative cells on every built-in map.
- A readability review passes for effect tiles, zones, and concealment boundaries on every theme.
- The floor is configured per theme in the catalog, with no hard-coded colors.

# Verification

- A focused alignment test, if feasible.
- Native captures per map.
- `just test`.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
