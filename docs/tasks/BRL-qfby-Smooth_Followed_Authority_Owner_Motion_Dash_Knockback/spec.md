Reviewed 2026-11-15 against HEAD 3437c0d8: still valid, not started; paths moved to packages/brawler-client (BRL-0170), refs added. Note the dash path data is server-only, so smoothing must use replicated state.

# Context
BRL-sh10 (commit 08b7f789, BRL-0169) added partial-tick extrapolation of the owner's newest predicted step between fixed ticks:
`OwnerPrediction::presented_position_at` (packages/brawler-client/src/prediction.rs:146-164), applied at prediction.rs:613.
While local movement is suspended (`OwnerPrediction::is_following_authority`, prediction.rs:189: dash, defeat,
inactive participant) `record_step` sets `step_velocity = Vec2::ZERO` (prediction.rs:279-283) by design, because the motion
arrives from reconciliation rather than local integration. A dash moves about 20 world units per tick, so on
a 120 Hz display the owner (and camera) still visibly step during the dash.

What the client has during a dash:
- Replicated `AbilityState { phase: AbilityPhase::Dashing { ends_at_tick } }` (src/builds/model.rs:454-460).
- Not replicated: `DashRuntime { origin, endpoint, started_at_tick, .. }` is `#[cfg(feature = "server")]` (src/abilities/dash.rs:84-93); the dash path is a lerp from origin to endpoint (dash.rs ~80).
- Confirmed authoritative poses via reconciliation (`AuthoritativePose`).

Render-report sampler: `sample_owner_motion` / `OwnerMotionSamples::sample` (packages/brawler-client/src/presentation_3d/diagnostics.rs:186-215, 408-432)
reads `prediction.step_velocity()`; frames with expected speed < `OWNER_MOVING_SPEED` (diagnostics.rs:181, :212) are not counted as moving, so dash frames are excluded today.

# Scope
- Measure the dash stepping with the BRL-sh10 render-report owner-motion fields (schema=6, `owner_moving_frames` … `owner_correction_max`, diagnostics.rs:518): extend the sampler to count followed motion (e.g. a separate `owner_followed_frames` / followed speed ratio fed by presented displacement while `is_following_authority`), bumping the report schema.
- If visible, smooth followed motion without delaying it, e.g. extrapolate from the two newest confirmed followed poses (velocity = delta / tick) clamped to `ends_at_tick`, or interpolate between the two newest confirmed followed poses and record the latency cost. Zero the extrapolation once `ends_at_tick` passes so dash end does not overshoot.

# Out of scope
Prediction/reconciliation model, server authority, or protocol changes (do not replicate DashRuntime).

# Plan
1. Add followed-motion counters to `OwnerMotionSamples` + unit test beside `owner_motion_separates_tick_stepping_from_smooth_frames` (diagnostics.rs:642).
2. Native >60 Hz baseline: `just v3-render-evidence` (scripts/v3-render-evidence.sh) on a 120 Hz display with a dash, record before numbers.
3. Implement a followed velocity in `OwnerPrediction` (set in reconciliation when following authority), used by `presented_position_at`; unit tests in packages/brawler-client/src/prediction/tests.rs (~line 359-400 pattern).
4. Re-measure, record after numbers in the ticket.

# Acceptance
- Native >60 Hz before/after measurement of dash frames recorded.
- No snap or overshoot at dash end beyond the existing correction smoothing; `prediction::dash_converges_without_snapping` (tests/network/prediction.rs:323) stays green (see BRL-thwh for its flake).
- `just test`, `just lint` pass.


# Implementation record (2026-11-18)

## Research
- Dash path (src/abilities/dash.rs): `activate_dash` sets `ends_at_tick = tick + duration_ticks`; each
  server tick moves to `lerp(origin, endpoint, (t - started_at + 1) / duration)` and settles the
  phase on the tick where `t + 1 >= ends_at_tick`. So the path is linear and the final step (onto the
  endpoint) runs at simulation tick `ends_at_tick - 1`; the replicated phase is already settled in
  that final sample. Default build: Dash, 360 u over 18 ticks = 20 u/tick (1200 u/s).
- Knockback is not followed: it is predicted through `ExternalMotion` (`movement_is_suspended` is only
  dash, defeat, inactive participant), so it already uses the BRL-sh10 step extrapolation. Only the
  dash moves while following authority.
- While following, `record_step` leaves `position` unchanged and every confirmed sample moves it by
  the reconciliation error. At dash start that error is the dash step minus one leftover walking
  step the client predicted before it learned of the dash (one is withdrawn per sample), so the
  presented per-tick motion is not the raw sample-to-sample delta.
- Render evidence: `BRAWLER_RENDER_COMBAT=1` (1v1, aim 1,0) never charged the ultimate in the window
  (0 followed frames). The native (Fifo) profile ran at 60 Hz on this machine this session (p50
  16.66 ms, built-in ProMotion display), so >60 Hz evidence uses `BRAWLER_RENDER_PROFILE=high-refresh`
  (AutoNoVsync), as BRL-sh10 did; it ran at 120 Hz (p50 8.1-8.3 ms) and in early runs ~240 Hz
  (p50 4.1 ms).

## Decisions
- Followed velocity = per-tick displacement the newest confirmed sample folded into the prediction
  (`position` after fold minus before, divided by the Lightyear tick gap to the previous sample,
  gap 1..=4), not the raw sample delta: extrapolating the raw delta overshot by the leftover walking
  step and pulled back each tick at dash start (diagnosed with temporary per-tick logging).
  Snaps (respawn) contribute zero.
- Extrapolation is `followed_step * min(ticks_since_fold + overstep/fixed_step, 1, remaining)`, with
  `remaining = ends_at_tick - 1 - estimated sample simulation tick` (`AuthoritativePose` server clock).
  Only while following with a replicated `Dashing { ends_at_tick }`; defeat/inactive never extrapolate.
  Resolved against the same replicated map blockers as BRL-sh10.
- At most one tick ahead: a first version allowed two ticks to bridge late samples; native runs then
  showed 33 followed pull-backs (max 15.7 u) because the next sample often arrived alone (stream
  shifted a tick). With one tick a late sample holds at the next confirmed pose (a short stall) and
  never pulls back.
- No protocol, replication, or server change; `DashRuntime` stays server-only. No added latency: the
  newest confirmed pose is shown at the tick boundary exactly as before, the extrapolation only adds
  motion between ticks.
- Report schema 6 -> 7, new fields `owner_followed_frames`, `owner_followed_stalled_frames`,
  `owner_followed_ratio_p05/p50/p95` (presented speed / followed velocity on frames following moving
  authority), `owner_followed_pullback_frames`, `owner_followed_pullback_max` (pull-backs on a frame
  following moving authority or the frame right after, i.e. an end-of-dash overshoot). Owner-motion
  report lines moved into `OwnerMotionSamples::report_fields` (compose_report hit too_many_lines).
  Note: BRL-rr1x's spec also plans a 6 -> 7 bump; whichever lands second takes 8.
- `scripts/v3-render-evidence.sh`: the supervisor now gets `--data-directory <report>.server-data`
  (it previously wrote `data/server/`), and `BRAWLER_RENDER_COMBAT=dummy` runs the measured client
  with `--aim-dummy --fire --ultimate`, chasing and shooting the Practice bot so the default Dash
  charges and fires repeatedly.

## Measurements
Command (reports in `target/qfby/`):
`BRAWLER_RENDER_PROFILE=high-refresh BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_COMBAT=dummy
BRAWLER_RENDER_BIND=127.0.0.1:5041 BRAWLER_RENDER_WARMUP_SECONDS=5 BRAWLER_RENDER_MEASURE_SECONDS=30
BRAWLER_RENDER_TIMEOUT_SECONDS=120 ./scripts/v3-render-evidence.sh target/qfby/<run>.txt`
MacBook Pro built-in Liquid Retina XDR, release client, Practice wipeout. "Before" runs use the
sampler build with followed extrapolation disabled (before-3: no extrapolation code yet; before-4/5:
temporary switch returning no followed travel, since removed).

| run | frame p50 | followed frames | stalled | ratio p05 / p50 / p95 | followed pull-backs (max) |
|---|---|---|---|---|---|
| before-3 | 4.16 ms | 340 | 157 (46 %) | 0.012 / 0.057 / 4.461 | n/a |
| before-4 | 4.14 ms | 356 | 161 (45 %) | 0.013 / 0.056 / 4.150 | 4 (4.46 u) |
| before-5 | 8.09 ms | 260 | 89 (34 %) | 0.014 / 0.101 / 4.051 | 3 (3.80 u) |
| after, 2-tick max (rejected) | 8.25 ms | 261 | 2 | 0.933 / 1.033 / 2.800 | 33 (15.70 u) |
| after-5 (final) | 8.27 ms | 267 | 18 (7 %) | 0.030 / 1.041 / 2.257 | 4 (25.17 u) |
| after-6 (final) | 8.26 ms | 188 | 16 (9 %) | 0.020 / 1.018 / 2.935 | 3 (12.23 u) |
| diag-1 (final + logging) | - | 182 | 17 (9 %) | 0.025 / 1.023 / 3.042 | 2 (15.96 u) |
| final-after-1 (final, cleaned) | 8.26 ms | 262 | 19 (7 %) | 0.045 / 1.031 / 2.925 | 4 (12.96 u) |

All runs `result=pass`. Walking (`owner_*` non-followed) fields unchanged (0 stalled, p50 1.000).
Remaining followed stalls are late samples held one tick (followed by one catch-up frame, the p95
~2-3); the remaining followed pull-backs were traced in diag-1: the 15.96 u one is the first frame
of a dash aimed opposite the previous walking direction (genuine reversal), and the end-of-dash one
(3.67 u) is walking back toward the bot after the dash passed it; no frame presented past the dash
endpoint. The pull-back counter cannot separate reversals from overshoot, so counts stay noisy
across runs (bot geometry differs per run).

## Verification
- Unit tests: `prediction::tests::followed_dash_presents_authoritative_motion_between_ticks_without_overshoot`,
  `followed_motion_without_a_dash_end_or_after_a_snap_is_not_extrapolated`,
  `followed_velocity_includes_withdrawn_walking_steps_so_extrapolation_never_pulls_back`;
  `presentation_3d::diagnostics::tests::owner_motion_counts_followed_dash_frames_separately`.
- `just lint` (VS Code ripgrep on PATH): pass. `just test`: pass (2494 passed, 0 failed).
- `cargo test -p brawler-client --features network-test --test network prediction` x5: 9/9 pass
  every run, including `prediction::dash_converges_without_snapping`.
- Native render evidence above (all reports `result=pass`). Note: the before runs' expected
  followed velocity came from the raw confirmed-sample delta; the final sampler uses the fold
  displacement (differs only at dash start by one walking step), so ratios are comparable.

## Durable docs
- docs/08-network-architecture.md owner-prediction paragraph: followed dash extrapolation.

## Learning
- Mistake: first extrapolated the raw confirmed-sample delta and bridged late samples for two ticks;
  both produced pull-backs only visible in native runs. Cause: while following, the presented pose
  moves by the fold displacement (includes withdrawn walking prediction), and late samples often
  mean the stream shifted rather than a burst follows. Prevention: extrapolate exactly what the
  presentation moved by, never more than one tick, and keep a pull-back counter scoped to the
  motion being changed in the render report.

## Deferred / limits
- Followed pull-back counter includes genuine direction reversals; a dash interrupted out of bounds
  (rare) can still present up to one tick past the stop and then pull back.
- Native Fifo (true display-refresh) >60 Hz evidence was not available: the native profile ran at
  60 Hz this session; numbers are from the high-refresh (AutoNoVsync) profile at 120/240 Hz.
- Playtest: `just server` + `BRAWLER_RENDER_PROFILE=high-refresh just client` (or native on a 120 Hz
  ProMotion display), Practice match, charge the Dash on a bot and dash repeatedly in different
  directions; observe the fighter and camera glide during the dash instead of stepping, with no snap
  back at the dash end.
