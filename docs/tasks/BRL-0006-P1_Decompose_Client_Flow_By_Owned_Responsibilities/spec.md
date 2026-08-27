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


# Implementation progress (2026-08-27)

Behavior-neutral first pass completed:

- `flow/model.rs` owns the stable public state and error contracts while `flow.rs` preserves their existing re-export paths.
- `flow/actions.rs` owns bounded session observations, UI intents, pending arbitration input, and deferred commit output.
- `flow/connection.rs` owns validated targets, DNS resolution, attempt/candidate deadlines, candidate spawning, and connection presentation.
- `flow/persistence.rs` owns connection-state loading, safe-default recovery, and the cross-client persistence resource.
- `flow/reducer.rs` owns deferred commit and teardown mutation.
- `flow/screens/results.rs` and `flow/screens/server_select.rs` are the first focused screen owners.
- `ClientFlowPlugin` remains the single composition point and its schedule/deferred-command ordering is unchanged.

Verification so far:

- `cargo fmt --all -- --check`
- `cargo check --locked --no-default-features --features client`
- `cargo clippy --locked --no-default-features --features client --lib -- -D warnings`
- `cargo test --locked --no-default-features --features client --lib client::flow::tests` — 41 passed

Remaining before closeout: move the large reducer/input body and enduring dashboard/brawler/game-select screen owners, remove the obsolete Build Editor through BRL-0007 rather than extracting it, then run the full client suite and native UI evidence.


Additional verification: `cargo test --locked --no-default-features --features client --lib` — 427 passed.

## Additional implementation progress (2026-08-27)

- `flow/input.rs` now owns pure dashboard focus navigation, overlay button eligibility, explicit-vs-ordinary action priority, grapheme-aware caret movement, and bounded editor insertion.
- `flow/reducer.rs` now also owns post-removal favorite focus policy.
- BRL-0007 removed the obsolete Build Editor instead of extracting it.

The main action arbitration coordinator and enduring dashboard/brawler/game-select presentation bodies remain to be moved.

## Game-select ownership update (2026-08-27)

- `flow/screens/game_select.rs` now owns the complete game-type catalog root, map/rules copy, scroll input, and post-layout focus visibility policy.
- Focused game-type flow tests pass and client Clippy is clean.

## Verification update (2026-08-27)

- Full combined client/server library suite passes: 581 tests.
- Client check and Clippy pass after the input-policy and game-select moves.


## Responsibility split completion (2026-08-27)

- `flow/input.rs` now owns the complete input-collection system in addition to its pure focus, editor, and arbitration policies.
- `flow/reducer.rs` now owns the complete action-resolution coordinator and its decision helpers, alongside deferred commit and teardown.
- `flow/screens/dashboard.rs` owns dashboard construction, live facts, responsive layout, scrolling/focus visibility, and the dashboard menu.
- `flow/screens/brawlers.rs` owns saved-brawler list, creation, details, editing, deletion confirmation, weapon equipment, and their local focus/scroll policies.
- `flow/screens/game_select.rs` continues to own the game-type selection screen.
- `flow.rs` is reduced from 7,925 to 3,371 lines and remains the single plugin/schedule composition point plus shared UI and focused tests.

Behavior and ordering remain unchanged: the existing `ClientFlowSet` chain, `ApplyDeferred` boundary, action arbitration slots, and deferred `FlowCommit` mutation are preserved.

Verification after the complete moves:

- `cargo fmt --all`
- `cargo clippy --locked --no-default-features --features client --lib -- -D warnings`
- `cargo test --locked --no-default-features --features client --lib client::flow::tests -- --nocapture` — 39 passed
- `cargo test --locked --no-default-features --features client --lib` — 416 passed

Remaining closeout evidence: relevant native dashboard, game-select, and saved-brawler UI smoke verification.

## Native UI closeout (2026-08-27)

- A fresh isolated 1280x720 client profile rendered the Create Brawler flow at `target/brl-dashboard-native-clean/brawler-000540.png`; visual inspection confirmed the three-column fighter, weapon, and ultimate selection layout, responsive bounds, focus treatment, and Create Brawler action.
- A seeded isolated profile rendered the connected dashboard at `target/brl-dashboard-seeded-screens/brawler-000540.png`; visual inspection confirmed server status, 3D fighter preview, saved-brawler card, mode card, Practice/Play controls, and settings/menu presentation without the removed Build Editor.
- The existing bounded native two-client gameplay render gate also passes on rerun (p95 17.003 ms and 17.008 ms; zero frames above 25 ms).
- Client Clippy, all 416 client library tests, the full routed/network matrix, and the native UI smoke pass. Acceptance is complete.
