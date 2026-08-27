# Scope

Split client flow by demonstrated state owner while preserving the ClientFlowAction/state-transition contract, state-scoped roots, and one ClientFlowPlugin composition point.

# Acceptance

- Separate model/actions, reducer/commit, connection, persistence, and focused screen owners.
- Production behavior and public(crate) flow contracts remain unchanged.
- Screen-specific layout, focus policy, and customization stay local.
- Reducer decisions and deferred-command boundaries remain explicit and tested.
- Use reviewable behavior-neutral moves before simplification.
- Client check, Clippy, tests, and relevant native UI evidence pass.

# Constraints

Do not introduce a generic UI framework. Coordinate with DEAD-01 so the obsolete Build Editor is removed rather than granted a permanent module.
