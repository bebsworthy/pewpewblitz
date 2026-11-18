Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec updated — all listed paths still publish nothing; added exact owners and lines.

# Outcome
Deferred from BRL-ek8p (5cb62cbc). Remaining keyboard/gamepad shell input makes the same UI sounds as the controls BRL-ek8p covered.

# Context
`ui::feedback::UiFeedback` (`packages/brawler-client/src/ui/feedback.rs:15-25`: `Activated(Option<Surface>)`, `Back`, `Moved`, `Stepped`) is mapped by `audio/producers/ui.rs` to ui-click/confirm/back/move with 50 ms press dedupe and 70 ms tick spacing. Current emitters: `flow/input.rs:488,534,546`, `shell.rs:514,551,564,577`, `presentation.rs:149,172`, `flow/screens/practice_maps.rs:516,546,559`, `flow/screens/map_maker.rs:1301,1326,1336,1631`, `shell/settings_screen.rs:841`, `shell/controls/capture.rs:63`. Still silent:

- Text fields:
  - Server-select address/name editor commit (Enter/South) and cancel (Esc/cancel) — `flow/input.rs:~334-356` inside `collect_flow_input`.
  - Brawler name editor commit/cancel — `edit_name_input`, `flow/input.rs:687-734` (`:723` commit, `:732` cancel).
  - Map Maker create/rename name *confirm* (`map_maker.rs:1297`, `:1322`; cancels already sound).
  - Map Maker palette search close on Enter/Esc (`map_maker.rs:1363`) and `/` open (`:1380`).
- Map Maker editor (`map_maker_input`, `map_maker.rs:1230`): grid cursor arrows/D-pad (`:1406-1420`), Tab/RT2/LT2 asset cycle (`:1423-1428`), R rotate, M symmetry, P/E/V tools (`:1429-1443`), Enter/Space/South place (`:1579`), Delete/Backspace/RT erase (`:1589`), Z/Y/S/T/G/C/F (`:1599-1619`), and Esc that closes validation or clears a selection (`:1620-1629`; only the final back-to-library sounds).
- In-match menu open/close via the pause binding / platform Back: toggled in `apply_pause_request` (`lib.rs:596-608`), which writes no feedback; inside the menu only moves (`presentation.rs:149`) and keyed activation (`:172`) sound.
- Automation-only UI gallery keyboard path (`ui/gallery.rs:165-169`, `:384-429`).

# Plan
1. Add `MessageWriter<UiFeedback>` to each owner and write where state actually changes (compare before/after, as `presentation.rs:146-150` does for focus).
2. Decisions (record in ticket): text commit = `Activated(Some(Surface::Primary))`→ui-confirm (proposed); cancel = `Back`; Map Maker tool/asset/rotate/symmetry = `Activated(None)`→ui-click; grid cursor = `Moved` (tick spacing already limits rate) or silent; menu open = `Activated(None)`, close = `Back`.
3. Pause toggle: emit from a small system reading `ClientInputContext` Gameplay↔Menu transitions rather than inside `apply_pause_request` (pure fn), skipping rebind capture.
4. Gallery: optional; skip if it only serves automation.

# Acceptance
- Each listed path sounds once per real action and stays silent when nothing changes (e.g. cursor at grid edge, failed commit plays nothing or `Back` — decide).
- Focused ECS tests per emitting owner using the `RecordedUiFeedback` pattern (`shell.rs:1428-1439`): server editor, brawler name editor, Map Maker editor keys, Map Maker search, pause open/close.
- `cargo test -p brawler-client --lib`, `just lint`.
- Owner playtest (desktop); Pixel 8a device check of Android Back opening/closing the match menu with sound.

# Result (2026-11-18)

## Decisions (mapping)
- Text commit → `Activated(Some(Surface::Primary))` (ui-confirm); cancel → `Back`. A failed commit (rejected name) plays nothing.
- Map Maker tool/asset/rotate/symmetry/place/erase and Z/Y/S/T/G/C/F shortcuts → `Activated(None)` (ui-click), at most one per frame. These click on the key, like the toolbar button they stand for, so a no-op result (for example undo with empty history) still clicks, the same as the pointer button does. Enter with the Move tool queues nothing and stays silent. Mouse strokes and wheel asset cycling stay silent, as before.
- Grid cursor arrows/D-pad → `Moved` only when the cursor cell changes; silent at a grid edge.
- Palette search: `/` open → `Activated(None)`; Enter close → `Activated(Some(Primary))`; Esc/Back close → `Back`; typing silent.
- Map Maker create/rename Enter: the confirm resolves after input, so `map_maker_input` keeps a `Local<Option<KeyedMapNameCommit>>` and plays confirm on the next frame only if the name field actually closed. A failed create or rename is silent.
- Editor Esc that closes validation or clears an interaction/selection → `Back` (back-to-library already sounded).
- Match menu: the new `publish_match_menu_toggle_feedback` (presentation.rs, chained after `handle_match_menu_input`) watches `ClientInputContext`: Gameplay→Menu → `Activated(None)`, Menu→Gameplay → `Back`. Menu↔Shell (Settings) and match start/end are silent. Rebind capture never toggles the context, so it stays silent too. A Resume row or the HUD menu button already sound themselves; the UI producer's 50 ms press spacing drops the same-gesture duplicate.
- Scoreboard (BRL-ejag): holding Tab/Select is a peek, not a menu, so it stays silent on purpose. Closing the latched scoreboard with Esc / controller cancel / platform Back was silent; it now writes `Back` (a trivial fix in `handle_match_menu_input`). The pointer close button already sounded.
- UI gallery keyboard path: skipped (automation only).

## Files
- packages/brawler-client/src/flow/input.rs — server-select editor commit/cancel; `edit_name_input` returns `Option<UiFeedback>` for the brawler edit and creation name fields.
- packages/brawler-client/src/flow/screens/map_maker.rs — editor keys, cursor, search, Esc, keyed name confirm + tests.
- packages/brawler-client/src/presentation.rs — pause toggle watcher, latched-scoreboard cancel Back + test.
- packages/brawler-client/src/flow/tests.rs — server and brawler editor tests.

## Tests
- flow::tests::server_select_editor_commit_and_cancel_publish_feedback_and_a_rejected_name_is_silent
- flow::tests::brawler_name_editor_commit_and_cancel_publish_feedback_and_a_rejected_name_is_silent
- flow::screens::map_maker::tests::map_editor_keys_sound_once_per_real_action_and_stay_silent_at_the_grid_edge
- flow::screens::map_maker::tests::map_palette_search_open_commit_and_cancel_sound_and_typing_is_silent
- flow::screens::map_maker::tests::map_library_name_confirm_sounds_only_when_the_commit_succeeds
- presentation::tests::pause_requests_sound_the_match_menu_opening_and_closing_once

## Verification
- `just lint` — pass (target/yrys/lint.log).
- `just test` — pass, all suites (target/yrys/test.log).
- Not verified: real audio output, desktop or device playtest.

## Owner playtest (desktop)
Run `just server`, then `just client`. Listen for confirm (Primary), back, click and move ticks:
- Server Select: edit address/name, press Enter → confirm; Esc → back; an invalid name (for example a private-use character) + Enter → silence.
- Brawler details/creation name: Enter → confirm; Esc → back; invalid name → silence.
- Map Maker library: create/rename + Enter → confirm only when it succeeds.
- Map Maker editor: arrows → tick, and silence when pressed against an edge; Tab/R/M/P/E/V/Enter/Space/Delete/Z/Y/G/F → click; `/` → click; type → silence; Enter → confirm; `/` then Esc → back; Esc with a selection or with validation open → back.
- Match: Esc → menu opens (click); Esc → closes (back); Resume row → one sound, not two; menu → Scoreboard, then Esc → back.

## Owner playtest (Pixel 8a)
`just android-build <host:port>`, `just android-install`. In a match, the Android Back gesture opens the match menu (click) and closes it (back). The HUD menu button makes a single sound.
