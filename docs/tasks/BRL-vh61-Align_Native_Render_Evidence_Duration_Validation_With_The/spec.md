# Scope

- Reconcile `RenderMeasurementConfig` duration validation with the canonical report validator's minimum sample count and render cadence assumptions.
- Prefer an early actionable configuration error or a duration-aware canonical launcher default over a late completed-run failure.
- Preserve the locked 1,200-sample performance evidence threshold unless a separate measured architecture decision changes it.
- Add focused configuration and script tests and update the native evidence runbook.

# Acceptance criteria

- Every duration accepted by the canonical `v3-render-evidence` path can satisfy the sample gate under its intended cadence, or validation clearly explains the required duration.
- The default command passes its sample-count gate and explicit invalid short windows fail before launching the routed topology.
- Existing report schema and historical reports remain readable.

# Research (2026-11-19)

- The report gate is `render_measurement_failure` in `packages/brawler-client/src/presentation_3d/diagnostics.rs`: `sample_count < 1_200` fails. Samples are one per new `FrameTimeDiagnosticsPlugin::FRAME_TIME` measurement during the measurement window, so the count is (window length) x (effective frame rate).
- `RenderMeasurementConfig` validation (`automation/config.rs`) only bounded warm-up/measurement to 1-120 s, so e.g. 8 s was accepted and deterministically failed `sample_count` after a full routed run.
- Cadence after BRL-g7ny: off-screen runs are paced by `ScheduleRunnerPlugin::run_loop(offscreen_frame_interval(profile))` (native 120 Hz, 60fps/30fps keep their rate, high-refresh uncapped). Windowed runs: native/high-refresh are display-paced, 60fps/30fps are winit-reactive caps.
- Measured effective off-screen native rate is below nominal: 3,430 samples / 30 s = 114 Hz, 1,217 / 11 s = 111 Hz (the runner sleeps `interval - frame time`, plus sleep overshoot). The old Heist Practice default 4 s + 11 s therefore had only 17 samples of margin.

# Decision

- One source of truth in the client: `RENDER_REPORT_MINIMUM_SAMPLES = 1_200` lives beside `RenderMeasurementConfig` and the report validator uses it. Threshold unchanged.
- `planned_render_frame_interval(profile, offscreen)` (lib.rs, beside the pacing it mirrors): off-screen pace; windowed 30/60 Hz caps; 60 Hz for uncapped or display-paced runs (slowest development display refresh, and close to the slowest rate the 18.5 ms p95 gate can pass).
- Validation requires `ceil(1,200 x 1.2 x interval)` seconds: 12 s native off-screen, 24 s at 60 Hz, 48 s at 30 Hz. 20% headroom (not 10%) because of the measured 110-114 Hz effective rate; 20% still passes at an effective 100 Hz. The error names the requested and required duration and the cadence.
- Early rejection: new automation-only `--check-config` flag (parse + full validation, print, exit 0 without building an app). `scripts/v3-render-evidence.sh` now builds the measured and peer command lines once as arrays and runs both with `--check-config` (peer under its `BRAWLER_RENDER_PEER_PROFILE`) after the cargo builds and before the supervisor starts; on rejection it exits 2 naming `BRAWLER_RENDER_MEASURE_SECONDS`. No shell-side duplicate of the rule.
- Heist Practice default measurement 11 s -> 12 s (4 s + 12 s = 16 s, still inside the ~18 s earliest bot-driven match end). Other defaults (10 s + 30 s) unchanged.
- No report field or schema change; schema 10 and historical reports read as before.
- Not changed: the off-screen schedule runner's sub-nominal pacing (owned by BRL-g7ny presentation pacing; accounted for by headroom), and the script's 75 s watchdog versus long explicit windows.

# Verification (2026-11-19)

- Focused tests in `automation/config.rs`: `render_measurement_must_reach_the_sample_gate_at_the_planned_cadence` (every profile x off-screen/windowed: required duration accepted, one second less rejected with "use at least N seconds", and required duration x cadence >= 1,200) and `the_render_evidence_launcher_defaults_reach_the_sample_gate_off_screen`. Test helper pins the native profile so `BRAWLER_RENDER_PROFILE` cannot change results.
- `BRAWLER_RENDER_BIND=127.0.0.1:5044 scripts/v3-render-evidence.sh target/vh61/render.txt`: passed; measured 3,430 samples, peer 3,427, p95 9.9/10.0 ms, schema 10, render_target=offscreen.
- Heist Practice `BRAWLER_RENDER_BIND=127.0.0.1:5045 BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_MODE=heist`: passed, 4 s + 12 s, 1,327 samples, p95 10.4 ms, left_match=false. With `BRAWLER_RENDER_COMBAT=1`: passed, 1,322 samples.
- Fast fail: `BRAWLER_RENDER_MEASURE_SECONDS=8` -> exit 2, "--render-measure-seconds 8 cannot reach the report's 1200-sample gate at the 120 Hz off-screen native cadence; use at least 12 seconds", no supervisor started, no report or capture created (the 128 s wall time was the script's cargo builds waiting on other agents' build locks). `BRAWLER_RENDER_PEER_PROFILE=30fps` at the default 30 s -> exit 2 in 3 s from the peer check ("use at least 48 seconds").

- `just lint` and `just test` pass (after the final 20% headroom change).
- Final default run on the final code: two runs while other agents' builds held load average at 13-18 failed `frame_p95_ms` (79 ms, then 30 ms; samples 1,249 and 2,381): host contention, not the sample gate. Rerun at load ~5: `target/vh61/render-final3.txt` passed, 3,316 / 3,315 samples (110.5 Hz effective), p95 10.4 ms. Under contention the effective rate fell to ~41 Hz, which no headroom can absorb; the frame-time gate fails such runs anyway.

# Mistakes and prevention

- First cut used 10% headroom from the nominal 120 Hz; the Heist run then passed with 1,217 samples (17 to spare). Prevention: size headroom from measured effective cadence, not nominal pacing.
