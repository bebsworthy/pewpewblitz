Reviewed 2026-11-15 against HEAD 3437c0d8: still valid, not started; line refs moved (test now prediction.rs:323, assertion :300-303) and harness/threshold facts added.

# Context
Found during BRL-rz4s verification (2026-11-14). The first `just test` run failed
`prediction::dash_converges_without_snapping` (tests/network/prediction.rs:323; failing assertion in
`assert_converged_without_teleport`, prediction.rs:300-303, "dash: no visible teleport"; reported largest frame
step 40.755). It passed 3/3 in isolation and in the immediately repeated full `just test`. BRL-rz4s did not
touch prediction or movement code. Its harness change, skipping suspended clients in `Harness::step`
(tests/network/harness.rs:710-725), leaves non-suspended clients unaffected.

Facts:
- Threshold: `snap_distance / 2` = 2 cells × 32 u / 2 = 32 u (packages/brawler-client/src/prediction.rs:56,
  `MAP_CELL_SIZE_WORLD` src/map/catalog.rs:17). A dash is about 20 u/tick, so 40.755 is roughly two ticks of
  dash travel landing in one presented frame (a doubled/skipped reconciliation step, not a large error).
- The harness is a Crossbeam transport with a deterministic delay line (harness.rs:1-45) and
  `TimeUpdateStrategy::ManualInstant(self.now)`, but `now` starts at `Instant::now()` (harness.rs:360/396/602),
  and the network suite runs serially: `cargo test -p brawler-client --features network-test --test network
  -- --test-threads=1` (justfile:175-176), so ordering is fixed; a residual wall-clock read (Lightyear sync/ping,
  `Time<Real>`, timers) or startup timing is the most likely source.
- The test sets `AbilityState { charge: 1000, Ready }`, steps 10 ticks, presses ULTIMATE for 3 ticks, then walks
  40 ticks (prediction.rs:323-353). Whether the dash starts on the same tick relative to replication depends on
  when the server consumes the ULTIMATE input.

# Scope
- Reproduce (loop the full network suite, or the test under CPU load, e.g. `for i in $(seq 20); do just _test-network || break; done`)
  and find the nondeterminism (wall-clock dependence, test ordering, or a threshold too tight for the dash step).
- Instrument the failing run: per-frame presented pose, `OwnerPredictionStats` (followed/smoothed counts) and the
  server dash start tick.
- Fix the cause; do not just loosen the assertion without evidence. If the cause is followed-authority stepping
  (two dash ticks in one reconcile), coordinate with BRL-qfby.

# Plan
1. Loop to reproduce; capture the println output of the failing case.
2. Identify the wall-clock / timing input; make the harness fully deterministic or make the test independent of it.
3. Fix and loop again.

# Acceptance criteria
- The test passes in repeated full-suite runs (for example 10 consecutive `just _test-network` runs, plus one full `just test`).
- `just lint` passes.


# Related flake (2026-11-16)
In one full `just test` during BRL-2nsg, `local_attack::server_rejected_ultimate_is_rejected_locally_and_authority_is_unchanged` needed 21 frames against its bound of 20. It passed in isolation and on the rerun. This is likely the same wall-clock timing source as this ticket; check both when fixing.
