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
- Attack: while the finger is outside the dead zone, `aim_axis` = projected direction and `aim_distance` = post-dead-zone magnitude x weapon range for lobbed/splash weapons (same rule as the right stick), so the BRL-2dv1 lane shows while dragging. Release outside the dead zone latches one `PRIMARY_FIRE` with that aim. Release without ever leaving the dead zone is a tap: viewport auto-aim (`ViewportAutoAim`) supplies direction/distance, or facing is kept when nothing is targetable, and one `PRIMARY_FIRE` is latched. Dragging back into the dead zone after aiming and releasing cancels.
- Ultimate: same gesture. While a targeted ultimate is dragged, touch arms `targeted_ultimate` so the violet/Demolition area preview shows, and the distance maps onto the ultimate's maximum range. Release latches one `ULTIMATE` (targeted ultimates bypass the mouse/trigger two-phase confirm; `resolve_targeted_ultimate_input` still strips it when ineligible). Tap = auto-aim at the ultimate's assisted target, or facing/full range when none. Immediate ultimates (Dash, Sentry, Self Cloak) use the drag/auto-aim direction as facing.
- The release aim is held in `PendingLocalActions` until the fixed tick consumes the latch, so a render frame without a fixed tick cannot drop the release direction.
- Multi-touch: each control owns at most one touch id; extra fingers are ignored. Owned touches that disappear from `Touches` without Ended/Canceled (the GameActivity >8-pointer issue) are treated as canceled; Canceled never fires. A touch that starts and ends in one frame is still a tap. At most 8 concurrent touches are considered.
- Gating: touch controls are active only in `ClientInputContext::Gameplay` with the playable gate; leaving gameplay (pause, shell, match end) cancels every gesture without firing. Shell menus keep the BRL-drny press model (activate on release) and existing drag-scroll; nothing changes there.
- Settings: `ClientInputSettings` gains `touch_deadzone` (0.0-0.5, default 0.15) and `touch_sensitivity` (0.0-1.0, default 0.5), persisted with `serde(default)` like BRL-jgj4, shown on the Controls > Advanced page and the pause overlay calibration list.
- Visuals / BRL-3zas seam: the `TouchControls` resource exposes the layout and live stick state (`base`, `knob`, kind, aimed). A minimal placeholder overlay (translucent circles for the ultimate button and active stick base/knob, no animation, so reduced motion is unaffected) appears only after a touch is seen in gameplay; desktop never shows it. BRL-3zas replaces the overlay by reading the same resource.
- Protocol: unchanged (`FighterInput` encoding untouched).
- Deferred: in-match touch pause / Android Back mapping, and fire queuing while the weapon is reloading (a release while not Ready is dropped, like a mouse click).


# Implementation (2026-11-13)
- New `src/client/touch/`:
  - `mod.rs` composes the module.
  - `model.rs` holds the pure layout, the floating sticks, release classification, and touch ownership/`track`.
  - `sample.rs` holds `apply_touch_controls`, which maps touch into `PendingLocalActions`.
  - `overlay.rs` holds the placeholder visuals.
  - `tests.rs` holds the tests.
- `src/client/session/mod.rs` chains `sample_local_input -> apply_touch_controls -> resolve_targeted_ultimate_input` and initializes `TouchControls`. `src/client/mod.rs` adds the overlay plugin for windowed clients only.
- `src/client/input.rs` extracts `targeted_ultimate_range` and `primary_placement_range` from `controlled_gamepad_aim_range`, so touch and gamepad share one range rule (no behavior change).
- Settings: `ClientInputSettings.touch_deadzone` (0-0.5, default 0.15) and `touch_sensitivity` (0-1, default 0.5).
  - Validated, persisted with `serde(default)`, and edited as `CalibrationField::TouchDeadzone`/`TouchSensitivity` under a "TOUCH STICKS" heading on the shell's Controls > Advanced page. Page reset covers them.
  - Deviation from the design note: they are not added to the pause-overlay row list. That overlay is keyboard/gamepad-only, and its fixed 24-row cycle is covered by existing tests.
- `docs/13-player-ux.md` gains a "Touch play" paragraph in the input section.
- Protocol, server, and wire encoding are unchanged.

# Design changes made during verification
- **Release hold (4 fixed ticks).**
  - Finding: with the phone on Wi-Fi, the server logged 662 `server_late_input_mismatch` corrections in about 5 minutes of owner play. A late packet for the only tick carrying a one-tick press loses that action, because authority already simulated the tick.
  - Change: a touch release keeps its button and aim asserted for `RELEASE_HOLD_TICKS = 4` fixed ticks (about one quick mouse click).
  - Why this is safe: authority fires once per Ready weapon (catalog fire cooldowns are 9 ticks or more) and activates ultimates only on a rising edge.
  - Device check: one drag-release consumed exactly 1 of 4 ammo.
- **Aim lead for auto-aimed immediate-ultimate taps.**
  - Finding: Dash reads `Rotation` in `AbilitySet::Activation`, which runs before `GameplaySet::Simulation` applies that tick's `aim_update`. On the device, a Dash tap with the old facing pointed into a wall and did nothing.
  - Change: an auto-aimed tap on an immediate ultimate sends the aim for one fixed tick (tracked on the `Time<Fixed>` clock) before asserting `ULTIMATE`. Targeted ultimates already use `aim_update` for their center, and primary fire runs after movement, so neither needs the lead.

# Verification (2026-11-13)
Focused tests: `cargo test --features client,automation --lib -- touch::` reports 11 passed. They drive synthetic `TouchInput` through `touch_screen_input_system` and the production chain `sample_local_input -> apply_touch_controls -> resolve_targeted_ultimate_input`:
- Zones and settings scaling.
- Floating move stick: full-speed direction, base follow, release to neutral.
- Attack drag: lane aim and lobbed distance, release fires once along the drag, release hold across fixed ticks, then clears.
- Same-frame tap fires with auto-aim fallback.
- Drag back into the dead zone cancels.
- Reveal Scan drag arms the preview and the release latches ULTIMATE with placement; ultimate tap works for targeted and immediate ultimates.
- Two simultaneous touches plus a third ignored finger.
- Platform Canceled and lost (stale) touches never fire.
- Leaving gameplay drops gestures and disarms.
- Aim-lead for auto-aimed immediate ultimates.
- Settings: `settings_saved_before_touch_controls_load_with_default_touch_tuning`.

Canonical runs, all on the final code:
- `just lint`: exit 0. A real ripgrep is put first on PATH through a scratch wrapper, because `rg` is only a shell function here.
- `just test`: exit 0, 2230 passed, 0 failed.
- `just e2e 2`: exit 0, 1 exact 1v1 roster reached Active.
- `just android-build 192.168.31.47:5000`: exit 0, 119 MB APK.

Mid-run problem: one `just test` run failed at link time with "symbol(s) not found". I had killed a cargo test mid-build and corrupted the artifacts. `cargo clean -p brawler` plus a rerun fixed it; this was not a code issue.

# Device evidence (Pixel 8a, serial 45171JEKB03025, Wi-Fi to the Mac server)
Server setup: `BRAWLER_ROUTED_BIND=0.0.0.0:5001 BRAWLER_DEV_DATA_DIR=<scratch>`. Port 5001 was needed because the phone's saved account binding for 192.168.31.47:5000 points at the BRL-zj53 scratch server. Connecting to :5000 failed with "server address is already bound to a different logical server". `data/` and `config/` are unchanged.

What happened:
- The owner created a brawler (Pulse Sidearm + Dash) and entered Practice Wipeout 3v3.
- **The owner then played on the phone themselves** (getevent showed live touchscreen input), so injection was paused.
  - The overlay showed engaged move and attack sticks.
  - The server received PRIMARY_FIRE with aim (84) and without aim (26), and ULTIMATE (3), in its late-input dumps.
  - Two Practice workers returned results.
  - No panics or FATAL entries; only audio one-shot cap warnings.
- Later, on the release-hold build, I drove input with `adb shell input motionevent`:
  - The drag showed a red, blocked aim lane that followed the finger direction.
  - Release fired exactly one shot (ammo 4/4 -> 3/4).
  - The move stick moved the fighter.
  - The match ran to the results screen (8-10).
- An ultimate **tap** while facing a wall did not Dash. That led to the aim-lead fix above.
- The final APK (with the aim lead) is installed and connects. Getting back into Practice to re-check the Dash tap was blocked by DPAD focus on the clipped phone Dashboard (BRL-30za territory), so the Dash-tap fix is covered by a unit test only.

Screenshots (scratch only, not in the repo), under `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/shots/`:
- 27.png, 29.png: the owner in a match with the sticks overlay.
- 51.png: single shot, ammo 3/4.
- 52.png: aim lane while dragging.
- 54.png: move stick.
- 56.png: ultimate tap did not dash (before the fix).
- 57.png: results.

Logs (scratch only): `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/server.log` and `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/logcat-final.txt`.

Observations for other tickets:
- **BRL-g57a:** about 784 `server_late_input_mismatch` corrections over the Wi-Fi session.
- **BRL-30za:**
  - Server Select fields show no values.
  - Touch drag-scroll does not scroll Create Brawler, Choose Game, or the Dashboard at 914x411. They are reachable only with DPAD key events, and Practice sits below the fold.
  - There is no way to forget a saved server binding after the server identity changes.

# Playtest request (owner, Pixel 8a)
Setup:
- Run: `BRAWLER_ROUTED_BIND=0.0.0.0:5001 BRAWLER_DEV_DATA_DIR=<any scratch dir> just server`.
- On the phone, type `192.168.31.47:5001` in Server Address (or reuse your original BRL-zj53 data dir on :5000).
- The installed APK is the final build.

Then play Practice Wipeout 3v3 to the result by touch only, and check:
1. Movement: left half, floating stick, full speed. Does the stick feel too large or small? Adjust under Settings > Controls > Gamepad > Advanced > TOUCH STICKS.
2. Attack: drag on the right half to see the lane, release to fire. Does the lane follow your finger, and does every release fire?
3. Quick tap auto-aims at the nearest visible enemy.
4. Drag back to the center to cancel.
5. Ultimate (violet ring, upper right): drag to aim, release to Dash. Also tap with an enemy on screen: the Dash should now go toward the enemy.
6. If you can, try a targeted-ultimate brawler (Reveal Scan or Fire Field): the violet area should follow your drag.
7. Hold move and attack at the same time; add a third finger.
8. Leave the match and come back.

Known limitations:
- The visuals are placeholders, and the movement base can overlap the HEALTH box (BRL-3zas owns the touch HUD).
- There is no touch pause or Back gesture (BRL-094c).
- A release while the weapon is reloading does nothing, as with mouse fire.

# Learn-from-errors
- **One-tick latches are fragile over real networks.** Desktop tests and localhost e2e never exposed this; device logs did. Action: for touch release actions, assert over a short hold, and check `server_late_input_mismatch` counts on any Wi-Fi playtest.
- **Same-tick aim does not steer immediate ultimates.** `AbilitySet::Activation` runs before movement applies `aim_update`. Action: before using an input as "aim then act" in one tick, check which schedule set reads `Rotation`.
- **Never kill a running cargo build.** It corrupted the incremental artifacts and also deleted `target/debug/brawler-server` while a dev server depended on it. Action: wait for builds, or clean only after stopping dependent servers.
- **The owner may be using the phone live.** Action: check `getevent` activity before injecting input, and do not reinstall over an active session.

# Deferred
- BRL-094c (todo, related, parent BRL-q5c4): open the match menu by touch and the Android Back gesture.


Correction to the playtest setup: the phone is now bound to the logical server in `/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/devdata` for `192.168.31.47:5001`. That directory also holds the brawler "Brawler 1". Use `BRAWLER_DEV_DATA_DIR=/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/devdata` with port 5001. A fresh data dir on :5001 hits the same "different logical server" rejection until saved bindings can be forgotten (BRL-30za observation).


# Closeout (2026-11-14)

Moved to done under the owner's process: implementation and agent verification are complete. Owner playtest happens on done tickets; open decisions above stay recorded for that review, and changes come back as a reopen or a new ticket.

# Device check 2026-11-15 (Pixel 8a)
Setup: Pixel 8a (45171JEKB03025, Android 17), release APK built from HEAD 6a6d6599. A scratch protocol-48 supervisor ran on 0.0.0.0:5001 and the phone reached it over Wi-Fi. Input came from adb. Evidence is under /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/dc/.
- Dash tap re-check, Practice Hot Zone 3v3: an 80 ms tap on the ultimate button with an enemy (Bot 2) to the north. The fighter dashed north toward it with a trail, and the ring went from gold Ready to charging (`s-d0.jpg`). **Pass** (one sample).
- The touch shell reached Practice on every attempt (BRL-30za resolved the old Dashboard blocker).
- `server_late_input_mismatch` (lightyear_inputs ERROR, target `lightyear_debug::input`): **0** in the scratch server log (`server.log`, about 30 min, 8 matches over Wi-Fi). Caveat: the input was scripted and sparse (short drags, ~50 fire taps), much lighter than the owner's 5 min of live play that logged 662.
- Not done: a full touch-only match to the result by hand. It needs the owner's play.
