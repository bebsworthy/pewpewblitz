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


# Progress 2026-11-16
**Host repro (subagent):** no failure. A phone-style `kill -9` mid-match, then a relaunch on the same or a different source port, at 0-23 s delays: the lobby connected in about 0.13 s and the seat was always reclaimed.
- Ruled out for the incident: each relaunch attempt uses a new random port and a new client id, and lightyear logs every refusal except `Ignored`. A stale lobby route (lead b) is real but bounded at 10 s, and only for a kill while in the lobby. With the same client id, `ClientIdInUse` refusals can use up the 8-datagram pre-auth budget.
- Latent margin: with a lobby round trip of 700 ms or more, the pre-auth budget runs out before the challenge-response is sent, so the client can never connect.

**Diagnostics committed (70a46541):**
- Supervisor `brawler-supervisor ingress dropped reason=… source=…` (rate-limited), a per-window `summary`, and `lobby-route-reused … route_age_ms idle_ms connection_request`.
- Client `routed UDP socket bound local_addr=… route="lobby"|"match"`.

**Pixel 8a cycles (HEAD 70a46541, server 0.0.0.0:5000):** Wipeout 1v1 against a headless opponent that leaves at Active (seat held), then force-stop 8 s into the match, relaunch, CONNECT.
- Valid cycles 1, 2, 3 and 5: rejoined 5-6 s after relaunch with `seat reclaimed by a rejoined session`, first connect attempt each time. (Cycle 4 is invalid: a failed adb `logcat -c` left a stale flow.)
- **Cycle 6, new failure, before any kill:** the lobby to match handoff connect timed out.
  - Phone 20:26:49.79Z: `socket bound local_addr=0.0.0.0:36991 route="match"`, then 10 s later `client connect failed. connection request timed out`.
  - Server: the match worker was spawned and ready at 20:26:48.96, the lobby completed the handoff, and the opponent connected to the match at 49.13. **There was no ingress drop line and no `New connection` for the phone.** The worker later exited (code 0, then `worker failed category=WorkerExitMismatch`).
  - So the phone's netcode requests either never reached the supervisor, or reached the match worker and were silently `Ignored` by lightyear_netcode (unlogged), for example because of a token or address check.
  - The same signature as the original incident: no challenge and no server-side trace.
  - The phone then stayed on "Match loading / Connecting" indefinitely (filed separately).
- Earlier the same day, two other "first connect timed out" cases had environmental causes: the server was unreachable after the Mac hibernated, and once the phone was on another Wi-Fi subnet.

**Next:** count, per route, the public datagrams the supervisor forwards to a match worker, and log why lightyear_netcode ignores a connection request in the worker. Then repeat the device cycles until the handoff timeout recurs.


# Resolution 2026-11-16
**Device verification (Pixel 8a, HEAD 2359aa41, server with full tracing and `lightyear_netcode` debug logs):** the force-stop/relaunch rejoin path passed **14 of 14 valid cycles** (1, 2, 3, 5 and 7-16).
- Each relaunch connected on the first attempt (`socket bound … route="lobby"` once) and was back in the match 5-6 s after relaunch, 9-11 s after the kill, with `brawler match seat reclaimed by a rejoined session`.
- Cycles 7-16: 61 `handshake completed` lines, no `Netcode error` warnings, and no ingress drops for the phone.
- Two log lines are false positives, both an 18-byte stray non-request packet from the phone's just-closed lobby session: one `handshake stalled … packets=1 connection_requests=0` and one `dropped packet for a closed route`.
- The `Netcode error: Ignored` debug lines are trailing lobby packets after each handoff, and harmless.

**Not reproduced:** neither the original lobby-connect outage nor the cycle-6 match-handoff timeout came back in 24 device cycles plus the host matrix. A recurrence is now fully attributable from logs:
- no `capability-route-first-seen` / `ingress dropped` line for the phone's source means the packets never reached the Mac;
- `peer handshake stalled challenge_sent=true` or `replies>0` means the return path dropped the replies (`routed_udp.rs` drops replies whose source differs from the server it sent to);
- `challenge_sent=false` together with a `Netcode error` warning means a refusal.

**Follow-ups:**
- BRL-hjmq: the stuck "Match loading" screen after a connect timeout.
- A new backlog ticket for the rare handoff timeout, with this grep guide.
- A new backlog ticket for the pre-auth budget, which fails at a lobby round trip of 700 ms or more.

Owner: if it recurs, run `RUST_LOG=info,wgpu=error,naga=warn,lightyear_netcode::error=debug,lightyear_netcode::server=debug BRAWLER_ROUTED_BIND=0.0.0.0:5000 just server` and keep the phone's logcat.


# Reopened 2026-11-16 22:15Z: reproduced with the new tracing (HEAD 721f68cc)
A fresh app launch, then CONNECT to 192.168.31.47:5000. The phone (`socket bound local_addr=0.0.0.0:41768 route="lobby"`) timed out after 10 s. Server, in order:
- `22:14:53.927 routed worker peer opened worker=<lobby> route=136528…015 peer=140579…221 payload_len=1078 connection_request=true`
- `22:14:54.024 routed worker dropped packet for a closed route or peer … route=136528…015 … connection_request=true` (about 100 ms after open; later drops are folded)
- `supervisor ingress dropped reason=source-limited source=192.168.31.63:41768`, with `source_limited=84` in that window
- `22:14:55.945 peer handshake stalled … packets=1 connection_requests=1 challenge_sent=true`

**Cause class:** the lobby worker accepted the first connection request and sent a challenge, then treated the route or peer as closed about 100 ms later. Every retry and the challenge response were then discarded by the worker, until the 8-datagram pre-auth budget was used and the supervisor rate-limited the source. Phone↔Mac RTT at the time was 60-80 ms.

Evidence: scratchpad/hm/server-2nsg-repro.log and 2nsg-repro-logcat.txt.
