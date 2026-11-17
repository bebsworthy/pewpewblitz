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


# Implementation (2026-11-18)
- `flow/screens/overlays.rs`: the generic error overlay and `present_cancel_confirmation` now share `overlay_scrim()` (INK 0.72 scrim, `GlobalZIndex(500)`), `overlay_panel()` (`Surface::Panel`, 88% / max 720 px, padding 40x28, transparent fill/border so Surface owns chrome) and an `overlay_actions()` column (100% / max 560). Title via `connection_shell::spawn_text(.., 40.0)`, message via `font(assets, 22.0)` + `TEXT` + centered word-wrap (max 600). Buttons via `spawn_action(.., Some(role), true, action_node(percent(100), 56.0))`.
- Roles (decision): the first action is `Surface::Primary` when it retries or continues (RetryConnection, RetrySave, RetryQueue, TryAgainQueue, ContinueWithDefaults); everything else, including the lone Dashboard Back, is `Surface::Secondary`, matching the Connection Lost surface. `Surface::Back` was rejected: it rewrites the label into the corner chevron navigation button. Cancel confirmation: KEEP LOADING Primary, CANCEL MATCH START Secondary.
- Rate limit: Try Again is now a `spawn_action` button with `RateLimitTryAgain` + `InteractionDisabled` inserted; `update_rate_limit_try_again` updates the label through the button's `Children` (the `RateLimitTryAgainLabel` marker is removed). Indices stay `ERROR_BUTTON_BASE + offset` (cancel confirmation 0/1).
- Removed the now-unused `spawn_flow_error_button[_disabled]` from `shared.rs`; `spawn_heading` stays (queue, results).
- Test seam: `#[cfg(test)] ClientAssetHandles::with_font_for_test` so headless tests can tell the shell font from Bevy's default.

# Verification
- New `flow::tests::dashboard_error_and_cancel_confirmation_use_the_shell_panel_font_and_surfaces`: Dashboard lost-match error, rate-limited queue error and cancel confirmation each have a `Surface::Panel` child, every text uses the shell font handle, every `FlowButton` has a `Surface`; Back = index 1000 DismissError Secondary; Try Again = index 1000 Primary and enabled with no pending window.
- New `flow::tests::shell_overlays_fit_phone_and_desktop_viewports`: at 868x411, 914x411 and 1280x720 the Dashboard error, a two-action Persistence error with long copy and the cancel confirmation have no unreachable buttons and the panel is `on_screen`.
- Existing overlay tests (state-scoped roots, replacing error in place, rate-limit, phone reachability) pass. `just lint` passes; `just test` result in the next note.
- Native captures (release automation client, scratch supervisor on 127.0.0.1:5031 with data under `target/zvqd/`; a temporary, reverted, uncommitted flow hook set the overlay since no automation path opens it, see BRL-pw9k): `target/zvqd/dashboard-error-1280x720.png`, `target/zvqd/dashboard-error-914x411.png`, `target/zvqd/cancel-confirmation-1280x720.png`, `target/zvqd/cancel-confirmation-914x411.png`. They show the shell panel, Lilita One title and copy, and Surface buttons with focus. The cancel captures are over a forced MatchLoading without a reservation, so the backdrop is a blank screen, not the real loading screen.
- Still to do: the Pixel 8a device check (owner).

# Device check (owner)
Start a Practice match on the phone against `just server`, then kill that match's worker on the Mac: `pkill -KILL -f 'brawler-server --role match'`. The match session is lost and cannot be resumed, so the client returns through a fresh lobby to the Dashboard with CONNECTION ERROR "The connection to the match was lost, and the match could not be resumed." Check the shell panel and font, then check that Back dismisses it. Note: this path was not run natively here. A desktop automation smoke exits on disconnect, so it never reaches the Dashboard.

- `just test` passed (exit 0, 0 failures across all suites, including network and performance gates).


# Device check (2026-11-18)
Pixel not connected during closeout; device check left as a playtest: lose a match session (kill the match worker mid-Practice or suspend past the session limit), return to Dashboard, confirm shell styling and that Back dismisses.
