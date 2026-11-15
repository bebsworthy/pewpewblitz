Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec refreshed with file refs. Not measured yet. The catalog and the cascade code match the spec.

# Outcome
The Pixel 8a frame cost of High-tier arena cast shadows (BRL-hkj4 Option A: one cascade spanning the visible ground) is measured. `render_quality.ron` is adjusted only if the data says so.

# Context (verified at HEAD)
- BRL-hkj4 measured A against shadow maps off on an Apple M6 desktop only: about +0.2 ms GPU per frame, no frame-interval change at 120 Hz.
- `assets/catalogs/render_quality.ron` (schema 3): `high.shadow_maps: true`; `medium` and `low` have `shadow_maps: false`.
- `packages/brawler-client/src/presentation_3d/render_quality.rs` `auto_tier` (L215-223): Android → Medium (`mobile: cfg!(target_os = "android")`). Only a player who picks High on a phone pays for the pass.
- `presentation_3d/camera.rs` `arena_shadow_cascades` (L43-50): `num_cascades: 1`, far bound = the farthest visible ground + margin.
- BRL-ynt5 measured the Pixel 8a High tier at 33 FPS average before BRL-hkj4, with 4 near-empty default cascades. Frame pacing: `packages/brawler-android/src/frame_pacing.rs` (60 Hz vote, ADPF).
- The catalog ships inside the APK (assets srcDir = repo `assets/`). The comparison build needs a temporary local edit of `high.shadow_maps` and a rebuild. Do not commit it.

# Plan
1. Release APK at HEAD (`just android-install HOST:PORT`). In Settings > Display pick High. Run Practice Wipeout 3v3 on Verdant Crossfire with bots and scripted move + fire (the BRL-ynt5 scenario). Collect `adb shell dumpsys SurfaceFlinger --timestats` (enable, clear, run ~60 s, dump) and `dumpsys thermalservice` before and after.
2. Repeat with a temporary build where `high.shadow_maps: false`.
3. Record for each: median and p95 frame time, GPU post-acquire median, and thermal status. Optionally do a third run at Medium as a reference.

# Acceptance
- **Pixel 8a device check:** the recorded cost on the device, using the method above, posted on this ticket.
- A decision: keep High as is, or drop shadow maps from High on Android, with the rationale. If High changes on Android only, it needs a platform override in `render_quality.rs` (the catalog has no per-platform tiers). Add a `render_quality/tests.rs` test for it, and run `cargo test -p brawler-client render_quality`.
