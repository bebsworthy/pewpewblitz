# Milestone 02 — Network connection and replication sandbox

## Tracking

| Field | Value |
|---|---|
| Version | v1 — gameplay MVP |
| Roadmap | [roadmap.md](./roadmap.md) |
| Status | User playtest |
| Specification validation | User requested implementation directly |
| Implementation | Complete |
| Verification | Complete |
| User validation/playtest | Pending handoff |

Update this table and the roadmap together whenever the milestone changes phase.

## Outcome

Two local macOS clients connect through Lightyear to one dedicated authoritative server. The
server accepts compatible sessions, assigns stable session-scoped player and network-entity IDs,
spawns one placeholder player per accepted connection, and replicates the complete placeholder
roster to both clients. Rejection, timeout, disconnect, reconnect, and server shutdown have
explicit, observable outcomes and leave no session-owned entities behind.

This milestone validates real connection and replication behavior. It does not add movement,
input networking, prediction, interpolation, collision, combat, teams, or match rules.

## Source requirements

- [Product direction](../../00-product-direction.md)
- [Engine decision](../../01-engine-decision.md)
- [Gameplay MVP](../../05-gameplay-mvp.md)
- [Network architecture](../../08-network-architecture.md)
- [Version 1 roadmap](./roadmap.md)
- [Milestone 01 learn-from-errors review](./milestone-01.md#learn-from-errors)

## Milestone 01 feedback incorporated

Milestone 01's review and learn-from-errors section are binding inputs to this specification:

- Exact APIs and features must be checked against the Cargo-resolved Lightyear 0.29.0 crate, not
  inferred only from a nearby source snapshot or example.
- `ClientPlugins` or `ServerPlugins` must be installed before Brawler's protocol plugin, and all
  protocol registration must finish before any Lightyear client, server, or link entity is spawned.
- Headless tests must drive Bevy's real schedules and Lightyear lifecycle through deterministic
  `Time<Real>` / `TimeUpdateStrategy` advancement. Tests must not claim connection, timeout,
  replication, or cleanup coverage after directly inserting final lifecycle markers.
- Windowed winit coverage stays in a main-thread process smoke test. Headless integration apps use
  `MinimalPlugins` and do not construct a macOS event loop on Cargo test threads.
- Multi-process launchers must use Cargo-resolved execution, supervise every child, propagate the
  first failure, reject premature clean exits, clean up siblings, wait for explicit server
  readiness, and work with `CARGO_TARGET_DIR`.
- CI and documented dependency-resolving commands use `--locked`.
- Test names and recorded evidence must describe the behavior actually exercised. Links in the
  milestone record must be validated from this directory.

## Scope boundaries

### In scope

- Lightyear client/server plugin groups with the shared 60 Hz tick duration;
- UDP loopback I/O with Netcode for real local client/server processes;
- deterministic Crossbeam I/O with Netcode for separate-`App`, in-process integration tests;
- explicit server and client process configuration with validation and actionable errors;
- connection, compatibility, acceptance, rejection, timeout, disconnect, reconnect, and shutdown
  lifecycles;
- stable server-assigned player and network-entity IDs for the current session;
- one server-owned placeholder player per accepted connection;
- server-to-client component replication and join-in-progress roster recovery;
- session ownership and automatic cleanup on disconnect;
- one supervised local command for a server and two clients plus a bounded process smoke harness;
- locked CI coverage for the supported production feature graphs and the network-test graph.

### Out of scope

- internet hosting, NAT traversal, certificate management, matchmaking, accounts, or token services;
- trusting the client-supplied Netcode ID as Brawler's player identity;
- persistent identity across process restarts or reconnects;
- movement/facing input, client authority, prediction, rollback, or interpolation;
- `Transform` replication, rendering of fighters, cameras, HUD, collision, Avian, combat, teams, or
  match lifecycle;
- host-client or single-World authority shortcuts;
- WebTransport, WebSocket, Steam, compression, bandwidth prioritization, or interest management;
- a generic session framework or persistence abstraction.

## Research questions and conclusions

### Version and feature validation

- [x] Confirm the exact Lightyear 0.29.0 features needed for this milestone.
  - Production client: `client`, `replication`, `netcode`, `udp` (with `std` supplied by UDP).
  - Production server: `server`, `replication`, `netcode`, `udp`.
  - Integration tests add `crossbeam`; they do not add prediction, interpolation, or inputs.
- [x] Confirm plugin and entity ordering against Cargo-resolved 0.29.0 source: Lightyear plugin
  group, then shared protocol registration, then connection/server entities.
- [x] Confirm that Lightyear's replication feature installs its protocol-registry checksum check.
  Keep a separately versioned Netcode protocol ID because the checksum is exchanged only after a
  Netcode connection exists.
- [x] Confirm the dedicated server remains render-, window-, audio-, device-input-, and
  client-asset-free after the new networking features are enabled.

### Transport and connection model

- [x] Compare raw connection, Netcode over UDP, and WebTransport for the local sandbox.
  Netcode over UDP is selected because it supplies connection state, timeouts, rejections, and
  long-lived transport peer IDs without certificate setup. Raw connection lacks the lifecycle
  guarantees being tested; WebTransport adds certificate and async-I/O complexity not justified by
  a native loopback milestone.
- [x] Select a deterministic in-process transport. Crossbeam exercises Lightyear's normal link,
  Netcode, message, and replication layers while allowing separate client/server `App` worlds and
  simulated time without socket races or sleeps.
- [x] Keep a real UDP verification path. A local UDP integration/process smoke test proves the
  production I/O feature rather than treating Crossbeam success as UDP evidence.

### Identity, authority, and lifecycle

- [x] Separate three identities:
  - process-local Bevy `Entity`, never serialized by Brawler;
  - Lightyear `PeerId`, used internally to identify the transport connection;
  - Brawler `PlayerId` and `NetworkEntityId`, allocated monotonically by the server and replicated.
- [x] Define reconnect semantics: after cleanup, reconnecting with the same development Netcode ID
  creates a fresh session and receives new Brawler IDs. No account identity exists yet.
- [x] Define join-in-progress: a newly accepted client receives all currently replicated placeholder
  players, then every accepted client receives the new placeholder. The server snapshot is truth;
  clients do not reconstruct missed joins from event history.
- [x] Confirm cleanup support: `ControlledBy` with `Lifetime::SessionBased` despawns server-owned
  entities when the owning connection gains `Disconnected`; Lightyear then replicates the despawn to
  remaining clients.
- [x] Define duplicate safety: repeated connect, hello, disconnect, and cleanup observations are
  ignored once the connection's Brawler session phase has advanced. At most one placeholder exists
  per active connection.

### Compatibility and rejection

- [x] Use a nonzero, explicitly versioned `NETWORK_PROTOCOL_ID` in Netcode configuration. Change it
  whenever an incompatible wire protocol is introduced so incompatible peers fail before becoming
  active Brawler sessions.
- [x] Retain Lightyear's registry checksum exchange to detect mismatched message, component, and
  channel registration after connection.
- [x] Add a Brawler ordered-reliable hello/outcome exchange. The client sends its protocol version
  and package build version; the server sends an accepted or rejected outcome and never spawns a
  placeholder before acceptance.
- [x] Make failure visible: client connection status and structured logs record timeout, Netcode
  rejection, compatibility rejection, remote disconnect, or user-requested shutdown. Automated
  client mode exits nonzero on failure instead of waiting indefinitely.

### Verification strategy

- [x] Use separate Bevy `App` worlds for server and each client in integration tests.
- [x] Drive Netcode connection, messages, replication, timeout, and cleanup through normal
  `PreUpdate` → fixed schedules → `PostUpdate` execution with deterministic simulated time.
- [x] Add a real UDP smoke case using an OS-assigned loopback server port where possible, plus a
  supervised three-process local harness for the documented application binaries.
- [x] Preserve feature isolation: the deterministic test feature may unify client/server code only
  in its dedicated integration-test target; the production server is still checked with only the
  `server` feature.

## Research log

| Date | Local path or source | Finding | Implication/decision |
|---|---|---|---|
| 2026-08-13 | `docs/implementation/v1/milestone-01.md`, `## Learn from errors` | Exact-version assumptions, winit thread ownership, hard-coded binary paths, unsupervised children, manual schedule invocation, and unlocked CI all caused or could hide defects. | Carry each prevention into composition, testing, launch supervision, CI, and evidence requirements above. |
| 2026-08-13 | `references/lightyear/examples/simple_setup/{Cargo.toml,src/main.rs,src/shared.rs,src/client.rs,src/server.rs}` | Lightyear installs client/server groups before shared protocol registration and spawns configured endpoint entities afterward. UDP + Netcode is the smallest native authenticated lifecycle example. | Preserve that ordering; adapt only connection setup, not the example launcher or unrelated transports. |
| 2026-08-13 | `references/lightyear/examples/simple_box/{Cargo.toml,src/protocol.rs,src/server.rs,src/client.rs}` | Server link entities gain `ReplicationSender`; authoritative players spawn on `Connected`, use `Replicate`, and use `ControlledBy` for ownership. The example also enables prediction/interpolation and inputs. | Reuse only raw replication and ownership. Gate Brawler spawn on its compatibility hello and defer prediction, interpolation, and input plugins. |
| 2026-08-13 | Cargo-resolved `lightyear-0.29.0/{Cargo.toml,src/client.rs,src/server.rs,src/shared.rs,src/protocol.rs}` | Exact features and ordering match the selected design. The replication feature installs `ProtocolCheckPlugin`; UDP and Netcode are optional; Crossbeam is separately optional. | Enable the smallest feature set and add Crossbeam only to a dedicated network-test feature. |
| 2026-08-13 | Cargo-resolved `lightyear_connection-0.29.0/src/{client.rs,server.rs,shared.rs}` and `references/lightyear/crates/connection/connection/src/{client.rs,server.rs,shared.rs}` | Lifecycle is represented by `Connecting`, `Connected`, `Disconnecting`, and `Disconnected` components with structured disconnection reasons. Netcode drives these through normal receive/send systems. | Observe real lifecycle changes; do not create application truth in a parallel state machine or fake final markers in end-to-end tests. |
| 2026-08-13 | Cargo-resolved `lightyear_netcode-0.29.0/src/{client_plugin.rs,server_plugin.rs,token.rs}` | Netcode supplies protocol ID validation, stable `PeerId::Netcode`, request denial, request/challenge/connected timeouts, duplicate-client rejection, and server stop behavior. | Use Netcode for the production and deterministic test connection layers; treat the client-supplied ID only as development transport identity. |
| 2026-08-13 | `references/lightyear/crates/replication/replication/src/{lib.rs,control.rs}` and book `concepts/replication/{protocol.md,replicate.md}` | Registration order must match. `Replicate` sends registered components, while `ControlledBy` with session lifetime removes owned authoritative entities on disconnect. | Centralize protocol registration and attach ownership to each accepted placeholder. |
| 2026-08-13 | `references/lightyear/crates/io/{crossbeam/src/lib.rs,udp/src/lib.rs,udp/src/server.rs}` | Crossbeam uses normal `Link` buffers and lifecycle and is deterministic in-process; UDP supports server port `0` and records the OS-selected address. | Use Crossbeam for exhaustive deterministic tests and UDP for the real I/O smoke path. |
| 2026-08-13 | `references/lightyear/crates/tests/src/{stepper.rs,client_server/connection.rs,client_server/base.rs}` | Lightyear's own tests use separate apps, deterministic `Time<Real>`, Netcode over Crossbeam, and explicit server-first stepping for lifecycle/replication assertions. | Build a small Brawler-owned harness around public APIs; do not depend on Lightyear's unpublished test crate or copy its full stepper. |
| 2026-08-13 | Book `concepts/bevy_integration/system_order.md` and `concepts/reliability/channels.md` | Receive/replication occurs in `PreUpdate`, gameplay runs in fixed schedules, and sends flush in `PostUpdate`; ordered reliable channels preserve delivery order. | Process hello and lifecycle state in `Update`, retain authoritative gameplay in `FixedUpdate`, and use one ordered-reliable session channel. |

The checked-in material and Cargo-resolved 0.29.0 source answered the implementation questions, so
no internet fallback was required for this specification.

## Technical specification

Status: **Implemented; the user requested implementation directly. Automated verification is
complete and interactive user playtest is pending.**

### Decisions

| Decision | Selected option | Alternatives | Rationale and validation |
|---|---|---|---|
| Real local transport | Netcode over UDP loopback | Raw UDP connection; WebTransport | Exercises rejection and timeout with no certificate setup. Verify with real UDP integration and three-process smoke tests. |
| Deterministic integration transport | Netcode over Crossbeam between separate apps | Host-client; direct marker insertion; only real sockets | Preserves separate authority worlds and normal networking layers while allowing simulated time. |
| Lightyear feature graph | Production: role + `replication,netcode,udp`; test: both roles + `crossbeam` | Default Lightyear features | Excludes input, prediction, interpolation, WebTransport, and client presentation from the server graph. |
| Compatibility | Netcode `NETWORK_PROTOCOL_ID` + Lightyear registry check + Brawler hello/outcome | Package version only; checksum only | Provides an early incompatible-wire gate, exact registration validation, and an explicit user-visible join decision. |
| Brawler identity | Server monotonic `PlayerId` and `NetworkEntityId` newtypes | Netcode client ID; Bevy `Entity`; UUID | Server authority is explicit, values are compact and stable for the session, and process-local identity never crosses the wire. |
| Reconnect | Fresh session and fresh Brawler IDs after full cleanup | Reclaim old entity; persistent account identity | No authentication or persistence exists yet. Fresh identity is deterministic and prevents stale ownership reuse. |
| Placeholder state | Replicated marker, IDs, and small spawn-slot/state component | `Transform`; custom world snapshot | Proves component replication without pre-implementing movement or duplicating Lightyear snapshots. |
| Ownership cleanup | `ControlledBy { lifetime: SessionBased }` | Manual entity map only; persistent lifetime | Uses Lightyear's connection relationship and guarantees server-owned cleanup on disconnection. |
| Join-in-progress | Current replicated server world is the snapshot | Replay join messages | State replication naturally recovers existing placeholders and avoids an event-history subsystem. |
| Session messages | One ordered-reliable bidirectional channel with direction-restricted types | Default unreliable; one channel per message | Hello must precede outcome; the message volume is negligible. |
| Process configuration | Typed CLI config with validated socket addresses and development transport ID | Hard-coded constants; environment-only config | Gives reproducible two-client commands and actionable parse errors at the real process boundary. |
| Visual/user evidence | Distinct client window titles plus structured connection/roster logs | Gameplay rendering or HUD | Sufficient to verify two clients without pulling Milestone 03/06 presentation forward. |

### Cargo and application composition

Keep the single package and two production binaries. Extend the additive features without changing
the supported isolated builds:

```text
client
  ├── existing Bevy client presentation features
  └── Lightyear: client + replication + netcode + udp

server
  ├── existing Bevy headless features
  └── Lightyear: server + replication + netcode + udp

network-test (integration-test-only)
  └── client + server + Lightyear crossbeam
```

`network-test` is a supported test configuration, not a production host-client mode. The integration
harness owns separate server/client `App` values and never runs authoritative and client state in one
`World`. `--all-features` remains outside the documented application command surface.

Application plugin order is a correctness constraint:

```text
Client App
  base plugins (DefaultPlugins or headless test base)
  → Lightyear ClientPlugins { tick_duration: SIMULATION_TICK }
  → GameplayPlugin
  → ProtocolPlugin
  → ClientNetworkPlugin
  → ClientPresentationPlugin (windowed process only)
  → spawn configured client connection entity

Server App
  MinimalPlugins + runner/logging/shutdown
  → Lightyear ServerPlugins { tick_duration: SIMULATION_TICK }
  → GameplayPlugin
  → ProtocolPlugin
  → ServerNetworkPlugin
  → DedicatedServerPlugin
  → spawn configured server endpoint entity
```

The composition root must pass `SIMULATION_TICK` to Lightyear rather than adding a second tick
constant. Protocol registration finishes before endpoint entities are spawned.

### Network protocol

The shared protocol contains only types used in this sandbox:

```text
Constants
  NETWORK_PROTOCOL_ID: u64        nonzero; bumped for incompatible wire changes
  SUPPORTED_PROTOCOL_VERSION      exact application protocol version

SessionChannel                    ordered reliable, bidirectional

Client → server
  ClientHello
    protocol_version
    build_version
    registry_fingerprint

Server → client
  JoinOutcome
    Accepted { player_id, network_entity_id }
    Rejected { reason }

Replicated components
  PlaceholderPlayer               zero-sized marker
  PlayerId(u64)
  NetworkEntityId(u64)
  PlaceholderState { spawn_slot: u64 }
```

Message directions are restricted during registration even though they share a bidirectional
channel. No gameplay input type or gameplay-event channel is registered yet. Lightyear's own
metadata/replication channels remain library-owned.

Compatibility policy for this milestone is exact equality for both Brawler protocol version and
package build version, plus an application-owned Lightyear registry fingerprint. A mismatch
produces `JoinOutcome::Rejected`, no placeholder spawn, a visible client failure status, and
client-initiated disconnect. Lightyear observer errors use a non-panicking log policy; the Brawler
fingerprint rejection is the controlled application outcome. The server applies a deterministic
handshake deadline and cleans a peer that never sends a valid hello. Netcode protocol-ID mismatch
and Netcode request denial are surfaced through `DisconnectedReason` and likewise create no
placeholder.

### ECS ownership and lifecycle

#### Authored/configuration data

- `ServerNetworkConfig`: bind address, maximum accepted sandbox sessions, handshake timeout,
  Netcode protocol ID/private development key, and automation options.
- `ClientNetworkConfig`: server address, local UDP bind address (`127.0.0.1:0` by default), unique
  development Netcode ID, expected compatibility values, and automation options.
- These are process/runtime configuration resources, not replicated gameplay data.
- The development key is explicitly local-only. It is not authentication and must not be presented
  as production security.

#### Server-only state

- `NextSessionIds` resource allocates checked, monotonically increasing `PlayerId` and
  `NetworkEntityId` values starting at 1. Exhaustion rejects the join rather than wrapping.
- `ServerSessionPhase` on each `ClientOf`/`LinkOf` entity records `AwaitingHello`, `Active`, or
  `Rejecting` plus its deadline/assigned IDs. It coordinates lifecycle work; Lightyear's components
  remain connection truth.
- Each accepted connection owns exactly one authoritative placeholder through `ControlledBy`.

#### Client-only state

- `ClientJoinStatus` on the client connection entity records connecting, awaiting outcome, active,
  rejected, or disconnected UI/automation status. It mirrors user-visible progress but does not
  replace Lightyear lifecycle components.
- The client roster is derived from entities carrying Lightyear's receiver-side `Remote` marker and
  the replicated placeholder components. Do not maintain a second authoritative roster resource.

#### Accepted placeholder entity

The server spawns:

```text
PlaceholderPlayer
PlayerId
NetworkEntityId
PlaceholderState
Replicate::to_clients(NetworkTarget::All)
ControlledBy {
  owner: server_connection_entity,
  lifetime: Lifetime::SessionBased,
}
```

Clients receive mapped local Bevy entities. Brawler logic, logs, tests, and future references use
`PlayerId`/`NetworkEntityId`, never equality of Bevy `Entity` values across worlds.

### Lifecycle behavior

#### Server startup

1. Parse and validate configuration before constructing the endpoint.
2. Install the server Lightyear group and Brawler protocol.
3. Spawn the UDP + Netcode server endpoint and trigger `Start`.
4. Emit the bound address, protocol/build version, and 60 Hz tick in structured logs.
5. A successful endpoint has both Lightyear `Started` and transport `Linked`; only then does the
   server write the optional `BRAWLER_SERVER_READY_FILE` readiness sentinel used by the launcher.
6. Bind/link failure is reported as an error `AppExit`; the launcher waits up to 10 seconds for its
   own readiness sentinel and never starts clients against an unproven or pre-existing server.

#### Client connect and acceptance

1. Spawn one configured UDP + Netcode client entity with `ReplicationReceiver`; trigger `Connect`.
2. On real `Connected`, send one `ClientHello` and enter `AwaitingOutcome`.
3. The server adds `ReplicationSender` when the `LinkOf` appears, but does not spawn a player yet.
4. The server validates capacity and compatibility. Duplicate hello messages reuse the prior
   outcome and never allocate another identity.
5. On acceptance, allocate IDs, spawn the authoritative placeholder, mark the session active, and
   send `Accepted`.
6. Both clients derive the same roster from replication. The owning client additionally receives
   Lightyear's local `Controlled` marker.

#### Rejection and timeout

- Netcode protocol mismatch, duplicate active development ID, or request denial transitions the
  client to a visible disconnected/rejected state with no Brawler entity.
- Invalid Brawler hello receives an explicit rejection and the client requests disconnect.
- A connected peer that sends no valid hello before the deterministic deadline is closed and owns
  no placeholder.
- A client unable to reach a server leaves `Connecting` through Netcode's configured timeout and
  enters `Disconnected`. An active client whose automated roster target is not reached by the same
  configured bound also exits nonzero.

#### Disconnect and reconnect

- User-requested disconnect triggers Lightyear `Disconnect` on the client entity.
- When the server connection becomes `Disconnected`, `ControlledBy` session cleanup despawns its
  authoritative placeholder. The despawn replicates to other connected clients.
- Cleanup systems tolerate repeated lifecycle observations and missing entities.
- After the old server connection and owned placeholder are gone, the same development Netcode ID
  may reconnect. The new session receives new Brawler IDs and the current replicated roster.

#### Server shutdown

- Graceful shutdown triggers Lightyear `Stop` on the server endpoint before app exit.
- Connected client links transition through disconnect, session-owned placeholders are removed, and
  the endpoint reaches `Stopped`/`Unlinked` before the bounded shutdown deadline.
- Client window closure and automated success trigger Lightyear `Disconnect` before app exit. The
  forwarding systems run in `Last`, after `Update` producers such as Ctrl-C and window-close
  handling, so the lifecycle request is initiated before Bevy observes the exit. The supervised
  launcher propagates unexpected child failure, rejects a clean server exit before both clients
  complete, requests graceful `SIGINT` shutdown, and escalates only after its bounded cleanup wait.

### Schedule and ordering contract

Use Lightyear's documented schedule flow instead of inventing a parallel network schedule:

```text
PreUpdate
  Lightyear UDP/Crossbeam receive
  → Netcode receive and lifecycle changes
  → message and replication receive/apply

Update
  Brawler hello/outcome processing
  → session phase transitions and authoritative spawn/cleanup requests
  → client roster/status observation

FixedUpdate
  GameplaySet::Input → GameplaySet::Simulation → GameplaySet::Presentation
  (no networked movement systems in this milestone)

PostUpdate
  Lightyear message/replication packet construction
  → Netcode send
  → UDP/Crossbeam send

Last
  Update-produced AppExit forwarding
  → Lightyear Stop/Disconnect
  → wait for Stopped/Disconnected before replaying AppExit
```

Where a system both observes lifecycle state and spawns/despawns an entity needed by replication in
the same frame, declare ordering or apply deferred commands at the narrow boundary that requires
visibility. Tests must prove the externally relevant result; they must not merely inspect that a set
or observer was registered.

### Process configuration and command surface

The implemented command surface provides these capabilities:

```text
brawler-server
  --bind <IP:PORT>                default 127.0.0.1:5000
  --max-clients <N>               bounded positive sandbox capacity
  --handshake-timeout-ms <N>      bounded positive value

brawler-client
  --server <IP:PORT>              default 127.0.0.1:5000
  --client-id <u64>               required by multi-client commands; Netcode-only identity
  --headless                      automation path without winit
  --exit-after-roster <N>         automation-only success condition with timeout
```

Unknown flags, malformed addresses, duplicate launcher client IDs, zero/invalid bounds, bind
failure, and invalid automation combinations fail before an indefinite app run. Window titles and
logs distinguish the two development clients. CLI parsing belongs in binaries/configuration code;
network and session behavior remains in Bevy plugins and testable systems.

`just network-smoke` sets a 30-second process deadline. Windowed `just network` intentionally stays
open for manual disconnect/reconnect playtesting; when one client window closes, the server and
remaining client stay alive. Restart the closed client with its same Netcode-only ID, for example:

```sh
cargo run --locked --no-default-features --features client --bin brawler-client -- \
  --server 127.0.0.1:5000 --client-id 1
```

`BRAWLER_NETWORK_ADDR` selects the server address and
`BRAWLER_NETWORK_TIMEOUT_SECONDS` can add a bounded deadline to the windowed launcher.

Canonical workflow additions:

- `just network` (or the reviewed equivalent): supervised server plus two windowed clients;
- a bounded headless three-process smoke command that returns success only after both clients report
  the same two-player roster;
- a dedicated locked `network-test` integration command.

The launcher must follow Milestone 01's corrected supervision rules and must not hard-code
`target/debug`.

## Trackable implementation plan

### Dependency and composition

- [x] Extend client/server Lightyear features with `replication`, `netcode`, and `udp` while
  preserving isolated production builds.
- [x] Add a dedicated `network-test` feature/target with Crossbeam and a locked CI lane.
- [x] Install `ClientPlugins`/`ServerPlugins` with `SIMULATION_TICK` before `ProtocolPlugin`.
- [x] Re-run and, if needed, strengthen the server feature-isolation script for the new graph.

### Protocol and identity

- [x] Replace the Milestone 01 placeholder protocol registration with the exact session messages,
  channel, and replicated components specified above.
- [x] Add the explicit Netcode protocol ID and supported Brawler protocol/build constants.
- [x] Implement checked server-owned ID allocation without exposing Bevy `Entity` on the wire.
- [x] Add protocol composition and actual transport-round-trip tests.

### Server networking

- [x] Implement validated server configuration and UDP + Netcode endpoint startup.
- [x] Configure new `LinkOf` entities with `ReplicationSender` and pending handshake state.
- [x] Validate hello/capacity exactly once and spawn one owned authoritative placeholder on accept.
- [x] Implement handshake timeout, rejection, idempotent cleanup, reconnect, and graceful stop.

### Client networking and presentation

- [x] Implement validated client configuration and UDP + Netcode connection startup.
- [x] Send hello on real connection, consume outcomes, and expose structured join/disconnect status.
- [x] Derive and log the replicated roster by stable IDs; distinguish development client windows.
- [x] Add bounded headless automation behavior without constructing winit in headless tests.

### Workflow and CI

- [x] Add supervised one-server/two-client local and process-smoke commands using Cargo-resolved
  execution, explicit server readiness, and propagated exit statuses.
- [x] Document individual and combined locked commands plus expected logs/outcomes.
- [x] Add locked CI checks for network-test and retain isolated client/server lint/test/build lanes.
- [x] Record exact automated, UDP, and process evidence; interactive user evidence remains pending
  handoff rather than being inferred from automation.

## Test plan

### Unit and composition tests

- [x] Client and server apps install the proper Lightyear group before identical protocol
  registration; endpoint entities are created only afterward.
- [x] All Brawler message/component/channel registrations and directions are present.
- [x] Invalid CLI configuration fails with actionable errors and valid defaults/overrides round-trip.
- [x] Checked ID allocation is monotonic, unique, starts at 1, and rejects exhaustion without wrap.
- [x] The production server feature graph still excludes rendering, windowing, audio, device input,
  and client assets.

### Deterministic separate-app integration tests

Tests use Netcode over Crossbeam, separate `App` worlds, and simulated Bevy real/fixed time:

- [x] First client completes the real connection and Brawler hello before its server-owned
  placeholder appears.
- [x] Second client joins in progress; both clients receive exactly the same two stable player and
  network-entity IDs, while their local Bevy entity IDs are not used for comparison.
- [x] Server placeholders contain `Replicate` and session ownership; client roster entries are
  receiver-side `Remote` entities. No client creates an authoritative placeholder.
- [x] A mismatched Netcode protocol ID, rejected request, and mismatched Brawler build/version each
  produce a visible failure and zero owned placeholders.
- [x] Deliberately mismatched message, component, or channel registration is detected by
  Lightyear's registry protocol check and cannot produce an accepted Brawler session.
- [x] Connection and Brawler-handshake timeouts cross their thresholds through simulated time, not
  wall-clock sleeping, and leave no owned entities.
- [x] Client disconnect removes its server placeholder and replicates the despawn to the other
  client. Repeating disconnect/cleanup transitions is safe.
- [x] Reconnecting the same development Netcode ID after cleanup creates exactly one fresh player
  with new Brawler IDs and receives the current roster.
- [x] Graceful server stop disconnects both clients and leaves no server connection or placeholder
  entities after the bounded simulated deadline.

### Real UDP and process verification

- [x] A real loopback UDP test uses an OS-assigned server port and proves connection, hello, and one
  replicated placeholder without relying on Crossbeam.
- [x] The supervised headless process harness launches the actual server and two client binaries,
  waits for the harness-owned server readiness sentinel, proves both report the same two-player
  roster, propagates any child failure, rejects premature clean server exit, cleans all children,
  and times out with diagnostics rather than hanging.
- [ ] Interactive `just network` opens two distinguishable responsive client windows, connects both,
  logs the same roster, and shuts all processes down when requested.
- [x] Server bind failure, absent server, duplicate client transport ID, and protocol rejection are
  visible and bounded in the actual process path.

### Evidence rules

- A test named for connection must trigger `Connect` and observe Lightyear's resulting lifecycle.
- A test named for replication must send through Lightyear and observe a receiver-side `Remote`
  entity in another `World`.
- A timeout test must advance simulated time through `App::update()`; directly inserting
  `Disconnected` is only suitable for a narrowly named cleanup-unit test.
- Crossbeam evidence does not count as UDP evidence, and a window launch does not count as a
  two-client replication test.
- All Cargo commands recorded as CI/canonical evidence use `--locked`.

## Verification record

Automated and local process verification completed on 2026-08-13:

- `cargo fmt --all -- --check` passed.
- `cargo clippy --locked --no-default-features --features client --all-targets -- -D warnings`
  passed; the equivalent `server` and `network-test` commands also passed.
- Client feature tests passed: 6 unit tests. Server feature tests passed: 8 unit tests.
- `cargo test --locked --no-default-features --features network-test --test network
  -- --test-threads=1 --nocapture` passed all 10 tests. This includes separate-App Crossbeam
  connection/replication, true join-in-progress, Brawler build and protocol-version rejection,
  controlled Lightyear registry mismatch rejection, active-roster timeout, simulated handshake
  timeout, two-client despawn replication, repeated disconnect cleanup, fresh-ID reconnect, and
  graceful two-client server stop.
- The Lightyear registry mismatch test now uses the production non-panicking Bevy error policy and
  asserts `JoinRejection::RegistryMismatch`, client disconnection, and zero server placeholders.
- `cargo build --locked --no-default-features --features client --bin brawler-client` and the
  equivalent server build passed. `./scripts/check-server-features.sh` passed.
- `RUST_LOG=brawler=info BRAWLER_NETWORK_HEADLESS=1 BRAWLER_NETWORK_ADDR=127.0.0.1:5038
  ./scripts/network.sh` passed with status 0; it launched the actual UDP server and two client
  binaries, waited for both two-player rosters, requested graceful server shutdown, and left no
  Brawler processes. The real UDP integration test also passed using an OS-assigned server port.
- With an independent server already bound to `127.0.0.1:5041`, the supervised collision smoke
  observed the new server's `Address already in use`, did not launch clients, returned status 1,
  and left the independent process untouched. A server endpoint that exits cleanly before readiness
  is likewise mapped to launcher failure rather than smoke success.
- An absent server at `127.0.0.1:59998` logged a structured disconnected transport reason and
  returned status 1 after the configured 5-second bound. Invalid headless configuration and zero
  server capacity returned status 2 with actionable messages. A bind collision returned promptly
  with `Address already in use`.
- Two real UDP clients launched with the same development Netcode ID did not produce two active
  sessions: one completed and the other returned a bounded timeout while the server logged Netcode
  crypto/duplicate-identity warnings.
- A real server process launched with the Cargo-resolved binary handled `SIGINT` through Lightyear
  `Stop`, logged UDP socket closure, and returned status 130. `bash -n scripts/network.sh`,
  `just --dry-run`, and `just --dry-run network-smoke` passed.
- Client and server lifecycle unit tests prove that an `AppExit` produced during `Update` is only
  observed after the `Last`-schedule forwarding systems have initiated Lightyear `Disconnect` or
  `Stop`.

The interactive windowed `just network` smoke remains a user-playtest item because it requires
visual inspection of two macOS windows and manual shutdown.

## Visual and user smoke-test plan

Handoff after implementation will provide one command, expected client titles, expected structured
connection/roster log fields, shutdown instructions, and known limitations. The requested user
observations will be:

- whether both clients connect predictably and show/log the same two stable identities;
- whether closing one client removes only its placeholder from the remaining roster;
- whether restarting that client creates one fresh identity and restores a two-player roster;
- whether closing the clients or pressing Ctrl-C returns to the prompt with no orphan server;
- whether rejection, missing-server, and bind-failure messages are understandable.

No gameplay controls or fighter visuals are expected in this milestone.

## Feedback review

| ID | Feedback | Decision | Rationale | Task/backlog link |
|---|---|---|---|---|
| R1 | Registry mismatch used Bevy's panic policy. | Implemented | Install a non-panicking error handler, add an application-owned registry fingerprint to `ClientHello`, and assert controlled rejection/disconnect. | `tests/network.rs`, registry mismatch test |
| R2 | Production AppExit/launcher shutdown bypassed Lightyear lifecycle. | Implemented | Bridge AppExit to Lightyear `Stop`/`Disconnect`, reset inherited signal dispositions in supervised children, and use bounded graceful cleanup escalation. | `src/client.rs`, `src/server.rs`, `scripts/network.sh` |
| R3 | Active roster automation and launchers could hang or block reconnect playtesting. | Implemented | Add active-roster timeout, launcher deadline, keep windowed sessions alive after one client closes, and document same-ID restart. | `src/client.rs`, `scripts/network.sh`, README |
| R4 | Integration evidence overstated join, despawn, repeat cleanup, and protocol-version coverage. | Implemented | Add true late join, two-client despawn replication, repeated disconnect, protocol-version, and active-roster timeout tests. | `tests/network.rs` |
| R5 | `spawn_slot: u16` imposed an unintended session cap. | Implemented | Use the monotonic `u64` player ID directly as placeholder slot state. | `src/protocol.rs`, `src/server.rs` |
| R6 | Bind failure could leave a server process alive while the smoke clients joined a different pre-existing server. | Implemented | Require a `Started` + `Linked` readiness sentinel before launching clients; fail startup and cleanly supervise the failed child. | `src/server.rs`, `scripts/network.sh` |
| R7 | A clean server exit or late AppExit producer could bypass smoke failure or Lightyear shutdown. | Implemented | Map premature clean server exit to status 1 and run AppExit forwarding in `Last` after `Update` producers, with regression tests. | `scripts/network.sh`, `src/client.rs`, `src/server.rs` |
| — | Awaiting interactive Milestone 02 smoke feedback | Pending | Automated and process verification are complete; window behavior and operator ergonomics still need user observation. | User playtest handoff below |

## Learn from errors

Completed on 2026-08-13:

- A protocol-registration unit test initially omitted the Lightyear role plugin and Bevy
  `StatesPlugin`; component registration then failed for a missing `ProtocolHasher` or state
  schedule. The fix was to build that test with the same minimal foundation and role ordering as
  production apps.
- Rejected links were initially despawned in the same server frame as their reliable rejection
  outcome, so the client could miss the outcome and only observe a disconnect. A one-frame flush
  boundary now separates outcome delivery from rejected-link cleanup.
- Bevy's `AppExit::Error` was initially ignored by both binary entry points, making a timed-out
  client log an error but return status 0. Returning `AppExit` from `main` now propagates the
  bounded failure status to Cargo and the supervisor.
- The registry mismatch path is detected by Lightyear's exact 0.29 protocol check, but the default
  Bevy error handler previously panicked. The production apps now install a non-panicking handler
  and independently exchange a registry fingerprint in `ClientHello` so the server can return a
  structured `RegistryMismatch` outcome before spawning a placeholder.
- AppExit previously ended the process without a network lifecycle transition, while the launcher
  used signals that could bypass Lightyear. Shutdown now routes through `Stop`/`Disconnect`; the
  launcher resets inherited signal dispositions, asks children to handle `SIGINT`, and escalates
  only after a bounded wait.
- The original test harness created all clients before time advanced and did not prove cleanup
  replication to another client. The harness now adds the second client after the first is active and
  keeps a second client through disconnect/reconnect assertions.
- A session-slot conversion to `u16` silently created a lifetime cap unrelated to concurrent server
  capacity. Placeholder state now uses the monotonic `u64` ID without narrowing.
- The fingerprint handshake and widened placeholder state both change serialized wire data, so the
  explicitly versioned Netcode protocol ID was bumped with the implementation.
- A UDP bind error is an observer error, not proof that an endpoint is listening. The launcher now
  waits for an application-owned readiness sentinel written only after `Started` and `Linked`,
  which prevents a pre-existing listener from producing false-positive client results.
- Bevy's runner observes `AppExit` after the main schedule, so forwarding it from `Update` relies on
  registration order among independent producers. Moving the bridge to `Last` makes the lifecycle
  ordering explicit and keeps the request in Lightyear before the runner exits.
- These are repository-specific lifecycle/composition lessons, so no new reusable Codex skill was
  justified. Milestone 03 should continue to verify exact Lightyear ordering and keep prediction
  or input work behind the same server-authoritative integration harness.

## Exit checklist

- [x] Research questions are resolved or explicitly deferred with rationale.
- [x] Technical specification is accepted by the user's direct implementation request.
- [x] All accepted implementation tasks are complete.
- [x] Locked format, lint, test, build, feature-isolation, and network-test commands pass.
- [x] Two real local clients connect and receive the same two server-owned placeholders.
- [x] Rejection, timeout, disconnect, reconnect, and shutdown outcomes are visible and repeatable.
- [x] Rejected/disconnected sessions leave no authoritative owned entities behind.
- [x] Crossbeam deterministic tests and real UDP/process tests each provide evidence for their own
  layer.
- [x] The dedicated server remains headless by dependency graph and runtime behavior.
- [ ] User smoke-test feedback is incorporated or triaged; awaiting the interactive handoff.
- [x] Learn-from-errors review is complete and reusable lessons are captured where justified.
- [x] Roadmap status and current milestone are updated.
