Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec refreshed — the report has no GPU timing yet; the BRL-hkj4 probe was scratch-only and is gone.

# Outcome

`scripts/v3-render-evidence.sh` reports include a per-frame GPU-time distribution (p50/p95/p99) next to the frame interval, so a render change's cost shows up on a fast desktop.

# Context (BRL-hkj4, 2026-11-14)

On the Apple M6 MacBook Pro the frame interval is vsync-bound at 120 Hz (8.33 ms p50) for every variant, even with `BRAWLER_RENDER_PROFILE=uncapped` and vsync off (the macOS compositor paces the windowed Metal app). The report therefore could not separate shadow options. Bevy's `RenderDiagnosticsPlugin` does not span the shadow pass, and wgpu encoder timestamp queries returned zeros on that Metal device. A temporary probe registered `Queue::on_submitted_work_done` after each submission, with a thread polling the device every 50 µs. It gave stable numbers: about 4.3-4.9 ms p50 GPU per frame, with about 0.2 ms resolution between variants (BRL-0180 scratch `gpu-*.txt.gpu`, for example `gpu n=3000 done_p50=4.344 done_p95=6.404 done_p99=6.605`). The probe source was not kept.

# Current code

- Report writer: `packages/brawler-client/src/presentation_3d/diagnostics.rs` (automation-only, `presentation_3d/mod.rs:33-34`). It has `REPORT_FIELDS` (line 23), `RenderMeasurementPlugin` (262), and `sample_and_finalize_render_measurement` (295, frame samples from `FrameTimeDiagnosticsPlugin`). It also has `percentile` (450), `compose_report` (480, `schema=6`), and `validate_report` (590), plus tests at 633+.
- APIs: Bevy 0.19.1 `RenderSystems::Render` / `Cleanup` (bevy_render `lib.rs:157-204`), wgpu 29 `Queue::on_submitted_work_done` (`api/queue.rs:303`) and `Device::poll(PollType)` (`api/device.rs:95`). `render_quality.rs:19,192` already reads `RenderDevice`.

# Plan

1. In `diagnostics.rs`, add a `RenderApp` system ordered after `RenderSystems::Render`. It stamps `Instant::now()` and registers `RenderQueue::on_submitted_work_done`, which pushes the elapsed ms into an `Arc<Mutex<Vec<f64>>>` (bounded by `SAMPLE_CAPACITY`). Install it only from `RenderMeasurementPlugin`, so it stays automation-only and is absent from player builds and gameplay.
2. Add a small poll thread (`device.poll(PollType::Poll)` every ~50 µs) that runs only while measurement is active and is joined or stopped at finalize. Alternatively, use timestamp queries where `Features::TIMESTAMP_QUERY` works and returns non-zero values.
3. Sample only inside the warmup..warmup+measurement window that the main-world system already tracks. Share the window through the same `Arc` with an `active` flag.
4. Report `gpu_time_source=submitted-work-done|timestamp|unavailable`, `gpu_sample_count`, `gpu_p50_ms`, `gpu_p95_ms`, and `gpu_p99_ms`, with `unavailable`/`0` and source `unavailable` when no samples arrive. Bump the schema (to 7, coordinating with BRL-rr1x if it lands first) and extend `REPORT_FIELDS`. GPU time stays informational and is not part of `result`.
5. Document the caveat in the report docs and code comments: completion time approximates GPU time only when the GPU is idle at submit (vsync-bound runs).

# Acceptance

- Unit tests: `compose_report` emits the GPU fields with given samples and the unavailable form with none, and `validate_report` accepts both.
- `cargo build -p brawler-client` (without `automation`) and `just lint` (`_client-features`) show no GPU probe in the player build; `just test` passes.
- One desktop run, `BRAWLER_RENDER_PRACTICE=1 just v3-render-evidence target/89kq/report.txt`, shows `gpu_time_source=submitted-work-done` and a plausible p50 (around 4-5 ms on the M6), noted on the ticket.


# Result (2026-11-18)

- `packages/brawler-client/src/presentation_3d/diagnostics/gpu_time.rs` (new, automation-only): `GpuTimeProbe` installs a `Render` system after `RenderSystems::Render` / before `Cleanup` that stamps `Instant::now()` and registers `RenderQueue::on_submitted_work_done`. The callback pushes elapsed ms into a shared `Arc` bounded by `SAMPLE_CAPACITY`, and only while `active`. A `render-evidence-gpu-poll` thread runs `RenderDevice::poll(PollType::Poll)` every 50 µs. It starts at the first post-warmup frame, and finalize (or the probe's `Drop`) stops and joins it. APIs were checked against vendored wgpu 29.0.4 (`Queue::on_submitted_work_done`, `Device::poll`, `PollType::Poll`) and bevy_render 0.19.1 (`RenderQueue` deref, `RenderDevice::poll`, `render_resource::PollType`, `render_system` submit and present inside `RenderSystems::Render`).
- `diagnostics.rs`: the report is now schema 9 and adds `gpu_time_source` (`submitted-work-done|unavailable`), `gpu_sample_count`, and `gpu_p50_ms`/`gpu_p95_ms`/`gpu_p99_ms` (`unavailable` with no samples). GPU time is informational and never part of `result`. Timestamp queries were not implemented because they returned zeros on this Metal device (BRL-hkj4).
- Caveat: completion time approximates GPU time only when the GPU is idle at submit, as in vsync-bound runs. It is recorded in the module docs and in `docs/11-art-and-presentation-direction.md`.
- Tests: the new `report_carries_informational_gpu_time` checks the measured and unavailable forms, that both validate, and that `result` stays pass. The gpu_time unit tests cover bounded, active-only recording and the unavailable source when there is no render app.

Verification:
- `just lint`: pass (`target/89kq/lint.log`).
- `just test`: pass (`target/89kq/test.log`).
- `cargo build -p brawler-client` (player): the debug player binary has 0 occurrences of `render-evidence-gpu-poll`. The automation release binary has 1.
- `BRAWLER_RENDER_BIND=127.0.0.1:5035 BRAWLER_RENDER_PRACTICE=1 just v3-render-evidence target/89kq/report.txt`: Apple M6/Metal, `result=pass`, frame p50 8.329 ms, `gpu_time_source=submitted-work-done`, n=3600, gpu p50/p95/p99 = 4.707/6.632/6.822 ms.
- Default `just v3-render-evidence target/89kq/default.txt` (bind 5035): both reports pass.
  - Measured client: frame p50 16.663, gpu n=1800, 4.414/4.995/5.348.
  - Peer: frame p50 8.330, gpu n=3599, 4.341/6.025/6.303.
- Not verified: Android, and GPU-bound (non-vsync-limited) runs, where queueing inflates the value.
