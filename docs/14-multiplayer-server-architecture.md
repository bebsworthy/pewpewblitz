# Multi-process server and single-port UDP/IPC transport

## Decision status

Accepted for V2 implementation on 2026-08-18 and completed on 2026-08-20. The V2 roadmap and
milestones own the exact IPC primitives, framing, limits, process evidence, and closeout record.
V3 retained this topology unchanged while replacing client gameplay-world presentation.

## Context

V2 requires one logical server to run multiple matches concurrently. The v1 server is one Bevy
`App`/`World` whose mutable resources, installed game mode, Avian physics state, Lightyear server,
map, terrain, and lifecycle describe one authoritative match. Hosting many matches in that world
would require pervasive match scoping and would leave every match in one failure boundary.

Several independently ticked Bevy apps in one process would isolate worlds but add custom
scheduling and task-pool coordination without crash or memory isolation. A process per match reuses
the existing one-world composition, lets the operating system schedule across cores, and gives each
match a clear resource and failure boundary.

A public port per match is the simplest process topology, but expands firewall, container,
configuration, and client handoff concerns. V2 instead targets one stable public UDP endpoint, with
local IPC between a thin router and its workers.

## Decision

One logical Brawler server consists of:

1. One **supervisor/router process** owning the public UDP socket, routing capabilities, bounded
   route and worker registries, host admission, child handles, and shutdown coordination.
2. One long-lived **lobby worker process** owning a headless Bevy `App`/`World`, Lightyear
   authority for game types, sessions, profiles, queues, and reservations, plus the exclusive
   bounded SQLite profile-storage executor—not match simulation. It
   requests host admission and match-worker allocation from the supervisor. Embedding the lobby in
   the supervisor is a deviation that must return to specification review with measured evidence;
   it is not the default topology.
3. One **worker process per active match**. Each worker owns one headless Bevy `App`/`World`, one
   Lightyear server, one mode/map/physics instance, authoritative gameplay, replication,
   recovery, telemetry, and terminal result.
4. One **public UDP endpoint**. The supervisor routes opaque Lightyear packets between clients and
   lobby/match authority over dedicated framed IPC.
5. Separate **bounded control IPC** for manifest, readiness, heartbeat, result, stop, and failure.
   Standard streams may bootstrap control, but gameplay data never shares a stream with logs; logs
   use standard error.

The supervisor is infrastructure authority, not lobby or gameplay authority. It must not host a
Bevy lobby/match authority, deserialize lobby or combat messages, mirror ECS state, calculate
replication, or become an alternate application path. Keeping the lobby out of this process limits
a lobby/Lightyear/Bevy failure to admission and queue service instead of also crashing the router
and every already-running match. The supervisor remains a v2 logical-server failure point.

```text
Client UDP
    |
    v
Supervisor/router — one public socket
    | route capability -> authority + peer
    |
    +-- framed packet IPC --> lobby worker Lightyear Link --> authoritative lobby World
    |
    +-- framed packet IPC --> match worker A Lightyear Link --> authoritative World A
    |
    +-- framed packet IPC --> match worker B Lightyear Link --> authoritative World B
```

## Connection and handoff model

The lobby and a match worker are independent Lightyear/Netcode authorities. An established lobby
connection is not migrated between processes:

1. The client establishes a lobby Lightyear connection through the public endpoint.
2. Queue formation reserves players; the lobby requests host admission and match-worker allocation
   from the supervisor.
3. The worker validates its immutable match manifest and reports `Ready`.
4. The supervisor mints a short-lived opaque routing capability from the validated lobby allocation
   request, bound to the reservation, intended authority, protocol/content identity, and expiry;
   the lobby delivers it to the participant over the authenticated lobby connection.
5. The client closes the lobby connection and establishes a fresh match connection to the same
   public address using that capability.
6. The router validates and strips the outer route envelope, then forwards the still-opaque
   Lightyear datagram to the assigned worker.
7. On completion, leave, or failure, the client closes the match connection and establishes a fresh
   lobby connection. V2 does not promise match-session resumption.

The capability is not a displayable or predictable `MatchId`. It is a high-entropy, expiring
**routing authorization** used only by the supervisor to select the intended lobby or match
authority. The router strips it before forwarding and never treats it as player/gameplay identity.
The receiving Lightyear/Netcode authority still performs its normal connection authentication,
reliability, fragmentation, protocol, and replication behavior.

M01 retains Brawler's existing development manual Netcode credentials and additionally admits a
match connection only when its authenticated Netcode client ID is present in the immutable match
manifest. The routing grant therefore carries no second Netcode token. Replacing development
credentials with production-issued credentials is security hardening outside M01; the route
capability never substitutes for the manifest/client-ID admission check.

The manifest carries that nonzero Netcode client ID separately from the supervisor-minted routed
`PeerId`; neither identity is derived from or substituted for the other.

Possession is nevertheless security-relevant: a stolen capability can direct traffic toward its
assigned authority and consume bounded router, IPC, and worker work even when inner authentication
later fails. Capabilities therefore require scope, expiry, revocation, replay policy, secrecy, and
rate limits. Compromise of both routing and Netcode credentials is outside the protection offered
by this separation.

## Routed packet transport

Lightyear 0.29 exposes `Link` as its transport-neutral boundary: concrete IO pushes opaque received
payloads into `LinkReceiver` and drains opaque outbound payloads from `LinkSender`. Its UDP server
binds one socket and creates child links keyed by remote address. Its Crossbeam transport
demonstrates the same integration for in-process channels but cannot cross processes.

V2 will implement a Brawler-owned routed transport at that boundary. It will not fork Lightyear or
wrap gameplay replication in a second snapshot format.

Conceptual external envelope:

```text
magic | envelope_version | route_capability | opaque_lightyear_datagram
```

Conceptual internal packet frame:

```text
frame_version | direction | route_id | peer_id | payload_length | opaque_lightyear_datagram
```

The specification must define fixed endianness, maximum sizes, unknown-version behavior, route
lifecycle, peer identity, and malformed-frame handling. The envelope reduces effective Lightyear
MTU; the transport must advertise or enforce a measured safe payload size.

Routing solely by external source address may be a comparison/test mode, not the target contract:
NAT rebinding and client socket recreation can change that address. An explicit capability envelope
keeps routing independent of those changes.

## IPC and process contract

Gameplay data requires a dedicated binary, bidirectional, framed channel. The logical contract is
one versioned frame codec with explicit maximum sizes and validation independent of the selected
backend. Message/packet transports preserve one encoded frame per record; byte-stream transports
add an explicit fixed-endian frame-length prefix and must handle partial reads/writes. Backend
record boundaries never replace validation of the frame's own lengths.

Route-envelope, packet-frame, control-frame, and process-manifest versions are decoded by the
supervisor and workers before or below the Lightyear application handshake. They therefore retain
independent framing/schema versions and fail closed on unknown versions. This is distinct from
application-message evolution: the enduring one-current-schema policy, global compatibility
handshake, and no-per-message-version rule are defined in
[Network architecture](./08-network-architecture.md#application-protocol-compatibility-and-evolution).

M01 will compare:

- Unix-domain socket on macOS/Linux, preferring packet/message semantics where reliable;
- Windows named pipe when Windows implementation enters scope, using the same logical frame codec
  whether the selected pipe API exposes byte- or message-oriented behavior;
- framed loopback TCP as a portable fallback if it materially reduces risk.

Unix FIFOs and standard input/output are byte streams and do not preserve packet boundaries. They
are acceptable for bounded control only with explicit framing and independent logs. Gameplay
packets are never newline-delimited.

Every queue is bounded. Backpressure behavior distinguishes transient pressure, deliberate
UDP-style packet loss, control failure, and a dead worker. One worker cannot cause unbounded router
memory or starve unrelated matches.

The initial control lifecycle is:

```text
Spawned -> ManifestSent -> Ready -> Running -> Draining -> Exited
                         \-> Failed
```

The immutable match manifest contains match/game-type identity, protocol/content fingerprints,
mode, map/seed, rules, topology, accepted participants and opaque immutable V3 loadout snapshots,
route identities, and declared limits. It contains no account ID, profile cache, or database path.
The lobby manifest instead contains logical-server identity, its lobby-only profile database path,
game-type catalog/configuration fingerprints, default-route identity, declared limits, and restart/reconciliation inputs. Each worker
validates its role-specific manifest completely before reporting readiness or accepting a client.

## Authority and ownership boundaries

| Concern | Owner |
|---|---|
| Public UDP socket and route envelope | Supervisor/router |
| Route capability creation, validation, expiry, revocation | Supervisor from validated lobby allocation input |
| Game-type catalog, queue tickets, formation, reservation | Lobby authority |
| Host worker count and resource admission | Supervisor |
| Match manifest validation | Worker |
| Match ECS, physics, map, terrain, modes, combat, results | Worker authoritative world |
| Lightyear connection and replication state | Receiving lobby or worker authority |
| UI flow and reconnect orchestration | Client presentation/session code |

Stable IDs cross process boundaries; Bevy `Entity` values, pointers, resources, and process-local
handles never do.

## Failure and security requirements

- Reject malformed, oversized, unknown-version, expired, revoked, replayed where prohibited, or
  unauthorized envelopes before allocating unbounded state.
- Rate-limit default-route traffic and capability failures separately from established routes.
- Bound routes, routes per peer/session, workers, pending spawns, packet queues, control frames, and
  retained results.
- Detect worker exit/heartbeat failure, revoke its routes once, release capacity, and notify the
  lobby of affected reservations/matches.
- Detect lobby-worker failure, reject new default-route admission, preserve bounded supervision of
  active match workers, and follow an explicit restart/reconciliation policy researched by M01.
- A worker crash may terminate its match but not another worker. Supervisor failure remains a
  logical-server outage and is not hidden by v2.
- Graceful shutdown stops admission, drains or terminates matches under a deadline, closes routes,
  and reaps every child.
- IPC endpoints and inherited handles are private to the supervisor/worker relationship.
- Metrics cover packets, routes, queue depth/drop, handoff, worker start/exit, CPU, memory, and fixed
  tick without logging capability secrets.

## Reusable foundation, not a throwaway spike

V2 M01 is a spike because it retires architectural risk early. Its planned artifacts are production
foundations once the M01 specification is validated:

- versioned envelope/frame types with parser and validation tests;
- supervisor route table and bounded queues;
- reusable process supervision plus role-specific lobby and match-worker lifecycle/manifest
  contracts;
- Lightyear routed-IPC transport plugin and peer/link lifecycle;
- client routing adapter and sequential lobby/match reconnect seam;
- deterministic in-memory test backend plus real cross-process backend;
- process harness, impairment controls, metrics, and crash/cleanup fixtures.

These components live behind intentional APIs used by the production supervisor, client, lobby
worker, match worker, and integration tests. Lobby and match workers reuse framing, supervision,
diagnostics, and handle hygiene, but keep distinct role manifests and lifecycle policies. M01 may
begin with one lobby and one match worker, but it must not create a separate demo binary, protocol,
or mock simulation that later requires replacement.

## Superseded direction

The earlier `13-player-ux.md` draft proposed multiple match instances inside one server process and
depended on coarse Lightyear room filtering (`GAP-NET-ROOMS`). This decision supersedes that
direction: each match now owns a process, Bevy world, Lightyear authority, and route. Cross-match
interest filtering is therefore unnecessary; ordinary per-client visibility within one match
remains a separate future optimization only if measured gameplay scale justifies it.

## Alternatives rejected for v2

### One Bevy world containing every match

Requires pervasive scoping of resources, queries, messages, physics, terrain, recovery, telemetry,
mode installation, and cleanup while preserving a shared crash boundary.

### Multiple Bevy apps inside one process

Requires custom ticking, task-pool/resource coordination, and routing while retaining a common
crash and memory boundary.

### One public port per worker

Retained as the contingency if routed transport evidence fails. It reuses Lightyear UDP directly
but exposes port-range, firewall, orchestration, and endpoint-handoff complexity.

Single-port routing remains the accepted v2 product choice because it gives direct-connect clients
one stable address, hides worker allocation, reduces exposed configuration/container ports, and
keeps lobby/match reconnection at the same endpoint. Before M01 specification validation, research
must still cost a bounded worker-port-range reference design: port allocation/reuse, client handoff,
firewall/container configuration, cleanup, and the custom transport work it avoids. This is a
decision input and contingency definition, not permission to implement and maintain two production
transports in parallel.

### Supervisor-owned decoded replication gateway

Rejected because it would duplicate Lightyear connection/replication responsibilities and weaken
the worker authority boundary.

### Sharing one UDP socket among workers

Rejected because descriptor sharing or port-reuse hashing cannot route according to lobby-owned
match assignments portably or deterministically.

## Consequences

Benefits include match crash/state isolation, reuse of the current server, one stable external
endpoint, OS scheduling across cores, and explicit host admission/measurement.

Costs include a Brawler-owned transport/router below Lightyear, reconnect-based loading transitions,
extra packet copies/queues/latency, a shared supervisor ingress failure point, and per-process
Bevy/Lightyear/catalog memory overhead.

## Evidence and validation

Version-pinned local evidence:

- `references/lightyear/book/src/SUMMARY.md` and its transport/connection sections;
- Lightyear 0.29 `lightyear_link/src/lib.rs` for the `Link` boundary;
- Lightyear 0.29 `lightyear_udp/src/server.rs` for one-socket, per-peer links;
- Lightyear 0.29 `lightyear_crossbeam/src/lib.rs` for an in-process transport example;
- Lightyear 0.29 `lightyear_connection/src/network_topology.rs` for conventional topology limits;
- Lightyear 0.29 `lightyear_netcode/src/packet.rs` and `token.rs` for encrypted connect-token data
  and handshake packet structure;
- `tests/network/harness.rs` and current server composition as reuse targets.

Current primary cross-checks: [Lightyear 0.29](https://docs.rs/lightyear/0.29.0/lightyear/) and
[Rust process IO](https://doc.rust-lang.org/std/process/struct.Stdio.html).

Before M01 can leave `Researching`, it sets thresholds from the M11 direct-UDP baseline and records
the costed worker-port-range reference design. Implementation then measures packet overhead/loss,
IPC latency/queues, worker startup and memory, fixed-tick behavior, handoff time, crash cleanup, and
two-worker isolation. A failed hard threshold after bounded optimization returns the bounded
worker-port-range contingency to specification review; it does not silently replace this decision.
