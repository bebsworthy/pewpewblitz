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


## Related native review evidence, 2026-09-08
During BRL-0119 review, the isolated 127.0.0.1:5134 supervisor (/tmp/brl117-preview-server.log) spawned worker 78084145296260721536696168547249502481 at ts_ms=1788891973378, reported ready, then reaped code 0 about 22 seconds later with WorkerExitMismatch. The GUI displayed Connecting and later returned to Dashboard; gameplay was not verified. Similar exits appear earlier in the same log before the loading UI change. This is related evidence, not a confirmed identical root cause; correlate client and worker readiness before assigning causality.

2026-09-08 user authorized investigation after shell commit 536ea3d. First reproduce current Practice handoff with matched binaries and isolated data; distinguish native readiness failure from historical terminal Result race before changing lifecycle code. Validate Practice loading/gameplay/return as BRL-0119 evidence. Preserve unrelated workspace changes.

Confirmed reproduction on current source: both diagnostic clients authenticate the match, but drive_match_loading_check_in excludes non-product-shell headless clients. They instead send legacy SetReady, never populate the allocation loading gate, and the 20-second readiness deadline plus 2-second exit produces WorkerExitMismatch before any Result exists. Correct all routed clients to use allocation check-in, preventing legacy MatchCommand readiness from racing it. Re-run canonical terminal smoke before deciding whether terminal IPC needs changes. Fresh Practice 2v2 automated Active check passed.

Native shell root cause confirmed in captured client log: initial lobby uses selected 127.0.0.1:5134 and generated identity; advance_routed_transition spawns match using immutable startup config (127.0.0.1:5000/client 0). Preserve the effective product lobby connection config and logical target across entity teardown, use it for both match and fresh-lobby generation. This is the owning client lifecycle correction for native Practice. A separate diagnostic issue remains: --match-rules verification no longer overrides explicit production duration in automatic lobby allocations; investigate bounded fixture rather than relaxing watchdog.

The corrected diagnostic check-in avoids the 22-second exit. Remaining watchdog failure is a 180-second production match: the automatic lobby allocation hardcodes embedded catalog values, so supervisor --match-rules verification has no effect after explicit rules moved into allocation requests. Use the supervisor-supplied raw catalog in the diagnostic allocator and give the canonical verification launcher an isolated 60-second catalog fixture (matching existing verification active duration), retaining its 90-second watchdog and normal production values. This repairs the harness configuration without changing protocol or loosening deadlines.

Verification: canonical routing suite (84 unit + integration groups), client (672 + public API), server (516 + public API), network (97), and performance (12) all passed via just _test-routing _test-client _test-server _test-network _test-performance. Client/server library Clippy passed with -D warnings. The canonical terminal smoke passed twice (~68 seconds) with accepted Result, clean Exit, and both clients authenticated into fresh lobby; no terminal IPC change was needed. Native Practice on selected port 5134 reached actual gameplay; Leave Match returned to Dashboard on the same endpoint and Netcode identity (log /tmp/brl31-native-final.log). ECS regression verifies endpoint, identity and logical target across match and return; server regression verifies supervisor catalog duration. Remaining genuine loading-failure AppExit cleanup is explicitly deferred to BRL-0121, independent of successful handoff.

Learning: the reported worker-exit symptom was downstream of missing readiness, and the native failure was hidden by tests using identical startup/runtime connection settings. Preserve connection state at the session lifetime, test nondefault runtime endpoints and IDs across entity replacement, and derive smoke duration from the current authoritative catalog contract rather than obsolete profile overrides. Initial investigation treated Result timing as a hypothesis; logs disproved it before lifecycle code was changed. Native keyboard automation must target the actual process to avoid mistaking a restarted app for a successful return.

Final product gates: just practice-e2e wipeout-1v1 passed; just e2e 2 passed (exact multiplayer 1v1 reached Active). git diff --check passed. Implementation and verification scope complete; unrelated dirty work preserved. Changes are not committed in this turn.
