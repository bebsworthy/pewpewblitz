Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec refreshed with code refs — selection still bumps a full editor rebuild.

# Outcome
Selecting or cycling to another asset on the same palette page updates the selected-card border and badge in place; the six palette preview viewports are not recreated.

# Context
- BRL-cwm9 (f08518d8) retained the grid/symmetry/search/status labels (`MapMakerValueLabel`, `packages/brawler-client/src/flow/screens/map_maker.rs:178-185`) and made rotation, search toggling and non-filtering search keystrokes rebuild-free.
- `MapMakerSelectAsset` (map_maker.rs:569-577) and `MapMakerCycleAsset` -> `cycle_palette` (:566-568, :1022-1041) still call `bump_render` (:952), which bumps `render_revision`; `present_map_editor` respawns the `MapMakerRoot(render_revision)` tree (:2033-2053).
- `palette_item_button` (:3246-3313) spawns the ACCENT border node and `palette_selection_badge` (:3315) only when `selected`, so the selection is structural today. `SurfaceState.selected` on the card is already kept live by `update_flow_button_chrome` via `MapMakerRuntime::action_selected` (`flow/screens/shared.rs:217`, map_maker.rs:958).
- The visible window is page-aligned: `palette_window` (:3091-3094), `VISIBLE_PALETTE_ITEMS = 6` (:36). A selection change within the same page leaves the window unchanged; ±1 cycling across a page edge or a page jump (`PALETTE_PAGE_DELTA`) changes it.
- Each rebuild with the drawer open respawns about 97 UI nodes, including 6 `MapPalettePreviewHost` nodes (:212). `presentation_3d/map_palette_preview.rs::spawn_previews` (:42-125) gives every host without a `ViewportNode` a fresh render-target image, `Camera3d`, directional light and visual scene. Clicking a card or wheeling over the palette therefore recreates 6 offscreen cameras.

# Scope
- Spawn the border/badge on every card with a marker (e.g. `PaletteSelectionMark(index)`) and toggle `Visibility` from `runtime.palette_index` in a retained-update system (alongside `update_map_editor_values`), or drive it from `SurfaceState.selected`.
- In `MapMakerSelectAsset` / `cycle_palette`, call `bump_render` only when the visible page window (start of `palette_window`), category, or filtered entries change; otherwise value-only.
- Optional: keep preview viewports across page changes by reusing hosts per slot.

# Plan
1. Always spawn border + badge, `Visibility::Hidden` unless selected.
2. Retained system updates visibility when `MapMakerRuntime` changes.
3. Compute old/new window start in select/cycle; bump only on change.

# Acceptance criteria
- ECS test next to `value_only_edits_keep_the_same_editor_ui_nodes` (map_maker.rs:3910): selecting another card on the same page keeps the same Node entities and `MapPalettePreviewHost` entities, and the new card's mark is visible / old hidden; crossing a page edge still rebuilds.
- Visible behavior unchanged; native check of palette click and wheel.
- `just lint`, `just test`.

# Out of scope
Map Maker feature work (BRL-6vjz, BRL-tztp).


# Result (2026-11-18)
- Every palette card now spawns its ACCENT border and check badge, tagged `PaletteSelectionMark(palette_index)`, as `Visibility::Hidden` unless selected. `update_map_editor_values` toggles their visibility from `MapMakerRuntime::palette_index`, alongside the BRL-cwm9 retained labels.
- `PaletteLayout` now keys on the page-window start (`page_start`) instead of the selected index. `MapMakerSelectAsset` and `cycle_palette` compare the layout before and after the change and call `bump_render` only when it differs. Crossing a page edge, changing category or changing the filter still rebuilds; search-driven reselection within the same page no longer does.
- `palette_item_button` builds its own `FlowButton` index and action from the palette index, which dropped the `too_many_arguments` allow.
- New shell-capture route `map-editor-palette` (`automation.rs`, `scripts/shell-capture.sh`): the `map-editor` route plus one canonical `MapMakerCycleAsset(1)`.

## Verification
- ECS test `flow::screens::map_maker::tests::same_page_palette_selection_keeps_the_editor_ui_nodes`, next to `value_only_edits_keep_the_same_editor_ui_nodes`. A card click and a -1 cycle on the same page keep every `Node` entity and the 6 `MapPalettePreviewHost` entities and leave `render_revision` unchanged. Only the newly selected index's border and badge are visible. Cycling +1 from the page's last card rebuilds: the node and preview-host sets are disjoint, and the next page's first card is marked.
- `just lint`: pass (`target/kgba/lint.log`). `just test`: pass, exit 0, 2557 tests (`target/kgba/test.log`).
- Native, 1280x720: `target/kgba/capture-default/1280x720/map-editor.png` shows the default Arena Wall card with its border and badge. `target/kgba/capture/1280x720/map-editor-palette.png` shows Ashen Pine with the border and badge, Arena Wall without them, the pointer preview updated, and all six previews rendered.
- Not verified: hand-driven mouse click and wheel in an interactive client, the 914x411 size, and native evidence that viewports are not recreated (only the ECS test covers that).
