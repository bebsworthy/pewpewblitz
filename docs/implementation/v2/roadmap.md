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
- By explicit user direction on 2026-08-19, M06 research, specification, and implementation overlap
  M05 playtest. The delivered slice keeps one serialized M05 handoff at a time, reusable
  supervisor-owned worker capacity, client-local Results, and no active-match/recovery ownership in
  the lobby. After the reported Queue Again path was fixed and passed a real two-client routed
  completion-to-fresh-Join smoke, the user explicitly directed M06 closeout on 2026-08-20. The
  unexecuted manual-flow and repeated heterogeneous lifecycle campaigns remain visible backlog work.
- By explicit user direction on 2026-08-19, M07 research and specification preparation overlap M06
  implementation. M06 is now complete and its seams are reconciled. The user validated the revised
  M07 specification and directed implementation on 2026-08-20. After the reported native HUD
  query-conflict panic was fixed and the exact `just run 2` startup remained stable, the user
  explicitly directed M07 closeout on 2026-08-20. The unexecuted native layout, physical-controller,
  and perceptual-audio matrix remains visible as `V2-M07-MANUAL-MATRIX`.
- By explicit user direction on 2026-08-20, M08 research and specification preparation overlapped
  M07 implementation. Multiplayer queues remain unchanged; Practice selects any compatible game
  type and fills its non-human roster positions outside the queue. The server-hosted inert-bot slice
  and automated verification are complete. On 2026-08-20 the user accepted the playtest after four
  gameplay rounds covering bots and multiple clients; M05 and M08 are complete.
- By explicit user direction on 2026-08-20, M09 was reduced to the minimum v2 closeout: one
  canonical regression pass, disposition of any blocking defect it finds, and truthful closeout
  records. Production optimization, exhaustive campaigns and matrices, internet-facing hardening,
  resumption, and legacy direct-UDP retirement remain backlog work.
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
| Status | Complete |
| Current milestone | None — v2 is complete |
| Entry gate | Satisfied 2026-08-18: V1 M11 is complete, the user accepted the basic v1 MVP, release polish is explicitly deferred, and the worker-readiness audit plus reproducible direct-UDP baseline are delivered with no v2 blocker. The user validated the M01 specification by directing implementation on 2026-08-18 |
| Completion gate | The canonical regression suite passes, any blocking regression is resolved or explicitly blocks closeout, and v2 limitations and learning are recorded truthfully |

## Milestone overview

| Milestone | Status | Deliverable | Plan |
|---|---|---|---|
| 01 | Complete | Reusable routed multi-process server foundation | [milestone-01.md](./milestone-01.md) |
| 02 | Complete | Product client shell, navigation, settings, and persistence | [milestone-02.md](./milestone-02.md) |
| 03 | Complete | Direct-connect lobby session and advertised game selection | [milestone-03.md](./milestone-03.md) |
| 04 | Complete | Product build editor and authoritative queue admission | [milestone-04.md](./milestone-04.md) |
| 05 | Complete | Exact formation and match handoff | [milestone-05.md](./milestone-05.md) |
| 06 | Complete | Concurrent match lifecycle, results, and requeue | [milestone-06.md](./milestone-06.md) |
| 07 | Complete | Minimal combat HUD, menus, readability, and accessibility | [milestone-07.md](./milestone-07.md) |
| 08 | Complete | Server-hosted bot practice for any game type | [milestone-08.md](./milestone-08.md) |
| 09 | Complete | Minimal v2 closeout | [milestone-09.md](./milestone-09.md) |

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

M06 keeps one M05 reservation/allocation handoff in flight at a time, clears it at Active, and starts
another roster when the supervisor advertises a free worker slot. Match workers own gameplay and
results; clients retain local Results presentation. There is no lobby active-match registry, result
forwarding, terminal replay, returning-player recognition, or cross-generation reconciliation.
Queue Again is an ordinary fresh queue Join, and confirmed Leave uses the existing disconnect path.

Gate: simultaneous Wipeout and Hot Zone matches cannot leak state or traffic, and repeated
formation/completion/failure returns routes, processes, memory, and queues to bounds.

### M07 — Combat HUD, menus, readability, and accessibility

Replace debug presentation with a minimal product combat HUD, a top-right mode-owned score/objective
slot, scoreboard, non-pausing menu, results, non-color team identity, UI scaling, reduced effects,
audio/display settings, and supported-layout validation. Gameplay-relevant information stays in the
product HUD; connection, input, identity, tick, entity, and similar development facts stay behind
the separate diagnostics mode.

M07 polishes and extends M06's functional Results and minimal Leave surface; it does not replace
M06's result, return-to-lobby, or Queue Again authority contracts.

Gate: automated layout/state tests and supervised controller/keyboard playtests show readable
combat and complete navigation at supported resolutions and accessibility settings.

### M08 — Server-hosted bot practice for any game type

Expose Practice from Title through the normal server selection/connection path. Let the player
select any compatible server-advertised game type and request an immediate authoritative match
worker with bots filling the remaining roster. The request uses ordinary supervisor capacity but
never creates a queue ticket or changes multiplayer pool formation; PvP queues remain human-only
in M08. Bots are inert authoritative fighters identified only by ordinary `Bot N` display names;
all AI is deferred beyond v2. The client does not launch or package server processes.

Gate: a connected client can select any compatible game type and reach controllable authoritative
practice within one minute when server capacity is available, complete the normal match/Results
flow, and exit through the normal disconnect path with no queue mutation or alternate authority
path.

### M09 — Minimal v2 closeout

Run the canonical regression suite once against the completed v2 product path, fix only a blocking
regression it exposes, and record final limitations, feedback disposition, and reusable learning.
M09 adds no product feature, architecture, optimization target, soak campaign, exhaustive manual
matrix, production hardening, or legacy-path cleanup.

Gate: [milestone-09.md](./milestone-09.md) passes and the user accepts v2 closeout.

## Cross-version technical policies

### Authority and process ownership

- The supervisor routes and supervises; it does not simulate or decode gameplay.
- The lobby is a long-lived isolated worker by default; embedding it in the supervisor requires an
  evidence-backed specification change.
- A match worker owns one authoritative Bevy world and composes the v1 gameplay implementation.
- Lobby and match connections are distinct Lightyear sessions at one public endpoint.
- Stable typed IDs and bounded versioned frames cross IPC; process-local Bevy entities and handles
  do not.
- The client never becomes authority, including Practice.

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
| V2-ROUTE-RESUMPTION | Resume an interrupted match connection | Backlog; v2 returns to a fresh lobby session. Revisit only when continuity becomes a product requirement |
| V2-ROUTED-HARDENING | Routed latency and packet-only IPC measurement, correlated CPU campaigns, dual-stack MTU capture, and optimization of the recorded +12.31% directional egress delta against the former 10% target | Backlog; no current player-visible or correctness problem justifies optimization. Revisit with a real deployment target or measured bottleneck |
| V2-HOSTING-HARDENING | Fleet-scale capacity/security work, production credentials, and internet-facing hardening | Backlog; revisit when public or production hosting enters scope |
| V2-M03-MANUAL-MATRIX | Broader physical-controller feel and full aspect/UI-scale matrix for the direct-connect lobby | Backlog; automated controller lifecycle regressions and representative native layouts passed. Revisit only for a supported-platform or usability release gate |
| V2-M04-MANUAL-MATRIX | Representative resolution/UI-scale inspection and separate physical-controller feel matrix for Game Select, Build Editor, Queue, and recovery overlays | Backlog; current automated input, focus, authority, recovery, and presentation regressions passed |
| V2-M06-MANUAL-FLOW | Physical-controller Results actions and confirmed Leave presentation/feel matrix | Backlog; automated navigation/lifecycle evidence and real Queue Again passed end to end |
| V2-M06-LIFECYCLE-SOAK | Simultaneous heterogeneous modes plus repeated completion/requeue bounded-growth campaigns | Backlog; deterministic isolation/capacity tests, cleanup, serial multi-worker evidence, and hands-on multiple-client play passed. Revisit when host scale or a leak warrants it |
| V2-M07-MANUAL-MATRIX | Exhaustive native layout, contrast, physical-controller, audio, and non-pausing-menu matrix | Backlog; revisit for a supported-platform release gate, not v2 closeout |
| V2-V1-DIRECT-UDP-RETIREMENT | Retire the legacy v1 direct-UDP launch path | Backlog housekeeping; remove when its comparison/debug value is gone, without making another optimization campaign a prerequisite |

## Explicitly deferred beyond v2

- bot AI, including movement, combat decisions, objective play, difficulty, and competitive tuning;
- global/cross-server matchmaking, rank, skill rating, and leaderboards;
- public registry, NAT traversal, relay, and production internet reachability;
- accounts, authentication services, parties, invitations, cloud saves, and entitlements;
- join-in-progress, match resumption, host migration, and spectators;
- production fleet scheduling, autoscaling, orchestration APIs, moderation, and administration;
- complete terrain-theme, character-rig/skin, VFX, and production-art replacement.
