# Found
BRL-2nsg device cycle 6 (2026-11-16, Pixel 8a, HEAD 70a46541). During a normal lobby-to-match handoff the phone bound `route="match"` (local port 36991), sent Netcode connection requests for 10 s, and timed out. The server logged no ingress drop and no `New connection` for it. The headless opponent joined the same worker normally. It did not recur in 10 later traced cycles or in the host matrix. The original BRL-2nsg lobby-connect outage had the same no-trace signature.

# Plan
Run phone sessions with the tracing from BRL-2nsg (commits 70a46541, 2359aa41) and `RUST_LOG=…lightyear_netcode::error=debug,lightyear_netcode::server=debug`. On a recurrence, classify it:
- no `capability-route-first-seen` / `ingress dropped` line for the phone's source: the packets never reached the Mac (Wi-Fi, Android or OS side);
- `peer handshake stalled challenge_sent=true` or `capability-route-window replies>0`: the return path dropped the replies. Check the reply source address against `RoutedUdpIo::receive`'s `public_peer` filter (packages/brawler-client/src/routed_udp.rs ~L309);
- `challenge_sent=false` with a `Netcode error`: a refusal.

Fix according to the class, with a test.

# Acceptance
The cause is recorded with logs, then fixed or shown to be environmental.


# Likely explained (2026-11-16)
BRL-2nsg's root cause, a same-burst race in `routed_worker.rs receive` that silently closed a new peer when its first two packets arrived in one IPC burst (fixed in 1bea10fd), applies to match capability routes as well. The cycle-6 signature, a timeout with no server-side trace before the diagnostics existed, fits it. Keep this ticket open until the handoff runs clean across more device sessions; on a recurrence, the closed-drop line now names `closed_reason`.

# Device run 2026-11-19 (Pixel 8a, Android 17): no recurrence in 25 handoffs
Setup: a worktree routed server built from main caee2a43 (which includes the 1bea10fd same-burst fix) on `0.0.0.0:5150`, data in `target/pixel/server`, `RUST_LOG=info,wgpu=error,naga=warn,lightyear_netcode::error=debug,lightyear_netcode::server=debug`. Phone: the main-branch release APK on Wi-Fi (source 192.168.31.63), Wipeout 1v1. For each cycle a fresh headless opponent (`--product-match-smoke-1v1 --product-game-type wipeout-1v1 --leave-match-cue <existing file> --exit-after-lobby-return`) queued, and the phone pressed PLAY / PLAY AGAIN. The opponent left as soon as the match was Active, and the phone got Victory by forfeit and returned to a fresh lobby. Each cycle is a full lobby → match handoff and a match → lobby return.

Result: **25 of 25** handoffs connected on the first attempt. The phone logged `routed UDP socket bound … route = "match"`, then `Client Netcode(…) connected` 0.23–0.77 s later (median about 0.38 s), and reached Results each time. Server side: 25 `capability-route-first-seen … kind=match source=192.168.31.63:*` lines, one per handoff. None of the 25 match handoffs had an ingress drop before connect, a `handshake stalled`, or a non-`Ignored` `Netcode error`.
- The only post-close `ingress dropped … category=Revoked` lines and `Netcode error: Ignored` debug lines come from a route after its match ended.
- Two `handshake stalled … packets=2/9 connection_requests=0 challenge_sent=false` warnings were on lobby-worker routes. Each opened with an 18-byte non-request packet: the same known false positive as BRL-2nsg (a stray packet from a just-closed session), not a handoff.

**Explained:** the cycle-6 signature (a match-route connect that times out with no server trace) matches the BRL-2nsg same-burst receive race in `routed_worker.rs` that 1bea10fd fixed. It applied to match capability routes too. With the fix and the full tracing, 25 phone handoffs ran clean, on top of the 24 BRL-2nsg device cycles. No code change in this ticket. If it ever recurs, the diagnostics above classify it, as listed in the Plan. Evidence: session scratchpad `d3hb/` (`results.txt`, per-cycle phone logcat and headless logs) and `target/pixel/server.log` in the worktree.

## Closed (2026-11-20)

25/25 traced phone handoffs connected first try (0.23-0.77 s) with no drops, stalls or refusals. Recorded as explained by the BRL-2nsg same-burst fix (1bea10fd). Reopen on recurrence; the closed-drop line names `closed_reason`.
