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


# Implementation (2026-11-18)
- New `packages/brawler-client/src/hud/scoreboard.rs` owns the panel: `ScoreboardState` resource (derived in `update_readiness_hud` with `set_if_neq`, like `PlateState`) and `present_scoreboard`, chained after `present_plate`. `spawn_scoreboard` runs from `spawn_readiness_hud`; `presentation::spawn_client_hud` and its `presentation_3d` call are gone (the HUD owns its nodes; `presentation` still owns visibility, latch and dismiss).
- Layout: full-screen `ScoreboardOverlay` root (GlobalZIndex 250, light dim) centering a `Surface::Panel` (max 1180 authored px). Header: SCOREBOARD title + `ScoreboardCloseButton` (`Surface::Back`, "‹ BACK", 76 authored px tall, about 49 logical px on a 411 px phone). Two observer-relative team sections side by side: section header chip in team color (YOUR TEAM ally blue / OPPONENTS enemy red; a teamless observer sees TEAM 1 / TEAM 2 in neutral PANEL_RAISED), then one row card per player (plate ink background, local row green border plus a YOU tag): line 1 name (clipped, never wrapped) + YOU; line 2 health bar (green, orange under a third, empty when defeated or unknown), "HP a/b" or "HP --", and a status label right-aligned (ALIVE, READY, RESTART READY, DEFEATED, RESPAWN IN Ns, PROTECTED Ns, DISCONNECTED; "BOOST +N" appended when a Zombie boost is held).
- Retained nodes: two sections are spawned once; each section's rows are spawned on demand (already filled) when the roster grows past the rows it has, then rewritten in place and hidden beyond the current roster, so row count is bounded by the largest participant count seen. No per-frame respawn.
- Close button: `handle_match_menu_input` dismisses the latch on Back (cancel), the close button's `ReleaseActivation`, or any tap/click (BRL-094c), all returning to `ClientInputContext::Menu`. `update_client_hud` shows the close button only while latched; the held (Tab/Select) panel hides it.

# Decisions
- Weapon preset dropped from the scoreboard (and from `CachedRosterEntry`): it is an optional numeric source-preset ID, not a player-readable name; naming it would need a catalog lookup outside this ticket's scope (name, YOU, health, status). Team is conveyed by sections, boost only when non-zero.
- The public status keeps the lobby `ready` flag through the match (`concealment/mod.rs` precedence: respawn > protected > defeated > restart_ready > ready > alive), so every fighter read READY in play. During Countdown/Active the scoreboard shows READY/RESTART READY as ALIVE; outside play it keeps the ready labels. Presentation-only; no server or wire change.
- "HP --" is shown when the observer has no fighter health (a culled/concealed fighter); the scoreboard reads only `MatchRosterPresentation` (public participant state + replicated fighters), so it stays observer-safe.
- Kept `Surface::Back` for the close control so it shares the shell's back styling and UI_BACK cue.

# Verification
- New tests: `hud::screen_tests::latched_scoreboard_fits_six_long_names_and_its_close_button` (four VIEWPORTS, six NAMES, longest statuses, names measured in the shipped Lilita One: 6 rows, close button and section headers on screen without collisions, health/status labels inside their row without overlap, no clipped name); `presentation::tests::the_latched_scoreboard_close_button_returns_to_the_menu_and_shows_only_while_latched` (press then release on the close button clears the latch and returns to Menu; close hidden while only held); `hud::scoreboard::tests::{sections_are_observer_relative_with_the_local_player_first, rows_use_readable_health_and_status_labels}`. Existing `menu_scoreboard_is_latched_and_cancel_returns_to_the_continuing_menu` still covers Back and tap-anywhere. `hud.rs` retained-text tests now track `ScoreboardState` instead of the old Text; `roster_entry_text` and its test were removed; `hud/scoreboard.rs` joined the display-font glyph scan.
- `just lint`: pass. `just test`: pass (exit 0, every suite ok, including network and performance gates; log target/ejag/just-test.log).
- Native captures (macOS, Practice wipeout-3v3 with bots, own supervisor on 127.0.0.1:5077 with `--data-directory target/ejag/server`, automation client `--product-practice-smoke --window-size WxH --screenshot-dir ... --render-report ...`). A temporary env-gated hook (removed before handoff) latched or held the scoreboard once Active, applied the product shell's `viewport_fit` UiScale and installed `SurfacePlugin`, which the auto-connect automation client otherwise lacks (it does not compose the product shell):
  - target/ejag/cap-914x411-latched/brawler-000700.png
  - target/ejag/cap-914x411-held/brawler-000700.png
  - target/ejag/cap-1280x720-latched/brawler-000700.png
  - target/ejag/cap-1280x720-held/brawler-000700.png
  All six rows readable, no overlap; BACK shown only when latched.
- Not done here: the Pixel 8a device check (open SCOREBOARD from the match menu in a 3v3, read it, close it with the button).

# Mistakes
- First capture used the automation client's UiScale 1.0 and no SurfacePlugin, which made the panel overflow 914x411 and hid the Back styling; the product shell applies `viewport_fit` and the surface plugin, so captures must mirror that. First row layout let a long status overlap "HP 1000/1000"; fixed by letting only the bar shrink and adding a label-overlap assertion to the layout test.


# Device check (2026-11-18)
Pixel not connected during closeout; left as playtest: open SCOREBOARD from the match menu in a 3v3, read it, close it with BACK.
