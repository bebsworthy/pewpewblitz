Reviewed 2026-11-15 against HEAD 3437c0d8: valid as written — dead system still runs; owner decision still needed.

# Outcome
No dead retained-UI code remains for the Map Maker cursor readout. If the readout is wanted, it is visible again.

# Context
- Found during BRL-cwm9. `MapMakerCursorCopy` (`packages/brawler-client/src/flow/screens/map_maker.rs:176`) and `update_map_editor_cursor_copy` (:352-372) are still registered (`packages/brawler-client/src/flow.rs:213`, and ordered before `update_map_editor_values` at flow.rs:219). The system has no `run_if(in_state(MapEditor))`; it early-outs unless `MapMakerRuntime` changed. No code spawns a `MapMakerCursorCopy` entity since b2907162 ("Match Map Maker to approved mockups" removed the spawn), so the "90° · SYMMETRY · CELL x,y" readout never shows.
- Symmetry is already shown by the retained `MapMakerValueLabel::Symmetry` label (map_maker.rs:2141). Placement rotation (R in Place mode) is visible only in the 3D pointer preview (map_maker.rs ~596-599); BRL-cwm9 therefore made it a rebuild-free value-only edit.
- `rotation_degrees` (:3532) is used only by the dead system and a test (:3609); `symmetry_label` (:3506) is also used by the symmetry label.

# Decision needed
Should the editor show a rotation/cell readout? (owner) — ask via `ticket task BRL-yaa4 question add` before implementing. Default recommendation: remove (approved mockups dropped it; symmetry is already shown).

# Plan
- Remove: delete `MapMakerCursorCopy`, `update_map_editor_cursor_copy`, its `flow.rs` import (:39) and scheduling (:213, :219 `.after`), and `rotation_degrees` + its test if nothing else uses it.
- Restore: spawn a `MapMakerCursorCopy` text in the editor chrome (outside the rebuild tree or respawned with it), gate the system with `run_if(in_state(ClientFlow::MapEditor))`, and add an ECS test that R and cursor moves update the text without bumping `render_revision`.

# Acceptance criteria
- Either the readout is spawned in the editor chrome and updated in place, or the component and system are removed.
- `just lint`, `just test`.

# Decision (2026-11-15)
Go with the default: **remove** the dead readout system and `rotation_degrees` (the approved mockups dropped it, and symmetry is already shown). The owner can reopen the ticket to restore it instead.


# Result (2026-11-18)

I removed the readout, following the recorded default decision:
- Deleted `MapMakerCursorCopy`, `update_map_editor_cursor_copy`, `rotation_degrees` and its overflow test from `flow/screens/map_maker.rs`.
- Removed the system's import, its scheduling and its `.after` edge from `flow.rs`.
- `symmetry_label` stays because the retained Symmetry label still uses it.
- No spawn site existed, so nothing visible changes.

Verification: `just lint` passes, and `cargo test -p brawler-client --lib` passes (828). Logs are in `target/yaa4/`.

If the owner wants the readout back, reopen this ticket and use the "Restore" plan above.
