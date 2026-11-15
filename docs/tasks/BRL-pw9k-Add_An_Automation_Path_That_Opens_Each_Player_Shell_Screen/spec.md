Reviewed 2026-11-15 against HEAD 3437c0d8: valid and still open (no shell-navigation automation exists); spec updated. Recommend merging BRL-pqzt into this ticket as its first milestone, since both need the same mechanism.

# Outcome

A development-only automation flag opens a named shell screen with seeded scratch profile data and captures it through the existing `--screenshot-dir` hook.

# Context

- BRL-51zd could capture only Connecting and first-brawler creation natively: the other screens need clicks, and agents here have no OS input automation. The Dashboard needs a saved brawler in a scratch server data dir. BRL-pqzt (Dashboard, details, card, weapon previews) is a subset of this ticket.
- Already available:
  - The interactive product shell (no `--auto-connect`) accepts `--screenshot-dir/--screenshot-first/--screenshot-every/--screenshot-count`. `validate_screenshot_schedule` (`automation/config.rs:273`) rejects only headless clients, and `compose_client_app` installs the schedule after the shell plugins (`packages/brawler-client/src/lib.rs:703-724`).
  - `--window-size` covers 1280x720 and the 914x411 phone viewport.
  - The shell takes `--server` as a prefill (`bin/client/automation.rs:202-210`).
  - `BRAWLER_CLIENT_DATA_DIR` isolates client data.
- Missing: nothing drives `FlowUiAction`s. `--auto-connect` turns the shell off (`presents_product_shell`, `src/config.rs:376`), so the match-automation flags cannot reach shell screens.
- Canonical navigation surface:
  - `FlowUiAction` (`flow/actions.rs:10-122`) and `PendingFlowActions { explicit, ordinary }` (line 165). `begin_flow_frame` resets these every frame, so a driver must write `ordinary` in `ClientFlowSet::CollectFlowInput` `.after(collect_flow_input)` (`flow.rs:168`), as BRL-kp7d's scratch `evidence_drive.rs` did.
  - Destinations are `ClientFlow` (`flow/model.rs:6`) plus `ClientOverlay` (line 35): Settings, Credits, DashboardMenu, BrawlerList, BrawlerDetails, BrawlerCreation, BrawlerEditor, WeaponEquipment, DeleteBrawlerConfirmation, Confirmation, ChangeServerConfirmation, LeaveConfirmation, and Error.
- Shared prerequisites:
  - BRL-tpd8: honor `CARGO_TARGET_DIR` and use isolated server and client data dirs. Copy its script pattern; the BRL-51zd scratch `shell-evidence.sh` shows the shape.
  - BRL-hkbm: the scheduled capture toast would otherwise overlay later shell captures.

# Plan

1. Add a client flag `--shell-capture <script>` (automation feature only, rejected with `--auto-connect`/`--headless`). The script is a named route such as `dashboard`, `settings`, `brawler-details`, `weapon-equipment`, `practice-maps`, `game-select`, `map-maker`, `results`, or `match-menu`, or a short step list.
2. Add `packages/brawler-client/src/flow/automation.rs` (`#[cfg(feature = "automation")]`, registered from `ClientFlowPlugin`). It waits for each step's expected `ClientFlow`/`ClientOverlay`, then writes the next `FlowUiAction` into `PendingFlowActions.ordinary`. After the screen settles (a fixed frame offset, including mid-burst offsets for Results and accepted equipment), it requests a capture through `evidence_capture::request_scheduled_evidence_capture`, then exits after the last step. Saved-brawler IDs come from `ClientProfileModel::snapshot()`. Only flow actions are used, never a second navigation path.
3. Seed the profile through the shell itself: on an empty profile, `ConfirmCreateBrawler` (as BRL-kp7d did), or reuse `profile/automation.rs` `plan_automation_brawler` for a chosen weapon.
4. Add `scripts/shell-capture.sh` plus a `just shell-capture route="all" size="1280x720"` recipe. It starts a scratch supervisor (`--data-directory`, ephemeral `--bind`, never 5000) and runs one client per route at each size, using `${CARGO_TARGET_DIR:-target}` binaries and writing to `target/shell-capture/<size>/<route>.png|json`.
5. Extend `scripts/check-client-features.sh` so the player build contains no `--shell-capture` string or symbols.

# Acceptance

- `just shell-capture` writes a PNG/JSON pair for each listed screen at 1280x720 and 914x411, and `git status --porcelain data/server` stays empty.
- `cargo build -p brawler-client` (player build) rejects or does not know `--shell-capture`; `just lint` (including `_client-features`) and `just test` pass.
- A unit test of the route table checks that each step's expected flow/overlay matches the reducer's result for its action, using the existing `flow/tests.rs` harness.

# Absorbs BRL-pqzt (2026-11-15)
BRL-pqzt (automation capture of the Dashboard, brawler card, details and weapon previews) was a subset of this ticket and is canceled. Its targets are part of this ticket's acceptance: the `--shell-capture` run must save Dashboard, brawler card, brawler details and weapon preview frames. See BRL-pqzt's spec for the per-screen notes.
