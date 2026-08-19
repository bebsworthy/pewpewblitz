# V2 milestone 05 — Exact formation, worker allocation, and match loading/handoff

## Tracking

| Field | Value |
|---|---|
| Status | Specification review |
| Prepared | 2026-08-19; research and specification preparation overlapped M04 implementation by explicit user direction |
| Objective | Turn exact authoritative queue pools into deterministic 2v2 or 3v3 reservations, allocate one compatible isolated match worker through the existing supervisor, hand every retained participant to a fresh worker connection, synchronize and check in the complete roster, and begin exactly one authoritative countdown |
| Entry dependency | Pending: M04 must complete and its delivered queue/editor/client-flow seams must be reconciled before M05 implementation |
| Scope authority | Research and specification only. Production implementation requires user validation after final M04 reconciliation |

M04 remains the current delivery milestone. This document makes the next boundary reviewable while
M04 is being built; it does not authorize reservation, allocation, routed-control, worker, client,
or gameplay changes.

## Player-visible outcome

When the oldest exact roster exists for an advertised game type, the lobby moves those players from
Queue to Match Loading as one atomic reservation. Match Loading names the accepted game type,
selected map, topology, and the player's accepted build. It reports a small honest phase—reserving,
starting server, connecting, synchronizing map/terrain, or waiting for players—without exposing
capabilities, route IDs, process IDs, internal retries, or another player's build.

The supervisor admits host capacity, starts one isolated authoritative match worker, and returns one
short-lived route grant per reserved lobby session only after the worker validates its immutable
manifest and reports Ready. Each client then closes its lobby Lightyear session and creates one fresh
match Lightyear session through the same public UDP endpoint. The worker admits only identities in
the manifest. A client checks in only after its match handshake is accepted, the authoritative map
snapshot is reconstructed, the matching terrain generation has converged, required client assets
are ready, and its controlled fighter/build state matches the manifest-backed admission.

The worker starts the existing authoritative countdown only after every manifest participant is
connected and checked in. There is no second client-side countdown and no ready-up prompt in the
product routed flow. The direct-UDP development baseline retains its current explicit ready path.

Cancellation, disconnect, worker refusal/failure, route expiry, and loading timeout never leave an
invisible reservation. Before activation, the cancelling or missing ticket is removed and every
still-valid retained ticket returns to the front of its original pool in original admission order.
Infrastructure failure requeues the whole retained roster. The Queue screen then resumes with a
fresh authoritative population; a bounded notice explains why loading did not complete.

M05 ends when a complete roster reaches Countdown in one worker. Match completion, leave/forfeit,
results, Queue Again, Change Game, worker-result reconciliation, and repeated concurrent-match
cleanup are M06.

## Research findings

### Existing reusable foundation

- M04's in-progress `QueueState` already owns one immutable `QueueTicket` per authenticated lobby
  session, exact game identity/revision, the accepted public build summary, the complete resolved
  loadout, monotonic admission order, FIFO pools, idempotent Join/Cancel memory, disconnect cleanup,
  aggregate revision, and bounded telemetry. Formation should extend this authority; it must not
  introduce a second queue in the supervisor or transition driver.
- The operator catalog already resolves stable game IDs, modes, compatible map lists, exact
  topology, rules summaries, and a canonical catalog revision at lobby-worker startup. It currently
  rejects everything except 2v2. M05 is the milestone that must validate and exercise the roadmap's
  exact 2v2/3v3 boundary.
- Both embedded production maps currently provide four spawn points for each of two teams. They can
  satisfy 2v2 and 3v3 without authored map changes, but startup validation must prove the selected
  topology against every advertised map rather than relying on that observation.
- `ResolvedMatchCapacity` already derives checked team/fighter capacity and validates exact team
  slots plus per-team spawn capacity. Terrain is already bounded to 24 active fighters, and the
  server configuration admits eight clients. The remaining blockers are hard-coded 2v2 checks in
  lobby validation, lifecycle rules, and match-manifest admission.
- `brawler-routing` already supplies bounded IDs, an exact canonical match manifest, worker process
  supervision, manifest/Ready validation, host worker/route ceilings, capability minting and
  expiry, grant delivery to the lobby control adapter, routed packet isolation, and deterministic
  in-memory plus real-process harnesses.
- The M01 allocation request and supervisor runtime deliberately accept exactly two participants.
  The request lets the supervisor choose a map and rules from a mode-only policy. M05 must instead
  carry the lobby's validated game-type/map/topology selection through the allocation boundary;
  the supervisor must not become a second game catalog authority.
- The current match manifest participant row carries only preset identity, recipe fingerprint, and
  build revision. Match admission requires a preset and therefore cannot reproduce M04 custom
  builds. M05 needs one bounded opaque application-owned build transfer in each routing-owned
  participant row, decoded and revalidated only by the match worker.
- The M01 `LobbyTransitionDriverPlugin` allocates automatically when exactly two authenticated
  sessions exist and remembers process-lifetime tombstones. It remains an explicit compatibility
  smoke only. Product formation replaces neither its state nor its behavior in place; the product
  lobby composition gets a reservation/allocation owner, while the smoke is migrated to drive the
  product transaction or is retained as a narrowly named fixture until equivalent coverage passes.
- The client already owns a sequential routed lifecycle: accept a redacted grant, intentionally
  disconnect/unlink the lobby entity, wait for deferred despawn, then create one fresh match entity
  with the capability. Product-shell flow currently does not use that automatic transition path.
  M05 integrates this proven socket/entity boundary into the flow arbiter and Match Loading UI.
- Map reconstruction exposes `ClientMapReadiness`; terrain convergence exposes
  `ClientTerrainReadiness`; assets and match handshake already have explicit readiness facts.
  These are suitable inputs to one client check-in predicate. Presentation facts never mutate the
  match or start the countdown directly.
- The match worker already admits Netcode client ID plus routed peer only when both match its
  immutable manifest, installs the manifest team, and reuses the authoritative gameplay graph.
  The current match Hello immediately spawns the fighter and the legacy `SetReady` command unlocks
  countdown. M05 adds a manifest-scoped loading/check-in gate and makes product routed readiness
  automatic rather than adding another gameplay path.
- Pinned Lightyear receives connections, messages, and replication in `PreUpdate`, application
  systems observe them later, and sends buffered messages/replication in `PostUpdate`. The M05
  schedule must preserve a deferred unlink boundary before spawning a replacement client and must
  not infer cross-message order from unrelated typed receivers.

### Gaps M05 must close

1. No queue ticket has a reserved/loading state, and M04 intentionally has no
   cancellation-versus-reservation boundary.
2. No production system selects the oldest exact roster, assigns deterministic teams, or rotates
   compatible maps.
3. Allocation identity is `RequestId`, scoped to the old automatic driver rather than a product
   reservation, and the BRCT v1 request cannot carry game type, selected map, exact topology, or a
   custom build snapshot.
4. A grant is currently delivered once and the headless transition client disconnects immediately.
   Product handoff needs an application acknowledgement before the intentional lobby teardown and
   bounded recovery when the client never accepts the offer.
5. The lobby has no terminal loading fact from a match worker. It cannot distinguish activation,
   a missing participant, a failed worker, or expiry after grants have been issued.
6. The worker has no complete-roster loading deadline or explicit per-participant check-in.
7. Product flow ends at Queue. There is no `MatchLoading` state, loading-specific error mapping, or
   return-to-queue commit path.
8. Match lifecycle still allows only two players per team and the public advertisement validator
   rejects 3v3.

### Primary-source cross-check

- The pinned local Lightyear 0.29 book and examples confirm that connection lifecycle is driven by
  `Connect`/`Disconnect`, connection/replication receive happens in `PreUpdate`, buffered network
  send happens in `PostUpdate`, and channel mode—not application wishful thinking—provides
  reliability/order. The existing separate lobby/match entities and ordered application messages
  remain the correct seam.
- The Lightyear 0.29 upstream tag still identifies 0.28–0.29 as the Bevy 0.19 line and documents
  transport-agnostic Links, channels, fragmentation, and world replication. No current upstream
  source justifies replacing Brawler's delivered routed Link or introducing HTTP for handoff.
- Bevy 0.19 documentation confirms that explicit `ApplyDeferred` is the same-schedule visibility
  boundary and that state transitions have ordered exit/transition/enter schedules. M05 keeps the
  existing flow arbiter and explicit deferred teardown instead of using UI state as network
  authority.

## Decisions for specification review

| Concern | M05 decision |
|---|---|
| Formation owner | The lobby worker's queue/reservation module is the only formation authority; the supervisor never reads queue tickets or chooses a roster |
| Exact roster | A game type forms only when its FIFO contains at least `team_count * players_per_team`; it removes exactly that many oldest valid tickets and leaves overflow queued |
| Same-frame race | Queue command/ack collection and authenticated disconnect reconciliation run before formation. A valid Cancel or disconnect observed in that frame wins; once formation commits reservation state, a later Cancel is a reservation-cancel intent |
| Team assignment | Sort the reserved tickets by `(admission_order, ticket_id)` and assign `team = index % team_count`; for v2 `team_count` remains exactly two, yielding exact balanced 2v2 or 3v3 teams without parties, skill, or randomness |
| Map selection | Each game type owns a monotonic reservation ordinal. Select `map_preset_ids[ordinal % len]` in advertised operator order; bind it immutably to the reservation and advance once per committed reservation attempt, never by client vote or worker timing |
| Reservation identity | Add unpredictable nonzero `MatchReservationId(u128)` generated by the lobby; one reservation also owns one nonzero monotonic supervisor `RequestId` and later the returned `AllocationId`/`MatchId` |
| Ticket state | A ticket is exactly one of queued or reserved. It remains immutable and queryable while reserved, is absent from public queued counts, and cannot join another reservation |
| Host admission | The supervisor keeps sole process/worker/route capacity authority. It admits against existing hard ceilings (one lobby plus at most four match workers under `MAX_WORKERS = 5`, route/capability bounds, pending-allocation bound) before spawning |
| Gameplay selection across IPC | The lobby resolves and sends game type revision, mode, selected map preset/revision, exact two-team topology, rules profile, and stable participant rows. The supervisor validates structural bounds and copies these values; it does not infer them from mode or parse the operator catalog |
| Custom build transfer | Each participant carries a length-bounded opaque `build_snapshot` (maximum 256 bytes) in the routing contract. The lobby encodes one application `MatchBuildSnapshotV1` containing accepted canonical recipe, source preset if any, revision, fingerprint, and point total. The worker decodes, resolves against embedded catalogs, and byte/identity-checks it before Ready. Routing never imports Bevy or Brawler build types |
| Manifest evolution | Replace the one current match/allocation manifest/control schema rather than adding compatibility decoding. Bump the independent BRCT/manifest versions where their below-Lightyear framing changes; bump the one global application compatibility version for lobby/match messages. No message-level fallback versions |
| Grant protocol | After validated worker Ready, the lobby sends one bounded `ReservationOffer` per live reserved session over the ordered reliable lobby channel. The client acknowledges the exact reservation/allocation/ticket before the lobby sends `BeginMatchConnect`. Only then does the client intentionally unlink and connect to the worker |
| Capability lifetime | Preserve the supervisor's 30-second pending activation and 10-minute hard lifetime. The lobby uses a stricter 20-second end-to-end loading deadline; unused grants are revoked on dissolution, so the hard lifetime is never the product wait policy |
| Match admission | A worker accepts only the manifest's Netcode client ID paired with its routed peer and rejects duplicates. All manifests are validated before Ready; no partial roster is added later and join-in-progress remains absent |
| Client check-in | Add one idempotent ordered-reliable `MatchLoadingReady` request scoped to allocation, match, and request ID. The client sends it only after accepted Match Hello, map Ready, terrain Ready for the matching generation, assets Ready, and manifest-backed controlled state are all observed |
| Server check-in | The worker records each manifest participant once. Only an exact connected manifest roster with every check-in may set product participants ready and unlock the existing fixed-tick countdown |
| Countdown | The authoritative `MatchState` remains Waiting through loading. The existing server fixed-tick transition creates the only Countdown after the full check-in gate commits; Match Loading UI never runs an independent countdown |
| Cancellation after reservation | Before activation, a matching Cancel removes the cancelling ticket and dissolves the reservation. All other connected, compatible tickets return to the front of the same pool in original admission order. Repeated Cancel is idempotent |
| Disconnect/timeout culprit | Before activation, a disconnected or un-checked-in ticket is removed; retained tickets requeue at the front. A failure with no attributable participant requeues every still-valid ticket |
| Automatic recovery bound | One reservation makes one semantic allocation request; transport may retry the identical request. Capacity refusal dissolves and requeues with supervisor `retry_after` clamped to 1–5 seconds as a per-pool formation cooldown. Worker/route/loading failure may auto-requeue a retained ticket at most twice consecutively; the third failed reservation removes it with a recoverable `MatchStartUnavailable` outcome. A fresh manual Join starts a new budget |
| Loading deadline | Twenty seconds from accepted reservation to full worker check-in. Sub-deadlines are 10 seconds for allocation/Ready and the remaining time for offer acknowledgement, fresh connection, sync, and check-in. The earliest terminal failure dissolves once |
| Worker-to-lobby terminal fact | Extend bounded control lifecycle with allocation progress resolved by stable allocation/reservation IDs: `Activated` or `Dissolved { reason, missing sessions }`. The supervisor validates correlation and forwards lifecycle facts; it does not decode gameplay state |
| Client failure path | A dissolved reservation returns retained connected clients to Queue with a bounded notice. A removed/cancelling client returns to Game Select. An unexpected match-session failure before activation starts a fresh lobby session and reconciles by reservation identity; it never resumes the failed match connection |
| Product/legacy boundary | Product routed workers use automatic loading check-in. The direct-UDP baseline retains explicit build/ready behavior. The M01 transition smoke is migrated to the product reservation contract before its old exact-two driver can be removed |
| M05/M06 boundary | The first authoritative Countdown transition commits activation and ends lobby reservation ownership. Disconnect, forfeit, worker Result, completion, return-to-lobby, results, and requeue after that point belong to M06 |

## Scope

### Included

- exact FIFO formation for advertised 2v2 and 3v3 game types;
- atomic ticket reservation and deterministic cancellation/disconnect race resolution;
- deterministic team assignment and operator-order map rotation;
- queue/reservation snapshots and targeted loading outcomes without opponent build disclosure;
- product allocation requests derived from immutable reservations;
- below-Lightyear allocation/control/manifest evolution needed for game identity, topology, map,
  build transfer, lifecycle progress, cancellation, and route cleanup;
- supervisor host-capacity admission, worker spawn/manifest validation, Ready, route creation, and
  capability grants using the production process topology;
- complete custom/preset build transfer and worker-side re-resolution;
- 3v3 capacity enablement in catalog, match rules, manifest validation, roster admission, map
  capacity checks, and representative Wipeout/Hot Zone behavior;
- product Match Loading flow and presentation using existing shell/focus/style primitives;
- two-phase lobby offer acknowledgement and fresh lobby-to-match Lightyear handoff;
- exact manifest connection admission, client readiness predicate, idempotent check-in, loading
  deadline, and one server-owned countdown;
- bounded dissolution, front requeue, retry/cooldown, route revocation, and process teardown;
- privacy-safe formation/allocation/loading telemetry and deterministic evidence;
- migration of the explicit routed smoke to exercise product formation and preservation of the
  named direct-UDP baseline.

### Deferred

- match completion, result presentation, Queue Again, Change Game, leave, forfeit, and normal
  return-to-lobby (M06);
- post-activation disconnect semantics and countdown departure repair (M06);
- concurrency soak across heterogeneous active/completing workers and complete route/process
  reclamation campaigns (M06, with final hardening in M09);
- product combat HUD, scoreboard, accessibility matrix, and non-pausing in-match menu (M07);
- bots, practice game types, and local supervisor launch (M08);
- global matchmaking, parties, skill balancing, map voting, join-in-progress, match resumption,
  spectators, accounts, production Netcode-token issuance, and fleet orchestration;
- increasing the current four-match host ceiling or dynamically sizing it from machine resources;
- authored 4v4/other topology even though routing retains an eight-participant structural bound.

## Architecture and ownership

```text
src/
  lobby/
    queue.rs                   shared M04 queue wire contract, extended with reservation messages
    loading.rs                 shared bounded reservation/loading identities and outcomes
  server/lobby/
    queue.rs                   M04 admission/cancel authority, extended with queued/reserved states
    formation.rs               exact roster, team/map choice, reservation/requeue policy
    allocation.rs              reservation <-> worker-control adapter and terminal reconciliation
  client/
    queue.rs                   queue/reservation message observation and acknowledgements
    match_loading.rs           loading readiness predicate and focused presentation
    flow.rs                    sole cross-flow arbiter; gains MatchLoading transitions only
  server/
    admission.rs               evolved manifest/build/topology validation
    worker.rs                  sole control-stream owner and loading progress adapter
    loading.rs                 match-worker check-in/deadline/activation gate
packages/brawler-routing/src/
  allocation.rs               structural product allocation validation, no game inference
  control.rs                  evolved bounded allocation/lifecycle control bodies
  manifest.rs                 evolved immutable match manifest and opaque build snapshot rows
  runtime.rs                  host admission, spawn, grants, cancellation, progress forwarding
```

Exact filenames may change during M04 reconciliation, but ownership may not. `mod.rs` files remain
composition/public surfaces. The supervisor stays Bevy-free and gameplay-opaque. The lobby has no
map, terrain, physics, or combat state. The match worker has no queue authority. Client readiness is
an intent/check-in fact, never authority to choose the roster, map, or start tick.

### Queue and reservation state

`QueueState` keeps ticket storage and command idempotency. Add a focused reservation owner rather
than overloading public pool rows:

```text
Queued
  -> Reserved(allocation not requested)
  -> AllocationPending
  -> OfferPending
  -> HandingOff
  -> CheckingIn
  -> Activated
   \> Dissolving -> removed culprit + retained tickets front-requeued
```

A reservation contains only bounded stable/application values:

- `MatchReservationId`, game/catalog/configuration identity, mode and rules profile;
- selected map preset/revision and reservation ordinal;
- exact topology and deadline;
- ordered participant rows containing ticket/session/player/Netcode identity, assigned team,
  accepted build transfer, handoff/check-in phase, and consecutive recovery count;
- optional supervisor request/allocation/match/worker identity once learned;
- one terminal disposition flag so cancellation, timeout, disconnect, supervisor rejection, and
  worker failure cannot dissolve twice.

The public queue snapshot still counts only queued tickets. Its revision changes once when an exact
roster leaves the pool and once when retained tickets re-enter. No transient zero/re-add snapshot is
published inside one atomic dissolution transaction. Targeted reservation messages carry no other
participant names, identities, builds, or check-in detail.

### Deterministic formation transaction

In one lobby `Update`:

1. snapshot sessions authenticated at frame start;
2. collect ordered queue messages and outcome acknowledgements;
3. reconcile routed disconnects;
4. apply Queue Join/Cancel transactions in the M04 stable order;
5. apply Cancel to existing reservations and stage any dissolution;
6. commit dissolutions/requeues;
7. for each advertised game type in catalog order, form at most one exact roster if its pool is not
   in cooldown and reservation capacity remains;
8. publish targeted outcomes and one aggregate snapshot from the final state;
9. stage bounded allocation controls for the worker control owner.

At most eight game types and 32 authenticated sessions make a full ordered scan sufficient. No
priority queue or general matchmaking framework is warranted. Formation is limited to one new
reservation per game type per frame and globally by the supervisor/lobby outstanding-allocation
bound.

### Allocation and manifest contract

The evolved `AllocateRequestBody` is idempotent by request ID and byte-equivalent body. It contains
reservation identity, game configuration identity/fingerprint, mode, selected map/revision, rules
profile, exact two-team topology, and 1..=8 stable participant rows. Validation rejects zero IDs,
duplicates, mismatched team counts, a roster other than `teams * players_per_team`, oversized build
snapshots, and any request exceeding declared lobby/supervisor limits.

The supervisor:

1. validates caller is the one Ready lobby worker and request correlation is new or identical;
2. checks worker, pending allocation, route, capability, and participant ceilings;
3. allocates process/worker/allocation/match/route/peer identities and CSPRNG seed;
4. copies the lobby-selected application fields into the immutable match manifest;
5. spawns the production `brawler-server --worker-role match` process;
6. waits for manifest digest/version/identity Ready under the existing lifecycle deadline;
7. registers all routes/capabilities atomically or rolls the worker back;
8. returns the complete grant set to the lobby.

The worker decodes `MatchBuildSnapshotV1`, resolves the canonical recipe with embedded catalogs,
and proves revision, preset identity, recipe fingerprint, point total, topology, map capacity,
mode/rules compatibility, protocol registry, and content fingerprint before Ready. A malformed row
fails the complete manifest; no participant receives a grant.

### Two-phase client handoff

`ReservationOffer` contains only the receiving player's ticket/reservation/allocation/match IDs,
game/map/topology summary, its own accepted build summary, redacted route grant, and loading
deadline. The client accepts it only if all IDs match its current acknowledged Queue membership and
connection generation. It sends `ReservationOfferAck` on the existing one ordered client envelope.

The lobby marks that participant offer-acknowledged and sends `BeginMatchConnect`. The client then:

1. commits flow to Match Loading;
2. intentionally disconnects and unlinks the lobby entity;
3. waits through the existing explicit deferred despawn boundary;
4. clears lobby-generation queue/editor/snapshot resources but retains one bounded loading context;
5. creates exactly one fresh routed match entity with the offered capability;
6. performs normal Netcode and Brawler Match Hello authentication.

Duplicate/stale offers, acknowledgements, Begin messages, or old-generation network data are
ignored by exact identity. The client never logs or persists the capability. A lobby disconnect
before Begin is unexpected and recovers through a fresh lobby session; the server reservation
deadline remains authoritative.

### Match loading and check-in

The match worker creates a `MatchLoadingState` from the validated manifest before accepting
connections. Match Hello may create the participant fighter and replicated state as today, but the
participant remains non-ready and input-gated. Product routed sessions do not expose or send
`SetReady`.

The client readiness predicate requires all of:

- current routed phase is Match and connection generation matches loading context;
- `ClientJoinPhase::Active` is accepted for the manifest-backed participant;
- the replicated `MatchState.match_id` equals the offer;
- `ClientMapReadiness::Ready` and the presented map identity equals the selected manifest map;
- `ClientTerrainReadiness::Ready` refers to that map/terrain generation;
- retained product client assets report Ready;
- exactly one controlled fighter exists with the accepted player/build identity;
- no disconnect, invalid convergence, or local loading error is active.

It then sends one idempotent `MatchLoadingReady` and waits. The worker validates request identity,
manifest membership, current connected link, map/match identity, and duplicate/stale semantics. It
records check-in but does not trust the client for map content, build data, team, or gameplay state.

Once every manifest participant is still connected and checked in, one fixed-tick loading-commit
system marks the exact roster ready. The existing authoritative lifecycle observes the complete
roster and transitions Waiting -> Countdown. This transition emits the control-plane `Activated`
fact correlated to the allocation. Client flow enters Match only from the replicated authoritative
Countdown, not from its sent check-in or a lobby prediction.

### Dissolution and requeue policy

All pre-activation failures converge on one transaction:

1. mark the reservation terminal exactly once;
2. revoke unactivated/active routes and stop the incomplete worker under existing bounded process
   deadlines;
3. identify explicit cancelling/disconnected/timed-out participants when evidence exists;
4. remove those tickets and queue a targeted recoverable outcome for a fresh/current lobby session;
5. retain only sessions that are still authenticated, catalog-compatible, and below the two-failure
   automatic recovery budget;
6. sort retained tickets by original `(admission_order, ticket_id)` and prepend them to their one
   original pool without allocating new ticket IDs or admission revisions;
7. publish one final aggregate revision and apply a per-pool cooldown when requested;
8. discard capability bytes and reservation state after bounded terminal acknowledgements/expiry.

Front requeue preserves the retained players' original precedence over overflow tickets. It does
not promise that they remain together or retain prior teams/map on the next exact formation.
Cancellation removes only the canceller. A capacity refusal has no culprit and requeues everyone;
its cooldown prevents a busy-loop. A third consecutive worker/route/loading failure removes the
affected retained ticket and asks the player to join again, bounding an unhealthy roster's automatic
churn.

### Schedule and role boundaries

Lobby application systems continue to observe typed Lightyear messages after receive in `Update`.
The formation/reservation sets extend M04's chain while keeping the queue mutation and snapshot
publication order visible. Worker control IPC still has exactly one stream reader/writer owner;
Bevy resources provide bounded inbox/outbox handoff rather than reading Unix streams from gameplay
systems.

Match loading check-in observation runs in `Update`; its activation commit runs before the common
match lifecycle in `FixedUpdate`. An explicit deferred boundary makes participant/check-in mutation
visible before roster refresh. Countdown creation stays in the existing lifecycle set. Network
sends remain buffered for Lightyear `PostUpdate`.

The dedicated-server feature graph remains free of rendering, windowing, audio, device input, and
client assets. The routing package remains Bevy-free. Product UI never enters supervisor or server
features.

## Protocol and bounds

### Application protocol

Register the smallest bounded current-schema messages in `protocol.rs`:

- server reservation offer / begin / dissolution outcome;
- client offer acknowledgement using the ordered lobby envelope;
- client match-loading ready request and worker acknowledgement;
- any presentation-safe loading phase needed by Match Loading.

Every ID is nonzero and every collection has a custom bounded deserializer. Canonical encoded-size
tests cover maximum 3v3/8-participant shapes. Route capability Debug remains redacted. The one global
compatibility handshake changes once with the application registry; old clients fail cleanly rather
than decoding fallback forms.

### Routing/control protocol

Evolve the one current routing contract with:

- product allocation request fields and maximum 256-byte opaque build snapshot per participant;
- allocation cancellation/revocation correlated by request/allocation identity;
- worker/supervisor/lobby loading progress and terminal activation/dissolution facts;
- manifest topology/game/map/build fields;
- explicit rejection categories for malformed, capacity, incompatible, spawn/Ready, cancelled,
  expired, and internal failures.

Keep control records below 64 KiB and match manifests below 4 KiB. With at most eight participant
rows and 256 build bytes each, the semantic 4 KiB manifest bound remains feasible and must be proven
by a maximum-shape test rather than assumed. Control queues retain their existing frame/byte bounds;
new progress bodies cannot create unbounded history.

### Operational bounds

| Bound | M05 value |
|---|---|
| Authenticated lobby sessions / tickets | 32 |
| Advertised game types | 8 |
| Teams | exactly 2 |
| Players per team | 2 or 3 |
| Formed roster | exactly 4 or 6; routing structural maximum remains 8 |
| Match workers | at most 4 plus one lobby under current `MAX_WORKERS = 5` |
| Tracked allocations | current supervisor maximum 8; live process capacity is stricter |
| Build transfer | at most 256 encoded bytes per participant |
| Pending capability activation | 30 seconds infrastructure maximum |
| Product loading deadline | 20 seconds total |
| Allocation/Ready sub-deadline | 10 seconds |
| Capacity cooldown | clamp supervisor retry hint to 1–5 seconds |
| Automatic failed-reservation requeues | at most 2 consecutive per ticket |

## Diagnostics and evidence

All diagnostics are bounded aggregates or redacted stable correlations. Record:

- formation attempts/successes by game type and topology;
- FIFO age at reservation, overflow left queued, map rotation selection, and cancellation race
  disposition;
- current/high-water reservations, pending allocations, loading workers, and requeued tickets;
- allocation accepted/rejected reason, spawn-to-Ready and Ready-to-grant latency;
- offer/ack/begin/check-in counts and phase latencies;
- loading activation, timeout, missing participant, disconnect, route expiry, worker failure, and
  dissolution reason;
- automatic requeue count, cooldown, exhausted recovery budget, routes revoked, and workers stopped;
- manifest/build/topology validation failures without recipe, identity, capability, address, or
  player-name bytes;
- time from exact roster availability to authoritative Countdown.

Process evidence correlates reservation -> allocation -> worker -> activation with redacted IDs in
structured test records, not ordinary logs. Existing routing packet/queue/process metrics remain the
transport source of truth.

## Implementation plan

Implementation begins only after M04 completes, this document is reconciled against its actual
public/private seams, and the user validates the resulting specification.

### Slice 1 — Reconcile M04 and establish pure reservation rules

- [ ] Record M04's final ticket, pool, command-memory, schedule, client-flow, snapshot, telemetry,
  and teardown seams; update this specification where they differ.
- [ ] Add bounded reservation/loading shared identities and targeted outcomes.
- [ ] Extend `QueueState` with queued/reserved ownership without weakening M04 idempotency.
- [ ] Implement/test exact oldest-roster extraction, deterministic alternating teams, catalog-order
  map rotation, overflow preservation, same-frame cancel/disconnect precedence, front requeue, and
  recovery budgets as pure state transactions.
- [ ] Expand advertisement/catalog/runtime validation from exact 2v2 to exact 2v2 or 3v3 and prove
  every advertised map satisfies resolved capacity.

### Slice 2 — Evolve allocation/manifest contracts before UI

- [ ] Define/measure `MatchBuildSnapshotV1`; prove preset and custom recipes round-trip below 256
  bytes and re-resolve to the M04 accepted identity/loadout.
- [ ] Evolve allocation request, match manifest, control versions, codecs, bounds, fixtures, and
  unknown-version behavior in `brawler-routing`.
- [ ] Move game/map/topology selection from supervisor mode policy to validated lobby request;
  retain only infrastructure policy in the supervisor.
- [ ] Generalize exact-two allocation validation to exact manifest topology and preserve duplicate,
  conflict, capacity, Ready, and rollback safety.
- [ ] Add cancellation and terminal loading progress with correlation, redaction, bounded queues,
  and route/worker cleanup.
- [ ] Update match-worker manifest validation and admission for 2v2/3v3 plus custom builds before
  any manifest can report Ready.

### Slice 3 — Connect production formation to real worker allocation

- [ ] Add lobby formation/allocation plugins and ordered schedule sets on the M04 queue.
- [ ] Stage one stable request per reservation through the existing sole worker-control owner.
- [ ] Validate complete grant sets and map each grant to the exact current reservation participant.
- [ ] Implement allocation rejection, timeout, cancellation, disconnect, worker failure, cooldown,
  and front-requeue as one terminal transaction.
- [ ] Migrate the routed process smoke from automatic exact-two session allocation to explicit
  product queue formation; retain a narrow compatibility fixture only until equivalent evidence
  passes.

### Slice 4 — Deliver product Match Loading and fresh connection handoff

- [ ] Add `MatchLoading` to the existing client flow/overlay model without creating a second
  transition arbiter.
- [ ] Implement Reservation Offer -> Ack -> Begin ordering, exact generation/identity checks, one
  retained loading context, and secret-free diagnostics.
- [ ] Reuse the delivered intentional disconnect/unlink/deferred-despawn/fresh-entity path in the
  product shell.
- [ ] Present mode/map/topology/own build and honest phase/error text with controller, keyboard,
  pointer, focus, and 960x540 scrolling behavior.
- [ ] Recover a pre-activation failure through a fresh lobby connection and authoritative
  reservation reconciliation; never resume a failed match session.

### Slice 5 — Add exact worker check-in and authoritative countdown gate

- [ ] Install manifest-scoped `MatchLoadingState`, deadline, and participant check-in state before
  accepting clients.
- [ ] Compose the client readiness predicate from accepted Hello, map, terrain, assets, controlled
  fighter/build, and generation facts.
- [ ] Add idempotent Match Loading Ready/Ack and reject stale, duplicate-conflicting,
  non-manifest, wrong-map, or disconnected requests.
- [ ] Gate product routed participant readiness on the exact full manifest; preserve the direct-UDP
  ready baseline.
- [ ] Commit once in fixed tick and prove the existing authoritative lifecycle produces exactly one
  Countdown and no client-side substitute.
- [ ] Emit Activated/Dissolved lifecycle facts and stop/revoke incomplete workers under deadlines.

### Slice 6 — Verify and hand off

- [ ] Add focused pure, codec, ECS/schedule, in-memory routing, real process/UDP/IPC, UI/controller,
  impairment, failure, and capacity evidence below.
- [ ] Run canonical role checks/tests/clippy/format plus routed product and direct-UDP baselines.
- [ ] Measure maximum manifest/control/application message sizes and handoff latency phase bounds.
- [ ] Update canonical `just`/README commands only for validated product formation/loading paths.
- [ ] Set M05 to `User playtest` with a four- or six-client launch path, controls, expected loading
  phases, known M06 limitations, and requested observations.

## Verification contract

### Pure queue/formation/build tests

- fewer than exact topology does not reserve; exact N removes the N oldest; N+k leaves ordered
  overflow;
- simultaneous eligible game types form independently in catalog order within global capacity;
- alternating assignment gives exact balanced teams for 2v2 and 3v3 and is stable across runs;
- map rotation follows operator order, binds once per reservation, wraps, and does not change on
  request retry;
- same-frame valid Cancel/disconnect wins before formation; post-reservation Cancel dissolves once;
- retained tickets prepend in original admission order without new ticket/admission identity;
- explicit culprit removal, infrastructure-wide requeue, capacity cooldown, two automatic retries,
  and third-failure removal match policy;
- preset and custom accepted builds encode below 256 bytes, decode, re-resolve, and exactly match
  revision/fingerprint/point total; tampering fails before Ready;
- 2v2/3v3 catalog/map/rules validation passes while 1v1, 4v4, uneven, oversized, bad-map, and bad
  rules/profile configurations fail closed.

### Codec and control tests

- every new application/control/manifest maximum shape round-trips canonically within its byte
  bound; every truncation, trailing byte, zero ID, count mismatch, duplicate identity, invalid team,
  oversized build, reserved bit, and unknown version fails;
- identical allocation request ID/body is idempotent; conflicting reuse rejects without a second
  worker, route, or response history;
- 4- and 6-player requests produce exact manifest/grant rows; structural maximum-eight remains
  bounded even though product catalog rejects unsupported topology;
- supervisor copies lobby-selected game/map/topology/build bytes but cannot interpret Brawler build
  types;
- manifest or Ready failure creates no grant; partial route/capability registration rolls back all;
- cancel/reject/progress/Activated/Dissolved correlation ignores stale or wrong worker generations;
- Debug/log output redacts build bytes, capability, nonce, digest, network identity, and source.

### ECS and schedule tests

- queue messages and disconnect reconciliation precede cancellation/formation; final snapshot
  observes the committed result in the same update;
- at most one formation per game type per update and no pool/reservation owns one ticket twice;
- full control inbox/outbox retries a stable request without allocating another identity;
- worker app cannot report Ready until topology/map/build/registry/content validation passes;
- client offer observation cannot spawn a match entity before Begin and the old lobby entity's
  deferred unlink/despawn;
- loading readiness remains false for each missing prerequisite independently and resets on session
  generation change;
- check-in is idempotent and manifest-scoped; a nonparticipant cannot mark another participant;
- four/six exact connected check-ins set product roster ready once; partial roster remains Waiting;
- activation commit is visible to roster refresh before lifecycle evaluation and produces one
  Countdown with the configured future start tick;
- no product presentation system mutates match/queue authority; server feature graph remains
  client-render/audio/input free.

### In-memory routed/network tests

- connect lobby -> catalog -> Join exact roster -> reservation offer/ack/begin -> fresh match
  connection -> map/terrain convergence -> check-in -> Countdown for Wipeout 2v2 and Hot Zone 3v3;
- overflow remains queued and its aggregate count is correct while the first roster loads;
- Cancel before formation, Cancel after reservation, disconnect during allocation, offer loss,
  duplicate offer/ack/begin, stale generation, route expiry, and one missing check-in have exact
  ticket/worker/route dispositions;
- consecutive loss/retry preserves one semantic request and one worker; sequenced queue snapshots
  remain independent of lossless targeted reservation outcomes;
- a custom M04 build reaches the worker and controls the spawned loadout without fallback to preset;
- wrong capability, peer, Netcode ID, allocation, match, map, build, or check-in identity fails
  without admitting a fighter or unlocking countdown;
- capacity rejection requeues with cooldown and no busy-loop; a later capacity opening forms the
  retained oldest roster.

### Real process/UDP/IPC tests

- canonical routed product smoke forms and loads an exact 2v2 through the public endpoint using the
  real lobby worker, supervisor, match child, Unix IPC, and fresh client sockets;
- a six-client Hot Zone 3v3 process scenario reaches exactly one authoritative Countdown with all
  manifest teams/builds/map identities correct;
- cold worker spawn/Ready and full handoff complete inside the 20-second product deadline under the
  accepted development environment;
- kill before Ready, kill after grants, malformed manifest, stalled control stream, route
  activation expiry, client loss during each loading phase, and supervisor capacity refusal dissolve
  once, revoke routes, reap incomplete child processes, and apply exact requeue policy;
- a second queued roster cannot receive the first reservation's routes, packets, grants, progress,
  or replicated world state;
- the named direct-UDP smoke still reaches its current explicit ready/countdown behavior;
- routed packet MTU, control queue, manifest, and capability bounds remain unchanged or are updated
  with measured evidence.

### Visual/controller checks

- controller, keyboard/mouse, and pointer can observe reservation and Match Loading without focus
  loss or duplicate activation;
- Match Loading remains legible at 960x540, 1280x720, and 1920x1080 with minimum/default/maximum M02
  UI scale and long valid server/game/map names;
- phases do not imply progress percentages or queue estimates; stale/failed loading gives a plain
  reason and context-valid Queue/Game Select/Disconnect action;
- only the player's own build is shown; secret/internal IDs and opponent builds never render;
- map/terrain sync cannot accept gameplay input before check-in and Countdown;
- the authoritative Countdown appears once after the last participant checks in.

### Performance and bounds

- record p50/p95/max exact-roster-to-Ready, Ready-to-offer, offer-to-match-connect,
  match-connect-to-check-in, and exact-roster-to-Countdown latency;
- prove 32 tickets, eight pool rows, maximum live reservations/allocations, four workers, 24 match
  participant routes, and retained targeted outcomes stay within declared memory/queue bounds;
- run representative 3v3 fixed-tick/performance gates for both modes and terrain admission without
  regressing existing thresholds;
- repeated pre-activation failure cannot grow request history, reservation memory, child handles,
  routes, capabilities, control frames, or client entities.

## Exit criteria

- [ ] M04 is complete and its actual delivered seams are reconciled here.
- [ ] The user validates the reconciled M05 specification before implementation begins.
- [ ] Exact 2v2 and 3v3 formation, deterministic teams/map rotation, cancellation race, overflow,
  front requeue, and bounded retry policies match pure and network evidence.
- [ ] Preset and custom M04 accepted builds cross the manifest boundary and revalidate before Ready.
- [ ] The supervisor remains queue/gameplay-opaque and the lobby remains simulation-free.
- [ ] Every retained participant establishes a distinct manifest-authenticated worker session
  through the same public endpoint and checks in against the correct map/terrain/build generation.
- [ ] Partial rosters never enter Countdown; a complete roster produces exactly one authoritative
  Countdown with no product ready-up or client countdown.
- [ ] Every pre-activation cancel/disconnect/refusal/failure/timeout has one route, worker, ticket,
  UI, and diagnostic disposition with no leaks or invisible membership.
- [ ] Required pure, codec, ECS, in-memory, real-process, impairment, visual/controller, role,
  capacity, and direct-baseline checks pass with recorded evidence.
- [ ] User playtest feedback is recorded and triaged before M05 is marked Complete.
- [ ] Learn-from-errors review records mistakes, causes, prevention, and reusable skill updates where
  justified.

## Research record

### Repository and version-pinned local sources

- `docs/00-product-direction.md` — short-match/readability/build-tradeoff product constraints.
- `docs/08-network-architecture.md` — server authority, stable wire identity, one global
  application compatibility schema, and replication boundaries.
- `docs/13-player-ux.md` — exact pool formation, Match Loading, fresh session, check-in, failure,
  and screen-flow product contract.
- `docs/14-multiplayer-server-architecture.md` — accepted supervisor/lobby/match ownership,
  single-public-port handoff, capability, manifest, IPC, and failure contract.
- `docs/implementation/v2/{roadmap,milestone-01,milestone-03,milestone-04}.md` — milestone boundary,
  delivered routed/product lobby seams, and M04's explicit reservation deferral.
- `packages/brawler-routing/src/{allocation,control,manifest,runtime,limits}.rs` and
  `supervisor/` — exact-two allocation, match manifest, host/process/route bounds, capability
  lifecycle, idempotency, cleanup, and process supervision.
- `src/server/lobby/{mod,catalog,queue}.rs`, `src/lobby/{queue}.rs`, and `src/lobby.rs` — current
  authenticated sessions, operator catalog, in-progress M04 tickets/pools, wire messages, and 2v2
  validation.
- `src/server/{admission,worker,routed_worker}.rs` and `src/server/mod.rs` — worker bootstrap,
  manifest admission, sole IPC owner, match Hello, loadout install, and current explicit ready path.
- `src/client/{mod,session,flow,queue}.rs` — delivered sequential routed transition, one client
  entity, product flow arbiter, and in-progress queue state.
- `src/map/client.rs`, `src/terrain/client/`, and `src/client/assets.rs` — concrete client readiness
  inputs and generation cleanup.
- `src/matchplay/{model,server,spawns}.rs` — capacity derivation, map proof, deterministic spawn,
  full-roster readiness, and authoritative Countdown transition.
- `content/v1/maps.ron` and `config/server/game-types.ron` — current two-team/four-spawn-per-team
  maps and checked-in 2v2 operator examples.
- `references/lightyear/README.md`, `book/src/SUMMARY.md`,
  `book/src/concepts/{connection/title,reliability/channels,replication/replicate,bevy_integration/system_order}.md`,
  and `examples/{simple_setup,simple_box}` — pinned 0.29 connection, channels, receive/send schedule,
  replication, and plugin composition.
- `references/bevy/examples/README.md` plus the pinned Bevy 0.19.1 dependency source selected by
  `Cargo.lock` — version warning, states, schedules, and deferred-command behavior.
- `justfile`, root `README.md`, `scripts/network-routed.sh`, and `tests/routed_process.rs` — canonical
  role checks, routed/product/direct commands, process harness, and transition baseline.

### Current primary sources

- Lightyear 0.29 upstream tag and README:
  <https://github.com/cBournhonesque/lightyear/tree/0.29.0> — confirms the Bevy 0.19 compatibility
  line and current connection/Link/channel/replication model. The local snapshot matches this tag's
  APIs more precisely than the moving latest documentation.
- Lightyear releases:
  <https://github.com/cBournhonesque/lightyear/releases> — the Bevy 0.19 line emphasizes input
  authorization and replication hardening; M05 keeps manifest admission and explicit check-in
  validation rather than trusting client readiness as gameplay state.
- Bevy 0.19 state documentation:
  <https://docs.rs/bevy/0.19.1/bevy/state/index.html> — states are appropriate for app-scale flow,
  while authority remains in server resources/components.
- Bevy 0.19 `ApplyDeferred` documentation:
  <https://docs.rs/bevy/0.19.1/bevy/ecs/schedule/struct.ApplyDeferred.html> — confirms explicit
  same-schedule visibility semantics used by client teardown and server activation ordering.

The internet cross-check found no reason to deviate from the pinned local APIs. Exact implementation
must continue to use the pinned Bevy 0.19.1/Lightyear 0.29.0 source and compile tests rather than a
moving latest snippet.

## Specification validation

Awaiting user review. Before implementation, reconcile this document after M04 completion and
resolve any review changes in the decisions, plan, verification contract, and exit criteria. A user
direction to implement the reconciled M05 specification is the approval event that moves status to
`Implementing`.
