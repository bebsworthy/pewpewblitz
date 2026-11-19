Scope: post-match Results only. No in-game HUD, scoreboard, or gameplay scoring changes. Reuse the accepted results header, shared slanted surfaces, team ordering, and action buttons. Hot Zone shows final capture progress per team; Heist shows each team's remaining idol health with exact current/maximum totals and a percentage bar. Do not invent individual objective statistics. Preserve authoritative outcome including draws/forfeits.

Add a bounded replicated objective completion component containing match identity, result, and final objective values, published after authoritative outcomes. Retain it through the routed lobby transition; wait for the matching snapshot before unlinking. Use the global protocol revision for the new wire component. Reuse existing replay gating. Verify snapshot capture (including missing/wrong identity), final rendering text, compact layout and focused client/server checks. Native wide/compact evidence required before visual handoff. Preserve unrelated dirty changes. User visual acceptance remains required.


Implementation: a terminal ObjectiveResult component is published after MatchSet::Outcomes and before combat finalization; it clears on restart and does not update after capture. Client handoff requires matching match/result. Protocol 46 registers the new component. Hot Zone cards show capture percentage; Heist cards show exact idol current/maximum health and a proportional bar. Existing Wipeout rows, replay gating, and HUD are unchanged.


Verification so far: 76 client flow tests; client objective snapshot retention test; 2 server terminal-snapshot tests; client/server library Clippy with -D warnings; 5 Heist network scenarios and 9 Hot Zone scenarios (including restart soak), serial execution, all passed. Native routed Hot Zone 3v3 at 1440x900 reached Results with 21.6% vs 100.0%; panels and action focus inspected. Native review uses isolated server 127.0.0.1:5142 and target/ui-review/objective-server data. CUA can return stale inactive-window images; refocus and compare AX before accepting a render.


Learning: Heist cannot use the old scalar score fallback because its objective health lives on separate replicated entities. Capture final values with the authoritative outcome instead of racing teardown or guessing from winner/loser. Share the existing results shell, but use objective-only panels rather than inventing per-player statistics. Keep the headless wire component separate from all UI code. Native review must refocus the actual app because an inactive CUA capture can be stale despite updated AX state.


Native Heist 3v3 completed through the real routed Practice flow: Results retained 0/2000 versus 1765/2000 after worker cleanup, with correct defeat, team panels, health bars, and focused Practice Again. Client launched at 800x700, but the final captured window had a wider aspect, so this is not claimed as final compact-layout evidence. Repeated CUA noWindowsAvailable/cgWindowNotFound errors prevented reliable resize and replay activation; leave compact native and native replay verification pending rather than claiming an attempted action succeeded. Automated existing replay tests and both mode network scenarios pass. No physical-gamepad verification claimed. User visual acceptance remains pending; ticket stays doing. No HUD source modified.


# Compact layout and replay evidence (2026-11-19, via BRL-wqst `results` route)

Commands, run from the worktree on branch worktree-agent-a0d4c8fc0337e7e80 (BRL-wqst commit e1d9deee):

    BRAWLER_SHELL_CAPTURE_GAME_TYPE=hot-zone-3v3 BRAWLER_SHELL_CAPTURE_BIND=127.0.0.1:5101 \
      BRAWLER_SHELL_CAPTURE_DIR=target/wqst/h0yc-hot-zone-3v3 ./scripts/shell-capture.sh results all
    BRAWLER_SHELL_CAPTURE_GAME_TYPE=heist-3v3 BRAWLER_SHELL_CAPTURE_BIND=127.0.0.1:5101 \
      BRAWLER_SHELL_CAPTURE_DIR=target/wqst/h0yc-heist-3v3 ./scripts/shell-capture.sh results all

Each run played a real routed Practice 3v3 match against bots, using the capture catalog (30 s match limit, authoritative end). It saved Results off-screen at 1280x720 and 914x411, sent Practice Again, waited for the replayed match, and left through the match menu back to the Dashboard. All four runs passed.

| Capture | Outcome | Panels |
|---|---|---|
| `target/wqst/h0yc-hot-zone-3v3/1280x720/results.png` | DEFEAT · HOT ZONE | Your team 11.9% (bar ~12%) vs opponents 15.2% (bar ~15%) |
| `target/wqst/h0yc-hot-zone-3v3/914x411/results.png` | VICTORY · HOT ZONE | Your team 19.3% vs opponents 12.9% |
| `target/wqst/h0yc-heist-3v3/1280x720/results.png` | DEFEAT · HEIST (idol destroyed at ~22 s) | Your idol 0 / 2000 (empty bar) vs 1200 / 2000 (60% bar) |
| `target/wqst/h0yc-heist-3v3/914x411/results.png` | DEFEAT · HEIST (time limit) | Your idol 800 / 2000 (40% bar) vs 2000 / 2000 (full bar) |

What I checked in each PNG:
- The final capture percentage uses one decimal place.
- Idol health shows exact current/maximum values, and each bar matches its value.
- The outcome agrees with the values: lower progress or lower idol health loses, and a destroyed idol loses.
- YOUR TEAM is first and highlighted, and OPPONENTS is second.
- PRACTICE AGAIN is focused, with DASHBOARD beside it.

At 914x411 the two objective cards stay side by side (the compact sizing applies, stacked does not) and fit with both actions visible. I found no clear layout defect in h0yc's scope, so there is no code change.

Replay: Practice Again from Results started a new match (`ClientFlow::Match`) in all four runs (`target/wqst/h0yc-*/<size>.run/results.log`), and the leave path returned to the Dashboard. Dashboard navigation from Results was not separately automated here; existing flow tests cover it.

Verification on that commit: `just lint` passes, and `just test` passes, including network 132/132.

Still needs the owner:
- Visual acceptance of the objective panels.
- At 914x411 the shared Results shell scrolls to keep the focused action visible, which pushes the trophy icon almost fully off the top. The objective cards are taller than the Wipeout rows, so this is more visible here than on Wipeout, where it is partly clipped too. This is shared-shell behavior and I didn't change it. The owner should decide whether compact objective cards should shrink or the header should compress.
- Reward-burst motion. Captures are taken after the settle, not during the burst.
- Physical gamepad and touch focus on Results.
