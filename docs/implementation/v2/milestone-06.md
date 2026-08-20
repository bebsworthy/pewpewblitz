# V2 milestone 06 — Reusable match slots, results, and fresh requeue

## Tracking

| Field | Value |
|---|---|
| Status | Implementing |
| Prepared | 2026-08-19; rewritten against the simplified M05 delivered at User playtest |
| Objective | Repeat M05's exact handoff whenever a supervisor-owned match slot is free, while earlier matches continue in isolation, then show the replicated result and let each player join the lobby queue again as a fresh request |
| Entry dependency | Satisfied 2026-08-19: the user validated the simplified specification by directing implementation while M05 remains at User playtest |

## Core outcome

The lobby can start another exact First Blood, Wipeout, or Hot Zone roster while previous match
workers are Active. When a match finishes, its clients see Results, reconnect to the lobby, and may
submit a normal new queue Join. When the worker exits and is reaped, its host slot becomes available
for a later roster.

M06 reuses M05's disposable-attempt rule: if allocation or handoff fails before Active, that attempt
is cleaned up, affected players return to Game Select, and they may join again. No old ticket,
reservation, route, or queue position is restored.

## Small ownership model

```text
Lobby: queue -> one in-flight handoff -> forget at Active
Supervisor: allocate worker -> route -> stop/reap -> publish free slot count
Match worker: Countdown -> Active -> authoritative result -> exit
Client: match result -> Results -> fresh lobby -> optional new Join
```

| Owner | Owns |
|---|---|
| Lobby worker | live lobby sessions, queue tickets, exactly one pre-Active reservation/allocation, latest free-slot count |
| Supervisor | match-worker processes, routes/capabilities, host limits, cleanup, free-slot calculation |
| Match worker | admission, gameplay, departure/forfeit, replicated completion, terminal Result |
| Client | one connection, in-memory route grant while relevant, local Results presentation, navigation choices |

The lobby does not own active matches, results, rewards, match history, reconnect tokens, or returning
player records. The supervisor does not inspect queue order or gameplay results.

## Review of delivered M05

M05 now provides the exact reusable primitive M06 needs:

- exact live 1v1, 2v2, and 3v3 roster formation in the lobby;
- one direct per-player `BeginMatchConnect` with a private grant;
- fresh match connections and immutable manifest admission;
- flat per-game objective and timing values, including one-kill First Blood;
- match-worker-owned check-in and existing authoritative Countdown;
- disposable pre-Active failure with fresh return to Game Select;
- one current product reservation/Active match boundary.

M06 must not reintroduce the discarded M05 designs: reservation offers, roster-wide acknowledgements,
reconnect leases, handoff resumption, automatic requeue, FIFO restoration, failure attribution,
cooldowns, or supervisor prepare/commit activation.

The concrete M05 seams to change are small:

- `ProductFormationState.active` remains the one in-flight handoff;
- `ProductFormationState.occupied` and `QueueState.product_match_occupied` currently stop the lobby
  permanently after Active;
- Active currently removes unrelated overflow tickets and rejects later Join;
- `LobbyState` retains singular pending/allocation/grant fields, which are sufficient when startup is
  serialized but must be cleared after Active;
- the M01 compatibility `allocated_clients` memory must not bound or reject explicit product requeue
  across a long-running lobby;
- the supervisor already owns multiple worker processes, isolated routes, terminal Result handling,
  packet drain, Stop, Exit, reap, and hard worker capacity.

## Decisions

1. **Keep one in-flight handoff.** M06 does not start several workers in the same lobby update and
   does not replace singular reservation/allocation state with maps. Once a worker reports Active,
   the lobby clears that handoff and may form the next roster on a later update.
2. **The supervisor publishes only capacity.** Add one ordered idempotent control fact containing
   `free_match_slots: u8`. Send it after lobby Ready and whenever allocation or actual child reap
   changes capacity. It contains no match, player, or result information.
3. **A slot is reusable only after reap.** Receiving a worker Result is not sufficient. The existing
   Result -> packet drain -> Stop/Exit -> reap path remains the process authority.
4. **Keep overflow queued.** Remove M05's Active overflow deletion, occupied snapshot state, and Join
   rejection. When `free_match_slots == 0`, the lobby simply does not form another reservation.
5. **Use simple cross-pool fairness.** Among complete eligible game pools, form the roster whose first
   selected ticket has the oldest `(admission_order, ticket_id)`; use catalog order only as a tie-break.
6. **Capacity races are disposable.** A stale slot count can still produce an allocation rejection.
   Clean up that M05 attempt and return its players to Game Select. Do not restore their tickets.
7. **Results are client-local presentation.** Capture the replicated authoritative completion before
   unlinking the match. The supervisor's terminal Result remains process lifecycle evidence and is
   not forwarded to the lobby.
8. **Queue Again is a fresh Join.** It may prefill the prior game and accepted build, but it creates a
   new request, ticket, and FIFO position.
9. **Leave is confirmed disconnect.** After a non-destructive confirmation, the client disconnects
   from the match. The match worker's existing connection observer owns participant removal and
   existing shorthanded/empty-team forfeit rules. Add no Leave request/outcome protocol.
10. **Failures stay honest.** A lost match worker returns the client to a fresh lobby with a failure
    notice. It creates neither a result nor queue membership.
11. **Direct UDP remains unchanged.** Its existing restart behavior remains the named comparison
    baseline; routed product matches do not send `ReadyForRestart`.

## Behavior

### Formation and capacity

The supervisor enforces the existing one-lobby-plus-match-workers ceiling and the lobby manifest's
`active_matches` limit. Its free-slot value is the smaller remaining capacity. Control delivery is
ordered, so duplicate equal values are harmless and a newer value replaces the older local value.

The lobby forms only when:

- no M05 reservation/allocation/handoff is currently in flight;
- `free_match_slots > 0`; and
- at least one pool has an exact live eligible roster.

The singular in-flight handoff prevents the lobby from using the same advertised capacity twice;
the supervisor publishes the lower count after registering the worker. On successful Active, the
lobby completes the reservation, clears allocation/grant state, and forgets those participants. It
does not wait for the match to finish and does not retain an active-match record. On failure, normal
M05 cleanup removes the attempted reservation and sends the clients back to Game Select.

### Completion and Results

Before the existing `observe_completed_match` requests match unlink, it copies a bounded result
context from replicated state:

- authoritative result variant;
- the local player's team;
- game identity and presentation name.

The existing selected game and accepted local build state provide UI prefilling only; Results does
not create another match record.

`ClientFlow::Results` displays that immutable local context while the normal fresh lobby connection
is established. It offers:

- **Queue Again** — after lobby authentication/catalog readiness, send the ordinary M04 Join;
- **Change Game** — enter Game Select with the prior game focused when still advertised;
- **Disconnect** — close the current session and return to Server Select.

If completion was not replicated before worker/link loss, Results is not shown. The client returns to
a fresh lobby with a match-server failure notice.

### Leave and failure

During Active, **Leave Match** opens a confirmation with **Keep Playing** focused by default.
Confirming performs the existing intentional match unlink and fresh-lobby transition. The server's
disconnect observation removes the participant; gameplay decides whether the match continues
shorthanded or ends by the existing forfeit rule. The leaver returns to Game Select and receives no
fabricated result.

A lobby-worker crash retains current M05 behavior: the supervisor may stop matches allocated by that
lobby. Restart reconciliation and reconnecting to an active match with the retained client token are
M09 work, not M06.

## Implementation plan

### Slice 1 — Reuse the M05 handoff slot

- [x] Add the supervisor-to-lobby `free_match_slots` fact and enforce manifest/worker limits in its
  calculation.
- [x] Remove global occupied state, Active overflow removal, and occupied Join rejection.
- [x] On Active, clear the singular M05 reservation/allocation/grant state so the next update may
  form one new handoff.
- [x] Ensure explicit product Join/allocation is not limited by M01's process-lifetime identity
  tombstones.
- [x] Select the oldest complete roster across First Blood, Wipeout, and Hot Zone pools.

### Slice 2 — Add the minimal post-match client flow

- [x] Capture replicated completion before unlink and add `ClientFlow::Results`.
- [x] Add Queue Again as ordinary Join, Change Game, and Disconnect.
- [x] Add Leave confirmation that uses intentional disconnect with no new network command.
- [x] Return worker/link failure to a fresh lobby with no result and no automatic Join.

### Slice 3 — Verify reuse and isolation

- [ ] Run simultaneous First Blood/Wipeout/Hot Zone workers through the existing supervisor ceiling.
- [x] Prove queued overflow survives Active and the lobby serially starts later rosters while earlier
  matches run.
- [x] Prove Result, packet drain, Stop/Exit, reap, and later capacity publication clean each worker.
- [ ] Prove repeated Results -> Queue Again creates fresh tickets without growing lobby, route,
  allocation, worker, or client-session state.
- [ ] Run role checks, full tests, direct baseline, real-process concurrency smoke, and focused
  Results/Leave controller checks.

## Implementation and verification evidence

- `just lint` passed formatting, routing/client/server Clippy, and the dedicated-server feature
  isolation check.
- `just test` passed 81 routing unit tests plus routing process/integration tests, 348 client tests,
  299 server tests, 81 serialized network scenarios, and 14 performance gates.
- `just e2e 2`, `just e2e 4`, and `just e2e 6` each reached authoritative Active through the real
  supervisor, lobby, match worker, and routed clients.
- `BRAWLER_PRODUCT_PLAYERS_PER_TEAM=1 BRAWLER_PRODUCT_CLIENT_COUNT=6
  ./scripts/network-product-match.sh` formed three serial First Blood allocations from one lobby;
  all three exact rosters reached Active with three distinct workers alive before shutdown cleanup.
- `BRAWLER_NETWORK_HEADLESS=1 BRAWLER_NETWORK_TIMEOUT_SECONDS=30 ./scripts/network.sh` preserved the
  direct-UDP baseline with both clients exiting successfully.
- `BRAWLER_ROUTED_BIND=127.0.0.1:5021 BRAWLER_PRODUCT_PLAYERS_PER_TEAM=1
  BRAWLER_PRODUCT_CLIENT_COUNT=2 BRAWLER_PRODUCT_REQUEUE_SMOKE=1
  ./scripts/network-product-match.sh` ran two real routed clients through First Blood completion,
  worker result/reap, fresh lobby authentication, Queue Again, and a second authoritative `Joined`
  outcome; both clients exited successfully and the script returned status 0.
- Focused tests cover idempotent capacity publication, capacity decrease while workers exist,
  cross-pool oldest-complete selection, overflow survival, concurrent worker route isolation,
  completion capture, fresh lobby return, and intentional return-to-lobby lifecycle.
- Pending user evidence: Results/Queue Again/Change Game/Disconnect and controller Leave
  confirmation, plus a mixed-mode simultaneous First Blood/Wipeout/Hot Zone playtest.

## Playtest feedback

- **Implemented now — Queue Again did nothing from a Draw Results screen.** Results had relied on
  the older `SelectedGameType` resource, so a cleared or stale selection made the action return
  silently. The later `The queue connection is unavailable` failure had a separate concrete cause:
  the action query required `RuntimeLobbyTarget`, but production fresh-lobby entities created after
  a match do not carry that server-select-only component. The query therefore discarded the valid,
  authenticated lobby. The component is now optional; Queue Again selects the current routed lobby
  session, binds the queue model to that session's generation, and uses the game ID retained from the
  authoritative successful Join. The focused test deliberately omits `RuntimeLobbyTarget`, clears
  UI/result game identity, overlaps retiring match and fresh lobby entities, and mismatches lifecycle
  generation. A real two-client process smoke now covers the complete match-to-requeue path and only
  succeeds after both clients receive the fresh `Joined` outcome.

## Verification contract

- only one reservation/allocation handoff is in flight per lobby;
- the number of Active plus starting match workers never exceeds configured capacity;
- full capacity leaves existing queue membership untouched and forms nothing;
- an Active handoff clears all lobby state specific to that match;
- a stale-capacity rejection follows disposable M05 failure and restores nothing;
- oldest complete cross-pool selection is deterministic;
- simultaneous workers have distinct manifests, routes, capabilities, Worlds, and traffic;
- replicated completion enters Results exactly once before match teardown;
- Queue Again creates a new request, ticket, and admission order;
- confirmed Leave and unexpected disconnect use the same authoritative departure path;
- worker failure never invents a result or queue membership;
- capacity increases only after supervisor-owned child reap;
- repeated cycles return processes, routes, capabilities, allocations, queues, and client entities to
  declared bounds;
- server features remain free of rendering, windowing, audio, device input, and client assets; the
  routing package remains Bevy-free.

## Exit criteria

- [ ] M05 playtest feedback is triaged and the user validates this specification.
- [x] Multiple isolated matches can be Active while the lobby keeps queueing and serially handing off
  later exact rosters.
- [x] The lobby stores no active-match or returning-player records.
- [x] Worker reap publishes reusable capacity; no result/replay protocol reaches the lobby.
- [ ] Results, Queue Again, Change Game, Disconnect, and confirmed Leave work through one current
  connection at a time.
- [x] No M05 discarded recovery or activation-arbitration machinery is reintroduced.
- [ ] Required automated, process, direct-baseline, and focused manual evidence passes.
- [ ] User playtest feedback and closeout learning are recorded before completion.

## Deferred

- lobby/supervisor restart recovery and reconnect-to-active-match hardening (M09);
- final Results styling, scoreboard, combat menus, and accessibility polish (M07);
- rewards, accounts, durable match history, rematch, parties, spectators, join-in-progress, and global
  matchmaking (outside v2).
