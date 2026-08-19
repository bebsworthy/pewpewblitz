# Milestone 01 — Reusable routed multi-process server foundation

## Tracking

| Field | Value |
|---|---|
| Version | v2 — player UX and server-local matchmaking |
| Roadmap | [roadmap.md](./roadmap.md) |
| Architecture | [Multi-process server and single-port UDP/IPC transport](../../14-multiplayer-server-architecture.md) |
| Status | Complete |
| Research | Complete 2026-08-18. Exact Lightyear 0.29 link, connection, Netcode, UDP, Crossbeam, and transport sources; current Brawler composition; macOS IPC/process APIs; and the delivered M11 baseline were inspected. Decisions, limits, risks, contingency, and implementation contract are below. |
| Entry artifacts | Complete 2026-08-18: [V1 M11 worker-readiness handoff](../v1/milestone-11.md#v2-worker-readiness-handoff) and [direct-UDP baseline](../v1/evidence/v2-baseline/README.md) |
| Specification validation | Approved 2026-08-18 when the user explicitly directed implementation of v2 M01. |
| Implementation | Complete 2026-08-18. All implementation slices below are present in production role graphs and commands. |
| Verification | Development-use gate passed 2026-08-19. Canonical `CARGO_INCREMENTAL=0 just verify` passed: formatting; client/server/routing Clippy; 95 routing, 270 client, 261 server, 77 network integration, and 14 performance tests; server feature isolation; and a clean two-client routed lobby→match→fresh-lobby smoke with graceful match/lobby reap. Both-mode, crash/restart, RSS, public-envelope accounting, cleanup, and an exact 3,600-tick paired lane are also implemented. Exhaustive production campaigns and performance optimization are deferred to M09 by the user-approved scope decision below. |
| User playtest | Accepted 2026-08-19. The user reported that the routed build “seems to work roughly” and directed M01 closeout. No specific correctness defect was reported; roughness remains visible for later UX/performance hardening. |

## Outcome and evidence labels

M01 established one logical Brawler server at one public UDP endpoint, with a thin process
supervisor routing opaque Lightyear/Netcode datagrams to one isolated lobby worker and isolated
authoritative match workers. A client closes its lobby session and creates a fresh match session at
the same public address. The supervisor owns routing and process admission only; it never decodes or
mutates gameplay.

The following labels distinguish fact from choice:

- **Observed** means behavior or a measurement exists in this repository, pinned Lightyear 0.29
  source, or a cited primary API source.
- **Selected** means this specification chooses the behavior; implementation must prove it.
- **Derived limit** means arithmetic from observed evidence is shown.
- **Deferred** means a later milestone owns the behavior and M01 must not approximate it.

M01 is a production foundation, not a disposable spike. The user approved this specification on
2026-08-18 and accepted the development-use implementation on 2026-08-19; closeout is complete.

### 2026-08-19 verification scope decision

The user accepted the routed topology for development use and explicitly rejected treating a small
performance-threshold miss as grounds to discard the implementation. M01 therefore requires its
correctness, isolation, bounded-lifecycle, clean-smoke, and user-playtest gates, while exhaustive
production campaigns and late-stage performance optimization move to M09. This does not turn a
failed or unsupported measurement into a pass: the latest exact 3,600-tick paired run remains
recorded as 12.31% routed egress overhead versus the selected 10% target (ingress +0.35%, total
+7.92%), and
full IPC latency, packet-only IPC overhead, correlated CPU, dual-stack capture, and full campaign
cardinalities remain deferred work.

## Scope boundary

### In scope

- one public UDP socket owned by the supervisor;
- versioned public route envelopes and framed packet/control IPC;
- bounded high-entropy route capabilities;
- client and worker routed-link adapters at Lightyear's IO seam;
- deterministic memory and production macOS IPC backends with identical logical semantics;
- a plain supervisor runtime, worker lifecycle, bounded queues, fairness, diagnostics, security
  limits, and crash cleanup;
- one isolated minimum lobby authority, one reused authoritative match-worker composition, a
  sequential lobby-to-match transition, and two-worker isolation evidence;
- exact measurements against the delivered M11 direct-UDP baseline.

### Deferred or excluded

- **Deferred to M03–M06:** product game advertisement, build editing, queues, formation, results UI,
  requeue, and complete reconciliation.
- **Deferred to M09:** hardened recovery/resumption, fleet-scale security/capacity closeout, and
  final direct-UDP retirement.
- **Deferred until Windows is active:** named-pipe IPC; its logical backend boundary is required now.
- **Excluded:** accounts, global matchmaking, parties, discovery, NAT traversal, relays, fleet
  orchestration, join-in-progress, live connection migration, supervisor-side ECS/gameplay
  decoding, and a general distributed runtime.

## Resolved evidence

### Current Brawler seams

- **Observed:** Cargo.toml is one package with separately gated client/server binaries.
  src/bin/client.rs and src/bin/server.rs are thin; src/server/mod.rs owns authoritative App
  composition; src/client/session.rs owns connection creation and terminal disconnect today.
- **Observed:** the dedicated server is structurally one match World, composing authoritative map,
  movement, combat, terrain, mode, diagnostics, and one Lightyear endpoint.
- **Observed:** ServerNetworkConfig defaults to eight clients and three-second handshake/client
  timeouts. Gameplay protocol registration remains in src/protocol.rs.
- **Observed:** tests/network/harness.rs drives separate Apps over Crossbeam and manipulates Link
  buffers for deterministic delay, loss, duplication, and reordering.
- **Observed:** diagnostics already own bounded fixed-tick, high-water, byte/packet, drop/error,
  closeout, and process evidence. M01 extends observation, never gameplay mutation.

### Exact Lightyear 0.29 seam

- **Observed:** Link owns receive/send queues and LinkMtu. LinkSystems::Receive runs in PreUpdate and
  LinkSystems::Send in PostUpdate. LinkReceiveSystems::BufferToLink precedes ApplyConditioner.
  Lightyear chains receive link -> connection -> transport and send transport -> connection -> link.
- **Observed:** UDP and Crossbeam adapters push opaque datagrams to Link.recv in BufferToLink and
  drain Link.send in LinkSystems::Send. A routed adapter replaces only IO while retaining Netcode,
  replication, channels, and Connect/Disconnect.
- **Observed:** ServerUdpIo creates one LinkOf child per SocketAddr. Crossbeam proves the adapter seam
  but is in-process and unbounded.
- **Observed:** LinkStart/Linking/Linked/Unlinked and Unlink involve deferred observers. UDP server
  source handles packets arriving before deferred link setup. A multiplexed adapter must make the
  child visible before connection processing.
- **Observed:** NetworkTopology rejects multiple simultaneous conventional clients in one App.
  M01 closes lobby then starts a fresh match session; it never migrates a connection.
- **Observed:** Lightyear owns channel reliability, ordering, packetization, and message
  fragmentation using LinkMtu; Netcode owns outer authentication/encryption. Routing stays below
  both and duplicates neither.
- **Observed:** Lightyear's default link MTU and Netcode maximum application packet constant are
  1,200 bytes. Exact Netcode encoding adds at most 25 bytes (one prefix, up to eight sequence bytes,
  and a 16-byte authentication tag). Its connect request is 1,078 bytes.

### Implementation-verification risks

No research blocker remains, but implementation must prove:

1. every Netcode path respects the selected 1,158-byte inner-datagram bound;
2. first-packet link creation is same-frame correct across ApplyDeferred;
3. Unlink/despawn/revocation is exactly once under simultaneous EOF, timeout, and child exit;
4. partial Unix-stream IO and bounded writes never stall Bevy or supervisor;
5. copies/context switches meet the numeric gates.

A failure triggers one bounded optimization slice or the specified port contingency, never Netcode
parsing or weaker bounds.

## Selected architecture

### Process and authority map

| Owner | Runtime | Owns | Must not own |
|---|---|---|---|
| Client | existing Bevy App | input/presentation, one current Lightyear session, route wrapping | authority, capability minting, workers |
| Supervisor | plain Rust loop | public UDP, capabilities/routes/queues, admission, IPC, children, routing metrics | Bevy, Lightyear, Netcode parsing, lobby/gameplay state |
| Lobby worker | isolated minimal Bevy server | default-route Netcode auth, bounded M01 sessions/allocation/capability delivery | product queue/formation/results/requeue, match simulation |
| Match worker | existing authoritative Bevy composition plus routed IO | one match World, Netcode auth, gameplay/result | other matches, route admission, spawning |

The supervisor is a plain Rust process. Descriptor readiness, timers, bounded queues, and child
state need no Bevy World; omitting Bevy enforces dependency and authority isolation.

### Package, binary, and features

Implementation adds one small workspace package, provisionally brawler-routing, because codecs and
process IPC are a demonstrated cross-executable boundary.

- Its library has no Bevy, Lightyear, Avian, rendering, audio, or gameplay dependency and owns
  stable routing/process IDs, codecs, limits, queues, capabilities, backend traits, memory/Unix
  backends, and supervisor runtime.
- Its binary is provisionally brawler-supervisor.
- Root brawler uses its library for client and worker adapters.
- Existing brawler-server gains explicit lobby-worker and match-worker modes. Match-worker calls
  server::build_app_with_config rather than copying the plugin graph.
- Client, lobby-worker, match-worker, supervisor, network-test, and process-metrics graphs remain
  separately checkable. Supervisor must contain no Bevy/Lightyear.

Exact internal filenames follow cohesive ownership during implementation; the boundary above is
fixed. Do not add crates per role, backend, or message.

### Supervisor loop and processes

- One Mio Poll owner thread polls public nonblocking UDP, both Unix listeners/streams per worker,
  and Mio Waker. It solely mutates workers, routes, capabilities, partial IO, and queues.
- A signal handler thread may only set an atomic shutdown flag and wake the owner.
- The owner tracks monotonic timers and calls Child::try_wait at most every 100 ms. Poll timeout is
  at most 10 ms and shortens to the next deadline.
- Ready tokens rotate round-robin with bounded bursts. No async runtime, general pool, blocking IO,
  or per-worker thread is selected.
- Supervisor creates an owner-only private runtime directory. Workers connect to unique packet and
  control socket paths after spawn. Only role, identity, and paths cross argv. Manifests and secrets
  cross the control stream.
- stdin is null; stderr may use the operator log; stdout is not binary IPC. No public socket,
  listener, other-worker stream, capability, or manifest is intentionally inherited. Tests inspect
  descriptor hygiene.

Shutdown: close admission; revoke pending capabilities; send Stop; allow two seconds; stop packet
ingress; reconcile control/OS status; terminate remaining children; require reap within one further
second; unlink validated socket paths; remove only the validated private directory; close public
UDP. Overall deadline is five seconds and forced actions are reported.

## Lightyear integration contract

### Client

The routed client retains Client, NetcodeClient, replication, connection, protocol, and lifecycle
components and replaces UdpIo only.

- PreUpdate in LinkReceiveSystems::BufferToLink reads the public burst, validates/removes envelope,
  and pushes opaque inner bytes to Link.recv before conditioner, connection, and transport receive.
- PostUpdate in LinkSystems::Send, after transport and connection send, drains Link.send, prepends
  the envelope, and sends one datagram per inner packet.
- LinkMtu is 1,133 for lobby and match.
- All-zero selector is lobby-only. Match capability installs only from authenticated AllocationGrant.
- M01 retains the existing development manual Netcode credential. AllocationGranted carries no
  second Netcode token; after Netcode authentication the match worker admits only client IDs listed
  in its immutable manifest. Production credential issuance is deferred security hardening.
- The lobby delivers a registered `MatchRouteGrantV1` Lightyear message containing RequestId,
  AllocationId, MatchId, RouteId, PeerId, game mode, the 32-byte capability, and both expiries. The
  message is server-to-client on the authenticated lobby session, is redacted in Debug/log output,
  and is accepted only once for the client's current LobbySessionId/request. Outer route and IPC
  types remain outside `src/protocol.rs`.
- Transition is Disconnect lobby -> observe Unlinked/apply deferred work -> replace route -> Connect
  fresh Netcode. Error/timeout returns to a fresh lobby attempt; no match resumption.

The adapter never acknowledges, retries, reorders, fragments, encrypts, authenticates, or
deserializes inner data.

### Worker peer/link lifecycle

Packet IPC is multiplexed per worker. PeerId, RouteId, WorkerId, ProcessId, LobbySessionId,
AllocationId, MatchId, and LogicalServerId are random nonzero u128 values; none is Entity or PID.

- First valid packet creates a Server LinkOf child with Link(LinkMtu 1,133), Linked, and private
  PeerId. Explicit ApplyDeferred occurs before ApplyConditioner/ConnectionSystems::Receive and the
  triggering packet is buffered in that frame.
- Later payloads enter the mapped Link.recv with receive time.
- PostUpdate after transport/connection send drains each Link.send into packet IPC with per-route
  stable round-robin.
- PeerClose/revocation/EOF/stop triggers Unlink once, deferred observers, mapping removal, and
  lifecycle-compatible child despawn.
- Reconnect gets fresh PeerId, Link, capability, and Netcode session. Stale generation/route/peer
  frames are rejected.
- Failure of either IPC stream unlinks all peers and fails the worker.

## Versioned formats

### Common codec

- All multi-byte integers are big-endian. u128 is 16 bytes. Required IDs are nonzero.
- Versions begin at 1; reserved flags/bits must be zero.
- String = u16 byte length + UTF-8, maximum 255. Blob = u32 length + bytes. List = u16 count +
  elements. Optional = u8 tag 0/1. Boolean = u8 0/1.
- No serde/postcard or native layout crosses process/public boundaries.
- Validation order: fixed header, magic, advertised size/hard maximum, exact remaining bytes,
  version, type, flags, identity/generation, bounded counts/fields, UTF-8, then state semantics.
  Allocate only after bounds.
- Truncation, mismatch, trailing data, invalid enum/tag, zero ID, and reserved data are malformed.
  Public malformed input drops/counts silently. Packet IPC malformed fails that worker. Control
  malformed sends Failure when possible, closes/fails worker, and revokes routes.
- Public unknown version/type drops without reply; IPC unknown version/type is incompatibility.
- Logs/Debug omit capabilities, Netcode tokens, payloads, player manifests, raw frames, and source
  addresses. They use IDs, sizes, codes, counters.
- SHA-256 is the fixed 32-byte digest algorithm for v1 manifests and results, with domain prefix
  ASCII BRAWLER-MANIFEST-V1 or BRAWLER-RESULT-V1 followed by the exact canonical bytes. Digests
  detect accidental/cross-record mismatch but are not MACs. The private owner-only Unix endpoints
  are the IPC peer boundary; a hostile process running as the same OS account is outside M01.

### Public route envelope v1

| Offset | Width | Field |
|---:|---:|---|
| 0 | 4 | magic ASCII BRTE |
| 4 | 1 | version 1 |
| 5 | 1 | kind: 1 default lobby, 2 capability |
| 6 | 2 | flags 0 |
| 8 | 32 | all-zero lobby selector or random capability |
| 40 | 2 | inner_length |
| 42 | inner_length | opaque Netcode datagram |

Overhead is 42 bytes. Encoded maximum is 1,200; inner maximum 1,158; empty is invalid. Oversize is
dropped before parsing. IPv6 minimum path MTU 1,280 minus 40-byte IPv6 and 8-byte UDP headers leaves
1,232, so 1,200 leaves 32 bytes. Supported IPv6 paths therefore need no IP fragmentation. A
supported IPv4 path must expose path MTU at least 1,228 bytes (20-byte IPv4 + 8-byte UDP + 1,200
payload); the normal 1,500-byte baseline path qualifies. IPv4 paths below 1,228 are rejected as
unsupported rather than relying on fragmentation. Effective inner MTU is 1,158 for both families,
and LinkMtu is 1,133 = 1,158 - 25 worst Netcode overhead. Connect request plus envelope is 1,120.
M01 never preserves the old inner MTU by permitting IP fragmentation.

The public envelope has no separate integrity tag: UDP checksum detects ordinary corruption,
capability lookup authenticates only route selection, and the untouched inner Netcode packet
authenticates the session/payload. Header manipulation therefore causes validation drop, another
bounded route lookup, or inner Netcode rejection; it never authorizes gameplay.

### Packet IPC v1

Unix streams prefix each record with u32 record length. Record fields:

| Width | Field |
|---:|---|
| 4 | magic BRPK |
| 1 | version 1 |
| 1 | direction: 1 supervisor->worker, 2 worker->supervisor |
| 2 | flags 0 |
| 16 | WorkerId |
| 16 | RouteId |
| 16 | PeerId |
| 2 | payload_length |
| 1..1158 | opaque payload |

Header is 58 bytes, record max 1,216, prefixed max 1,220. Payload length exactly equals remaining
bytes. Direction must match endpoint. Frames retain UDP semantics and are not retransmitted.

### Control frame v1

Each record has u32 length prefix, then:

| Width | Field |
|---:|---|
| 4 | magic BRCT |
| 1 | version 1 |
| 1 | ControlType |
| 2 | flags 0 |
| 8 | sender sequence, nonzero and increasing |
| 16 | ProcessId |
| 16 | WorkerId |
| 4 | body_length |
| body_length | typed body |

Header is 52 bytes; record max 65,536; body max 65,484; prefixed max 65,540. Each direction has its
own sequence.

| Type | Body | Body/record maximum |
|---|---|---:|
| 1 Manifest | role u8; manifest_length u32; canonical manifest | 4,101 / 4,153 B |
| 2 Ready | manifest_digest[32]; generation u64; route_version u8; packet_version u8; control_version u8; flags u8=0 | 44 / 96 B |
| 3 Heartbeat | generation u64; uptime_ms u64; active_peers u16; packet_frames u16; packet_bytes u32; control_frames u16; control_bytes u32; fixed_tick_lag_us u32; health_flags u32 | 38 / 90 B |
| 4 AllocateRequest | RequestId u64; LobbySessionId u128; mode u16; participant_count u8; participants | 395 / 447 B |
| 5 AllocationGranted | RequestId u64; AllocationId u128; MatchId u128; WorkerId u128; grant_count u8; grants | 825 / 877 B |
| 6 AllocationRejected | RequestId u64; reason u16; retry_after_ms u32 | 14 / 66 B |
| 7 PeerClose | RouteId u128; PeerId u128; reason u16 | 34 / 86 B |
| 8 Stop | StopId u64; reason u16; graceful_deadline_ms u32 | 14 / 66 B |
| 9 Result | MatchId u128; AllocationId u128; result_digest[32]; result_length u32; canonical result | 4,164 / 4,216 B |
| 10 Failure | phase u16; category u16; related_sequence u64; detail_code u32 | 16 / 68 B |
| 11 Exit | role u8; exit_category u16; result_sent u8; terminal_peers u16; terminal_queue_bytes u32 | 10 / 62 B |

Allocate participant: LobbySessionId u128, PlayerId u64, authenticated Netcode client ID u64, team
u8, optional source preset u16, build recipe fingerprint u64, build revision u16. M01 accepts
exactly two; format max is eight.

Grant: LobbySessionId u128, RouteId u128, PeerId u128, capability[32], activation_expiry_unix_ms
u64, route_expiry_unix_ms u64. It is secret-bearing and never logged.

Result uses versioned canonical matchplay bytes, max 4 KiB; digest covers those bytes. M01 verifies
identity/exactly-once delivery but M06 owns product results.

### Lobby manifest v1

Common prefix: manifest_version u16=1, role u8=1, LogicalServerId/ProcessId/WorkerId each u128,
generation u64, Brawler network protocol u64, protocol registry fingerprint u64, content
fingerprint u64, route/packet/control versions each u8, flags u8=0.

Lobby fields: game mode u16 (1 Wipeout, 2 Hot Zone), default RouteId u128, max authenticated
sessions u16=32, outstanding allocations u16=2, active matches u16=4, heartbeat_ms u32=1,000,
nonce u128, digest[32]. Maximum 256 bytes. Digest covers preceding manifest bytes. Validate before
Netcode install/Ready.

### Match manifest v1

Common prefix role=2. Fields: MatchId u128, AllocationId u128, game mode u16 (1 Wipeout, 2 Hot Zone),
map preset u16, map revision u16, internal execution rules profile u8, objective target u16,
match-duration/countdown/respawn ticks each u64, reserved u8=0, seed u64, participant_count u8,
participants, heartbeat_ms u32, nonce u128, digest[32]. The execution profile remains an internal
production-versus-verification selector; operator-authored game types use the explicit objective
and timing values.

Participant: LobbySessionId u128, PlayerId u64, authenticated Netcode client ID u64, PeerId u128,
team u8, optional source build preset u16, recipe fingerprint u64, revision u16. The Netcode client
ID and routed PeerId are distinct identities and must never be inferred from one another. Worker
resolves selection against matching embedded content fingerprint before Ready. Max eight
participants and 4,096 total bytes. Entities, resolved floats, tokens, capabilities, and addresses
are excluded.

## IPC decision

| Backend | Evidence/tradeoff | Decision |
|---|---|---|
| Unix datagram | records preserved; oversize/truncation and queues complicate unified lifecycle; std supports nonblocking | Reject production |
| Unix sequence-packet | reliable records; not exposed by std/Mio high-level Unix API; needs lower socket2/libc surface | Reject M01 |
| Framed Unix stream | private path; ordered reliable bytes; std/Mio nonblocking; maps to named pipes | Select |
| Framed loopback TCP | same framing and portable; exposes IP endpoints/TCP tuning | Test/debug fallback only |

Use separate packet and control Unix streams per worker so large/retried control cannot share
byte-stream head-of-line state with packet data.

- Nonblocking reader retains prefix/current record; writer retains current offset. WouldBlock yields.
- EOF, invalid/oversize prefix, or non-WouldBlock IO maps to IpcPacketClosed, IpcControlClosed,
  IpcMalformed, or IpcIo and fails worker.
- Supervisor owns listeners, accepted handles, partial buffers, queues. Worker owns its connected
  pair only.
- Shutdown stops packet enqueue, flushes bounded Exit/control until deadline, Shutdown::Both, reaps,
  then unlinks.
- Backend API exposes logical packet/control send/receive/readiness/close, not descriptors. Windows
  named pipes must preserve record bytes, partial IO, limits, EOF, ownership.
- Memory backend passes exact encoded bytes through the same parsers and bounded queues. It may
  manually drive readiness, but never bypasses framing, validation, drops, IDs, sequences, lifecycle.

## Worker contracts

### Minimum lobby

Lobby accepts default-route sessions, performs normal Netcode authentication, creates bounded
LobbySessionId, and runs one two-participant transition. With two authenticated accepted builds it
sends one idempotent AllocateRequest. Only after match Ready does it receive AllocationGranted,
deliver capabilities over authenticated lobby links, and direct fresh match connections.

It does not advertise a product catalog, edit builds, implement general queues/formation, decide
capacity, author results, or requeue; M03–M06 do.

### Startup, health, reconciliation

1. Supervisor creates endpoints and spawns identity/role.
2. Worker connects both streams and validates exactly one Manifest: role, generation,
   protocol/content, limits, digest.
3. Worker composes Bevy/routed link then sends Ready. No route targets it earlier.
4. Heartbeat every one second; Suspect after three, Failed/revoked after five.
5. Stop is idempotent by StopId. Worker stops peers, triggers normal Unlink/AppExit, sends at most
   one Result and Exit, then closes.
6. Child status is final truth; it is reconciled with Exit. Missing/conflicting Exit is failure.

Exact duplicate control is ignored only after identical-content check. Lower generation/completed
request is stale/counts. Same identity with different content fails protocol. Result is immutable
and once per active MatchId/AllocationId.

Lobby restarts at most three times/60 s with one-second backoff. Admission closes while absent;
matches continue. M01 has no durable lobby state, so sessions/pending allocations fail and clients
create fresh sessions. Match workers never auto-restart: fail result, revoke routes, fresh lobby.

## Capabilities and security

- Capability is 32 OS-CSPRNG bytes (256 bits); entropy failure aborts allocation. Never derive from
  IDs/time/counters/Netcode.
- Bind to logical server/supervisor generation, worker/generation, route, peer, lobby session,
  allocation, match, protocol/content, and expiries.
- Pending 30 s; first valid envelope activates once. Token repeats per UDP packet until expiry.
- Active idle timeout 10 s, hard lifetime 10 min. No rotation; replacement means fresh session/token.
- Valid token permits at most two source address changes/10 s for NAT rebind; old source then drops.
- Only the most recently accepted source address remains valid after a rebind; the prior address is
  removed immediately so the allowance bounds changes rather than simultaneous bindings.
- Stop/failure/cancel/activation/idle/hard expiry/PeerClose revokes and unlinks. Bounded negative
  record lasts to original hard expiry.
- Secret type redacts Debug/Display and is forbidden in argv/env/log/panic/metrics/evidence.
  Diagnostics use RouteId.
- Theft can consume bounded queue/route work, race activation, or redirect within rebind limits, but
  cannot pass inner Netcode or create gameplay identity. Revoke and return to fresh lobby.
- Valid capability never replaces match-worker Netcode and manifest participant authentication.
- For M01, manifest participant authentication means matching the authenticated Netcode client ID
  against the immutable participant manifest; the shared development private key is an explicit
  local-foundation limitation, not a production credential claim.

## Capacity, backpressure, fairness

| Limit | Value | Basis |
|---|---:|---|
| Public datagram | 1,200 B | IPv6-safe derivation above |
| Pre-auth source | 8 datagrams and 9 KiB/10 s | eight 1,120 B handshakes, bounded work |
| Malformed source | 32/10 s then suppress 60 s | conservative, no reply amplification |
| Capabilities | 2 per authenticated lobby session/source | active plus one replacement |
| Active routes | 64 | 32 lobby + 4×8 match |
| Pending/active match workers | 2 / 4, plus one lobby | bounded spawn/memory; provisional host cap |
| Peers/worker | 8 | current server max |
| Packet record | 1,216 B; 1,220 prefixed | schema |
| Route packet queue | 64 frames, 77,824 B | 64×1,216, roughly 1 s at 60 packet/s |
| Worker packet queue | 512 frames, 622,592 B | 8 routes×64 |
| Global packet queue | 2,048 frames, 2,490,368 B | 4 match workers×512; lobby reserved |
| Control record | 65,536 B; 65,540 prefixed | bounded future manifest |
| Worker control queue | 16 frames, 262,144 B | one max frame plus lifecycle; bytes bind |
| Global control queue | 128 frames, 2,097,152 B | eight worker slots; bytes bind |
| Bursts | 64 UDP; 64 packet/worker; 16 control/worker; 8 consecutive/route | bounded turn |
| Poll/child poll | 10 ms / 100 ms | below fixed tick, avoids busy wait |
| Heartbeat/suspect/fail | 1/3/5 s | five misses; beyond 3 s client timeout |
| Ready | 5 s | Current 3 s handshake timeout + 1.356 s first-page-in process envelope + 0.644 s scheduling margin; the M11 sample was not a readiness measure |
| Graceful/forced/global stop | 2/additional 1/5 s | 21–23 ms baseline with process margin |

Packet service is per-route deficit round-robin, one-frame quantum, max eight consecutive. Lobby
has a reserved turn. Worker/control tokens rotate. A stalled worker owns no other queue/opportunity.

The selected data path permits at most two additional application-visible payload copies per
one-way routed datagram beyond the current Lightyear/client buffers: one into the supervisor's owned
framed queue and one from the worker's decoded frame into the Link payload (the reverse direction is
symmetric). Exact kernel copies and context switches are OS scheduling details and are not claimed
from source inspection; implementation records syscall counts and stage timestamps and profiles
them. Fixed reusable 1,200/1,220-byte receive buffers are selected. Additional pooling or
scatter/gather is allowed only after profiling identifies allocation/copy cost against the CPU or
latency gate; it may not change framing or queue ownership.

Packet overflow drops newest and counts; never retry/reorder. Nominal gate is zero drops. Revoke a
route if full for three heartbeats or 64 drops/10 s. Packet EOF/malformed/identity conflict unlinks
that worker. Control overflow/malformed/no progress for three heartbeat intervals fails worker.

Metrics expose envelope outcomes, capability reasons, queue current/high-water frames/bytes,
drops/deferrals, malformed/suppressed counts, partial IO/WouldBlock, heartbeats, lifecycle times,
child exits, routes/links, errors—using IDs, never secrets/addresses.

## Baseline and implementation gates

There are no routed measurements yet. Existing values are from
[M11 direct UDP](../v1/evidence/v2-baseline/README.md); selected routed values are review gates.

| Measure | Existing evidence/derivation | Gate and measurement |
|---|---|---|
| Overhead | Selected exact formats | **Hard:** 42 B public; 62 B packet IPC including prefix. Assert codec and counters |
| MTU | Lightyear 1,200; Netcode max wrapper 25 | **Hard:** public≤1,200, inner≤1,158, LinkMtu=1,133, no IP fragmentation. Boundary tests + IPv4/6 capture |
| Route/IPC latency | fixed tick 16.67 ms; none routed | **Hard:** added routing/IPC one-way p95≤2 ms (12% of one tick), measured by stage timestamps from supervisor public receive through IPC decode (and the symmetric send path), excluding ordinary wait for the next Bevy schedule; paired routed-minus-direct end-to-end p95 is corroborating evidence. p99/max diagnostic; 10,000 packets/direction |
| Queues/drops | M11 zero transport drops/errors | **Hard:** zero nominal and zero unaffected-route stall drops; **target:** nominal HWM≤25%. Counters across local/typical/adverse/burst/stall |
| Spawn-ready | UDP bind ~0.3 ms; first metrics launch had 1.356 s whole-process overhead beyond its 10.03 s tick window, not a readiness measure | **Hard:** each≤5 s = 3 s handshake + 1.356 s cold envelope + 0.644 s margin; **target:** p95≤2 s/20 cold starts. Spawn call to validated Ready |
| Stop-reaped | AppExit-to-report 21–23 ms | **Hard:** graceful≤2 s, forced total≤3 s over 25 cycles |
| Match RSS | idle ~38.7 MB; loaded ~42–43 MB | **Hard:** idle≤45, loaded≤50 MB (43×1.15 rounded). Same 2 Hz ps sampling |
| Lobby/supervisor RSS | no v1 equivalents | **Hard:** lobby≤45 MB (reuse idle-server cap), supervisor≤32 MB (must stay materially below a Bevy server); **diagnostic:** supervisor+lobby+2 loaded matches≤177 MB = 32+45+2×50 |
| Fixed tick | worst comparable 2v2 p95: HZ 1,233 µs, Wipeout 1,119; synthetic p95 5,702/16,667 | **Hard:** same scenario≤10% regression; current global≤1,356 µs (1,233×1.10 rounded); synthetic unchanged. Existing sampler, 3 paired runs |
| CPU | no reliable multi-process baseline | **Hard:** aggregate routed CPU time≤20% over newly paired direct; per-process diagnostic. Same host/build, 10 Hz and process time, 3 pairs |
| Bandwidth | 2v2 Wipeout 295,568/102,214 B, 1,480/1,411 packets; HZ 310,023/104,526 B, 1,491/1,422 | **Hard:** public=inner+42×datagrams and IPC=payload+62×frames within 1%; inner gameplay≤10% paired regression. Directional counters/capture |
| Handoff | handshake timeout 3 s; Ready limit 5 s | **Hard:** allocation accepted to Connected≤8 s; **target:** p95≤3 s/20 local. Correlated RequestId timestamps |
| Lifecycle | M11 25 soaks/mode, 20 reconnects, 100 terrain cycles; terminal links 0 | **Hard:** 25 graceful/mode +20 crash/restart end zero children/routes/queued bytes/socket files; RSS drift≤5 MB diagnostic |

The 20% CPU limit is conservative because M11 has no valid routed/multiprocess CPU baseline; it is
a gate, not a current claim. Reference host is the baseline Apple M3, 8 cores, 16 GiB, macOS 26.5.1.

## Error taxonomy

Stable categories: PublicMalformed, PublicOversize, PublicUnsupported, SourceLimited;
CapabilityUnknown/PendingExpired/RouteExpired/Revoked/Binding/RebindLimited; PacketQueueFull,
ControlQueueFull, IpcPacketClosed, IpcControlClosed, IpcMalformed, IpcIo; ManifestMalformed,
ManifestIncompatible, ManifestIdentity, WorkerReadyTimeout, HeartbeatTimeout,
WorkerProtocolConflict, WorkerReportedFailure, WorkerCrash, WorkerExitMismatch, WorkerStopTimeout,
AllocationCapacity, AllocationCancelled, InnerAuthenticationFailed, SupervisorShutdown,
SupervisorInternal.

Unauthenticated public errors get no response. Authenticated lobby rejection uses bounded code/retry
hint. Worker failures revoke owned routes; lobby loss closes admission; supervisor invariant failure
closes admission and performs bounded global shutdown.

## Implementation slices and ownership

The approved implementation proceeds in the following slices.

1. **Contract/baseline:** add Bevy-free routing package, IDs/constants/errors/codecs/secrets/memory
   backend and exhaustive parser tests. Preserve current routed-unaware commands; add named direct
   baseline before later default change.
2. **Supervisor:** registries, bounded queues/fairness/timers/metrics, Unix backend, spawn/reap,
   signal wakeup, cleanup; test admission, sequences, duplicates/stale/conflicts/overload/time.
3. **Worker link:** server-owned IPC transport plugin and explicit peer/link sets; lobby/match entry
   modes; reuse authoritative graph; composition/deferred tests.
4. **Client link:** route selection, MTU, sequential session lifecycle; preserve direct baseline;
   link/recovery tests without product UI.
5. **Minimum lobby/transition:** manifests, two-client AllocateRequest/Grant, worker startup, handoff
   to existing Wipeout/Hot Zone match composition.
6. **Real evidence:** UDP+Unix+children, two-worker isolation, malformed/impairment/stall/crash/
   forced shutdown/descriptor/cleanup tests.
7. **Measure/default:** paired direct/routed M11 scenarios. Only after routed transition passes,
   scripts/network.sh and just network default routed; direct stays under explicit baseline name.
8. **Closeout:** remove temporary diagnostics, lock commands/contracts, limitations, feedback and
   learning; meet exit or return to review with contingency evidence.

### Implementation log

| Slice | Status | Evidence |
|---|---|---|
| Contract and named direct baseline | Complete | Bevy-free `brawler-routing` workspace package; exact BRTE/BRPK codecs, typed IDs, redacted OS-CSPRNG capability, byte-faithful memory backend; `just network-direct` and `just network-direct-smoke` |
| Supervisor core | Complete | Bounded worker/route/capability registries, packet/control queues, reserved lobby fairness, activation/rebind/expiry/revocation, stalled-route isolation, metrics, and exactly-once cleanup |
| Control and manifests | Complete | Exact BRCT bodies/framing/sequence validation and canonical SHA-256 lobby/match manifests with redacted secret-bearing diagnostics |
| Allocate identity correction and supervisor orchestration | Complete (bounded slice) | `AllocateParticipant` now carries authenticated `NetcodeClientId` immediately after `PlayerId` (395 B body / 447 B framed maximum); allocation keeps that identity distinct from generated routed `PeerId`, validates exact-two/idempotent requests, and waits for match Ready before routes/capabilities and bounded Grant delivery |
| Client routed link adapter | Complete | BRTE UDP adapter, bounded receive burst, observable transport failure, spawn-time `LinkMtu=1,133`, focused Lightyear ordering/MTU/IPv4+IPv6 loopback tests, and routed session lifecycle selects it explicitly; client derives an IPv6 local socket when `--server` selects IPv6 and accepts explicit `--local-addr` |
| Unix IO and supervisor runtime | Complete (routing vertical slice) | Nonblocking partial packet/control streams, private endpoint cleanup, bounded Mio public UDP routing, per-source lobby routes, peer validation, teardown, and real local UDP/Unix tests |
| Match-worker manifest admission | Complete | Explicit Netcode client ID plus exact manifest PeerId, exact u128 MatchId, immutable whitelist/team/build validation, and reuse of the authoritative App graph |
| Worker link and process roles | Complete | Routed Lightyear peer links, bounded per-peer post-Netcode send FIFO, control sequencing/heartbeat/Result/Exit, and supervisor child spawn/reap/restart lifecycle |
| Minimum lobby and sequential transition | Complete | Two authenticated clients allocate one match worker, receive redacted grants, fully unlink lobby Netcode/Link, reconnect at the same public address, pass manifest admission, and replicate the two-player roster |
| Real-process evidence and measurement | Development-use gate complete; hardening deferred | Production two-worker isolation, lobby restart, Wipeout and Hot Zone Result→packet-drain→Exit→reap→fresh-lobby cycles pass. The 2026-08-18 both-mode run recorded 12,228/12,335 supervisor-owner diagnostic samples (maximum per-cycle p95 64 µs; not the IPC hard gate), supervisor/lobby/match RSS maxima 6.5/29.8/41.4 MiB, exact public-envelope overhead, directional mixed IPC diagnostics, zero live routes/capabilities/queues/source limits, and no leftover children/socket files. IPv4 and IPv6 production loopback smokes pass. On 2026-08-19 a clean two-client routed smoke again completed allocation, both handoffs, authoritative Result/Exit, graceful worker/lobby reap, and cleanup. The paired lane now proves an exact 3,600-tick authoritative interval with identical nonzero protocol/content fingerprints; the latest run measured routed ingress +0.35%, egress +12.31%, and total +7.92% versus direct. The egress target remains an honest miss, most plausibly affected by startup replication and transport-boundary sampling, and is deferred with full IPC latency, packet-only IPC overhead, correlated CPU, dual-stack MTU capture, fixed-tick comparison, and full 25/20-cycle campaigns to M09. See [routed evidence](./evidence/m01-routed/README.md) and [paired evidence](./evidence/m01-paired/README.md). |

After approval, expected existing ownership touched is workspace/Cargo; client session; server
entry/composition; diagnostics; network/process tests; canonical scripts/justfile/README.
Outer routing types do not belong in src/protocol.rs. Exact new internal filenames remain
implementation choices; authority/dependency boundaries do not.

## Verification contract

### Focused and schedule tests

- exact codec byte fixtures/round trips; truncation at each boundary; trailing/oversize/bad
  version/type/flag/enum/tag/UTF-8/count/ID; secret redaction;
- capability entropy failure/binding/activation/repeat/replay/rebind/expiry/revocation and stolen
  token with failed Netcode bounded work;
- admission and dual queue bounds, fairness, drops, control escalation, high-water;
- manifest digest/identity/fingerprint/role/mode/map/rules/participant/build;
- idempotent duplicates, stale generation/sequence/request, conflicting duplicates, one Result/Exit;
- Bevy client/worker set ordering, first-packet ApplyDeferred visibility, LinkStart/Unlink cleanup,
  and match-worker reuse of authoritative composition;
- memory backend uses identical bytes/parsers/queues/routes/transition driver.

### Real process/network tests

- IPv4 and IPv6 lobby Netcode through default route;
- two authenticated clients receive capabilities and create match sessions at unchanged address;
- existing Wipeout/Hot Zone authority replicates through production worker;
- two workers isolate routes, peers, processes, Worlds, physics, gameplay, terrain, result/failure;
- malformed/oversize/default floods allocate no worker and stay bounded;
- typical/adverse loss/latency/jitter/duplication/reorder retain Lightyear semantics;
- stalled IPC causes specified own drops/revocation and zero unrelated drops;
- match crash affects only its routes; lobby crash closes admission while matches continue;
- normal/missing/conflicting Exit, hung worker, signal, forced reap leave no child/handle/route/queue/
  socket file; descriptor inspection proves no inherited public/other-worker handles.

### Command requirements

Implementation adds canonical just commands for isolated role checks/tests, deterministic routing,
real routed smoke/two-worker, impairment/backpressure/crash/cleanup, bounded routed process
evidence, paired measurement when its instrumentation exists, and named direct-UDP baseline. The
bounded evidence command is `just network-routed-evidence`; it runs 5 cold Wipeout cycles by
default (or `just network-routed-evidence <cycles> <timeout-seconds> <mode>`, with 1–25 cycles),
captures per-role RSS from `ps`, requires one exact
spawn→Ready→Result→successful-reap→graceful-stop→cleanup chain, validates final supervisor
route/queue/drop counters, and writes
`target/routed-evidence-<UTC timestamp>.json`. Pass `mode=hot-zone` for Hot Zone, `mode=both` to
run the requested cycles per mode, or `mode=crash-restart` for bounded production-worker crash
isolation plus lobby restart tests; the latter's Rust assertions require zero children, routes,
live capabilities, queued bytes, and private socket files at terminal cleanup. Every selected mode
is carried by the digest-bound lobby manifest and recorded per cycle. Public-envelope overhead is
validated exactly. Directional mixed IPC counters and supervisor owner-boundary latency are
diagnostic only: they do not include worker decode and cannot satisfy the IPC hard gate. It marks
full routed IPC latency, packet-only IPC overhead, IPv4/IPv6 MTU capture, and fixed-tick paired
regression as `unsupported`; correlated stop/reap and allocation-to-connected samples are measured
but remain below their campaign cardinalities. `just network-paired-evidence 1 90
wipeout` is the bounded one-pair comparison smoke; `just network-paired-evidence 3 90 wipeout`
(or `hot-zone`) is the three-pair CPU/inner-gameplay-bandwidth gate. It runs the existing direct
and routed launchers sequentially with the same host/build/scenario contract, records per-process
CPU/RSS samples, requires exact process-role cardinality, and compares direct server transport
bytes against match-worker routed inner ingress/egress only after a correlated common observation
interval is proven. Until then it reports raw totals as unthresholded diagnostics and the hard
gates as unsupported. Public-envelope and mixed packet/control IPC bytes remain separate.
`just test-paired-evidence` exercises the parser and threshold math without launching processes.
The implementation now provides `just network-routed-ipv6-smoke` for a production IPv6 loopback
run and `just network-routed-capture capture=<path>` for an optional macOS `tcpdump` run. The
capture wrapper requires BPF permission (it does not invoke `sudo`) and delegates final evidence
to `scripts/verify-routed-capture.py`, which only passes a real classic pcap containing observed
IPv4/IPv6 UDP traffic with payloads ≤1,200 bytes and no IPv4 fragmentation or IPv6 Fragment
headers; an unavailable, empty, malformed, or unsupported capture remains `unsupported`.
Set `--keep-artifacts`
when inspecting per-cycle logs. README records exact names. Existing role checks/tests/Clippy/
feature audit/network/performance gates remain. Deterministic ECS tests advance schedules/time
without sleeps.

### User handoff

After automation, a canonical routed two-client windowed check shows:

1. both lobby diagnostics use the same public address;
2. allocation/Ready is visible without exposing capability;
3. each disconnects once and reconnects at unchanged address into one Wipeout match;
4. movement, firing, damage, respawn, terrain, and HUD remain readable;
5. stopping match reports bounded transition and creates a fresh lobby session;
6. shutdown leaves no child.

User observes duration/stutter, duplicate/lost state, feel versus direct UDP, and stale HUD/window
state. No product lobby/results UI is claimed.

## Worker-port-range contingency

Alternative retains supervisor/control but workers use ordinary Lightyear UDP:

- one stable lobby port plus contiguous pool eight: four Active, two Starting, two Quarantine;
- state Free -> Starting(WorkerId/generation) -> Active(Route/Match) -> Quarantine. Free only after
  reap/socket close plus five seconds (longer than current 3 s timeout), with fresh Netcode material;
- grant changes endpoint/port; ceiling min(4 active, non-quarantined free); exhaustion rejects;
- firewall/security group/container exposes/forwards nine UDP ports instead of one;
- stale packets drop in quarantine or fail fresh Netcode;
- avoids envelope, client routed adapter, packet forwarding/copies/IPC, worker multiplex adapter;
  retains manifests, control IPC, supervision, lobby allocation, handoff, cleanup;
- adds port pool, endpoint-changing UX, operations, lower density, nine unauthenticated endpoints,
  and stale-port handling.

A qualifying failure only returns M01 to Specification review with the recorded evidence and a
recommendation. The worker-port-range contingency cannot be selected, implemented, or made the
default without the user's express approval. Evidence eligible for that recommendation is one
documented bounded optimization still reproducibly failing a hard fragmentation/MTU, nominal-loss,
unaffected-route, p95 latency, tick, CPU, memory, spawn/reap, or cleanup gate, or exact lifecycle
tests showing that the selected link seam cannot preserve opaque Netcode. Diagnostic targets alone
do not qualify. No silent switch or dual production transport is permitted.

## Exit criteria and limitations

M01 leaves Specification review only after user approval. Complete requires the approved
development-use scope:

- one endpoint routes real lobby/match Lightyear over real IPC;
- fresh lobby->match->lobby sessions at same address;
- normal client owner and existing authoritative match graph;
- focused/role/integration/process correctness tests and the clean routed smoke pass;
- two-worker process/route/peer/World/gameplay/terrain/result/failure isolation;
- bounded observable malformed/stolen/expired/pressure/IPC/crash/shutdown behavior;
- isolated feature graphs; identical memory/real semantics; no test-only authority;
- commands, honest measurements, user check, feedback, and learning complete;
- direct UDP retained explicitly; M09 owns removal.

The 2026-08-19 scope decision defers exhaustive latency/CPU/overhead/MTU measurement, performance
optimization, and full campaign cardinalities to M09. Those observations retain their failed or
unsupported labels; they are not production-readiness completion claims.

Limitations: scripted two-client transition, four-worker provisional host cap, one host, no
resumption/join/durable lobby reconciliation/product lobby/results UI/Windows backend, and bounded
but not eliminated bearer-capability theft. The production two-match isolation test drives the
supervisor allocation seam and terminates one real Bevy child; it does not connect public clients
to both matches concurrently, so per-match replicated gameplay/terrain/result convergence remains
covered by the sequential routed smoke and deferred to the concurrent lifecycle milestone.

## Research log

### Repository and local sources

- AGENTS.md; docs/00-product-direction.md; docs/08-network-architecture.md;
  docs/13-player-ux.md; docs/14-multiplayer-server-architecture.md
- v2 roadmap/M01; v1 roadmap/M11/direct-UDP baseline; README.md; Cargo.toml; Cargo.lock; justfile
- src/bin/client.rs; src/bin/server.rs; src/client/session.rs; src/server/mod.rs; src/protocol.rs;
  src/config.rs; src/diagnostics/; tests/network/harness.rs
- scripts/network.sh and relevant process, measurement, feature-audit, closeout scripts
- references/lightyear/examples/README.md, simple_setup source/Cargo
- references/lightyear/book/src/SUMMARY.md, concepts/transport/, concepts/connection/,
  tutorial/build_client_server.md

### Exact installed Lightyear 0.29

- lightyear_link-0.29.0 src/lib.rs, server.rs, mtu.rs
- lightyear_udp-0.29.0 src/lib.rs, server.rs
- lightyear_crossbeam-0.29.0 src/lib.rs
- lightyear_connection-0.29.0 src/lib.rs, network_topology.rs
- lightyear_netcode-0.29.0 src/client_plugin.rs, server_plugin.rs, packet.rs, token.rs, client.rs,
  server.rs
- lightyear_transport-0.29.0 packetization, buffering, and plugin source

All are under /Users/boyd/.cargo/registry/src/index.crates.io-1949cf8c6b5b557f/. Exact source was
necessary because checked-in Bevy is 0.20-dev while Brawler pins Bevy 0.19.1/Lightyear 0.29.0.

### External primary sources

Local material did not define Rust/macOS stream/process/readiness/entropy behavior, so research used:

- [Rust UnixStream](https://doc.rust-lang.org/std/os/unix/net/struct.UnixStream.html)
- [Rust UnixDatagram](https://doc.rust-lang.org/std/os/unix/net/struct.UnixDatagram.html)
- [Rust Command](https://doc.rust-lang.org/std/process/struct.Command.html)
- [Rust Child](https://doc.rust-lang.org/std/process/struct.Child.html)
- [Mio 1.2.2](https://docs.rs/mio/1.2.2/mio/), [Poll](https://docs.rs/mio/1.2.2/mio/struct.Poll.html),
  and [Waker](https://docs.rs/mio/1.2.2/mio/struct.Waker.html)
- [socket2 0.6.5](https://docs.rs/socket2/0.6.5/socket2/)
- [getrandom 0.4.3 fill](https://docs.rs/getrandom/0.4.3/getrandom/fn.fill.html)
- [NIST FIPS 180-4 Secure Hash Standard](https://csrc.nist.gov/pubs/fips/180-4/upd1/final)

Cargo.lock contains Mio 1.2.2, socket2 0.5.10/0.6.5, and getrandom 0.2.17/0.3.4/0.4.3 transitively.
Implementation explicitly locks the smallest compatible direct dependency, not a transitive API.

## Specification validation

Research resolved and the user approved this specification on 2026-08-18 before implementation.
The later 2026-08-19 development-use scope decision and its deferred hardening are recorded above;
no unapproved transport contingency was selected.

## Feedback review

| Feedback | Disposition |
|---|---|
| Routed development build “seems to work roughly.” | **Accepted for M01.** The successful interaction satisfies the development-usability gate. No actionable correctness defect was identified, so no speculative code change is made. General transition/presentation roughness remains owned by M02–M07, while routed performance and exhaustive production hardening remain `V2-ROUTED-HARDENING` in M09. |
| Close M01. | **Implemented.** Verification evidence, limitations, deferred measurements, and this feedback disposition are recorded; M01 and the roadmap are marked Complete. |

## Learn-from-errors review

1. **Development usability and production hardening were initially conflated.** The first exit
   contract allowed narrow performance thresholds and exhaustive campaigns to block use of an
   otherwise functioning foundation. Cause: the specification optimized for production confidence
   before the product needed that confidence. Prevention: future infrastructure milestones must
   separate correctness/development-use gates from later capacity, optimization, and operational
   hardening at specification time. M09 now owns the deferred routed gates without relabeling their
   failed or unsupported evidence.
2. **Control ownership was too distributed.** Runtime and lifecycle code could allocate from the
   same BRCT sequence space during shutdown, and Result could overtake final packet IPC on a
   separate stream. Cause: cross-stream ordering and sequence ownership were implicit. Prevention:
   keep one explicit sequence owner, suppress non-shutdown controls after Stop, retain lifecycle
   polling/reaping, and require the packet-EOF drain barrier before Result cleanup. Focused
   regressions now cover both boundaries.
3. **Evidence labels briefly exceeded what the instrumentation measured.** Owner-loop timing was
   not full public-to-worker IPC latency; mixed control/packet counters were not packet-only
   overhead; matching zero fingerprints did not prove build identity. Cause: convenient counters
   were treated as end-to-end facts. Prevention: name the exact observation boundary, fail closed
   on absent identity/cardinality, preserve `unsupported` when a boundary is missing, and keep raw
   diagnostics separate from threshold claims.
4. **The final canonical gate found issues narrower checks missed.** Interrupted incremental builds
   complicated linking, and late diagnostics/outbox edits passed focused tests before all-role
   Clippy exposed reviewability issues. Prevention: use clean non-incremental recovery when a build
   is interrupted and run the canonical `just verify` after the final source edit, not merely before
   handoff. The closeout run passed formatting, all role Clippy lanes, 95 routing, 270 client, 261
   server, 77 network, and 14 performance tests, feature isolation, and the routed process smoke.
5. **Reusable lesson:** preserve strict truthfulness without letting measurement work displace the
   product stage. A failed optimization target stays failed and documented, but it blocks a
   milestone only when the user-visible or correctness outcome actually depends on it.
