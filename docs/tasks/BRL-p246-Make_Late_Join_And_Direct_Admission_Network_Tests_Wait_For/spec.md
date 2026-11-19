Found in BRL-thwh. `movement::late_join_receives_current_poses…` and `loadouts::direct_admission_resolves_distinct_weapon_bases…` depend on exact step counts: the client input timeline syncs only at server tick 30 (after three pings), and late-join movement starts on the last of its 24 steps. They failed with harness RTT >= 12 ms (they were hidden flakes under host load before BRL-thwh pinned RTT to 1 ms). Make them wait on the sync condition rather than a fixed step count.

## Implementation (commit d613b898, branch worktree-agent-adad62ce56ff224d6)

Test-only change; no production code touched.

- `tests/network/harness.rs`: `Harness::step_until_within(limit, what, condition)` (bounded, `#[track_caller]`, panics with `"network harness: {what} did not happen within {limit} steps"`) and `Harness::client_input_synced(index)`, which reads Lightyear's client `LocalTimelineSync::is_synced()` (the gate before the client sends input).
- `movement::late_join_receives_current_poses_without_duplicating_static_arena`: waits (≤240 steps) for client 0 input sync, then steps until the server pose has moved more than 1.0 (≤60 steps) instead of 24 fixed steps.
- `loadouts::direct_admission_resolves_distinct_weapon_bases_and_spawns_expected_deliveries`: waits (≤240) for both clients' input sync instead of 60 fixed steps, then steps until ≥6 projectile deliveries (≤30) and asserts exactly 6, instead of 3 fixed steps.

## Verification

- RTT stress: temporarily set `HARNESS_LINK_RTT` to 12 ms, then 30 ms. The two tests passed at both (`cargo test -p brawler-client --features network-test --test network -- late_join_receives_current_poses direct_admission_resolves_distinct_weapon_bases`). Control: the original tests at 12 ms both FAILED (movement.rs:192 movement assert, loadouts.rs:52 delivery count). Restored the 1 ms pin before committing.
- `just lint`: pass.
- `just test`: all suites pass, including network (`--test-threads=1`, 132/132), except the wall-clock performance gates, which failed under host load average ~13-21 from concurrent agent builds. Rerun serially (`cargo test --locked -p brawler-client --features network-test --test performance -- --nocapture --test-threads=1`): 12/12 pass, worst p95 14.3 ms.
- Plain `cargo test -p brawler-client --features network-test --test network` (default parallel threads): 128 passed, 4 failed (hot_zone timeout-leader-tie, matchplay four-client restart, two soaks). All 4 panic with "expected late-input diagnostic capture scopes must not overlap" (src/logging.rs:136). That process-global capture scope requires serial execution, which `just test` provides. This pre-exists and is unrelated to this change; the two target tests pass in that run.
