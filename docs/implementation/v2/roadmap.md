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
- By explicit user direction on 2026-08-18, M02 is the bounded exception: its specification was
  prepared during M01 verification. M01 completed on 2026-08-19, the user validated M02 on
  2026-08-19, and the milestone returned to `Implementing` after its first implementation review.
- By explicit user direction on 2026-08-19, M03 specification research overlapped M02's user
  playtest/review. The user subsequently accepted M02 and directed M03 implementation on
  2026-08-19; M02 is complete, and M03 completed after accepted playtest fixes on 2026-08-19.
- By explicit user direction on 2026-08-19, M04 research and planning overlapped M03
  implementation. Its specification was reconciled against M03's delivered seams, the user
  subsequently directed implementation, and M04 completed on 2026-08-19 after five review passes
  and explicit user closeout.
- By explicit user direction on 2026-08-19, M05 research and specification preparation overlapped
  M04 implementation. Its simplified implementation and automated verification are complete, so
  M05 is now at `User playtest`.
- By explicit user direction on 2026-08-19, M06 research and specification preparation overlap M05
  implementation. M05 remains the current delivery milestone; M06 is at `Specification review` with
  a simplified lobby-only queue/formation boundary. Production implementation is not authorized
  until M05 verification completes and the user validates the M06 specification.
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
| Status | User playtest |
| Current milestone | M05 — Exact formation and match handoff |
| Entry gate | Satisfied 2026-08-18: V1 M11 is complete, the user accepted the basic v1 MVP, release polish is explicitly deferred, and the worker-readiness audit plus reproducible direct-UDP baseline are delivered with no v2 blocker. The user validated the M01 specification by directing implementation on 2026-08-18 |
| Completion gate | Direct-connect, queue, isolated match, results/requeue, and practice pass automated, process, network, controller, visual, accessibility, recovery, and capacity evidence |

## Milestone overview

| Milestone | Status | Deliverable | Plan |
|---|---|---|---|
| 01 | Complete | Reusable routed multi-process server foundation | [milestone-01.md](./milestone-01.md) |
| 02 | Complete | Product client shell, navigation, settings, and persistence | [milestone-02.md](./milestone-02.md) |
| 03 | Complete | Direct-connect lobby session and advertised game selection | [milestone-03.md](./milestone-03.md) |
| 04 | Complete | Product build editor and authoritative queue admission | [milestone-04.md](./milestone-04.md) |
| 05 | User playtest | Exact formation and match handoff | [milestone-05.md](./milestone-05.md) |
| 06 | Specification review | Concurrent match lifecycle, results, and requeue | [milestone-06.md](./milestone-06.md) |
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

Replace the auto-connect/terminal windowed entry with a functional controller-friendly Title,
Settings, Credits, one local-error overlay, visible focus, simple styling/motion, and versioned atomic
local settings. Preserve headless automation and authoritative networking. Defer the general screen
flow and richer UI framework until later milestones demonstrate a need.

Gate: controller and keyboard/mouse can operate every delivered action with visible focus; settings
survive restart and fail safely; three representative layouts remain usable; and explicit headless,
direct, and routed auto-connect paths retain their accepted behavior. See
[milestone-02.md](./milestone-02.md).

### M03 — Direct-connect lobby session and advertised game selection

Add validated address entry, favorites, recents, generated/session display names, recoverable
connection lifecycle, compatibility checking, bounded game-type advertisement, and GameSelect.

Gate: a player can connect, inspect server-owned game types, disconnect, retry, and select another
server without exiting the application.

### M04 — Product build editor and authoritative queue admission

Turn the debug build overlay into a bounded editor with local last-used persistence. Add server
validation at admission, immutable ticket build snapshots, exact game-type FIFO pools, honest
aggregate state, cancellation, overflow, idempotency, bounded command retry, and disconnect cleanup.

Gate: players can edit, correct rejected builds, join/cancel queues, and remain safe under
duplicates, command retries, bounded reliable outcomes, stale snapshot delivery, and disconnects;
the editor communicates each tradeoff with focused before/after changes, equivalent re-Join
preserves the original ticket/admission revision, one ordered client envelope preserves
acknowledgement/command order, pending identical recovery is token- and wire-copy-neutral at the
bounded ten-second Retry cadence, repeated early duplicates disconnect, and the first over-rate new
command fails softly before continued abuse disconnects. Only sessions authenticated at frame start
may issue queue commands, and byte-equivalent current-revision refreshes renew client freshness while
older snapshots do not. Privacy-safe bounded diagnostics make admission, cleanup, abuse, snapshot
publication intent, and client freshness aging observable. M04 ends before
ticket reservation exists, so it does not fabricate a formation-boundary race.

### M05 — Exact formation and match handoff

Form exact 1v1/2v2/3v3 rosters, assign teams deterministically, select a compatible map, reserve
tickets, admit host capacity, start/validate a worker, issue routing capabilities, establish fresh
worker connections, synchronize match state, and check in before the one authoritative countdown.
M05 exposes one honest product match per logical-server process: after Active it pauses admission,
capacity-removes queued overflow, suppresses legacy fixed-roster restart, and retains only minimal
completed-match Disconnect presentation until M06 supplies reusable lifecycle.

Gate: exact 1v1, 2v2, and 3v3 rosters reach an existing Wipeout or Hot Zone worker through
the public endpoint and enter the server-owned Countdown. A failed pre-Active start discards its
reservation and returns clients to a clean Game Select; no exact roster remains invisibly stranded
behind the temporary one-match product slot.

### M06 — Concurrent match lifecycle, results, and requeue

Replace M05's one-match admission pause with multiple heterogeneous workers and reusable completion.
Complete leave/forfeit, worker result, route cleanup, return-to-lobby, Results, Queue Again, Change
Game, worker crash, supervisor shutdown, and cross-match isolation behavior.

M06 keeps the lobby limited to current sessions, queue tickets, and pre-handoff reservations. The
supervisor owns worker capacity and cleanup; match workers own gameplay/results; clients retain their
in-memory route grant and result presentation. There is no lobby active-match registry, result
forwarding, terminal replay, returning-player recognition, or cross-generation reconciliation.
Queue Again is an ordinary fresh queue Join.

Gate: simultaneous Wipeout and Hot Zone matches cannot leak state or traffic, and repeated
formation/completion/failure returns routes, processes, memory, and queues to bounds.

### M07 — Combat HUD, menus, readability, and accessibility

Replace debug presentation with product combat HUD, score/objective display, scoreboard,
non-pausing menu, results, non-color team identity, UI scaling, reduced effects, audio/display
settings, and supported-layout validation.

M07 polishes and extends M06's functional Results and minimal Leave surface; it does not replace
M06's result, return-to-lobby, or Queue Again authority contracts.

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
reconnect-to-lobby, soak/growth evidence, routed performance/IPC/MTU measurement and optimization,
usability flows, attribution, and feedback/learning
records. Review and remove the legacy v1 direct-UDP executable/configuration after the final
comparison evidence unless a documented compatibility requirement remains. This closes gaps; it
does not add global matchmaking or fleet services.

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
| V2-TRANSPORT-CONTINGENCY | Bounded worker-port range using standard Lightyear UDP | Costed in M01. A qualifying routed hard-gate failure may only return M01 to specification review with evidence; selecting or implementing this contingency requires the user's express approval |
| V2-WINDOWS-IPC | Production Windows named-pipe backend | Preserve the contract in M01; implement when Windows becomes an active target |
| V2-ROUTE-RESUMPTION | Resume an interrupted match connection | Deferred; v2 returns to a fresh lobby session |
| V2-ROUTED-HARDENING | Complete routed latency, packet-only IPC overhead, correlated CPU, dual-stack MTU capture, 25/20-cycle campaigns, and optimize the latest recorded +12.31% directional egress delta | Deferred from M01 to M09 by the user-approved 2026-08-19 development-use scope decision; measurements retain their failed/unsupported labels until rerun |
| V2-M03-MANUAL-MATRIX | Broader physical-controller feel and full aspect/UI-scale matrix for the direct-connect lobby | Deferred by explicit M03 closeout acceptance on 2026-08-19. Automated controller lifecycle regressions and representative native layouts passed, but separate physical-controller/full-matrix execution is not claimed; revisit in M07's supported-layout/controller validation |
| V2-M04-MANUAL-MATRIX | Representative resolution/UI-scale inspection and separate physical-controller feel matrix for Game Select, Build Editor, Queue, and recovery overlays | Deferred by explicit M04 closeout direction on 2026-08-19. Automated input, focus, authority, recovery, and presentation regressions passed, but physical-controller/full-layout execution is not claimed; revisit with M03's matrix in M07's supported-layout/controller/accessibility validation |
| V2-V1-DIRECT-UDP-RETIREMENT | Retire the legacy v1 direct-UDP launch path | M01 makes the routed supervisor path the default for `network.sh`/`just network` after validating its minimum transition driver and retains direct UDP only as an explicitly named compatibility/baseline command; M09 removes the legacy executable/configuration after final comparison evidence unless a documented compatibility requirement remains |

## Explicitly deferred beyond v2

- global/cross-server matchmaking, rank, skill rating, and leaderboards;
- public registry, NAT traversal, relay, and production internet reachability;
- accounts, authentication services, parties, invitations, cloud saves, and entitlements;
- join-in-progress, match resumption, host migration, and spectators;
- production fleet scheduling, autoscaling, orchestration APIs, moderation, and administration;
- complete terrain-theme, character-rig/skin, VFX, and production-art replacement.
