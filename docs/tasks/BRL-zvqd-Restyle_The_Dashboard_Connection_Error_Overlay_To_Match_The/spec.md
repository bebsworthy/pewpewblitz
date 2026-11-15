Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec refreshed with code refs — the generic error overlay is still the old unstyled panel.

# Outcome
The Dashboard-context error overlay (for example 'The connection to the match was lost, and the match could not be resumed.' with Back) uses the shell's panel, typography and button styling, like the full-screen Connection Lost surface.

# Evidence (BRL-g57a device walk, Pixel 8a, 2026-11-14)
After a match session was lost and the fresh lobby succeeded, Dashboard showed a 'CONNECTION ERROR' box in a monospace debug-looking font with an orange outline over a dimmed Dashboard, unlike the full-screen Connection Lost panel. Screenshot: agent scratchpad brl0162/dev-16-after-suspend.png (not kept in the repo).

# Current code (HEAD 3437c0d8)
- Message source: `packages/brawler-client/src/flow/reducer/match_flow.rs:94-104` sets `FlowError { kind: Connection, return_flow: Dashboard, actions: [Back] }` after a lost match.
- `present_flow_error_overlay` (`flow/screens/overlays.rs:74`) only uses the shell treatment for `Connection` errors returning to `ServerSelect` (`spawn_connection_recovery`, overlays.rs:100 and 163-284). Every other error (Dashboard connection, queue, save, content, practice) takes the generic branch at overlays.rs:103-161:
  - plain `Node` with `BackgroundColor(PANEL)` and `BorderColor::all(PLAY)` (the orange outline), not `Surface::Panel`;
  - heading via `spawn_heading` (`flow/screens/shared.rs:95`) = `TextFont::from_font_size(38.0)`, i.e. Bevy's default font, not `ui::theme::font(assets, ..)`;
  - message `Text` with no font and `TextColor(PLAY_HIGHLIGHT)`;
  - buttons via `spawn_flow_error_button` (shared.rs:103-140): default-font label; the role is added later by `update_flow_button_chrome` (shared.rs:231-233, `surface_role` -> `Surface::Action` for `DismissError`).
- Title strings come from `FlowErrorKind::title` (`flow/model.rs:71`, tested at `flow/tests.rs:2573`).
- Shell helpers to reuse: `connection_shell.rs` `spawn_text` (78), `spawn_action` (98), `action_node` (133); `font` / `TEXT` from `ui/theme.rs`.
- `present_cancel_confirmation` (overlays.rs:339-397) uses the same old pattern (Match Loading "CANCEL MATCH START?"); restyle it in the same pass so no overlay keeps the debug look.

# Plan
1. Pass `assets: Option<&ClientAssetHandles>` into the generic branch; replace the panel `Node` + PANEL/PLAY border with `Surface::Panel` (transparent `BackgroundColor`/`BorderColor` like overlays.rs:180-193) inside the dimmed full-screen scrim (keep `INK.with_alpha(0.72)`, `GlobalZIndex(500)`, `DespawnOnExit`).
2. Title with `spawn_text(.., title, ~40.0)`; message with `font(assets, 22.0)`, `TextColor(TEXT)`, `TextLayout::new(Justify::Center, LineBreak::WordBoundary)`, `max_width`.
3. Buttons with `spawn_action(.., Some(role), true, action_node(percent(100), 56–64))`; first action Primary when it is a retry/continue, Back = `Surface::Back` or Secondary. Keep `ERROR_BUTTON_BASE + offset` indices and the `TryAgainQueue` rate-limit button (`RateLimitTryAgain`/`RateLimitTryAgainLabel`, overlays.rs:38-67, 286-314) working — give it the same font and a `Surface` role.
4. Apply the same panel/typography to `present_cancel_confirmation`.
5. Phone width: panel `width: percent(88)`, `max_width ~ 720`, padding that fits 411 px height; keep Back reachable without scrolling.

# Acceptance
- Unit/ECS test (flow/tests.rs, near `match_failure_and_fresh_lobby_return_preserve_result_and_one_shot_error_paths` :737 / `replacing_error_in_place_rebuilds_message_and_actions` :2328): the Dashboard connection error root contains a `Surface::Panel`, its texts use the shell font handle, and every `FlowButton` has a `Surface`.
- Layout test with `ui::layout_harness::add_layout` at 868x411 and 914x411 (and 1280x720): `unreachable_buttons` is empty and the panel is `on_screen`.
- Existing overlay tests (`state_scoped_flow_roots_replace_exactly_and_error_waits_for_destination` :2202, rate-limit tests) still pass; `just lint`, `just test`.
- Native capture of the Dashboard error (force via test hook or by killing the server mid-match) on desktop.
- Pixel 8a device check: lose a match session, return to Dashboard, confirm shell styling and that Back dismisses.
