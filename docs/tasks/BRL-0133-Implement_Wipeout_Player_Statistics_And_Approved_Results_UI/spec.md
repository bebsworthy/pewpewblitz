## Scope
Implement approved scoreboard-v2.png and results-v2.png from asset_src/player-shell/concepts. Share team/player statistics presentation using the existing native UI toolkit. Preserve other mode behavior. No fabricated metrics, rewards or ranks.

## Acceptance
- Wipeout records kills using the same validated defeat attribution as team scoring, including recent hostile damage credit. Every valid defeated participant gains a death; duplicate/stale facts do not count. Stable player identities retain departed participants and reset on match restart.
- Replicated bounded player statistics and team totals form one scoring snapshot. Preserve final statistics before routed teardown, including local identity and names; respect global protocol compatibility.
- Live scoreboard and results show two teams, one score each, named players and KILLS/DEATHS columns. Highlight local player without redundant selected labels. Use approved cream/teal/orange/navy styling and shared slanted, cut-corner actions.
- Dashboard and replay retain existing authoritative navigation/availability behavior. Keyboard/gamepad focus remains visible and isolated from gameplay.
- Verify attribution/reset/lifecycle with focused tests, client/server checks and applicable network tests. Inspect native wide and compact rendering, including actual results; record limits honestly.

## Implementation constraints
Server authority only; no presentation-derived counters. Reuse existing scoring fact validation, stable PlayerId, public roster, shared surfaces, fonts and background. Preserve unrelated working-tree changes. No new general UI framework.

## Design acceptance
User accepted the latest results-v2 mockup with “last one is good go for it”; scoreboard shares its kill/death model. BRL-0132 owns design evidence; this ticket owns implementation.


## Scope correction
User explicitly says “don't touch the in game HUD”. Keep the HUD and live scoreboard visually unchanged. Implement only approved post-match results and the authoritative statistics required by that screen. Live-scoreboard rendering acceptance is deferred; no HUD presentation changes in this ticket.


## Verification so far
- Client role Clippy with warnings denied passes; server role Clippy with warnings denied passes.
- 466 client-related unit/ECS tests pass. After final presentation changes, all 75 flow tests pass.
- Four focused Wipeout tests pass: existing rules validation, duplicate/stale facts, uncredited environmental deaths, and retained recent hostile attribution after departure.
- New separate-App network test proves exact final scoring snapshot replication to both clients and reset on restart.
- Client lifecycle test verifies missing/incomplete Wipeout snapshots delay routed unlink and retained names/counters/local-team survive root teardown.
- Native 3v3 Practice verification is in progress on isolated localhost:5140 using the production client/server; no user server processes were stopped.


## Native wide verification
Logged 3v3 Practice on Verdant Crossfire completed with Victory 10–8. Native results showed all six players; own kills 6/4/0 and deaths 1/3/4, opponents kills 5/2/1 and deaths 3/4/3. Counters reconcile with team scores. Local row highlighted; all text/columns/actions readable. Left/Right moved focus between Dashboard and Practice Again; Return on Practice Again entered a fresh same-mode match at 0–0. Initial unlogged observation landed at Dashboard, but the logged repeat confirmed authoritative capture, fresh lobby admission and stable Results; no routing failure reproduced.

The broader match suite initially hit the existing process-global diagnostic-capture guard under parallel test execution. Re-run with the repository's canonical --test-threads=1 passed all 14 match scenarios, including four-player repeated restart and the new snapshot test. Use canonical serialization from the start for these tests.


## Completion evidence
- Native 800x700 3v3 Results verified: Victory 10–8; own kills 5/4/1, deaths 4/2/2; opponents kills 4/3/1, deaths 3/3/4. All six rows, column headings, local-player highlight and both actions fit without clipping. Wide 1440x900 and compact rendering use the production toolkit. Physical gamepad button testing was not performed; keyboard Left/Right/Return and shared navigation were verified.
- All 14 match-network scenarios pass with --test-threads=1. Client and server library Clippy pass. Client build/server build pass. Formatting and diff whitespace checks pass.
- Broader network-test --tests Clippy has seven pre-existing warnings, recorded in BRL-0134. New-change findings were corrected; no global suppressions added.
- HUD/live scoreboard visuals and gameplay input remain unchanged. The only hud.rs changes supply defaults in two test fixtures for the expanded scoring shape.
- Durable UX and global protocol documentation updated. Protocol 45 requires matching rebuilt client and server roles.

## Learning review
Keep final counters and completion in one authority-owned replicated snapshot: component arrival order must not permit a partial result to trigger teardown. Use stable match player identity after fresh lobby admission assigns a new identity. Native verification is necessary in addition to isolated tests; inspected actual routed results and replay, not only a constructed fixture. Consult canonical network test options before execution because diagnostic capture is process-global. Shared control rows impose control-specific padding; noninteractive results rows use the existing panel surface with consistent column geometry.

## Feedback disposition
Approved results design implemented. Explicit instruction not to touch the in-game HUD honored. Live-scoreboard redesign is excluded from this delivery. No implementation commit requested in this turn.
