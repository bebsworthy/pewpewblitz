Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec stale on paths. The UI kit moved to `packages/brawler-client/src/ui/`. Many shell buttons outside `spawn_control` are also under 48 dp.

# Context (found in BRL-rs10, 2026-11-14, Pixel 8a; re-verified at HEAD)
- Phone fit: `packages/brawler-client/src/ui/viewport.rs` `viewport_fit` = logical height / 640, clamped to 0.5..1. `shell.rs` `preview_shell_preferences` (L917-931) sets `UiScale = settings.ui_scale × fit`. A Bevy logical px on Android is 1 dp, so on the Pixel 8a (868x411 logical after the cutout pad) 1 UI px = 0.642 dp at 100 %.
- `ui/controls.rs` `spawn_control` (L110-136): slider rows use `SLIDER_ROW_MIN_HEIGHT = 76` (L57), which is 48.8 dp. Every other row (toggles, binding chips, action rows, category tabs) uses `min_height: 56`, which is about 36 dp.
- UI scale range: `settings/persistence.rs` L14-15, `MIN_UI_SCALE = 0.8`, `MAX_UI_SCALE = 1.4`. At 80 % slider rows are ~39 dp and other rows ~29 dp.
- Other shell buttons below 48 dp at the phone fit (min_height UI px → dp): `flow/screens/shared.rs` L128 (44 → 28), `overlays.rs` L55 (44), `queue.rs` L133 (42), `dashboard.rs` L962 (non-Play rows 42) and L1208 (48 → 31), `server_select.rs` L68 (72 → 46), `map_maker.rs` L1827/L3413/L3481/L3576 (54/50/50/38).
- Existing phone reachability tests (BRL-30za / BRL-0196): `shell.rs` `settings_and_controls_fit_a_landscape_phone` (L1111), credits (L1180), L1218; `flow/tests.rs` `PHONE_VIEWPORT = (868, 411)` (L3484), `server_select_fits_a_landscape_phone_and_shows_field_values` (L3501), `player_path_screens_keep_every_control_reachable_on_a_landscape_phone` (L3570); `ui/layout_harness.rs` `unreachable_buttons` (L88); `hud/screen_tests.rs` VIEWPORTS.

# Plan
1. Policy (record it on the ticket): a touch-platform minimum target of 48 dp, expressed in UI px as `48 / UiScale` and applied as a floor on top of authored heights. Desktop is unchanged. The floor holds at every UI-scale setting, including 80 %. Decide whether "touch platform" means `cfg(target_os = "android")` or "any touch seen"; prefer a resource the tests can set.
2. Apply it in the shared kit. Add a `ui::controls` / `ui::surface` helper (for example a `TouchTarget` component or a `min_touch_height()` used by `spawn_control` and the `Surface::Row` / button builders). A PostUpdate system raises `Node.min_height` from the current `UiScale`, so screens do not each hard-code it.
3. Move the per-screen buttons listed above onto the helper. Re-run the reachability tests and fix any overflow, for example tighter vertical padding or scroll.

# Acceptance
- A new layout-harness test walks Settings (every category), Server Select, Dashboard, Game Select, Queue, Results and overlays at 868x411. With touch mode on, it asserts every visible, enabled `Button` has a computed height ≥ 48 dp at the default UI scale and at `MIN_UI_SCALE`.
- The existing phone reachability tests listed above and `cargo test -p brawler-client` pass. The desktop 1280x720 layouts do not move (existing desktop layout tests pass).
- **Pixel 8a device check:** screenshots of Settings (Display/Audio/Controls), Server Select and Dashboard at 100 % and 80 % UI scale. Every row can be tapped comfortably and none is clipped.

# Decisions (2026-11-18, implementation)
- **Touch platform** means the Android build, `cfg!(target_os = "android")`. It is the default of the `ui::touch_target::TouchTargetPolicy { enabled }` resource, and tests set the resource directly. "Any touch seen" was rejected for two reasons. A touchscreen laptop would reflow the shell mid-session. And desktop layouts must stay fixed. (The HUD's `touch_mode` is different: its touch visuals can appear later, but shell geometry should not jump.)
- **Floor**: on a touch platform, each `TouchTarget` gets `Node.min_height = max(authored, 48 / UiScale)` UI px. One logical px is one dp on Android, and `UiScale` is already the setting times the viewport fit. So the floor holds at every UI scale, including `MIN_UI_SCALE` 0.8 (about 93 UI px at the phone fit). An `Auto` minimum becomes the floor; relative minimums are left alone. `apply_touch_target_floor` runs in `PostUpdate` before `UiSystems::Layout`. It remembers the authored value, so owners that rewrite nodes (the Dashboard layout runs in `Update`) keep working, and turning the policy off restores authored heights exactly.
- **Shared adoption instead of per-screen edits**: `TouchTarget` is a required component of `FlowButton` (every product-flow screen and overlay), `ShellButton` (Settings rows, tabs, bindings, sliders, Credits, shell overlays), `MatchMenuButton` and `ScoreboardCloseButton`. Practice-map controls (`PracticeControl`, no `FlowButton`) insert it explicitly. No screen hard-codes 48 dp. Authored desktop heights (`spawn_control` 56/76, `shared.rs`, `queue.rs` 42, `dashboard.rs` 42/48, `server_select.rs` 72, …) are unchanged, and the floor raises them only on touch.
- `add_touch_targets(app)` (installed once, like `add_release_activation`) is called from `ClientFlowPlugin`, `ClientShellPlugin`, `ClientPresentationPlugin` and `ClientReadinessHudPlugin`.
- **In-match HUD**: trivially covered. The match menu buttons (88 px tall, 52 px min) and the scoreboard BACK (76 px) take the floor. Gameplay touch controls are unchanged: they are not `Button`s, and `hud/layout.rs` sizes them already.
- **Map Maker is exempt** (`TouchTarget::exempt()` on its 11 buttons) → deferred to **BRL-8q4c**. Measured at 868x411: without the floor, the library buttons are 19-25 dp. With the floor, CREATE MAP / BACK are pushed below the window, because the library does not scroll. Its dense editor toolbars need a phone layout, which is outside this ticket.

# Changes
- New `packages/brawler-client/src/ui/touch_target.rs`: policy resource, `TouchTarget`, floor system and unit tests.
- Requires and installs it in `flow/screens/shared.rs`, `shell.rs`, `flow.rs`, `presentation.rs`, `presentation/match_menu.rs`, `hud.rs` and `hud/scoreboard.rs`. Inserts it in `flow/screens/practice_maps.rs`. Map Maker is exempt in `flow/screens/map_maker.rs`.
- `ui/layout_harness.rs`: `short_buttons(world, min_dp)` finds visible, enabled, laid-out buttons that are too short. `overlapping_buttons(world, exempt)` finds overlapping buttons under the same UI root, so a raised control never covers a neighbour. The Dashboard brawler-preview backdrop is exempted.
- `queue/matchmaking.rs`: `#[cfg(test)] set_membership_for_test`, so the flow harness can present the Queue screen.
- Docs: `docs/13-player-ux.md` (policy, beside the small-window fit and the slider-row note), `ui/README.md`, and the `SLIDER_ROW_MIN_HEIGHT` comment.
- No screen needed layout changes. With the floor on, every walked screen stayed reachable (scroll areas absorbed the growth) and had no button overlaps.

# Verification
- New tests:
  - `flow::tests::shell_controls_are_48_dp_touch_targets_on_a_landscape_phone`, at 868x411 with touch on, at 100 % and 80 % UI scale. It walks Server Select, the connection-error overlay, Dashboard, the Dashboard menu, the change-server confirmation, the Dashboard error overlay, Choose Game, Practice maps, Brawlers, Brawler details, Create Brawler, Queue, the cancel-match-start confirmation and Results. It asserts reachability, every visible enabled `Button` >= 48 dp, and no overlaps. Sanity check: it fails with the policy off (Queue CANCEL 27 px).
  - `shell::tests::settings_controls_are_48_dp_touch_targets_on_a_landscape_phone`: Display, Audio, Accessibility, Controls and Credits at 100 % and 80 %, with the same three assertions.
  - `flow::tests::desktop_policy_keeps_authored_control_heights`: the policy is off by default on desktop. At 1280x720 the authored heights are kept, and enabling then disabling the policy restores them exactly.
  - `ui::touch_target::tests::*`: raise, keep taller controls, follow the scale and owner rewrites, restore.
  - `hud::screen_tests::latched_scoreboard_fits_six_long_names_and_its_close_button` now also runs with touch on: BACK >= 48 dp, with no collisions at every viewport.
- Existing phone and desktop layout tests pass unchanged: `settings_and_controls_fit_a_landscape_phone`, `credits_back_is_reachable_on_phone_and_desktop`, `server_select_fits_a_landscape_phone_and_shows_field_values`, `player_path_screens_keep_every_control_reachable_on_a_landscape_phone`, `shell_overlays_fit_phone_and_desktop_viewports`, and the HUD `VIEWPORTS` tests.
- `just lint`: pass.
- `just test`: pass (exit 0, 24 suites ok, including network and performance gates). Two doc-comment and visibility-only edits came after it, and the focused touch/phone tests were rerun green.
- Not run: native desktop captures (optional). **Pixel 8a device check: owner** (Settings Display/Audio/Controls, Server Select and Dashboard at 100 % and 80 %). Also on device, look at the match menu at 80 % (four buttons of about 93 UI px). No headless layout test covers it, and the arithmetic fits about 540 of 752 UI px.

# Learning
- Overlays share the window with the screen beneath them, so an overlap check must compare only controls under one UI root. The Dashboard brawler preview is a deliberate backdrop hit area.
- An injected `OpenMapMaker` races `resolve_map_maker_action` in the flow harness (both run before `resolve_flow_action`). Set `NextState` directly when measuring Map Maker.
