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

# Implementation and device check (2026-11-19/20, Pixel 8a, Android 17, branch worktree-agent-ac06786c38f60ba96, commit f6c30497)

## Profiling (simpleperf, cpu-clock 2 kHz with call graphs, 20 s on the idle Dashboard, temporary profileable build of main caee2a43)
- About 1.29 cores sampled. `libc syscall` (futex wake/park) was 26% of all samples, and `raw_waker::wake` 18%, almost all from Bevy's `MultiThreadedExecutor` spawning one task per system and waking the two compute workers (`tick_executor`, `Executor::spawn`, `Parker::park`). The work itself is a flat spread over hundreds of systems at well under 1% each. `run_fixed_main_schedule` was under 0.5%, so fixed-tick catch-up and networking are not the cost.
- The render thread's `render_system` was 16%. Most of it was the **arena camera**: HDR, MSAA 4, bloom and a shadow cascade, rendering an empty world behind the opaque Dashboard every frame.
- Asset churn: `present` in `ui/surface.rs` took `get_mut` on every shell surface material every frame. The shared combat VFX animators (fire/support areas, projectile trails, splash, cone spray) did the same on the Dashboard, where the tick never advances. Every one was re-prepared each frame: `PreparedUiMaterial::prepare_asset`, `create_buffer_init` and the `gpu_allocator` paths, together about 5–8%.

## Changes (one commit)
- `packages/brawler-client/src/flow_executors.rs`: new `FlowExecutorPolicyPlugin`, installed only by `packages/brawler-android/src/lib.rs`, so desktop is unchanged. In every flow except MatchLoading and Match, every main-world schedule (switched from `Main` after `Main::run_main`) and every render-world schedule (switched from `ExtractSchedule`) runs on `SingleThreadedExecutor`. Match and its loading screen switch back to `MultiThreadedExecutor`, so match frames schedule exactly as before. The schedules that Bevy and Avian pin single-threaded (`Main`, `FixedMain`, `RunFixedMainLoop`, `PhysicsSchedule`, `SubstepSchedule`, `RenderStartup`) are never touched; a grep of Bevy 0.19.1, Lightyear 0.29 and Avian 0.7 found no other `set_executor`. The policy is the pure `battery_flow_runs_systems_in_parallel(ClientFlow)`, next to `battery_flow_winit_settings`.
- `presentation_3d/camera.rs` `render_arena_only_where_shown` (all platforms): the arena camera is active only in MatchLoading (pipelines stay warm), Match and MapEditor. Every other shell screen is an opaque full-screen UI. While the arena is off, the HUD camera clears the window to `SHELL_BG_BOTTOM` instead of `ClearColorConfig::None`. Apps without the product flow (automation, VFX Lab) always render the arena.
- `ui/surface.rs` `present` writes a surface material only when a value changed. `combat/common.rs` `advance_material_time` does the same for the five shared VFX animators.
- Docs: `docs/11-art-and-presentation-direction.md`, in the Android pacing paragraph.

## Experiments (temporary `debug.brawler.*` system-property knobs, removed before the commit; 30 s `simpleperf stat`, 3 interleaved rounds, arena camera already off, thermal status 2–3)
| main / render executors | CPU time per 30 s | instructions |
|---|---|---|
| multi / multi (as main) | 36.3–37.4 s | 22.4–26.9 G |
| single / single (shipped) | 28.5–30.3 s | 10.3–10.4 G |
| single / single, 1 compute thread | 27.0–29.2 s | 9.1 G |
| single main / multi render | 43.8–45.2 s | 14.5–14.6 G |
Making only the main world single-threaded is worse, so the policy switches both worlds. A single compute thread was only marginally better and would also change matches, so it was not taken. The arena camera off on its own cut `render_system` from 16% to 6.6% of samples, but with the multi-threaded executor the freed time went into more wake/park churn, which is why the executor change matters.

## Before/after (spec method: 120 s idle on the Dashboard, utime+stime from `/proc/<pid>/stat`, 20 s settle after connect; main = the caee2a43 release APK, after = f6c30497 release APK, interleaved, phone charging over USB)
| run | thermal status | before (main) | after |
|---|---|---|---|
| round 1 | 2–3 | 150.6 s | 122.1 s |
| round 2 | 3–4 | 151.6 s | 113.9 s |
| earlier pair (hot) | 4 | 144.6 s / 134.1 s | 134.2 s / 127.2 s (profileable build of the same code) |
- Cool rounds: **−19% and −25%** CPU time (about 1.25 → 0.95–1.0 cores). With the phone already throttled at thermal status 4 the gap shrinks to −6%. Throttled cores run every frame longer whatever the scheduling.
- `top -H` after: the main loop `android_main` is about 28–32%, each `Compute Task Pool` worker 7–11% (was 21–35%), and the second `android_main` (render) no longer shows. The process RES dropped from about 1.8 GB to 0.94 GB without the arena render targets.
- Instructions retired over 30 s on the Dashboard: 10.2 G after, against 16.7–27 G for the multi-threaded variants.
- SurfaceFlinger timestats: still 30 FPS presented on the Dashboard (907–909 frames per 30 s).

## Not achieved / follow-up
The Dashboard still uses about one core at 30 Hz. The rest is broad per-frame Bevy work: hundreds of systems in Update/PostUpdate, UI layout and text, and render extraction and submission. It is not one hotspot. Getting to "a small fraction of a core" would need a lower idle menu rate, or updates driven only by animation and input. That is a product decision on menu smoothness, so it is not done here.

## Verification
- New tests: `flow_executors::tests::{only_match_and_its_loading_screen_keep_parallel_systems, switching_executors_between_flows_keeps_every_schedule_running}`, `presentation_3d::camera::tests::{opaque_shell_flows_skip_the_arena_pass_and_the_hud_clears_instead, apps_without_the_product_flow_always_render_the_arena}`, `ui::surface::tests::present_rewrites_a_surface_material_only_when_its_look_changes`.
- `just lint` passes (ripgrep on PATH). `just test` passes. `cargo ndk --target arm64-v8a --platform 31 clippy --locked -p brawler-android --all-targets -- -D warnings` passes.
- Device: logcat shows `main-world executor policy applied parallel=false schedules=27` and `render-world … schedules=6` on the Dashboard. A Practice match (Verdant Crossfire) renders the arena and HUD normally (screenshot). The Dashboard, Server Select, Choose Game, Practice map select and Results screens look as before.

## Match frame rate and visual check (release APKs, 2026-11-20)
- Practice Wipeout 2v2 on Verdant Crossfire with bots, scripted move and fire, SurfaceFlinger timestats over about 45 s, back to back: main caee2a43 gave 888 frames, median 50 ms, p95 50 ms; f6c30497 gave 939 frames, median 50 ms, p95 66 ms. **Both runs were at thermal status 4.** The phone was charging over USB and could not cool below status 3–4 during the session, so every match ran at about 20 FPS whatever the build. **Not verified: the 60 FPS median in a cool match** (BRL-ynt5 scenario); this needs a re-check on a cool phone. By construction a match schedules exactly as before: the multi-threaded executor and the arena camera are active in MatchLoading and Match.
- Shipped f6c30497 APK screens checked: Select Server, Dashboard, Choose Game, Brawlers, Practice map select, a live Practice match (arena, HUD, overheads), and Results. All render as before, with no uncleared background now that the arena pass is off.
- Owner judgement still open: whether menus feel the same. Taps and the preview animation should be unchanged; only the executor and the hidden arena pass changed.

## Mistakes and prevention (device session)
- To cool the phone I sent `input keyevent KEYCODE_SLEEP`, which locked it on its secure keyguard, and the owner had to unlock it. Prevention: never sleep the device. With `stay_on_while_plugged_in=15`, cool it by force-stopping the app and going home.
- A batch loop of `while read … done < variants` silently ran only its first variant, because `adb shell` inside consumed stdin. Another loop in zsh did not word-split `$v`, so variants ran with the wrong knobs. Prevention: give scripted adb calls `</dev/null` and run loops in bash.
- On Darwin 27 the adb daemon (USB backend) crashed at random (`libunwind: invalid compact unwind encoding`). `ADB_LIBUSB=1` plus a `start-server`/`get-state` retry made the runs reliable.
- CPU-time comparisons on a phone that is charging over USB drift with thermal status (2 → 4 within an hour). Interleave before and after runs and record the thermal status with every sample.

## Merge verification (main, 2026-11-20)

Cherry-picked onto main as 9363f31e. On the combined tree: `just lint` pass, `just test` pass. Off-screen captures at 914x411: dashboard, results, brawler-list, map-editor (arena still renders in the editor). On the Pixel 8a (release APK from main + this commit): Dashboard 3D character renders (preview camera unaffected), Results and Dashboard navigation work.

Outcome partly met: idle Dashboard CPU dropped ~19-25% (unthrottled), not to a small fraction of a core. The rest is broad per-frame menu work; going further needs a product call on a slower idle menu rate, filed as a backlog ticket. Not verified: 60 FPS match median on a cool phone (the phone stayed at thermal status 4 while charging; both builds ran ~20 FPS). Playtest: on a cool, unplugged Pixel, play one Wipeout match and confirm it feels as smooth as before; menus should respond the same.
