# Outcome

Oil-barrel explosions no longer mutate combatant health, defeat state, collision, active effects, damage/defeat facts and cues, or combat telemetry in map code. Map authority produces deterministic typed combatant targets; combat authority validates and commits the bounded environment-damage transaction shared with effect-tile damage.

# Scope and decisions

1. Characterize and preserve explosion target ordering, shared maximum-target budget with world objects, map occlusion, current-lineage attribution, allied/self damage behavior, fighter/deployable outcome kinds, inherited attack identity, environment source identity, defeat cleanup, facts, cues, and map telemetry.
2. Generalize the existing combat-owned environment damage API to accept typed source/attack/protection policy and fighter-or-deployable targets. Keep typed bounded inputs; do not expose arbitrary World mutation to map handlers.
3. Keep combatant spatial selection and map line-of-sight planning in map authority, but pass the selected ordered entity targets to combat for revalidation, identity reservation, atomic health/lifecycle commit, facts, cues, and combat telemetry.
4. Keep damageable world-object health, terminal reactions, explosion facts, object cues, chain requests, map transitions, and secondary-application telemetry in map authority.
5. Preserve fixed-post ordering: object damage/explosions remain in WorldTargets before mode objectives and damage-tile EnvironmentReactions; no new protocol, schema, balance, renderer, or client-authority path.
6. Reuse the combat-owned transaction for damage-tile pulses without changing their neutral attribution, spawn-protection behavior, cadence, or eligibility.

# Acceptance criteria

- `src/map/runtime/object_authority.rs` contains no direct combatant health, Defeated, collision-layer, active-effect, motion, combat-fact, damage/defeat-cue, or combat-telemetry mutation.
- One combat-owned environment transaction handles both explosion fighter/deployable targets and damage-tile fighter targets with explicit policy differences.
- Explosion ordering, caps, occlusion, attribution/credit, fighter and sentry terminal outcomes, source identity, map facts/cues/telemetry, and existing player-visible damage remain stable.
- Identity exhaustion cannot leave a partially committed selected combatant batch.
- Focused combat and map tests cover damage, protected/no-op behavior, fighter defeat cleanup, deployable destruction, lineage credit, stale lineage, deterministic ordering, target cap sharing, occlusion, and identity exhaustion.
- Existing map, combat, match-scoring, network, and performance tests remain green.
- `cargo fmt --all`, `git diff --check`, role checks, `just check`, and `just lint` pass.

# Verification

- Focused `combat::environment` and `map::runtime` tests.
- Existing map object/explosion and effect-tile tests.
- Relevant network map, Wipeout scoring, lifecycle, and sentry scenarios.
- Role-specific client/server checks, `just check`, `just lint`, and canonical `just test` proportional to the authority change.

## Source characterization and final decisions

The pre-change barrel path damaged fighters and sentries regardless of team, self, or spawn protection; valid current lineage populated initiating player/fighter, while only a hostile non-self team supplied score credit. It reused the initiating attack ID, ordered combatants by distance/network ID after object-first target budgeting and LOS, emitted legacy Damage/Defeat cues and environment outcome facts, and cleared defeat state directly without combat telemetry. Effect tiles already used a narrower combat-owned helper, allocated a neutral attack, respected spawn protection, and updated telemetry.

BRL-0078 preserves the accepted barrel eligibility, including spawn-protection bypass, friendly/self damage, inherited attack identity, and object-first cap semantics. The unified combat transaction intentionally repairs two diagnostic/atomic defects: barrel damage now records the combat telemetry matching its existing facts/cues, and the complete selected event range is reserved before any health mutation. Damage-tile neutral attack allocation now rolls back on event exhaustion.

Audit also found `EnvironmentExplosionProfile.maximum_chain_reactions` was validated, fingerprinted, and Balance-Lab-visible but inert. It now caps total secondary world-object damage applications across one root explosion transaction; `maximum_targets` remains the per-blast object-first shared target budget, and the global secondary ceiling remains code-owned safety. The built-in authored value remains 16, so accepted Feature Yard behavior is unchanged.

## Implementation and verification evidence

Implemented one combat-owned bounded environment-damage batch in `src/combat/environment.rs`. It snapshots fighter/deployable targets, validates current attribution, applies explicit spawn-protection policy, pre-reserves the complete event range, commits health/lifecycle atomically, and projects facts, compatibility cues, retained evidence, combat logs, and telemetry. Barrel explosions reuse initiating attack identity and ignore spawn protection exactly as before; damage tiles allocate a neutral attack and respect protection. Map runtime retains object-first selection, LOS, authored target/chain limits, world-object reactions, and map telemetry, but delegates all selected combatant mutation to combat.

The authored `maximum_chain_reactions` now bounds total secondary world-object damage applications across one root explosion transaction. The built-in value remains 16. Durable documentation records the authority and policy boundary.

Verification passed:
- `cargo test --no-default-features --features server --lib combat::environment::tests -- --nocapture` - 6 passed.
- `cargo test --no-default-features --features server --lib map::runtime::tests -- --nocapture` - 9 passed.
- New exact network barrel transaction/replication test - 1 passed.
- Existing barrel convergence and Heist objective-isolation scenarios - passed.
- `just check` - passed.
- `just lint` - passed, including all feature graphs and repository audits.
- `just test` - passed: routing, client (500), server (466), Balance Lab (488), cross-feature catalog, all 96 network scenarios, and all 12 performance gates.
- `git diff --check` - passed.

## Feedback, limitations, and learning review

No native playtest is required because damage values, eligibility, ordering, cues, presentation assets, and built-in chain behavior are unchanged; the change is an authority/atomicity refactor with replicated integration evidence.

The audit exposed two defects that source-local testing could miss: the Balance-Lab-visible chain limit was inert, and per-target event reservation could partially mutate an explosion batch. Prevention: treat every authored balance field as requiring one runtime-consumption assertion, and require complete identity reservation before multi-target authoritative mutation. The first Heist regression command used a guessed test name and selected zero tests; listing the source-owned test name before rerunning produced the intended exact passing scenario. Future exact-filter evidence should confirm the runner reports a nonzero test count.
