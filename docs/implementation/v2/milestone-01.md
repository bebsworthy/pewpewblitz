# Milestone 01 — Reusable routed multi-process server foundation

## Tracking

| Field | Value |
|---|---|
| Version | v2 — player UX and server-local matchmaking |
| Roadmap | [roadmap.md](./roadmap.md) |
| Architecture | [Multi-process server and single-port UDP/IPC transport](../../14-multiplayer-server-architecture.md) |
| Status | Researching |
| Research | Initial topology, current server/harness, and exact Lightyear 0.29 transport-source findings recorded 2026-08-17; the milestone remains in research until framing, IPC backend, route handshake, lobby lifecycle, module boundaries, baselines, thresholds, and the port-range reference cost are resolved |
| Entry artifacts | Complete 2026-08-18: worker-readiness audit delivered in [V1 M11](../v1/milestone-11.md#v2-worker-readiness-handoff), [direct-UDP baseline](../v1/evidence/v2-baseline/README.md) delivered, and final Slice 7 reconciliation found no v2 blocker |
| Specification validation | Pending; this research draft does not authorize production implementation |
| Implementation | Not started |
| Verification | Not started |
| User playtest | Pending definition of the observable connection-handoff check |

## Outcome

Brawler gains a production-reusable foundation for one logical server to expose one public UDP
endpoint and run isolated authoritative match processes. A thin supervisor routes opaque Lightyear
datagrams over bounded IPC, starts and reaps workers through a versioned process contract, and
supports a client moving from a lobby connection to a fresh worker connection at the same address.

This is an architectural spike only because it retires risk before matchmaking depends on it. The
planned route types, transport plugin, process supervisor, lobby/match worker entry paths, tests,
metrics, and failure handling become production foundations after specification validation. There
is no throwaway protocol, fake server architecture, or second gameplay implementation.

## V1 M11 entry artifacts

M01 consumes, but does not ask M11 to implement, the worker foundation. The worker-readiness audit
is recorded in [V1 M11](../v1/milestone-11.md#v2-worker-readiness-handoff), and the reproducible
[direct-UDP baseline](../v1/evidence/v2-baseline/README.md) is available now. Together they contain:

- the exact dedicated-server composition, executable/feature, startup inputs, endpoint readiness,
  result/report outputs, exit codes, and shutdown sequence;
- the audit of process-global recorders/hooks, background work, endpoints/handles, output paths, and
  terminal cleanup assumptions;
- the reproducible direct-UDP comparison scenario and its cold readiness, idle/loaded resident
  memory, fixed-tick, transport-byte, entity/link, report-size, stop-to-exit, and cleanup evidence.

M11 Slice 7 closed on 2026-08-18 after the user accepted the basic v1 MVP. Its final reconciliation
confirmed these delivered artifacts and found no blocker to M01 research or specification review;
release-quality gameplay polish remains a separate pre-release backlog item.

M01 reruns that same gameplay scenario through the routed UDP/IPC path. It owns the comparison,
numeric overhead thresholds, worker manifest, transport adapter, process lifecycle, and every
multi-worker assertion. Missing M11 evidence delays threshold-setting; it does not authorize M01 to
invent or retroactively modify the v1 result.

## Scope boundary

### In scope

- one public UDP socket owned by a supervisor/router;
- versioned outer route envelope and internal framed packet/control contracts;
- high-entropy bounded route capabilities for a lobby-to-worker reconnect experiment;
- route registry mapping validated capabilities/peers to lobby or worker authority;
- Lightyear 0.29 transport plugin moving opaque payloads between worker `Link` entities and IPC;
- a client routed-UDP adapter integrated into the production `client/session.rs` connection path;
  focused tests may substitute the backend, but not create a test-only connection architecture;
- role-specific immutable lobby/match manifests and spawn, ready, heartbeat, stop, result, failure,
  and exit messages over shared framing/supervision machinery;
- a minimal isolated lobby authority that accepts inner Lightyear/Netcode-authenticated
  default-route sessions, requests one bounded worker allocation through the M01 transition driver,
  and delivers supervisor-minted route capabilities; complete game-type advertisement, queueing,
  formation, results, and requeue remain owned by M03–M06;
- one production match-worker composition reusing existing `brawler-server` authoritative plugins;
- deterministic in-memory backend plus one real cross-process IPC backend on macOS;
- bounded queues, frame limits, malformed-input handling, rate/admission limits, route revocation,
  and worker crash cleanup;
- measurements for packet bytes/copies, route/IPC latency, queues/drops, startup, memory, fixed tick,
  and cleanup;
- one isolated lobby worker/default route, one match route, two-client traffic, and a two-match-worker
  isolation fixture.

### Out of scope

- product menus, favorites, recents, settings UI, and complete recoverable presentation;
- complete game-type advertisement, build editor, queue pools, formation, results, or requeue;
- global matchmaking, registry, accounts, parties, NAT traversal, relays, service APIs, or fleet
  orchestration;
- live Lightyear/Netcode connection or match-state migration between processes;
- match resumption or join-in-progress;
- supervisor-side combat decoding, ECS mirroring, snapshots, or gameplay authority;
- production Windows IPC while retaining a contract implementable with named pipes later;
- a general distributed runtime or actor framework.

## Confirmed research findings

1. The v1 server is structurally one match world. Installed mode, lifecycle, map, terrain, physics,
   combat, and mutable resources do not support independent matches in one `World` without a
   pervasive rewrite.
2. Worker processes preserve that composition and add crash/memory isolation. Multiple Bevy apps in
   one process retain routing and lifecycle complexity without that isolation.
3. Lightyear 0.29 `Link` is transport-neutral. IO pushes mutable receive payloads into its receive
   buffer and drains opaque outbound bytes from its send buffer.
4. Lightyear's UDP server owns one socket and creates a child `Link` per remote `SocketAddr`.
   Replacement IO must preserve per-peer identity in IPC frames; raw bytes alone are insufficient.
5. Lightyear Crossbeam is a useful plugin/schedule example, but its channels are in-process. It is
   evidence for the adapter seam, not the production backend.
6. Lightyear conventional topology rejects multiple connected conventional clients or multiple
   started servers in one app. The client should close lobby and establish a fresh worker session.
7. An established lobby Netcode session cannot be transferred to an independently initialized
   worker. The router must direct a new worker handshake using an explicit routing capability.
8. Standard input is one-way, and standard streams/FIFOs do not preserve datagram boundaries.
   Framed control may use process streams; packet data needs an independent binary channel.
9. The route envelope consumes MTU. The implementation must lower the inner MTU or prove another
   safe bound instead of permitting accidental fragmentation.
10. The existing deterministic Crossbeam harness already manipulates Lightyear `Link` packet queues
    and impairment. M01 should extract/reuse that capability rather than create unrelated tests.
11. The routing capability authorizes only router selection of an authority. It is stripped before
    forwarding and does not replace inner Lightyear/Netcode authentication, but possession can
    consume bounded router/IPC/worker resources and therefore remains security-sensitive.
12. Netcode connect-token user data is protected for the receiving authority, and later packets
    still need stable route selection. Making the router parse or decrypt Netcode would couple it to
    inner protocol/security behavior, so the outer envelope remains the accepted default.
13. Queue/reservation authority and routing authority stay distinct: the lobby requests an
    allocation and delivers the result over its authenticated session; the supervisor mints,
    activates, validates, and revokes the corresponding routing capability against its route table.

## Research questions before specification review

### Transport and framing

- What exact client hook adds/removes the route envelope without replacing Lightyear connection,
  reliability, fragmentation, or replication behavior?
- Should worker IO multiplex peers over one endpoint and create `LinkOf` children, or use one local
  stream/link per peer?
- Which `LinkSystems`/`LinkReceiveSystems` ordering and deferred-command boundaries are required for
  peer creation, receive buffering, send flushing, unlink, and reconnect?
- What header, endianness, size, integrity boundary, and effective MTU are safe for supported
  IPv4/IPv6 paths?
- Confirm that Lightyear/Netcode exposes no stable, opaque, pre-auth route discriminator usable for
  the entire session without parsing/decrypting inner protocol packets. Any native hint may only
  simplify post-auth bookkeeping; replacing the accepted outer envelope requires specification
  review and proof that the router remains transport-opaque.

### IPC backend and backpressure

- Define one logical frame codec and maximum record size. Compare Unix-domain
  datagram/sequence-packet/stream sockets and framed loopback TCP on Rust/macOS for record mapping,
  peer credentials, nonblocking integration, queue limits, portability, and shutdown; byte streams
  use an explicit fixed-endian frame-length prefix and partial-read/write state.
- Which capacities and per-route scheduling prevent one stalled worker from starving lobby or other
  workers?
- Under pressure, what may drop with UDP semantics, and when must the transport unlink rather than
  reorder or retry indefinitely?
- How many copies/context switches occur per direction, and what pooling is justified by evidence?

### Process lifecycle and ownership

- What owns the supervisor event loop and scheduling: a plain Rust process, a minimal
  infrastructure-only Bevy `App`, or another bounded runtime? Identify which threads/tasks poll
  public UDP, packet IPC, control IPC, child status, timers, and shutdown signals; compare fairness,
  wake-up behavior, testability, dependency/feature isolation, and cleanup. The choice must not
  introduce a supervisor gameplay `World` or client/gameplay dependencies.
- What role-specific manifest, restart/reconciliation, readiness, and shutdown policy does the
  default isolated lobby worker require beyond process supervision and framing shared with match
  workers? Embedding it in the supervisor requires evidence and a return to specification review.
- What executable/subcommand/Cargo-feature composition reuses the server without giving the
  supervisor gameplay or client-presentation dependencies unnecessarily?
- How is the manifest transferred atomically, validated before `Ready`, and kept out of command-line
  and log leakage?
- What heartbeat/shutdown deadlines distinguish a slow tick, backpressure, deadlock, crash, and
  normal completion?
- What child-reaping and inherited-handle rules work on macOS and preserve a Windows path?

### Security and capacity

- Define routing-capability entropy, scope/binding, expiry, reuse, revocation, rotation, replay, and
  secrecy behavior without treating it as player identity or expanding into an account-authentication
  service. Bound the work possible with a stolen routing capability before inner authentication.
- Define pre-allocation limits for datagrams, capabilities per source/session, routes, pending
  workers, workers, IPC bytes, and control frames.
- Establish provisional worker CPU, memory, tick, startup, and bandwidth budgets from the M11
  baseline, then measure router overhead separately.

## Reuse and module-boundary requirements

Research may change names, but the specification must identify focused ownership equivalent to:

```text
routed protocol
  envelope/frame/control/manifest types, validation, stable IDs

client routed transport
  public UDP adapter, route selection, Lightyear Link integration

supervisor/router
  ingress, route table, bounded queues, worker registry, admission

long-lived lobby worker
  lobby Lightyear authority, sessions, game-type catalog, queue/reservation state

worker IPC transport
  IPC backend, per-peer Lightyear Link lifecycle, send/receive systems

process supervision
  spawn, readiness, heartbeat, stop, exit/reap, diagnostics

test support
  deterministic memory backend, impairment, process harness, measurements
```

The public API is the smallest surface required by production composition and integration tests.
Cross-executable protocol types may justify a module or crate, but research must prove any crate and
feature boundary before creating it. Gameplay ECS types do not migrate into the router protocol.

Memory and real IPC backends implement the same packet/control semantics. Tests do not bypass route
validation or acquire gameplay authority merely because a focused case avoids an OS process.

The match-worker entry evolves the composition currently reached through `src/bin/server.rs` and
`server::build_app_with_config`; it does not copy the gameplay plugin graph into a second server.
After the minimum routed transition driver is validated, `network.sh` and `just network` default to
the routed supervisor path. The v1 direct-UDP executable remains only behind an explicitly named
compatibility/baseline command while M01 comparisons require it; M09 reviews its removal after the
final comparison evidence. New gameplay composition must be shared and drift-tested. The lobby
worker is a distinct role over the same supervision/framing foundation, not a mode of the match
simulation.

## Provisional lifecycle

```text
bind public endpoint
  -> start and validate isolated lobby worker
  -> accept default-route lobby handshake
  -> allocate worker and send manifest
  -> wait for validated Ready
  -> activate participant route capabilities
  -> client closes lobby and reconnects to worker at the same endpoint
  -> route opaque packets until result, leave, or failure
  -> revoke routes and stop/reap worker
  -> client establishes fresh lobby connection
```

No worker route is usable before readiness. No result becomes lobby truth until its frame validates
against the active worker/match identity. Duplicate exit, stop, revoke, and result are idempotent.

## Planned production slices

These are provisional until specification validation. Every slice extends one production path.

1. **Baseline and API proof:** import and reproduce the M11 direct-UDP worker baseline; build the
   smallest production-path `Link` adapter test; decide module/crate and schedule boundaries; cost
   the bounded worker-port-range reference design; set numeric routed overhead thresholds for
   specification review without implementing a second production transport.
2. **Versioned bounded protocol:** implement and fuzz/property-test envelopes, frames, manifests,
   controls, capabilities, and errors with size/version limits.
3. **Deterministic routed backend:** implement route registry, bounded queues, memory transport,
   peer/link lifecycle, impairment, and schedule tests.
4. **Real process backend:** implement selected macOS IPC, spawn/readiness/stop/reap, handle hygiene,
   logs, crash detection, and cleanup.
5. **UDP router and client adapter:** own one socket, wrap/validate envelopes, preserve peer identity,
   integrate Lightyear ordering, and enforce effective MTU.
6. **Sequential handoff:** drive lobby -> capability -> worker -> lobby connections through the same
   endpoint using production components.
7. **Isolation and capacity:** run two workers, stall/crash one, impair traffic, prove unrelated
   routes, and record overhead against thresholds.
8. **Closeout:** remove temporary diagnostics, lock protocol/commands, document contingency
   evidence, triage feedback, and record lessons.

## Verification plan

### Focused tests

- round-trip versioned frames and reject truncation, trailing/oversized data, unknown versions/types,
  invalid lengths/IDs, and secret-bearing debug output;
- capability creation/expiry/revocation, unknown route, wrong peer/match, duplicate/replayed setup,
  bounded allocation, and bounded unauthenticated work when a valid route capability is presented
  without valid inner Netcode authentication;
- per-route fairness, full queues, drop/requeue policy, worker removal, and idempotent cleanup;
- Lightyear link creation/unlink and send/receive ordering with the memory backend;
- manifest rejection for incompatible fingerprints, topology, mode/map/rules, participants, builds,
  limits, and result identity.

### Cross-process and network tests

- a client connects through public UDP to lobby authority;
- a capability establishes a fresh worker connection at the same address;
- two clients exchange existing authoritative replicated state through a worker;
- two workers have isolated routes and peer identities through one public socket;
- malformed/default-route floods stay bounded and allocate no worker;
- stalled IPC applies specified backpressure without starving unrelated routes;
- worker crash revokes only its routes, releases capacity, reports one failure, and reaps the child;
- lobby-worker crash stops new admission without crashing the supervisor or active match workers,
  then follows the validated restart/reconciliation policy;
- completion produces one result and leaves no child, route, queue, or inherited endpoint;
- latency, loss, duplication, jitter, and reorder profiles reach documented outcomes.

### Measurement matrix

- envelope/frame bytes and effective MTU;
- router/IPC latency distributions, packets, bytes, copies, and drops;
- queue high-water under nominal, burst, and stalled-worker load;
- worker spawn-to-ready and stop-to-reaped duration;
- supervisor and worker resident memory at idle and gameplay load;
- worker fixed-tick duration against the M11 one-dedicated-server-process direct-UDP baseline;
- CPU/bandwidth for lobby plus one and two workers;
- allocation-to-worker-connected handoff duration;
- resource counts after repeated complete and crash cycles.

The specification-review evidence also records the worker-port-range alternative's required port
pool/allocation state, reuse timing, client endpoint handoff, firewall/container configuration,
cleanup behavior, and the custom routed-transport work avoided. This comparison is documented and
costed, not maintained as a parallel implementation.

## Provisional exit criteria

Numeric thresholds must be set from M11 baselines before specification review. M01 cannot exit on
qualitative “works locally” evidence. At minimum:

- one production endpoint routes real Lightyear traffic to lobby and worker over real IPC;
- lobby-to-worker and worker-to-lobby use fresh sequential sessions at the same address;
- existing authoritative gameplay composition runs inside the production worker path;
- two match workers remain route-, process-, ECS-, physics-, replicated gameplay/map/terrain
  recovery-state-, and result-isolated; this does not claim match-session resumption;
- malformed traffic, expired capabilities, full queues, stalled/crashed workers, and shutdown stay
  bounded and clean up deterministically;
- router/IPC overhead, worker memory/startup, handoff, and fixed tick meet validated thresholds;
- client, worker, and supervisor feature graphs preserve role isolation;
- deterministic and process tests use the same protocol and transport interfaces;
- the production client session path and shared authoritative server composition are used; no
  test-only client adapter or divergent match-server plugin graph remains;
- no throwaway server, client, protocol, or mock gameplay path remains;
- canonical commands, limitations, feedback, and learn-from-errors records are complete.

If a hard threshold fails after bounded optimization, return to `Specification review` with the
measured bounded worker-port-range contingency. Do not silently weaken bounds or maintain two
production transports.

## Research log

### Exact-version sources inspected 2026-08-17

- `docs/08-network-architecture.md`
- `docs/13-player-ux.md`
- `docs/implementation/v1/roadmap.md`
- `docs/implementation/v1/milestone-11.md`
- `docs/implementation/v1/evidence/v2-baseline/README.md`
- `src/server/mod.rs`
- `tests/network/harness.rs`
- `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lightyear_link-0.29.0/src/lib.rs`
- `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lightyear_udp-0.29.0/src/server.rs`
- `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lightyear_crossbeam-0.29.0/src/lib.rs`
- `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lightyear_connection-0.29.0/src/network_topology.rs`
- `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lightyear_netcode-0.29.0/src/packet.rs`
- `/Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/lightyear_netcode-0.29.0/src/token.rs`

### Primary cross-checks

- [Lightyear 0.29 API](https://docs.rs/lightyear/0.29.0/lightyear/)
- [Rust `std::process::Stdio`](https://doc.rust-lang.org/std/process/struct.Stdio.html)
- [Rust Unix-domain networking](https://doc.rust-lang.org/std/os/unix/net/index.html)

## Specification validation

Pending. Research must answer the open questions, set numeric thresholds, finalize
plugin/schedule/process composition, and present the complete specification before moving to
`Specification review`.

## Feedback review

Pending.

## Learn-from-errors review

Pending.
