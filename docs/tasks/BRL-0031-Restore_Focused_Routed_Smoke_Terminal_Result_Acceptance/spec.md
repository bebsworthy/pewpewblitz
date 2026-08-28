# Reproduction

On clean detached `HEAD` c34d339 and on the BRL-0025 worktree:

```sh
BRAWLER_NETWORK_HEADLESS=1 BRAWLER_ROUTED_TIMEOUT_SECONDS=90 RUST_LOG=brawler=info ./scripts/network-routed.sh
```

Both clients authenticate the lobby, accept the same grant, connect to the match, receive the map and accepted player identities, and the match worker exits successfully after the verification match. The supervisor then reports `WorkerExitMismatch`, cleans the worker without completing the expected terminal result transaction, and both clients time out instead of returning to a fresh lobby. The product 1v1 smoke that exits after Active passes.

# Scope

- Trace the match worker Result/control emission, packet-drain completion, supervisor control sequencing, process reaping, and allocation terminal state for the verification-rules completion path.
- Determine why a successful worker process exit is classified as `WorkerExitMismatch` before or without an accepted Result.
- Correct the smallest owning routing/server lifecycle defect.
- Preserve protocol bytes, rejection categories, security boundaries, process topology, and normal product match behavior.
- Add focused characterization for the failing Result/process-exit order and retain worker failure isolation.

# Acceptance criteria

- The reproduction command passes repeatedly and prints the fresh-lobby transition success marker.
- The supervisor accepts the match Result, drains/revokes routes, and does not classify the successful worker as `WorkerExitMismatch`.
- Both clients observe authoritative completion and authenticate a new lobby generation without timeout.
- Product 1v1 routed smoke remains green.
- Routing, server, client, network, and performance suites pass.
- Verification evidence, root cause, feedback disposition, learn-from-errors review, and conflict-free `ticket sync` are recorded.

# Non-goals

- No protocol version/schema change, timeout relaxation, hidden retry, or suppression of genuine worker-exit mismatches.
