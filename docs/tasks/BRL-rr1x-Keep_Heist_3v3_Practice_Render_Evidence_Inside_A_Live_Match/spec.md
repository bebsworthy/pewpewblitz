Reviewed 2026-11-15 against HEAD 3437c0d8: partial — the 100 ms gate failure no longer reproduces, but Heist Practice runs still measure across match end and lobby return; spec updated.

# Observation (BRL-m10f, 2026-11-12)

`BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_MODE=heist BRAWLER_RENDER_PLAYERS_PER_TEAM=3 BRAWLER_RENDER_COMBAT=1 just v3-render-evidence <report>` failed `frames_over_100_ms` on the unchanged baseline (commit 4c82ec64) as well as with BRL-m10f. The client log showed the Practice match completing during the measurement window ("observed authoritative match completion; returning to lobby"), so the window included the lobby-transition hitch. p50/p95 were otherwise ~8.3/8.5 ms.

# Current state (2026-11-15)

- Later Heist Practice runs (BRL-0178 / BRL-0180 scratch evidence, 2026-11-14) pass `frames_over_100_ms=0`, but the match still ends 18-30 s after match-ready, inside the default 10 s warmup + 30 s window. The client then keeps sampling through a fresh lobby session until the window closes. The report still says `measurement_context=gameplay` and `result=pass`.
- This is by design today: `packages/brawler-client/src/presentation_3d/diagnostics.rs:321-324` keeps the original readiness anchor so the report "covers the product match and its routed return-to-lobby lifecycle". Nothing records or rejects a window that left the match.
- Heist 3v3 is `safe_health: 2000, match_duration_seconds: 180` (`config/server/game-types.ron:31`). Practice bots plus the measured client's `--fire --ultimate` (combat smoke, `scripts/v3-render-evidence.sh:59-62`) end the match by safe destruction well before 40 s.
- The routed phase is observable: `RoutedClientLifecycle.phase` / `RoutedClientPhase::Match` (`session/admission.rs:670`; the match-completion log is `session/routing.rs:205-207`).

# Plan

1. In `diagnostics.rs` `sample_and_finalize_render_measurement` (line 295), read `Option<Res<RoutedClientLifecycle>>`. Once the context is `Gameplay` and warmup has started, record `left_match_during_measurement=true` if the phase leaves `RoutedClientPhase::Match` before warmup + measurement ends.
2. Add the field to `REPORT_FIELDS` (line 23) and `compose_report` (line 480; bump `schema=6` to 7, or share one bump with BRL-89kq). Make `render_measurement_failure` (line 458) return `left_match_during_measurement` as `first_failure`, so the run fails with a clear reason instead of silently measuring the lobby. Optionally finalize early instead of failing, with `measurement_seconds` recording the actual span.
3. In `scripts/v3-render-evidence.sh`, keep Heist Practice in the match. Pick the smallest fix that holds: drop `--ultimate` and `--fire` on the safe for Heist, or default Heist Practice to a shorter `BRAWLER_RENDER_MEASURE_SECONDS` (for example warmup 5 and measure 12). Document the choice next to the mode defaults (lines 80-91).
4. Record one passing Heist 3v3 Practice combat run.

# Acceptance

- A unit test in `diagnostics.rs` tests: a report composed with the match-left flag fails with `first_failure=left_match_during_measurement`, and `validate_report` accepts the new field set.
- `BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_MODE=heist BRAWLER_RENDER_PLAYERS_PER_TEAM=3 BRAWLER_RENDER_COMBAT=1 just v3-render-evidence target/rr1x/heist.txt` passes. Its client log has no "returning to lobby" line before "V3 render report written", and the report has `left_match_during_measurement=false`.
- The default `just v3-render-evidence` (Wipeout 1v1) still passes; `just lint` and `just test` pass.
- Run beside the dev server only after BRL-tpd8 lands, or with a scratch copy that passes `--data-directory`, because the script still writes `data/server/`.


# Result (2026-11-18)

## Changes

- `packages/brawler-client/src/presentation_3d/diagnostics.rs`: render report schema 7 -> 8 with new field `left_match_during_measurement` (after `measurement_context`). A `MatchWindow` on `RenderMeasurementState` observes `RoutedClientLifecycle` each frame once the context is `Gameplay`: it keeps the first `RoutedClientPhase::Match` generation and marks the window left when the phase leaves `Match` or the match generation changes. Non-routed or dashboard windows never mark it. `render_measurement_failure` checks it first, so `first_failure=left_match_during_measurement`, `result=fail`. The window still closes on its original anchor (no early finalize). New tests: `match_window_marks_leaving_the_first_routed_match`, `report_fails_a_window_that_left_the_match` (composes stayed/left reports, both pass `validate_report`, schema 8). Nothing else reads the schema number: no script, doc, or tool checked `schema=7` (searched with rg).
- `scripts/v3-render-evidence.sh`: Heist Practice now defaults to warmup 4 s + measurement 11 s (15 s). Other modes keep 10 + 30. Explicit `BRAWLER_RENDER_WARMUP_SECONDS` / `BRAWLER_RENDER_MEASURE_SECONDS` still override. The reasoning is in a comment beside the mode defaults.

## Why the short window and not a different combat input

Match-end timing after match-ready, Heist 3v3 Practice, default 40 s window (target/rr1x/heist-*-probe, heist-aimwall-*):
- combat=1 (`--aim-axis 1,0 --fire --ultimate`, baseline): ended at 23 s. The new field caught it: `left_match_during_measurement=true`, `first_failure=left_match_during_measurement` (heist-baseline.txt).
- `--fire` without `--ultimate`: ended at about 21 s.
- aim into the back wall (`0,-1`): 1 of 3 runs lasted 40 s+. The other two ended at 21 s and 18 s.
- combat=0: lasted 40 s+ (1 run). combat=dummy: lasted 40 s+ in 2 runs, but in one the enemy safe was at 460/2000 by about 22 s.
- Traced captures (heist-trace-probe) show the measured client parks against the right wall. The Practice bots then decide the match by breaking a safe. Changing the measured client's input does not reliably keep the match alive past 40 s.
- So the window has to end before the earliest observed end (18 s). 4 + 11 = 15 s gives 1,320 samples at 120 Hz against the 1,200-sample gate.

## Commands and results (all with BRAWLER_RENDER_BIND=127.0.0.1:5032)

- `BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_MODE=heist BRAWLER_RENDER_PLAYERS_PER_TEAM=3 BRAWLER_RENDER_COMBAT=1 just v3-render-evidence target/rr1x/heist{,-2,-3}.txt`: 3/3 pass. Each has `left_match_during_measurement=false`, sample_count 1321, p95 about 8.55 ms, p99 about 8.68 ms, frames_over_100_ms=0. No "returning to lobby" line in any client log. The report was written about 15.0 s after match-ready.
- `just v3-render-evidence target/rr1x/default.txt` (Wipeout 1v1): pass. Both reports are schema 8 with `left_match_during_measurement=false`. Measured p95 17.15 ms at 1801 samples (60 Hz pacing), peer 9.32 ms.
- `just lint`: pass (target/rr1x/lint.log). `just test`: pass (target/rr1x/test.log).
- Captures target/rr1x/heist.txt.captures/brawler-000300.png and -000660.png show the live match: HUD countdown, then 2:59, both safes at 100%, own team at its safe.

## Known limits

- With a 15 s Heist Practice window, the measurement covers the countdown, the approach, and the measured client's own fire. The bots' engagement starts at about 12-15 s, so the bot-vs-bot fight load is barely sampled. Longer Heist combat measurement needs a mode that cannot end early; that was not attempted here.
- On a 60 Hz display, Heist Practice's 11 s window gives about 660 samples and fails `sample_count`. Pass explicit durations there. A match end then fails clearly as `left_match_during_measurement`.
- Margin is about 3 s against the fastest observed end (18 s). If bots get faster, the run fails loudly through the new field instead of silently measuring the lobby.
- `BRAWLER_RENDER_COMBAT=dummy` and Hot Zone Practice keep 10 + 30. The new field fails them if their matches end inside the window.
