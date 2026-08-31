# Technical specification

## Outcome and scope

Refactor only the authoritative composed-projectile sweep currently owned by `combat::delivery::sweep_composed_projectiles`. Extract a focused server-only projectile-delivery module whose schedule-facing system coordinates three named phases:

1. build one immutable, bounded batch snapshot from ECS queries;
2. deterministically plan each ordered projectile against that snapshot and Avian read-only spatial queries;
3. commit plans sequentially through the existing authoritative message/resources/commands boundary.

This is an organization and testability phase. It must not change weapon definitions, balance formulas, collision layers, target eligibility, attack identity, wire types, schedule placement, physics ordering, authoritative outcomes, telemetry, or player-visible delivery behavior.

## Current problem

The 300+ line sweep system currently performs disconnected-owner lookup, fighter/object/static-geometry snapshotting, projectile sorting, lob movement and landing, persistent-splash capacity checks/spawn, expiry/cancellation, straight trajectory validation, Avian shape casts, sticky arming, direct fighter/world payload expansion, impact emission, runtime mutation, telemetry, tracker settlement, and despawn. It uses broad mutable queries and command/message/resource dependencies in the same control flow, carries narrow complexity suppressions, and is difficult to characterize independently.

## Architecture

### Module ownership

- Keep `combat::delivery` as the delivery composition/shared-geometry surface.
- Move composed-projectile sweep implementation into a focused private `combat::delivery::projectiles` submodule; do not move melee, cone spray, payload application, client preview, or authored definitions.
- Preserve the existing `sweep_composed_projectiles` crate-visible path through a narrow re-export if schedule composition or tests rely on it.
- Default new implementation types/helpers to private; expose only the schedule-facing system and demonstrated test seams.

### Snapshot phase

- Introduce named snapshot shapes for projectile identity/runtime/position/body/lob state, fighters/sentries, damageable objects/Heist safes, disconnected controllers, static blocking geometry, active splash counts, match identity, and active sticky state where required.
- Build identity/entity lookup collections once per batch; do not repeatedly scan broad ECS queries per projectile.
- Snapshot values must use stable network/attack/delivery/object identities where deterministic ordering is required. Process-local `Entity` may remain internal for ECS mutation and Avian filtering and must never cross the wire.
- Keep snapshot size bounded by existing ECS/content capacities; do not create a second gameplay model or persist snapshots across ticks.

### Planning phase

- Sort projectile snapshots exactly by `(attack_id, delivery_index, is_lob)` as today.
- Represent the finite decisions with focused plan variants: cancel disconnected owner, move lob, land/convert splash, land ordinary lob, expire/cancel invalid straight delivery, advance straight delivery, arm sticky on expiry/impact, and straight impact against fighter/object/geometry.
- Pure trajectory, target/payload, and capacity decisions should be testable without Commands or mutable ECS access. Avian shape casts remain read-only planning inputs.
- Preserve exact collision filter masks/exclusions, candidate predicate, contact fraction, engagement distance, falloff inputs, target ordering, sticky attachment point, splash timing/caps, and map/object liveness rules.
- Planning must not emit messages, mutate runtime state/resources, spawn/despawn entities, or change attack trackers.

### Sequential commit phase

- Commit plans in the existing sorted projectile order.
- The commit adapter alone may mutate `ComposedProjectileRuntime`, sticky state, commands, combat IDs/trackers/telemetry, pending payload/delivery messages, world/objective damage queues, and replication-owned splash entities.
- Preserve deferred-command behavior and current same-tick semantics. Do not introduce a new `ApplyDeferred`, schedule, event bus, resource queue, or parallel commit.
- Preserve the exact settlement distinctions between ordinary termination, sticky arming, unresolved splash settlement, lob continuation, and impact delivery.

## Preserved contracts

- Dedicated-server authority and client intent-only boundaries remain unchanged.
- `CombatDeliverySet`/fixed-tick/Avian ordering and existing `ApplyDeferred` relationship remain exact.
- Disconnected owners cancel delivery before any movement/impact.
- Lob interpolation/landing, ordinary area payloads, persistent splash capacity/timing/replication/match membership, straight expiry/range, invalid-body cancellation, shape-cast filters, closest accepted collision, sticky expiry/impact arming, direct fighter/object damage, impact facts, telemetry, tracker completion, and despawn remain behaviorally identical.
- Multiple same-frame projectiles retain the current sorted outcome/message order and deferred visibility semantics.
- No melee, spray, payload transaction, definition/catalog, protocol, routing, map authority, presentation, audio, or VFX changes.

## Tests and verification

1. Pure lob and straight trajectory plan boundaries, including tick/range/invalid geometry.
2. Snapshot candidate acceptance for connected/disconnected, defeated, allied/hostile, live/dead objects, and blocking geometry.
3. Stable projectile sort order and plan order from scrambled ECS insertion.
4. Plan functions prove no mutation; commit tests prove the exact runtime/message/resource mutation for advance, expire/cancel, lob land, sticky arm, straight fighter hit, straight object/Heist hit, and static geometry hit.
5. Existing sticky expiry/impact/chain behavior and splash cap/settlement tests remain green.
6. Network behavior remains green for first-tick hit, closest target, cover stop, map object point-blank collision, sticky, splash, disconnect-before-sweep, orphan rejection, and posthumous attribution.
7. Focused server tests, `just check`, `just lint`, `just test`, and representative performance gates.
8. Native combat evidence only if the refactor changes presentation-visible timing or if automated parity reveals a correction; otherwise prior accepted projectile presentation is not re-approved by an organization-only phase.

## Implementation plan

1. Characterize the existing sweep branches, ordering, mutations, and schedule boundary with focused tests.
2. Add the private projectile module and immutable snapshot/index types.
3. Extract pure trajectory and per-projectile decision planning.
4. Introduce explicit plan variants and one sequential commit adapter.
5. Replace the monolithic loop while preserving the public schedule-facing system path.
6. Add scrambled-order, branch parity, mutation-boundary, and existing regression coverage.
7. Run focused/canonical verification and independent review; record evidence and learning before closeout.

## Exclusions

Attack emission, melee decomposition, cone spray, payload/effect transaction redesign, physics replacement, query-wide caching across ticks, new jobs/parallelism, prediction/rollback, protocol changes, balance changes, presentation changes, and generalized command/event frameworks are excluded.

## Implementation record — 2026-08-31

The authoritative composed-projectile sweep now has a private server-owned `combat::delivery::projectiles` implementation. Its schedule-facing coordinator:

1. collects owned immutable `ProjectileSweepFact` values plus one frozen target/environment snapshot;
2. sorts facts by the preserved `(attack_id, delivery_index, is_lob)` key and produces a complete owned plan vector;
3. reacquires mutable projectile runtimes and commits those plans sequentially through the existing Commands/messages/resources boundary.

Plans own lob movement/landing outputs, splash activation or capacity rejection, straight movement/impact/termination outputs, and sticky arming. `StickyPlanningLedger` is decision-only local state that preserves per-owner/global caps and existing/same-batch primary chain writes without mutating ECS during planning. Commit variants apply precomputed decisions only.

Area fighter/object selection was mutualized into one deterministic pure policy in `combat::delivery`. The existing immediate writer remains an adapter, while projectile planning supplies frozen candidate facts. Stable ordering, one shared fighter/object target budget, LOS result, falloff, world-object/Heist routing, and selected-target counts remain unchanged.

No schedule, `ApplyDeferred`, protocol, catalog, balance, presentation, melee, spray, or payload-transaction contract changed.

## Acceptance and review evidence

- Added pure trajectory, disconnected-owner precedence, candidate acceptance, scrambled projectile ordering, straight-impact ownership, shared fighter/object budget/order, sticky planning-ledger cap/chain, and production sweep tests.
- Independent branch audit characterized all termination, lob, splash, straight, sticky, tracker, telemetry, object, and Heist branches before correction.
- Independent implementation review initially rejected a shallow plan/commit split because projectile facts were not frozen and splash/sticky decisions remained in commit. The implementation was corrected to a true immutable batch -> complete plan vector -> decision-free sequential commit.
- Final independent review found no P0/P1 issue and confirmed schedule/order/settlement parity. It noted eager read-only LOS evaluation as a possible performance cost; canonical and isolated capacity evidence remained well inside the fixed-tick budget.
- Native evidence was not repeated because this organization-only phase preserves player-visible timing and presentation and automated authority/replication/performance parity is complete.

## Verification evidence

Passed:

- `cargo fmt --all -- --check`
- `git diff --check`
- focused delivery/projectile tests: 15/15
- focused sticky tests: 4/4
- strict server-library Clippy with `-D warnings`
- `just check`
- `just lint`
- `just test`
  - routing library 83/83 plus routing process suites
  - client 543/543
  - server 491/491
  - Balance Lab 513/513
  - combined Balance Lab/network smoke 1/1
  - separate-App network 97/97
  - performance 12/12
- isolated `m05_simultaneous_lob_landings_with_area_candidates_stay_within_fixed_tick_budget`: p95 1.050125 ms
- isolated `one_hundred_headless_fighters_and_two_hundred_projectiles_stay_within_fixed_tick_budget`: p95 1.171416 ms

## Learn-from-errors review

The first extraction moved code but did not fully transfer decision ownership: it planned trajectory while leaving splash, ordinary lob, and sticky policy in commit, and still interleaved projectile planning with mutation. Cause: optimizing initially for branch parity and file movement rather than checking every acceptance statement against the live mutation boundary. Prevention: for future coordinator decompositions, require three reviewable artifacts before considering the split complete—an owned immutable fact batch, a complete owned plan collection, and a commit API that cannot access rule queries or capacity policy. Run all-target Clippy, not only library Clippy, because new test targets exposed float-comparison and collection-shape warnings missed by the narrower gate.
