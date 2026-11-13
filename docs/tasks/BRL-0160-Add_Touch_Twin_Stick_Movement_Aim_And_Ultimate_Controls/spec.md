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


# Design (2026-11-13, before implementation)

## Research findings
- `sample_local_input` (`src/client/input.rs`) rewrites `PendingLocalActions` every render frame from keyboard/mouse/gamepad in `RunFixedMainLoop::BeforeFixedMainLoop`, then `resolve_targeted_ultimate_input` converts targeted ultimates. `write_client_input` (FixedPreUpdate) turns `held_buttons | latched_buttons`, `move_axis`, `aim_axis`, `aim_distance` into one `FighterInput` per tick and clears the latch.
- The aim lane (`aim_preview.rs`) and camera aim lead read only `PendingLocalActions.aim_axis`/`aim_distance`; the violet targeted-ultimate preview additionally needs `targeted_ultimate.armed_for`. Touch therefore needs no presentation changes to show the lane.
- Authority fires on any tick with `PRIMARY_FIRE` while the weapon is Ready, using facing that the same input's `aim_update` sets. A one-tick latched press is the existing confirm pattern (targeted ultimate) and survives Lightyear's redundant input history.
- Targeted ultimates read `aim_update` + `aim_distance` (clamped to range, fallback facing at full range). Dash/Sentry/Self Cloak use facing. `ACTIVE_ITEM` has no authoritative consumer, so there is no separate ability button.
- Bevy `Touches` keeps `just_pressed` entries even when a touch starts and ends in the same frame, and positions are logical window pixels (y down). The arena camera has no yaw: screen right = +x, screen up = +y, with perspective foreshortening.
- winit maps the Android Back gesture only to the logical `NamedKey::BrowserBack`; there is no in-match touch pause today.

## Decisions
- New client module `src/client/touch/` (input sampling only): `model.rs` (pure gesture state + layout), `sample.rs` (the system that maps `Touches` into `PendingLocalActions`), `overlay.rs` (minimal placeholder visuals), `tests.rs`. The system runs in the existing chain `sample_local_input -> apply_touch_controls -> resolve_targeted_ultimate_input`, so keyboard/mouse/gamepad code paths are untouched and touch only overrides a channel while it is engaged.
- Layout in logical px, unit `u = min(width, height)` (411 on the Pixel 8a landscape):
  - Top band `y < 0.12h` is reserved for HUD and ignored.
  - Left half: floating movement stick; base = touch-down point, base drags along when the finger exceeds the travel radius.
  - Ultimate button: fixed circle at `(w - 0.62u, h - 0.58u)`, hit radius `0.16u`, checked first in the right half.
  - Rest of the right half: floating attack stick (base = touch-down point), which is more forgiving than a fixed button with no final HUD art.
- Stick travel radius = `u * (0.26 - 0.16 * touch_sensitivity)` (default sensitivity 0.5 -> 0.18u, about 74 px). Dead zone = `touch_deadzone` fraction of travel (default 0.15), never below a 10 px tap slop.
- Movement: outside the dead zone the direction is projected through the unshaken arena view (fighter screen point + finger direction -> ground ray) and sent at full magnitude (genre convention; also avoids interplay with the server's own 0.20 move dead zone). Inside the dead zone move is zero.
- Attack: while the finger is outside the dead zone, `aim_axis` = projected direction and `aim_distance` = post-dead-zone magnitude x weapon range for lobbed/splash weapons (same rule as the right stick), so the BRL-0149 lane shows while dragging. Release outside the dead zone latches one `PRIMARY_FIRE` with that aim. Release without ever leaving the dead zone is a tap: viewport auto-aim (`ViewportAutoAim`) supplies direction/distance, or facing is kept when nothing is targetable, and one `PRIMARY_FIRE` is latched. Dragging back into the dead zone after aiming and releasing cancels.
- Ultimate: same gesture. While a targeted ultimate is dragged, touch arms `targeted_ultimate` so the violet/Demolition area preview shows, and the distance maps onto the ultimate's maximum range. Release latches one `ULTIMATE` (targeted ultimates bypass the mouse/trigger two-phase confirm; `resolve_targeted_ultimate_input` still strips it when ineligible). Tap = auto-aim at the ultimate's assisted target, or facing/full range when none. Immediate ultimates (Dash, Sentry, Self Cloak) use the drag/auto-aim direction as facing.
- The release aim is held in `PendingLocalActions` until the fixed tick consumes the latch, so a render frame without a fixed tick cannot drop the release direction.
- Multi-touch: each control owns at most one touch id; extra fingers are ignored. Owned touches that disappear from `Touches` without Ended/Canceled (the GameActivity >8-pointer issue) are treated as canceled; Canceled never fires. A touch that starts and ends in one frame is still a tap. At most 8 concurrent touches are considered.
- Gating: touch controls are active only in `ClientInputContext::Gameplay` with the playable gate; leaving gameplay (pause, shell, match end) cancels every gesture without firing. Shell menus keep the BRL-0156 press model (activate on release) and existing drag-scroll; nothing changes there.
- Settings: `ClientInputSettings` gains `touch_deadzone` (0.0-0.5, default 0.15) and `touch_sensitivity` (0.0-1.0, default 0.5), persisted with `serde(default)` like BRL-0158, shown on the Controls > Advanced page and the pause overlay calibration list.
- Visuals / BRL-0155 seam: the `TouchControls` resource exposes the layout and live stick state (`base`, `knob`, kind, aimed). A minimal placeholder overlay (translucent circles for the ultimate button and active stick base/knob, no animation, so reduced motion is unaffected) appears only after a touch is seen in gameplay; desktop never shows it. BRL-0155 replaces the overlay by reading the same resource.
- Protocol: unchanged (`FighterInput` encoding untouched).
- Deferred: in-match touch pause / Android Back mapping, and fire queuing while the weapon is reloading (a release while not Ready is dropped, like a mouse click).
