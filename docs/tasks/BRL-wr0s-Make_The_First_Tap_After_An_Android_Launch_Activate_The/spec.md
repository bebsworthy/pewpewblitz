Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec refreshed. Not fixed. `ui/press.rs` is unchanged since the crate split (dc8302ea). The paths moved to `packages/brawler-client`. Two concrete hypotheses are added below.

# Evidence (BRL-h088 device walk, Pixel 8a, Android 17, 2026-11-14)
- Fresh launch (`am force-stop` + `am start`), Select Server: the first `adb shell input tap` on QUIT only moved focus to QUIT. A second tap on the same spot quit. Seen twice: once on the build before BRL-h088 and once after it (evidence: scratchpad/brl0198/b01-serverselect-before.png, b02-relaunch-after-quit.png, d38-fresh.png). Later taps in the same session activated the first time.
- Not caused by BRL-h088, which made no change to `ui::press` or touch sampling.

# Code at HEAD
- `packages/brawler-client/src/ui/press.rs`: `ReleaseActivation::observe` (L49-77) activates only after it has seen `Interaction::Pressed` and then a release. `track_release_activation` (L109-126) runs in `PreUpdate` after `UiSystems::Focus`. `held.touch` comes from `Touches::first_pressed_position` on the press frame.
- `packages/brawler-client/src/shell.rs` `collect_pointer_actions` (L639-674) moves `InputFocus` on any changed Hovered or Pressed (L659-664). That is the "focus moved" symptom. The action needs `release.activated()` (L666).
- Bevy 0.19.1 `bevy_ui/src/focus.rs` L174-211: the UI cursor is `window.physical_cursor_position()` or `touches.first_pressed_position()`. A touch that starts and ends in one frame is no longer in `pressed`, so it never hovers or presses anything. BRL-rs10 hit the same thing for sliders, and `ui/slider.rs` L158-L195 hit-tests from layout instead. (Test: `a_tap_that_starts_and_ends_within_one_frame_still_sets_the_value`, L361.)

# Hypotheses to check first
1. Long first frames: right after launch, frames are slow (pipeline and shader compilation, asset loads). The adb tap's down and up (~1 ms apart) can then land in one frame, or the press can be seen without the release on the frame where the target exists. Log `TouchInput` phases and frame indices on device for the first tap.
2. Entity replaced between press and release: the Server Select tree is rebuilt on an early frame. A `UiScale` / viewport-fit change in `shell.rs` `preview_shell_preferences` (L917-931) once the real window height arrives, a focus-driven rebuild, or the surface entrance can each do it. The new `ReleaseActivation` has no `held`, so the release is lost while focus stays. Check whether the QUIT entity id changes between the two frames.

# Plan
1. Reproduce on the host. In a layout-harness app (`ui/layout_harness.rs`, 868x411), drive the first ever `TouchInput` Started then Ended on a Server Select button: (a) on separate frames, (b) in the same frame, (c) with a `UiScale` change between them. Assert one activation each time.
2. Fix this in the shared path (`ui/press.rs`), not per screen:
   - For same-frame taps, hit-test `Touches::iter_just_released()` positions against the control's `ComputedNode` (as `slider.rs` does) and activate when start and end are inside and closer than `DRAG_CANCEL_DISTANCE`.
   - If hypothesis 2 is confirmed, keep the press keyed by a stable control identity (`ShellButton.id` / `FlowButton`) across rebuilds, or avoid the rebuild.
3. Keep the drag-cancel and mouse-off-target behaviour.

# Acceptance
- New focused tests in `ui/press.rs` (or `flow/tests.rs`) for the first-ever touch, same-frame and rebuild cases pass. The existing `press.rs` tests (L141-L234), `touch/tests.rs` and the slider tests still pass: `cargo test -p brawler-client press slider touch`.
- **Pixel 8a device check:** after `am force-stop` + `am start`, a single `adb shell input tap` on QUIT (and on CONNECT) on Select Server activates it. Repeat 3 times.

# Verified 2026-11-16 (commit ea2d3608)
- Cause on host: the same-frame theory was wrong, because Bevy 0.19.1 `first_pressed_position` falls back to `just_pressed`. Two real losses were found and fixed in `ui/press.rs`: a finger down before the control is laid out where it ends up, and a control rebuilt between press and release. A same-frame drag now cancels too. Tests: `press::tests::touch::*` (8) and `flow::tests::first_tap::*` (4). `just lint` and `just test` pass.
- Pixel 8a, release build with a temporary TouchInput probe (since removed). After force-stop and launch, one `input tap` on the saved-server row at 1.8, 2.0, 2.2, 2.4, 2.6 and 2.8 s:
  - Every tap that reached the app activated on the first tap and connected (2.4 s onwards).
  - Taps at 1.8–2.2 s never reached the app. `WindowFocusChanged -- 0` fired right after the tap, and `dumpsys input` showed the focused window as `Embedded{}`, a TRUSTED_OVERLAY owned by `com.google.android.gms.ui` (pid 24580) covering the screen. That is an OS / Play Services overlay during launch, not this code. Follow-up: BRL ticket "Investigate the Play Services overlay that swallows taps in the first seconds after launch".
- The original "first tap only moves focus" symptom did not reproduce on 2026-11-16. Taps at 3 s or later activated on the first tap, both before and after the fix.
