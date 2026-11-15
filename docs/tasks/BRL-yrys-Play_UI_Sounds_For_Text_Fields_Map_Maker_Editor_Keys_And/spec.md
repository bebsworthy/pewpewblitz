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
