# Outcome

Discrete Map Maker edits that only change a displayed value (grid/symmetry labels, rotation, palette search text, status line) update retained UI nodes instead of respawning the whole editor UI.

# Context

- BRL-0163 removed the per-frame rebuild during camera pan/zoom and made the selection toolbar a retained, camera-following node.
- `MapMakerRuntime::bump_render` still has ~49 one-shot callers in `src/client/flow/screens/map_maker.rs`; each discrete action (click, key, palette wheel step, search keystroke) respawns every editor UI root once via `present_map_editor`.
- These are per action, not per frame, so there is no demonstrated performance problem yet.

# Scope (only when evidence justifies it)

- Measure whether one-shot rebuilds cause visible hitches, lost hover/press state, or input latency (for example while typing a palette search).
- If they do, convert the affected value-only controls to retained nodes updated by focused systems, keeping structural changes (tool switch, drawer open/close, validation popover) on the rebuild path.

# Out of scope

Map Maker feature work (BRL-0037, BRL-0139).

# Acceptance criteria

- Evidence recorded before implementing (frame timing or a reproducible UX defect).
- Converted edits produce zero editor UI root respawns (ECS test) with unchanged visible behavior.

# Verification

- ECS test, `just lint`, `just test`, native Map Maker check.
