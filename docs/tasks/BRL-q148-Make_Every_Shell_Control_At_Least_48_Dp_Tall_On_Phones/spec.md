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
