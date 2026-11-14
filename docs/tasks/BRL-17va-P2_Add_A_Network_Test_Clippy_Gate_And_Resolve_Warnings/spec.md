# Scope

Resolve strict Clippy findings for the network-test test/performance configuration and add it to the canonical lint matrix.

# Acceptance

- Make portability and precision-sensitive conversions explicit and checked.
- Split long tests only when ownership improves; use narrow reasoned allowances otherwise.
- cargo clippy --locked --no-default-features --features network-test --tests -- -D warnings passes.
- just lint and CI enforce the same gate.
- Network integration and performance suites remain behaviorally unchanged and pass.

# Constraints

Do not mechanically refactor scenario tests solely to silence line-count lints.

## Verification (2026-08-27)

- `cargo clippy --locked --no-default-features --features network-test --tests -- -D warnings` passed.
- `cargo test --locked --no-default-features --features network-test --test network -- --test-threads=1` passed: 88/88.
- `cargo test --locked --no-default-features --features network-test --test performance -- --nocapture` passed: 12/12.
- Rejected-session teardown now uses Lightyear server-side `Disconnecting`, restoring the handshake-timeout lifecycle test without duplicate despawn ownership.
