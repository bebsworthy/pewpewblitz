# Outcome

The shipped client is smaller and simpler, and test automation stays fully available to scripts.

# Context

- `src/bin/client.rs:59-240` parses about 30 flags, mostly smoke/automation switches.
- Evidence serialization lives inside renderer modules (for example `collect_vfx_evidence_snapshot` at `presentation_3d/combat/effects.rs:653`).
- Roughly 21k production lines are tooling, evidence, or diagnostics.

# Scope

- A non-default `automation` feature gating smoke flags, evidence capture, render reports, and evidence snapshots.
- Move evidence code out of renderer modules into owned submodules.
- `justfile` and scripts enable the feature. The default client and Android builds exclude it.
- A check script, like `check-server-features.sh`, verifies the exclusion.

# Out of scope

Removing diagnostics that players benefit from (failure records, the optional overlay).

# Acceptance criteria

- The player build compiles without automation code (check script).
- All scripted E2E and Practice runs pass with the feature on.

# Verification

- `just lint`, `just test`, `just e2e 2`, `just practice-e2e wipeout-1v1`.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.


# Design (2026-11-13, before implementation)

## Research findings

- Every script and recipe builds the client with `--no-default-features --features client`; `default = ["client"]`. `network-test = [client, server, ...]` drives in-process headless clients through the `ClientNetworkConfig` automation fields.
- Automation drivers live in owned places: `src/bin/client.rs` (about 30 smoke/demo/capture/render flags), `client/queue/automation.rs` (queue/Practice/requeue smoke), `client/session/automation.rs` (controller demo), `client/input.rs` (headless input synthesis), `client/mod.rs` (`HeadlessAutomation`, scheduled screenshots), `client/session/match_commands.rs` (`finish_product_match_smoke`), `client/session/observation.rs` (exit-after-roster), `client/profile.rs` (automation default brawler), `client/presentation_3d/diagnostics.rs` (native render report), and the developer-only `vfx_lab` and `ui::gallery` surfaces plus their bins.
- **Conflict found:** F12 / north-button "Instant evidence capture" is a documented player-facing, rebindable action (`docs/13-player-ux.md` "Instant evidence capture", README controls table), while this ticket's scope lists "evidence capture" and "evidence snapshots". Removing it would change player behavior and the user's own playtest feedback path through `just run`/`just client`.
- BRL-0170 (crate split) is not started; this ticket must keep execution-role feature gates simple so that split can later move automation into its own crate. No constraints conflict.
- Baseline release player build (`cargo build --locked --release --bin brawler-client --no-default-features --features client`, default release profile, commit 373afceb): **171,728,688 bytes** (`strip -x`: 125,672,688; `__TEXT` 110,166,016; `__text` 82,629,648). Automation markers present: `--product-practice-smoke`, "brawler product queue smoke", "headless movement automation enabled", `frame_p95_ms`, "windowed synthetic controller demo", "Automation Brawler".

## Decisions

- New non-default Cargo feature **`automation = ["client"]`**. `default` stays `["client"]` (the player build). `network-test` adds `automation` because its separate-App harness drives headless clients.
- **Player build** = `--no-default-features --features client`. Used by `just client`, `just run`, `just balance-lab` (`scripts/dev.sh`) and `scripts/macos-client-bundle.sh`; BRL-0159 Android will use it too. `just run`/`just client` behavior is unchanged for the user (they never pass automation flags).
- **Automation build** = `--features client,automation`. Used by `scripts/network-product-match.sh` (so `just e2e`, `just practice-e2e`, `just zombie-e2e`), `network-product-lobby.sh`, `network-product-queue.sh`, `network-match.sh`, `network-routed.sh` (default client features), `v3-render-evidence.sh` (release), `just ui-gallery`, `just vfx-lab`.
- Gated by `automation` at each owning composition point (ordering kept visible with `.before/.after`): automation CLI flags (player CLI keeps `--server`, `--transport`, `--window-size`, `--help`); queue/requeue/Practice smoke drivers; headless input synthesis and `HeadlessAutomation` (moved from `input.rs`/`mod.rs` into a new gated `client/automation.rs`); controller demo; `finish_product_match_smoke`; exit-after-roster; automation default brawler; scheduled screenshot capture; `RenderMeasurementPlugin` native render report; `vfx_lab`, `ui::gallery` and their bins (`required-features = ["client", "automation"]`) including the `BRAWLER_UI_GALLERY_CAPTURE` hook and the VfxLab-only lifecycle seam.
- **F12 instant evidence capture stays in the player build** (conservative; durable docs define it as player UX). Its VFX snapshot collector moves out of the renderer file `presentation_3d/combat/effects.rs` into an owned `effects/evidence.rs` submodule so a later gate is a one-line change. Ticket question added for the user to decide whether player/Android builds should drop it.
- Inert `ClientNetworkConfig` automation data fields stay (they can only be set through the library API by automation callers); splitting them into a gated config type touches every network test and is deferred to a follow-up ticket.
- Out of this ticket (deferred ticket): legacy direct-UDP client combat evidence (`BRAWLER_NETWORK_COMBAT_CLIENT_READY_FILE`), client process closeout/window sampling, and the config-field split. Server-side verification is not part of the player client.
- Verification: new `scripts/check-client-features.sh` wired into `just lint` as `_client-features`: asserts `automation` exists and is not reachable from `default`/`client`, builds the debug player client and asserts automation marker strings are absent from the binary. Lint/check also run Clippy/check for both `client` (player) and `client,automation`; `_test-client` runs with `client,automation` so every client unit test still runs.


# Implementation (2026-11-13)

- `Cargo.toml`: `automation = ["client"]` (non-default); `network-test` includes it; `brawler-vfx-lab` and `brawler-ui-gallery` require `["client", "automation"]`.
- `src/bin/client.rs`: player CLI only (`--server`, `--transport routed-udp`, `--window-size`, `--help`). The unchanged full automation parser moved to gated `src/bin/client/automation.rs`.
- New gated `src/client/automation.rs`: `HEADLESS_FIRE_DURATION_TICKS`, headless input synthesis (`apply_headless_input`, helpers, `advance_headless_automation`), scheduled paired capture (`ScheduledScreenshots`), plus their tests (moved from `input.rs`, `mod.rs`, `tests.rs`).
- Gated in place at owning composition points with explicit `.before/.after` replacing former chain positions: `queue/automation.rs` (+ `last_accepted_game_type_id`), `session/automation.rs` (controller demo, now also owns `ControllerDemoGamepad` and `finish_product_match_smoke`), `profile.rs` automation default brawler, `presentation_3d/diagnostics.rs` (render report) and its install, `request_scheduled_evidence_capture`, `vfx_lab`, `ui::gallery`, VfxLab lifecycle seam/re-exports, `VfxPresentationIdentity::Explicit`, gallery-only icon lists (kept for tests).
- Evidence snapshot moved out of the renderer lifecycle file: `presentation_3d/combat/effects.rs` → `combat/effects/evidence.rs` (`collect_vfx_evidence_snapshot`, helpers, its test). F12 capture remains player-visible pending question q-000020.
- Scripts: `network-product-match.sh` (so `e2e`/`practice-e2e`/`zombie-e2e`), `network-product-lobby.sh`, `network-product-queue.sh`, `network-match.sh`, `network.sh`/`network-routed.sh` defaults, `network-paired-evidence.py`, `v3-render-evidence.sh` build `client,automation`. `dev.sh` and `macos-client-bundle.sh` keep the player build.
- `justfile`: `ui-gallery`/`vfx-lab` use automation; new `_check-automation`, `_clippy-automation`, `_client-features`; `_test-client` runs `client,automation`.
- New `scripts/check-client-features.sh`: feature-closure check, dev-tool `required-features` check, gated-declaration check, marker-in-source guard, and player-binary `strings` assertion.
- Docs: `docs/01-engine-decision.md` feature list, README executable configuration/lint/feature notes.

# Verification (2026-11-13)

- `just lint` (real ripgrep first on PATH): pass, including `_clippy-client` (player), `_clippy-automation`, `_server-features`, and new `_client-features`.
- `just test`: pass (23 suites, 2,218 tests, 0 failed). First run failed in `_test-balance-lab` only because I narrowed `ControllerDemoGamepad` visibility mid-run (private type in a `pub(super)` system signature); reverted and the full rerun passed.
- `just e2e 2`: pass (1v1 roster reached Active). `just practice-e2e` (wipeout-1v1): pass. `just zombie-e2e`: pass.
- `BRAWLER_RENDER_COMBAT=1 just v3-render-evidence <scratch>/render-report.txt` (release `client,automation`, scratch client data dir): `result=pass`, 1,801 samples, p50 16.661 ms, p95 17.628 ms, 3 paired PNG/JSON captures written.
- `just run 1` with `BRAWLER_DEV_DATA_DIR`=scratch and a separate bind: player client built, connected, lobby accepted; processes stopped cleanly. Player binary rejects `--headless` ("unknown flag") and prints the short usage. `data/` and `config/` unchanged (`data/server/profiles.sqlite3` mtime predates the session; canonical e2e/render scripts use `data/server` by design).
- Positive control: a `client,automation` debug binary contains every binary marker; the player binary contains none.

## Binary size (release player build, default release profile)

| | Before (373afceb) | After | Delta |
|---|---|---|---|
| `brawler-client` | 171,728,688 | 171,311,664 | -417,024 (-0.24%) |
| `strip -x` | 125,672,688 | 125,368,400 | -304,288 |
| `__TEXT` segment | 110,166,016 | 109,871,104 | -294,912 |
| `__text` section | 82,629,648 | 82,425,092 | -204,556 |

The win is small because Bevy/wgpu/dependencies dominate and the linker already dead-stripped the unused VfxLab/gallery code; the value is that player/Android builds cannot reach automation paths or flags. LTO/strip/panic tuning stays with BRL-0159.

# Usage note

- Play / playtest: `just client`, `just run N`, `just balance-lab` (player build; F12 capture still works).
- Automation and evidence: `just e2e`, `just practice-e2e`, `just zombie-e2e`, `just v3-render-evidence`, `just ui-gallery`, `just vfx-lab`, and scripts (all enable `automation`).
- Manual automation client: `cargo build --no-default-features --features client,automation --bin brawler-client`.

# Feedback disposition / deferred

- F12 instant capture in player builds: awaiting user answer (q-000020); kept unchanged meanwhile.
- Inert automation config fields/validation, `HeadlessAutomation` progress resource, legacy client combat-evidence observers, and client process closeout/window sampling: deferred to BRL-0194 (related BRL-0165, parent BRL-0171).

# Learn from errors

- Mistake: tightened a type's visibility while `just test` was compiling other feature sets; the network-test build saw a private type in a `pub(super)` system signature. Cause: editing source during a multi-config gate. Prevention: never edit sources while a canonical gate runs; compile every feature set after visibility changes.
- Mistake: first binary markers were weak — a CLI match-arm literal is not stored contiguously, and a validation string also lives in the lib. Prevention: pick markers from automation-only log/usage literals and verify each against a positive-control automation binary.
- Lesson: a ticket scope line ("evidence capture") can conflict with durable player UX docs; check `docs/13-player-ux.md` before gating anything with a player binding, and raise a question instead of removing player behavior.
