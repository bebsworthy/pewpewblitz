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
