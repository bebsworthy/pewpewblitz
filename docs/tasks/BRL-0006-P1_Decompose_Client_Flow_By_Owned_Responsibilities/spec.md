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
