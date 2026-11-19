# Context

Deferred from BRL-pw9k, which ships `--shell-capture <route>` (`packages/brawler-client/src/flow/automation.rs`, `scripts/shell-capture.sh`) for the 19 shell routes reachable from an idle Dashboard. These routes were not included:

- `results`: needs a completed match. A Practice match lasts up to 180 s (Heist bots end it in 18-40 s, but not reliably), so a route would either wait minutes or need a server-side short-match rule. Leaving with `ConfirmLeaveMatch` releases the seat and does not show Results.
- `match-menu`: the in-match menu is `ClientInputContext::Menu` with a `MatchMenuState` selection (`presentation.rs`), not a `FlowUiAction`. Opening it from the driver needs a new flow action or a second navigation path, which BRL-pw9k's scope rules out.
- `leave-confirmation` (`ClientOverlay::LeaveConfirmation`) is set from that match menu, so it has the same problem.
- `match-loading`: transient, so it needs a mid-transition capture policy.

# Plan sketch

1. Add a Practice prefix to the route table: `OpenPracticeMaps` -> `StartPractice`, waiting up to the formation deadline for `ClientFlow::Match`.
2. Decide how the match menu is opened canonically (for example a `FlowUiAction::OpenMatchMenu` that the existing Escape/Start path also uses) before adding `match-menu` and `leave-confirmation` routes.
3. For Results, choose either a bounded wait on a Heist Practice match or an operator match rule that ends quickly, plus capture offsets inside the Results reward burst.

# Acceptance

- `just shell-capture` adds PNG/JSON pairs for the new routes at 1280x720 and 914x411, with no second navigation path.


# Decisions (2026-11-19)

- **Match menu opening:** the driver uses `crate::apply_pause_request`, the same local pause request the keyboard, controller, platform Back and touch HUD samplers call. It adds no new `FlowUiAction` and no second navigation path. The match menu is input context (`ClientInputContext::Menu`), not flow state, so a flow action would have needed a larger refactor of the Escape/Start path. That refactor isn't needed.
- **Leave confirmation:** `MatchMenuState` gets an automation-only (`#[cfg(feature = "automation")]`) `request_row_activation(row)`. `handle_match_menu_input` consumes it like a keyed activation of that row, so the existing LEAVE MATCH row code opens `ClientOverlay::LeaveConfirmation`. Player builds compile it out.
- **Results:** reached through the real authoritative time limit, with no server-side test rule. Before the first live route, `scripts/shell-capture.sh` restarts its scratch supervisor on the same data with a capture catalog. The catalog is a copy of `config/server/game-types.ron` with `match_duration_seconds` set to `BRAWLER_SHELL_CAPTURE_MATCH_SECONDS` (default 30). This is the same operator-catalog contract `scripts/network-routed.sh` verification uses, and the server validates it normally. Hot Zone's `capture_seconds: 30` stays valid because it must not exceed the duration. `BRAWLER_SHELL_CAPTURE_GAME_TYPE=<id>` keeps only one game type, which Practice then selects because it is first. Earlier non-live routes keep the production catalog, so Game Select captures are unchanged.
- **Leaving each match:** every live route leaves its match after the capture through pause, the LEAVE MATCH row and `ConfirmLeaveMatch`, ending on an idle Dashboard. Without this, the next route would get an automatic rejoin. `results` first sends `QueueAgain` (Practice Again) and waits for `ClientFlow::Match`, which is an automated replay check.
- **Match Loading:** a Practice load lasts about 0.5-0.9 s, and off-screen frames run well below 60 Hz. The route therefore captures 0.3 s of `Time` after Match Loading appears (just past the 0.25 s surface entrance) instead of after the 90-frame settle. Captures of other screens are unchanged.
- **Driver changes:** steps also match the match-menu flag. Live steps get a 24,000-frame timeout. The script gives live routes a 300 s wall-clock timeout (`BRAWLER_SHELL_CAPTURE_LIVE_ROUTE_TIMEOUT_SECONDS`).

# Verification (2026-11-19, branch worktree-agent-a0d4c8fc0337e7e80, commit e1d9deee)

- `cargo test -p brawler-client --features automation --lib -- automation shell_capture match_menu`: 33 passed. This includes the new `live_shell_capture_routes_leave_the_match` and `driver_opens_the_match_menu_through_the_pause_request_and_menu_rows` tests. `shell_capture_route_steps_match_the_reducer` now skips the live routes.
- `just lint` passes (fmt, Clippy for every role, feature isolation, layout checks).
- `just test` passes. Every suite is ok, including the network suite (132 passed, serial) and the performance gates.
- Native off-screen captures with `BRAWLER_SHELL_CAPTURE_BIND=127.0.0.1:5100`, one script run per route, both sizes, `BRAWLER_SHELL_CAPTURE_DIR=target/wqst/live-<route>`. All four routes passed at 1280x720 and 914x411:
  - `target/wqst/live-match-loading/{1280x720,914x411}/match-loading.png` shows Wipeout 2v2 Match Loading ("Connecting", CANCEL MATCH START focused).
  - `target/wqst/live-match-menu/.../match-menu.png` shows MATCH MENU with RESUME focused.
  - `target/wqst/live-leave-confirmation/.../leave-confirmation.png` shows LEAVE MATCH? with KEEP PLAYING focused.
  - `target/wqst/live-results/.../results.png` shows Wipeout 2v2 Results (DEFEAT, team scores and per-player rows, PRACTICE AGAIN focused). Each run then played again and left back to the Dashboard.
- I inspected every PNG. No layout defects in the new routes. At 914x411 the Results trophy icon is scrolled partly off the top, because the shell scroll keeps the focused action visible. This is existing shared-shell behavior and also happens on Wipeout. I'm leaving it for owner review.
- Hot Zone and Heist Results evidence for BRL-h0yc used `BRAWLER_SHELL_CAPTURE_GAME_TYPE=hot-zone-3v3|heist-3v3` on port 5101 (see BRL-h0yc).

# Known limits

- The capture is taken after the 90-frame settle, so it shows the settled Results state rather than a frame inside the reward burst. Burst motion still needs a native look.
- Match Loading content is owned by concurrent map-publishing work. This ticket only captures it.
- Re-verified on the committed build: `leave-confirmation` at 914x411 passed again (`target/wqst/final`, port 5102).
