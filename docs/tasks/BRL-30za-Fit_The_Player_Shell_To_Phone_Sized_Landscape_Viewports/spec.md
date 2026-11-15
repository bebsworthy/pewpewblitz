# Outcome
Shell screens (Server Select, Dashboard, Brawlers, Create Brawler, errors) fit a landscape phone viewport.

# Evidence (BRL-zj53 device smoke, 2026-11-13, Pixel 8a, 2400x1080 at density 420, ~914x411 logical)
- Dashboard "Create your first brawler": the fighter preview is clipped at the top and the layout runs
  off the bottom.
- "Connection lost" error panel: the Retry button is cut off at the bottom edge.
- Server Select: the Server Address and Player Name rows show only their labels; the prefilled values
  were not visible, although Connect used the prefilled address.
- Screens were designed around a 16:9 desktop window; phone aspect ratios are about 20:9 and the logical
  height is small.

# Scope
- Choose a UI scale or layout rule for small logical heights (respecting the existing UI-scale setting).
- Fix the Server Select value rendering and the clipped Dashboard and error layouts.
- Respect display-cutout safe areas together with BRL-06pz if that ticket has not landed yet.

# Acceptance criteria
- Device screenshots of each shell screen with no clipped controls and visible field values; desktop
  layout unchanged at 1280x720 and above.

# Priority (2026-11-13)
- Owner feedback on the Pixel 8a: the Dashboard and most shell pages are not sized for the phone and their buttons are unreachable, which blocks creating a brawler and entering Practice on Android (BRL-zj53 playtest, BRL-hcyk touch testing). Moved to the top of todo; scope must cover every shell page on the brawler-creation → Dashboard → Practice path at ~914x411 logical px, not only Select Server and the error panel.


# Design (2026-11-13, implementation agent)
Research: the Pixel 8a runs the app full screen at 2400x1080 physical, density 420, so Bevy sees a
914x411 logical window. Landscape (ROTATION_90) reports a 121 px (46 dp) display-cutout inset on the
camera side; system bars are hidden (immersive). A desktop client at `--window-size 914x411`
reproduces the phone layout exactly (same logical size), so desktop captures are a fast proxy.
Causes found:
- Every shell layout is authored for >=600 logical px of height; at 411 px fixed-size cards, logos and
  rows overflow, and several screens switch to their narrow "compact" variant (width < 1000).
- Touch has no wheel: every scroll area only had mouse-wheel and focus-reveal handlers, so content
  below the fold was unreachable by touch.
- Server Select value text used `min_width: 0` with no basis/growth, so flex layout collapsed it to
  zero width on every platform (reproduced in a headless layout test).
- An address bound to an earlier logical server rejects a fresh server at the same address and the
  shell offered no way to clear that binding.

Decisions:
1. Viewport fit (`src/client/ui/viewport.rs`): effective `UiScale` = player UI-scale setting x
   clamp(logical_height / 600, 0.5, 1). Windows >= 600 logical px tall (1280x720 and above, and the
   900x600 compact gallery) are unchanged; the phone renders the authored layouts on a 1334x600
   virtual canvas (fit 0.685), which selects the existing wide layouts. Applied where the shell
   already sets `UiScale` (`preview_shell_preferences`), so shell, overlays and HUD share it; touch
   sticks already divide by `UiScale`.
2. Generic touch drag-scroll (`ui::scroll::drag_scroll_by_touch`, installed with the shared
   release-activation plugin): once a finger travels past the same 12 px `DRAG_CANCEL_DISTANCE`
   that cancels a press, the innermost overflowing `Overflow::scroll_y` node under the touch origin
   follows the finger (clamped). Presses under a drag stay cancelled; taps still activate. No
   per-screen touch code.
3. Server Select: value text grows into the row beside its label (and no longer repeats the label);
   each recent server row gets a FORGET action that removes the recent entry, any favorite, and the
   address's binding to a logical server (the old account id is kept for that logical server).
4. Display cutout: Android 15+ forces edge-to-edge, so `MainActivity` pads the content view by the
   display-cutout insets (black strip); the Bevy window then lies entirely in the safe area. The
   other BRL-06pz items (soft keyboard, signing, icon) stay there.
5. Tests: a test-only headless layout harness (`ui::layout_harness`) lays out real screens at
   914x411 and asserts every visible control is on screen or inside an on-screen vertical scroll
   area it fits; touch-scroll/tap tests drive real `TouchInput` messages.


# Design amendments during implementation
- Fit reference raised from 600 to 640 logical px: the Dashboard's Wide layout needs an effective
  height >= 640 (`DASHBOARD_COMPACT_HEIGHT`), and at 600 the phone still got the stacked Compact
  Dashboard. Fit is now clamp(height / 640, 0.5, 1); the phone (868x411 after the cutout strip) runs
  at 0.642 on a ~1352x640 canvas. Windows >= 640 tall are unchanged; desktop windows 600-639 tall
  (for example the 900x600 compact gallery) now shrink slightly (0.94).
- World-anchored UI assumed `UiScale == 1` (a latent bug for any non-100% UI-scale setting): the
  fighter overheads, floating damage numbers, damageable-object health bars and the map-editor
  selection toolbar now convert camera viewport positions into UI space (`ui::viewport::viewport_to_ui`).
- The shell's Settings focus reveal re-clamped the focused row every frame, which snapped touch
  drags back; it now reveals once per focus change (like the flow screens already did).
- Touch drag-scroll ignores mostly sideways gestures (sliders, carousels).

# Verification (2026-11-13)
- Focused tests (new): `ui::viewport` fit/viewport conversion; `ui::layout_harness` (headless real UI
  + text layout); `ui::scroll::tests::touch` (drag scrolls the area and cancels the press; tap
  activates without scrolling); `flow::tests::server_select_fits_a_landscape_phone_and_shows_field_values`
  (failed before the fix: value width 0) incl. Connection Lost panel and Connecting;
  `flow::tests::player_path_screens_keep_every_control_reachable_on_a_landscape_phone` (Dashboard is
  Wide; Brawlers, Brawler details, Create Brawler, Choose Game, Practice maps; fails with the fit
  disabled); `shell::tests::settings_and_controls_fit_a_landscape_phone` (Display/Audio/Accessibility/
  Controls reachable, shell applies the fit from the window, a scrolled focused row is not snapped
  back; fails with the old per-frame reveal); `connection_persistence` forget/unbind test.
- `just lint` (real ripgrep first on PATH): pass. `just test`: pass (2241 passed, 0 failed).
  `just e2e 2`: pass. `just android-build 192.168.31.47:5001`: pass; installed on the Pixel 8a.
- Desktop: 1280x720 Create Brawler capture after the change matches the pre-change capture;
  914x411 desktop proxy shows the wide Create Brawler. UI gallery captures (compact 900x600 and the
  new `BRAWLER_UI_GALLERY_PHONE=1` 914x411) produced.
- Pixel 8a walk (adb taps/swipes, screencap each screen): Select Server (values visible, FORGET,
  touch scroll to SETTINGS/QUIT, swipe did not activate), FORGET of the stale :5000 binding,
  Settings Display and Controls (touch scroll), Connect -> Wide Dashboard (Practice visible),
  Menu overlay, Brawlers, Create Brawler, Choose Fighter, create "Brawler 2" -> details, Customize
  Weapon (swipe over a slot focused but did not activate), Select for Play, Choose Game, Choose
  Practice Map, Match Loading, in-match HUD with touch sticks and correctly anchored overhead, Results
  (first run), back to Dashboard, Connection Lost panel. The camera-cutout strip is black and no
  control sits under it.

Evidence (scratchpad, not committed):
/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/brl0196/
(p01-launch, p02-serverselect-scrolled, p03-forgot-5000, p04-settings, p05-controls,
p06-controls-scrolled, p11-dashboard, p12-brawlers, p13-create, p14-fighter-picker, p17-after-create,
p18/p19-weapon-parts, p20-after-select, p21-choose-game, p26-menu, p27-practice-maps, p33-loading
(connection lost), p36-loading, p37-match, p31-now (results), p32-dashboard-after-results),
.../scratchpad/desk/{base-1280,after-1280,base-phone,after-phone}/ and .../scratchpad/gallery/.

# Known limitations / deferred
- BRL-h088: Choose Game and Choose Practice Map have no on-screen Back (desktop uses Esc), and the
  Android Back gesture finishes the activity (relaunch then hangs on the splash until force-stop).
- BRL-zy4b: after a Practice match the lobby reconnect timed out over Wi-Fi and the client stayed on
  MATCH COMPLETE with no error (pre-existing flow gap).
- The cutout is handled by padding the Android content view (black strip on the camera side);
  in-app safe-area layout remains with BRL-06pz if a full-bleed look is wanted.
- A failed identity binding still shows the generic connection error; it does not point at FORGET.
- Text on the phone is ~64% of desktop size; readability is a playtest question (UI scale up to
  140% compensates, at the cost of the Compact Dashboard).

# Playtest request
Install with `just android-install HOST:PORT` (server: `BRAWLER_ROUTED_BIND=0.0.0.0:5000 just
server`). On the Pixel 8a walk: Select Server (values, FORGET, scroll) -> Connect -> Dashboard ->
View Brawlers -> Create Brawler -> pick parts -> Create -> Customize Weapon -> Select for Play ->
Choose Game -> Practice -> map -> match -> results -> Dashboard; plus Settings > Controls (TOUCH
STICKS sliders). Report: anything clipped or unreachable, text too small to read, any scroll that
fights a tap or a tap that fires during a scroll, and whether the black cutout strip is acceptable.
Also check one desktop window at the default size for regressions.

# Learn from errors
- Mistake: chose the 600 px fit reference from one screen (Create Brawler) before checking every
  screen's own breakpoint; the Dashboard's 640 px Compact threshold showed up only on the device.
  Prevention: grep the existing responsive thresholds before picking a global reference.
- Mistake: first layout tests passed vacuously (headless nodes were never "visible", and the shell
  focus test focused a control outside the scroll area). Prevention: every reachability helper now
  asserts it checked at least one control, and each new test was confirmed to fail with its fix
  reverted.
- Lesson: a global UI scale exposes every place that mixes camera viewport pixels with UI pixels;
  search `world_to_viewport` whenever `UiScale` semantics change.


# Closeout (2026-11-14)

Moved to done under the owner's process: implementation and agent verification are complete. Owner playtest happens on done tickets; open decisions above stay recorded for that review, and changes come back as a reopen or a new ticket.

# Device check 2026-11-15 (Pixel 8a)
Setup: Pixel 8a (45171JEKB03025, Android 17), release APK built from HEAD 6a6d6599. A scratch protocol-48 supervisor ran on 0.0.0.0:5001. Evidence is under /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/dc/.
- These screens fit the 2400x1080 landscape viewport with every control reachable and values visible: Select Server (scrolls to SETTINGS/QUIT and the saved list), Create Brawler, Brawlers, Brawler details, Customize Weapon, Customize Abilities, Choose Ultimate (second row below the fold), Dashboard, Dashboard menu, Choose Game, Choose Practice Map, Change Server confirm, Leave Match confirm, Settings (Display/Audio), Connection Lost, Connection Error, the Queue screen and Results.
- **Pass.** Evidence: `01-launch.jpg`, `05-connect.jpg`, `s-h02.jpg`, `s-h05.jpg`, `h09-dashboard.jpg`, `s-h10.jpg`, `u01.jpg`, `s-n11.jpg`, `r05.jpg`, `k03.jpg`, `z11.jpg`.
