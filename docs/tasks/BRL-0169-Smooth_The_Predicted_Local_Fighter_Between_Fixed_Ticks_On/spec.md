# Context
BRL-0141 presents the owner's predicted pose as the latest fixed-tick (60 Hz) prediction plus a decaying correction offset (`src/client/prediction.rs`, `present_owner_prediction`). Remote fighters are interpolated per frame by Lightyear, but the owner pose only changes on frames where a fixed tick ran, so a 120 Hz display can show 60 Hz stepping of the local fighter and camera.

# Scope
- Measure on a high-refresh display whether the stepping is visible.
- If it is, add per-frame presentation smoothing (for example, `Time<Fixed>` overstep extrapolation from the last predicted velocity, or interpolation between the last two predicted ticks) and record its latency cost against BRL-0141's first-frame criterion.

# Out of scope
Changing the prediction or reconciliation model, server authority, or protocol.

# Acceptance criteria
- Native observation on a >60 Hz display recorded before and after.
- Input-to-visible latency remains first frame after the input's fixed tick.


# Research (2026-11-14)
- Owner pose path: `predict_owner_movement` (FixedUpdate, `GameplaySet::Simulation`) records one
  predicted position per local input tick; `present_owner_prediction` (Update, after
  `InterpolationSystems::All`) writes `position + correction_offset` into the owner's `Position`;
  `write_fighter_visual_poses` and `follow_3d_camera` (PostUpdate, before transform propagation)
  read that `Position` every frame. The correction offset also decays only inside the fixed step.
  So on a 120 Hz display the owner fighter and the camera target change on every other frame, and
  with an uncapped present mode most frames repeat the previous pose.
- Client input is sampled into `ActionState<FighterInput>` once per fixed tick
  (`FixedPreUpdate`, `InputSystems::WriteClientInputs`), so between ticks the next intent is not
  known yet.
- Lightyear 0.29 `FrameInterpolationPlugin` / `FrameInterpolate` (book
  `concepts/advanced_replication/visual_interpolation.md`, crate
  `lightyear_frame_interpolation`) renders `lerp(previous, current, overstep_fraction)`, i.e.
  **one fixed tick behind** the simulation, and restores the canonical component before the fixed
  loop. Rejected: (1) the one-tick visual delay contradicts this ticket's "no added input
  latency" and BRL-0141's first-frame criterion (at 60 Hz with frames aligned to ticks the input
  tick's motion would not be visible until the next frame); (2) the owner's `Position` is not a
  Lightyear-`Predicted` component here - it is network-interpolated by Lightyear and overwritten by
  the custom owner prediction in Update, so the Restore/Update/Interpolate pipeline would fight
  the network interpolation writer. The book names the alternative: "simulate an extra partial
  tick for rendering" without committing it to the canonical timeline.
- `bevy_transform_interpolation` 0.5 (Avian's easing, including velocity extrapolation) eases
  the fighter entity's `Transform` from Avian `LinearVelocity`; the owner prediction is not an
  Avian body and the rendered fighter is a separate visual entity whose `Transform` is written
  from `Position` in PostUpdate, so it does not reach the rendered pose. Rejected.
- Bevy 0.19 has no built-in fixed-timestep visual smoothing beyond `Time<Fixed>::overstep()` /
  `overstep_fraction()` (the `physics_in_fixed_timestep` example pattern).
- Pre-existing related high-refresh defect: `write_fighter_visual_poses` decides the run/idle
  animation from `distance_squared > 0.25` per *frame*. That threshold is frame-rate dependent:
  70 u/s is 0.58 u/frame at 120 Hz but 0.49 u/frame at 144 Hz (reads as idle), and with tick
  stepping every other 120 Hz frame is 0 (animation flicker).

# Design decisions
- Present the owner as a bounded partial-tick extrapolation of the newest committed predicted
  step: `presented = position + last_step_velocity * overstep + correction_offset * 0.5^(overstep
  / half_life)`, where `overstep = Time<Fixed>::overstep()` read in Update after the fixed loop.
  `last_step_velocity` is the realized (post-collision) displacement of the newest predicted step
  divided by its fixed delta, so wall slides extrapolate along the wall; it is zero while the
  owner follows authority (dash, defeat, inactive) and before the first step. This is
  presentation only: `OwnerPrediction.position`, history, reconciliation, and authority are
  unchanged; the extrapolated pose is written to the same presented `Position` as today.
- Exactness: at a tick boundary (`overstep == 0`) the presented pose equals today's pose exactly.
- Latency: the first frame after an input's fixed tick shows that tick's full step plus a
  partial step of the same velocity, so no visual delay is added (unlike frame interpolation).
- Known cost: on a release or a new wall contact the extrapolated partial step (at most one tick
  of travel, about 1.2-1.8 world units at 70-110 u/s, under 6 % of a 32-unit cell) is withdrawn on
  the next tick. Overstep is clamped to one fixed step so a stalled fixed loop never extrapolates
  further.
- Fix the frame-rate-dependent run/idle threshold by comparing per-frame displacement against a
  speed (30 u/s, the old 0.5 u/frame at 60 Hz) times the frame delta.
- Diagnostics: add bounded automation-only owner-motion jitter fields to the native render report
  (`--render-report`, schema 4): moving frames, frames whose presented pose did not advance while
  the prediction was moving, and per-frame presented-speed dispersion, to quantify before/after on
  the ProMotion display.


# Implementation (2026-11-14)
- `src/client/prediction.rs`: `OwnerPrediction` records the realized velocity of each predicted
  step (`step_velocity`, zero while following authority); new
  `presented_position_at(overstep, fixed_step, tuning)`; `present_owner_prediction` (Update,
  unchanged ordering after `InterpolationSystems::All`) now reads `Time<Fixed>::overstep()` and
  presents the partial-tick extrapolation. Prediction, history, reconciliation, authority, and
  protocol are unchanged.
- `src/client/presentation_3d/combat/entities.rs`: run/idle uses a 30 u/s speed threshold
  (`FIGHTER_MOVING_SPEED`) instead of 0.5 units per frame.
- `src/client/presentation_3d/diagnostics.rs` (automation only): render report schema 4 adds
  `owner_moving_frames`, `owner_stalled_frames`, `owner_speed_ratio_p05/p50/p95` (per-frame
  presented speed / predicted step speed while the owner's prediction is moving).
- `docs/08-network-architecture.md`: owner-prediction section documents the between-tick
  presentation and why Lightyear frame interpolation is not used.

# Verification (2026-11-14)
- New tests in `src/client/prediction/tests.rs`: pure extrapolation/exact-at-boundary/clamp,
  continuous correction decay across a tick, no extrapolation while following authority; ECS
  schedule tests driving Bevy's fixed loop (`accumulate_overstep`/`discard_overstep`) with
  120/144/90/240 Hz frame deltas: the owner advances on every rendered frame, lies on the
  continuous path within 0.01 u, and equals the committed prediction exactly when a frame lands on
  a tick boundary; a 3-unit in-flight reconciliation blends with no frame step above one frame of
  travel plus decay; releasing input withdraws at most one tick of travel and settles exactly.
  Diagnostics test separates 60 Hz stepping from smooth motion.
- `just lint` (VS Code ripgrep first on PATH): pass. `just test`: pass (2312 passed, 0 failed,
  including `prediction::dash_converges_without_snapping`). `just e2e 2`: pass (1v1 routed roster
  reached Active). Tracked `data/server/` untouched (evidence runs used `--data-directory` in a
  scratch copy of `scripts/v3-render-evidence.sh`).
- Native evidence, MacBook Pro Liquid Retina XDR (ProMotion), release client, Practice smoke
  walking `--move-axis 1,0`, 2 s warmup + 20 s window
  (`BRAWLER_RENDER_PRACTICE=1`; reports in the session scratchpad `brl0169/`):

  | run | frame p50 | moving frames | stalled frames | speed ratio p05/p50/p95 |
  |---|---|---|---|---|
  | before, native (Fifo, 120 Hz) | 8.32 ms | 1851 | 911 (49 %) | 0.000 / 1.039 / 2.179 |
  | before, high-refresh (AutoNoVsync) | 8.21 ms | 2367 | 1250 (53 %) | 0.000 / 0.000 / 2.661 |
  | after, native | 8.33 ms | 1914 | 0 | 1.000 / 1.000 / 1.027 |
  | after, high-refresh | 8.34 ms | 2269 | 0 | 1.000 / 1.000 / 1.003 |

  The first "after, native" run failed the report's frame-time gate (p99 29 ms, 32 frames over
  25 ms) with 0 stalled frames and ratios 1.000/1.000/1.010; the immediate rerun passed (p99
  8.70 ms). Treated as host load noise: the change adds one multiply-add per frame.
- Latency: the frame after an input's fixed tick presents that tick's full step plus the partial
  step (schedule test asserts the continuous path), so BRL-0141's first-frame criterion holds and
  no frame of delay is added.
- Pixel 8a sanity check: APK built and installed (`just android-install 192.168.31.47:5001`),
  app launched, but the phone timed out connecting to the scratch routed server on
  `0.0.0.0:5001` twice although ICMP to the Mac succeeded and the macOS firewall is off; no
  in-match observation. Not caused by this change (connection phase); left for the playtest.

# Playtest request
- Run `just server` and `just client` on a ProMotion Mac (and optionally
  `BRAWLER_RENDER_PROFILE=high-refresh just client`). Enter a Practice match and walk in circles,
  along walls, and stop abruptly; dash; take knockback.
- Observe: the local fighter and camera glide without 60 Hz judder; no visible backward twitch
  on release or wall contact (at most about 1.8 u); run/idle animation does not flicker; no change
  in responsiveness. Optionally repeat on the Pixel 8a (60 Hz surface) for no regression.
- Known limitation: dash/defeat/inactive followed motion still steps at 60 Hz (BRL-0217).

# Deferred
- BRL-0217 (todo, parent BRL-0171, related BRL-0169): smooth followed-authority owner motion
  (dash) between ticks.
- Release/wall-contact withdrawal of one partial step is accepted (bounded, sub-cell); revisit
  only if the playtest reports it.

# Learn from errors
- I first wrote the schedule test assuming the first fixed tick after attachment moves; it does
  not (no server clock yet), and my frame-delta cycle never landed exactly on a tick boundary, so
  the exactness assertion never ran. Prevention: in fixed-step tests, construct the frame cycle so
  its sum is an exact multiple of `SIMULATION_TICK` and assert the boundary case was reached.
- The release test initially combined the residual correction-offset decay with the
  extrapolation withdrawal; separate effects before asserting a bound.
- A diagnostics-only accessor tripped `dead_code` in the client-only Clippy configuration; gate
  automation-only accessors with `cfg(any(test, feature = "automation"))` and run `just lint`, not
  only the automation Clippy target.
- `scripts/v3-render-evidence.sh` runs the supervisor with the default tracked
  `data/server` directory; evidence runs need `--data-directory` (used a scratch copy). Worth a
  script option if it recurs.
