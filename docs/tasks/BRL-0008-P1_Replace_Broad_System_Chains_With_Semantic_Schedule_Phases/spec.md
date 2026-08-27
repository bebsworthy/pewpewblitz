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
