# Scope

Make production-like Bevy App tests fail immediately on unexpected ECS system errors, then repair the practice-bot worker test composition that currently lacks Lightyear RepliconChannelMap and Avian SpatialQueryDiagnostics resources.

# Acceptance

- A shared test-app finalization path installs a fail/panic fallback handler after plugin composition.
- Negative tests use an explicit capturing handler and assert the expected failure.
- The practice-bot worker test runs without Bevy system-parameter validation errors.
- A focused test proves missing required resources fail the test.
- Server and Balance Lab unit suites pass without the unexpected validation logs.

# Constraints

Preserve the production controlled-error policy. Expected negative tests and product processes must not be made to panic.


# Implementation evidence (2026-08-27)

- Added `crate::test_app::finalize`, which follows Bevy's runner lifecycle (`plugins_state` readiness, `finish`, and `cleanup`) before installing a panic fallback handler for unexpected fallible-system errors.
- Added `finalize_with_error_handler` for negative tests and a focused capturing-handler test.
- Added a focused regression test proving that a missing required `Res<T>` panics under the default test path.
- Finalized production-like match-worker apps before their first manual update. This installs Lightyear's `RepliconChannelMap` and Avian's `SpatialQueryDiagnostics`; the practice-bot test no longer emits their system-parameter validation errors.

Verification:

- `cargo fmt --all -- --check`
- `cargo clippy --locked --no-default-features --features server --lib -- -D warnings`
- `cargo test --locked --no-default-features --features server --lib` — 333 passed
- `cargo test --locked --no-default-features --features balance-lab --lib` — 350 passed
- Exact practice-bot regression test passed without missing-resource validation errors.
