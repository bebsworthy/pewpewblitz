# Scope

Review the long Update chains in WorldPresentationPlugin, client session composition, and ServerNetworkPlugin. Replace them with semantic phases and strict subchains only around real transactions.

# Acceptance

- Every retained order edge has a documented data or deferred-command dependency.
- Presentation separates readiness/materialization, reconciliation, cue consumption, animation/state, and cleanup where independent.
- Session/server separates receive/validate, commit, observe/diagnose, enforce, and terminal work.
- Required ApplyDeferred and fixed-tick/replication ordering remain explicit.
- Schedule tests freeze critical order and all role/network tests pass.
- Capture executor/frame measurements before performance claims.

# Constraints

Do not reorder authority outcomes for theoretical parallelism. Address SCHED-02 before or alongside this work.

## Implementation progress (2026-08-27)

- Replaced WorldPresentationPlugin’s single 18-system Update chain with six named semantic phases: asset preparation, topology materialization, state reconciliation, cue consumption, animation, and cleanup.
- Retained a strict two-system asset-readiness subchain; systems within the other phases are no longer serialized merely by tuple position.
- Preserved the CombatClientSet::Sync boundary and the existing map Materialize3d ownership marker.

Client session/server lifecycle phase decomposition, ambiguity verification, schedule tests, and measurements remain in progress.

## Additional implementation and verification (2026-08-27)

- Client session now uses seven semantic phases; server session authority uses five. Transactional handshake/commit/deferred-command subchains remain strict, while observation systems within their phase are no longer serialized by tuple order.
- Added focused phase-order tests for presentation, client session, and server session.
- Those owned schedule tests enable Bevy ambiguity detection at Error with owning-set reporting via the shared test-app helper; third-party schedules are not globally opted in.
- Combined client/server check and Clippy pass. The three phase-order tests pass.

## Final verification in this pass (2026-08-27)

- Retained strict combat-reconcile/import-upgrade, fighter-state/animation, client AppExit observation, interpolation trace, and server verification subchains where systems share mutable state or require deferred visibility.
- Full combined client/server library suite passes: 581 tests.
- Combined client/server check and Clippy remain clean after the final phase refinement.
