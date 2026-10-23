# Outcome

A complete match is playable on a phone with touch only, through the same input intent that keyboard, mouse, and controller use.

# Context

- There is no production touch handling (`TouchPhase` appears only in tests).
- Input is keyboard/mouse/gamepad-centric, and aim reads the mouse cursor (`src/client/input.rs:718-728`).
- The wire input (quantized move axis, aim vector/distance, button bits; `protocol.rs:151-165`) and viewport auto-aim (`client/targeting.rs`) already fit twin-stick touch.

# Scope

- A floating left movement joystick.
- A right attack control: drag to aim with the aim lane preview, release to fire, quick tap to auto-aim fire.
- An ultimate button with drag-to-aim and release-to-activate.
- Multi-touch tracking with a guard for the reported GameActivity >8 touch issue.
- Dead zones and sensitivity in settings.
- Visual controls come from the touch-first HUD.
- Menus: activate on release, with drag-scroll.

# Out of scope

Gyro aim and custom control layouts.

# Acceptance criteria

- A full Practice 3v3 match is playable on a device by touch only.
- The protocol encoding is unchanged.
- Unit tests cover the touch-to-intent mapping, including auto-aim and release-to-fire.

# Verification

- Focused tests.
- A device playtest.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.
