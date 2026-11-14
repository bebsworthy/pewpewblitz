# Outcome

Selecting or cycling to another asset on the same palette page updates the selected-card border and badge in place; the six palette preview viewports are not recreated.

# Context

- BRL-cwm9 retained the grid/symmetry/search/status labels and made rotation, search toggling and non-filtering search keystrokes rebuild-free.
- `MapMakerSelectAsset` and `MapMakerCycleAsset` still call `bump_render`. `palette_item_button` spawns the selected border and badge only on the selected card, so the selection is structural today.
- Each rebuild with the drawer open respawns about 97 UI nodes, including 6 `MapPalettePreviewHost` nodes. `presentation_3d/map_palette_preview.rs` gives each new host a fresh render-target image, `Camera3d`, light and visual scene. Clicking a card or wheeling over the palette therefore recreates 6 offscreen cameras.

# Scope

- Spawn the border/badge on every card and toggle visibility from `runtime.palette_index`. Alternatively, drive it from `SurfaceState.selected` like the existing chrome.
- Keep the rebuild when the visible page window, category, or filtered entries change.
- Optional: keep preview viewports across page changes by reusing hosts per slot.

# Acceptance criteria

- ECS test: selecting another card on the same page keeps the same Node entities and `MapPalettePreviewHost` entities.
- Visible behavior unchanged; native check of palette click and wheel.

# Out of scope

Map Maker feature work (BRL-6vjz, BRL-tztp).
