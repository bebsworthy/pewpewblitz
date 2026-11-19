# Problem

`scripts/network-product-match.sh` defaults `BRAWLER_ROUTED_TIMEOUT_SECONDS` to 60 seconds, while the current `wipeout-1v1` configuration has a 180-second authoritative match limit and ten-elimination target. The requeue smoke can reach Active cleanly yet be killed by its watchdog before a legitimate terminal result. On 2026-08-28 the default and 90-second runs timed out; the identical run with a 220-second watchdog completed at about 187 seconds and passed fresh-lobby requeue.

# Specification

- Make the requeue smoke's default completion bound derive from or explicitly cover the current authoritative game duration plus bounded routing/cleanup margin.
- Prefer a deterministic faster terminal trigger if one already exists in production verification paths and preserves the real Results-to-requeue lifecycle; do not weaken match authority or fake the client-side result.
- Keep normal product Active smokes fast and retain explicit environment overrides.
- Emit enough phase evidence to distinguish failure to complete, failure to return to lobby, and failure to obtain the new Joined outcome.

# Acceptance criteria

- The documented default requeue command passes without a timeout override.
- The smoke observes an authoritative terminal result, match-worker cleanup, fresh lobby authentication, and a new queue Joined outcome for both clients.
- The watchdog remains bounded and reports the failed lifecycle phase on timeout.
- Normal 1v1 Active and Practice routed smokes remain green.
- Focused script/config tests plus relevant routing, client, server, and network tests pass.
- Verification and learn-from-errors evidence are recorded before completion.

# Non-goals

- No gameplay balance, match-duration, protocol, queue, or result-authority change solely to shorten the smoke.

## Re-check on current code (2026-11-19)

**Finding: the defect still reproduces.** `wipeout-1v1` is still 180 s / 10 kills in
`config/server/game-types.ron`, and the script still defaulted to a 60 s watchdog.

- Baseline, unchanged script: `BRAWLER_ROUTED_BIND=127.0.0.1:5071 BRAWLER_SERVER_DATA_DIR=target/xbq8/requeue-before BRAWLER_PRODUCT_PLAYERS_PER_TEAM=1 BRAWLER_PRODUCT_CLIENT_COUNT=2 BRAWLER_PRODUCT_REQUEUE_SMOKE=1 ./scripts/network-product-match.sh`
  printed `timed out after 60s` while the match was live. The server's combat telemetry showed
  122 shots and 0 hostile hits: the scripted clients aim along a fixed axis and never score, so the
  match can only end at its time limit.

## Decision

I looked for a faster production terminal path. The only deterministic one is the forfeit used by
the completed-rejoin smoke, and it makes one client leave the match rather than reach Results and
press Queue Again. That would not exercise the Results-to-requeue lifecycle for both clients, so I
chose the derived watchdog. No gameplay, duration, protocol or queue behavior changed.

## Fix

- `scripts/e2e-config.sh`: `e2e_requeue_timeout_seconds` reads from `game-types.ron` and computes
  `grant_deadline + loading_deadline + countdown + match_duration + 60 s` routing/cleanup margin.
  For `wipeout-1v1` that is 10 + 30 + 3 + 180 + 60 = 283 s. It fails closed for an unknown game type
  or incomplete timing.
- `scripts/network-product-match.sh`, requeue smoke:
  - The game type defaults to `wipeout-1v1` and is passed explicitly with `--product-game-type`, so
    the watchdog and the queued game always agree. `BRAWLER_PRODUCT_GAME_TYPE` overrides it.
  - `BRAWLER_ROUTED_TIMEOUT_SECONDS` still overrides the watchdog. The default for non-requeue
    paths stays 60 s, so Active smokes stay fast.
  - The script prints how the watchdog was derived.
  - Each client log and the supervisor log go to `target/e2e/requeue.XXXXXX/`.
- Phase evidence: on timeout, on client failure and on success, the script prints each client's
  furthest phase:
  - "never entered a match"
  - "entered the match but never saw it complete"
  - "saw the match complete but never returned to a fresh lobby"
  - "returned to a fresh lobby but got no new queue Joined"
  - "received a new queue Joined"

  It also prints the supervisor's match-result and worker-reap counts. Success additionally requires
  a supervisor `worker result-received` line.
- README documents the default requeue command. `scripts/check-e2e-config.sh` (in `just lint`)
  asserts that the derived watchdog exceeds countdown + match duration and that an unknown game type
  fails.

## Verification

- Default command, no override: `BRAWLER_ROUTED_BIND=127.0.0.1:5072 BRAWLER_SERVER_DATA_DIR=target/xbq8/requeue-after BRAWLER_PRODUCT_PLAYERS_PER_TEAM=1 BRAWLER_PRODUCT_REQUEUE_SMOKE=1 ./scripts/network-product-match.sh`
  exited 0 against the 283 s watchdog.
  - The supervisor received the result at about 186.3 s, and the worker was reaped with code 0 and cleaned.
  - Both clients logged authoritative match completion, a fresh lobby session with lobby accept, and
    a fresh queue Join.
  - Output ended: `Results -> Queue Again received a fresh Joined outcome`.
- Override plus phase evidence: the same command with `BRAWLER_ROUTED_TIMEOUT_SECONDS=45` exited 124
  with `timed out after 45s`. Both clients reported `entered the match but never saw it complete`,
  and the supervisor reported 0 results and 0 reaps.
- The "never returned to lobby" and "no new Joined" classifications were checked by running the
  phase awk on the successful run's real client log, truncated before `starting fresh lobby session`
  and before `fresh queue Join`.
- Normal 1v1 Active smoke: `./scripts/e2e.sh 2` (port 5074) passed. `e2e.sh 4` and `e2e.sh 6` also passed.
- Not re-run in this pass: the Practice routed smoke, and cargo test suites (no Rust changed).
- `just lint` (with ripgrep on PATH): exit 0, including the new `_e2e-config` check. Commit c5a6821f on branch worktree-agent-a484ae02404e5df00 (not pushed).

## Learning

The first phase marker, the match route-grant log, is not emitted in the headless log. The first
timeout run therefore misreported "never entered a match". I switched the marker to the match
admission line (`brawler client accepted`) and re-ran to confirm. Prevention: verify every log
marker a script greps against a real captured log, not against source strings.
