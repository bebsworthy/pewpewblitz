# Evidence (BRL-0198 device walk, Pixel 8a, Android 17, 2026-11-14)
- Fresh launch (`am force-stop` + `am start`), Select Server: the first `adb shell input tap` on QUIT
  only moved focus to QUIT; the second tap on the same spot quit. Seen twice: once on the build before
  BRL-0198 and once after it (evidence: scratchpad/brl0198/b01-serverselect-before.png,
  b02-relaunch-after-quit.png, d38-fresh.png). Later taps in the same session activated first time.
- Not caused by BRL-0198 (no change to `ui::press` or touch sampling).

# Scope
- Find why the first touch after launch yields `Interaction::Pressed` without a release activation
  (`ui::press::ReleaseActivation::observe`), e.g. a missing hovered/touch state on the first frame,
  or the press and release landing before the UI focus pass runs.
- Fix it in the shared release-activation path; add a focused test that drives the first ever
  `TouchInput` of an app.

# Acceptance criteria
- On the Pixel 8a a single tap on any shell control right after launch activates it.
- Existing press/drag-cancel tests still pass.
