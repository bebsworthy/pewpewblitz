# Milestone 01 — Rust and Bevy application foundation

## Tracking

| Field | Value |
|---|---|
| Version | v1 — gameplay MVP |
| Roadmap | [roadmap.md](./roadmap.md) |
| Status | Not started |
| Specification validation | Pending |
| Implementation | Not started |
| Verification | Not started |
| User validation/playtest | Not started |

Update this table and the roadmap together whenever the milestone changes phase.

## Outcome

Create the smallest Bevy/Rust foundation that can build and launch a macOS client application and a dedicated headless-server application predictably. Establish Bevy-native composition, a verified Cargo feature graph, fixed-tick ownership, development commands, and CI without prematurely fixing the number of packages, crates, libraries, or public APIs.

This milestone proves application composition and dependency isolation. It does not prove networking or gameplay.

## Source requirements

- [Engine decision](../../01-engine-decision.md)
- [Gameplay MVP](../../05-gameplay-mvp.md)
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

- [ ] Inventory the relevant Bevy and Lightyear files and record exact paths in the research log.
- [ ] Confirm the exact Rust toolchain supported by Bevy 0.19 and Lightyear 0.29.
- [ ] Identify every API taken from the 0.20-dev Bevy snapshot and verify its Bevy 0.19 equivalent before specifying it.
- [ ] Confirm the smallest Lightyear feature set needed to compile client and server composition without implementing connections.

### Project topology and feature graph

- [ ] Compare at least these topology candidates: one package with feature-gated targets/modules; one package with a reusable library target and separate binaries; and a small workspace with separate client/server packages only where feature isolation requires it.
- [ ] Evaluate each candidate for Cargo feature unification, duplicate composition code, independent client/server builds, headless dependency isolation, integration-test ergonomics, incremental compile cost, and future host-client testing.
- [ ] Decide where minimal gameplay systems and protocol registration belong across packages, modules, or plugins. Do not create a crate solely to name a conceptual layer.
- [ ] Define the supported Cargo feature/build matrix, including which combinations are valid, invalid, or intentionally unsupported.
- [ ] Prove from `cargo metadata` and `cargo tree -e features` that the selected dedicated-server build does not enable rendering, windowing, audio, device-input, or client-asset features.

### Bevy and Lightyear composition

- [ ] Inspect Bevy's headless, plugin, and plugin-group examples and identify the smallest appropriate base-plugin sets for client and server.
- [ ] Inspect Lightyear `simple_setup` for its single-package, feature-gated client/server/host-client composition and document which parts fit Brawler and which are example-only convenience.
- [ ] Inspect `simple_box` for shared/plugin/protocol placement and authoritative topology; explicitly defer prediction, interpolation, P2P, and host-client behavior.
- [ ] Define the plugin responsibilities and ordering for base application setup, protocol registration, authoritative gameplay, any gameplay genuinely reused by a future predicted client, client presentation, and dedicated-server hosting.
- [ ] Define fixed-tick ownership once, including how both application configurations receive the same tick duration without duplicating constants.
- [ ] Define initial schedules or system sets only where they establish an ordering contract needed by upcoming milestones; avoid an empty taxonomy.

### Workflow and verification

- [ ] Define startup configuration for current needs only, such as logging and explicit client/server mode if the chosen target layout needs it. Defer network address, port, and client identity to Milestone 02.
- [ ] Define canonical commands for formatting, linting, tests, client build/run, and dedicated-server build/run on macOS.
- [ ] Define CI checks for every supported build configuration, feature-graph isolation, and plugin-composition smoke tests.
- [ ] Decide how future asset/data/provenance locations will be documented without creating unused directories.

## Research log

Record primary sources, inspected examples, findings, and implications. Do not convert an unverified finding into a technical decision.

| Date | Local path or source | Finding | Implication/decision |
|---|---|---|---|
| 2026-08-13 | `references/lightyear/examples/simple_setup/{Cargo.toml,src/main.rs,src/shared.rs}` | Lightyear demonstrates one package with additive client/server/transport features and Bevy plugins; it does not require separate client/server crates. | Treat this as one topology candidate and test feature isolation rather than copying it blindly. |
| 2026-08-13 | `references/lightyear/examples/simple_box/{Cargo.toml,src/lib.rs,src/main.rs}` | Shared, protocol, client, server, and renderer concerns can be modules/plugins behind Cargo features in one package. | Keep package versus module boundaries open until the feature graph is measured. |
| 2026-08-13 | `references/bevy/examples/app/{headless.rs,plugin.rs}` | Bevy's native composition units are plugin sets and schedules; headless operation is primarily a Bevy-feature/base-plugin concern. | Specify plugin composition and enabled features, not facades or adapters. |
| 2026-08-13 | `references/bevy/Cargo.toml`, `references/lightyear/Cargo.toml` | The Bevy snapshot is 0.20-dev while Lightyear 0.29 pins Bevy 0.19. | Validate exact APIs against Bevy 0.19 during formal research. |

These entries document the planning review only. Formal milestone decisions remain pending research and user validation.

## Technical specification

Status: **Pending research and user validation.**

### Decisions

| Decision | Selected option | Alternatives | Evidence and tradeoffs | Validation |
|---|---|---|---|---|
| Repository/package topology | Pending | Single package / library plus binaries / small workspace | Pending | Pending |
| Cargo target and feature matrix | Pending | Pending | Pending | Pending |
| Rust toolchain | Pending | Pending | Pending | Pending |
| Bevy and Lightyear features | Pending | Pending | Pending | Pending |
| Client base-plugin composition | Pending | Pending | Pending | Pending |
| Headless-server base-plugin composition | Pending | Pending | Pending | Pending |
| Brawler plugin/module composition | Pending | Pending | Pending | Pending |
| Fixed tick and schedule ownership | Pending | Pending | Pending | Pending |
| Startup configuration and logging | Pending | Pending | Pending | Pending |
| Local and CI command surface | Pending | Pending | Pending | Pending |

### Required composition constraints

- The dedicated-server build must not enable rendering, windowing, audio, device input, or client-asset capabilities.
- Gameplay components and systems must install only where they execute without pulling unrelated client-presentation or server-hosting concerns. Systems intended for both server authority and future client prediction may share a module or plugin; server-only rules need not be client-installable. A pure domain crate is not required.
- Protocol registration used by both application configurations must use stable network/definition identifiers and must not expose process-local ECS entity identity across the wire.
- Client presentation and dedicated-server hosting must compose through explicit Bevy plugins or composition functions. Separate client/server library crates and public facades are not requirements.
- Fixed-step systems must have one documented tick-duration source and explicit schedule/system-set ordering where ordering matters.
- Cargo features are additive. The specification must explain how the selected package/target layout avoids a supposedly headless server inheriting client-only features.
- Binaries may parse process-level configuration and compose an `App`, but gameplay rules must live in ECS systems/plugins that can be exercised without launching a process.

### Required composition map

Pending research. The validated map must show:

- Cargo packages and targets, if more than one exists;
- feature gates and the supported build matrix;
- Bevy plugins and which client/server configurations install them;
- module ownership for authoritative gameplay, genuinely shared prediction behavior, protocol registration, and client presentation;
- relevant schedule and system-set ordering;
- dependency arrows and the concrete boundary each split enforces;
- test placement and how plugin composition is exercised.

Do not require “facades” or “adapters” in this map. Name external boundaries by their concrete responsibility, such as transport, runtime configuration, filesystem access, or platform input.

### Configuration and error behavior

Pending research. Specify only configuration consumed in this milestone, its defaults and precedence, invalid-value behavior, process exit behavior, and useful structured startup logs. Network endpoint and client-identity configuration belongs to Milestone 02.

## Trackable implementation plan

Do not start these tasks until the technical specification is validated by the user.

### Cargo and application topology

- [ ] Pin the validated Rust toolchain and exact dependency versions.
- [ ] Create the validated package, target, module, and Cargo-feature topology.
- [ ] Implement the supported client and dedicated-server composition roots.
- [ ] Implement the validated Bevy plugins/modules for reusable application setup, client-only presentation startup, and server-only startup.
- [ ] Register the minimal protocol plugin in each application configuration that needs it, without adding network connections or gameplay messages.
- [ ] Configure one fixed-tick source and the minimum schedule/system-set contracts required by the next milestone.

### Development infrastructure

- [ ] Configure rustfmt and Clippy policy for every package and target.
- [ ] Add plugin-composition and startup-configuration smoke tests.
- [ ] Add structured logging and clear startup failure handling.
- [ ] Add CI for formatting, linting, tests, and every supported client/server feature combination.
- [ ] Add a reproducible feature-graph check for accidental client presentation dependencies in the dedicated-server build.

### Local workflow and repository conventions

- [ ] Document canonical commands for client and dedicated-server build/run workflows.
- [ ] Document future asset, authored-data, map, and third-party provenance locations; create directories only when they gain real content.
- [ ] Update `AGENTS.md` or the root README with commands established by the implementation.

## Test plan and evidence

### Structural and feature-graph verification

- [ ] Every supported Cargo feature/target combination builds independently.
- [ ] Invalid or unsupported feature combinations fail clearly or are excluded by a documented command surface.
- [ ] `cargo metadata` and `cargo tree -e features` evidence confirms that the dedicated-server configuration excludes client rendering, windowing, audio, device-input, and asset-presentation features.
- [ ] The minimal gameplay and protocol-registration plugins compose in their intended application configurations without requiring separate crates.
- [ ] No architecture test encodes a facade, adapter, service-layer, or pure-domain-crate topology.

### Unit and plugin-composition tests

- [ ] Startup configuration accepts valid values and reports invalid values clearly.
- [ ] A minimal test `App` can install the reusable non-presentation plugin set without windowing or rendering.
- [ ] The client composition test installs the expected base, protocol, gameplay, and presentation responsibilities selected by the specification.
- [ ] The dedicated-server composition test installs the expected headless, protocol, and gameplay responsibilities selected by the specification.
- [ ] Fixed-tick configuration is identical in both compositions and schedule/system-set ordering is asserted where the specification declares an ordering contract.

### Process smoke tests

- [ ] The macOS client launches to a blank responsive state from the documented command.
- [ ] The dedicated server launches headlessly from the documented command and shuts down cleanly.
- [ ] Both processes emit useful startup mode, version, and tick-configuration logs.
- [ ] No connection or multi-client behavior is required until Milestone 02.

### Visual check

- [ ] The blank client window opens, remains responsive, and closes cleanly.
- [ ] Startup failures are visible and actionable.
- [ ] No gameplay presentation is required in this milestone.

Record exact commands, feature sets, dependency evidence, dates, and results here once verification begins.

## User validation and handoff

### Specification review

- Date: Pending
- User decision: Pending
- Required changes: Pending

### Smoke-test handoff

- Build/run instructions: Pending implementation
- Expected client result: Blank responsive client application
- Expected server result: Headless process with useful startup logs and clean shutdown
- Known limitations: No networking or gameplay in Milestone 01
- Requested user observations: startup reliability, command clarity, shutdown behavior, and useful error output

## Feedback review

| ID | Feedback | Decision | Rationale | Task/backlog link |
|---|---|---|---|---|
| — | No feedback yet | — | — | — |

## Learn from errors

Complete after implementation and feedback:

- What went wrong or caused rework?
- Which Bevy, Lightyear, Cargo-feature, or version assumption caused it?
- Which test, checklist, document, or composition rule would prevent recurrence?
- Is the lesson reusable enough to create or improve a Bevy/Lightyear project skill?
- Which roadmap or future-milestone assumptions must change?

## Exit checklist

- [ ] Research questions are resolved or explicitly deferred with rationale.
- [ ] Technical specification and implementation plan are validated by the user.
- [ ] All accepted implementation tasks are complete.
- [ ] Formatting, linting, tests, and every supported independent build pass.
- [ ] Client and dedicated server launch from documented commands and shut down cleanly.
- [ ] Dedicated-server feature isolation is verified with recorded dependency evidence.
- [ ] Plugin composition and fixed-tick ownership are verified without enforcing a layered architecture.
- [ ] User smoke-test feedback is incorporated or triaged.
- [ ] Learn-from-errors review is complete.
- [ ] Reusable skills are created or improved where justified.
- [ ] Roadmap status and current milestone are updated.
