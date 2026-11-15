# Outcome
Every Settings slider (UI scale, master/effects/music volume, touch tuning) can be set by touch on Android.

# Context (found in BRL-ynt5, 2026-11-14, Pixel 8a)
`shell/settings_screen.rs::pointer_sliders` only reads `ButtonInput<MouseButton>`; a touch press focuses
the row but never moves the value, and slider rows deliberately skip release activation
(`shell.rs`, "Sliders adjust while held"). On the phone the sliders are therefore read-only.
BRL-ynt5 sidestepped this for Graphics quality by making it a tap-to-cycle chip.

# Scope
- Drive slider tracks from touch (Bevy `Touches` or picking pointer events) with the same
  defer-save-while-held behaviour as the mouse path.
- Keep desktop mouse/keyboard/gamepad behaviour unchanged.

# Acceptance criteria
- On the Pixel 8a every slider follows a finger drag and saves on release.
- Focused tests for the touch path; desktop slider tests still pass.


# Design decisions (2026-11-14, before implementation)
Research: Bevy 0.19 `ui_focus_system` already feeds the first pressed touch into `Interaction` and
`RelativeCursorPosition`, but only while the finger is down, and `pointer_sliders` (settings and
controls pages) gate on `ButtonInput<MouseButton>`, so touch never reaches the value. A
press-time jump (the mouse model) is wrong for touch: every vertical scroll that starts on a slider
would change it.

- **One shared touch-slider gesture in the UI kit** (`src/client/ui/slider.rs`), installed by the
  existing `ReleaseActivationPlugin` beside `drag_scroll_by_touch` (PreUpdate, after
  `UiSystems::Focus`). It publishes at most one `SliderAdjust { owner, fraction, held }` per frame.
  Screen owners (Settings preferences, Controls calibration, UI gallery) keep range, step, units and
  persistence; a kit helper merges the unchanged mouse samples with the touch sample so every owner
  reads one adjustment list.
- **Gesture start:** the first finger goes down on a slider row (its `Interaction` is `Pressed`, so
  Bevy's clipping, z-order and overlay blocking apply) and inside the track's horizontal extent plus
  the knob radius. Touches on the row label do not start it (they only focus the row as before).
- **Horizontal drag adjusts; vertical drag scrolls.** After the shared `DRAG_CANCEL_DISTANCE`
  (12 logical px) the gesture uses the same "mostly sideways" rule as `drag_scroll_by_touch`
  (shared helper): sideways becomes a slider drag that follows the finger (and keeps following if
  the finger drifts off the row); vertical abandons the gesture with no value change while the scroll
  area scrolls.
- **Tap sets the value at the tapped position (jump), on release.** Chosen over step-toward because it
  matches the mouse click model players already know on desktop, the volume/UI-scale steps are coarse
  (a 10% step is ~10 dp of track on the phone) so a tap lands precisely, and step-toward would need up
  to ten taps. Applying on release, not on press, is what keeps a vertical scroll from editing a slider.
  Fine 0.01 calibration steps remain available through drag, keyboard and gamepad.
- **UI-scale mapping:** finger position (logical window px) is multiplied by the window scale factor
  and mapped against the track's laid-out physical rect, which already includes `UiScale`. The track
  rect is captured at touch-down so a live UI-scale change during the drag cannot feed back into the
  mapping (no oscillation while dragging the UI-scale slider).
- **48 dp touch target:** slider rows get a taller minimum height (76 UI px; 76 x 411/640 = 48.8 dp
  at the phone fit with the default 100% UI scale). The touch zone is the row height by the track
  width (~100 dp) plus knob overhang. Other rows' sizes are unchanged (out of scope).
- **Saving:** settings `autosave` keeps deferring while the slider is held by mouse *or* touch and
  saves on release, exactly like the mouse path. Keyboard, gamepad and mouse paths are unchanged.
- Known limitation accepted: if a calibration drag rebuilds the Controls page (first change clears a
  recommendation/undo), the touch gesture ends because its row was respawned; lifting and dragging
  again continues.


## Design amendment (device finding, 2026-11-14)
On the Pixel 8a an instant `adb shell input tap` on a slider track focused the row but left the
value unchanged, while a 120 ms press worked: the finger started and lifted within one frame, so
`Interaction` was never `Pressed` when the gesture looked for it. The gesture now hit-tests the
slider row from layout (row rect, `CalculatedClip` from its scroll area, inherited visibility)
instead of reading `Interaction`, and a touch that starts and ends in the same frame is treated as
a tap immediately. Sliders only exist on the fullscreen Settings overlay and owners already gate on
it, so `Interaction`'s overlay blocking is not needed. Tests cover the same-frame tap and a row
clipped by its scroll area's padding.


# Implementation (2026-11-14)
- `src/client/ui/slider.rs` (new): `SliderTouch` gesture resource and `track_touch_sliders`
  (PreUpdate, after `UiSystems::Focus`, installed by `ReleaseActivationPlugin`); `SliderPointer`
  SystemParam whose `adjustments()` merges the unchanged mouse samples with the touch adjustment.
- `ui/press.rs`: shared `is_sideways` drag classification (also used by `ui/scroll.rs`); plugin
  installs the slider gesture. `ui/controls.rs`: `SLIDER_KNOB_RADIUS`, `SLIDER_ROW_MIN_HEIGHT` (76).
- Owners switched to `SliderPointer`: `shell/settings_screen.rs::pointer_sliders`,
  `shell/controls/model.rs::pointer_sliders`, `ui/gallery.rs::interact`. `autosave` defers while the
  mouse button **or** a slider touch is held.
- `ui/layout_harness.rs`: `touch(app, phase, position)` helper (`tap` reuses it).
- Docs: `docs/13-player-ux.md` (touch slider behaviour), `src/client/ui/README.md` (`slider`).

# Verification (2026-11-14)
- Focused tests (`client::ui::slider::tests`): sideways drag follows the finger, keeps adjusting
  off-row, clamps and finishes on release; tap sets the tapped position on release only and a tap
  on the label does nothing; same-frame tap; vertical drag scrolls the column with no adjustment;
  mapping at UI scale 1.0, 0.642 and 0.514 including knob overhang; slider row >= 48 dp and track
  >= 48 dp at 868x411; rows clipped by scroll-area padding ignore touches outside the clip; mouse
  samples unchanged and merged. Shell end-to-end
  `phone_touch_drag_and_tap_set_sliders_and_save_on_release` (868x411 Audio page: drag sets master
  to 30 with no save while held, saved on release; tap sets music to 50). Existing mouse/gamepad
  slider tests unchanged and passing.
- `just lint` (VS Code ripgrep first on PATH): pass. `just test`: pass (lib 959, network 109,
  performance and others all ok). `just e2e 2`: pass (1 exact 1v1 roster reached Active).
- Pixel 8a (`just android-install 192.168.31.47:5001`, no server needed): Audio master/music by
  drag and effects by 120 ms tap; after the same-frame fix, instant `input tap` sets master (80)
  and music (100); effects by drag (70); UI scale by drag (100 -> 120, layout relayouts live, no
  oscillation) and by instant tap (90, mapped correctly at the new scale); Gamepad > Advanced: a
  vertical drag starting on the Trigger press track scrolled the page and left it at 55%; Touch
  deadzone by drag (15 -> 30%), Touch sensitivity by instant tap (50 -> 80%). Force-stop and
  relaunch: every value persisted (80/70/100, 90% UI scale, 30%/80%). The device was then reset to
  its previous values with the pages' Reset controls and the app force-stopped.
- Evidence: `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0201/`
  (`00`-`19` PNGs, `lint.log`, `test.log`, `e2e.log`, `android-install*.log`).

# Playtest request
Install with `just android-install <mac-lan-ip>:5001`; open Settings from Select Server.
- Audio: drag each volume knob, then tap a few points on each track; leave Settings and relaunch.
- Accessibility: drag UI scale to 140% and back; tap 80%. Does the slider feel stable while the UI
  resizes under your finger?
- Controls > Gamepad > Advanced analog controls: drag Touch deadzone / Touch sensitivity; start a
  vertical scroll on a slider track and confirm the value does not change.
- Report: does a fast flick ever change a value you meant to scroll? Is tap-to-jump precise enough
  for the 1% calibration sliders, or do you want +/- steppers?

# Deferred
- BRL-q148 (todo, parent BRL-q5c4): every other shell row is ~36 dp at the phone fit, and slider
  rows drop below 48 dp when UI scale is below 100%.
- Fine 1% touch adjustment (steppers or relative drag) for calibration sliders: not filed; wait for
  playtest feedback above.
- If the first calibration change rebuilds the Controls page (clearing a recommendation or undo),
  the touch drag ends with the respawned row; lifting and dragging again continues (accepted).

# Learn from errors
- Mistake: the first design gated the touch gesture on the row's `Interaction::Pressed`. Unit tests
  passed because the harness always spans press and release over separate frames; the device's
  instant `adb input tap` (and any very quick tap at a low frame rate) starts and ends inside one
  frame, so Bevy UI focus never reports `Pressed`. Cause: assuming Bevy's focus state is the source
  of truth for touch-down. Prevention: for touch gestures, hit-test from layout and `Touches`
  directly and always include a same-frame start+end test (BRL-hcyk had already recorded this
  rule for gameplay touches).
- Mistake: the new required resource broke a gallery test that builds its own `App`; caught by
  `just test` (the focused filter did not include the gallery). Prevention: when adding a
  `Res<T>` to a shared helper, grep every consumer's hand-built test apps.
- Clippy `too_many_arguments` on the Controls owner was resolved by a real `SliderPointer`
  SystemParam that every owner now uses, not an allow.


# Closeout (2026-11-14)

Moved to done under the owner's process: implementation and agent verification are complete. Owner playtest happens on done tickets; open decisions above stay recorded for that review, and changes come back as a reopen or a new ticket.

# Device check 2026-11-15 (Pixel 8a)
Setup: Pixel 8a (45171JEKB03025, Android 17), release APK built from HEAD 6a6d6599. A scratch protocol-48 supervisor ran on 0.0.0.0:5001. Evidence is under /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/dc/.
- Settings -> Audio: a finger drag on Music moved it 70% -> 10%. A tap on the Master track set 40%. Reset this category restored 100/100/70 (`s-s03.jpg`, `s-s05.jpg`). **Pass.**
- Not re-checked: persistence across a relaunch, UI scale and touch-tuning sliders (rs10's own walk covered them).
