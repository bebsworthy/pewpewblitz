# BRL-0074 specification

## Outcome

Simulation-unit conversion and lobby match-formation timing each have one explicit owner. Production UI and Balance Lab conversions derive from `SIMULATION_TICK_HZ`; operator-authored formation timing is resolved once and supplies both wire-advertised loading duration and server-enforced Practice/product deadlines. Accepted behavior remains exactly 60 Hz, 30 seconds loading, and 10 seconds for route grants.

## Scope

1. Add small checked/shared conversion APIs in `src/timing.rs` for whole seconds to simulation ticks and simulation ticks to whole/display seconds. Derive floating-point editor scaling from `SIMULATION_TICK_HZ` rather than literal `60.0`.
2. Replace simulation-rate literals in Balance Lab editor descriptors and game-selection/dashboard summaries. Do not rewrite unrelated 60-second-per-minute formatting, 60 FPS render policy, world coordinates, test geometry, or non-simulation client timeouts.
3. Add focused boundary tests for exact conversion, truncation/display behavior, and multiplication overflow.
4. Add a typed `MatchFormationTiming` operator policy to `config/server/game-types.ron`; bump only the operator catalog schema required by the additive field.
5. Validate nonzero bounded loading/grant seconds, require the grant deadline not to exceed the overall loading deadline, and convert the advertised loading milliseconds without overflow.
6. Store the resolved timing in `ResolvedLobbyCatalog` and include its authored policy in the private operator policy revision. The public game-type catalog revision remains derived solely from advertised game types.
7. Make Practice and product formation use the same resolved policy for loading/grant deadlines and `ReservationStarted.loading_deadline_millis`.
8. Add catalog and small-App/lobby tests proving exact values, invalid-policy rejection, policy fingerprinting, and advertisement/enforcement convergence.
9. Update durable operator/network documentation where it clarifies ownership.

## Constraints

- Preserve server authority, wall-clock formation semantics, fixed simulation scheduling, protocol shapes, admission precedence, queue ordering, and retry behavior.
- Do not expose operator-only grant timing on the wire or add a protocol/schema version solely for this refactor.
- Keep engine/protocol capacity ceilings in code; author only the current operator policy within bounded limits.
- No player-visible timing change is authorized.
- Preserve unrelated worktree changes.

## Verification

- Focused timing unit tests.
- Focused lobby catalog and formation tests.
- Client and server role checks.
- Relevant client flow/queue tests and routed admission tests if affected.
- `cargo fmt --all`, `git diff --check`, and `just check`.

No native evidence is required because values and presentation output remain unchanged.

## Acceptance criteria

- [x] Production simulation conversions contain no audited literal 60 Hz assumptions outside the canonical timing module.
- [x] Balance Lab and game-selection UI derive conversions from shared timing APIs/constants.
- [x] Operator catalog owns validated 30-second loading and 10-second grant policy.
- [x] Practice/product advertisement and enforcement consume the same resolved timing value.
- [x] Invalid and overflowing timing policy fails closed.
- [x] Public protocol shapes and game-type catalog revision behavior remain unchanged.
- [x] Focused tests and `just check` pass.
- [x] Verification and learning are recorded before closeout.


## Implementation and verification record — 2026-08-30

Implemented canonical simulation conversion helpers in `src/timing.rs`; Balance Lab seconds/per-second descriptors and the Dashboard/game-selection rule summaries now derive from them. Audited remaining `60` values were classified rather than mechanically replaced: minute formatting, render-frame policy, geometry fixtures, and non-simulation client timeouts remain under their existing owners.

Added validated `formation_timing` policy to operator catalog schema 5 with the accepted 30-second loading and 10-second grant values. `ResolvedLobbyCatalog` now resolves one bounded `MatchFormationTiming`; Practice and product formation use it for both absolute server deadlines and the wire-advertised loading milliseconds. Formation policy contributes to the private operator policy revision, while the public game-type catalog revision and protocol shapes remain unchanged.

Verification passed:

- `cargo test --locked --no-default-features --features server --lib timing::tests` — 1 passed.
- `cargo test --locked --no-default-features --features server --lib server::lobby::catalog::tests` — 5 passed.
- `cargo test --locked --no-default-features --features server --lib practice_uses_one_human_and_fills_the_selected_roster_with_named_bots` — 1 passed.
- `cargo test --locked --no-default-features --features client --lib dashboard_mode_card_separates_title_and_pool_without_claiming_a_selected_map` — 1 passed.
- `cargo test --locked --no-default-features --features balance-lab --lib server::balance_lab::editor::tests` — 6 passed.
- `just check` — routing, client, server, network-test, Balance Lab Rust targets, Balance Lab web tests, and web build passed.
- Final `cargo check --locked --no-default-features --features server --lib`, `cargo fmt --all`, and `git diff --check` passed after the shared catalog conversion call.

No native evidence was required because timing values and rendered copy remain unchanged.

## Learn-from-errors review

- The first client compile exposed that Hot Zone target progress is intentionally `u16` while active limits are `u64`. Prevention: keep shared conversion APIs on the authoritative wide type and make bounded wire-to-runtime widening explicit at call sites.
- A literal-value search also finds unrelated 60-second minute formatting, 60 FPS rendering, geometry, and client timeout values. Prevention: classify units and ownership before replacing literals; shared simulation helpers must not capture unrelated time domains.
- The first combined Cargo filter invocation was invalid because `cargo test` accepts one filter. Prevention: run feature-specific filters serially, which also avoids artifact-directory contention in this large Bevy workspace.
