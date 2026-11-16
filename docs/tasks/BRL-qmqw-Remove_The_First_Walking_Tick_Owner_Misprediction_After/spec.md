Reviewed 2026-11-15 against HEAD 3437c0d8: still valid, not started; no fix landed since a5678e12 filed it. Refs, a test plan and a clearer cause hypothesis added. The original trace evidence path (scratchpad/brl0169-vibrate) is no longer present, so reproduce first.

# Outcome
The local fighter's first step after a match starts doesn't snap backward.

# Context
BRL-sh10 (BRL-0169, commits eba755e9 / 08b7f789) found the cause in native traces. Every Practice match has exactly one owner reconciliation correction, on the first walking tick after start: 2.3–4.7 u, behind the fighter. It is the existing late-arriving-input misprediction: the server consumes a tick for which the walking input has not arrived and repeats the previous (idle) input, so authority lags the prediction by one or more ticks of motion.
- Steady walking already tolerates a one-tick repeat (about 1.2 u): tests/network/prediction.rs:219-223 asserts `max_smoothed_error < 2.0`. The first-walk case is larger (2–4 ticks of motion), which points at match-start input timing (inputs held before `MatchPhase::Active` are not predicted, joined_harness comment at prediction.rs:38) rather than steady-state jitter.
- Input config: `InputPlugin::<FighterInput>` with `send_interval: SIMULATION_TICK`, `packet_redundancy: 5` (src/protocol.rs:746-752). Reconciliation: `OwnerPrediction::reconcile` (packages/brawler-client/src/prediction.rs:205) / `reconcile_owner_prediction` (prediction.rs:424); counters in `OwnerPredictionStats` (prediction.rs:306-316).
- Existing tests miss it: `joined_harness` (prediction.rs:26-66) idles for `30 + delay` ticks after activation before any test walks.

# Plan
1. Reproduce: a native Practice match with owner-prediction tracing (BRL-0169 method), and in the harness a test that starts walking on the first ticks after `MatchPhase::Active` and records `stats.max_smoothed_error`.
2. Find which tick the server first applies the walk input vs the client's predicted tick (match-start tick alignment, `simulation_tick_at` / `observe_server_clock`, prediction.rs:174-200, or server-side handling of inactive→active fighters).
3. Fix the cause (align the client's prediction start with the tick the server begins applying inputs, or have the server not repeat a stale pre-activation input), not the smoothing threshold.

# Acceptance criteria
- The first walk after match start reconciles with no visible correction (offset < 0.1 u).
- A regression test in tests/network/prediction.rs that walks immediately after activation and asserts `max_smoothed_error < 0.1` and `snaps == 0` at delays from `PROFILES`.
- `just test`, `just lint`, `just practice-e2e` pass.


# Resolution 2026-11-16
**Root cause:** the server writes no `AuthoritativePose` while a fighter is inactive. On the activation tick (`starts_at_tick`) it applies the held walk input and writes its first pose. The client attaches `OwnerPrediction` only when that pose arrives, about a round trip later. With no prediction history, `reconcile` rebased the sample's position onto the current local tick and dropped the server's motion for the ticks in between. The next sample then corrected 2-4 ticks of motion (3.5 u at delay 4). This was not a tick-alignment bug and not the server repeating a stale input.

**Fix (client only, `packages/brawler-client/src/prediction.rs`):** `OwnerPrediction::reconcile_replaying` handles a sample with no matching prediction (the first one after attach, or a history overrun). It rebases onto the sample, then re-simulates every local input tick between the sample and the current tick, using the Lightyear input buffer (`NativeBuffer<FighterInput>`) and the same movement step as prediction (the new `OwnerStepWorld` / `OwnerStepState`). For the first sample, only the attach pose versus authority counts as a correction. No protocol or server change.

**Tests:**
- `tests/network/prediction.rs` `first_walk_after_match_start_reconciles_without_correction` uses the new `activated_harness`. It covers walk held through activation, pressed on activation, and pressed 2 ticks after activation, at delays 0, 4 and 7.
  - Before the fix: max error 2.33-3.5 u. After: 0.000.
  - The "pressed 2 ticks after" case keeps the existing one-tick 1.167 u idle-to-walk error at delay > 0. That is the same tolerance steady walking already has (< 2.0).
- Unit test `the_first_sample_replays_the_inputs_the_server_already_applied`.
- `just lint`, `just test` (2458 tests) and `just practice-e2e` pass.

**Native check:** a headless routed client walked from the start (`--move-axis 1,0 --product-network-change-smoke`, wipeout-1v1) against the local supervisor, with a temporary probe logging every reconciliation error above 0.01 u.
- HEAD baseline: `Smoothed { error: 3.50 }` at the first walking sample.
- With the fix: 4 of 4 runs had no correction at match start. The only logged snaps (about 262 u) follow the smoke's deliberate 4 s traffic outage, as on baseline.

**Possible follow-ups (not filed):**
- If the `MatchState` change to Active arrives after the fighter's first pose, the replay treats the ticks as suspended.
- Respawn has the same late start, but the respawn teleport snaps it anyway.
