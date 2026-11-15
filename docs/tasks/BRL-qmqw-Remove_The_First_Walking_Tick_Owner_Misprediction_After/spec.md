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
