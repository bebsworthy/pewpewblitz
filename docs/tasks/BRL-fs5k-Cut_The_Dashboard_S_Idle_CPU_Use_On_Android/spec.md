# Outcome
The Dashboard on Android uses a small fraction of one core when idle, not about 1.4 cores.

# Evidence (BRL-m4eh device check, 2026-11-18, Pixel 8a, Android 17)
- App CPU time over 120 s idle on the Dashboard (`/proc/<pid>/stat` utime+stime): 209.6 s before BRL-m4eh (≈60 FPS), 170.0 s after (30 FPS). Halving the frame rate cut CPU by only 19%.
- `top -H` on the Dashboard: two `Compute Task Pool` threads at 34-45% each and two `android_main` threads at 17-41%, at both 60 and 30 FPS.
- So most of the cost does not scale with presented frames. Suspects: fixed-tick catch-up (networking/prediction systems in `FixedUpdate` while connected to the lobby), the task-pool workers spinning or parking poorly, or per-update extraction/UI layout work that runs whatever the frame rate.

# Plan
1. Profile on the device (simpleperf, or Bevy trace with a profiling build) to find which systems and threads use the CPU on the Dashboard.
2. Fix the biggest consumers. Desktop behaviour and the match frame rate (60 FPS median in the BRL-ynt5 scenario) must not regress.

# Acceptance
- **Pixel 8a:** idle Dashboard CPU, measured as above, recorded before and after with a clear reduction.
- `just lint` and `just test` pass.
