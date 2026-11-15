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
