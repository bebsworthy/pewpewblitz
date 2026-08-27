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
