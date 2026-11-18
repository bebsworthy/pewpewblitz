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

# Result (2026-11-18)

Implemented; absorbs BRL-pqzt (Dashboard, brawler card/list, details, weapon preview frames are all captured).

- `packages/brawler-client/src/flow/automation.rs` (automation-gated, installed from `compose_client_app`): `ShellCaptureRoute` route table plus one driver system in `ClientFlowSet::CollectFlowInput` after `collect_flow_input`. It writes only `PendingFlowActions.ordinary`. Each step waits for its expected `ClientFlow`/`ClientOverlay` (state-driven, with timeouts), the last step settles for 90 unchanged frames, and the capture goes through `request_scheduled_evidence_capture` (`CaptureFeedback::LogOnly`). Then the process exits 0, or 1 on timeout or when the screen changes during settling. On an empty profile it seeds through `ConfirmCreateBrawler` -> `BackToBrawlerList` -> `CloseBrawlerList`.
- CLI: `--shell-capture <route>` needs `--screenshot-dir`, the interactive shell, and no `--screenshot-first/every/count`. Validation is in `ClientAutomationConfig`. Capture JSON now records `client.flow` and `client.overlay`.
- `scripts/shell-capture.sh` and `just shell-capture route="all" size="all"`: per size, a fresh scratch supervisor on `127.0.0.1:5031` (refuses :5000) with `--data-directory target/shell-capture/<size>.run/server-data`. One release automation client runs per route, sharing `BRAWLER_CLIENT_DATA_DIR` in that run dir. Binaries come from `${CARGO_TARGET_DIR:-target}`.
- `scripts/check-client-features.sh`: gates `flow.rs: mod automation;`. Markers are `--shell-capture` and the driver log line.
- Routes (19): first-brawler, dashboard, dashboard-menu, settings, credits, change-server, brawler-list, brawler-details, brawler-create, brawler-editor, ultimate-picker, weapon-equipment, weapon-parts, delete-brawler, game-select, practice-maps, map-library, map-editor, queue.
- Deferred to BRL-wqst (linked): results, match-menu, leave-confirmation, and match-loading. The match menu is `ClientInputContext::Menu`, not a flow action, and Results needs a completed match.
- Tests: `flow::tests::shell_capture_route_steps_match_the_reducer` runs every route's steps through the flow harness. The harness injector now runs before `resolve_map_maker_action`, as production input does. Also added driver state-machine tests and a config validation test.

Verification:
- `just shell-capture` passed, writing 19 PNG/JSON pairs each to `target/shell-capture/1280x720/` (2560x1440 physical) and `target/shell-capture/914x411/` (1828x822). `git status --porcelain data/server` was empty.
- Contact sheets `target/pw9k/sheet-<size>.png` (route order, row-major) were reviewed. Each frame shows its intended screen settled, and the JSON flow/overlay matches the route.
- The player `cargo build -p brawler-client` rejects `--shell-capture` ("unknown flag"). `BRAWLER_CLIENT_FEATURES_POSITIVE_CONTROL=1 scripts/check-client-features.sh` passed.
- `just lint` and `just test` passed. Logs are in `target/pw9k/`.

Not done: mid-burst capture offsets for accepted equipment or Results (not needed for the shell routes), and route order in the script is a hand-kept copy of `ShellCaptureRoute::ALL`.
Mistake and its prevention: the first draft exceeded clippy's `too_many_lines` in the route table and the driver. I split the step constructors into helpers and moved the driver into a pure `advance` state machine, which is now unit-tested.
