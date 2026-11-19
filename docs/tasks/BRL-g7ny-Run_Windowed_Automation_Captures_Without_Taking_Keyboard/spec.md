# Outcome
Every script that opens a client, UI gallery or VFX Lab window runs it in the background on macOS. The window renders and captures normally, and the developer's active app keeps keyboard focus.

# Research (2026-11-19, verified on this Mac, Darwin 27)
- **Cause:** winit 0.30.13 calls `NSApplication.activateIgnoringOtherApps(true)` in `applicationDidFinishLaunching`. Bevy 0.19.1 builds the event loop inside `WinitPlugin::build` and does not expose `EventLoopBuilderExtMacOS::with_activate_ignoring_other_apps`.
- **Probe:** a small AppKit tool prints `NSWorkspace.frontmostApplication` (name, bundle id, pid) every 250-500 ms during a capture. System Events AppleScript is not authorized here, and `lsappinfo` returns nothing.
- **Findings:**
  - A plain launch became frontmost.
  - A regular `.app` launched with `open -g` rendered correctly but became frontmost for about 1 s.
  - An LSUIElement agent bundle with `open -g` did not become frontmost while Chrome was active. But it captured a black frame, because the never-focused window used winit's unfocused reactive update mode and was not composited.
  - Activation could still happen while the Claude desktop app was frontmost, because it is the responsible launcher.
- **Working combination:**
  1. `scripts/lib/background-window.sh` hard-links the binary into a throwaway LSUIElement bundle and starts it with `open -g -n -W` (env via `--env`, logs via `--stdout`/`--stderr`). It resolves the real pid and removes the bundle afterwards. It always passes `BRAWLER_BACKGROUND_WINDOW=1`.
  2. Client: `automation/background_window.rs`. Before the event loop exists it installs a shared `NSApplication` subclass whose `activateIgnoringOtherApps:` and `activate` do nothing; winit reuses an existing shared application. The window has `focused: false` and `WindowLevel::AlwaysOnTop` (without the floating level the inactive window captured black), and `unfocused_mode = focused_mode`.
  3. Result: dashboard, settings, game-select and weapon-parts captures were correct, and the bundle was never frontmost (0 of about 1,200 samples), including while the Claude app was frontmost.
- **Escape hatch:** `BRAWLER_FOREGROUND_WINDOWS=1` restores the old direct launch.

# Remaining
- Use the helper in `scripts/shell-capture.sh` (make capture/run dirs absolute; macOS starts the program in `/`), `scripts/v3-render-evidence.sh`, the e2e/network scripts that start windowed clients, and the UI gallery / VFX Lab capture paths (their window builders need the same `background_window::window` / `winit_settings` / `prepare_process` calls).
- README automation section: document the behaviour and the escape hatch.
- `just lint` (client feature isolation: objc2 deps are automation-only and macOS-only), `just test`, and a full `just shell-capture` with the probe showing no focus taken.


# Owner decision 2026-11-19: render captures off-screen
- **Floating approach rejected.** The bundle + `open -g` + NSApplication-subclass approach did stop focus theft, but it needed `WindowLevel::AlwaysOnTop` to render: the owner saw capture windows float over everything with no way to send them behind. `AlwaysOnBottom` and the normal level capture black frames, because macOS stops compositing covered windows.
- **Decision:** automation captures render off-screen, with no OS window. Camera(s), including the UI, target an `Image` render target at the requested size. Screenshots read that image, the app runs without winit (`ScheduleRunnerPlugin`), and no NSApplication exists, so nothing can activate or appear.
- **Remove the rejected parts:** the `.app`/`open -g` launcher, the NSApplication subclass, the objc2 dependencies and AlwaysOnTop. Keep only what off-screen mode needs.

# Off-screen implementation (2026-11-19)

## Research
- Bevy 0.19.1 (`~/.cargo/registry/.../bevy_render-0.19.1`, `bevy_window-0.19.1`, `bevy_ui-0.19.1`; pattern from `references/bevy/examples/app/headless_renderer.rs`):
  - `WindowPlugin` spawns the primary `Window` with an empty `RawHandleWrapperHolder`; without winit nothing fills it, so renderer init creates no surface (`initialize_renderer` takes `None`) and `extract_windows` skips the window (it requires `RawHandleWrapper`). No OS window and no `NSApplication` exist.
  - `RenderTarget` is a camera component; `Screenshot(RenderTarget::Image(..))` reads an image target. The screenshot target must equal the camera target *including* `ImageRenderTarget::scale_factor` (it is part of `Eq`/`Hash`), or the screenshot never receives the frame.
  - `DefaultUiCamera` falls back to the highest-order camera that targets the primary window when no camera has `IsDefaultUiCamera`. Retargeted cameras lose that fallback (the UI gallery captured only its clear colour until this was handled).
- Today's windowed pacing on this Mac is the 120 Hz display (BRL-cgcf report: `frame_p50_ms=8.333`); the Heist Practice render default (11 s measured, 1,200-sample gate) depends on it, and VFX Lab / gallery / scheduled captures are frame-count based.

## Decisions
- **One seam: a winit-less primary `Window`.** Off-screen runs keep the ordinary primary `Window` entity as the viewport description (size, scale factor), so the ~60 `PrimaryWindow` readers (UI scale, layout, HUD fitting, camera aspect, safe area) work unchanged; `ui/layout_harness.rs` already uses the same pattern in tests. No per-system patches, no new viewport resource. Window mutations (fullscreen, vsync) have no backend; the target size is fixed at startup.
- `automation/offscreen.rs` (`add_offscreen_plugins`): disables `WinitPlugin`, adds `ScheduleRunnerPlugin::run_loop`, sets the window to a fixed **2x** scale (914x411 -> 1828x822, identical to the Retina captures, now on any machine), creates one `Rgba8UnormSrgb` render-target image of the window's physical size, retargets every window-targeting camera to it with an `Insert<RenderTarget>` observer (dashboard/map preview image cameras keep theirs), and keeps Bevy's default-UI-camera fallback by marking the highest-order retargeted camera when none is marked.
- Screenshots: `evidence_capture::capture_target` uses the off-screen image when `OffscreenRenderTarget` exists (automation-gated), else the primary window. The gallery capture uses it directly.
- Opt-in: client `--offscreen` (automation CLI; rejected with `--headless`), passed by `scripts/shell-capture.sh` and both clients of `scripts/v3-render-evidence.sh`; VFX Lab `--offscreen` (requires `--capture-after` and `--exit-after-capture`); UI gallery: `BRAWLER_UI_GALLERY_CAPTURE` always renders off-screen (it is scripted and exits by itself). Interactive `just client`, `just run`, `just ui-gallery`, `just vfx-lab` are unchanged and windowed.
- Pacing: `native` -> 120 Hz, `60fps` -> 60 Hz, `30fps` -> 30 Hz, `high-refresh` -> uncapped (`offscreen_frame_interval` in `lib.rs`). 120 Hz rather than 60 keeps frame-count capture schedules showing the same moments as today's captures and keeps the Heist Practice sample gate valid.
- Render report schema 9 -> **10**: new `render_target=window|offscreen`. Off-screen frame times measure CPU+GPU work at the schedule-runner pace without swapchain acquire/present or compositor effects; the pass thresholds are unchanged (they are CPU frame-interval gates and still meaningful). `docs/11` and README document this.
- Rejected approach fully removed: `scripts/lib/background-window.sh`, `automation/background_window.rs`, objc2/objc2-app-kit deps (Cargo.toml/Cargo.lock back to HEAD), `AlwaysOnTop`/focus/winit-settings hacks, absolute-path script edits (no longer needed: the process starts in the script's cwd again). Its old outputs were moved to `target/g7ny/rejected-floating/`.

## Verification
- `just lint`: pass (includes client feature isolation; new marker `rendering off-screen without an OS window` added to `scripts/check-client-features.sh`, absent from the player binary, present in the automation release binary).
- `just test`: see below.
- New unit tests (`automation/offscreen.rs`): logical size kept at 2x; window cameras retargeted while preview image cameras keep their target; highest-order retargeted camera becomes the default UI camera.
- Focus/window probe (`front` = `NSWorkspace.frontmostApplication`, `wins` = `CGWindowListCopyWindowInfo(.optionAll)` for the client pid, sampled every 250 ms):
  - full `just shell-capture` (`BRAWLER_SHELL_CAPTURE_DIR=target/g7ny/shell-capture`, bind 127.0.0.1:5040): passed, 40 PNG/JSON pairs; 375 samples with a live client, every one `windows=0`; frontmost app Chrome in all 770 samples.
  - `scripts/v3-render-evidence.sh target/g7ny/render.txt` (`BRAWLER_RENDER_BIND=127.0.0.1:5039`): passed; both reports `schema=10 render_target=offscreen window 2560x1440 sample_count≈3380 frame_p50≈8.8 ms p95≈10.3 ms p99≈11.2 ms gpu_p50≈5.0 ms result=pass`; 3 captures; 225 live-client samples all `windows=0`, frontmost Chrome throughout.
  - UI gallery (`BRAWLER_UI_GALLERY_PHONE=1 BRAWLER_UI_GALLERY_CAPTURE=target/g7ny/gallery/gallery.png`): 3 PNGs at 1828x822, correct content, `windows=0`.
  - VFX Lab (`--fixture splash --capture-after 90 --exit-after-capture --offscreen`, `target/g7ny/vfx-lab/`): PNG+JSON at 2560x1440, correct content, `windows=0`.
- Pixel comparison against the windowed captures in `target/fe7t/phase2/after/` (same sizes): settings, map-library, map-editor, map-editor-palette identical (mean abs diff 0.000); game-select, weapon-equipment, first-brawler, brawler-list <= 0.25 mean abs diff (< 0.4 % pixels); dashboard 2 % pixels from the animated idle pose and the random profile name only. Visually checked.

## Known limits
- Off-screen render-evidence frame times are not display-presentation times (recorded as `render_target=offscreen`).
- The schedule runner's sleep overshoots slightly (native p50 ≈ 8.8 ms, not 8.33 ms).
- Off-screen captures are always 2x, also on a 1x display; window mutations do not resize the target.
- Opt-in `BRAWLER_NETWORK_HEADLESS=0` windowed network scripts stay windowed (human-watched by design, out of scope).
- The interactive windowed path was not launched (its code path is unchanged apart from building the same plugin group into a local first).

- `just test` (after the clippy fixes): exit 0, every suite `ok` (including the 132-test network suite and the 901-test client lib suite, `target/g7ny/test.out`).
