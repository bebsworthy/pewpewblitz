# Outcome
A measured decision on a WebSocket/WebTransport (or TCP) fallback for the routed public endpoint.

# Context (from BRL-g57a, 2026-11-14)
BRL-g57a evaluated and did not add a fallback: the supervisor owns one public UDP socket and the route envelope is a datagram contract. A fallback needs a TCP (and TLS for public servers) listener in the supervisor, a framed stream envelope, a second client transport (lightyear_websocket / lightyear_webtransport exist in 0.29), and head-of-line-blocking measurements. BRL-g57a instead shows an actionable error: no-reply failures say some networks block the UDP traffic the game uses.

# Scope
- Evidence that target players hit UDP-blocked networks.
- Costed design: supervisor ingress, envelope framing, capability handling, client selection/fallback order, Android behavior.

# Acceptance criteria
- An architecture decision recorded in docs/14 (adopt or reject with evidence); implementation, if adopted, in a follow-up ticket.
