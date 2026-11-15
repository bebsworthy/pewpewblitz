Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec refreshed with code refs and a second lead (a stale per-source lobby route). Not diagnosed. 3437c0d8 changed only `src/server/seat_hold.rs` marker visibility, not ingress.

# Found
Device check 2026-11-15 (Pixel 8a), HEAD b97a8888, server on 0.0.0.0:5000. Seen once in 4 force-stop/relaunch cycles.

# What happened
- 17:32:36Z: phone in a Wipeout 1v1 (player 12), headless opponent seat held from 17:32:49. Force-stop about 17:32:45; the phone's seat was held at 17:32:55.
- The relaunched app, CONNECT at about 17:33:0x and RETRY at 17:34:39: "client connecting to server 192.168.31.211:5000" then TimedOut after 10 s (Connection Lost screen). The server log has no "New connection" from the phone in that time.
- During the outage, `adb shell ping 192.168.31.211` succeeded, and a headless client from the Mac (127.0.0.1:5000) connected at 17:33:59. The macOS firewall is off.
- RETRY at 17:35:15 connected at once. The held seat had expired in the meantime (17:33:55), so the rejoin was lost (match forfeit).

# Code at HEAD (packages/brawler-routing/src)
- `runtime/routing_io.rs` `receive_public` (L17-100) drops a datagram silently (only `note_error` counters) in four cases: the source is suppressed (L30-34); a malformed envelope pushes the source into 60 s suppression (L37-55); a DefaultLobby envelope arrives before `default_lobby_ready()` (L67-72); `admit_default` returns `SourceLimited` / `Suppressed` (L73-80).
- `ingress.rs` `SourceIngressLimiter`: per `SocketAddr`, 8 pre-auth datagrams / 9 KiB per 10 s window, 32 malformed → 60 s suppression (`limits.rs` L65-69). Promotion to `authenticated` happens only on a capability selector (`routing_io.rs` ~L95). Records expire 60 s after `last_seen` (`ingress.rs` L151-156, from `runtime/capacity.rs` L98).
- Counters only leave the process at shutdown: `bin/supervisor.rs` `write_metrics` (L154+, `source_limited`, `error_counts`) and only with `--metrics-file`. Nothing is logged live.
- Second lead: `supervisor/mod.rs` `route_public` DefaultLobby (L428-443) reuses `lobby_sources[source]` when it exists. The client binds `local_addr` port 0 (`src/config.rs` L334, `packages/brawler-client/src/routed_udp.rs` L207-227). If Android hands the relaunched app the same ephemeral port, its new Netcode connection requests reach the old lobby route and session for that address. The Netcode server (lightyear_netcode 0.29 `server.rs` ~L786) may ignore them until the old session times out. That would fit "once in 4" and the later recovery.

# Plan
1. Add a rate-limited supervisor diagnostic: at most one line per source per window, plus a periodic summary. It names the drop reason (Suppressed / SourceLimited / LobbyNotReady / Malformed) and the source. Also log when a DefaultLobby datagram reuses an existing `lobby_sources` route, with route age and last seen.
2. Log the client's bound local address on every connect attempt (routed_udp bind).
3. Reproduce: 10 force-stop/relaunch cycles mid-match with `BRAWLER_ROUTED_BIND=0.0.0.0:5000 just server` on port 5000. Correlate the phone's source port with the diagnostics.
4. Fix according to the cause. For example: drop a stale lobby route when a fresh Netcode connection request arrives from its source; or give the client a fresh source per attempt and ensure promotion; or loosen the pre-auth budget for Netcode handshake retries. Add a routing runtime test (`runtime/tests.rs`) for the scenario found.

# Acceptance
- The cause is identified and recorded on this ticket, with the log lines.
- The new runtime/supervisor tests pass: `cargo test -p brawler-routing`, and `cargo test --features network-test --test routed-process` if routing changes.
- **Pixel 8a device check:** 10 force-stop/relaunch cycles during a match. Each relaunch connects on the first attempt and reclaims the held seat inside the 60 s hold, or the ingress outcome is logged for any miss.

Evidence (/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/vc): r4-ss.jpg, r4-c.jpg, r4-logcat2.txt, r4-logcat3.txt, r4-logcat4.txt, probe.log, server.log (17:32:36-17:35:16).
