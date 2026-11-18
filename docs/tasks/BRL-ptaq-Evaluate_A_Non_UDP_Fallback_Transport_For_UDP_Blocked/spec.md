Reviewed 2026-11-15 against HEAD 3437c0d8: still valid (evaluation only, nothing built); facts refreshed with file refs and the rejoin contract.

# Outcome
A measured decision on a WebSocket/WebTransport (or TCP) fallback for the routed public endpoint.

# Context (from BRL-g57a, 2026-11-14; rechecked 2026-11-15)
BRL-g57a evaluated and did not add a fallback: the supervisor owns one public UDP socket and the route envelope is a datagram contract. A fallback needs a TCP (and TLS for public servers) listener in the supervisor, a framed stream envelope, a second client transport (`lightyear_websocket` / `lightyear_webtransport` 0.29.0 exist in the registry; the workspace pins `lightyear = "=0.29.0"` with only `lightyear/udp` enabled, Cargo.toml:35 / :122), and head-of-line-blocking measurements. BRL-g57a instead shows an actionable error: `NO_REPLY_HINT` in packages/brawler-client/src/flow/reducer.rs:758-768 ("Some networks block the UDP traffic the game uses…").

Current state:
- docs/14-multiplayer-server-architecture.md:128-129 states "A non-UDP fallback transport is not part of this contract."
- Since BRL-g57a the routed contract grew: bounded source rebind (BRL-sqz0), match rejoin after routed-session timeout (BRL-yyeg, 6a6d6599: packages/brawler-routing/src/runtime/rejoin.rs, src/server/lobby/rejoin.rs, src/server/seat_hold.rs), and Leave Match / SeatReleased (b97a8888). Any fallback must carry capability/rebind/rejoin semantics too (protocol 49, control IPC 7).
- Supervisor public socket: packages/brawler-routing/src/runtime/mod.rs.

# Scope
- Evidence that target players hit UDP-blocked networks (playtest reports, how often the no-reply hint appears; none recorded yet).
- Costed design: supervisor ingress (TCP/TLS listener beside the UDP socket), stream envelope framing vs the datagram route envelope, capability handling incl. rebind/rejoin, client transport selection and fallback order/timeouts, Android behavior (Pixel 8a), and a head-of-line latency measurement under loss.

# Plan
1. Collect evidence (owner question via `ticket task BRL-ptaq question add` if no field data exists).
2. Prototype-free cost estimate per component above; optionally a throwaway spike measuring WebSocket HOL latency with `lightyear_websocket` in a scratch copy.
3. Record the decision in docs/14 near line 128 (replace the "not part of this contract" sentence with the decision + rationale).
4. If adopted, file an implementation follow-up ticket; do not implement here.

# Acceptance
- An architecture decision recorded in docs/14 (adopt or reject with evidence); implementation, if adopted, in a follow-up ticket.
- Docs-only change: `just lint` still passes.


# Result (2026-11-18)

**Decision: defer** the non-UDP fallback, with a trigger. Recorded in docs/14 §Network-change continuity, replacing "A non-UDP fallback transport is not part of this contract." Current protocol is 50 (the context above says 49).

## Evidence
- No field reports of UDP-blocking networks. Searches covered "no reply", "UDP", "blocked", "firewall", "WebSocket", "hotel", "school" and "cellular".
- The no-reply hint has been seen 4 times, all on the owner's LAN, and none was a UDP block. BRL-g57a: the first Retry about 15 s after Wi-Fi returned; the second Retry connected. BRL-2nsg: the Mac hibernated, the phone was on another subnet, and the BRL-d3hb handoff timeout. BRL-sh10: a scratch server on :5001.
- Owner question q-mncb6fv (non-blocking, default = defer).

## Cost estimate (no prototype)
| Component | Estimate | Notes |
|---|---|---|
| Supervisor TCP/TLS ingress | 1-1.5 wk | A second Mio listener beside `PUBLIC_TOKEN` in runtime/mod.rs. It needs per-connection read/write buffers, backpressure, accept/conn limits and slowloris bounds. TLS (rustls) and certificate provisioning are needed for public 443. Mio is non-blocking, so TLS goes on a state machine, not tokio. |
| Stream envelope | 3-5 d | A length-prefixed frame carrying the same selector and payload as `PublicEnvelope`, decoded into the existing per-route IPC path. The ≤`INNER_MAX_DATAGRAM_BYTES` bound is kept. |
| Capability, rebind and rejoin | 1 wk | The connection is the source identity. A TCP reconnect after a network change is a new connection presenting the capability, so `CapabilityRebind` (BRL-sqz0 grace, 2 changes per 10 s) needs a connection-keyed variant. Lobby routes are keyed by `SocketAddr` (ingress.rs) and need a connection key. BRL-yyeg rejoin and SeatReleased are unchanged above the route, but they need tests over the stream path. |
| Client selection, order and timeouts | 3-5 d | A new `RoutedStreamIo` beside `RoutedUdpIo`, because the client does not use Lightyear's UdpIo. Order: UDP first, then WSS only after a no-reply. That adds the stream attempt timeout to every genuine "server down" failure. The choice is kept for the session; a route switch mid-match is not allowed. |
| Android | 2-3 d | INTERNET permission is already present. Native rustls roots (rustls-native-certs is not available on Android, so it needs webpki-roots or platform verifier). Network-change behavior differs: a TCP connection dies on an address change, so every Wi-Fi/cellular switch is a reconnect, never the UDP silent rebind. |
| Head-of-line under loss | analysis | Each lost segment stalls every later snapshot and input for ≥1 RTT (fast retransmit), or for the RTO (≥200 ms) when fewer than 3 segments follow. At 1-2% loss that is a visible stall a few times a minute. It is acceptable only as a degraded path. |
| **Total** | **~3-5 wk** | Plus a second transport in every network e2e. |

## Lightyear transports (read in ~/.cargo/registry/src, 0.29.0 present)
- `lightyear_websocket`: about 360 lines of glue over `aeronet_websocket` 0.21 (tokio rt-multi-thread, tokio-tungstenite, rustls). It is a Bevy `Server` / `Link` component. It cannot serve the Mio supervisor, which terminates ingress outside Bevy and forwards route-enveloped datagrams to workers. On the client it would replace `RoutedUdpIo`, so the routed envelope would have to be reimplemented anyway. It is useful as reference only.
- `lightyear_webtransport`: `aeronet_webtransport`, then wtransport 0.6, then quinn. That is QUIC over UDP, so it **does not help on UDP-blocking networks**. Ruled out.

## HOL spike
Not run. It is optional. A meaningful spike would build tokio, tungstenite and rustls plus a loss shaper while an APK build was running in this checkout, and TCP HOL behavior is well characterized. The analytic bound above stands in for it.

## Trigger to reopen
A player outside a development LAN reports the no-reply hint persisting on one network while another network works, or a launch targets school, workplace or hotel networks. The fallback would then be WSS on 443, tried after a UDP no-reply. No follow-up ticket was filed because the decision is not to adopt.

## Verification
- `just lint`: exit 0 (log target/ptaq/lint.log). The change is docs-only.
