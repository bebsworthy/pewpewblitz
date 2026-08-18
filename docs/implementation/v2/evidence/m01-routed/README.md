# M01 routed verification evidence

Recorded 2026-08-18 through 2026-08-19 on the Apple M3 reference host. These are development-use
verification facts, not a production-readiness claim; the active milestone and M09 backlog retain
the deferred hardening measurements and soak cardinalities.

## Passing commands

- `just network-routed-evidence 1 90 both`
- `just network-routed-evidence 1 90 crash-restart`
- `just network-routed-ipv6-smoke` (production routed IPv6 loopback smoke; run on a host with IPv6 loopback enabled)
- `cargo test -p brawler-routing --all-targets -- --test-threads=1`
- `cargo test --no-default-features --features network-test --test routed-process -- --test-threads=1`
- `cargo test --no-default-features --features network-test --test network -- --test-threads=1`
- client/server/routing Clippy with `-D warnings`, `just check`, server feature audit, formatting, and
  diff hygiene
- `env CARGO_INCREMENTAL=0 just network-routed-smoke` on 2026-08-19 after the final shutdown
  sequence/fingerprint fixes; both clients handed off, the authoritative match emitted Result and
  Exit, and match plus lobby workers stopped/reaped/cleaned gracefully
- `env CARGO_INCREMENTAL=0 just verify` on 2026-08-19: formatting and all role Clippy lanes; 95
  routing, 270 client, 261 server, 77 network integration, and 14 performance tests; server feature
  isolation; then another clean two-client routed Result/Exit/reap smoke

## Observed both-mode facts

- Wipeout and Hot Zone each completed one production lobby→match→fresh-lobby cycle.
- Each cycle had exactly one dynamic match worker and the ordered facts
  Spawned→Ready→ResultReceived→successful reap→graceful stop→cleanup.
- Owner-boundary diagnostic samples: 12,228 public-receive→packet-IPC-enqueue and 12,335
  worker-packet→public-send; maximum per-cycle p95 was 64 µs. This excludes IPC transit and worker
  decode and therefore is not evidence for the 2,000 µs routed-IPC hard gate.
- Maximum RSS: supervisor 6,624 KiB, lobby 30,512 KiB, match 42,400 KiB, all below their selected
  32/45/50 MiB limits.
- Terminal state: zero workers, process workers, routes, live capabilities, packet/control queued
  frames and bytes, source-limit drops, forced stops, leftover processes, and private socket files.
- Public-envelope bytes passed the exact `public = inner + 42 × datagrams` validation. Directional
  IPC counters mix packet and variable control frames, so packet-only IPC overhead remains
  unsupported. Expected in-flight packets rejected after the two deliberate capability
  revocations remained within the bounded terminal allowance.

## Observed crash/restart facts

- Killing one production Bevy match worker preserved its sibling match and cleaned only the failed
  worker's ownership.
- Killing the production lobby exercised bounded restart and exact final cleanup.
- Both tests asserted zero children, routes, live capabilities, queued bytes, and runtime socket
  files at terminal cleanup.

## Deferred production hardening

- full public↔worker IPC latency and packet-only IPC overhead;
- a full correlated graceful-stop/reap duration campaign;
- the paired one-run raw CPU and match-only bandwidth totals are diagnostic only because the
  launchers lack correlated common observation-window checkpoints; the three-pair gate remains
  unrun;
- IPv4/IPv6 packet capture proving no fragmentation. The optional macOS command is
  `just network-routed-capture capture=target/routed-capture.pcap`; it runs `tcpdump` on `lo0` and
  runs both `127.0.0.1` and `::1`, then invokes `scripts/verify-routed-capture.py`. BPF capture permission may require an approved
  administrator capture session. The verifier exits unsupported on an empty, malformed, or
  unavailable capture, so this item remains open until a real pcap passes with IPv4/IPv6 UDP
  payloads no larger than 1,200 bytes and no IPv4 fragmentation or IPv6 Fragment header.
- paired fixed-tick regression and correlated allocation→Connected p95;
- the specified 25 normal cycles per mode and 20 crash/restart cycles;
- the user windowed playtest and feedback/learning closeout.
