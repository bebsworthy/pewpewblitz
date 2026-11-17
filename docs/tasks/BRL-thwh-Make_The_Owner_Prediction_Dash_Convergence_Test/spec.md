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

# Findings and fix (2026-11-17)

## Root cause
Lightyear 0.29 measures ping RTT/jitter with host `Instant::now()` (`lightyear_sync` `PingPlugin::receive`/`send`),
not with the harness's `TimeUpdateStrategy::ManualInstant` clock. Everything else the harness drives is virtual.
Those wall-clock values feed the client sync objective (`remote + RTT/2 + 4*jitter + 1 tick + jitter margin + error
margin`), the remote-tick estimate, input-delay selection, and reliable resend/NACK timing. So the client's lead and its
time-dilation corrections depended on how long each harness step took on the host.

Evidence (temporary per-frame probe in `step_tracking`, `prediction::dash_converges_without_snapping`, delay=7):
- Unloaded: RTT 9.3 ms, jitter 0.15 ms (the modeled round trip is about 8 ticks, ~133 ms). Passes, largest step 24.0.
- Under 14 parallel `yes` processes: RTT 12-18 ms, jitter 0.5-3.7 ms. Failed 4 of 25 runs. Failing runs show
  `predicted_ticks 102 / followed 16` instead of `103 / 17`, and a 44.7 u frame (two dash ticks presented in one frame)
  right after two short frames (3.2, 2.7). So the sync controller shifted the client by one tick mid-dash.
  This is not followed-authority stepping in prediction code, so BRL-qfby's product code is not involved.
- `local_attack::server_rejected_ultimate_*` has the same source: rejection latency depends on the client lead.

## Fix (test harness only)
`tests/network/harness.rs`: `pin_link_latency` runs in every harness app (lobby, routed match, mode harness servers and
every client) in `PreUpdate` after `lightyear_sync::ping::plugin::PingSystems::Receive`. It pins
`PingManager.rtt_estimator_ewma.final_stats` and `Link.stats` to `HARNESS_LINK_RTT` = 1 ms with zero jitter. Added the
dev-dependency `lightyear_sync = "=0.29.0"` because the `lightyear` facade does not re-export `PingSystems`.

How the value was chosen (sweep):
- 0 ms breaks 4 impairment tests. A zero RTT gives reliable channels a zero resend delay, which Lightyear treats as
  "never resend" (`send_reliable.rs:147`).
- 1-8 ms passes everything.
- 12 ms and 16.7 ms deterministically fail `movement::late_join_receives_current_poses_without_duplicating_static_arena`
  and `loadouts::direct_admission_resolves_distinct_weapon_bases_and_spawns_expected_deliveries`.
- 1 ms is a loopback-scale value at the low end of the unloaded host measurements the suite was tuned against.

## Verification
- Dash under load, 30 runs: bit-identical per-frame trace; largest step 24.014 against a bound of 32.
- Rejected ultimate under load: 13 frames in all 15 runs for both profiles (bounds 20/26).
- 10 consecutive `just _test-network` runs: 128/128 each. Plus 3 more under 14-process CPU load: 128/128 each.

## Known limitation / follow-up
`movement::late_join...` and `loadouts::direct_admission...` are tick-exact. The client input timeline only syncs at
server tick 30 (three-ping handshake), and movement starts on the last of the test's 24 steps, so one extra tick of lead
breaks them. Under the old wall-clock RTT (12+ ms under load) they were latent flakes from the same source. They are now
deterministic, but they would be sturdier if they waited for the input timeline to sync instead of counting steps.
