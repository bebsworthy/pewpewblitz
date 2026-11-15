# Outcome

A phone player can open and close the in-match menu without a keyboard or controller.

# Context

- BRL-hcyk added touch movement, attack, and ultimate controls. The match menu still opens only from the keyboard pause binding or the controller Start button.
- winit maps the Android Back gesture only to the logical `NamedKey::BrowserBack`. Its physical key is unidentified, so the existing `KeyCode` pause binding never sees it.
- The touch HUD (BRL-3zas) owns where on-screen controls go. A pause control belongs in the top band that BRL-hcyk reserves for the HUD (`y < 0.12h`), which touch controls ignore.

# Scope

- Map logical `BrowserBack` to the same pause/cancel request as the keyboard pause binding, on every client.
- Add a pause control to the touch HUD. Coordinate placement with BRL-3zas.
- In the match menu, Back closes the menu. It does not leave the match.

# Out of scope

Shell-screen Back navigation, unless it needs the same key handling.

# Acceptance criteria

- On the Pixel 8a, both the Back gesture and the on-screen control open and close the match menu during Practice.
- Desktop pause bindings are unchanged.
- A focused test covers the BrowserBack mapping.


# Shared Back seam (from BRL-h088)
- BRL-h088 adds `client::back::BackInput` (PreUpdate `BackInputSystems`). `just_pressed()` covers
  the logical `Key::BrowserBack` key (API 31-32 and hardware keyboards) and Android's predictive
  back callback (`MainActivity` `OnBackPressedCallback` -> JNI `nativeOnBackPressed` ->
  `BackInput::request()`); at targetSdk 37 the gesture never arrives as a key event, so map pause
  from `BackInput::just_pressed()` rather than from the KeyCode binding or `NamedKey::BrowserBack`.
- `BackInput::leaves_app()` is published by the shell flow and is false outside Dashboard / Select
  Server roots, so during a match Android always forwards Back to Rust. BRL-h088 leaves in-match Back
  unhandled (except Back on the leave confirmation = Keep Playing, same as Esc).


# Design decisions (recorded before implementation)
- Reuse the existing match menu (`client/presentation.rs` `handle_match_menu_input` +
  `presentation/match_menu.rs`: RESUME / SETTINGS / SCOREBOARD / LEAVE MATCH, "Match continues").
  It is purely client-local `ClientInputContext::Menu`; the server-hosted Practice simulation keeps
  running. No server pause is added.
- Back mapping: `sample_local_input` treats `BackInput::just_pressed()` exactly like the keyboard
  pause binding (Esc): it toggles Gameplay <-> Menu through `apply_pause_request` and sets
  `cancel_pressed`. Every existing Esc consumer therefore behaves the same for Back: the latched
  scoreboard returns to the menu, Back on Leave Match confirmation = Keep Playing (flow, from
  BRL-h088), Back in match Settings closes Settings back to the menu (shell, from BRL-h088), and a
  rebind capture still swallows it. Desktop KeyCode/gamepad pause bindings are untouched.
- On-screen control: a touch-mode-only menu button (the shell `menu` pictogram on the HUD plate
  ink) in the top-left corner. Its square is pure geometry in `hud/layout.rs`
  (`HudControlLayout::menu`, window px scaled by the same control unit as attack/ultimate, >= 48 dp
  on the Pixel 8a) so the drawn button and the touch hit area cannot drift.
- Touch ownership: the touch sampler owns the button like the attack/ultimate buttons (not a Bevy
  UI `Button`). `TouchLayout::classify` returns a new `TouchControlKind::Menu` for touches that
  start inside the square (checked before the move/attack zones), so that finger never feeds the
  movement or aim sticks. Releasing that finger inside the square opens the menu via
  `apply_pause_request`; releasing outside or a platform cancel does nothing. This is per-finger
  correct under multi-touch, which Bevy UI `Interaction` + `ReleaseActivation` (first-touch
  position) is not.
- Shown only in touch mode (Android, or after a touch reached gameplay), like the touch sticks:
  on desktop a UI click would also be sampled as primary fire, and Esc already opens the menu.
- Zombie teammate strip (top-left) moves right of the menu button while the button is shown, so
  `screen_tests.rs` keeps every HUD element collision-free at 868x411, 914x411, 1280x720, 1560x720.
- Closing: Back toggles the menu closed; touch closes it with RESUME (existing release-activated
  menu button, about 56 px tall on the phone). The HUD button opens only.


# Scope addition found in the device walk (recorded before the fix)
- Choosing SCOREBOARD from the match menu latches the scoreboard text panel. Only cancel
  (Esc / controller cancel / now Back) returned to the menu, so a touch-only player had no on-screen
  way back. Fix: while the scoreboard is latched, a completed tap or click anywhere also returns to
  the menu (the release that chose SCOREBOARD is consumed in that frame and does not count). The
  scoreboard panel's own redesign stays with BRL-wfce/BRL-3zas follow-ups.


# Implementation
- `src/client/input.rs`: `sample_local_input` ORs `BackInput::just_pressed()` into the pause
  toggle and `cancel_pressed` (same as the keyboard pause binding).
- `src/client/hud/layout.rs`: `HudControlLayout::menu` square (0.12 control units, 0.025 inset;
  49 px on the Pixel 8a, 58 px at 720 p).
- `src/client/touch/model.rs` / `sample.rs`: `TouchControlKind::Menu` claims touches that start in
  the square (before the move/attack zones); release over the square opens the menu through
  `apply_pause_request`; slide-off or platform cancel does nothing.
- `src/client/hud/controls.rs`: `ControlPart::MenuButton` (plate ink, `menu` pictogram), shown in
  touch mode with the controls group. `src/client/hud/plate.rs`: `place_team_strip` keeps the
  Zombie teammate strip right of the button while it is shown.
- `src/client/presentation.rs`: a finished tap or click dismisses the latched scoreboard back to
  the menu (`LatchedScoreboardDismiss`).
- `docs/13-player-ux.md`: Back mapping, HUD menu button, scoreboard dismissal.

# Verification (2026-11-14)
- Focused tests: `platform_back_opens_and_closes_the_match_menu_like_the_pause_key` (BrowserBack /
  platform request toggles Gameplay <-> Menu, sets cancel, ignored in Shell);
  `menu_button_tap_opens_the_match_menu_without_feeding_a_stick` (with a movement finger held; the
  menu finger never moves or aims; sticks work again after closing);
  `menu_button_released_off_the_button_does_nothing`; layout tests (menu on screen, top-left,
  >= 48 px on the phone); `screen_tests.rs` now includes the menu button in the no-overlap set and
  checks drawn square == touch square at 868x411, 914x411, 1280x720, 1560x720 for Wipeout and
  Zombie (strip); the latched-scoreboard test now also covers dismissal by a touch release.
- `just lint` (VS Code ripgrep 15.0.0 first on PATH): exit 0.
- `just test`: exit 0, 2285 passed, 0 failed.
- `just e2e 2`: exit 0, 1 exact 1v1 roster reached Active (run before and after the scoreboard fix).
- Desktop (automation client, local routed server, Practice Wipeout 2v2): 1280x720 capture shows
  the unchanged desktop HUD (no menu button, LMB/E hints). The 914x411 scheduled captures are
  entirely black on this Mac both with these changes and on stashed baseline main; recorded on
  BRL-fgzg (owner of black scheduled captures). Esc could not be injected (System Events has no
  accessibility permission for this session), so desktop Esc was not exercised natively; it is
  unchanged by construction (the KeyCode pause path was not touched) and covered by the existing
  pause tests.
- Pixel 8a (45171JEKB03025, `just android-install 192.168.31.47:5001`, server on 0.0.0.0:5001 with
  scratch devdata), Practice Wipeout 2v2 Feature Yard:
  - menu button drawn top-left right of the cutout, clear of plate, kill feed, sticks and buttons;
  - Back (keyevent) opened the menu, Back closed it;
  - touch tap on the button opened the menu, RESUME closed it, the movement stick then drove the
    fighter (knob visible, camera moved);
  - a drag that started on the button and slid into the move half neither moved nor opened;
  - Back from latched SCOREBOARD -> menu; SETTINGS -> Back -> menu; LEAVE MATCH -> Back = Keep
    Playing (gameplay); touch menu -> LEAVE MATCH -> LEAVE MATCH returned to the Dashboard;
  - after the scoreboard fix: touch menu -> SCOREBOARD -> tap anywhere -> menu -> RESUME -> stick;
    Back -> LEAVE MATCH -> confirm -> Dashboard;
  - Zombie co-op Practice: the teammate strip sits right of the menu button with no overlap; Back
    menu -> leave worked. Game type restored to Wipeout 2v2 afterwards.
- Evidence (scratchpad, not committed):
  /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0197/
  (p03 HUD with button, p04/p05 Back open/close, p06/p07 touch open/resume, p08/p09 stick after
  resume, p10/p11 drag-off, q02-q10 scoreboard/settings/leave chain, z04 Zombie strip, s04-s09 tap
  dismissal and leave; desktop/1280x720 and desktop/914x411 + base-914x411; lint.log, test.log,
  e2e.log, android-install.log, server.log).

# Playtest request
- Run: `BRAWLER_ROUTED_BIND=0.0.0.0:5001 just server`, `just android-install <mac-ip>:5001`, then
  Practice (any mode).
- Try: tap the top-left menu button mid-fight while still moving with the left thumb; swipe Back
  mid-match; Back again to resume; SCOREBOARD then tap to return; Leave Match.
- Please report: whether the button is easy to hit without catching it while moving, whether its
  size and corner feel right next to the cutout, whether Back-to-resume (rather than Back = leave)
  is what you expect, and whether the Zombie teammate strip still reads well beside the button.

# Deferred / known limitations
- BRL-ejag (new, todo, parent BRL-q5c4, related BRL-094c): the latched scoreboard is still the old
  text panel and has no visible close control (tap-anywhere and Back work).
- The match menu does not pause the server-hosted Practice simulation (existing, documented).
- Desktop 914x411 scheduled captures are black on this machine; noted on BRL-fgzg.
- Desktop shows no menu button (Esc/Start remain); a touchscreen laptop gets the button once a touch
  reaches gameplay, and a mouse click on it there would also be sampled as primary fire.
- adb's server repeatedly died between commands on this Mac (restarted automatically); environment
  only.

# Learn from errors
- Mistake: the first device walk ran a long scripted sequence across a match end, so later taps
  landed on Results/Dashboard. Prevention: on device, check state after each step (or keep scripted
  chains short) and start a fresh match before multi-step menu checks.
- Mistake: I first assumed every menu action was touch-reachable; the latched scoreboard only left
  on cancel. Prevention: when making a menu touch-reachable, walk every sub-state it can enter and
  confirm a touch exit for each, not just the top level.
- Mistake: the first desktop run passed `--product-practice-smoke` without a render report and a
  System Events key injection that hung on permissions. Prevention: copy the windowed flags from
  `scripts/v3-render-evidence.sh` and do not rely on osascript input in this environment.
- Lesson: a full-screen Bevy UI `Button` with `ReleaseActivation` is the wrong tool for a HUD
  control that coexists with touch sticks (first-touch pointer, mouse also sampled as fire); owning
  it in the touch sampler with layout-shared geometry keeps it per-finger and drift-free.


# Closeout (2026-11-14)

Moved to done under the owner's process: implementation and agent verification are complete. Owner playtest happens on done tickets; open decisions above stay recorded for that review, and changes come back as a reopen or a new ticket.

# Device check 2026-11-15 (Pixel 8a)
Setup: Pixel 8a (45171JEKB03025, Android 17), release APK built from HEAD 6a6d6599 (`scripts/android.sh build 192.168.31.211:5001`). A scratch protocol-48 supervisor ran on 0.0.0.0:5001 with a scratch data dir; the phone reached it over Wi-Fi. Input came from adb (`input tap`/`motionevent`/`keyevent KEYCODE_BACK`). Evidence is under /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/dc/.
- Back gesture in Practice Hot Zone 3v3: opened and closed the menu repeatedly (`s-m01.jpg`). **Pass.**
- Touch: a press of ~200 ms on the HUD menu button opened the menu (`s-q.jpg`), and RESUME/SETTINGS/LEAVE MATCH respond to touch. **Pass.** An instant adb `input tap` (down/up in one frame) did not open it. Filed as low priority: **BRL-5hhp**.
- Zombie 3p: the teammate strip sits right of the menu button with no overlap (`z03.jpg`).
- **Regression:** LEAVE MATCH -> LEAVE MATCH no longer reaches the Dashboard. The BRL-yyeg seat hold auto-rejoins the same match, up to 3 times (`s-l04.jpg`, `s-l06.jpg`). Filed **BRL-8y1f**.
- Defect found while here: match-menu Settings is stuck after the match ends. Filed **BRL-nqnm**.
