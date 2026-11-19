# Found (BRL-p246, 2026-11-19)
Plain `cargo test -p brawler-client --features network-test --test network` (parallel by default, and the AGENTS.md focused form) gives 128 passed / 4 failed: `hot_zone::hot_zone_timeout_leader_tie…`, `matchplay::four_clients_converge…` and two `soaks::` tests panic with "expected late-input diagnostic capture scopes must not overlap" at `src/logging.rs:136`. The capture scope is process-global; `just test` passes because it uses `--test-threads=1`.

# Acceptance
- The parallel command passes (prefer a per-test capture scope over a global lock), or the documented focused command and AGENTS.md state `--test-threads=1` explicitly.
