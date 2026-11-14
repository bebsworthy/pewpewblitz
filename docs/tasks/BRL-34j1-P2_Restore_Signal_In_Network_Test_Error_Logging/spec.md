# Scope

Make expected network impairment diagnostics observable without flooding CI or hiding unexpected errors.

# Acceptance

- Shared multi-App harness setup installs the global logger/subscriber once.
- Impairment/soak cases scope filtering or capture to the exact expected late-input diagnostic.
- Expected events remain counted and asserted.
- Unexpected ERROR diagnostics remain visible and fail where appropriate.
- The network suite passes with materially smaller, useful output.

# Constraints

Do not globally suppress ERROR or weaken impairment assertions.

## Verification (2026-08-28)

- The network harness installs one process-global logger before constructing multiple Bevy Apps; client, authoritative server, and lobby-worker builders share the same one-time owner.
- Seven restart/disconnect/soak scenarios capture only `lightyear_debug::input` events with `kind=server_late_input_mismatch`, count them, and assert the expected path was exercised.
- `cargo test --locked --no-default-features --features network-test --test network -- --test-threads=1` passed: 88/88, without logger-installation errors or late-input floods.
- Strict client, server, and network-test Clippy gates passed with warnings denied.
