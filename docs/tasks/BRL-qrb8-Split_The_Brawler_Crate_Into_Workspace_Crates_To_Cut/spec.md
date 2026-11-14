## Problem

`brawler` is one package whose library is compiled under five configurations (`client`, `server`, `balance-lab`, `network-test`, plus `brawler-routing` separately), each as a library, test harness, five bins, and the `network`/`performance` integration tests. Measured on 2026-10-23:

- `target/` reached 117 GB (`target/debug/deps` 76 GB, `target/debug/incremental` 38 GB).
- 51 distinct `brawler` artifact hashes, 11 `libbrawler*.rlib` copies of 450 MB–1.1 GB each, about 13,800 `.rcgu.o` objects (39 GB), and 44 incremental session folders for `brawler`.
- Any edit under `src/` recompiles the entire library for every feature set it is built in, so one change followed by the canonical gate means roughly 10–15 large compiles of the same crate.

AGENTS.md permits a new package for a demonstrated compile-time boundary. This ticket provides that demonstration first, then makes the split.

## Scope

1. **Baseline measurement (before any split).** With the cheaper debug-info profile in place (see Related), record on a quiet machine:
   - clean-build time for `just check` and `just test`;
   - incremental time for `just _test-client` and `just _test-server` after touching (a) a client presentation file, (b) a shared gameplay file, (c) a server-only file;
   - `target/` size after a full `just lint && just test`.
2. **Design.** Propose the minimal crate graph in the spec before moving code, following the existing execution-role boundaries. Starting hypothesis, to be confirmed or revised from the dependency graph:
   - a headless shared gameplay crate (protocol, content, timing, combat/movement/map/modes/matchplay/abilities/builds/profiles models and shared systems), no rendering or windowing features;
   - a client crate (`src/client/`, presentation, audio, VFX, input, HUD);
   - a server crate (`src/server/`, lobby, Balance Lab, Practice, bots if they are server-only);
   - thin bin crates or bins inside the role crates.

   Keep the number of crates as small as the measured win allows. Record which `pub(crate)` items become cross-crate API and keep that surface narrow (relates to BRL-nrab).
3. **Implementation.** Move modules without behavior, protocol, wire-identity, or content-fingerprint changes. Preserve plugin composition, `SystemSet` ordering, feature gates, and the rule that the server graph never acquires rendering, windowing, audio, device input, or client assets. Update `justfile`, scripts, and `docs/01-engine-decision.md` / AGENTS.md source-layout sections.
4. **Re-measure** the same matrix as step 1 and record the comparison.

## Acceptance criteria

- Baseline and post-split measurements are recorded in this spec with commands and machine conditions.
- An edit to a client-only presentation file no longer recompiles the shared gameplay crate or any server artifact (verified with `cargo build -v` / `--timings` output).
- Incremental `just _test-client` after a client-only edit is measurably faster than the baseline (target: at least 2×; record the actual number either way).
- `target/` size after a full gate is lower than the baseline.
- `just lint`, `just test`, `just e2e`, `just practice-e2e`, and `scripts/check-server-features.sh` pass; the server feature graph still excludes client-only dependencies.
- No wire, protocol, fingerprint, persistence, or player-visible behavior changes; existing network and schedule tests pass unchanged.
- Durable documentation reflects the new layout.

## Constraints

- Do not start while Phase 1 gameplay tickets (BRL-x1x6–BRL-wfmj) are mid-flight in the same files; schedule it between tickets to avoid large merge conflicts.
- No speculative extra crates, public API, or plugin layers beyond what the measurement justifies. If the baseline shows the win is small, record that and cancel instead of splitting.
- Do not change dependency feature sets for other reasons in the same change, except where unification is required by the split and verified against server isolation.

## Related

- The cheaper dev profile (`debug = "line-tables-only"` for the crate, `debug = false` for dependencies) and a `just sweep` stale-artifact recipe are a separate small change applied first, so the baseline reflects them.
- BRL-nrab (narrow the application crate public API).


## Baseline measurement (2026-11-14, before any split)

Machine: Apple Silicon (12 cores, 32 GB), macOS, rustc/cargo 1.95.0, repository HEAD `1d38cc1b`.
The "cheaper dev profile" named under Related has **not** landed (`[profile.dev]` is still
`opt-level = 1` with full debuginfo); both baseline and post-split use the current profile so the
comparison is like-for-like. Measurements ran in an exported copy of HEAD with a dedicated
`CARGO_TARGET_DIR` under the session scratchpad, so the owner's `target/` and running routed
server were never touched. The clean steps overlapped with light `cargo check` prototyping, so
treat clean wall times as approximate; the incremental matrix ran on an otherwise idle machine.

Clean, sequential, one fresh target dir:

| Step | Wall |
|---|---|
| `just check` (clean) | 155 s |
| `cargo build --no-default-features --features client` (`--timings`) | 367 s; `brawler` lib unit 119 s |
| `cargo build --no-default-features --features server` (`--timings`) | 131 s; `brawler` lib unit 52 s |
| `just test` (after the two builds) | 896 s, all suites green |
| `just lint` | clippy recipes green; `_client-features` fails under a custom `CARGO_TARGET_DIR` because the script reads the literal `target/debug/brawler-client` (pre-existing defect, fixed in this ticket) |
| `target/` after check + test + lint | 57.1 GB (`debug/deps` 37.2 GB, `debug/incremental` 18.3 GB) |

Incremental matrix: each edit is applied, then every target is rebuilt in turn (each target has its
own artifacts, so each sees exactly one pending edit); the edit is then reverted and the matrix is
re-run (two samples per cell). Two edit kinds: a dead `const` appended to the file ("item"), and
`std::hint::black_box(0_u32);` inserted as the first statement of a live function ("body").
Targets, using the real feature sets from `justfile`:

- `build-client` = `cargo build --locked --no-default-features --features client`
- `build-server` = `cargo build --locked --no-default-features --features server`
- `testlib-client` = `cargo test --locked --no-default-features --features client,automation --lib --no-run`
- `testlib-server` = `cargo test --locked --no-default-features --features server --lib --no-run`
- `test-client-all` = the `_test-client` compile (`--features client,automation --all-targets --no-run`)

Baseline incremental results (wall seconds, `add` sample / `revert` sample):

| Edit (add / revert, seconds) | build-client | build-server | testlib-client | testlib-server | test-client-all |
|---|---|---|---|---|---|
| client-presentation | 7.8 / 4.7 | 0.2 / 0.2 | 6.7 / 4.8 | 0.2 / 0.2 | 9.7 / 6.4 |
| client-presentation-body | 40.0 / 41.2 | 0.2 / 0.2 | 52.1 / 50.3 | 0.2 / 0.2 | 42.0 / 41.2 |
| combat-rule | 0.2 / 0.2 | 5.1 / 2.9 | 0.2 / 0.2 | 3.9 / 2.6 | 0.2 / 0.2 |
| combat-rule-body | 35.6 / 36.9 | 11.5 / 11.5 | 32.5 / 33.2 | 15.5 / 15.2 | 29.8 / 29.6 |
| protocol | 4.6 / 6.5 | 3.0 / 3.0 | 4.8 / 4.8 | 2.8 / 2.8 | 6.3 / 6.3 |
| protocol-body | 34.8 / 35.0 | 16.0 / 15.7 | 41.9 / 41.0 | 18.2 / 17.9 | 46.5 / 46.0 |

Reading: item-level edits are cheap everywhere (5-10 s). Body edits, the realistic case, cost
30-52 s in every client-feature configuration versus 11-18 s in the server configuration: the cost
scales with the size of the crate being re-codegened, and the client configuration compiles the
whole 187k-line crate. A client-only edit never touches server artifacts already.

Edit files: client presentation `src/client/presentation_3d/hit_feedback.rs`
(`combat_cue_hit_feedback`), shared combat rule `src/combat/definitions/resolver.rs`
(`spread_angles`, used by server attack emission and client aim preview), protocol
`src/protocol.rs` (`FighterInput::is_valid`). The "item" combat sample used
`src/combat/effects/planning.rs`, which turned out to be server-only (client builds 0.2 s).

## Dependency analysis

- 186.7k Rust lines: `src/client/` 92.9k (half the crate), `src/server/` 23.4k, shared ~66k.
- `src/server/` is not the expensive boundary: server authority is interleaved through the shared
  domain modules (about 400 `#[cfg(feature = "server")]` sites in combat/abilities/map/matchplay/
  modes/bots), so a server crate would need a large behavioral refactor. A client-only edit already
  never rebuilds a server artifact (`build-server`/`testlib-server` stay 0.2 s): cfg'd-out
  `mod client;` files are not in the server build's dep-info.
- `src/client/` depends one way on shared modules except for four non-test reverse references:
  `brawler::testing::spawn_crossbeam_client`, combat evidence reading `client::HeadlessAutomation`,
  `map::client` writing `client::ClientPlayableGate`, and the diagnostics overlay reading
  `client::ClientJoinPhase/ClientJoinStatus`.
- No Lightyear-registered type (component, message, channel) is defined in `src/client/`. Lightyear's
  registry hasher hashes `DebugName::type_name` of every registered type, so the protocol registry
  fingerprint depends on crate names: replicated types must stay in a crate named `brawler`.
  The content fingerprint is built from catalog domain IDs/material, not type names.
- Client sources include repository files by relative path (`include_str!`) and use
  `CARGO_MANIFEST_DIR` for the Bevy asset root and asset tests; a moved package must re-anchor these.

## Chosen boundary

One new package, `packages/brawler-client`, holding the former `src/client/` (crate root
`src/lib.rs`), the three client binaries, and the `network`/`performance` integration suites (they
need both roles). The root package keeps the name `brawler` (wire/fingerprint stability), the
shared modules, the shared-side client adapters (`combat::client`, `map::client`, combat evidence,
client protocol registration) behind its `client` feature, the server, and the server binaries.
No server crate and no separate "core" crate: neither is justified by the dependency structure or
the measurements above.

Cross-crate API: shared items the client uses become `pub` (they were `pub(crate)`); recorded in
the implementation notes below and kept to what the client consumes (BRL-nrab). `HeadlessAutomation`
moves to `brawler::config` and `ClientPlayableGate` to `brawler::map` (both re-exported from
`brawler_client` at their old relative paths); the diagnostics overlay moves into the client
crate; `spawn_crossbeam_client` moves to `brawler_client::testing`. Integration tests and bins
import client items as `brawler_client::...` instead of `brawler::client::...`; that path change is
unavoidable because `brawler` cannot depend on `brawler-client`.


## Implementation (2026-11-14)

Built and verified first in a scratch prototype copy, then applied to the working tree as one
patch. Layout:

- `packages/brawler-client/` (new): former `src/client/` (crate root `src/lib.rs`, was `mod.rs`),
  `src/bin/{client.rs,client/,vfx_lab.rs,ui_gallery.rs}`, and `diagnostics_overlay.rs` (was
  `src/diagnostics/overlay.rs`, with its two tests). Features: `automation`, `network-test`
  (`automation` + `brawler/network-test`), `balance-lab` (passthrough for one network scenario).
  Owns test targets `network`/`performance`, whose sources stay at `tests/` (`path = "../../tests/..."`).
  Client-only deps moved here: `bevy_hanabi`, `arboard`, `directories`, and dev-deps `skrifa`, `parley`.
- Root `brawler`: drops the client binaries, `automation` feature, and those deps; adds
  `autotests = false` (explicit `map_public_api`, `routed-process`) and a dev-only `test-support`
  feature exposing `brawler::test_app` to client unit tests.
- Client code says `brawler::<domain>::...` for shared items (mechanical rewrite of
  `crate::<domain>::`; `crate::client::` became `crate::`). Relative `include_str!` paths gained one
  `../`; `CARGO_MANIFEST_DIR` uses go through `REPOSITORY_ROOT` (`<pkg>/../..`), so the Bevy asset
  root is still the repository `assets/` directory.
- Reverse references resolved: `HeadlessAutomation` -> `brawler::config` (fields now `pub`),
  `ClientPlayableGate` -> `brawler::map` (both re-exported from `brawler_client`),
  `spawn_crossbeam_client` -> `brawler_client::testing`, overlay moved to the client crate; a stale
  `PendingLocalActions` insert removed from one `combat::client` schedule test.
- Cross-crate API (`pub(crate)` -> `pub`, about 60 lines, all consumed by `brawler-client`):
  `combat::{client (module), delivery (module), DeduplicatedCombatCue}` and the aim/spray preview
  helpers (`AimMark`, `AimTone`, `AimTrace*`, `PreviewGeometry`, `PreviewPrimitive`,
  `MAX_CONE_SPRAY_SEGMENTS`, `aim_lane_marks`, `cone_spray_primitives`, `straight_map_travel`,
  `ultimate_area_marks`); `map::{authoring (module), ClientPickupCueSet}`; `matchplay::{ObjectiveResult,
  ObjectiveTotals}`; `modes` (module, `#[doc(hidden)]`) with `ModeRegistry`, `ModeCatalog`,
  `ModeRegistryPlugin`, `ModeRegistration{key, presentation}`, `ModePresentationProjection{selection_label}`,
  `BuiltInModeRegistrationsPlugin`, `descriptor_for_definition`; `movement::{MovementIntent,
  MovementModifiers, movement_intent, movement_is_suspended, resolved_movement_velocity}`;
  `logging` (module, `#[doc(hidden)]`, `add_log_plugin_once`); `diagnostics::unix_micros_now`,
  `ProcessDiagnosticsState::record_error_exit`; `protocol::...::to_routing_capability`. Five new
  `#[must_use]` attributes satisfy `clippy::must_use_candidate`. Narrowing this belongs to BRL-nrab.
- `justfile`: client recipes now select `-p brawler -p brawler-client` with
  `--features brawler/client[,brawler-client/automation]`; network/performance/balance-lab-network
  recipes use `-p brawler-client`. Scripts build the client with `-p brawler-client [--features automation]`;
  `BRAWLER_ROUTED_CLIENT_FEATURES`/`BRAWLER_NETWORK_CLIENT_FEATURES` now name `brawler-client`
  features (`automation`, `brawler/process-metrics`). `check-client-features.sh` checks the new
  package and honors `CARGO_TARGET_DIR`; `check-server-features.sh` also forbids `brawler-client`,
  `bevy_hanabi`, `arboard`; v3/v8 checks scan `packages/brawler-client/src`.
- Docs: `docs/01-engine-decision.md` (packages, features), `AGENTS.md` stack/source layout,
  `README.md`, and path references in durable docs/catalog comments. Historical records untouched.

No wire, protocol, fingerprint, persistence, or behavior change: `brawler-server routing-identity`
is identical before/after (`network_protocol=4778953997791089217`,
`protocol_registry_fingerprint=18160765825630381060`, `content_fingerprint=7772007507332977560`,
also equal to the owner's running supervisor), and the same 2317 tests pass.

## Post-split measurements

Same machine, same profile, fresh scratch target dir, sequential:

| Step | Before | After |
|---|---|---|
| `just check` (clean) | 155 s | 162 s |
| clean client build (`--timings`) | 367 s (`brawler` 119 s) | 323 s (`brawler` 51 s + `brawler-client` 63 s) |
| clean server build (`--timings`) | 131 s (`brawler` 52 s) | 119 s (`brawler` 51 s) |
| `just test` | 896 s, 2317 passed | 681 s, 2317 passed |
| `just lint` | 74 s (stopped at `_client-features`, see baseline) | 67 s, all recipes green |
| `target/` after check + test + lint | 57.1 GB | 52.6 GB (deps 32.7 GB, incremental 17.8 GB) |

Incremental matrix, mean of the add/revert samples, before -> after (after targets are the
equivalent package selections: `build-client` = `cargo build -p brawler-client`, `testlib-client` /
`test-client-all` = `-p brawler -p brawler-client --features brawler/client,brawler-client/automation`
with `--lib` / `--all-targets --no-run`):

| Edit | build-client | build-server | testlib-client | testlib-server | test-client-all |
|---|---|---|---|---|---|
| client-presentation | 6.2 → 4.4 (1.4×) | 0.2 → 0.2 | 5.8 → 3.5 (1.6×) | 0.2 → 0.2 | 8.1 → 3.7 (2.2×) |
| client-presentation-body | 40.6 → 28.8 (1.4×) | 0.2 → 0.2 | 51.2 → 27.1 (1.9×) | 0.2 → 0.2 | 41.6 → 29.5 (1.4×) |
| combat-rule | 0.2 → 0.2 | 4.0 → 3.8 (1.0×) | 0.2 → 0.2 | 3.2 → 3.2 (1.0×) | 0.2 → 0.2 |
| combat-rule-body | 36.2 → 7.2 (5.1×) | 11.5 → 11.2 (1.0×) | 32.9 → 13.3 (2.5×) | 15.3 → 15.3 (1.0×) | 29.7 → 6.1 (4.9×) |
| protocol | 5.5 → 8.0 (0.7×) | 3.0 → 3.8 (0.8×) | 4.8 → 8.3 (0.6×) | 2.8 → 3.2 (0.9×) | 6.3 → 6.6 (1.0×) |
| protocol-body | 34.9 → 28.2 (1.2×) | 15.8 → 15.5 (1.0×) | 41.5 → 36.3 (1.1×) | 18.0 → 17.6 (1.0×) | 46.2 → 25.9 (1.8×) |

- A client-only edit recompiles only `brawler-client` (cargo `Compiling` lines in the bench logs);
  no `brawler` or server artifact rebuilds.
- `_test-client` after a client-only edit: 2.2x for an item edit, 1.4x for a function-body edit
  (41.6 s -> 29.5 s). The 2x target is met for item edits but not for body edits; the remaining
  cost is `brawler-client` codegen plus test/bin linking (follow-up BRL-r8ev).
- Biggest win is shared gameplay body edits in client configurations (36 s -> 7 s, 30 s -> 6 s):
  `brawler` recompiles only its half and the downstream client crate reuses almost everything.
- Regression: item-level protocol edits cost 1-3 s more because both crates rebuild.

## Verification (logs in the session scratchpad `brl0170/`)

- `just check`, `just test`, `just lint` (real ripgrep first on PATH): green (`final-*.log`).
- Role checks: `scripts/check-server-features.sh`, `scripts/check-client-features.sh` green inside lint.
- `just e2e 2` and `just practice-e2e wipeout-1v1` equivalents through a scratch
  `network-product-match.sh` (scratch `--data-directory`, ephemeral 127.0.0.1 port, scratch target):
  "1 exact 1v1 roster(s) reached Active" and "Practice wipeout-1v1 reached Active with one human".
  Tracked `data/server/` checksums unchanged.
- Android: `cargo ndk --target arm64-v8a --platform 31 build --locked -p brawler-android --profile android`
  finished (3m25s). No gradle, no adb.
- Python evidence-script unit tests: 38 OK.
- Not run: `just ci` 4/6-client and full Practice matrices, `network-change-e2e`, native/windowed play.

## Deferred

- BRL-r8ev: cheaper dev debuginfo profile + `just sweep` (assumed by this spec, never landed).
- BRL-fk77: pre-existing `clippy::float_cmp` in `tests/network/builds.rs` under `network-test,balance-lab`.
- BRL-nrab: narrow the cross-crate `pub` surface listed above.

## Learn from errors

- Item-level probes understated real edit cost 5-8x; measure with function-body edits too.
- The first "shared combat" probe hit a server-only file; confirm a probe file is compiled in
  every measured configuration (the client build staying at 0.2 s gave it away).
- Re-exporting shared modules at the client crate root collided with `use super::*` globs and
  `unused_qualifications`; explicit `brawler::` paths were cleaner than re-rooting.
- Scripts that hard-code `target/debug` break under a custom `CARGO_TARGET_DIR`; verification
  that must not touch the owner's `target/` needs scripts that honor it.
- Check type-name-sensitive fingerprints (Lightyear registry hashes type paths) before choosing
  which side of a split keeps the original crate name.
