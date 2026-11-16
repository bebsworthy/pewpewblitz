# Found
BRL-2nsg host matrix (2026-11-16): a UDP proxy delayed supervisor-to-client traffic.
- 300 ms: connects in 0.83 s.
- 800 ms: never connects. Every retry, each on a fresh port, logs `ingress dropped reason=source-limited`, because 8 Netcode connection requests at 10 Hz use the whole 8-datagram / 9 KiB pre-auth window (`packages/brawler-routing/src/limits.rs` L65-69, `ingress.rs` `SourceIngressLimiter`) before the challenge-response is sent.

A lobby round trip of about 700 ms or more therefore makes connecting impossible, for example on a poor mobile link or a distant server.

# Plan
Make the budget cover a full Netcode handshake at realistic worst-case latency. Options: count only the first N connection requests per token, give challenge responses their own allowance, or size the window to 10 Hz × timeout. Keep the abuse protection. Add a routing runtime test with a delayed reply.

# Acceptance
A client with a lobby round trip of up to 1.5 s connects (proxy test). The existing ingress-abuse tests still pass.
