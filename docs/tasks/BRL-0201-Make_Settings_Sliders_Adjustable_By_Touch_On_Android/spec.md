# Outcome
Every Settings slider (UI scale, master/effects/music volume, touch tuning) can be set by touch on Android.

# Context (found in BRL-0161, 2026-11-14, Pixel 8a)
`shell/settings_screen.rs::pointer_sliders` only reads `ButtonInput<MouseButton>`; a touch press focuses
the row but never moves the value, and slider rows deliberately skip release activation
(`shell.rs`, "Sliders adjust while held"). On the phone the sliders are therefore read-only.
BRL-0161 sidestepped this for Graphics quality by making it a tap-to-cycle chip.

# Scope
- Drive slider tracks from touch (Bevy `Touches` or picking pointer events) with the same
  defer-save-while-held behaviour as the mouse path.
- Keep desktop mouse/keyboard/gamepad behaviour unchanged.

# Acceptance criteria
- On the Pixel 8a every slider follows a finger drag and saves on release.
- Focused tests for the touch path; desktop slider tests still pass.
