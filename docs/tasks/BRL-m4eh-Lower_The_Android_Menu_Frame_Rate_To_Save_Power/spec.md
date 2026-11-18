Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec refreshed with code refs. Nothing has been implemented. Android still updates continuously in every flow.

# Outcome
Lower power use outside matches on Android without making menus feel sluggish.

# Context (verified at HEAD)
- Deferred from BRL-zj53 to BRL-ynt5, then split out. With BRL-ynt5's ADPF hint session the Dashboard already presents at 60 FPS, so a lower menu rate is a power decision, not a frame-rate fix. Desktop update behaviour must stay unchanged.
- `WinitSettings` is chosen once at startup: `packages/brawler-client/src/lib.rs` `render_profile_settings` (L742-761), called from `compose_client_app` (L657). Android has no command line, so `RenderProfile::from_env()` (`src/config.rs` L21-28) is `Native`, which gives `WinitSettings::game()` (focused continuous). Nothing changes it per flow.
- Flows: `packages/brawler-client/src/flow/model.rs` `ClientFlow` (Connecting, ServerSelect, Dashboard, GameTypeSelect, PracticeMapSelect, MapLibrary, MapEditor, Queue, MatchLoading, Match, Results).
- Android pacing: `packages/brawler-android/src/frame_pacing.rs` votes 60 Hz on the surface (`request_game_frame_rate`). It reports each frame's work to an ADPF session with a fixed 16.7 ms target (`begin_frame` / `report_frame`). With reactive updates that report should stay correct, or be paused, so idle gaps are not reported as work.
- Things that move in menus and need the reduced rate to look acceptable: the Dashboard 3D weapon/fighter preview (`packages/brawler-client/src/dashboard/weapon_preview.rs`), shell background motifs (`ui/motifs.rs`, which follow Reduced motion), surface entrance and burst animations (`flow.rs` `begin_surface_entrance`, `screens/shared.rs` `apply_flow_burst`), and slider/scroll touch drags.

# Plan
1. Add an Android-only system, installed from `packages/brawler-android/src/lib.rs` next to `frame_pacing::install` so desktop is untouched. On `ClientFlow` changes it sets `WinitSettings`:
   - Match and MatchLoading: `focused_mode: Continuous`.
   - All other flows: `UpdateMode::reactive(1/30 s)` focused and `reactive_low_power` unfocused.
2. Input wakes immediately (reactive already wakes on window and touch events). Confirm that a touch drag on a slider or scroll list updates at the input rate, not at 30 Hz.
3. Optionally vote 30 Hz on the native window in menus (`ANativeWindow_setFrameRate`) so the panel can drop refresh. Decide this from the measurements.
4. Make `frame_pacing.rs` report only real frames, and keep the 16.7 ms target in Match.
5. Unit-test the flow → `WinitSettings` mapping as a pure function, on the host, without Android.

# Acceptance
- `cargo test -p brawler-client` (and the new mapping test) passes. The desktop `WinitSettings` is unchanged; check `render_profile_settings` callers.
- **Pixel 8a device check:** record the Dashboard frame rate (`dumpsys SurfaceFlinger --timestats`) and the power estimate (`dumpsys batterystats --reset`, 5 min idle on the Dashboard, `dumpsys batterystats` app estimate) before and after.
- **Pixel 8a device check:** the Match frame rate is unchanged (60 FPS median, BRL-ynt5 scenario). Menus respond to taps without visible delay, and the Dashboard preview motion stays smooth enough (owner judgement).


# Implementation (2026-11-18, commit 1901a888 on branch worktree-agent-a313b111ff96d258e)

## Changes
- `packages/brawler-client/src/lib.rs`: new `pub fn battery_flow_winit_settings(ClientFlow) -> WinitSettings`, next to `render_profile_settings`. Match and MatchLoading return `WinitSettings::game()` (focused Continuous, unfocused `reactive_low_power(1/60 s)`). Every other flow returns `reactive(1/30 s)` when focused and `reactive_low_power(1/30 s)` when unfocused. The match is exhaustive, so a new flow must pick a mode.
- `packages/brawler-android/src/frame_pacing.rs` (installed by the existing `frame_pacing::install`, so desktop is untouched): the `apply_flow_update_mode` system in `Last` writes `WinitSettings` whenever `State<ClientFlow>` changes. Bevy 0.19's winit runner re-reads `WinitSettings` after every update and forces a redraw when the mode changes, so a switch takes effect on the next frame.
- ADPF reporting: work is still measured from `First` to `Last` within one update. The idle wait between reactive updates is therefore never reported. `report_frame` now also skips updates with no window surface: no `PrimaryWindow` with a `RawHandleWrapper`. Bevy removes that wrapper for the final update before an Android suspend. The 16.7 ms target is unchanged; Match needs it, and short menu frames only report headroom.

## Decisions
- The pure mapping is in brawler-client, not brawler-android. brawler-android is a cdylib, and no `just test`/`just lint` recipe builds it on the host. brawler-client's tests already run in `just test`, beside the `render_profile_settings` test. The dependency still points one way: brawler-android uses brawler-client.
- MapEditor and Results also use 30 Hz reactive, following the plan. Drags wake on input anyway.
- 30 Hz `ANativeWindow_setFrameRate` vote in menus: not done. The surface still votes 60 Hz. Treat this as a measurement-driven follow-up: if timestats show the panel holding 60/120 Hz while the app presents at 30, a menu vote of 30 with `FrameRateCompatibility::Default` (and back to 60 on Match) is the next step.

## Research (Bevy 0.19.1 source, ~/.cargo/registry bevy_winit-0.19.1)
- `state.rs::window_event` sets `window_event_received` for every winit `WindowEvent`, Touch included. `UpdateMode::reactive` sets `react_to_window_events: true`, so a touch wakes the loop immediately, and drags update at the input event rate, not 30 Hz.
- The 0.19.1 engine crates have no `RequestRedraw` writers, and the repo has none, so nothing keeps menus continuous by accident.
- The menu motion is all driven by `Time`, so it keeps animating at the 30 Hz wake rate with larger steps. That covers motifs (`time.elapsed_secs()`), surface entrance (`flow.rs`), flow burst (`screens/shared.rs`), and the Dashboard card preview's `AnimationPlayer`. `dashboard/weapon_preview.rs` only manages render targets and has no per-frame motion of its own. Nothing counts frames. FixedUpdate and networking catch up with 2 ticks per menu update.

## Verification
- `just lint`: pass (ripgrep on PATH).
- `just test`: pass. The new `tests::battery_flow_settings_keep_matches_continuous_and_menus_reactive_at_30_hz` also asserts that desktop `render_profile_settings(Native)` stays focused Continuous.
- Android cross check: `ANDROID_NDK_HOME=~/Library/Android/sdk/ndk/29.0.14206865 cargo ndk --target arm64-v8a --platform 31 clippy --locked -p brawler-android --all-targets -- -D warnings`: pass.
- APK not built and device not used (the device was busy with another agent).

## Device-check plan (Pixel 8a)
1. Baseline on the pre-change APK, then repeat with this commit (`just android-build HOST:5000 && just android-install`).
2. Dashboard rate: `adb shell dumpsys SurfaceFlinger --timestats -clear -enable`, idle on the Dashboard for 30 s, then `adb shell dumpsys SurfaceFlinger --timestats -dump` and read the app layer's present intervals. Expect about 30 FPS idle (was 60).
3. Power: `adb shell dumpsys batterystats --reset`, unplug or `adb shell dumpsys battery unplug`, idle on the Dashboard for 5 min, then read the app estimate with `adb shell dumpsys batterystats <package>`. Compare with the baseline, then `adb shell dumpsys battery reset`.
4. Match: the BRL-ynt5 scenario must still show a 60 FPS median (timestats).
5. Feel (owner judgement): taps on menu buttons respond without visible delay; slider and scroll drags track the finger smoothly; motifs, entrance/burst animations and the Dashboard preview look smooth enough at 30 Hz. Watch for a stall when entering MatchLoading/Match: the switch to continuous should be immediate.
6. Optional: if timestats show the display still refreshing at 60/120 Hz in menus, consider the 30 Hz surface vote from Decisions.
