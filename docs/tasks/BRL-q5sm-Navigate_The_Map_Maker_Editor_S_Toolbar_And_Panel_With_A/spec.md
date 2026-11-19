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

# Implementation (2026-11-19)

## Bindings

- **Keyboard: Q** toggles map-cursor ↔ chrome focus. Q was unbound in the editor; every existing shortcut (P/V/E, Tab, R, M, G, F, T, S, Z, Y, C, /, Delete/Backspace, Enter/Space, arrows, Esc) is unchanged.
- **Controller: Select (View)** toggles. Select was unbound in the editor (gameplay uses it for the scoreboard only); D-pad, South, East, West, North, LB/RB, LT/RT keep their roles.
- **In chrome focus:** arrows / D-pad move the flat 3 px accent ring to the nearest control in that direction (`library::spatial_neighbor`, shared with My Maps); Enter, Space or South presses the focused control; Esc or East first closes the category menu, then returns to the map cursor. Q/Select again also returns. All letter, trigger and face-button shortcuts still work in chrome focus; only the arrows/D-pad and Enter/Space/South change owner.
- Entering chrome focus starts on the active tool's segment (Place/Select/Erase); later it keeps the last focused control across rebuilds (stable indices).

## What was built

- `MapMakerRuntime::chrome_focus` plus `FlowUiAction::MapMakerToggleChromeFocus`; the toggle rebuilds the editor so the badge swaps icon and copy.
- `map_maker/focus.rs`: `navigate_map_editor` (runs after `map_maker_input` in `CollectFlowInput`, MapEditor only, skipped while asset search is typed), the mode badge, and `show_map_editor_focus_badge`.
- **Visible mode:** a corner badge left of the side panel, "Map cursor · Q / Select: controls" (crosshair) or "Controls · Q / Select: back to map" (menu icon, cyan outline). It is itself a flat control, so a click toggles and chrome focus can reach it. It hides with the ring after a touch press (`FocusVisibility`), so touch players never see it.
- `style_flat_controls` draws the ring in the editor only in chrome focus (My Maps unchanged).
- `reveal_map_maker_focus`: in chrome focus, asset rows follow focus instead of the palette selection, so focused rows scroll into view.
- `map_maker_input`: in chrome focus the arrows/D-pad no longer move the map cursor and Enter/Space/South no longer place.
- Durable doc: `docs/13-player-ux.md` (Map Maker paragraph).

## Verification

- `flow::tests::map_editor_chrome_focus_walks_the_controls_and_returns_to_the_map_cursor` (real layout at 1280x720, keys through `InputPlugin`): arrows move the cursor and no ring shows in map mode; Q enters chrome focus on Place with exactly one ring; Right goes to Select without moving the cursor; Down from the Assets tab reaches the asset rows; Enter selects the focused row and keeps chrome focus; Esc returns without leaving the editor; arrows move the cursor again and the ring is gone.
- `map_maker::tests::map_editor_keys_sound_once_per_real_action_and_stay_silent_at_the_grid_edge` updated: Q now clicks like a control (K is the unbound-key check).
- `just lint`: pass. `just test`: every suite passed (2612 tests) except the wall-clock performance gates, 11 of 12 failing under the parallel full run; `cargo test --locked -p brawler-client --features network-test --test performance -- --test-threads=1` then passed 12/12.
- Captures (off-screen, `BRAWLER_SHELL_CAPTURE_BIND=127.0.0.1:5130`, 1280x720 and 914x411): before in `target/q5sm/before/<route>/<size>/`, after in `target/q5sm/after/<route>/<size>/` (worktree `agent-ac1506e403a2eb479`). New route `map-editor-focus` (Assets on Cover, chrome focus on: ring on Place, badge "Controls"). The badge shows on every editor route in the macOS capture client (no touch). The "before" focus capture is the same route on the base code, where the toggle did not exist.

## Pixel / controller playtest

- **Run:** `BRAWLER_ROUTED_BIND=0.0.0.0:5000 just server`, `just android-build <mac-lan-ip>:5000`, `just android-install`; pair a Bluetooth controller to the Pixel 8a.
- **Try:** open a draft; with only touch, confirm no badge and no ring. Press any controller button: the badge appears ("Map cursor"). D-pad moves the cursor, South places. Press **Select**: ring on the tool segment, badge "Controls". Walk the top bar, the tabs, Cover's asset rows (they scroll into view), Check rows and their Add/Show; press South on each. Press East to return to the cursor. Tap the screen: ring and badge hide. Repeat on macOS with a keyboard (Q, arrows, Enter, Esc).
- **Look for:** the ring never on two controls; D-pad never moving both the ring and the cursor; nothing unreachable (the collapse chevron, the chip, the selection toolbar while Select has a selection).

## Known limits

- The badge copy names keyboard Q and controller Select together; it does not adapt to the device in use.
- Spatial navigation follows on-screen positions; inside the category dropdown it can also leave the menu (Esc closes it).

## Commit

cdf9f562 on branch `worktree-agent-ac1506e403a2eb479` (shared with BRL-855z).
