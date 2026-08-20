# Brawler repository guide

## Quick orientation

Brawler is an original, cross-platform top-down arena shooter built around player-authored fighter builds. Combat readability, meaningful build tradeoffs, short matches, reusable content primitives, and server-authoritative networking are the core product constraints.

Start with:

1. `docs/00-product-direction.md` for product intent and non-goals.
2. `docs/implementation/v3/roadmap.md` for the active 3D-presentation migration, milestone order,
   and enduring V3 decisions.
3. `docs/implementation/v3/milestone-02.md` for the current implementation contract.
4. `docs/08-network-architecture.md` for enduring gameplay authority and replication boundaries.
5. `docs/13-player-ux.md` and `docs/14-multiplayer-server-architecture.md` for the completed V2
   player-flow and routed-process decisions that V3 preserves.
6. `docs/implementation/v2/roadmap.md` and `milestone-09.md` for the completed routed product
   baseline and closeout evidence.
7. `docs/implementation/v1/roadmap.md` and `milestone-11.md` for the completed gameplay MVP,
   verification evidence, deferred release polish, and the direct-UDP comparison baseline.

V1 completed on 2026-08-18 as a server-authoritative gameplay MVP after the final basic user
playtest. It is not a release-ready claim: controller feel, audio, HUD/readability, balance, pacing,
and related tuning remain tracked as `POST-V1-RELEASE-POLISH`. V2 completed and was accepted on
2026-08-20. V3 is the active version. M01 completed on 2026-08-20 after the user accepted the 3D
feasibility result and its projectile-origin corrections. M02 completed on 2026-08-20 after its
default 3D arena/map/terrain/camera/input cutover, projectile-placement feedback fix, removal of the
obsolete projectile sprite/XY writer, affected verification, and user acceptance. M03 is next and
not started; it moves remaining world visuals to dedicated 3D presentation entities. M04 audits
complete retirement of the legacy XY presentation convention.

## Technical stack

- The main Rust package provides independently buildable macOS-client and headless gameplay-worker
  configurations; `packages/brawler-routing` owns the completed V2 route/IPC protocol used by the
  supervisor, lobby worker, and match workers.
- Bevy 0.19 for ECS, application/plugin structure, client-side 2D/3D rendering, input, assets,
  animation, audio, and UI.
- Lightyear 0.29 for client/server transport, input networking, replication, interpolation, and later prediction/rollback where evidence justifies it.
- Avian 2D 0.7 for authoritative planar collision and generated terrain colliders. V3 does not
  replace it with 3D physics.
- Fixed-tick, dedicated-server-authoritative simulation from the first gameplay code.
- macOS is the initial client development target; local dedicated-server and multi-client testing are required.

Use Bevy's `World` as the runtime gameplay model. Keep authored definitions, selected builds, runtime ECS state, networking registration, and client presentation distinct without assuming that each concern needs a crate or architectural layer. The dedicated-server configuration must exclude rendering, windowing, audio, device input, and client assets. Networked types use stable player, match, and definition IDs rather than exposing process-local ECS entity identity across the wire.

Group focused systems, components, resources, and messages into cohesive plugins. Share a gameplay system between server and client only when it genuinely executes in both places, such as measured client prediction. Keep server-only authority rules on the server and presentation systems on the client. Create another package or public API only for a demonstrated feature-isolation, platform, compile-time, testing, or reuse boundary.

For architecture decisions, prioritize Brawler's gameplay and authority requirements, the local Lightyear material, verified Bevy 0.19 APIs, and Bevy-native patterns before general Rust architecture advice. Server-oriented DDD or hexagonal architecture is not the governing model; use ports/adapters only at a concrete external boundary where they solve an observed problem.

## Current source layout

The crate keeps one public gameplay/application API while organizing implementation by ECS state
ownership, execution role, plugin composition, and schedule phase:

```text
src/
  lib.rs                   shared crate module and role feature gates
  bin/{client,server}.rs   thin executable entry points
  gameplay.rs              shared fixed-tick schedule/set composition
  protocol.rs              wire registration and network protocol boundary
  config.rs                validated client/server/process configuration
  content.rs               build-embedded catalog loading and content fingerprints
  timing.rs                shared simulation time definitions
  abilities/
    mod.rs                 ability composition root, schedule sets, public re-exports
    charge.rs              ultimate charge ownership and outcome observation
    dash.rs                authoritative dash activation/movement/interruption
    sentry.rs              deployable activation, targeting, firing, and cleanup
    passives.rs            passive trigger/application rules
    telemetry.rs           bounded ability records and aggregates
    tests.rs               focused ability composition and behavior tests
  builds/
    mod.rs                 build composition root and public API
    model.rs               authored selections and resolved immutable loadouts
    definitions.rs         catalogs, validation, resolution, fingerprints
    server.rs              waiting-phase authoritative build transaction
    telemetry.rs           bounded selection/build records and aggregates
    tests.rs               focused build rule and composition tests
  combat/
    mod.rs                 combat composition root, public re-exports, shared sets/plugins
    model.rs               stable identities and shared/runtime combat state shapes
    cues.rs                gameplay-to-presentation combat facts
    definitions/           authored catalog, validation, resolution, fingerprints, tests
    authority.rs           authoritative fighter lifecycle and authority helpers
    attack.rs              economy, attack acceptance, firing expansion, attack telemetry
    delivery.rs            straight, lobbed, and melee delivery geometry/execution
    effects/               staged payload planning/application/runtime transaction and tests
    outcomes.rs            bounded authoritative outcome-fact ownership
    telemetry.rs           bounded records, trackers, aggregates, summaries
    evidence.rs            bounded process/checkpoint evidence and convergence schemas
    server.rs              server combat plugin and schedule registration
    client/                previews, cues, world visuals, transient effects, HUD, and tests
    tests.rs               shared combat model/composition tests
  map/
    mod.rs                 map composition root, stable IDs/profiles, public re-exports
    model.rs               recipe/resolved snapshot/runtime map shapes and indexes
    definitions/           catalog parsing, validation, resolution, fingerprints, tests
    server.rs              authoritative map generation install/teardown and colliders
    client.rs              replicated map reconstruction and client presentation
    tests.rs               shared map composition and lifecycle tests
  matchplay/
    mod.rs                 common match schedule/restart composition and mode plugins
    model.rs               stable match state, results, participants, and summaries
    lifecycle.rs           fighter defeat/respawn/reset lifecycle helpers
    server.rs              common authoritative roster, phase, restart, and outcomes
    spawns.rs              mode-neutral team assignment and deterministic spawn selection
    wipeout.rs             Wipeout scoring and mode-owned reset
    hot_zone.rs            Hot Zone occupancy/progress and mode-owned reset
    telemetry.rs           bounded match/mode records and aggregates
    tests.rs               focused common/mode lifecycle tests
  movement/
    mod.rs                 movement plugins and authoritative schedule composition
    arena.rs               arena definitions, geometry, colliders, and spawn helpers
    authority.rs           server-owned movement decisions, collision, and mutation
    input.rs               pure input shaping plus server validation/freshness rules
    tests.rs               focused movement tests
  client/
    mod.rs                 client application composition and shared client state
    assets.rs              retained visual/audio handles and readiness
    audio.rs               bounded cue-to-audio presentation
    hud.rs                 session, combat, build, match, and mode HUD
    input.rs               keyboard, mouse, gamepad, and native-input sampling
    presentation.rs        camera, arena, effects, and replicated-pose presentation
    presentation_3d/       accepted V3 camera/coordinate/GLB/mesh feasibility foundation; M02
                           decomposes map and terrain lifecycle into their owning modules
    session.rs             connection, selection, shutdown, and headless automation lifecycle
    settings/              local calibration/rebinding state and pause-overlay UI
    tests.rs               client composition and behavior tests
  diagnostics/
    mod.rs                 closeout schemas, aggregation, registration, and public API
    failure.rs             bounded process failure classification
    overlay.rs             client authority/network diagnostics presentation
    process.rs             process-owned report/checkpoint lifecycle
    tests.rs               schema, lifecycle, and validation tests
  server/
    mod.rs                 dedicated-server and connection/session composition
    verification.rs        process-only movement/combat evidence validation
    tests.rs               server composition and lifecycle tests
  terrain/
    mod.rs                 terrain composition root and public API
    model.rs               stable terrain/grid/runtime state shapes
    grid.rs                quantized occupancy rules and rasterization
    collider.rs            generated collider ownership and rebuild rules
    lifecycle.rs           install, reset, teardown, and generation transitions
    authority.rs           authoritative brush transaction
    network/               server publication and client convergence rules
    client/                presentation and recovery ownership
    telemetry.rs           bounded mutation/convergence evidence
    tests.rs               terrain rule, lifecycle, and schedule tests
tests/
  network.rs               integration-test composition entry point
  network/
    harness.rs             reusable separate-App/Crossbeam/UDP test harness
    *.rs                   scenarios grouped by lifecycle, movement, map, selection, builds, combat, recovery, and modes
  performance.rs           fixed-tick and subsystem performance/capacity gates
```

`content/v1/` owns build-embedded authored gameplay data. `references/` contains read-only upstream
material and is not part of Brawler's production module layout.

The routed supervisor, route envelope, IPC transport, and isolated lobby/match-worker composition
are completed V2 production paths. `just server`, `just client`, `just run`, and `just e2e` exercise
that routed topology; `scripts/network.sh` remains only the explicitly named legacy direct-UDP
diagnostic baseline. M02 is replacing the development-selected M01 renderer with the default 3D
arena/map/terrain/camera/input composition.

## Code organization rules

- Treat each `mod.rs` as a composition and intentional public-API surface, not an implementation
  dumping ground. It may define shared system sets/resources, install plugins/schedules, and
  re-export the small API used by sibling concerns. Put focused algorithms and lifecycle work in
  owned submodules.
- Choose a module boundary from responsibility and runtime ownership, not line count alone. Split
  when code has different state owners, execution roles, feature gates, schedule phases, reasons to
  change, or independently testable algorithms. Do not create one plugin, architectural layer, or
  file per type merely to make files shorter.
- A schedule-facing Bevy system should coordinate a recognizable phase. When it grows to combine
  validation, candidate collection, deterministic ordering, mutation, telemetry, and cue emission,
  extract named helpers or focused systems while keeping ordering explicit. Moving one giant
  function unchanged into another file is not decomposition.
- Preserve fixed-tick ordering and deferred-command boundaries during extraction. Keep meaningful
  `SystemSet`, `.before`/`.after`, `.chain()`, physics refresh, and `ApplyDeferred` relationships
  visible at the composition point; add schedule tests when changing them.
- Keep execution roles strict. Authoritative mutation belongs to server-gated combat, movement, or
  session modules. Client modules sample intent and present replicated state/cues. Process evidence
  and verification may observe gameplay but must not become a second gameplay or mutation path.
- Keep authored definitions, selected/resolved builds, mutable ECS runtime state, protocol
  registration, telemetry/evidence, and presentation as separate concerns. A shared wire shape does
  not authorize shared execution of server-only rules.
- Keep network registrations in `protocol.rs`; keep stable shared protocol/gameplay types in the
  appropriate shared model/cue/definition module. Never expose process-local `Entity` identity on
  the wire. Preserve public module paths and wire contracts during organization-only changes unless
  the active milestone explicitly approves and tests a protocol change.
- Follow `docs/08-network-architecture.md` for application protocol evolution: use the one global
  compatibility handshake and current schema, and do not introduce per-message versions or
  compatibility decoders without a new validated architecture decision.
- Default new items and submodules to private. Use `pub(crate)` for demonstrated cross-module use
  and public re-exports only for the crate API consumed by another role, integration tests, or a
  genuine external boundary. Avoid wildcard re-exports that accidentally turn implementation
  details into API.
- Feature-gate role-owned modules at their ownership boundary. The server feature graph must not
  acquire windowing, rendering, audio, device input, or client assets through a convenient shared
  module. Run role-specific checks after moving imports or types across client/server boundaries.
- Avoid module/file-wide complexity suppressions. A necessary Clippy exception for a Bevy system
  query or deterministic orchestration function should be attached narrowly to that item and remain
  reviewable. New `too_many_lines`/`too_many_arguments` findings are prompts to inspect ownership
  and decomposition before adding an allow.
- Place pure rule tests beside the owning module, using `tests.rs` when a focused module's tests
  would otherwise obscure production code. Put separate-App authority/replication behavior under
  `tests/network/`, reuse `harness.rs`, and group scenarios by behavior rather than accumulating
  them in `tests/network.rs` or duplicating harness setup.
- When a file is already large but cohesive, add new code only if it shares that exact ownership and
  lifecycle. A new concern should get a named submodule; recurring growth inside one system should
  be decomposed into testable helpers. Do not use a hard line limit as a substitute for this review.

## Value, maintainability, and no-over-engineering rules

- Deliver a complete player-visible vertical slice before building general infrastructure. A
  milestone should end with functional value a player can exercise, not only reusable machinery.
- Build for current demonstrated requirements. Do not model future screens, states, protocol
  variants, settings migrations, widget variants, or extension points before an owned use exists.
- Start with local, direct code. Extract a helper, module, plugin, crate, or public API only after
  duplication, distinct ownership, platform separation, testing needs, or another concrete cost
  demonstrates the boundary. A second real use is evidence; an imagined future use is not.
- Prefer Bevy-native components, resources, systems, states, events/messages, assets, and UI before
  adding a custom framework or dependency. Add another abstraction only when the native approach
  creates a specific observed problem.
- Optimize for obvious ownership and readable execution flow, not the number of layers. A small
  action enum and coordinating system are preferable to reducers, command buses, callbacks, or
  multi-stage state machines when the feature does not require those mechanisms.
- Preserve the boundaries that protect the product: server authority, execution-role isolation,
  stable wire identity, recoverable persistence, bounded state, and accepted automation paths.
  Avoid generalizing behavior outside those boundaries without evidence.
- Keep presentation optional around behavior. Animation, audio, effects, and transitions must not
  become the authority for navigation, saving, networking, shutdown, or gameplay state.
- Organize by responsibility and lifecycle rather than line count. A cohesive file may remain
  moderately large; split it when responsibilities or owners diverge and the resulting boundary is
  easier to understand and verify.
- Test costly risks and important contracts, not every combination. Use focused pure/ECS tests,
  representative integration cases, and a small visual/manual matrix. Do not multiply every state,
  input, resolution, scale, timing sample, and failure into a Cartesian suite without evidence.
- Reuse production components, canonical commands, and existing harnesses. Do not create a general
  abstraction solely to make one test possible unless production code also benefits from the seam.
- Record deferred polish and known limitations in the owning milestone or backlog. Do not expand the
  current slice incidentally to solve future work.
- Prefer the smallest clear implementation that owns today's behavior and is easy to change when a
  new requirement becomes real. Maintainability means clear ownership, limited scope, and safe
  change—not maximum abstraction.

## Local implementation references

Use the checked-in source and examples before guessing an API or copying an unrelated internet snippet, but verify snapshot versions before transferring exact APIs:

- `references/bevy/examples/` — official Bevy example source. Start with `README.md`, then locate focused examples with `rg`; useful foundation examples include `app/headless.rs`, `app/plugin.rs`, and `app/plugin_group.rs`.
- `references/lightyear/examples/` — official Lightyear example projects and their `Cargo.toml` feature sets. Start with `README.md`; use `simple_setup` for minimal client/server composition, `simple_box` for authoritative replication/prediction/interpolation, and `avian_2d` only when physics integration is in scope.
- `references/lightyear/book/` — local Lightyear book. Start with `src/SUMMARY.md`, then read the relevant tutorial or concept pages for protocol, transport, replication, inputs, system ordering, shared plugins, client/server setup, prediction, interpolation, and Avian integration.
- `references/avian/crates/avian2d/examples` — official Avian 2D examples project and their `Cargo.toml` feature sets.

The Lightyear 0.29 snapshot targets Bevy 0.19, while the checked-in Bevy source is currently 0.20-dev. Use the Bevy snapshot for architectural examples, but confirm exact APIs against Bevy 0.19 source or official documentation before implementation.

Treat `references/` as read-only upstream material unless the user explicitly requests a snapshot update. Inspect the example README, source, and `Cargo.toml` together because feature flags and application topology are part of the example. Adapt the smallest relevant pattern to Brawler's authority and dependency boundaries; do not copy whole examples blindly.

When research still requires the internet, prefer current primary documentation and record why the local snapshot was insufficient.

## Versioned implementation docs

Implementation work lives under `docs/implementation/<version>/`:

```text
docs/implementation/
  v1/
    roadmap.md
    milestone-01.md
    milestone-02.md
    ...
  v2/
    roadmap.md
    milestone-01.md
    ...
  v3/
    roadmap.md
    milestone-01.md
```

`roadmap.md` defines version scope, ordering, delivery gates, status, and backlog. Each `milestone-NN.md` records the research, user-validated technical specification, implementation checklist, test evidence, playtest handoff, feedback decisions, and closeout learning for one milestone.

Create a milestone file when that milestone becomes next. Do not pre-author distant technical designs that should incorporate earlier evidence.

Allowed roadmap statuses are `Not started`, `Researching`, `Specification review`, `Implementing`, `Verifying`, `User playtest`, `Feedback review`, `Complete`, and `Blocked`.

## Milestone process

For the next non-complete milestone:

1. Update the roadmap and milestone status to `Researching`.
2. Inspect the relevant local Bevy/Lightyear references first, then research current primary sources, alternatives, compatibility, and risks. Record exact local paths and external links in the milestone file.
3. Write the technical specification, ECS ownership and lifecycle, plugin/schedule composition, network behavior, implementation tasks, test plan, visual checks, and exit criteria.
4. Set the status to `Specification review` and deliver the specification to the user. Do not begin production implementation until the user validates it.
5. Set the status to `Implementing` and complete the tracked tasks without silently expanding milestone scope.
6. Set the status to `Verifying` and run unit tests, integration tests, local network tests, and visual/controller checks required by the specification.
7. Set the status to `User playtest` and provide a clear build/run path, controls, scenario, known limitations, and requested observations.
8. Set the status to `Feedback review`. For each feedback item, record whether it is implemented now, deferred to the version backlog, rejected with rationale, or awaiting more evidence.
9. Re-run affected verification after accepted changes.
10. Perform a learn-from-errors review. Record mistakes, causes, prevention, and reusable lessons. Create or improve project/Codex skills when the learning is recurring and genuinely reusable.
11. Mark the milestone `Complete` only after exit criteria, evidence, user feedback triage, and the learning review are complete. Update the roadmap current milestone.

## Implementation and verification rules

- The current milestone file is the implementation scope contract. Update and revalidate it before materially changing scope or architecture.
- Server authority is not optional, including in-process and offline development modes.
- Clients send intent, not positions, hits, damage, scores, status triggers, or terrain edits.
- Separate authored definitions, selected builds, and runtime state.
- Keep gameplay events independent from rendering, audio, camera, and HUD presentation.
- Use focused pure-function tests where a rule is naturally independent of ECS. Test component, resource, lifecycle, and state behavior with small `App`/`World` schedule tests; add headless integration tests for authority and replication.
- Advance Bevy fixed time or explicitly run the relevant schedule in time-dependent tests rather than waiting on wall-clock sleeps.
- Visual verification complements automated tests; it does not replace them.
- Preserve unrelated user changes and keep deferred work visible in the active version backlog.

Canonical build, test, process, closeout, and playtest commands already live in `justfile` and the
root `README.md`; use those rather than inventing substitutes. V3 M01 may add only a bounded
development selector for its 3D feasibility composition; it must not replace the supported 2D path
before acceptance or turn the migration into a permanent renderer setting.
