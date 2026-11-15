Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec refreshed with a probable root cause from code. Not fixed. No shell or overlay code changed after 6a6d6599 (b97a8888 only added logging to `commit_flow`).

# Found
Device check 2026-11-15, Pixel 8a, HEAD 6a6d6599. Reproduced twice (09:24 and 09:30).

# Repro
1. Start any Practice match (Wipeout 2v2 on Feature Yard used here).
2. Match menu → SETTINGS. Stay on the Settings page until the match completes. Bots finish Wipeout quickly, or wait for the 3 min timer.
3. The client logs "observed authoritative match completion; returning to lobby" and joins a fresh lobby. The Settings page stays drawn.

Actual:
- The on-screen BACK, the Display/Audio tabs and the rows no longer react. Frames are still presented at ~30-60 fps, so the app is not frozen, only inert.
- The first Android Back does nothing visible. The second Back moves the task to the background (`moveTaskToBack`), as at a lobby root (Results).
- After about 24 s in the background, the session is dropped and a Connection Lost panel appears over the same Settings. Back on that panel reveals the stuck Settings again.
- Only a force-stop recovers.

Expected: the match-owned Settings (and the match menu) close when the match leaves the Match flow, and Results/Dashboard take input.

# Probable cause (code at HEAD, packages/brawler-client/src)
- Opening: `shell.rs` `handle_shell_actions` `OpenMatchSettings` (L703-712) sets `SettingsReturnTarget::MatchMenu` and `ClientOverlay::Settings`, inserts the drafts and spawns the Settings `OverlayRoot` (`shell/settings_screen.rs` L135, `GlobalZIndex(500)`).
- Completion: `flow/reducer/match_flow.rs` `MatchStartReturned | FreshLobbyReturn` (L72-104) moves to Results/Dashboard and commits `OverlayCommit::Clear`. `flow/reducer.rs` `commit_flow` (~L896) then sets `ClientOverlay::None` directly. The shell only despawns its roots through its own `close_overlay` (shell.rs ~L760). `present_flow_requested_overlay` (L267-301) returns early while a root exists. So:
  - The Settings root stays on screen above Results (`flow/screens/results.rs` L53 `GlobalZIndex(450)`) and absorbs its picks.
  - `active_layer(&ClientOverlay::None)` = `FlowOwned` (L425), so `collect_pointer_actions` skips every Settings button (L656). They are inert.
  - `restore_match_menu_after_settings` (L741-750) sees overlay None with target MatchMenu and sets `ClientInputContext::Menu` after Results is entered.
  - The `InputSettingsDraft` / `ShellSettingsDraft` resources leak.
- The same leak can happen for any external `OverlayCommit::Clear` or `Error` while the shell's Settings or Credits is open, for example the Connection Lost panel over Settings.

# Plan
1. In the shell (PresentFlow, before `present_flow_requested_overlay`), add a sync system. When `ClientOverlay` is no longer Settings or Credits but a shell `OverlayRoot` exists, despawn it and remove the drafts, as `close_overlay` does. Keep unsaved-draft semantics the same as Back: autosave already runs, so check that `settings_screen::autosave` is not skipped.
2. On `OnExit(ClientFlow::Match)` (`flow.rs` L265 `exit_match_input`), reset `SettingsReturnTarget` to `ProductFlow` so `restore_match_menu_after_settings` cannot force `ClientInputContext::Menu` outside a match. Also close `MatchMenuState` / the pause overlay.
3. Confirm that Android Back on Results then follows the normal Results path (BRL-h088).

# Acceptance
- A new shell + flow test (start from `shell.rs` tests `shell_test_app` L1070 / `request_settings` L1105, and the `flow/tests.rs` `inject_session_observation(..., FreshLobbyReturn)` pattern near L794). Enter Match, send `MatchSettingsRequest`, then inject `FreshLobbyReturn` with a result context. Assert no `OverlayRoot` remains, `ClientOverlay::None`, flow Results, `ClientInputContext::Shell`, no draft resources, and that a Results button release activates.
- The same test with an `Error` overlay (Connection Lost) over Settings leaves only the error panel interactive.
- `cargo test -p brawler-client` passes.
- **Pixel 8a device check:** repeat the repro and reach Results without a force-stop. Results buttons and Back work.

Evidence: /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/dc/x02.jpg (Settings opened in match), x03.jpg (after completion), s-x04.jpg (touch BACK and Back gesture with no effect), k03.jpg, h35-connlost-back.jpg, k05-k07.jpg (first occurrence), server.log 09:24 and 09:30.

# Verified 2026-11-16 (commit 6b1e8772)
- Tests: `match_completion_closes_settings_opened_from_the_match_menu` and `connection_lost_over_match_settings_leaves_only_the_error_interactive` fail without the fix and pass with it. `just lint` and `just test` pass.
- Pixel 8a (release APK, server 192.168.31.211:5000): Practice Wipeout 2v2 on Feature Yard. Opened Match menu → SETTINGS and stayed on that screen. At 00:18:12 the match completed (flow Match → Results), Settings closed by itself and Results showed (DEFEAT 7–10). DASHBOARD on Results worked. Dashboard MENU → SETTINGS → BACK still works. No force-stop needed.
- Evidence: scratchpad nq/n06s.png (Settings in match), n08s.png (Results after completion), n09s.png, n11s.png, n12s.png.
