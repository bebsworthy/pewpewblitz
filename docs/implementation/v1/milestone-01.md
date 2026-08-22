# Milestone 01 — Rust and Bevy application foundation

## Tracking

| Field | Value |
|---|---|
| Version | v1 — gameplay MVP |
| Roadmap | [roadmap.md](./roadmap.md) |
| Status | Complete (2026-08-18) |
| Specification validation | Implemented from the checked-in scope contract on 2026-08-13 |
| Implementation | Complete |
| Verification | Complete |
| User validation/playtest | Closed by the final v1 basic playtest; no M01 startup or shutdown blocker reported |

Update this table and the roadmap together whenever the milestone changes phase.

## Outcome

Create the smallest Bevy/Rust foundation that can build and launch a macOS client application and a dedicated headless-server application predictably. Establish Bevy-native composition, a verified Cargo feature graph, fixed-tick ownership, development commands, and CI without prematurely fixing the number of packages, crates, libraries, or public APIs.

This milestone proves application composition and dependency isolation. It does not prove networking or gameplay.

## Source requirements

- [Engine decision](../../01-engine-decision.md)
- [Gameplay MVP](./gameplay-mvp.md)
- [Network architecture](../../08-network-architecture.md)
- [Version 1 roadmap](./roadmap.md)

## Architecture guidance priority

Apply architecture guidance in this order:

1. Brawler's gameplay, authority, platform, and delivery requirements.
2. The checked-in Lightyear 0.29 examples and book.
3. Official Bevy 0.19 source and documentation for version-specific APIs.
4. Bevy-native ECS, plugin, schedule, system-set, state, asset, and Cargo-feature patterns.
5. General Rust API, dependency, and testability hygiene.
6. Ports/adapters concepts only at genuine external boundaries when they solve an observed problem; they are not the default game architecture.

Do not use a server-oriented DDD or hexagonal architecture as the governing model for this milestone. Bevy `App` composition and ECS ownership are the primary design vocabulary.

## Local implementation references

Inspect these checked-in sources before selecting APIs or project topology:

- [Bevy examples index](../../../references/bevy/examples/README.md), especially `app/headless.rs`, `app/plugin.rs`, and `app/plugin_group.rs`;
- [Lightyear examples index](../../../references/lightyear/examples/README.md), starting with `simple_setup`, then `simple_box`;
- [Lightyear book summary](../../../references/lightyear/book/src/SUMMARY.md), especially setup, client/server construction, shared plugins, protocol registration, and Bevy system ordering;
- the root `Cargo.toml` files for both snapshots, to verify their Bevy and Rust versions before transferring an API or pattern.

The Lightyear snapshot is version 0.29 and pins Bevy 0.19. The checked-in Bevy snapshot is 0.20-dev, so its examples are architectural references only until each used API is confirmed against Bevy 0.19. During milestone research, prefer Lightyear's pinned source and official Bevy 0.19 documentation for exact APIs.

Record exact inspected files in the research log. Read an example's README, source, and `Cargo.toml` feature declarations together because application topology and feature flags are part of the pattern. Treat `references/` as read-only upstream material.

## Scope boundaries

### In scope

- exact Rust toolchain and dependency/version policy;
- evidence-based choice of Cargo package, target, module, plugin, and feature boundaries;
- independently buildable macOS-client and dedicated-server configurations;
- Bevy base-plugin composition for windowed and headless applications;
- minimal protocol-registration and gameplay-plugin composition sufficient to prove feature isolation and plugin reuse, without networking behavior;
- fixed-tick configuration and explicit schedule/system-set ownership;
- formatting, linting, tests, logging, CI, and canonical local commands;
- startup configuration and failure behavior needed by this milestone;
- conventions for future runtime assets, authored data, maps, and third-party provenance.

### Out of scope

- network connections, transport configuration, client identity, or replicated entities;
- multi-client orchestration and in-process host-client topology;
- movement, collision, combat, maps, or game modes;
- Avian 2D; Milestone 03 owns the collision approach and dependency decision;
- production hosting, matchmaking, accounts, or persistence;
- empty placeholder directories or speculative abstractions for future content systems;
- public library APIs that have no current consumer.

## Research questions

### Version and source validation

- [x] Inventory the relevant Bevy and Lightyear files and record exact paths in the research log.
- [x] Confirm the exact Rust toolchain supported by Bevy 0.19 and Lightyear 0.29.
- [x] Identify every API taken from the 0.20-dev Bevy snapshot and verify its Bevy 0.19 equivalent before specifying it; no 0.20-only API was transferred.
- [x] Confirm the smallest Lightyear feature set needed to compile client and server composition without implementing connections.

### Project topology and feature graph

- [x] Compare at least these topology candidates: one package with feature-gated targets/modules; one package with a reusable library target and separate binaries; and a small workspace with separate client/server packages only where feature isolation requires it.
- [x] Evaluate each candidate for Cargo feature unification, duplicate composition code, independent client/server builds, headless dependency isolation, integration-test ergonomics, incremental compile cost, and future host-client testing.
- [x] Decide where minimal gameplay systems and protocol registration belong across packages, modules, or plugins. Do not create a crate solely to name a conceptual layer.
- [x] Define the supported Cargo feature/build matrix, including which combinations are valid, invalid, or intentionally unsupported.
- [x] Prove from `cargo metadata` and `cargo tree -e features` that the selected dedicated-server build does not enable rendering, windowing, audio, device-input, or client-asset features.

### Bevy and Lightyear composition

- [x] Inspect Bevy's headless, plugin, and plugin-group examples and identify the smallest appropriate base-plugin sets for client and server.
- [x] Inspect Lightyear `simple_setup` for its single-package, feature-gated client/server/host-client composition and document which parts fit Brawler and which are example-only convenience.
- [x] Inspect `simple_box` for shared/plugin/protocol placement and authoritative topology; explicitly defer prediction, interpolation, P2P, and host-client behavior.
- [x] Define the plugin responsibilities and ordering for base application setup, protocol registration, authoritative gameplay, any gameplay genuinely reused by a future predicted client, client presentation, and dedicated-server hosting.
- [x] Define fixed-tick ownership once, including how both application configurations receive the same tick duration without duplicating constants.
- [x] Define initial schedules or system sets only where they establish an ordering contract needed by upcoming milestones; avoid an empty taxonomy.

### Workflow and verification

- [x] Define startup configuration for current needs only, such as logging and explicit client/server mode if the chosen target layout needs it. Defer network address, port, and client identity to Milestone 02.
- [x] Define canonical commands for formatting, linting, tests, client build/run, and dedicated-server build/run on macOS.
- [x] Define CI checks for every supported build configuration, feature-graph isolation, and plugin-composition smoke tests.
- [x] Decide how future asset/data/provenance locations will be documented without creating unused directories.

## Research log

Record primary sources, inspected examples, findings, and implications. Do not convert an unverified finding into a technical decision.

| Date | Local path or source | Finding | Implication/decision |
|---|---|---|---|
| 2026-08-13 | `references/lightyear/examples/simple_setup/{Cargo.toml,src/main.rs,src/shared.rs}` | Lightyear demonstrates one package with additive client/server features and a shared protocol plugin; its transport and connection setup are beyond this milestone. | Use one package and defer transport/connection features to Milestone 02. |
| 2026-08-13 | `references/lightyear/examples/simple_box/{Cargo.toml,src/lib.rs,src/main.rs,src/protocol.rs,src/shared.rs}` | Shared protocol and gameplay concerns compose as Bevy plugins/modules; protocol registration follows the Lightyear client/server plugin groups. | Reuse the plugin composition pattern without copying prediction, interpolation, P2P, renderer, or host-client behavior. |
| 2026-08-13 | `references/bevy/examples/app/{headless.rs,plugin.rs,plugin_group.rs}` | Bevy's native composition units are plugin groups, plugins, and schedules. `MinimalPlugins` provides headless scheduling; `DefaultPlugins` provides the windowed application base. | Use explicit base-plugin selection and focused Brawler plugins. |
| 2026-08-13 | `references/bevy/crates/bevy_internal/src/default_plugins.rs` | `DefaultPlugins` and `MinimalPlugins` are feature-sensitive; the minimal group does not include the terminal Ctrl-C handler. | Add `TerminalCtrlCHandlerPlugin` and `LogPlugin` explicitly to the dedicated server. |
| 2026-08-13 | `references/lightyear/crates/core/lightyear/src/{client.rs,server.rs,shared.rs,protocol.rs}`, `references/lightyear/crates/transport/{messages/src/registry.rs,transport/src/channel/registry.rs}` | Lightyear 0.29 uses client/server plugin groups, a shared fixed tick, and stable typed message registration. Protocol registration must happen after the application networking plugin group; this milestone registers only the shared message registry. | Install the shared protocol plugin after Brawler's base/gameplay setup and omit network entities/connections until Milestone 02. |
| 2026-08-13 | `references/bevy/Cargo.toml`, `references/lightyear/Cargo.toml` | The Bevy development snapshot is 0.20-dev, while Lightyear 0.29's workspace pins Bevy 0.19 and Rust 1.95. | Validate implementation APIs against released Bevy 0.19.1 and pin Bevy `=0.19.1`, Lightyear `=0.29.0`, Rust `1.95.0`. |

These entries record the implementation research used for the selected topology. Final v1 basic
playtest acceptance on 2026-08-18 closed the remaining M01 validation; release polish is tracked in
the v1 roadmap rather than reopening this foundation milestone.

## Technical specification

Status: **Complete; final basic v1 smoke-test validation accepted 2026-08-18.**

### Decisions

| Decision | Selected option | Alternatives | Evidence and tradeoffs | Validation |
|---|---|---|---|---|
| Repository/package topology | One package with a reusable library target and separate binaries | Single package with feature-gated targets/modules; small workspace | The library keeps gameplay/protocol composition reusable while the binaries remain process roots. A workspace would add no boundary at this stage. | `Cargo.toml`, both independent builds, and composition tests |
| Cargo target and feature matrix | `client` and `server` additive features; `brawler-client` and `brawler-server` require their matching feature | Unified default build; host-client; separate client/server crates | `--no-default-features --features client|server` makes the supported matrix explicit. Host-client and all-features builds are deferred until networking requires them. | `cargo metadata`, `cargo tree -e features`, CI matrix |
| Rust toolchain | Rust `1.95.0`, rustfmt, Clippy | Floating stable/nightly | Matches the Lightyear 0.29 snapshot and is recorded in `rust-toolchain.toml`. | Locked build and CI configuration |
| Bevy and Lightyear features | Bevy `=0.19.1`, Lightyear `=0.29.0` with no Lightyear transport/replication features; client adds render/window/winit/assets/font/keyboard features | Bevy default features; Lightyear default networking/prediction/transport | The server graph contains no render/window/audio/asset or device-input backend features. The core `bevy_input` crate remains an internal Bevy API dependency, not a device backend. | `scripts/check-server-features.sh` and `cargo tree -e features` |
| Client base-plugin composition | `DefaultPlugins` plus client presentation, gameplay, and protocol plugins | Manual full plugin list; shared default base for server | `DefaultPlugins` is Bevy-native and supplies the blank macOS window/render loop. | macOS launch smoke test |
| Headless-server base-plugin composition | `MinimalPlugins` with fixed schedule runner, explicit Ctrl-C handler, logging, gameplay, and protocol plugins | `DefaultPlugins` with rendering disabled; custom runner | `MinimalPlugins` avoids client platform/presentation capabilities while preserving deterministic scheduled execution. | Server build, feature script, process smoke test |
| Brawler plugin/module composition | `GameplayPlugin`, `ProtocolPlugin`, `ClientPresentationPlugin`, `DedicatedServerPlugin` | Facade/domain/service layers; one crate per concept | Plugins own concrete ECS setup. No speculative library boundary or process-local entity identity is in the protocol. | Unit and composition tests |
| Fixed tick and schedule ownership | `SIMULATION_TICK_HZ = 60`, `SIMULATION_TICK` duration, `Time<Fixed>`, `FixedUpdate`, chained `GameplaySet::{Input,Simulation,Finalize}` | Per-binary constants; an empty schedule taxonomy | The shared timing module is the one source; the tick counter proves `FixedUpdate` execution. | Fixed-tick unit test |
| Startup configuration and logging | No network endpoint or identity config yet; `RUST_LOG` filters structured logs; Ctrl-C requests Bevy app exit | Premature CLI/network config | Current process needs are mode/version/tick startup logs and clean signal shutdown only. | Client/server process smoke tests |
| Local and CI command surface | README commands plus macOS CI matrix for client/server format, lint, test, build, and isolation | Ad hoc cargo commands; all-features CI | Commands are reproducible with `--locked` and feature isolation is checked separately. | `README.md`, `.github/workflows/ci.yml` |

### Required composition constraints

- The dedicated-server build must not enable rendering, windowing, audio, device input, or client-asset capabilities.
- Gameplay components and systems must install only where they execute without pulling unrelated client-presentation or server-hosting concerns. Systems intended for both server authority and future client prediction may share a module or plugin; server-only rules need not be client-installable. A pure domain crate is not required.
- Protocol registration used by both application configurations must use stable network/definition identifiers and must not expose process-local ECS entity identity across the wire.
- Client presentation and dedicated-server hosting must compose through explicit Bevy plugins or composition functions. Separate client/server library crates and public facades are not requirements.
- Fixed-step systems must have one documented tick-duration source and explicit schedule/system-set ordering where ordering matters.
- Cargo features are additive. The specification must explain how the selected package/target layout avoids a supposedly headless server inheriting client-only features.
- Binaries may parse process-level configuration and compose an `App`, but gameplay rules must live in ECS systems/plugins that can be exercised without launching a process.

### Required composition map

Validated composition map:

```text
brawler (one package)
├── library target: shared GameplayPlugin + ProtocolPlugin + timing
├── brawler-client [feature=client]
│   └── DefaultPlugins + shared plugins + ClientPresentationPlugin
└── brawler-server [feature=server]
    └── MinimalPlugins + ScheduleRunner + Ctrl-C + LogPlugin + shared plugins
```

The dependency direction is intentionally shallow:

```text
client/server binary → composition root → shared Brawler plugins → Bevy ECS
                                            └→ Lightyear protocol registry
```

The validated map also shows:

- Cargo packages and targets, if more than one exists;
- feature gates and the supported build matrix;
- Bevy plugins and which client/server configurations install them;
- module ownership for authoritative gameplay, genuinely shared prediction behavior, protocol registration, and client presentation;
- relevant schedule and system-set ordering;
- dependency arrows and the concrete boundary each split enforces;
- test placement and how plugin composition is exercised.

Do not require “facades” or “adapters” in this map. Name external boundaries by their concrete responsibility, such as transport, runtime configuration, filesystem access, or platform input.

### Configuration and error behavior

Configuration is intentionally process-light in this milestone. The binary selects its composition through its Cargo-required feature; there is no endpoint, port, client identity, or network argument yet. `RUST_LOG` is consumed by Bevy's logging/tracing stack and otherwise uses Bevy defaults. Invalid future network configuration is deferred to Milestone 02 rather than silently accepted here. Startup emits mode, package version, and 60 Hz tick fields. Ctrl-C is handled by Bevy's terminal handler and the process exits successfully after the runner observes `AppExit`.

## Trackable implementation plan

Implementation was started directly from the user's explicit implementation request; the provisional topology was validated against the local Bevy/Lightyear references and the resulting build matrix.

### Cargo and application topology

- [x] Pin the validated Rust toolchain and exact dependency versions.
- [x] Create the validated package, target, module, and Cargo-feature topology.
- [x] Implement the supported client and dedicated-server composition roots.
- [x] Implement the validated Bevy plugins/modules for reusable application setup, client-only presentation startup, and server-only startup.
- [x] Register the minimal protocol plugin in each application configuration that needs it, without adding network connections or gameplay messages.
- [x] Configure one fixed-tick source and the minimum schedule/system-set contracts required by the next milestone.

### Development infrastructure

- [x] Configure rustfmt and Clippy policy for every package and target.
- [x] Add plugin-composition and startup-configuration smoke tests.
- [x] Add structured logging and clear startup failure handling.
- [x] Add CI for formatting, linting, tests, and every supported client/server feature combination.
- [x] Add a reproducible feature-graph check for accidental client presentation dependencies in the dedicated-server build.

### Local workflow and repository conventions

- [x] Document canonical commands for client and dedicated-server build/run workflows.
- [x] Document future asset, authored-data, map, and third-party provenance locations; create directories only when they gain real content.
- [x] Update the root README with commands established by the implementation.

## Test plan and evidence

### Structural and feature-graph verification

- [x] Every supported Cargo feature/target combination builds independently.
- [x] Invalid or unsupported feature combinations are excluded by the documented command surface (`--all-features` is intentionally unsupported).
- [x] `cargo metadata` and `cargo tree -e features` evidence confirms that the dedicated-server configuration excludes client rendering, windowing, audio, device-input, and asset-presentation features.
- [x] The minimal gameplay and protocol-registration plugins compose in their intended application configurations without requiring separate crates.
- [x] No architecture test encodes a facade, adapter, service-layer, or pure-domain-crate topology.

### Unit and plugin-composition tests

- [x] Startup configuration is explicit through feature-required targets; no invalid network values are accepted because network configuration is deferred.
- [x] A minimal test `App` installs the reusable non-presentation plugin set without windowing or rendering.
- [x] The client composition test installs the expected reusable gameplay, protocol, and presentation responsibilities; the actual `DefaultPlugins` base is covered by the macOS process smoke test because winit requires the main thread.
- [x] The dedicated-server composition test installs the expected headless, protocol, and gameplay responsibilities.
- [x] Fixed-tick configuration is identical in both compositions and the declared set chain is installed.

### Process smoke tests

- [x] The macOS client launches to a blank responsive state from the documented command.
- [x] The dedicated server launches headlessly from the documented command and shuts down cleanly.
- [x] Both processes emit useful startup mode, version, and tick-configuration logs.
- [x] No connection or multi-client behavior is required until Milestone 02.

### Visual check

- [x] The blank client window opens, remains responsive, and closes cleanly.
- [x] Startup failures are visible and actionable through Cargo errors and structured startup logs.
- [x] No gameplay presentation is required in this milestone.

Verification evidence (2026-08-13, macOS arm64, Rust 1.95.0):

- `cargo fmt --all -- --check` — passed.
- `cargo clippy --locked --no-default-features --features server --all-targets -- -D warnings` — passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings` — passed.
- `cargo test --locked --no-default-features --features server --all-targets` — 3 library tests passed; server binary target had 0 tests.
- `cargo test --locked --no-default-features --features client --all-targets` — 3 library tests passed; client binary target had 0 tests.
- `cargo build --locked --no-default-features --features server --bin brawler-server` — passed.
- `cargo build --locked --no-default-features --features client --bin brawler-client` — passed.
- `./scripts/check-server-features.sh` — passed. `cargo metadata` exposed only the package's additive `client`/`server` feature definitions; the server `cargo tree -e features` contained no `bevy_render`, `bevy_winit`, `bevy_window`, `bevy_audio`, `bevy_asset`, keyboard, mouse, gamepad, touch, or gesture backend features. The core `bevy_input` crate remains because Bevy 0.19's `bevy` crate includes it in its core API surface; no device backend is enabled.
- Dedicated server process smoke: `target/debug/brawler-server` emitted `mode="dedicated-server" version="0.1.0" tick_hz=60` and exited 0 after SIGINT.
- Client process smoke: `target/debug/brawler-client` created a blank `brawler-client` window on the Apple M3 Metal adapter, emitted `mode="client" version="0.1.0" tick_hz=60`, and exited 0 after SIGINT.
- Combined launcher smoke: `just` launched both configurations through Cargo, closing the client returned the launcher with status 0, and no Brawler processes remained. The launcher now monitors the server's Cargo-run status and returns it after stopping the client on server exit.

## User validation and handoff

### Specification review

- Date: 2026-08-13
- User decision: Implementation requested directly; local-reference review and build evidence accepted as the implementation basis.
- Required changes: None before verification.

### Smoke-test handoff

- Build/run instructions: From the repository root, run `just`. It builds the isolated server and client configurations, launches both, and forwards shutdown when you press Ctrl-C or close the client. Set `RUST_LOG=info` for startup diagnostics. The individual `cargo run` commands remain available for focused checks.
- Expected client result: Blank responsive client application
- Expected server result: Headless process with useful startup logs and clean shutdown
- Known limitations: No networking or gameplay in Milestone 01
- Requested user observations: startup reliability, command clarity, shutdown behavior, and useful error output. Please confirm both commands on your machine and report any window, log, or shutdown issue.

## Feedback review

| ID | Feedback | Decision | Rationale | Task/backlog link |
|---|---|---|---|---|
| F1 | User requested one command to launch the dedicated server and client together. | Implemented | Added a `just` launcher that builds both isolated targets, starts the server and client through Cargo's target resolution, monitors both exit statuses, and cleans up the other process on exit or failure. | [`justfile`](../../../justfile), [`README.md`](../../../README.md) |
| F2 | Closing the client window left repeated “No windows are open, exiting” logs and did not return to the prompt. | Implemented | The client now requests `AppExit` on `WindowCloseRequested`, and the launcher terminates its background server with SIGTERM, which is reliable for shell background jobs. | [`src/client.rs`](../../../src/client.rs), [`justfile`](../../../justfile) |
| F3 | Review found that the launcher could hide a server startup failure and bypass Cargo's configured target directory. | Implemented | The launcher supervises Cargo-run process statuses, stops the client when the server exits, returns the server status, and uses `cargo run` rather than a hard-coded repository target path. | [`justfile`](../../../justfile) |
| F4 | Review found that the fixed-tick test manually ran `FixedUpdate` and did not prove Bevy's fixed loop or set chain. | Implemented | The headless test now uses `MinimalPlugins` with `TimeUpdateStrategy::FixedTimesteps(1)`, advances through `App::update()`, and records the declared Input → Simulation → Finalize order. | [`src/gameplay.rs`](../../../src/gameplay.rs) |
| F5 | Review found that CI Clippy, tests, and builds omitted `--locked`. | Implemented | All dependency-resolving CI commands now enforce the committed lockfile. | [`ci.yml`](../../../.github/workflows/ci.yml) |
| F6 | Final v1 basic playtest was okay; further improvement is wanted before release, not during v1 closeout. | Accepted / deferred | No M01 foundation blocker was reported. Product polish is tracked as `POST-V1-RELEASE-POLISH`. | [v1 roadmap](./roadmap.md) |

## Learn from errors

Implementation review (2026-08-13):

- What went wrong or caused rework? The first fixed-tick constant used a non-const `Duration::from_secs_f64`; Bevy 0.19 resource initialization also requires marker resources to implement `Default`. A client composition test initially finalized winit on Cargo's non-main test thread. Review also identified that the launcher used a hard-coded target path, could mask server startup failure, and that the fixed-tick test bypassed Bevy's automatic loop.
- Which assumption caused it? Bevy 0.19's exact API behavior differed from the development snapshot, and macOS winit event-loop ownership is process-thread-specific.
- Prevention: keep exact released dependency pins, test the reusable plugin set with a headless `App`, and reserve actual `DefaultPlugins`/winit validation for a process smoke test on the main thread. Use `cargo run` when launching Cargo-built binaries, supervise sibling process statuses, and use `TimeUpdateStrategy::FixedTimesteps` when testing Bevy's fixed loop. Keep the shared tick duration as a const nanosecond duration.
- Reusable skill: no new skill was justified; the existing Bevy game-engine skill plus the checked-in local references covered the recurring setup decisions.
- Future-milestone impact: Milestone 02 must add Lightyear client/server plugin groups before any connection entities or expanded protocol registration; do not treat this milestone's protocol registry as network validation.

## Exit checklist

- [x] Research questions are resolved or explicitly deferred with rationale.
- [x] Technical specification and implementation plan are implemented from the user's direct request and validated by local evidence.
- [x] All accepted implementation tasks are complete.
- [x] Formatting, linting, tests, and every supported independent build pass.
- [x] Client and dedicated server launch from documented commands and shut down cleanly.
- [x] Dedicated-server feature isolation is verified with recorded dependency evidence.
- [x] Plugin composition and fixed-tick ownership are verified without enforcing a layered architecture.
- [x] User smoke-test feedback is incorporated or triaged; final basic v1 testing reported no M01 blocker.
- [x] Learn-from-errors review is complete, including the final user disposition.
- [x] Reusable skills were evaluated; no new skill was justified.
- [x] Roadmap status and current milestone are updated.
