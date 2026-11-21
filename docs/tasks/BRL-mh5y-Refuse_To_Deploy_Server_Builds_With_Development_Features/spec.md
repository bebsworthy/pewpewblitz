# Outcome
A deployed server can never expose Balance Lab, verification match rules, scripted diagnostics or the development key, and CI proves it for every image.

# Found (2026-11-21)
## Cargo features (`Cargo.toml`)
- `server` is the production worker.
- `balance-lab` adds `tiny_http`, `serde_json` and `atomic-write-file` plus the operator HTTP surface (`src/server/balance_lab/`).
- `process-metrics` (`lightyear/metrics`), `network-test` (client + server + crossbeam) and `test-support` are dev-only.
- After BRL-z277, the token service will need `tiny_http` too, so the gate must check **features**, not merely the absence of `tiny_http`.

## Supervisor flags (`packages/brawler-routing/src/bin/supervisor.rs:389-452`)
`--match-rules verification`, `--mode <mode>` (forces one mode), and `--automatic-transition-driver`, which sets `BRAWLER_LOBBY_TRANSITION_DRIVER` and `BRAWLER_LOBBY_TRANSITION_MODE` for the lobby.

## Environment controls read by server roles
`BRAWLER_BOT_TRACE`, `BRAWLER_INPUT_TRACE`, `BRAWLER_NETWORK_ASSERT_MATCH`, `BRAWLER_NETWORK_ASSERT_MOVEMENT`, `BRAWLER_NETWORK_MATCH_REPORT_FILE`, `BRAWLER_NETWORK_MOVEMENT_READY_FILE`, `BRAWLER_NETWORK_PROFILE`, `BRAWLER_SERVER_EXIT_AFTER_TICKS`, `BRAWLER_SERVER_EXIT_AFTER_VERIFICATION(_MIN_TICKS)`, `BRAWLER_SERVER_READY_FILE`, `BRAWLER_QUEUE_EVIDENCE`, `BRAWLER_DIAGNOSTICS_*`, `BRAWLER_ROUTED_WINDOW_DIR`, `BRAWLER_LOBBY_TRANSITION_*` and `BRAWLER_SOURCE_DIRTY`. `BRAWLER_FAILURE_REPORT` is wanted in production (BRL-71r3).

## Existing isolation
`scripts/check-server-features.sh` already forbids client, render, window and audio crates in the server graph.

# Decisions (owner-approved 2026-11-21)
1. **Release mode is explicit.** The image entrypoint passes `--release-profile` (name chosen at implementation) to the supervisor. Release mode refuses to start:
   - with any verification or diagnostic flag;
   - with any `BRAWLER_*` control except an allowlist (`BRAWLER_FAILURE_REPORT`, `RUST_LOG`-style logging and the BRL-z277 key path);
   - with the development key.

   Workers inherit release mode through their manifest and apply the same environment check.
2. **Build identity.** `brawler-server routing-identity` (or a sibling `build-info` subcommand) also prints the enabled feature set and the git revision. The image build fails unless the features are exactly the release set (`server`, plus the token-service feature for that binary).
3. **CI gate.** The Linux image job (BRL-jjwa) runs the image's `build-info`, and greps the image filesystem to check that no `brawler-client`, Balance Lab web output or `tools/` content is present.

# Scope
Decisions 1–3, with unit tests for the flag and environment refusal and a CI step.

# Acceptance
The release supervisor refuses each forbidden flag, each non-allowlisted `BRAWLER_*` variable and the development key, with a clear error. CI fails on an image built with `balance-lab`.

# Verification
`just lint`, `just test`, and the CI image job.
