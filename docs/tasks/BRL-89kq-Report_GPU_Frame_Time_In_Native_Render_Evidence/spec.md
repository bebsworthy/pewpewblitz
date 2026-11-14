# Outcome

`scripts/v3-render-evidence.sh` reports include a per-frame GPU-time distribution (p50/p95/p99)
next to the frame interval, so a render change's cost is visible on a fast desktop.

# Context (BRL-0180, 2026-11-14)

On the Apple M6 MacBook Pro the frame interval is vsync-bound at 120 Hz (8.33 ms p50) for every
variant, even with `BRAWLER_RENDER_PROFILE=uncapped` and vsync off (the macOS compositor paces the
windowed Metal app), so the report could not separate shadow options. Bevy's
`RenderDiagnosticsPlugin` does not span the shadow pass, and wgpu encoder timestamp queries
returned zeros on that Metal device. A temporary probe that registered
`Queue::on_submitted_work_done` after each submission, with a thread polling the device every
50 µs, produced stable numbers (≈ 4.3-4.9 ms p50 GPU per frame, ~0.2 ms resolution between
variants).

# Scope

- Add an automation-only GPU-time sample to the render report (schema bump), using the completion
  approach above or working timestamp queries if a backend supports them; mark it unavailable
  rather than wrong where it cannot be measured.
- Keep it out of non-automation builds and out of gameplay.

# Acceptance criteria

- Report fields for GPU-time p50/p95/p99 (or an explicit unavailable marker) and a unit test for
  the percentile/reporting path.
- One desktop run showing the field.
