Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec refreshed — the scoreboard is still one text node built by `roster_entry_text`.

# Context (found during BRL-094c, 2026-11-14, Pixel 8a)
- Choosing SCOREBOARD in the match menu still shows the old text panel ("T1 YOU Brawler-42DD W1 HP 85/85 B0 ready"). At 914x411 it is small, unstyled, and reads as debug output. BRL-3zas left this panel out of scope; the results screens (BRL-wfce/BRL-h0yc) do not cover the live in-match scoreboard.
- BRL-094c (done) made a tap or click anywhere dismiss the latched panel back to the menu; there is still no visible close affordance.

# Current code (HEAD 3437c0d8)
- Spawn: `packages/brawler-client/src/presentation.rs:20-40` `spawn_client_hud` — a single `ScoreboardOverlay` `Text` (display font via `ui::theme::hud_text(font, 20.0)`, so not literally monospace, but one left-justified string), absolute 22%/22%/20% insets, dark `BackgroundColor`, `GlobalZIndex(250)`. Called from `presentation_3d/mod.rs:1628`.
- Text: `hud.rs:471-479` sorts `MatchRosterPresentation` entries by team and joins `roster_entry_text` (`hud.rs:1001-1034`, "T{n} YOU name W{preset} HP a/b B{boost} status"); written at `hud.rs:527-529`. Query at hud.rs:397-404.
- Visibility: `presentation.rs:223-240` `update_client_hud` (held `pending.scoreboard_held` from `input.rs:313-316`, or `MatchMenuState::scoreboard_latched`). Latch/dismiss: `presentation.rs:76-123` (`LatchedScoreboardDismiss`, cancel or any tap/click). Menu entry: `presentation/match_menu.rs:54` (index 2 -> latch at presentation.rs:182).
- Reusable HUD styling: `hud/plate.rs` (`spawn_plate` :227, `spawn_team_strip` :300 — team-colored rows with health/boost/down state); shell `ui::surface::Surface` for the panel/close button.
- Tests touching it: `tests.rs:1233` (held scoreboard visible without debug surface), `hud.rs:1090,1125` (spawn `ScoreboardOverlay` with `Text` and read it — these will need the new row structure), `presentation.rs:333-392` (latch/dismiss).
- HUD layout tests: `hud/screen_tests.rs:18` `VIEWPORTS = [(1280,720),(868,411),(914,411),(1560,720)]`.

# Scope
- Present the live scoreboard as a styled panel: per-team sections with team colors, one row per player (name, YOU marker, health bar or value, status), readable labels instead of W/B/T abbreviations. Observer-safe facts only (what `MatchRosterPresentation` already holds from public participant state); keep the held (Tab/Select) and latched (menu) entry points.
- A visible touch-reachable close/back control in addition to Back and tap-anywhere.
- Retained nodes: spawn rows for the roster and update text/visibility in place (no per-frame respawn); bound to the match's participant count.

# Plan
1. Replace the single `Text` with a `ScoreboardOverlay` root node + child row entities (marker components), spawned in `spawn_client_hud`; move formatting from `roster_entry_text` into per-field labels.
2. Update `update_readiness_hud` (hud.rs:365-530) to fill rows instead of `scoreboard_text`; keep the `Without<...>` query disjointness.
3. Add a close button (`Surface::Back`) on the latched panel that clears `scoreboard_latched` and returns to `ClientInputContext::Menu`; hide it when only held.
4. Adjust the existing tests listed above.

# Acceptance criteria
- Readable at 868x411 and 914x411 without overlap or clipping for 6 long names (`hud/screen_tests.rs` NAMES); desktop 1280x720 unchanged in behavior (held + latched both work).
- New layout test in `hud/screen_tests.rs` over the four `VIEWPORTS`: scoreboard rows and close button on screen, no collisions.
- ECS test: close button dismisses the latched scoreboard; tap-anywhere and Back still do.
- `just lint`, `just test`.
- Pixel 8a device check: open SCOREBOARD from the match menu in a 3v3, read it, close it with the button.
