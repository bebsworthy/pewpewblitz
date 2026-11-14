# Outcome

Discrete Map Maker edits that only change a displayed value (grid/symmetry labels, rotation, palette search text, status line) update retained UI nodes instead of respawning the whole editor UI.

# Context

- BRL-qs1f removed the per-frame rebuild during camera pan/zoom and made the selection toolbar a retained, camera-following node.
- `MapMakerRuntime::bump_render` still has ~49 one-shot callers in `src/client/flow/screens/map_maker.rs`; each discrete action (click, key, palette wheel step, search keystroke) respawns every editor UI root once via `present_map_editor`.
- These are per action, not per frame, so there is no demonstrated performance problem yet.

# Scope (only when evidence justifies it)

- Measure whether one-shot rebuilds cause visible hitches, lost hover/press state, or input latency (for example while typing a palette search).
- If they do, convert the affected value-only controls to retained nodes updated by focused systems, keeping structural changes (tool switch, drawer open/close, validation popover) on the rebuild path.

# Out of scope

Map Maker feature work (BRL-6vjz, BRL-tztp).

# Acceptance criteria

- Evidence recorded before implementing (frame timing or a reproducible UX defect).
- Converted edits produce zero editor UI root respawns (ECS test) with unchanged visible behavior.

# Verification

- ECS test, `just lint`, `just test`, native Map Maker check.

# Evidence (2026-11-14, recorded before converting)

- A reproducible cost, measured with the new ECS test plus a temporary `bump_render` restored on grid toggle. With the Place drawer open, every one-shot rebuild despawns and respawns 97 editor UI nodes. Six of them are `MapPalettePreviewHost` cards. `presentation_3d/map_palette_preview.rs::spawn_previews` gives each new host a fresh render-target `Image`, an offscreen `Camera3d`, a `DirectionalLight` and a visual scene. A grid toggle, a symmetry cycle, an R rotation, or one palette-search keystroke therefore recreated six offscreen cameras and textures and blanked the previews until the next render. It also reset hover/press state on every editor button.
- Frame timing was not measured natively: this session cannot drive the macOS window.

# Decision (2026-11-14)

Per-action classification at the `bump_render` call sites. No generic diff or retained-UI framework.

- Value-only, now rebuild-free:
  - `MapMakerToggleGrid` (label `GRID ON/OFF`).
  - `MapMakerCycleSymmetry` (label `SYMMETRY …`; symmetry is session state, not document state).
  - `MapMakerRotate` without a selection (`quarter_turns` is shown only by the 3D pointer preview).
  - `MapMakerToggleSearch` (search label).
  - Search keystrokes, Backspace, Enter/Esc while searching, when the palette layout is unchanged. "Layout" means the visible entry indices, the selected index, and whether the query is empty (the clear-search button). These are compared before and after the edit; anything else still rebuilds.
  - Status-only messages: the first CLEAR press (arming), the unsaved-changes warning on MAPS, TEST failures (mode missing, validation error, save error), and place/paint with no visible asset.
- Still structural, full rebuild: tool switch, drawer open/close, validation popover, category, palette selection and paging, clear search, all document edits (place/paint/erase/move/duplicate/delete/undo/redo/clear/save/test success), selection changes, selection rotate, library actions, Practice return. These are unchanged.
- Retained nodes:
  - Private `MapMakerValueLabel { Grid, Symmetry, Search, Status }` marks the label `Text` entities.
  - The status line (`MapMakerStatusLine`) is now always spawned and toggles `Display::None`, so a status appearing or disappearing is value-only.
  - The new `update_map_editor_values` (PresentFlow, MapEditor only, after `present_map_editor`, `update_map_editor_cursor_copy` and `position_map_editor_selection_toolbar`) runs only when `MapMakerRuntime` changed. It writes only labels that differ.
  - The spawn path and the update system share the `grid_label`, `symmetry_button_label`, `search_label` and `status_display` helpers, so the text is identical.
  - Selected/armed chrome was already applied every frame by `update_flow_button_chrome` through `MapMakerRuntime::action_selected`, so it needed no change.

# Implementation (2026-11-14)

- `packages/brawler-client/src/flow/screens/map_maker.rs`: the value-only `bump_render` calls are removed as listed above; the armed-CLEAR and selection-rotate branches keep theirs. Also adds the `PaletteLayout`/`palette_layout` comparison for search keystrokes, the markers, the always-spawned status line, the label helpers, `update_map_editor_values`, and a `label_marker` parameter on `compact_icon_action_button_disabled`.
- `packages/brawler-client/src/flow.rs`: registers `update_map_editor_values` in the existing nested map-editor PresentFlow tuple.
- New ECS test `value_only_edits_keep_the_same_editor_ui_nodes` (helpers `value_edit_test_app`, `press_editor_key`). It runs real `map_maker_input → resolve_map_maker_action → present_map_editor → update_map_editor_values` with the drawer open (6 preview cards). The sequence: search toggle, type a space, Backspace, Enter, G, M, R, C. After each step the set of `Node` entities is identical and the expected label text or status visibility is present. A final V (tool switch) proves the structural path still rebuilds: the node set is disjoint from the initial one.

# Verification (2026-11-14)

All runs used a scratch `CARGO_TARGET_DIR`, with VS Code's bundled ripgrep first on PATH.

- `cargo test --locked --lib map_maker` (brawler-client): 19 passed, including the new test and `five_seconds_of_pan_and_zoom_never_respawn_the_editor_ui`.
- Negative check: temporarily restoring the grid-toggle `bump_render` fails the new test with "grid toggle respawned". Reverted.
- `just check`: pass.
- `just lint`: pass. A first run failed on `too_many_lines` in the new test; fixed by extracting `value_edit_test_app` rather than adding an allow.
- `just test`: pass. 24 suites, 2345 tests, 0 failures, including `client_flow_update_schedule_has_no_owned_ambiguities`.
- Native check: not run. This session cannot drive the macOS client.

# Playtest request

This change is client-only; the server needs no restart. Run `just client` against the running server (or `just run`), open Map Maker, and open a draft.

1. Place tool with the ASSETS drawer open. Press G and M several times, and click GRID and SYMMETRY. Labels change immediately. Palette previews must not blink or blank. Buttons keep their hover highlight. The grid overlay toggles.
2. Press R a few times: the 3D pointer preview rotates and the palette does not flicker.
3. Press `/` and type in the search. A keystroke that changes results still refreshes the cards. Typing a trailing space, or Backspace on it, changes only the text. Enter/Esc exits search.
4. Press C once: the "Press CLEAR again…" status appears in the same place as before (above the drawer). Press C again: the map clears and the status changes.
5. Make an edit, then click MAPS: the unsaved-changes status appears. Press T on an invalid map: the validation status appears.
6. Switch tools (P/V/E) and open/close the drawer: the layout behaves as before.

Report any visual difference from before.

# Learn from errors

- New ECS tests that build a full App inline exceed clippy's `too_many_lines` quickly. Extract the App setup into a named test helper up front instead of finding it at lint time.
- The tempting fix for "palette search text" was a blanket "typing is value-only" rule. That would be wrong: a keystroke usually changes the filtered cards and the selected index. Classify value-only edits by comparing exactly what the spawn path reads (`PaletteLayout`), not by action name.
- Removing a rebuild trigger also removes the implicit "node appears/disappears" behavior. The status line had to become always-spawned (`Display::None`) before status-only edits could skip the rebuild. Check for conditional spawns whenever a value moves to the retained path.
- Found while auditing: `MapMakerCursorCopy` is never spawned, so the cursor/rotation readout is dead. Filed as a separate ticket rather than fixed here.

# Deferred

- BRL-kgba (backlog, parent BRL-q5c4, related BRL-cwm9): keep palette cards and preview viewports when only the selected asset changes.
- BRL-yaa4 (backlog, parent BRL-q5c4, related BRL-cwm9): remove or restore the unspawned cursor/rotation readout (owner decision).
