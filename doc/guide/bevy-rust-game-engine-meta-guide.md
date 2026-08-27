# Bevy, Rust, and Game-Engine Engineering Meta Guide

Status: working engineering guide  
Last researched: 2026-08-27  
Repository stack: Rust 1.95, Bevy 0.19.1, Lightyear 0.29.0, Avian 2D 0.7.0

## Purpose and boundary

This is a decision guide for maintaining Brawler as it grows. It intentionally does not repeat the
basic ECS, query, plugin, state, event, or asset-management material already covered by the local
`bevy-game-engine` and `bevy-ecs-patterns` skills. Use those skills for API patterns and focused
implementation help. Use this guide to decide:

- which source of truth applies to the pinned stack;
- where state and behavior should live;
- how to expose scheduling and authority invariants;
- when an abstraction, optimization, or engine feature has earned its cost;
- how to review, test, profile, and upgrade the resulting system.

The governing idea is simple: optimize for an explicit gameplay model and observable contracts,
not for a theoretically perfect engine architecture.

## 1. Source hierarchy for a fast-moving stack

Bevy and its ecosystem evolve quickly. “Current” advice can be wrong for the repository's pinned
versions even when it is correct upstream. Resolve a question in this order:

1. **Brawler's accepted product and architecture contracts.** Server authority, role isolation,
   fixed-tick behavior, protocol compatibility, and milestone scope override generic advice.
2. **Pinned dependency source and checked-in examples.** Verify exact APIs and schedules in
   `Cargo.lock`, the local dependency source, and the matching Lightyear/Avian examples.
3. **Pinned-version official documentation.** Use docs.rs pages that show Bevy 0.19.x,
   Lightyear 0.29.x, or Avian 0.7.x in their header.
4. **Official migration guides and release notes.** These explain deliberate semantic changes and
   quiet migration hazards. Bevy 0.19, for example, changed resource internals, feature
   collections, rendering schedules, and several lifecycle APIs.
5. **Current upstream source, issues, and discussions.** Use these for awareness and risk research,
   not as evidence that an unreleased API exists locally.

Record both the version and the evidence path in a milestone specification. A web result without a
version header is a lead, not an implementation reference.

## 2. Architectural invariants before module structure

Decide ownership before deciding filenames. Every runtime fact should have one answer to each row:

| Question | Brawler default |
| --- | --- |
| Who may decide it? | The dedicated server for gameplay outcomes; the client for local intent and presentation preferences. |
| What is its durable identity? | A stable domain ID or definition ID, never a process-local `Entity` on the wire. |
| Where is authored data? | Validated catalogs or map/profile content, separate from mutable ECS state. |
| Where is selected data? | An immutable resolved snapshot/loadout created at an authority boundary. |
| Where is mutable runtime state? | Components/resources owned by the plugin that advances its lifecycle. |
| How does another concern learn about it? | A narrow component query, bounded message/cue, or deliberate public helper. |
| What schedule phase owns mutation? | One named semantic phase with visible causal ordering. |
| What happens in headless builds? | Gameplay remains complete; rendering, UI, audio, device input, and client assets are absent. |

Module boundaries should follow different owners, execution roles, lifecycles, schedule phases, or
independently testable rules. Line count is evidence to inspect ownership, not an automatic split
trigger. A large cohesive catalog can remain together; a smaller file mixing connection state,
screen rendering, persistence, and navigation should not.

### A practical extraction test

Extract a helper, module, plugin, or public API only when at least one is true:

- two real callers need the same rule;
- a responsibility has a different state owner or execution role;
- a long orchestration system contains independently testable decision stages;
- feature isolation or platform compilation requires the boundary;
- the current boundary creates repeated defects or makes verification materially harder.

Keep customization in data and local composition. Avoid replacing repeated screen-specific logic
with a generic widget framework when two pure geometry/style helpers remove the duplication while
leaving each screen free to choose its own layout.

## 3. Schedule design is part of the architecture

Bevy schedules are a dependency graph. Treat their edges as contracts, not incidental plumbing.
The default `ScheduleBuildSettings` ignores system ambiguities, and automatically inserts deferred
command flushes when ordering dependencies require them. That makes an apparently working schedule
an insufficient proof that ordering is intentional.

### Prefer semantic phases over broad chains

Use named sets for phases such as collect, validate, decide, commit, observe, replicate, and
present. Then:

- order sets where the phase relationship is real;
- order individual systems only for a direct data or deferred-command dependency;
- use a short `.chain()` for an atomic transaction whose every adjacent edge is required;
- place `ApplyDeferred` explicitly when newly spawned/removed entities must be visible to the next
  phase and that boundary is important to understand;
- leave unrelated readers/writers unordered so Bevy can schedule them in parallel.

A long chain is not clearer merely because it is deterministic. It can hide which edges matter,
serialize independent work, and make a later system silently depend on an accidental predecessor.

### Make ambiguity review executable

For owned schedules in development and schedule-composition tests:

1. enable ambiguity detection at `Warn` or `Error`;
2. enable set reporting so conflicts name their semantic owners;
3. explicitly ignore only reviewed conflicts that are provably harmless;
4. assert the schedule initializes successfully after all plugins are installed.

Do not blindly enable errors across every third-party schedule. Establish the check at Brawler-owned
composition boundaries and document any library conflict that must be allowed.

### Fail tests on ECS system errors

Bevy can route a system-parameter validation failure to a fallback handler and continue the app.
That is useful for a product that needs controlled shutdown, but dangerous in tests: a test can pass
after the system it intended to exercise was skipped. Test app builders should install a panic/fail
handler for unexpected ECS system errors. Negative tests may opt into a capturing handler and must
assert the exact expected error.

This rule also applies to startup/plugin readiness failures. “The final component exists” is weak
evidence if the test log says several intervening systems never ran.

## 4. ECS data and access decisions

### Components, resources, messages, and local state

- Use a component when the data belongs to zero or more entities and follows entity lifecycle.
- Use a resource for unique app/world policy or aggregate state. In Bevy 0.19, resources implement
  component machinery internally, but unique ownership remains the meaningful semantic contract.
- Use messages/cues for bounded facts that may have multiple observers and do not define durable
  truth.
- Use `Local<T>` for truly system-private cache/state. Promote it when another system must observe
  it or its lifecycle needs direct tests.
- Use observers for lifecycle-local reactions, not as an invisible replacement for a multi-phase
  authority transaction.

### Change detection and reconciliation

Reconciliation systems should distinguish three classes of work:

1. **Topology work:** spawn/despawn or rebuild only when generation, catalog, readiness, or durable
   membership changes.
2. **State work:** update material/text/visibility when the relevant component or resource changes.
3. **Per-frame work:** animation, camera projection, and interpolation that genuinely depend on
   current frame time or transforms.

Do not make class 1 and class 2 pay class 3's frequency by default. Prefer `Added`, `Changed`,
`RemovedComponents`, resource change checks, events, or a small cached revision. When per-frame
projection is necessary, cache durable joins between domain owners and presentation entities so the
frame loop does not repeatedly rebuild maps from stable identities.

Bevy change detection is a work gate, not a performance guarantee. Measure query iteration,
allocations, asset mutation, command volume, and render extraction separately.

### Query and storage choices

- Start with normal table storage and straightforward queries.
- Separate frequently iterated hot data from rarely used bulky state only after measurement or a
  clear ownership split supports it.
- Use sparse-set storage for observed insertion/removal-heavy patterns, not as a generic marker
  optimization.
- Avoid repeated nested query scans when stable IDs can be indexed once per revision or entities
  can hold an explicit presentation-owner relation.
- Keep deterministic ordering at authority and evidence boundaries; do not pay for `BTreeMap` in a
  per-frame presentation loop when ordering is irrelevant.

## 5. Networking and physics integration

### Lightyear ownership

Lightyear 0.29 is a modular Bevy networking stack that provides replication, input, prediction,
interpolation, and Avian integration. Its plugin schedules do not replace Brawler's authority model.

- Clients send intent. The server resolves movement, contact, damage, status, score, and map edits.
- Register shared protocol types centrally and use one compatibility handshake/fingerprint.
- Replicate durable state for recovery; emit bounded cues for transient presentation.
- Treat observer-specific visibility as an authority/privacy decision, especially for concealment.
- Keep prediction optional and measured. A predicted presentation must converge to server truth and
  cannot become a second outcome path.
- Preserve Lightyear's receive/input/simulation/send phase expectations. Verify exact system sets
  against the pinned local book before adding ordering edges.

Useful pinned local references:

- `references/lightyear/book/src/SUMMARY.md`
- `references/lightyear/book/src/concepts/advanced_replication/system_order.md`
- `references/lightyear/book/src/concepts/advanced_replication/avian.md`
- `references/lightyear/examples/simple_setup/`
- `references/lightyear/examples/simple_box/`
- `references/lightyear/examples/avian_2d/`

### Avian pose ownership

Avian 0.7 runs its physics schedule in `FixedPostUpdate` by default and exposes high-level
`PhysicsSystems` phases. Brawler's authoritative planar pose should have one writer model:

- Avian `Position`/`Rotation` own the simulation plane;
- disable transform synchronization or interpolation paths that would compete with networked pose;
- run collision/contact work in explicit relation to Avian prepare, step, and writeback phases;
- convert to 3D `Transform` only at the presentation boundary;
- advance fixed schedules explicitly in tests—never sleep for physics.

Collision layers are gameplay policy. Define them from interaction rules (fighter, projectile,
cover, player-only blocker, sensor) and test representative permitted/forbidden contacts. Avoid
encoding a large collision matrix in scattered query filters.

## 6. Rust engineering rules for engine code

### Model invalid states out where it pays

Use newtypes, validated constructors, nonzero IDs, bounded collections, and enums at external or
authority boundaries. Inside a short local algorithm, plain values plus assertions may be clearer.
The goal is to prevent invalid durable/wire state, not to wrap every scalar.

Prefer integer/fixed-point arithmetic for networked rules that need exact cross-machine outcomes.
Keep floating-point presentation math away from authoritative fingerprints and comparisons unless
the exact representation and rounding boundary are specified.

### Visibility is an ownership tool

Default to private items, use `pub(crate)` for demonstrated cross-module use, and expose public
items only for another binary role, integration-test boundary, or genuine library consumer. A
public module tree is a compatibility promise even in an unpublished application crate; it makes
refactoring and dead-code discovery harder.

If integration tests need deep helpers, prefer a narrow feature-gated test-support surface to
making every implementation module public.

### Errors and panics

- Return typed errors at parsing, persistence, network, asset, and configuration boundaries.
- Add context once at the boundary that can explain the failed operation.
- Reserve `expect` for proven invariants and state the invariant in its message.
- Production ECS fallback handlers should classify and trigger controlled failure where possible.
- Test handlers should fail immediately on unexpected system errors.

### Lints and feature matrices

Clippy recommends `-D warnings` in CI with the same toolchain used to compile the project. For a
feature-gated application, “Clippy passes” means each supported role and important test target is
covered. Keep one canonical matrix rather than relying on `--all-features`, which may build invalid
role combinations.

A lint allowance is a local design record. Attach it to the smallest item, give an ownership-based
reason, and revisit clusters of `too_many_lines`, `too_many_arguments`, `type_complexity`, or
`wildcard_imports`: the cluster often reveals an oversized owner even when every individual allow
is justified.

## 7. Game-engine performance workflow

Performance work begins with a player-visible budget and a representative scenario:

1. state the frame/fixed-tick budget and target hardware;
2. reproduce with canonical content, maximum supported roster, and relevant impairment;
3. capture subsystem diagnostics, traces, allocation/command counts, and render timing;
4. identify the dominant cost rather than optimizing the most conspicuous loop;
5. change the smallest ownership/data/schedule decision that removes the cost;
6. compare distributions (median/p95/worst), not a single run;
7. retain a regression gate only for a stable and costly risk.

Use release-like profiles for runtime conclusions. Cargo's dev, test, and release profiles differ
in optimization, debug assertions, overflow checks, codegen units, and incremental compilation.
Profile settings are engineering tradeoffs: LTO and fewer codegen units can improve runtime at the
cost of link time, while optimized dependencies can make development builds of physics/render code
representative enough for iteration. Do not change profiles without measuring compile time and the
target scenario.

Common Bevy-specific cost signals:

- broad chains reducing executor parallelism;
- topology reconciliation running every frame;
- repeated `Assets<T>` mutation causing extraction/upload work;
- nested query scans or transient maps built from unchanged data;
- command bursts and hierarchy churn;
- UI tree rebuilds instead of updating retained nodes;
- logging at error level inside expected soak behavior;
- asset scene traversal or fitting repeated after readiness converges.

## 8. Delivery and content workflow

Build a vertical slice through authority, replication/recovery, client presentation, automation,
and player verification before generalizing. Every content primitive should make its degradation
contract explicit:

- authored gameplay footprint and collider;
- visual profile and provenance;
- imported-scene readiness behavior;
- deterministic primitive fallback;
- terminal/destroyed state;
- headless exclusion;
- accessibility/readability constraints.

Presentation can observe gameplay facts but cannot own navigation, saving, network lifecycle, or
outcomes. Asset loading and animation may delay polish, never authority.

## 9. Review checklists

### Feature specification

- Who owns every mutable fact?
- What is authored, selected/resolved, runtime, wire, evidence, or presentation data?
- Which stable IDs cross processes?
- What is the exact schedule phase and deferred-command boundary?
- What happens on disconnect, restart, late join, generation replacement, and asset degradation?
- What is bounded, and what happens at the bound?
- Which role features compile the code?
- What player-visible slice proves the work?

### ECS/system review

- Does the system coordinate one recognizable phase?
- Are validation, ordering, mutation, telemetry, and cues separable pure stages?
- Is every `.before`, `.after`, or `.chain()` edge causally required?
- Could change detection or a revision gate avoid most executions?
- Are nested queries or temporary maps rebuilding a stable join?
- Will deferred commands be visible when the next phase expects them?
- Does an error skip the system, terminate the process, or fail the test?

### Network/authority review

- Can a client request an outcome rather than an intent?
- Can a local presentation component leak into authority?
- Does recovery reconstruct all durable state without replaying transient effects?
- Is observer-specific visibility enforced server-side?
- Does protocol evolution use the global compatibility contract?
- Are message, collection, rate, and identity bounds validated before mutation?

### Performance review

- Is the scenario representative and the profile appropriate?
- What measured subsystem dominates?
- Are per-frame, per-tick, and per-transition work separated?
- Are logs and diagnostics themselves distorting the run?
- Does the proposed cache have a clear invalidation owner?
- Is the regression test stable enough to keep?

## 10. Upgrade playbook

For a Bevy, Lightyear, or Avian upgrade:

1. Create a dedicated milestone; do not mix it with gameplay scope.
2. Read every official migration guide between pinned and target versions.
3. Diff dependency feature sets, especially headless/client/render collections.
4. Revalidate Lightyear/Bevy/Avian version compatibility in their manifests.
5. Search local code for renamed schedules, sets, lifecycle hooks, resources, and feature flags.
6. Compile routing, client, server, balance-lab, and network-test roles independently.
7. Turn schedule ambiguity and ECS error handling into failures during migration verification.
8. Run focused authority/replication/physics tests before the entire suite.
9. Run canonical routed E2E, recovery, capacity, and native presentation checks.
10. Compare performance evidence and inspect degraded assets/UI, not only compilation.
11. Remove migration shims when the new compatibility floor is accepted.

## Primary research index

Pinned/local material:

- `references/bevy/examples/README.md`
- `references/lightyear/examples/README.md`
- `references/lightyear/book/src/SUMMARY.md`
- `references/lightyear/book/src/concepts/advanced_replication/system_order.md`
- `references/lightyear/book/src/concepts/advanced_replication/avian.md`
- `references/avian/Cargo.toml`
- `references/avian/crates/avian2d/`

Official sources:

- [Bevy 0.19 release notes](https://bevy.org/news/bevy-0-19/)
- [Bevy 0.18 to 0.19 migration guide](https://bevy.org/learn/migration-guides/0-18-to-0-19/)
- [Bevy `ScheduleBuildSettings` 0.19](https://docs.rs/bevy_ecs/latest/bevy_ecs/schedule/struct.ScheduleBuildSettings.html)
- [Bevy `Schedule` 0.19](https://docs.rs/bevy_ecs/latest/bevy_ecs/schedule/struct.Schedule.html)
- [Bevy query documentation](https://docs.rs/bevy_ecs/latest/bevy_ecs/system/struct.Query.html)
- [Lightyear 0.29 crate documentation](https://docs.rs/lightyear/latest/lightyear/)
- [Lightyear 0.29 package and feature documentation](https://docs.rs/crate/lightyear/latest)
- [Avian 2D 0.7 schedule documentation](https://docs.rs/avian2d/latest/avian2d/schedule/struct.PhysicsSchedulePlugin.html)
- [Avian 2D 0.7 crate documentation](https://docs.rs/avian2d/latest/avian2d/)
- [Rust Book: modularity and error handling](https://doc.rust-lang.org/book/ch12-03-improving-error-handling-and-modularity.html)
- [Cargo profiles](https://doc.rust-lang.org/cargo/reference/profiles.html)
- [Clippy continuous integration](https://doc.rust-lang.org/clippy/continuous_integration/index.html)

