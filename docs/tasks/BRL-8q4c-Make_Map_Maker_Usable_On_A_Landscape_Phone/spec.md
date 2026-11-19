# Context (found in BRL-q148, 2026-11-18)
- BRL-q148 added the shared touch-target floor (`packages/brawler-client/src/ui/touch_target.rs`): on Android every `FlowButton`/`ShellButton` gets `Node.min_height >= 48 / UiScale`.
- Map Maker is exempt (`TouchTarget::exempt()` on every button in `flow/screens/map_maker.rs`) because its layout is not phone-ready:
  - Without the floor, the library's buttons are 19-25 dp tall at 868x411 (100 % and 80 % UI scale).
  - With the floor, the library's CREATE MAP / BACK buttons go below the 411 px window (y 372-474). The library root does not scroll, and the card grid has no scroll area.
  - The editor's toolbars (38-54 UI px buttons, `map_maker.rs` tool/tab/asset builders) were not measured on a phone. They are dense and would need a compact layout.
- Measure with a scratch test like the one in BRL-q148. Use `configured_lobby_flow_app_with` plus `layout_harness::add_layout(868, 411)`, `apply_viewport_fit`, `MapCatalogResource(MapContentCatalog::embedded())`, then `NextState(ClientFlow::MapLibrary)`. Assert with `layout_harness::{unreachable_buttons, short_buttons, overlapping_buttons}`. Note: an injected `OpenMapMaker`/`CreateMap` races `resolve_map_maker_action`, so set the state directly.

# Plan
1. Make the library scroll (or use a compact layout) so every card action, CREATE MAP and BACK can be reached at 868x411.
2. Design a compact editor toolbar for phones.
3. Remove `TouchTarget::exempt()` from map_maker.rs. Add Map Library, Create Map and Editor to `flow::tests::shell_controls_are_48_dp_touch_targets_on_a_landscape_phone`.

# Acceptance
- With touch mode on, at 868x411, at 100 % and 80 % UI scale, every visible, enabled Map Maker button is reachable and >= 48 dp, and no two controls overlap.
- Desktop 1280x720 Map Maker layout is unchanged.

# Delivered with BRL-2xpc direction D (2026-11-19)

## What changed

- **Library:** My Maps (lighter-shell library) scrolls its tiles. The selected map's actions sit in one bar, and dialogs (create, delete) are their own UI layer. Every My Maps control can be reached at 868x411.
- **Editor:** a compact layout with a thin top bar and a collapsible Assets/Check side panel. Erase is in the top bar's segmented control, so a phone can always reach it.
- **Control sizing:**
  - `TouchTarget::exempt()` is gone from Map Maker and from the code.
  - Flat controls are authored at 48 dp: `ctl = max(76, ceil(48 / UiScale))` UI px whenever the window is under 640 logical px tall or touch is on. The Android floor therefore never resizes the layout, including at 80 % UI scale (93 UI px).
- **Desktop 1280x720:** the layout was redesigned with D, not kept unchanged. That was the owner's direction, which supersedes the original "desktop unchanged" acceptance.

## Verification

`flow::tests::shell_controls_are_48_dp_touch_targets_on_a_landscape_phone` now also covers Map Maker through `assert_map_maker_phone_touch_targets`, at 868x411 with touch on, at 100 % and 80 % UI scale:

- My Maps with 5 drafts;
- the Create map dialog;
- the editor collapsed;
- the editor with Assets open;
- the editor's category menu;
- the editor with Check open;
- the editor with the Erase tool active.

In every state, every visible enabled button is reachable (directly or inside a scroll area), at least 48 dp, and overlaps no other button. The test passes.

## Arena share

With the side panel collapsed, the editor's controls cover:

- 19.6 % of the screen at 914x411 (the board's estimate was 20.0 %);
- 11.0 % at 1280x720.

The test `map_editor_controls_leave_the_arena_most_of_a_landscape_phone` checks this.

The Pixel playtest is described in BRL-2xpc's spec.
