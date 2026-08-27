# Brawler Bevy/Rust Extended Code Audit

Date: 2026-08-27  
Scope: current working tree, including in-progress V13 material  
Method: architecture and call-site inspection, duplication/dead-code searches, feature-matrix
build/lint/test baselines, pinned local dependency research, and current primary documentation  
Production changes made by this audit: none

## Executive summary

Brawler has a strong authority and verification foundation. The canonical format, build, role
isolation, lint, unit, integration, and performance commands pass. Server-owned outcomes, stable
wire identities, bounded collections, deterministic ordering, explicit fixed-tick phases, and the
Avian/Lightyear pose boundary are consistently implemented.

The highest-value risks are concentrated rather than systemic:

1. a real ECS test can pass while Lightyear and Avian systems are skipped for missing resources;
2. `client/flow.rs` has become a multi-owner UI/application subsystem in one file;
3. the superseded full-build selection path remains spread across product, protocol, persistence,
   telemetry, and tests despite saved brawlers replacing it;
4. several long `.chain()` tuples serialize systems whose causal edges are not all explicit;
5. presentation reconciliation rebuilds stable joins and rewrites stable state every frame.

No P0 release-blocking defect was found. This report contains **5 P1**, **7 P2**, and **2 P3**
items. Priority represents expected value of remediation, not only bug severity.

## Priority model

| Priority | Meaning |
| --- | --- |
| P0 | Known correctness, security, data-loss, or authority failure requiring immediate action. |
| P1 | High-value reliability, architectural, or measured/obvious runtime risk; schedule soon. |
| P2 | Material maintainability, verification, or bounded performance improvement. |
| P3 | Low-risk cleanup that is useful when touching the owning area. |

## Verification baseline

| Command | Result | Notes |
| --- | --- | --- |
| `cargo fmt --all -- --check` | Pass | No formatting drift. |
| `just check` | Pass | Routing, client, server, network-test check, Balance Lab web tests/build. |
| `just lint` | Pass | All configured role Clippy gates, server feature isolation, renderer/map legacy guards. |
| `just test` | Pass | Routing/process, client, server, Balance Lab, 90 network scenarios, and 12 performance gates. |
| `cargo clippy --locked --no-default-features --features network-test --tests -- -D warnings` | Fail | 29 errors; this feature is tested but is absent from the lint matrix. See TOOL-01. |
| Isolated practice-bot worker test | Pass with unexpected ECS errors | Repeated missing `RepliconChannelMap` and `SpatialQueryDiagnostics`; see TEST-01. |

The full network suite also emits a very large volume of Lightyear `ERROR` diagnostics during
intentional late-input/impairment cases. Those cases pass their behavior assertions; the logging
problem is tracked separately as OBS-01.

## Findings by theme and value

### Verification and correctness confidence

#### TEST-01 — P1 — ECS tests pass after required systems fail parameter validation

**Evidence**

- [`src/server/admission.rs:732`](../src/server/admission.rs#L732) builds full direct and routed
  worker apps for practice-bot schedule tests.
- [`src/server/admission.rs:772`](../src/server/admission.rs#L772) calls `worker.update()`, then
  [`src/server/admission.rs:799`](../src/server/admission.rs#L799) directly runs `FixedUpdate` and
  `FixedPostUpdate` twelve times.
- The exact test at [`src/server/admission.rs:831`](../src/server/admission.rs#L831) returns success
  while Bevy logs that Lightyear's `receive_server_packets`/`send_server_packets` lack
  `RepliconChannelMap`, and Avian's `raycast`/`shapecast` lack `SpatialQueryDiagnostics`.
- [`src/server/mod.rs:323`](../src/server/mod.rs#L323) installs Bevy's logging fallback error
  handler. It reports the validation error and continues, so the affected systems are skipped.

**Impact**

This is a false-positive test-composition risk. The bot assertions currently exercise enough
independent logic to pass, but the test does not prove that its full production-like schedule ran.
The same error pattern appears under both `server` and `balance-lab` feature suites.

**Smallest sound remediation**

Create one test-app finalization helper that replaces the production fallback handler with a
panic/fail handler after plugin composition. Use it for schedule/integration-style `App` tests.
Tests intentionally exercising failure should install a capturing handler and assert the exact
error. Then either install the missing Lightyear/Avian resources through the supported plugin path
or narrow this bot test to the owned schedules/systems it actually intends to prove.

**Verification**

- The isolated test must fail before the missing-resource composition is repaired.
- After repair it must pass with no `bevy_ecs::error::handler` output.
- Add a small test proving the common test app handler panics on a missing required resource.

#### TOOL-01 — P2 — `network-test` is in CI tests but outside the Clippy matrix

**Evidence**

- [`justfile:31`](../justfile#L31) lints routing, client, server, and Balance Lab, but not
  `network-test`.
- [`justfile:65`](../justfile#L65) only checks the network-test feature; [`justfile:110`](../justfile#L110)
  and [`justfile:116`](../justfile#L116) execute its integration/performance tests.
- The manual `-D warnings` command currently fails with 29 errors: two routing-process test lints,
  three performance-test lints, and 24 network-test lints. Most are long-test/style findings, but
  several `u64`→`usize`/float casts are portability or precision assumptions worth making explicit.
- This gap is already recorded as `MAINT-NETWORK-TEST-LINT` in
  [`docs/backlog.md:87`](../docs/backlog.md#L87).

**Impact**

The canonical “lint passes” signal does not cover an important test harness and its performance
evidence. New warnings accumulate until the eventual cleanup becomes larger.

**Recommendation**

Resolve the substantive conversions first, split only tests whose setup/act/assert ownership is
actually unclear, and add a `_clippy-network-test` target to `just lint` and the CI feature matrix.
Use narrow allowances with reasons for deliberately long scenario tests.

#### OBS-01 — P2 — Expected network impairment floods `ERROR`, obscuring unexpected failures

**Evidence**

The successful 90-scenario network run prints thousands of lines from
`lightyear_debug::input` for expected already-simulated input corrections during soak and reconnect
scenarios, along with repeated global logger installation errors in multi-app tests.

**Impact**

The signal-to-noise ratio is poor enough to hide the missing-resource errors in TEST-01, inflate CI
logs, and make “an error appeared” unusable as a simple failure heuristic.

**Recommendation**

Install the logger once in shared harness setup. Give impairment/soak tests a scoped filter or
capturing diagnostics layer that suppresses only the exact expected Lightyear event while retaining
a counter asserted by the scenario. Do not globally suppress `ERROR`.

### Ownership and architecture

#### ARCH-01 — P1 — `client/flow.rs` is a multi-owner application subsystem

**Evidence**

- [`src/client/flow.rs`](../src/client/flow.rs) is 9,894 lines; production code runs through line
  8,193 and tests begin at [`src/client/flow.rs:8194`](../src/client/flow.rs#L8194).
- It owns connection resolution and errors, application flow state/actions, persistence loading,
  server selection, Dashboard, legacy Build Editor, saved-brawler list/details/editor, weapon
  equipment, game selection, results, focus/navigation, scrolling, and shared button construction.
- The central `resolve_flow_action` reducer begins at
  [`src/client/flow.rs:1505`](../src/client/flow.rs#L1505) and spans roughly 1,190 lines.

**Impact**

Unrelated product changes share a very large import/type namespace and reducer. Screen ownership is
hard to review, lint allowances lose diagnostic value, and the file is costly to navigate and merge.
This is responsibility concentration, not a complaint about line count alone.

**Smallest sound remediation**

Retain the existing `ClientFlowAction`, state-transition contract, state-scoped roots, and one
`ClientFlowPlugin` composition point. Split by demonstrated owner:

```text
client/flow/
  mod.rs            plugin, public(crate) surface, semantic sets
  model.rs          states, destinations, actions, shared facts
  reducer.rs        pure transition decisions and commit boundary
  connection.rs     target resolution, deadlines, connection copy/errors
  persistence.rs    orchestration of local flow data
  ui/
    server_select.rs
    dashboard.rs
    brawlers.rs
    equipment.rs
    game_select.rs
    results.rs
    common.rs        only proven scroll/focus/button geometry helpers
```

Do this incrementally with move-only commits and invariant tests. Do not introduce a generic UI
framework or change navigation/custom layout behavior during extraction.

#### DEAD-01 — P1 — The superseded full-build workflow remains a cross-cutting second loadout path

**Evidence**

- The repository already identifies this as `MAINT-LEGACY-BUILD-SYSTEM` at
  [`docs/backlog.md:85`](../docs/backlog.md#L85): saved brawlers are the sole product loadout
  workflow, while the Build Editor, standalone build persistence, direct-session selection path,
  named full-build presets, and preset-only telemetry/configuration remain.
- The dormant path still spans `src/client/build_editor.rs`, `src/client/build_persistence.rs`, the
  large flow reducer/UI, `src/builds/server.rs`, `src/protocol.rs`, client session handling, catalog
  definitions, telemetry, automation, and network fixtures.

**Impact**

There are two concepts named “build”: the active resolved weapon/loadout model needed by gameplay,
and an obsolete product selection/persistence workflow. The latter enlarges protocol and UI
surfaces, keeps unreachable screens alive, and makes content changes account for a non-product path.

**Recommendation**

Promote the existing backlog item to a focused cleanup milestone before adding more content to this
path. Preserve active weapon bases/presets, definitions, immutable resolved loadouts, profile
persistence, routed saved-brawler admission, and server authority. Remove obsolete product/UI/
protocol state in one compatibility-floor change, convert fixtures to saved brawlers or explicit
canonical recipes, advance the global compatibility schema, and fail closed rather than retaining
decoders.

#### API-01 — P2 — The application crate exposes most implementation modules as public API

**Evidence**

- [`src/lib.rs:3`](../src/lib.rs#L3) through [`src/lib.rs:25`](../src/lib.rs#L25) expose nearly every
  top-level gameplay and role module with `pub mod`.
- Broad wildcard re-exports exist in [`src/combat/mod.rs:64`](../src/combat/mod.rs#L64),
  [`src/map/mod.rs:51`](../src/map/mod.rs#L51), and [`src/movement/mod.rs:10`](../src/movement/mod.rs#L10).

**Impact**

Integration tests can reach internals, but the resulting public compatibility surface makes
ownership refactors and dead-code detection harder. An unpublished application crate does not need
to promise every implementation path to its binaries and tests.

**Recommendation**

Inventory actual cross-role and integration-test consumers. Narrow modules/re-exports to
`pub(crate)` by default and expose a small `testing`/harness surface under `network-test` for the
fixtures that truly need external access. Make this opportunistic after legacy-path removal rather
than a mass visibility rewrite.

### Scheduling and runtime cost

#### SCHED-01 — P1 — Broad `.chain()` tuples encode excess serialization

**Evidence**

- [`src/client/presentation_3d/mod.rs:250`](../src/client/presentation_3d/mod.rs#L250) chains 19
  systems: asset preparation, map/pickup/safe reconciliation, material updates, combat
  reconciliation/cues/state, animation, cleanup, and zone tint.
- [`src/client/session.rs:78`](../src/client/session.rs#L78) and
  [`src/client/session.rs:94`](../src/client/session.rs#L94) chain 10 and 13 lifecycle, selection,
  automation, overlay, logging, timeout, and trace systems.
- [`src/server/mod.rs:341`](../src/server/mod.rs#L341) chains 15 endpoint, session, legacy build,
  loading, command, deadline, verification, and exit systems, with two explicit deferred flushes.

**Impact**

Some edges are clearly required, especially session transitions and deferred-command commits, but
the tuples also force unrelated observation, presentation, and cleanup work into a serial path.
They obscure the actual causal graph and can create accidental order dependencies.

**Recommendation**

Introduce a small number of semantic sets and retain strict subchains only around real
transactions. For presentation, separate asset readiness/materialization, durable reconciliation,
cue consumption, animated state, and cleanup. For session/server, separate receive/validate,
commit, observe/diagnose, enforce, and terminal phases. Preserve the explicit `ApplyDeferred`
boundaries that transactions require.

Measure executor/frame behavior before claiming a speedup; the immediate value is clearer and
testable ordering.

#### SCHED-02 — P2 — Owned schedules do not enable ambiguity detection

**Evidence**

- The codebase has roughly 199 `add_systems` sites and 358 set/order/chain clauses.
- No Brawler code configures `ScheduleBuildSettings` or `ambiguity_detection`.
- Bevy 0.19 defaults ambiguity detection to `Ignore`.

**Impact**

Conflicting systems without a deliberate edge can remain unnoticed, while broad chains may be
retained merely to avoid uncertainty. This matters most at cross-plugin fixed-tick, replication,
physics, and presentation boundaries.

**Recommendation**

Add a schedule-composition verification mode for Brawler-owned schedules with ambiguity detection
at `Warn` initially, then `Error` after triage. Report owning sets and explicitly ignore only proven
benign conflicts. Avoid imposing the policy blindly on third-party internal schedules.

#### PERF-01 — P1 — Presentation reconciliation repeatedly rebuilds stable joins and stable state

**Evidence**

- [`src/client/presentation_3d/mod.rs:534`](../src/client/presentation_3d/mod.rs#L534) runs every
  `Update`, rebuilds a terminal-state `BTreeMap`, scans placements with `.iter().find()` for each
  existing visual, then scans all placements again to materialize missing visuals.
- [`src/client/presentation_3d/mod.rs:821`](../src/client/presentation_3d/mod.rs#L821) rebuilds a
  health map and rewrites every oil-barrel material handle each frame, even when health is stable.
- [`src/client/presentation_3d/mod.rs:914`](../src/client/presentation_3d/mod.rs#L914) must project
  health bars each frame, but also rebuilds durable damaged-object and visual joins using two
  `BTreeMap`s and a `BTreeSet`.
- [`src/client/presentation_3d/mod.rs:403`](../src/client/presentation_3d/mod.rs#L403) builds
  `safe_entities` from the same safe query and checks membership while iterating that query; the
  check is always true. [`src/client/presentation_3d/mod.rs:709`](../src/client/presentation_3d/mod.rs#L709)
  does the inverse tautology with pickup `owners`.

**Impact**

Cost scales with placements × visuals and continues after map generation and health state have
stabilized. Current performance gates are authority/fixed-tick oriented and do not measure this
client frame path, so this is a high-confidence code-path risk, not a measured frame regression.

**Smallest sound remediation**

- Gate topology reconciliation on accepted-map identity/generation, terminal-state revision, asset
  readiness, or relevant added/removed state.
- Index placements once per accepted snapshot/revision instead of nested scans.
- Link dynamic visuals directly to their damageable owner or keep a generation-scoped lookup.
- Update barrel material only for changed health/life state.
- Keep per-frame camera projection, but cache durable UI membership/owner joins.
- Delete the two redundant identity sets immediately when touching these systems.

Add a client diagnostic or focused benchmark for a maximum map with stable dynamic objects before
and after the change.

#### PRES-01 — P2 — One combat presentation system owns four distinct lifecycles

**Evidence**

[`src/client/presentation_3d/combat.rs:1026`](../src/client/presentation_3d/combat.rs#L1026)
`update_combat_visual_state` coordinates:

- fighter health/name/ammunition UI state;
- durable slow/knockback/reveal status visuals;
- dash-trail creation/update/removal;
- dynamic aim blockers and weapon/ultimate preview geometry.

It builds multiple temporary maps/sets/vectors each frame and searches the entire trail query for
each fighter (`trails.iter_mut().find(...)`). The local Clippy reason accurately lists the mixed
owners rather than demonstrating one owner.

**Impact**

Changes to HUD state, dash feedback, status markers, and aiming collide in one query signature and
execution phase. Per-fighter nested trail lookup is unnecessary, and change gates cannot be applied
independently.

**Recommendation**

Split into four named systems ordered only where data requires it: collect/cache fighter
presentation facts, reconcile overhead/status state, reconcile dash trails, and update aim preview.
Store/link one dash trail per owner so lookup is direct. Share a small read-only per-frame facts
resource only if repeated fighter scans measure as material; otherwise let focused queries remain
direct.

### Duplication and simplification

#### DUP-01 — P2 — Six scroll systems and six focus-visibility systems repeat the same geometry

**Evidence**

Scroll systems begin at [`src/client/flow.rs:3596`](../src/client/flow.rs#L3596),
[`src/client/flow.rs:4510`](../src/client/flow.rs#L4510),
[`src/client/flow.rs:5085`](../src/client/flow.rs#L5085),
[`src/client/flow.rs:5373`](../src/client/flow.rs#L5373),
[`src/client/flow.rs:6239`](../src/client/flow.rs#L6239), and
[`src/client/flow.rs:7519`](../src/client/flow.rs#L7519). Focus visibility repeats at lines 3619,
4537, 5321, 5397, 6575, and 7544. They differ mainly in markers, gating state, line multiplier, and
layout constants.

**Impact**

Wheel normalization, offset clamping, and “keep focused row inside viewport” fixes must be repeated
across screens and can drift. This duplication contributes directly to ARCH-01.

**Recommendation**

Extract pure helpers for normalized wheel delta, bounded scroll offset, and the minimal offset that
keeps an interval visible. Keep screen systems, markers, layout constants, focus policy, and render
trees independent. Similarly consolidate repeated button-node/color construction into a small
style specification, not a generic widget hierarchy.

#### DUP-02 — P2 — Four cue consumers duplicate bounded-effect eviction and sequencing

**Evidence**

[`src/client/presentation_3d/combat.rs:1530`](../src/client/presentation_3d/combat.rs#L1530),
[`src/client/presentation_3d/combat.rs:1604`](../src/client/presentation_3d/combat.rs#L1604),
[`src/client/presentation_3d/combat.rs:1683`](../src/client/presentation_3d/combat.rs#L1683), and
[`src/client/presentation_3d/combat.rs:1775`](../src/client/presentation_3d/combat.rs#L1775) each:

1. count `CombatEffect3d` entities;
2. find/despawn the oldest at `MAX_EFFECTS`;
3. increment a saturating local sequence;
4. spawn a nearly identical timed effect bundle.

**Impact**

Capacity behavior and ordering can drift across cue families, and every cue repeatedly scans the
bounded effect query. The current chain happens to define cross-family order, but that ownership is
implicit.

**Recommendation**

Centralize capacity reservation/order allocation in one presentation-owned effect budget or a
small helper and use one effect spawn descriptor. Keep each cue consumer responsible for its own
validation, geometry, material, label, and duration so visual customizability is not reduced.

### Dead and stale code

#### DEAD-02 — P3 — `ground_direction` is production-dead and retained only by its own test

**Evidence**

[`src/client/presentation_3d/coordinates.rs:14`](../src/client/presentation_3d/coordinates.rs#L14)
uses `#[cfg_attr(not(test), allow(dead_code))]`. The function is referenced only by tests in the same
file; production uses `ground_position`, `ground_point`, and `ground_rotation`.

**Recommendation**

Remove the helper and its direct basis test. Keep the rotation/round-trip tests, which still prove
the coordinate contract used by production. If a real directional caller appears later, re-add the
one-line helper then.

#### COPY-01 — P3 — Tall-grass diagnostic name contradicts the concealment contract

**Evidence**

[`src/client/presentation_3d/mod.rs:1675`](../src/client/presentation_3d/mod.rs#L1675) names generated
grass “non-concealing tall grass” although tall grass participates in the completed concealment
contract. The in-progress V13 specification independently records this stale label at
[`docs/implementation/v13/milestone-01.md:41`](../docs/implementation/v13/milestone-01.md#L41).

**Recommendation**

Rename the diagnostic entity to neutral “tall grass adjacency …” during the V13 grass presentation
work. No gameplay or protocol change is required.

## Healthy patterns worth preserving

- **Authority separation:** clients consistently send intent; server systems own movement, combat,
  concealment, objectives, map mutation, and outcomes.
- **Role isolation:** the server feature graph passes the canonical guard against rendering,
  windowing, audio, input-device, and client-asset dependencies.
- **Physics/network pose boundary:** Avian transform and interpolation ownership is disabled where
  Lightyear/authoritative `Position` owns planar state; 3D transforms are presentation output.
- **Explicit fixed-tick phases:** gameplay sets and important `ApplyDeferred` boundaries are visible
  at composition points.
- **Stable identities and bounded state:** wire types avoid process-local entities; catalogs,
  telemetry, queues, cues, and evidence generally enforce deterministic bounds.
- **Recovery and topology evidence:** separate-App, UDP, routed-process, impairment, late-join, and
  restart scenarios cover costly networking risks.
- **Performance evidence:** the 12 current fixed-tick gates pass comfortably on the audit machine;
  the combined combat case reported p95 3.024 ms and the 100-fighter/200-projectile case p95 2.138
  ms. These results should not be generalized to unmeasured client rendering paths.

## Reviewed items not classified as defects

- The legacy combat cue adapter in `src/combat/authority.rs` is documented as compatibility and
  evidence fan-out for the original straight-shot path, not a second authority implementation. It
  is a retirement candidate only after its consumers/evidence contract are deliberately removed.
- Duplicate `bitflags` major versions in the dependency graph arrive through platform/library
  dependencies; no actionable dependency-bloat issue was demonstrated.
- The optional owner-prediction feature and legacy direct-UDP diagnostic are intentionally
  trigger-bound/diagnostic surfaces. They are not dead merely because the default product path does
  not activate them.
- Large catalog/test files were not flagged on size alone when their ownership remains cohesive.

## Recommended remediation sequence

1. **TEST-01:** make ECS schedule tests fail on unexpected system errors and repair the bot-worker
   composition. This improves confidence in every later refactor.
2. **SCHED-02 + SCHED-01:** add ambiguity visibility, then replace broad chains with semantic phases
   while preserving causal/deferred edges.
3. **DEAD-01:** remove the already-scoped legacy build workflow before it receives more content.
4. **ARCH-01 + DUP-01:** split `client/flow` by existing owners and extract only the proven pure UI
   helpers.
5. **PERF-01 + PRES-01 + DUP-02:** add a client-frame measurement, then gate/index presentation
   reconciliation and split combat visual lifecycles.
6. **TOOL-01 + OBS-01:** close the feature lint gap and restore useful network-test log signal.
7. Apply API/dead-label/helper cleanup opportunistically after the owning high-value change.

Each production change should be promoted through the active milestone process rather than applied
as an untracked audit cleanup. The audit intentionally stops at evidence and recommendations.
