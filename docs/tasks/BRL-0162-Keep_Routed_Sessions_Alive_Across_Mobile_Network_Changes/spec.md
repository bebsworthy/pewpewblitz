# Outcome

Mobile network changes cause a short reconnect or a clear message, not a broken session.

# Context

Lightyear UDP identifies peers by remote socket address, so a Wi-Fi to cellular switch or NAT rebinding looks like a new peer and the old link times out. Some networks block UDP. Lightyear offers WebSocket/WebTransport transports.

# Scope

- Detect an address change or connection loss, then automatically reconnect through routed admission and rejoin the same match while it is still running.
- Bounded retries with a clear exit to the Dashboard.
- Evaluate and, if justified, add a WebSocket transport fallback.
- Handle app suspension longer than the server timeout.

# Out of scope

Server-side matchmaking changes beyond rejoin support.

# Acceptance criteria

- A Wi-Fi to cellular switch mid-match rejoins the same match within an agreed bound, or exits cleanly with a message.
- On a UDP-blocked network the client either connects through the fallback or shows an actionable error.
- Server authority and admission rules are unchanged.

# Verification

- A routed-process test for rejoin.
- A device test.

# Source

2026-10-23 review of commit `cc4d3da` (code, in-game/out-of-game UI/UX, rendering, engine, Android). Review page: https://claude.ai/artifact/HtwNHVkZueKuu6BrmGmXHN. Live 3v3 Practice capture taken the same day.


# Research and design (2026-11-14, before implementation)

## Findings (code and Lightyear 0.29 source)
- **Routing is already address-independent for match sessions.** The supervisor routes match
  traffic by the 256-bit capability, not by source address (`docs/14`), and `CapabilityRegistry`
  already permits NAT/network rebinding: at most 2 source changes per 10 s, outbound traffic follows
  the newest source (`route_sources`), and an Active capability is revoked after 10 s with no
  packets (`CAPABILITY_IDLE_MILLIS`). The match worker keys peers by `(RouteId, PeerId)`, so its
  Netcode session never sees the client's address. Lobby routes are keyed by source address
  (`lobby_sources`), so a lobby session survives an outage at the same address but not an address
  change.
- **The immediate killer is the client transport.** `RoutedUdpIo::send` triggers `Unlink`
  ("routed UDP send failed") on any non-WouldBlock `send_to` error, and `receive` does the same on
  receive errors. With Wi-Fi off (no route) Android returns ENETUNREACH, so the session is torn down
  on the first frame of an outage even though every server-side bound would have tolerated it.
- **Session timeout is the client's connect-token timeout.** With `Authentication::Manual`, Netcode
  stores `token.timeout_seconds` per client on the server (`lightyear_netcode` `conn_cache.add`), so
  the client's `NetcodeConfig::client_timeout_secs` is the keep-alive bound on *both* ends; the
  worker's `ServerNetworkConfig::client_timeout` (3 s) only applies to server-issued tokens. Today a
  routed session spawned by a transition inherits `connect_timeout` from whatever config spawned the
  lobby attempt (product shell: ceil of the attempt time left, normally 10 s; automation 10 s;
  tests 5 s) — the bound is accidental.
- **Rejoining a match after the session has timed out is not supported by the server**: the match
  worker rejects hellos once the match left `Waiting` (`MatchInProgress`), admission is one live
  connection per manifest participant, and the capability/route is revoked on idle or peer close.
  A post-timeout rejoin needs a lobby-mediated fresh capability for an existing route/peer, worker
  re-admission of an in-progress participant and fighter re-attachment — new lobby↔supervisor↔worker
  control-IPC contracts. That clearly exceeds this ticket's sanctioned changes.
- **BRL-0199 is on this ticket's path.** Both the post-match return and the return after a lost
  match session leave the flow on `Match` while a fresh lobby generation connects. If that lobby
  attempt fails, `observe_session` has no rule for "flow is Match, routed phase is Lobby, lobby
  generation Disconnected", so the screen stays on MATCH COMPLETE forever. A beyond-bound network
  loss mid-match reaches exactly that state when the network is still down.
- The capability registry rejects a source it has seen before (`seen_sources`), so a
  Wi-Fi → cellular → same Wi-Fi flap (same address and port) is refused after the second change.
  Changing that is a routing-security policy change (V2 M01 "old source then drops").
- WebSocket fallback: the supervisor owns one public UDP socket and the route envelope is a UDP
  datagram contract. A WebSocket fallback needs a TCP (and for public servers TLS) listener in the
  supervisor, a framed stream envelope, a second client transport, and head-of-line-blocking
  evaluation — a new transport, which exceeds this ticket's direction without evidence of a
  UDP-blocked target network. Evaluated, not justified now.
- Cellular on the test phone cannot reach the Mac's LAN server (192.168.31.47), so the device test
  can exercise a Wi-Fi drop/recovery, not a Wi-Fi→cellular switch to a reachable server.

## Decisions
1. **Keep the existing session alive through a network gap ("in-place rejoin").** Within the bound
   the same Netcode session, fighter, and match continue; no handoff or re-admission occurs, so
   server authority and admission are unchanged.
   - `RoutedUdpIo` treats transient network errors (network/host unreachable, network down, address
     not available, connection refused/reset/aborted, not connected, permission denied, timed out,
     interrupted) on send and receive as UDP packet loss: the datagram is dropped, the socket and
     link stay up, and one warning is logged per outage with a recovery log. Other errors still
     unlink. Netcode's keep-alive timeout remains the single authority that declares a session lost.
   - The wildcard-bound socket is kept; after the network returns the OS picks the new source
     address and the supervisor's existing capability rebind follows it.
2. **Agreed bound = 10 s** (`ROUTED_SESSION_TIMEOUT`, derived from `CAPABILITY_IDLE_MILLIS`, the
   supervisor's idle revocation, so neither end outlives the other). Routed sessions spawned by
   routed transitions (match session and post-match lobby) use it as their Netcode token timeout
   explicitly instead of the inherited connect-attempt value. Product lobby attempts keep their
   candidate-bounded value (10 s for a numeric address).
3. **Beyond the bound** (long outage, app suspended longer than 10 s): the match session times out,
   the client starts a fresh lobby session (existing path) and Dashboard shows an honest message:
   "The connection to the match was lost, and the match could not be resumed." (replaces "The match
   server stopped unexpectedly", which was wrong for network loss).
4. **BRL-0199 fix folds in:** if the fresh lobby session after a match (completed or lost) fails
   while the flow still presents the match, the flow commits the recoverable connection error
   (Retry / Change Server) instead of staying on MATCH COMPLETE. No automatic retry is added (the
   Retry button is one tap; the lobby attempt already has a 10 s bound).
5. **UDP-blocked networks: actionable error, no fallback transport.** No-reply connection failures
   say the server did not answer and that some networks block the UDP traffic the game uses, with
   Retry / Change Server. WebSocket fallback is deferred to a new ticket.
6. **Deferred to new tickets:** post-timeout match rejoin (server re-admission), returning to a
   previously seen source address in the capability rebind policy, WebSocket/TCP fallback.
7. Android `MainActivity` is unchanged: a wildcard UDP socket survives Android default-network
   changes, so ConnectivityManager callbacks are not needed for this behavior.

## Acceptance-criteria refinements
- AC1 is met as: a network gap of up to 10 s mid-match (including a source-address change) keeps
  the same match session; a longer gap exits to Dashboard with the message above; a failed lobby
  return shows Retry / Change Server.
- AC2 is met as: on a UDP-blocked network the connect attempt ends in the actionable error (no
  fallback transport in this ticket).
- AC3: no wire, admission, or capability-policy change.

## Verification plan
- Unit: transient-error classification and the transport keeping its socket/link on a transient
  send failure; session timeout constant equals the capability idle bound; flow tests for the
  failed lobby return during match completion and after match loss, and for the new copy.
- Separate-App network scenario (`tests/network/`, harness): a full two-way link blackout shorter
  than the session timeout keeps the same server session and fighter and resumes input; a blackout
  longer than the timeout disconnects on both sides.
- Routed-process smoke (`scripts/e2e-network-change.sh`): a real supervisor + workers + headless
  client, a mid-match blackout followed by a new client source port (forced socket rebind), and the
  match continuing on the same session.
- `just lint`, `just test`, `just e2e 2`; Pixel 8a Wi-Fi disable/enable mid-match if attached.


# Implementation (2026-11-14)
- `src/client/routed_udp.rs`: transient socket errors on send/receive (`is_transient_network_error`)
  are UDP packet loss: the datagram is dropped, the socket and link are kept, and one warning
  ("routed UDP network unavailable; keeping the session") plus one recovery line are logged per
  outage. Other errors still unlink. Automation-only `set_simulated_outage` / `rebind_socket` drive
  the process smoke.
- `src/client/session/mod.rs`, `connection.rs`, `routing.rs`: `ROUTED_SESSION_TIMEOUT` (10 s =
  `CAPABILITY_IDLE_MILLIS`) is the explicit Netcode token timeout for sessions spawned by routed
  transitions (`spawn_client_entity` takes the timeout explicitly).
- **Bug found on device and fixed:** a match session lost to a Netcode timeout was never unlinked
  in the product shell. `drive_routed_transition` only unlinked sessions it had asked to disconnect,
  so after `MatchFailed` the shell sat on a dark match screen forever (the same dead state BRL-0199
  reports). It now unlinks any disconnected session during a routed transition.
- `src/client/session/suspension.rs` (new): on `AppLifecycle::Running` after a suspension at least
  as long as the session bound, routed sessions are disconnected at once instead of the player
  watching a frozen match for another 10 s (Bevy clamps the clock, so Netcode only noticed 10 s after
  resume).
- Flow (`src/client/flow/observation.rs`, `actions.rs`, `reducer.rs`, `reducer/match_flow.rs`):
  `LobbyReturn` state and the new `SessionObservation::LobbyReturnFailed` move a failed post-match
  lobby return from the match screen to the Connection Lost panel (Retry / Change Server). This is
  the BRL-0199 fix. Copy: the match-loss notice now reads "The connection to the match was lost, and
  the match could not be resumed."; no-reply connection failures add "Check the address and that
  the server is running. Some networks block the UDP traffic the game uses; if so, try another
  network."
- Automation: `--product-network-change-smoke` (`ClientNetworkConfig::product_network_change_smoke`,
  `session/automation.rs` driver), `BRAWLER_PRODUCT_NETWORK_CHANGE_SMOKE` in
  `scripts/network-product-match.sh`, new `scripts/e2e-network-change.sh`, and the
  `just network-change-e2e [game type]` recipe (README).
- Tests: routed_udp classification and a macOS EADDRNOTAVAIL send that keeps the socket and link;
  flow observation and reducer tests for the failed lobby return (both the completed-match and the
  lost-match copy, through the real `observe_session`); a transition test that a timed-out match
  session is unlinked; suspension tests; `tests/network/session_continuity.rs` (harness
  `LinkBlackout`, which drops inbound datagrams before `ConnectionSystems::Receive` on both ends):
  a two-way outage shorter than the session timeout keeps the same session, fighter, and input
  control, and a longer one is declared lost by both ends.
- Docs: `docs/14` gains "Network-change continuity"; `docs/13` error contract updated.
- No wire, manifest, control-IPC, admission, or capability-policy change. Android Java is unchanged.

# Verification (2026-11-14)
Evidence: agent scratchpad `brl0162/` (logs, logcat dumps, screenshots), not kept in the repo.
- `just lint` with ripgrep 14.1.1 first on PATH: **pass** (exit 0; final run after every change).
- `just test`: **pass**: 23 suites `test result: ok`, 0 failed.
- `just e2e 2`: **pass**: 1 exact 1v1 roster reached Active.
- `just network-change-e2e`: **pass**. Real supervisor and workers with one headless client in
  Practice wipeout-1v1. The client dropped all traffic for 4 s, then resumed from a new source port
  (127.0.0.1:53125 → :56851). The same generation-2 match session kept receiving authoritative ticks
  (268 → 511).
- **Pixel 8a device** (Android 17, Wi-Fi to the Mac at 192.168.31.47:5001, final APK):
  1. Short Wi-Fi drop mid-Practice (`svc wifi disable`, 2 s, `enable`; about 4.5 s without
     traffic): the client logged "network unavailable; keeping the session", then "available
     again". There was no disconnect on the client or the server, and the match continued (score
     and clock advanced).
  2. Long drop (Wi-Fi off 36 s): the match timed out at exactly last-receive + 10 s. The fresh
     lobby attempt failed while Wi-Fi was still off, and the Connection Lost panel showed "The
     connection to the match was lost and the lobby could not be reached…" with Retry / Change
     Server. The first Retry, about 15 s after Wi-Fi came back, timed out with the new UDP hint
     copy. The second Retry reached Dashboard without restarting the app.
     Before the routing fix, the same scenario stuck on a dark screen; that was the bug found above.
  3. Home for 18 s mid-match (suspension beyond the bound): 1 s after resume the client logged
     "routed session outlived by app suspension; disconnecting". It started a fresh lobby,
     reached the lobby within 0.3 s, and Dashboard showed "The connection to the match was lost,
     and the match could not be resumed."
  4. A second short drop on the final build (3 s off) exited cleanly the same way. When Wi-Fi came
     back, the phone joined a **different saved Wi-Fi** (192.168.1.x) that cannot reach the Mac, so
     the session and the lobby retry correctly failed. The phone is still on that network.
- Cellular: the phone has LTE, but cellular cannot reach the LAN server, so a Wi-Fi → cellular
  switch to a reachable server was not exercised on device. While Wi-Fi was off, Android routed the
  socket over LTE: sends succeeded, but the datagrams were lost. The process smoke covers the
  source-address change.

# Playtest request
Server: `BRAWLER_ROUTED_BIND=0.0.0.0:5001 just server`. Install the phone build with
`just android-install <MAC_IP>:5001` and keep the phone on the same Wi-Fi as the Mac.
1. In a Practice match, toggle Wi-Fi off and back on quickly from quick settings. Within about 10 s
   without traffic the match should continue and your fighter should still respond.
2. Leave Wi-Fi off for 20 s or more. Expect "Connection lost … lobby could not be reached", then
   Retry once Wi-Fi is back. A first Retry can time out while Android still settles the network.
3. Press Home for 15 s or more mid-match, then return. Expect Dashboard and the match-lost message
   right away.
4. On a public or cloud server, if available: switch Wi-Fi to cellular mid-match. Expect the same
   session to continue if traffic resumes within 10 s.

Please report whether 10 s feels like the right bound and whether the copy is clear.

# Feedback disposition
- No owner feedback yet. The agreed bound (10 s) and the deferral of post-timeout rejoin await the
  owner's confirmation.

# Deferred (new todo tickets, parent BRL-0171, related BRL-0162)
- BRL-0204: rejoin a running match after the session timed out (lobby-mediated capability and
  worker re-admission).
- BRL-0205: allow a match route to return to a previously seen client address (Wi-Fi → cellular →
  Wi-Fi flap).
- BRL-0206: evaluate a non-UDP fallback transport for UDP-blocked networks.
- BRL-0207: restyle the Dashboard connection-error overlay. It renders in a monospace debug style.
- BRL-0208: accept a fresh lobby login after the app was killed mid-session. An immediate relaunch
  is rejected with AccountInUse for up to 10 s.
- Not filed: automatic lobby retry after a failed return (BRL-0199 suggested one). Retry is one tap,
  and the device run showed that the first retry can fail while Android settles the network.

# Learn-from-errors
- **Mistake:** the first design assumed the existing product shell already handled a lost match
  session ("existing path"). The device test showed it never unlinked a session it had not asked to
  disconnect, so loss beyond the bound produced a dark screen. **Cause:** only the automation path
  (`enforce_routed_timeout`) had been exercised for unexpected match loss; the product shell's
  transition assumed `disconnect_requested`. **Prevention:** when a design relies on an "existing"
  recovery path, find the test that drives that path in the same execution role (product shell, not
  automation) before relying on it. If none exists, add one first.
- **Mistake:** the first harness blackout drained `Link.recv` before `TransportSystems::Receive`, but
  Netcode reads in `ConnectionSystems::Receive`, which runs earlier. The "short outage survives"
  test passed vacuously. **Prevention:** pair every "survives X" network test with a "fails beyond
  X" twin that uses the same injection; the twin's failure exposed the ineffective injection.
- Suspension: Bevy clamps frame deltas, so client timeouts measured on the Bevy clock do not cover
  OS suspension. Measure wall time across `AppLifecycle` events instead.
- adb on macOS 27 restarts its daemon unpredictably and kills `adb logcat` streams. Use
  `logcat -d` dumps per step for evidence.
