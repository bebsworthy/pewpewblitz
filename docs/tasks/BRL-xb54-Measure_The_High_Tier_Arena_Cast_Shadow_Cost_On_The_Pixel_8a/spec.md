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


# Pixel 8a measurement (2026-11-18)
Setup: Pixel 8a (45171JEKB03025), display at 60 Hz, phone on USB charge. Routed server on 0.0.0.0:5000 at HEAD 40fc1563. Scenario: Practice Wipeout 3v3 on Verdant Crossfire with bots. Graphics quality was forced to HIGH in Settings > Display. Scripted move and fire came from serialized `adb shell input swipe` calls (left stick, then the aim stick, in a loop). For each run: thermal state, then `dumpsys SurfaceFlinger --timestats -enable/-clear`, a 45 s window starting about 12 s into the match (A1 used 60 s), then `-dump` and thermal state again. Every run was checked with a start and end screenshot (in the match, not on results) and the process-scoped logcat tier line. Before each run the game was force-stopped and the phone sat on the launcher until thermal status was 0 and VIRTUAL-SKIN was below 37.5 °C. Runs B2 to M1 used that cooldown.
- **A** = HEAD APK (`high.shadow_maps: true`; logcat `tier="HIGH" ... msaa=Sample4 hdr=true bloom=true shadow_maps=true`).
- **B** = temporary APK with `high.shadow_maps: false` (logcat `tier="HIGH" ... shadow_maps=false`). Not committed. The catalog was reverted with `git checkout`.
- **M** = HEAD APK at MEDIUM (player override), as a reference.
- Order: A1, A2, B1, B2, A3, A4, M1. The A and B runs are interleaved, so thermal drift and charge state are not confounded with the build.

| Run | Avg FPS | Frame (present→present) median / p95 | ≥2-vsync frames | GPU post→acquire median / p95 | Thermal status (skin °C) before → after |
|---|---|---|---|---|---|
| A1 shadows on (60 s) | 34.9 | 33 / 33 ms | 74% | 29 / 38 ms | 0 → 0 |
| A2 shadows on | 31.8 | 33 / 50 ms | 84% | 31 / 42 ms | 1 → 1. **Partially invalid:** the match ended inside the window (results screen in the end shot). |
| A3 shadows on | 32.4 | 33 / 50 ms | 82% | 31 / 42 ms | 1 → 2 (40.4 → 43.4) |
| A4 shadows on | 32.3 | 33 / 33 ms | 84% | 33 / 42 ms | 1 → 2 (40.6 → 43.7) |
| B1 shadows off | 29.9 | 33 / 50 ms | 91% | 38 / 54 ms | 1 → 1 (40.4 → 42.2). No cooldown before this run, and an Android build was running on the Mac. |
| B2 shadows off | 35.0 | 33 / 33 ms | 73% | 29 / 40 ms | 1 → 2 (40.5 → 43.4) |
| M1 Medium (reference) | 58-60 | 16 / 16 ms | 3% | 11 / 15 ms | 1 → 1 (39.6 → 41.4) |

Valid A runs (A1, A3, A4) average 33.2 FPS, with a GPU post→acquire median of 29-33 ms. B runs (B1, B2) average 32.5 FPS, with 29-38 ms. The run-to-run spread inside either build (about ±2.5 FPS, ±4 ms) is larger than any difference between the builds. The single-cascade shadow pass therefore costs less than the measurement noise on the Pixel 8a, likely ≤ about 1-2 ms GPU per frame. That fits the BRL-hkj4 desktop figure of about +0.2 ms. High without shadows still runs at 30-35 FPS (frame median 33 ms). Medium runs at about 60 FPS (16 ms, post→acquire 11 ms). The High-tier cost on the phone comes from the features that differ between High and Medium apart from shadows (MSAA 4x, bloom), not from shadow maps. Every High run reaches thermal status 2 within about 45 s. Medium stays at 1.

**Decision: keep High as is.** No Android override in `render_quality.rs`. Dropping shadow maps from High on Android would remove a visible readability and look feature from a tier the player chose explicitly. It would not move the frame rate: 32.5 vs 33.2 FPS, inside the noise. Auto on Android stays Medium, and Medium does not pay for the pass. If High on phones ever needs to be cheaper, the levers are MSAA and bloom. That is a separate decision; no ticket was filed.

Evidence: `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/xb54/`. It contains `measurements.txt`, `timestats-{A1..A4,B1,B2,M1}.txt`, `thermal-*.txt`, `tier-*.txt` (logcat tier lines), the screenshots `*-start.jpg`, `*-end.jpg` and `*-mappick.jpg`, the scripts (`run.sh`, `nav.sh`, `cool.sh`, `ts.py`), and the build logs `build-B.log` and `build-head.log`.

End state: the phone has the HEAD release APK reinstalled (`head.apk`, built from the reverted catalog; its packaged `render_quality.ron` has `shadow_maps: true`). Graphics quality is back on Auto; logcat shows `tier="MEDIUM" preference="AUTO" reason="mobile GPU"`. `assets/catalogs/render_quality.ron` is clean in git.

Process notes (learn from errors):
- I sent `KEYCODE_SLEEP` to cool the phone, and the secure keyguard locked it. That cost about 25 minutes until the owner unlocked it. Prevention: cool down by force-stopping the game on the launcher with the screen on (stay-awake while charging is already enabled). Never put the screen to sleep.
- Bot matches on Verdant end in about 70-90 s, so a 60 s window started late can include the results screen (A2). Use 45 s windows that start about 12 s into the match, and check the end screenshot.
- VIRTUAL-SKIN rises from about 37 °C to about 40 °C during the roughly 40 s of menus and match start, so every run starts at status 1. Use the same cooldown and navigation for each run, and interleave the builds.
- Relaunching within a minute of force-stopping rejoined the still-running practice match (rejoin path). Wait for the match to end before using menu-only navigation.
