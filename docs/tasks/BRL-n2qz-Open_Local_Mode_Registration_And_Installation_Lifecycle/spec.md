# Technical specification

## Scope and outcome

Open the process-local game-mode composition seam without changing Brawler's stable wire protocol. Wipeout, Hot Zone, and Heist each own one immutable registration plus a lightweight registration plugin. Runtime Apps receive a bounded validated `ModeRegistry`; pure map/catalog/manifest code receives an immutable built-in catalog built from those same registrations. Selected authoritative mode rules and plugins are installed through the registration callback during App construction.

This phase opens local registration, lookup, operator-rule resolution, and selected server-plugin installation. It does not make routed modes dynamically extensible, add a fourth production mode, hot-swap modes at runtime, or change any match behavior/value.

## Architecture

### Registration model

- Replace the static `MODE_DESCRIPTORS` array with `ModeRegistration` values owned beside Wipeout, Hot Zone, and Heist.
- Common registration fields include stable local definition ID, stable key, topology policy, and optional configured/routing/client/server projections.
- The server projection owns rules revision, default map, compatible-map policy, typed operator-rule resolver, and authoritative installer callback.
- The client projection owns the existing selection label. Presentation remains optional for a local-only synthetic registration but complete for every built-in routed mode.
- Installer input is one named `ModeInstallInput` containing the configured rules profile and existing optional objective/Wipeout/Heist policy values; do not retain an expanding positional callback signature.
- Define an explicit small registry capacity with bounded deterministic failure.

### Plugin lifecycle

- `ModeRegistryPlugin` installs a private builder during `Plugin::build` and validates/seals it during `Plugin::finish`, removing the mutable builder and inserting an immutable sorted `ModeRegistry` resource.
- `BuiltInModeRegistrationsPlugin` composes the three lightweight mode registration plugins. Each plugin contributes its owned registration through a crate-scoped registration helper.
- Duplicate definition IDs, keys, configured modes, and routing modes fail deterministically. Capacity overflow and incomplete built-in coverage fail closed.
- Registration order cannot affect sealed registry order or lookup behavior.
- `install_configured_server_mode` looks up the selected registration from the builder during App construction and immediately executes its server installer. Adding a Bevy plugin is not deferred until `finish`, because plugin finalization is too late for selected gameplay composition.
- Exactly one authoritative mode plugin remains installed per match worker/direct server. No runtime hot-swap is introduced.

### Pure pre-App path

- Build a process-static immutable `builtin_mode_catalog()` from the exact same three mode-owned registration constants.
- Pure map resolution, operator catalog parsing, worker CLI/manifest decoding, and other pre-World consumers use this catalog and never depend on `Res<ModeRegistry>` or construct a temporary App.
- The remaining centralized list of built-in registration constants is composition, not behavior dispatch; metadata and callbacks are defined only once at the owning mode.
- Local-only synthetic registrations may omit configured/routing projections, proving extension without inventing a protocol identity. They cannot enter routed/operator production paths until a deliberate stable wire change adds those projections.

### Operator rules

- Move the Wipeout/Hot Zone/Heist objective validation branch from `server/lobby/catalog.rs` behind each mode's typed operator-rule resolver callback.
- Give the resolver one bounded common input containing the authored objective fields, resolved lifecycle, match duration, and the existing mode policy values it needs.
- Return a `ResolvedModeRuleProjection` containing `objective_target` and the existing closed `AdvertisedRulesSummary` wire projection.
- Preserve exact validation/rejection precedence and serialized advertised values. `AdvertisedRulesSummary` remains intentionally closed.

### Organization

- Convert `src/modes.rs` into a cohesive `src/modes/` module:
  - `mod.rs` — composition, common API/re-exports, built-in catalog access;
  - `registry.rs` — builder, sealed resource/catalog, validation, registration helper, tests;
  - `wipeout.rs`, `hot_zone.rs`, `heist.rs` — owned registration, rule resolver, installer.
- Keep match lifecycle/objective systems and their schedule sets in `matchplay/{wipeout,hot_zone,heist}.rs`.
- Keep wire encoding/decoding and routed enums unchanged in `protocol.rs`, `config.rs`, and `brawler-routing`.

## Preserved invariants

- Existing configured `GameMode`, routing `GameMode`, definition IDs, keys, topology policies, rules revisions, labels, default maps, compatible-map behavior, objective values, and advertised summaries are byte-for-byte stable.
- Map resolution remains deterministic and callable without Bevy App construction.
- Fixed schedules, restart ordering, objective facts/cues, bots, HUD, replication, and authority ownership do not change.
- Production mode coverage is exact for `GameMode::ALL`; no fallback silently selects another mode.
- Registry resources are immutable after App finalization and unavailable as a mutation path for gameplay systems.
- Client feature graph never gains server installers/rules; server graph never gains client presentation dependencies.

## Acceptance criteria

1. Wipeout, Hot Zone, and Heist each own and plugin-register one registration; no central mode-specific installer or operator-rule match remains.
2. A bounded runtime `ModeRegistry` seals during plugin finalization, removes its builder, sorts deterministically, and rejects duplicate IDs/keys/configured modes/routing modes, missing built-ins, and capacity overflow.
3. Reversed plugin registration order produces the same sealed registry.
4. A synthetic local-only registration appears through one plugin without editing registry validation/dispatch and without requiring a routed enum value.
5. `builtin_mode_catalog()` derives from the same registration constants and remains available to pure map, catalog, admission, and worker paths.
6. Every configured built-in mode installs exactly the prior `MatchModeSetup`, rules resource, default map selection, and one matching authoritative mode plugin.
7. Operator catalog valid/invalid fixtures preserve exact summaries, objective targets, rejection precedence, and golden catalog revision.
8. All built-in map presets retain topology and compatible-mode validation.
9. Client selection labels resolve through the finalized runtime registry where an App resource is naturally available; pre-App callers remain on the pure catalog.
10. No protocol message/component/input/channel, routed enum, stable ID, or authored balance value changes.
11. Client, server, combined network-test, and Balance Lab feature graphs remain isolated and warning-free.
12. Durable architecture documentation explains local registration versus intentionally closed wire evolution.

## Implementation plan

1. Extract common registration/catalog/registry lifecycle into `src/modes/{mod,registry}.rs` with focused validation and reversed-order/synthetic tests.
2. Move each built-in registration, authoritative installer, and typed operator resolver into its owned mode module.
3. Install registry and built-in registration plugins in client/server application composition; route configured server installation through the builder before finalization.
4. Adapt pure lookup consumers to `builtin_mode_catalog()` and App-native client lookup to `Res<ModeRegistry>`.
5. Replace operator objective dispatch with registration-owned resolvers and preserve fixtures/golden revisions.
6. Add exact installation, role-isolation, pure topology, and synthetic-extension tests.
7. Document the boundary in `docs/08-network-architecture.md` or the closest durable mode architecture section.
8. Run focused modes/catalog/map/server/client tests, role-specific checks, `just check`, `just lint`, and `just test`. Native evidence is not required because behavior and presentation output are unchanged.

## Verification and evidence

Record exact commands/results before closeout. Compare any golden/catalog revision output explicitly. No native playtest is required unless implementation changes player-visible output, which is outside approved scope.

## Scope exclusions

- New production game mode or mode-specific gameplay.
- Routed/protocol enum or compatibility migration.
- Runtime registration, hot reload, or mode hot-swap.
- Dynamic executable/plugin ABI.
- General command bus, dependency injection, or service locator.
- Tile-handler, VFX/audio, or content-fingerprint work already owned by other phases.

## Implementation evidence — 2026-08-31

Implemented the process-local mode registration seam exactly within scope. `src/modes.rs` is now a cohesive `src/modes/` tree; Wipeout, Hot Zone, and Heist own their registration metadata, typed operator resolver, policy validator, and authoritative installer. `ModeRegistryPlugin` owns the bounded builder/seal lifecycle, `BuiltInModeRegistrationsPlugin` composes built-ins, the selected server plugin installs before finalization, and pure pre-App consumers use `builtin_mode_catalog()` assembled from the same constants. Client game selection reads the finalized `ModeRegistry`. Protocol/routing enums and stable IDs were not changed.

Verification passed:

- Server mode suite: 12 passed, including duplicate/capacity/coverage failures, reverse-order sealing, synthetic local registration, missing server projection, and exact selected-plugin/resource installation.
- Client mode suite: 6 passed, including missing presentation projection and runtime-registry extension coverage.
- Operator catalog suite: 7 passed; exact rejection context and fail-fast Wipeout/Heist policy precedence pass; golden public catalog digest remains `4fe08f5b69cb3d54ac960e6813ac4a5c3518cafbe1447672ab55c39cb4498832`.
- Map suite: 29 passed; built-in topology and compatible-map behavior remain unchanged.
- Focused client game-selection and client/server/lobby-worker composition tests passed.
- Combined `client,server,network-test` check and `server,balance-lab` check passed warning-free.
- `just check` passed.
- `just lint` passed, including all Clippy, feature-isolation, presentation-retirement, and map-cleanup gates.
- `just test` passed: routing 83 plus process/isolation suites, client 509, server 482, Balance Lab 504, combined Balance Lab network scenario 1, network integration 97, and performance 12.
- `git diff --check` passed.
- Independent code review found no remaining correctness or lifecycle issue after the shared manual-update client fixture was finalized before its first `App::update()`.

No native playtest was required: player-visible labels, values, presentation output, fixed schedules, and network behavior are unchanged.

## Learn-from-errors review

The first client flow fixture added registry plugins but called `App::update()` without plugin finalization. Bevy does not implicitly run `Plugin::finish` for manual updates, so a system requiring `Res<ModeRegistry>` would have failed. The shared fixture now calls `crate::test_app::finalize` before its first update. Reusable prevention: any manual-update test that consumes a resource sealed in `Plugin::finish` must use the repository finalization helper before schedule execution.

Clippy also rejected Hot Zone's no-op policy validator as an unnecessary `Result`. The return type is intentional because all registration-owned validators share one fail-closed callback signature; the exception is now narrow and reasoned on that one function. Reusable prevention: keep uniform extension callbacks typed consistently and document zero-policy implementations locally rather than weakening lint globally.
