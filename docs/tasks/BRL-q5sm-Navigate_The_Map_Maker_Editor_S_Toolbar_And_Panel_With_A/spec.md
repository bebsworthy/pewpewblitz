# Context (deferred from BRL-2xpc, 2026-11-19)

BRL-2xpc rebuilt Map Maker in direction D.

- **My Maps** has spatial keyboard and controller focus with a flat focus ring (`map_maker/widgets.rs` `style_flat_controls`, `map_maker/library.rs` `navigate_map_library`).
- **The editor** keeps its existing D-pad map cursor and keyboard/gamepad shortcuts:
  - P, V and E pick the tool;
  - Tab and the triggers cycle assets;
  - G toggles the grid, M the mirror and F fits the view;
  - T tests and S saves.
- **The gap:** the editor's flat controls take no focus. Their indices start at 1000, and the ring shows only in My Maps.

# Scope

- Add a visible switch between map-cursor mode and chrome-focus mode, for example on Select/View or a shoulder button.
- Reuse `spatial_neighbor` and the flat focus ring.
- Keep focused asset rows in view (`reveal_map_maker_focus`).
- Keep the touch-hides-focus rule (`FocusVisibility`).

# Acceptance

- Every editor control can be reached and activated with a controller, and with the arrow keys plus Enter.
- Map-cursor editing still works, and the current mode is visible.
