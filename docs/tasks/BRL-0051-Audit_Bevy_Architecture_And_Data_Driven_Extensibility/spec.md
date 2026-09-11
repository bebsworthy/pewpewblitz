# Scope

- Audit crate/module topology, Bevy plugin composition, schedules, ECS state and communication boundaries.
- Evaluate SRP, Law of Demeter, Open/Closed, and Dependency Inversion as adapted to Bevy ECS.
- Inspect the largest Rust files and heaviest systems for mixed responsibilities, duplication, and hardcoded balance/presentation/AI policy.
- Assess how new weapons, modes, tiles, objects, effects, and AI behaviors can be added today.
- Deliver a prioritized refactoring plan with quick wins, structural changes, and brief before/after Rust/Bevy examples.

# Constraints

- Read-only audit: do not refactor production code as part of this ticket.
- Preserve server authority, execution-role isolation, stable wire identity, persistence, bounded state, and fixed-tick ordering.
- Prefer concrete evidence over generic Rust architecture advice; distinguish verified findings from recommendations.
- Avoid speculative frameworks and abstractions without a demonstrated second use.

# Acceptance criteria

- Findings cite concrete repository files and line numbers.
- The audit covers topology/plugins, ECS principles, SRP/LoD/OCP/DIP, large files/systems, duplication, and hardcoded policy.
- Extensibility is assessed for weapons, modes, map tiles/objects, visual effects, and AI/entity behaviors.
- Recommendations are risk- and value-prioritized and include proportionate verification guidance.
- Ticket records audit evidence, conclusions, and learnings; `ticket sync` succeeds before handoff.


# Audit conclusions

## Strengths

- Client/server feature and plugin composition preserve role isolation and server authority.
- FixedUpdate and FixedPostUpdate ordering, including ApplyDeferred boundaries, is explicit and testable.
- Weapons, builds, maps, and runtime state generally follow authored definition -> validated resolution -> immutable/runtime ECS state.
- Mode implementations use focused plugins over a common lifecycle/outcome handoff.
- Maps and weapons are compositional when using existing engine-supported primitives.

## Priority findings

1. Effect-tile numeric values are authored in RON but production validation accepts only exact code constants, and movement reads the constants rather than the resolved authored behavior.
2. Weapon/build/part catalog validation freezes exact inventory cardinalities, IDs, kinds, and some costs, preventing additive content-only changes.
3. Dash, Sentry, charge, passive, match, and bot balance remains materially code-owned.
4. Authoritative concealment and Self Cloak read the presentation/network CombatOutbox to infer accepted attacks; gameplay must consume a dedicated authoritative fact instead.
5. New modes, ultimates, passives, terminal object behaviors, VFX/audio families, and bot behaviors fan out through central enums and exhaustive matches across unrelated owners.
6. The largest mixed-responsibility systems are resolve_flow_action, sweep_composed_projectiles, worker_control_receive, tick_sentries, emit_attack_deliveries, apply_composed_records, and authoritative_composed_fire.
7. Several composition roots are large but should remain schedule-visible; move implementations behind them rather than hiding ordering in generic dispatch.

## Refactoring sequence

- P0: remove exact inventory locks; move all balance numbers to validated catalogs/resources; make effect-tile runtime consume authored values; replace cue-to-authority reads with authoritative facts.
- P1: decompose high-risk systems into collect/validate/plan/commit/publish stages; split ability behavior plugins; introduce neutral gameplay phases and generic owner-cleanup hooks; centralize stable mode descriptors/mappings.
- P2: make bot policy additive/deterministic, catalog-drive supported VFX/audio profiles, and reduce large mod.rs/UI composition roots while preserving their public schedule boundaries.

# Verification performed

Read-only inspection of Rust sources, RON catalogs, Cargo feature topology, product/network architecture documents, plugin/system registrations, file sizes, complexity suppressions, and Repowise structural/health metadata at live HEAD 6c70090dee1d. No production source was modified and no build/test run was required for this audit-only ticket.

# Learning review

- File size was not used as a defect by itself; composition roots with explicit schedule/deferred boundaries were distinguished from mixed-responsibility systems.
- Serializable enums remain appropriate for bounded wire-compatible engine primitives. Open/Closed improvements should use process-local plugins/registries and data composition without trait objects in replicated schemas.
- The most valuable refactors are those that remove false configuration surfaces and cross-domain authority dependencies before cosmetic module splitting.
