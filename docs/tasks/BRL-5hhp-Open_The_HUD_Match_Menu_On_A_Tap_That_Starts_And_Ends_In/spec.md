Reviewed 2026-11-15 against HEAD 3437c0d8: valid but the proposed fix is likely already in place. On paper `TouchControls::track` already turns a same-frame start and end into a Tap, so the miss is probably elsewhere. Write the test first, then diagnose.

# Found
BRL-094c device check 2026-11-15, Pixel 8a, HEAD 6a6d6599, Practice Hot Zone 3v3.

# Observation
- `adb shell input tap 212 90` on the drawn menu button did not open the menu (2 of 2 tries). adb's tap sends down and up within about 1 ms, so both land in the same frame.
- `input swipe 212 90 212 90 150` also failed once.
- `input motionevent DOWN 210 90`, then 200 ms, then `UP`, opened it every time.
- Android Back opened and closed the menu every time.
At the ~30 fps High tier, or when frames drop, a real quick thumb tap can fall inside one frame.

# Code at HEAD (packages/brawler-client/src)
- Bevy 0.19.1 `Touches` (`bevy_input/src/touch.rs` L386-421): a same-frame Started+Ended leaves the touch in `just_pressed` and `just_released`, not in `pressed`.
- `touch/model.rs` `TouchControls::track` (L217-281) adopts touches from `iter_just_pressed()` at `start_position()`. When `get_pressed` is None it falls through to `get_released`, and for `TouchControlKind::Menu` it returns `GestureRelease::Tap` if `layout.menu.contains(knob)` (L263-268). So a same-frame tap should already open the menu.
- `touch/sample.rs` `apply_touch_controls` (L38-150) runs every frame in `RunFixedMainLoop` `BeforeFixedMainLoop` (`session/mod.rs` L170-178). It returns early unless `ClientPlayableGate` is true and the context is `Gameplay` (L58-65). `open_menu` → `crate::apply_pause_request` (`lib.rs` L596-609) toggles Gameplay↔Menu.
- The existing menu tests in `touch/tests.rs` (L395-L463) only cover multi-frame presses.

# Candidate causes to check
1. Double toggle: the same tap is also seen by another pause or dismiss path in the same or the next frame (`input.rs` L331 gamepad/keyboard pause, `presentation.rs` `handle_match_menu_input` L103+ `LatchedScoreboardDismiss` / `pointer.released()`), so Menu flips back to Gameplay.
2. The Bevy UI `Interaction` path: a same-frame touch never gets a UI cursor (`bevy_ui/src/focus.rs` L196-211 uses `first_pressed_position`). Any part of opening that relies on `Interaction` misses it.
3. The event reaches the app split differently than in the test (android-activity batching). Log `TouchInput` phases and frame indices on device for one `input tap`.

# Plan
1. Add `touch/tests.rs` `menu_button_tap_within_one_frame_opens_the_match_menu`: `touch(1, Started, menu.center())` and `touch(1, Ended, menu.center())` before a single `frame()`, then assert `ClientInputContext::Menu` after that frame and after one more frame (no flip-back).
2. If it fails, fix it in `track`. If it passes, build the full HUD + presentation app (`hud/screen_tests.rs` rig) with the same input to find the double toggle, then fix it there.
3. Check the attack and ultimate same-frame taps the same way. They worked on device with 60-80 ms holds. `quick_tap_fires_with_auto_aim_and_without_a_target_keeps_facing` (L211) uses multi-frame.

# Acceptance
- The new same-frame tests pass for Menu (and Attack/Ultimate taps). `cargo test -p brawler-client touch` passes.
- **Pixel 8a device check:** `adb shell input tap` on the menu button opens the menu 5 of 5 times in Practice, at High and at Auto.

Priority: low. Real fingers usually span frames at 60 fps.

Evidence: /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/dc/s-m01.jpg, s-m05.jpg, m07-touch-hold-open.jpg, s-q.jpg.
