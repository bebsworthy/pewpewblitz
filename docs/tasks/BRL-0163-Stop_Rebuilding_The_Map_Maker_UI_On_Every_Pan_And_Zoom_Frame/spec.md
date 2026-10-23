# Outcome

Panning and zooming in the Map Maker is cheap and smooth.

# Context

- Middle-drag pan calls `runtime.bump_render()` on every frame with mouse motion (`src/client/flow/screens/map_maker.rs:1428-1430`). Wheel zoom does the same (`:1444`).
- `present_map_editor` despawns and respawns all editor UI roots whenever `render_revision` changes (`:2006-2014`).
- `bump_render` has about 51 call sites.

# Scope

- Camera pan and zoom update the editor camera only.
- Audit the other `bump_render` callers for per-frame triggers.
- Retain UI nodes where state only changes values.

# Out of scope

Map Maker feature work (BRL-0037, BRL-0139).

# Acceptance criteria

- Five seconds of continuous panning or zooming produces zero editor UI root respawns (ECS test).
- Visible behavior is unchanged.

# Verification

- The ECS test.
- `just test`.
- Native check.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
