# Outcome
Every shell screen on the phone path has an on-screen way back, and the Android Back gesture means
"back" in the shell instead of finishing the activity.

# Evidence (BRL-30za device walk, Pixel 8a, 2026-11-13)
- Choose Game and Choose Practice Map have no Back control; desktop players use Esc. On the phone the
  only way out is SELECT GAME / START PRACTICE.
- The Android Back gesture finishes `MainActivity`; relaunching then sits on the splash because the
  Bevy app cannot restart in the same process, so the app must be force-stopped.

# Scope
- Add the shared Back surface to Choose Game and Choose Practice Map (same action as Esc/Cancel).
- Route the Android Back gesture (`NamedKey::BrowserBack`, see BRL-094c) to the shell's existing
  Back/cancel action; on the first screen it may ask before quitting. Coordinate with BRL-094c, which
  owns the in-match menu mapping.

# Acceptance criteria
- On the Pixel 8a every shell screen can be left by touch and by the Back gesture without closing
  the app; desktop Esc behavior is unchanged.


# Investigation (2026-11-14, Pixel 8a, Android 17 / API 37, current main)
- `adb shell input keyevent KEYCODE_BACK` on Select Server: no key reaches Bevy. At targetSdk 37
  predictive back is on, so the system runs the Activity's default `OnBackInvokedCallback`, which
  finishes `MainActivity`. The Bevy loop keeps running on its thread against the destroyed activity
  ("Attempted to ... after GameActivity was destroyed" log spam) and the process stays alive.
- Relaunch hang root cause: the new activity starts a second `android_main` in the same process and
  bevy_winit panics with `Failed to build event loop: RecreationAttempt` (winit allows one EventLoop
  per process), leaving the splash up. The in-app QUIT button hits the same hang: `android_main`
  returns, GameActivity finishes, the process lingers, and the next launch panics the same way.
- winit maps Android `KEYCODE_BACK` only to logical `Key::BrowserBack` (physical Unidentified). That
  key path is used only where the platform still dispatches the key event (API 31-32, or a hardware
  keyboard Back key); at API 33+ with the back-invoked dispatcher it never fires.

# Design decisions
1. Shared seam `client::back::BackInput` (public resource) with `BackInputSystems` (PreUpdate, after
   Bevy `InputSystems`). `just_pressed()` is true for one frame when: a keyboard press of logical
   `Key::BrowserBack` / physical `KeyCode::BrowserBack` arrives, or a platform adapter called
   `request()`. Consumers OR it with their existing Escape / controller-East check. Esc stays its own
   key: desktop behavior and the configurable pause binding are unchanged.
2. Shell consumers: flow input (every Escape mapping, including server-field and brawler-name edit
   cancel), Choose Practice Map input, shell Settings/Credits back, controls-capture cancel, and the
   Map Maker library/editor Escape paths.
3. Root screens: the flow publishes `BackInput::set_leaves_app(..)` every frame; it is true only on
   Dashboard or Select Server with no overlay and no field being edited (Esc there is a no-op on
   desktop and stays so). On Android a Back at the root moves the task to the background
   (`moveTaskToBack(true)`), the platform convention for a root activity: nothing is killed, the
   session and screen are kept, and returning resumes. QUIT remains the explicit exit.
4. Android adapter: `MainActivity` registers an androidx `OnBackPressedCallback` (at targetSdk 37 it
   is dispatched through `OnBackInvokedDispatcher`; the manifest sets
   `android:enableOnBackInvokedCallback="true"` so API 33-35 take the same path). The callback calls
   `native boolean nativeOnBackPressed()`: false at the root (Java then moves the task back), else it
   queues a request (atomic counter) and returns true. `brawler-android` drains the counter into
   `BackInput::request()` before `BackInputSystems` and publishes `leaves_app()` to the atomic in
   `Last`. API 31-32 has no dispatcher: GameActivity forwards KEYCODE_BACK to native and winit's
   BrowserBack reaches the same `BackInput`; root Back there does nothing (no JNI call back), noted
   as a limitation.
5. Relaunch hang fix (in scope; same root cause): Back no longer finishes the activity, and
   `MainActivity.onDestroy` ends the process when `isFinishing()` (after QUIT the native loop has
   already returned), so the next launch always starts a fresh process.
6. On-screen Back: Choose Game gets a header `‹ BACK` (shared `Surface::Back`, `CancelGameType`,
   flow button index 1001 on the confirm focus row); Choose Practice Map gets a header
   `PracticeControl::Back` (`CancelPracticeMaps`). Same actions as Esc.
7. Seam for BRL-094c: in-match pause reads `BackInput::just_pressed()`; `leaves_app` is never true
   during Match/Loading/Queue/Results, so on Android every Back in a match reaches Rust. This ticket
   does not change in-match behavior (Back in a match with no overlay still does nothing).

# Verification plan
- Flow tests: Back from every shell screen/overlay maps to the same action as Esc; root predicate.
- Back-input unit tests: BrowserBack key and `request()` each produce exactly one frame of
  `just_pressed`; Escape does not.
- layout_harness: Choose Game and Choose Practice Map Back controls on screen at 914x411 and 1280x720.
- `just lint`, `just test`, `just e2e 2`, native desktop capture, Pixel 8a walk with Back gesture
  (keyevent) and touch Back, QUIT + relaunch.


# Design amendments during implementation
- `publish_back_root` runs in `Last` (not PreUpdate), so the root flag reflects this frame's
  committed flow state and overlay. The Android bridge runs in `First`, after it in frame order,
  which drains the queued presses before `BackInputSystems`, so no cross-crate ordering set is needed.
- Correction to decision 3: going to the background keeps the screen and session only within
  BRL-g57a's suspension bound, because the Bevy loop does not run while the app is in the
  background. The Pixel 8a returned to the connected Dashboard after 3 s, but showed the recoverable
  "lobby connection was lost" panel after about 13 s. Home behaves the same way (checked with Home
  for 20 s), so root Back matches Home. This is the accepted BRL-g57a behavior, not a new gap.
- The styled game-selection test from BRL-3c10 asserted that there was no Back on Choose Game.
  That reference decision is reversed for touch players. The test now asserts exactly one Back
  (`GAME_TYPE_BACK_INDEX`).
- Back focus on Choose Game sits on the confirm row: Right from SELECT GAME reaches BACK. Keyboard
  users still have Esc.

# Implementation
- `src/client/back.rs` (new): `BackInput`, `BackInputSystems`, `add_back_input`, and unit tests.
  Re-exported as `brawler::client::{BackInput, BackInputSystems}`.
- Flow: `flow/input.rs` (Back is ORed into every Escape cancel; `back_leaves_app` and
  `publish_back_root`), `flow.rs` (installs Back input; publishes in `Last`).
  `screens/game_select.rs` adds a header BACK; `screens/practice_maps.rs` adds a header
  `PracticeControl::Back` and Back input. `screens/map_maker.rs` gives Back the same meaning as
  Escape.
- Shell: `shell.rs` (Settings/Credits Back), `shell/controls/capture.rs` (Back cancels capture).
- Android: `packages/brawler-android/src/back.rs` (new: JNI `nativeOnBackPressed`, atomics, `First`
  bridge), `src/lib.rs` (installs it), `MainActivity.java` (`OnBackPressedCallback` ->
  `moveTaskToBack` at the root; `onDestroy` ends the process when finishing), and
  `AndroidManifest.xml` (`android:enableOnBackInvokedCallback="true"`).
- Tests: `ui::layout_harness::{on_screen, tap}` helpers. Flow tests:
  `platform_back_steps_back_from_every_shell_screen_exactly_like_escape` (Choose Game, Choose
  Practice Map, Dashboard menu, Change-server confirmation, Brawlers, Brawler details, Create
  Brawler, Customize Weapon, Delete confirmation; Back moves and matches Esc),
  `platform_back_on_root_screens_leaves_the_app_to_the_platform` (Dashboard and Select Server; an
  address edit is cancelled first), `back_leaves_the_app_only_from_idle_root_screens`, and
  `choose_game_and_practice_map_back_controls_are_on_screen_and_go_back` (at 914x411 and 1280x720
  the Back is fully on screen without scrolling, uses `Surface::Back`, and a real `TouchInput` tap
  returns to the Dashboard). Shell test: `platform_back_closes_settings_and_credits`. back.rs:
  BrowserBack and request produce one frame of Back; Escape does not. Disabling the fix made the
  flow and shell tests fail (confirmed).

# Verification (2026-11-14)
- `just lint` (real ripgrep first on PATH): pass. `just test`: pass (2282 passed, 0 failed).
  `just e2e 2`: pass (1 exact 1v1 roster reached Active).
- `just android-install 192.168.31.47:5001`: built and installed. The JNI symbol
  `Java_com_pewpewblitz_client_MainActivity_nativeOnBackPressed` is exported from the .so.
- Pixel 8a (Android 17), with Back sent by `adb shell input keyevent KEYCODE_BACK` (routed through
  the predictive-back dispatcher at API 37):
  - Before: Back finished the activity, and the relaunch hung on the splash with
    `Failed to build event loop: RecreationAttempt`. QUIT followed by relaunch hung the same way.
  - After, Back did the following on each screen:
    - Select Server root: the task moved to the background with the activity alive and the same
      pid, and it resumed on Select Server.
    - Settings: closed.
    - Address edit: the first Back cancelled the edit; the second moved the app to the background.
    - Connection Lost: dismissed.
    - Dashboard root: background. Resuming after 3 s showed the connected Dashboard.
    - Dashboard menu: closed.
    - Choose Game: tapping BACK after changing the draft to Heist returned to the Dashboard with
      WIPEOUT 2v2 kept; the Back gesture also returned.
    - Choose Practice Map: the Back gesture and touch BACK both returned.
    - Brawlers, Brawler details, Create Brawler, Customize Weapon, Customize Abilities: each
      stepped back one level, ending on the Dashboard.
    - In a Practice match: nothing happened and the app stayed in front (BRL-094c owns this).
  - QUIT ended the process (`has died`), and the relaunch got a new pid with Select Server shown.
- Desktop: the automation client with a local server at 1280x720 and 914x411 rendered the shell
  normally (scheduled captures). Esc is unchanged by construction: no Escape path was removed, and
  the Back-equals-Esc test compares the two. I could not inject desktop input from this session,
  so the desktop Choose Game and Practice BACK were checked through the headless layout and tap test
  at 1280x720, not in a native window.

Evidence (scratchpad, not committed):
/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0198/
(b00-b03 before the fix; d01-d39 device walk after the fix; desktop/1280-*.png and 914-last.png;
lint.log, test.log, e2e.log, android-install.log, server.log).

# Playtest request
Install with `just android-install HOST:PORT` (server: `BRAWLER_ROUTED_BIND=0.0.0.0:PORT just
server`). On the Pixel 8a, use the real edge-swipe Back gesture (not only adb):
- Select Server: Back moves the app to the background; reopen it from the launcher.
- Settings: Back closes it.
- Connect, then on the Dashboard: Back moves the app to the background; return quickly.
- MENU, then Back.
- Choose Game: tap the on-screen BACK and also use the gesture.
- Practice map picker: both BACK and the gesture.
- Brawlers, details, Create, Customize Weapon, and Abilities: one gesture per level.
- QUIT, then relaunch from the launcher.
Report the following:
- any screen where Back closes the app or does nothing;
- whether the predictive-back animation or peek feels wrong;
- whether moving to the background at the root (rather than asking to quit) is the behavior you
  want;
- whether BACK is easy to hit in the header corner.

# Deferred / known limitations
- In-match Back (pause menu) is BRL-094c. It uses the `BackInput` seam recorded in its spec.
- Android 12 (API 31-32) has no back-invoked dispatcher. Back reaches the game as `BrowserBack` and
  works on inner screens, but at a root it does nothing instead of moving to the background.
  Untested; no device available.
- BRL-wr0s (new, todo): on a fresh launch the first tap on a shell control only focused it (QUIT on
  Select Server, seen before and after this change).
- Background longer than about 10 s loses the lobby session (BRL-g57a bound); unchanged.

# Learn from errors
- Mistake: the first root-flag publisher ran in PreUpdate and lagged a flow transition by one to
  two frames; the new test caught it. Prevention: publish state that a platform layer reads at the
  end of the frame (`Last`) and read it at the start (`First`).
- Mistake: the spec first claimed that moving to the background "keeps the session". The device
  showed the BRL-g57a suspension bound. Prevention: check the background and resume path against
  the existing lifecycle tickets before describing it.
- Mistake: adding a required resource to a shared input system broke a focused test app that wires
  `map_maker_input` without the plugin. Prevention: grep for direct test registrations of a system
  whenever its parameters change.
- Lesson: at targetSdk 36 and later, Android Back is not a key event. Handle it at the activity
  dispatcher and bridge it through JNI. The relaunch hang was the native loop's one-per-process
  limit (winit `RecreationAttempt`), not a Back bug.


# Closeout (2026-11-14)

Moved to done under the owner's process: implementation and agent verification are complete. Owner playtest happens on done tickets; open decisions above stay recorded for that review, and changes come back as a reopen or a new ticket.
