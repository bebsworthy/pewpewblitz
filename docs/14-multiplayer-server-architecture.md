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
   authority for game types, sessions, profiles, the server-derived advertised brawler catalog,
   queues, and reservations, plus the exclusive bounded SQLite profile-storage executor—not match
   simulation. It
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

## Network-change continuity

A phone that changes networks or briefly loses connectivity keeps its existing session rather
than reconnecting. The client transport treats transient socket errors (unreachable network or
host, network down, address unavailable, refused/reset, permission denied, timed out) as UDP
packet loss and keeps its wildcard-bound socket and Lightyear link. When traffic resumes, the OS
selects the new source address and the supervisor's capability rebind (at most two source changes
per 10 s; the newest source receives outbound traffic) follows it, while the match worker keys the
peer by route and peer identity, never by address. A return to an earlier source (a Wi-Fi to
cellular to Wi-Fi flap at the same address and port) is an ordinary rebind counted against the same
limit, once 2 s (`CAPABILITY_RETURN_GRACE_MILLIS`) have passed since that source was replaced.
Inside that grace its datagrams are refused (`Binding`), so late in-flight packets from the old
path cannot flip outbound traffic back. Lobby routes are keyed by source address, so a
lobby session survives a gap at the same address but not an address change.

Routed sessions created by a routed transition use a 10 s Netcode connect-token timeout, equal to
the supervisor's idle revocation of an active capability (`CAPABILITY_IDLE_MILLIS`). Netcode applies
the token timeout on both ends, so a gap shorter than 10 s keeps the same session, fighter, and
route, and a longer gap (including app suspension) is declared lost by every party at about the
same time. A lost match session returns the client to a fresh lobby. If its match still runs, the
client rejoins it automatically (see [Rejoining a running match](#rejoining-a-running-match));
otherwise it reaches Dashboard with an explanation. A non-UDP fallback transport is not part of
this contract.

Security note (BRL-sqz0). Allowing a return does not widen what a capability holder can do. The
256-bit capability, not the source address, authorizes a route; any holder could already move a
route to a fresh address within the rebind limit, so returning to a known one adds no power. A
datagram with a spoofed source address still needs the secret capability, and a holder who
spoofs a victim's earlier address can only redirect bounded outbound match traffic there, at most
two source changes per 10 s, which is the same reflection exposure a new address already had.
Netcode still authenticates every payload, so a redirected route carries no gameplay identity.
Only the newest source receives outbound traffic and keeps the route alive (idle 10 s); a refused
or rate-limited datagram refreshes neither. The supervisor remembers only sources retired inside
the grace, at most two per capability, so the per-capability state no longer grows with the number
of addresses seen. A datagram delayed longer than the grace can still flip the route back and
spend a rebind. The current path is then itself inside the grace, so its datagrams rebind the
route back about 2 s later if the limit allows; otherwise the route waits for the window to roll or
idles out like any lost path.

### Relaunch after the app was killed

A killed app sends no disconnect, so its lobby session would hold the account until the 10 s
bound. The lobby's account credential is the device-held `AccountId` presented in `LobbyHello`, not
the display name. A hello whose `AccountId` is held by another lobby session (accepted or still
loading its profile) therefore replaces that session once the holder has sent nothing for 2 s
(`STALE_LOBBY_SESSION_SILENCE`, twenty missed Netcode keep-alives). The lobby stamps each client
link's last inbound datagram between `LinkSystems::Receive` and `ConnectionSystems::Receive`. Until
the holder reaches the 2 s silence bound, the hello waits. If the hello has waited 2 s while the
holder kept sending, it is rejected with `AccountInUse`: two live copies of the game cannot share an
account. Replacement removes the holder's profile session and records it as a lobby-session loss,
so its queue ticket, reservation, or Practice formation is cancelled exactly as for a disconnect.
The holder's Netcode connection is marked disconnected and left to time out, because Lightyear 0.29
has no public server-side kick. A player in a match holds no lobby session (the lobby connection
closes at handoff), so a relaunch mid-match is admitted to the lobby at once and then rejoins its
held seat once the old match session has been released (below). No wire, manifest, or control-IPC
contract carries the takeover policy itself.

### Rejoining a running match

BRL-yyeg. A routed participant's fighter has a server-owned (`Lifetime::Persistent`) lifetime.
When its match session ends while the match is `Active`, the match worker *holds* the seat for
60 s (`SEAT_HOLD_DURATION_SECONDS`, code-owned session policy): the fighter keeps its identity,
health, position, score credit and team, counts for its team (a lone player's team does not
forfeit), and the server-hosted Practice bot policy drives it through the ordinary authoritative
`ActionState`/`InputFreshness` path with its usual delayed perception. A held fighter has no
Lightyear control relationship: the hold removes `ControlledBy` together with the replicated
`ControlledSend` marker it requires, because Lightyear hides that marker per link only while a
`ControlledBy` names an owner. Otherwise every session that connects during the hold, the
rejoining player's included, would receive the held fighter as `Controlled` (BRL-4zx2). Reclaiming
inserts `ControlledBy` for the rebound link, which shows the marker to that link only. At expiry
the fighter is removed and today's departure/forfeit rules apply; a session lost in Waiting,
loading or countdown still removes the fighter at once.

The account stays in the lobby. At handoff the lobby records, per `AccountId`, the seat's original
grant (request, allocation, match, manifest player) in a bounded table (one seat per account,
`MAX_WORKERS * MAX_PARTICIPANTS` seats), released on `MatchEnded` (sent once when the worker's
Result arrives or the worker ends for any reason) or a terminal refusal. When the same `AccountId` completes a later lobby admission, the lobby sends
`MatchRejoinStatus::Rejoining` before the welcome and a `RejoinRequest` to the supervisor. The
supervisor admits it only for an activated, not-yet-ended allocation of that lobby, at most one
live grant per seat (no live capability on any of the seat's routes, no rebind in flight) and at
most `MAX_REJOINS_PER_SEAT` (3) granted rejoins per seat per match. It mints a fresh 256-bit
capability on a fresh route and peer (30 s pending, 10 s idle as for any match route) and sends
`ParticipantRebind` to the worker, which accepts it only for a held seat of an Active or
Completed match and also enforces the per-seat limit. The worker answers `ParticipantRebound`; only then does the
supervisor register the route, bind the capability and send `RejoinGranted`, which the lobby
forwards as an ordinary `MatchRouteGrant`. Earlier capabilities are never revived. The immutable
manifest is unchanged: the worker keeps a runtime rebind overlay, so the seat is admitted only
under its fresh (Netcode client, routed peer) pair, which lets a relaunched app with a new Netcode
ID rejoin. A rejoin that finds the seat not yet released (`SeatConnected`, the earlier session
still inside its 10 s bound) is retried by the lobby every 500 ms for up to 12 s; every other
failure answers `MatchRejoinStatus::Unavailable` and the client shows today's Dashboard message.
A match that completes while a seat is held keeps that seat rejoinable (BRL-jxfy). The match
worker defers its `Result` while any seat held at completion is still held (each bounded by its
own 60 s hold, which keeps counting in `Completed`) and while a seat reclaimed in `Completed`
keeps its session connected, for at most the 10 s routed session bound after the reclaim. With no
held seat the `Result` goes out on the completion frame, as before; at worst it follows completion
by about 70 s. Because the allocation has not ended, the supervisor and lobby carry the rejoin
unchanged, and the worker admits the rebound held seat in `Completed`: the rejoined client
receives the replicated completed state and shows that match's Results (score, Wipeout stats
board, objective result) through the ordinary completed-match path; the client captures that
result only once its join outcome has arrived, because the completed snapshot can precede it and
the Results need the local player. When the root first becomes
`Completed`, the worker sends `SeatReleased` for every routed seat that is not held (its player
already sees Results or has gone), and later one for a held seat once it is reclaimed or its hold
expires, so each player's next lobby welcome after Results is ordinary. `MatchEnded` still follows
the `Result` once and releases any remaining seat. A match that completes while the lost session
is still inside its 10 s bound (not yet held) still releases the seat, and that player sees
Dashboard.

Leaving on purpose is not a lost session (BRL-8y1f). Netcode reports a requested disconnect and a
timeout identically to the server, so the client states the intent: confirming Leave Match sends
`MatchCommand::LeaveMatch` on the match session and disconnects when the worker answers (at most
2 s later; only a broken link falls back to the hold above). The worker marks the seat left, sends
one `SeatReleased {request, allocation, match, player}` fact to the supervisor, and removes the
fighter when that session ends, in every phase, under today's departure/forfeit rules (a lone
player's team in a two-team match forfeits; Practice teams keep their bots). A left seat is never
held and refuses every rebind (`SeatExpired`). The same fact releases any seat that can no longer
be rejoined after completion (above). The supervisor validates the fact against that
worker's allocation, refuses later rejoins of the seat and forwards it once to the owning lobby,
which forgets the account's seat (a rejoin already in flight is answered `Unavailable`), so the
player's next lobby session is an ordinary welcome to Dashboard.

A queue or Practice handoff is complete for the lobby once every participant has its
`BeginMatchConnect` (BRL-k0tr): the lobby completes the reservation and forgets the activation at
that point. Each participant then closes its lobby session on purpose, and that close is not a
participant loss, so it neither cancels the running activation nor sends `Removed` to a
participant whose lobby session closes a few frames later. A client also ignores a `Removed` for a
reservation whose match grant it already accepted. Match-start failures after the handoff are
reported by the match worker on the match session.

## Routed packet transport

Lightyear 0.29 exposes `Link` as its transport-neutral boundary: concrete IO pushes opaque received
payloads into `LinkReceiver` and drains opaque outbound payloads from `LinkSender`. Its UDP server
binds one socket and creates child links keyed by remote address. Its Crossbeam transport
demonstrates the same integration for in-process channels but cannot cross processes.

The routed production path implements a Brawler-owned transport at that boundary. It does not fork
Lightyear or wrap gameplay replication in a second snapshot format.

Conceptual external envelope:

```text
magic | envelope_version | route_capability | opaque_lightyear_datagram
```

Conceptual internal packet frame:

```text
frame_version | direction | route_id | peer_id | payload_length | opaque_lightyear_datagram
```

The codec defines fixed endianness, maximum sizes, unknown-version behavior, route lifecycle, peer
identity, and malformed-frame handling. The envelope reduces effective Lightyear MTU, so the
transport advertises and enforces its measured safe payload size.

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

M01 compared these backend choices:

- Unix-domain socket on macOS/Linux, preferring packet/message semantics where reliable;
- Windows named pipe when Windows implementation enters scope, using the same logical frame codec
  whether the selected pipe API exposes byte- or message-oriented behavior;
- framed loopback TCP as a portable fallback if it materially reduced risk.

The current macOS production backend uses private framed Unix-domain byte streams with explicit
length-prefix handling; deterministic in-memory channels exercise the same logical codecs in tests.
Windows named-pipe work remains deferred until Windows enters supported implementation scope.

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
game-type catalog/configuration fingerprints, default-route identity, declared limits, and
restart/reconciliation inputs. From its validated active build and weapon content, the lobby derives
the bounded brawler advertisement used for both profile legality and client presentation. Each
worker validates its role-specific manifest completely before reporting readiness or accepting a
client.

## Authority and ownership boundaries

| Concern | Owner |
|---|---|
| Public UDP socket and route envelope | Supervisor/router |
| Route capability creation, validation, expiry, revocation | Supervisor from validated lobby allocation input |
| Game-type and advertised brawler catalogs, profile content validation, queue tickets, formation, reservation | Lobby authority |
| Saved-profile structural persistence and atomic load/store | Lobby-owned storage executor |
| Host worker count and resource admission | Supervisor |
| Match manifest validation | Worker |
| Match ECS, physics, map, terrain, modes, combat, results | Worker authoritative world |
| Lightyear connection and replication state | Receiving lobby or worker authority |
| UI flow and reconnect orchestration | Client presentation/session code |

Stable IDs cross process boundaries; Bevy `Entity` values, pointers, resources, and process-local
handles never do.

Lobby formation timing is validated operator policy in `config/server/game-types.ron`. One resolved
value supplies both the loading deadline advertised with a reservation and the Practice/product
deadlines enforced while allocation and route grants converge. Clients may present the advertised
overall loading duration, but the lobby remains the sole deadline authority.

For private authored-map Practice, the lobby owns candidate validation and binds the accepted
recipe bytes, revision, fingerprint, mode and roster to the allocation request. The supervisor
routes that bounded immutable payload without interpreting map semantics. Match-manifest version 5
requires exactly one map source—a built-in preset or authored recipe—and the worker repeats bounded
decode, fingerprint and resolver validation before inserting the resulting server map selection.
Worker failure and cancellation use the same reservation, route-revocation and capacity cleanup as
built-in Practice; local draft persistence remains client-owned.

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
- Public ingress drops are never silent. The supervisor logs at most one line per source and
  reason per 10 s window: `brawler-supervisor ingress dropped reason=<suppressed | malformed |
  lobby-not-ready | source-limited | route-rejected> source=<addr> folded=<n>`. It also logs
  `ingress lobby-route-reused` (route age and idle time) when a default-lobby datagram resumes an
  existing per-source lobby route after at least 1 s of silence, or when a Netcode-request-shaped
  payload arrives on a route older than 1 s. A per-window `ingress summary` counts every drop.
  Clients log `routed UDP socket bound local_addr=...` on every connect attempt so these lines
  can be matched to a device (BRL-2nsg).
- Capability (match and rejoin) routes are observable too. The supervisor logs
  `ingress capability-route-first-seen route=… worker=… kind=… source=… payload_len=…
  connection_request=…` once per route and source, and, for a route that saw a new source or a
  Netcode connection request in a window, one `ingress capability-route-window` line with
  `forwarded`, `connection_requests`, `forwarded_bytes` and `replies` (worker datagrams queued
  back to the source). The worker logs `brawler routed worker peer opened` on a route's first
  packet, `peer handshake completed`, or `peer handshake stalled … challenge_sent=<bool>` 2 s
  after a first packet that never reached Netcode `Connected`, and rate-limited
  `dropped packet for a closed route or peer`, which names `closed_reason`
  (`supervisor-peer-close | link-unlinked | link-missing | endpoint-unlinked`), `closed_ago_ms`
  and `closed_match` (whether the route, the peer or both matched the closed entry). Every packet
  of a new route's first IPC burst joins the one child link spawned for it: a Wi-Fi client can
  deliver its first two connection requests together (BRL-2nsg). Netcode refusals are already `Netcode error`
  warnings; the only silent case, `Ignored` (a non-request packet with no Netcode client), is
  debug level: run the server with
  `RUST_LOG=info,wgpu=error,naga=warn,lightyear_netcode::error=debug,lightyear_netcode::server=debug`
  (workers inherit the supervisor's environment) to see it.

## Reusable foundation, not a throwaway spike

V2 M01 retired architectural risk early, and its artifacts are the current production foundations:

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

M01 set thresholds from the M11 direct-UDP baseline, recorded the costed worker-port-range reference
design, and measured packet overhead/loss, IPC latency/queues, worker startup and memory, fixed-tick
behavior, handoff time, crash cleanup, and two-worker isolation. The accepted closeout retains the
single-port routed design; a future measured hard-threshold failure must return any contingency to a
focused Ticket specification rather than silently replacing this decision.
