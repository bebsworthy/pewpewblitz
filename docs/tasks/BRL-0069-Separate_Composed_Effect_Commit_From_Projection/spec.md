# Outcome

`apply_composed_records` remains the deterministic coordinator for one reserved, sorted payload batch, but damage, healing, defeat, and runtime-effect work have explicit planning, authoritative commit, and committed-outcome projection boundaries. Facts, cues, telemetry, and evidence are derived from committed results rather than being interleaved with rule calculation.

# Scope

- Introduce the smallest private typed plans/results needed for:
  - damage calculation, Close Quarters modification, event cardinality, health mutation, and optional defeat;
  - healing calculation, healing-block policy, bounded health mutation, and projection;
  - target capabilities used by Tenacity and elemental/runtime effects;
  - defeat commit and projection.
- Shorten `apply_composed_records` into recognizable ordered record phases: owner/target admission, target gate, contact accounting, damage-first effect order, surviving-target runtime effects, and accumulated batch commit.
- Rename or split existing recording helpers so projection consumes committed damage/healing/defeat facts and cannot become a second mutation path.
- Preserve the existing `collect -> reserve -> resolve deliveries -> apply records -> commit accumulated state` system transaction and its single schedule position.
- Preserve the existing runtime-effect helper unless a small extraction is required to consume a typed committed target state; do not redesign condition runtime or split this transaction into additional Bevy systems.

# Decisions and constraints

- This is an organization-only server-authority refactor: no protocol, content schema, balance, schedule, physics, cue, or presentation changes.
- The sorted record order, damage-before-non-damage order, connection/retained-delivery behavior, protected-contact handling, event reservation consumption, health/defeat precedence, passive modifiers, effect stacking, and tracker completion order are locked behavior.
- Health and terminal entity state have one authoritative commit path. Projection helpers may write facts, cues, telemetry, and evidence only after the corresponding result is committed.
- Keep deferred `Commands` behavior and `ApplyDeferred` boundaries unchanged.
- Use private concrete Rust types and pure helpers; do not introduce a general effect service, trait-object dispatcher, command bus, or additional public API.
- Preserve unrelated workspace changes.

# Acceptance criteria

1. Damage and healing arithmetic is represented by focused pure plans with direct boundary tests, and authoritative mutation is separate from cue/fact/telemetry projection.
2. Defeat is committed exactly once with damage projection before defeat projection, preserving legacy and current event-ID order.
3. `apply_composed_records` no longer contains complete inline implementations for damage, healing, defeat, and projection; its deterministic coordination and atomic batch role remain obvious.
4. Runtime knockback, slow, cold, damage-over-time, healing-block, Tenacity, Close Quarters, owner/allied/hostile recipient rules, protected contact, sentry restrictions, and disconnect behavior remain exact.
5. Existing separate-App/network scenarios preserve damage, healing, condition, knockback, defeat, reciprocal lethal, splash, sticky, melee, cone, and cue convergence behavior.
6. Formatting, role checks, strict lint, canonical tests, and performance gates pass.
7. Evidence and a learn-from-errors note are recorded before closeout.

# Verification

- New pure plan tests for zero/partial/lethal damage, Close Quarters modification, zero/capped healing, and blocked healing.
- Focused composed-effects server tests for target gate, recipient policy, resistance/Tenacity, stacking, and event reservation parity.
- Routed scenarios covering primary damage/cues, reciprocal lethal ordering, launcher slow, sticky, persistent splash damage/healing, melee, cone spray, spawn protection, and impaired cue convergence.
- `cargo fmt --all -- --check`
- `git diff --check`
- `just check`
- `just lint`
- `just test`
- No native playtest is required unless behavior changes; this refactor must preserve gameplay exactly.

## Implementation design clarification

The transaction boundary is batch-wide, not merely per effect. Planning must simulate sequential same-target health/effects/motion/defeat state in sorted record order and produce ordered typed projection operations without mutating ECS, trackers, telemetry, cues, or facts. Commit then applies target and tracker mutations in one non-fallible pass; projection replays committed results afterward. `runtime::apply_runtime_effects` must therefore become a planning helper returning updated state plus projection operations instead of writing telemetry during planning. Deferred runtime-effect cues must remain after immediate record projections and must still be suppressed when that target is defeated later in the same batch. This clarification does not change schedules, event reservation, delivery resolution, or deferred-command boundaries.

## Progress — pure effect plans established

- Added typed pure `DamageApplicationPlan` and `HealingApplicationPlan` calculations and migrated the current authoritative loop to consume their requested/applied/resulting-health/lethal facts.
- Added direct boundary coverage for zero, partial, lethal, already-defeated sequential damage and partial/capped/zero-applied healing.
- Focused composed-effect tests pass (9), strict server all-target Clippy passes, formatting/diff hygiene pass, and `just check` passes across client, server, network-test, Balance Lab, routing, and web tooling.
- This is an intermediate checkpoint, not ticket completion. The remaining implementation must construct the batch-wide sequential `ComposedApplicationPlan`, make commit the sole ECS/tracker mutation pass, convert runtime effects into state-plus-projection planning, and replay immediate/deferred projections in the existing order with final-defeat cue suppression.

## Progress — runtime effects now return projection payloads

- Replaced the telemetry/cue-mutating `apply_runtime_effects` helper with `plan_runtime_effects`, which returns `RuntimeEffectPlan`: resulting `ActiveEffects`, optional `ExternalMotion`, weapon telemetry projections, passive telemetry projections, and deferred effect cues.
- The existing record coordinator now consumes the plan and publishes its projections afterward. This preserves current ordering while establishing the mutation-free runtime calculation seam required by the batch-wide planner.
- Focused composed-effect tests pass (13), including resistance, Cold, damage-over-time, plan boundaries, recipient policy, and healing-block behavior.
- Routed launcher slow replication, persistent splash, sticky, and cone-spray scenarios pass.
- `just check`, `just lint`, formatting, strict role isolation, and diff hygiene pass.
- Remaining: move target/tracker mutations and every immediate projection into the batch-wide `ComposedApplicationPlan`, then implement sole commit and ordered projection passes plus atomicity/order characterization.

## Progress — authoritative commits separated from projection

- Damage publication is now explicitly named `project_committed_damage` and runs only after the planned health result has been committed.
- Healing calculation/health commit now delegates all facts, cues, and telemetry to `project_committed_healing`.
- Defeat terminal mutation is isolated in `commit_target_defeat`; `project_committed_defeat` owns defeat telemetry, current/legacy cues and logs, and outcome facts. Damage projection remains before defeat projection and legacy event order is unchanged.
- Focused composed-effect tests pass (13). Routed reciprocal lethal attribution, launcher slow replication, persistent splash, and impaired full-cue convergence pass.
- `just check`, `just lint`, formatting, strict role isolation, and diff hygiene pass.
- Remaining: replace the still-immediate coordinator with a batch-wide sequential plan, make batch commit the only target/tracker mutation pass, and replay all immediate/deferred projections from that committed plan.
