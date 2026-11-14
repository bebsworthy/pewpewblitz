# Outcome

The player build (`--no-default-features --features client`) compiles none of the automation configuration or legacy evidence observers that BRL-zv4x left inert.

# Context (from BRL-zv4x)

BRL-zv4x added the non-default `automation` feature and gated every automation *driver* (CLI flags, smoke/requeue/Practice drivers, headless input synthesis, controller demo, scheduled capture, native render report, VfxLab/UI gallery). It deliberately left these inert pieces in the player build because gating them touches every separate-App network test:

- `ClientNetworkConfig` automation fields (`headless_*`, `product_*_smoke`, `exit_after_*`, `weapon_preset`, `windowed_*_demo`, `screenshot_schedule`, `render_measurement`) and their `validate()` messages (for example `--product-practice-smoke requires product match automation` is still in the player binary).
- Reads of those fields inside product systems: `session/match_commands.rs` (automatic ready), `session/admission.rs`, `session/routing.rs` (exit after lobby return), `session/observation.rs` (exit after roster), and the `HeadlessAutomation` progress resource.
- Legacy direct-UDP client combat evidence: `ClientCombatObservation`, `ClientCombatEvidenceStatus`, `receive_combat_evidence_checkpoints`, `capture_client_combat_checkpoints`, `record_headless_combat_observation` (`BRAWLER_NETWORK_COMBAT_CLIENT_READY_FILE`), and the client closeout fields that read them.
- Client process closeout/window sampling (`BRAWLER_DIAGNOSTICS_CLOSEOUT_FILE`, `BRAWLER_DIAGNOSTICS_WINDOW_FILE`); failure records and the optional overlay stay (player-benefiting).

# Scope

- Move automation fields into one `ClientAutomationConfig` value carried by `ClientNetworkConfig` only under `automation`; product systems ask a small cfg-gated helper rather than reading fields.
- Gate the client combat-evidence observers and their closeout projection behind `automation`.
- Decide (and record) whether client process closeout/window sampling is automation-only.
- Extend `scripts/check-client-features.sh` markers with a validation message and a combat-evidence string.

# Acceptance criteria

- `scripts/check-client-features.sh` passes with the new markers; the release player binary shrinks or stays equal.
- `just lint`, `just test`, `just e2e 2`, `just practice-e2e`, and the combat-profile script still pass with `automation` enabled.
- No wire, protocol, fingerprint, or player-visible behavior change.

# Related

- BRL-zv4x (introduced the `automation` feature), BRL-qrb8 (crate split; coordinate so automation can become its own crate).

# Design decisions (2026-11-14, BRL-3p70 implementation)

- **Automation config is a separate resource, not a `ClientNetworkConfig` field.** `ClientNetworkConfig` lives in the shared `brawler` crate, which deliberately has no `automation` feature (`scripts/check-client-features.sh` asserts that), so a field gated by `brawler-client/automation` cannot exist on it. The automation fields, their unit types (`WindowedCombatDemo`, `WindowedControllerDemo`, `ScreenshotSchedule`, `RenderMeasurementConfig`), and every automation validation message moved to `brawler_client::ClientAutomationConfig` (`packages/brawler-client/src/automation/config.rs`, compiled only with `automation`). It is a Bevy resource beside `ClientNetworkConfig`; `ClientNetworkConfig` keeps only transport/identity/compatibility, `headless`, `auto_connect`, render profile, and window size. `connects_on_startup()` is now `headless || auto_connect` (the automation CLI already sets `auto_connect` for both demos).
- **Product systems ask one helper.** `session/automation_hooks.rs` defines the `AutomationHooks` system parameter used by `send_client_hello`, `process_join_outcome`, `drive_match_loading_check_in`, `send_match_command`, `log_replicated_roster`, and `enforce_client_timeout`. With `automation` it reads `ClientAutomationConfig`/`HeadlessAutomation`; in the player build every method is a `const` constant or no-op, so no exit rule, field, or timing marker is compiled in. The `match-ready`, `match-accepted`, and routed `handoff-connected` stderr timing markers moved into it (only automation scripts parse them).
- `observe_fresh_lobby_return` moved into the gated `session/automation.rs`, registered at its former Handshake chain position (`.after(process_join_outcome).before(process_match_route_grant)`).
- **`HeadlessAutomation`** stays in `brawler::config` (the shared evidence adapter reads it) but lost its `FromWorld` read of config fields; it gained `weapon_preset` and is inserted only by `brawler-client`'s `install_automation_state` (from `ClientAutomationConfig`, during `ClientNetworkPlugin` build). The player app has no `HeadlessAutomation`.
- **Legacy direct-UDP client combat evidence** is a new shared `ClientCombatEvidencePlugin` (`ClientCombatObservation`, `ClientCombatEvidenceStatus`, checkpoint receive/capture, ready-file report) that `ClientReplicatedGameplayPlugin` installs only with `automation`. `receive_combat_cues` now takes `Option<ResMut<ClientCombatObservation>>` and calls two small helpers. System order is unchanged (checkpoint ingest `.after(receive_combat_cues)` in Ingest; capture then report chained in Evidence); none of these systems defer commands.
- **Decision: client process closeout/window sampling is automation-only.** Only `scripts/network.sh` and `scripts/network-routed.sh` set `BRAWLER_DIAGNOSTICS_CLOSEOUT_FILE`/`BRAWLER_DIAGNOSTICS_WINDOW_FILE` for clients, and both build `automation`. `brawler-client` installs `ProcessDiagnosticsPlugin` only with `automation`; the server is unchanged. **Failure records stay** in the player build: new `brawler::diagnostics::failure_record_path()` (the `ProcessDiagnosticsSettings` method delegates to it) is used by `record_client_failure` and the panic hook, so they no longer need the diagnostics settings resource.
- The shared crate still *compiles* the evidence plugin and closeout client projection under `client`; they are absent from player artifacts because nothing in the player build references them (linker dead-stripping, proven below). Full compile-time removal is deferred to BRL-rhw3.
- Automation entry point: `brawler_client::build_automation_app(config, automation)`; the player `build_app_with_config` is unchanged for desktop and Android.

# Implementation (2026-11-14)

- Shared crate: `src/config.rs` (fields, types, validation, and their tests removed; `HeadlessAutomation` reworked), `src/combat/client/{mod,cues,tests}.rs`, `src/combat/{mod,evidence}.rs`, `src/diagnostics/{failure,mod}.rs`, `src/diagnostics/process/mod.rs`.
- Client: `automation.rs` → `automation/mod.rs` + new `automation/config.rs` (config, validation, runtime derivation, `automatic_match_command_enabled`, moved tests); new `session/automation_hooks.rs`; `session/{admission,match_commands,observation,routing,automation,mod}.rs`; `profile/automation.rs`, `queue/automation.rs`, `presentation_3d/{mod,diagnostics}.rs`, `lib.rs`, `bin/client.rs`, `bin/client/automation.rs` (returns `(ClientNetworkConfig, ClientAutomationConfig)` and validates both), `tests.rs` (composition test asserts the evidence/diagnostics plugins are present exactly with `automation`).
- Tests: `tests/network/*` set `weapon_preset`/`exit_after_roster` on `ClientAutomationConfig`.
- `scripts/check-client-features.sh`: new markers `--product-practice-smoke requires product match automation`, `timing handoff-connected`, `headless client is extending its run until combat evidence completes`, `BRAWLER_NETWORK_COMBAT_CLIENT_READY_FILE`, `BRAWLER_DIAGNOSTICS_CLOSEOUT_FILE`; the source-existence guard now covers `src` too; optional `BRAWLER_CLIENT_FEATURES_POSITIVE_CONTROL=1` builds the automation client and asserts every marker is present.
- Docs: `docs/01-engine-decision.md` feature-list entry for `brawler-client/automation`.
- No wire, protocol, fingerprint, persistence, or player-visible change.

# Verification (2026-11-14, scratch `CARGO_TARGET_DIR`, no change to the user's server on UDP 5000)

- `BRAWLER_CLIENT_FEATURES_POSITIVE_CONTROL=1 ./scripts/check-client-features.sh`: pass — the player debug binary has none of the 12 markers and the automation debug binary has all of them.
- Baseline release player binary (untouched `git archive` of 82a76b59) contained all five new markers; the after-change release player binary contains none. The Android `cargo ndk --target arm64-v8a --platform 31 build -p brawler-android --profile android` (no adb, no jniLibs output): pass, and `libbrawler_android.so` contains none of the markers either.
- `just check`: pass. `just lint` (VS Code ripgrep first on PATH): pass, including `_clippy-client`, `_clippy-automation`, `_server-features`, `_client-features`, `_lint-exceptions`.
- `just test` (full, after the last edit): pass — 24 suites, 2,391 tests, 0 failed (network 115, performance included).
- E2E through a patched scratch copy of `scripts/network-product-match.sh` (scratch target binaries, scratch `--data-directory`, ephemeral port): 1v1 product match (`e2e 2` equivalent) reached Active; Practice `wipeout-1v1` (`practice-e2e` equivalent) reached Active with one human. Tracked `data/` and `config/` unchanged.
- Combat-profile script (`BRAWLER_NETWORK_PROFILE_RUNS=1 BRAWLER_NETWORK_PROFILE_PRESETS="1 3"`, ephemeral ports): `local-preset-1` passed end to end (ready files, closeout validation) — the legacy evidence path works with the new plugin. `local-preset-3` timed out with `missing=["active_lob_flight","active_slow","active_knockback"]`; the identical failure reproduces on the untouched 82a76b59 copy, so it is pre-existing → BRL-5fk5.

## Release player binary size (`cargo build --release -p brawler-client --bin brawler-client`, same-length scratch source paths)

| | Before (82a76b59) | After | Delta |
|---|---|---|---|
| `brawler-client` | 173,909,776 | 173,548,720 | -361,056 |
| `strip -x` | 127,403,504 | 127,099,120 | -304,384 |
| `__TEXT` segment | 111,820,800 | 111,525,888 | -294,912 |
| `__text` section | 83,942,264 | 83,715,156 | -227,108 |

# Playtest

Not needed: no player-visible behavior changes (the removed paths were inert in the player build; failure records and F12 capture are unchanged). Normal `just client`/`just run` use the player build exactly as before.

# Feedback disposition / deferred

- BRL-5fk5 (backlog): pre-existing combat-profile preset 3 lob checkpoint timeout.
- BRL-rhw3 (backlog): compile the legacy client evidence/closeout projection out of the shared crate's `client` role instead of relying on link-time exclusion.
- F12 instant capture in player builds remains BRL-zv4x question q-000020's concern; unchanged here.

# Learn from errors

- Mistake: a baseline debug build from a `git archive` copy shared my `CARGO_TARGET_DIR` and silently reused my *modified* shared-crate artifacts (Cargo hashes path packages relative to the workspace and judged the older-mtime copy fresh), so the "baseline" failed to compile against the new config. Cause: assuming different source paths give separate artifacts. Prevention: always give a comparison tree its own target directory; the release baseline was built first into an empty profile, so it stayed valid.
- Mistake: first Clippy pass failed on `struct_excessive_bools` and `field_reassign_with_default` for the moved config/CLI. Cause: moving fields dropped the original item-level allow, and builder-style reassignment of a `Default` value. Prevention: carry item-level lint exceptions with moved items and build new values with struct initializers.
- Lesson: a player-binary exclusion claim needs a positive control in the same check; the new optional control makes that one command. Debug-binary string checks only prove compile-time or link-time exclusion, so product-code markers must sit behind `cfg`, not behind a runtime `false`.
