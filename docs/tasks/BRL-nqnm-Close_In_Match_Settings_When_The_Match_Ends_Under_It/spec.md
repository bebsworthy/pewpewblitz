# Found
Device check 2026-11-15, Pixel 8a, HEAD 6a6d6599. Reproduced twice (09:24 and 09:30).

# Repro
1. Start any Practice match (Wipeout 2v2 on Feature Yard used here).
2. Match menu -> SETTINGS. Stay on the Settings page until the match completes. Bots finish Wipeout quickly, or wait for the 3 min timer.
3. The client logs "observed authoritative match completion; returning to lobby" and joins a fresh lobby. The Settings page stays drawn.

Actual:
- The on-screen BACK, the Display/Audio tabs and the rows no longer react. Frames are still presented at ~30-60 fps, so the app is not frozen, only inert.
- The first Android Back does nothing visible. The second Back moves the task to the background (`moveTaskToBack`), as at a lobby root (Results).
- After about 24 s in the background, the session is dropped and a Connection Lost panel appears over the same Settings. Back on that panel reveals the stuck Settings again.
- Only a force-stop recovers.

Expected: the match-owned Settings (and the match menu) close when the match leaves the Match flow, and Results/Dashboard take input. Or Settings stays usable and BACK returns to Results.

# Acceptance
- A flow/shell test opens the match-menu Settings, completes the match, and asserts that the overlay is closed or still interactive, and that Back does not leave a dead screen.
- Pixel 8a: repeat the repro and reach Results without a force-stop.

Evidence: /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/dc/x02.jpg (Settings opened in match), x03.jpg (after completion), s-x04.jpg (touch BACK and Back gesture with no effect), k03.jpg, h35-connlost-back.jpg, k05-k07.jpg (first occurrence), server.log 09:24 and 09:30.
