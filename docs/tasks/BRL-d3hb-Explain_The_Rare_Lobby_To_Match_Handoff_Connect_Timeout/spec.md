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
