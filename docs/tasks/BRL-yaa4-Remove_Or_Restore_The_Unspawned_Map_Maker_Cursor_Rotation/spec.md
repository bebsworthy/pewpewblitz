# Outcome

No dead retained-UI code remains for the Map Maker cursor readout. If the readout is wanted, it is visible again.

# Context

- Found during BRL-cwm9. `MapMakerCursorCopy` and `update_map_editor_cursor_copy` (`packages/brawler-client/src/flow/screens/map_maker.rs`) still run every Map Editor frame. No code spawns a `MapMakerCursorCopy` entity any more, so the "90° · SYMMETRY · CELL x,y" readout never shows.
- Placement rotation (R in Place mode) is visible only in the 3D pointer preview. BRL-cwm9 therefore made it a rebuild-free value-only edit.

# Decision needed

Should the editor show a rotation/cell readout? (owner)

# Acceptance criteria

- Either the readout is spawned in the editor chrome and updated in place, or the component and system are removed.
- `just lint`, `just test`.
