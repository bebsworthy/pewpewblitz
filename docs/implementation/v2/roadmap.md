# Version 2 implementation roadmap

## Purpose and scope

V2 turns the server-authoritative gameplay MVP into a coherent player experience with direct-connect
server selection, a product build flow, server-local matchmaking, isolated concurrent matches,
practice, and readable product UI. The feature boundary is
[Player UX and server-local matchmaking](../../13-player-ux.md); the hosting decision is
[Multi-process server and single-port UDP/IPC transport](../../14-multiplayer-server-architecture.md).

V2 excludes global matchmaking, public discovery, accounts, parties, rank, session resumption,
join-in-progress, fleet orchestration, and the complete production-art pipeline.

## Delivery rules

- Every milestone starts with R&D against the then-current code and exact pinned dependencies.
- Only the next milestone receives a detailed file. Later entries define outcomes, ordering,
  dependencies, and gates—not premature technical specifications.
- A milestone moves from `Researching` to `Specification review`; user validation is required before
  it moves to `Implementing`.
- Each milestone delivers a production-reusable vertical increment and extends shared process,
  network, UI, and test harnesses. No disposable spike or parallel gameplay path is accepted.
- Server authority, stable identity, bounded state, deterministic lifecycle, and recoverable client
  UX remain non-negotiable.
- Accepted scope changes update this roadmap and the active milestone before implementation.

Allowed statuses are `Not started`, `Researching`, `Specification review`, `Implementing`,
`Verifying`, `User playtest`, `Feedback review`, `Complete`, and `Blocked`.

## Version status

| Field | Value |
|---|---|
| Status | Researching |
| Current milestone | M01 — Routed multi-process server foundation |
| Entry gate | V2 research and planning may proceed while V1 M11 is in specification review; production implementation waits for M11 closeout—including its worker-readiness audit/direct-UDP baseline—and M01 specification validation |
| Completion gate | Direct-connect, queue, isolated match, results/requeue, and practice pass automated, process, network, controller, visual, accessibility, recovery, and capacity evidence |

## Milestone overview

| Milestone | Status | Deliverable | Plan |
|---|---|---|---|
| 01 | Researching | Reusable routed multi-process server foundation | [milestone-01.md](./milestone-01.md) |
| 02 | Not started | Product client shell, navigation, settings, and persistence | Create when next |
| 03 | Not started | Direct-connect lobby session and advertised game selection | Create when next |
| 04 | Not started | Product build editor and authoritative queue admission | Create when next |
| 05 | Not started | Exact formation, worker allocation, and match loading/handoff | Create when next |
| 06 | Not started | Concurrent match lifecycle, results, and requeue | Create when next |
| 07 | Not started | Combat HUD, menus, readability, and accessibility | Create when next |
| 08 | Not started | Authoritative bot practice and local supervisor launch | Create when next |
| 09 | Not started | Recovery, security, capacity, usability, and v2 closeout | Create when next |

## Ordering rationale and milestone gates

### M01 — Reusable routed multi-process server foundation

Retire the highest architectural risk first. Deliver the real supervisor/router, versioned packet
and control contracts, isolated lobby- and match-worker lifecycles, Lightyear IPC transport seam,
sequential lobby/match reconnection path, process harness, and measurements using one lobby worker
and at least one match worker.
Production and tests consume the same components. Matchmaking and product UI are not required beyond
the minimum driver needed to prove route transition.

Gate: [milestone-01.md](./milestone-01.md) thresholds pass, or the bounded worker-port-range
contingency returns to specification review with evidence.

### M02 — Product client shell, navigation, settings, and persistence

Replace auto-connect/terminal windowed flow with controller-first Title, Settings, Credits, error
overlay, flow/overlay state separation, focus restoration, device glyphs, and versioned atomic local
settings. Preserve headless automation and authoritative networking.

Gate: controller and keyboard/mouse navigate every implemented screen and recover from local
configuration errors without restarting the client.

### M03 — Direct-connect lobby session and advertised game selection

Add validated address entry, favorites, recents, generated/session display names, recoverable
connection lifecycle, compatibility checking, bounded game-type advertisement, and GameSelect.

Gate: a player can connect, inspect server-owned game types, disconnect, retry, and select another
server without exiting the application.

### M04 — Product build editor and authoritative queue admission

Turn the debug build overlay into a bounded editor with local last-used persistence. Add server
validation at admission, immutable ticket build snapshots, exact game-type FIFO pools, honest
aggregate state, cancellation, overflow, idempotency, and race handling.

Gate: players can edit, correct rejected builds, join/cancel queues, and remain safe under
duplicates, disconnects, and formation-boundary races.

### M05 — Exact formation, worker allocation, and match loading/handoff

Form exact 2v2/3v3 rosters, assign teams deterministically, select a compatible map, reserve
tickets, admit host capacity, start/validate a worker, issue routing capabilities, establish fresh
worker connections, synchronize match state, and check in before the one authoritative countdown.

Gate: a complete roster reaches an existing Wipeout or Hot Zone worker through the public endpoint;
failures dissolve or requeue reservations under an explicit bounded policy.

### M06 — Concurrent match lifecycle, results, and requeue

Run multiple heterogeneous workers concurrently. Complete leave/forfeit, worker result, route
cleanup, return-to-lobby, Queue Again, Change Game, worker crash, supervisor shutdown, and
cross-match isolation behavior.

Gate: simultaneous Wipeout and Hot Zone matches cannot leak state or traffic, and repeated
formation/completion/failure returns routes, processes, memory, and queues to bounds.

### M07 — Combat HUD, menus, readability, and accessibility

Replace debug presentation with product combat HUD, score/objective display, scoreboard,
non-pausing menu, results, non-color team identity, UI scaling, reduced effects, audio/display
settings, and supported-layout validation.

Gate: automated layout/state tests and supervised controller/keyboard playtests show readable
combat and complete navigation at supported resolutions and accessibility settings.

### M08 — Authoritative bot practice and local supervisor launch

Expose a first-run Practice path using the same supervisor/worker topology, normal validation, and
routed Lightyear connection. Start an explicitly named bot game and shut down/reap children
predictably. PvP queues never insert bots silently.

Gate: a fresh client reaches controllable authoritative practice within one minute, with no orphan
process or alternate authority path after normal or failed shutdown.

### M09 — Recovery, security, capacity, usability, and v2 closeout

Harden abuse boundaries, impaired-network behavior, diagnostics, host ceilings,
reconnect-to-lobby, soak/growth evidence, usability flows, attribution, and feedback/learning
records. This closes gaps; it does not add global matchmaking or fleet services.

Gate: the version completion gate and every prior deferred exit observation have evidence and user
feedback disposition.

## Cross-version technical policies

### Authority and process ownership

- The supervisor routes and supervises; it does not simulate or decode gameplay.
- The lobby is a long-lived isolated worker by default; embedding it in the supervisor requires an
  evidence-backed specification change.
- A match worker owns one authoritative Bevy world and composes the v1 gameplay implementation.
- Lobby and match connections are distinct Lightyear sessions at one public endpoint.
- Stable typed IDs and bounded versioned frames cross IPC; process-local Bevy entities and handles
  do not.
- The client never becomes authority, including local practice.

### Test and measurement policy

Every applicable milestone adds the lowest useful evidence:

- pure parser, validation, routing, queue, and transition tests;
- small Bevy `App`/`World` schedule and plugin-composition tests;
- deterministic in-memory routed-transport tests;
- real UDP plus real child-process IPC integration tests;
- impairment, backpressure, crash, cleanup, duplicate, stale, and malformed-input tests;
- CPU, memory, fixed-tick, packet, queue, drop, handoff, and startup measurements;
- visual/controller/accessibility checks where presentation is involved;
- repeated lifecycle soaks without wall-clock sleeps inside deterministic ECS tests.

### Scope discipline

The routed transport is a concrete external boundary and may justify a focused module or crate after
M01 research. It must not cause duplicate gameplay DTOs, a supervisor-side world model, or a
general distributed-services framework. Later milestones extend demonstrated APIs rather than
generalizing for global matchmaking, orchestration, accounts, or spectators.

## V2 backlog

| ID | Item | Disposition |
|---|---|---|
| V2-TRANSPORT-CONTINGENCY | Bounded worker-port range using standard Lightyear UDP | Cost the reference design before M01 specification validation; implement only if accepted single-port routing fails a hard threshold after bounded optimization |
| V2-WINDOWS-IPC | Production Windows named-pipe backend | Preserve the contract in M01; implement when Windows becomes an active target |
| V2-ROUTE-RESUMPTION | Resume an interrupted match connection | Deferred; v2 returns to a fresh lobby session |

## Explicitly deferred beyond v2

- global/cross-server matchmaking, rank, skill rating, and leaderboards;
- public registry, NAT traversal, relay, and production internet reachability;
- accounts, authentication services, parties, invitations, cloud saves, and entitlements;
- join-in-progress, match resumption, host migration, and spectators;
- production fleet scheduling, autoscaling, orchestration APIs, moderation, and administration;
- complete terrain-theme, character-rig/skin, VFX, and production-art replacement.
