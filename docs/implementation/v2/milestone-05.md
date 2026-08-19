# V2 milestone 05 — Exact formation, worker allocation, and match loading/handoff

## Tracking

| Field | Value |
|---|---|
| Status | Specification review |
| Prepared | 2026-08-19; research overlapped M04 by explicit user direction; authority/UX coherence repairs applied during specification review |
| Objective | Turn exact authoritative queue pools into deterministic 2v2 or 3v3 reservations, allocate one compatible isolated match worker through the existing supervisor, hand every retained participant to a fresh worker connection, synchronize and check in the complete roster, and begin exactly one authoritative countdown |
| Entry dependency | Pending: M04 must complete and its delivered queue/editor/client-flow seams must be reconciled before M05 implementation |
| Scope authority | Research and specification only. Production implementation requires user validation after final M04 reconciliation |

M04 remains in user playtest and review. This document makes the next boundary reviewable while its
evidence closes; it does not authorize reservation, allocation, routed-control, worker, client, or
gameplay changes. Before M05 implementation, its checked-in specification and code must be
reconciled once more against the final M04 disposition.

## Player-visible outcome

When the oldest exact roster of application-acknowledged queue memberships exists for an advertised
game type, the lobby moves those players from Queue to Match Loading as one atomic reservation. In
that transaction it stages each participant a targeted `ReservationStarted` phase before requesting
a worker, so worker startup cannot create a prolonged unreported reservation. Aggregate snapshots
remain non-authoritative for the client's own membership and cannot move it between flows. Match
Loading names the accepted game type, selected map, topology, and the player's
accepted build. It reports a small honest phase—reserving, starting server, connecting,
synchronizing map/terrain, waiting for players, cancelling, or returning to queue—without exposing
capabilities, route IDs, process IDs, internal retries, or another player's build.

The supervisor admits host capacity, starts one isolated authoritative match worker, and returns one
short-lived route grant per reserved lobby session only after the worker validates its immutable
manifest and reports Ready. The lobby first offers the public allocation summary and waits for the
whole roster to acknowledge it; no client receives capability bytes in that offer. Only the later
roster-wide `BeginMatchConnect` carries each recipient's own grant. Each client then closes its lobby
Lightyear session and creates one fresh match Lightyear session through the same public UDP endpoint.
The worker admits only identities in the manifest. A client checks in only after its match handshake
is accepted, the authoritative map snapshot is reconstructed, the matching terrain generation has
converged, required client assets are no longer loading and any declared degraded fallbacks are
active, and its controlled fighter/build state matches the manifest-backed admission.

The worker prepares activation only after every manifest participant is connected and checked in.
The supervisor serializes that request against cancellation and grants exactly one activation
commit; only that grant lets the worker start the existing authoritative countdown. There is no
second client-side countdown and no ready-up prompt in the product routed flow. The direct-UDP
development baseline retains its current explicit ready path.

Cancellation, disconnect, worker refusal/failure, route expiry, and loading timeout never leave an
invisible reservation. Before activation commit, a cancelling ticket is removed; at any point before
Active, a missing culprit is removed and every still-valid retained ticket recovers with its
original FIFO key after live or freshly authenticated lobby binding.
Infrastructure failure requeues the whole retained roster without charging an individual player
failure budget. During the intentional lobby-to-match disconnect, a bounded detached reservation
lease preserves each retained ticket by authoritative network identity and rebinds it only after a
fresh lobby authentication. The Queue screen then resumes with a fresh authoritative population; a
bounded notice explains why loading did not complete.

M05's player-visible success gate is a complete roster reaching Countdown in one worker; verification
also observes the automatic transition to Active once to prove the ownership handoff and temporary
one-product-match occupancy. A departure during Countdown must instead terminate the incomplete start
and recover the remaining reservation rather than stranding it. Reusable Active-match completion
lifecycle, leave/forfeit, Results, Queue Again, Change Game, worker-result reconciliation, and
repeated concurrent-match cleanup are M06. Until then, reaching Active capacity-removes any queued overflow
with an honest unavailable notice and pauses new admission; the one match may run to Completed, where
minimal Match presentation exposes the replicated result and Disconnect without legacy restart.

## Research findings

### Existing reusable foundation

- M04's delivered `QueueState` already owns one immutable `QueueTicket` per authenticated lobby
  session, exact game identity/revision, the accepted public build summary, the complete resolved
  loadout, monotonic admission order, FIFO pools, idempotent Join/Cancel memory, disconnect cleanup,
  aggregate revision, and bounded telemetry. Formation should extend this authority; it must not
  introduce a second queue in the supervisor or transition driver.
- M04 inserts a ticket into its FIFO before the client acknowledges the reliable `Joined` outcome,
  removes that ticket and command memory when the owning `LobbySessionId` disappears, and currently
  proves that every stored ticket is pooled. M05 must deliberately replace those invariants: only an
  acknowledged membership may form, and a reserved ticket may be temporarily detached from a lobby
  session while remaining owned by exactly one reservation and one Netcode client identity.
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
7. Product flow ends at Queue. There is no `MatchLoading` or `Match` state, Confirmation overlay,
   loading-specific error mapping, authoritative Countdown-to-Match transition, or return-to-queue
   commit path.
8. Match lifecycle still allows only two players per team and the public advertisement validator
   rejects 3v3.
9. M04 removes ticket/session memory on every lobby disconnect, so intentional handoff currently
   destroys the state required for failure requeue or fresh-session reconciliation.
10. The match lifecycle can return Countdown to Waiting and clear readiness after a departure; with
    an immutable manifest and no join-in-progress, M05 needs a terminal Countdown recovery instead
    of leaving that worker unable to activate.
11. Automatic match commands still expose both `SetReady` and `ReadyForRestart`. Product routed
    loading must replace the first with manifest check-in and suppress the second so M05 cannot
    revive the legacy fixed-roster restart path after a completed match.
12. Match-loading ready and cancel intents have no shared ordered application envelope or same-turn
    precedence rule. Supervisor arbitration alone cannot recover ordering discarded inside the
    worker before those facts reach control IPC.

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
| Exact roster | A game type forms only when its FIFO contains at least `team_count * players_per_team` formation-eligible tickets. Formation eligibility requires a current ticket whose reliable `Joined` outcome was application-acknowledged; merely inserting a ticket is insufficient. Formation removes exactly that many oldest eligible tickets and leaves ineligible/overflow tickets ordered in place |
| Recovery visibility barrier | A retained ticket reinserted after dissolution is queued and publicly counted but is formation-ineligible until the client acknowledges that exact reservation's reliable `Requeued` terminal phase. A recovery-pending rebind has the same barrier. The acknowledgement atomically releases the terminal tombstone and restores formation eligibility; it may precede a following Queue Cancel in the existing ordered lobby-client envelope. Session loss removes a live ineligible ticket through ordinary cleanup. Terminal expiry removes an unacknowledged recovered ticket and closes any still-live owning lobby session with the recoverable match-start-expired disposition rather than leaving a client presenting invisible Queue membership or making the ticket silently eligible. Consequently dissolution/rebind and a new reservation may occur in adjacent updates but never in the same update or before the player has observed recovery |
| Visible reservation commit | The formation transaction retains one targeted `ReservationStarted` phase per participant containing reservation/ticket correlation, the public game/map/topology/own-build summary, and bounded duration budgets but no allocation, route, or capability. Observing that phase moves the client from Queue to Match Loading immediately. The server stages it after the acknowledged Joined outcome and before the aggregate snapshot publication intent, but the two channels may arrive in either order; aggregate snapshots never mutate the client's own membership or flow |
| Same-frame race | Queue command/ack collection and authenticated disconnect reconciliation run before formation. A valid Cancel or disconnect observed in that frame wins; once formation commits reservation state, a later Cancel is a reservation-cancel intent |
| Team assignment | Sort the reserved tickets by `(admission_order, ticket_id)` and assign `team = index % team_count`; for v2 `team_count` remains exactly two, yielding exact balanced 2v2 or 3v3 teams without parties, skill, or randomness |
| Map selection | Each game type owns a monotonic offered-map ordinal. Select `map_preset_ids[ordinal % len]` in advertised operator order and bind it immutably to the reservation. Advance only after the supervisor returns one complete validated grant set and the lobby is ready to offer that map; capacity refusal, allocation retry, or failure before Ready retains the ordinal, while a failure after offers advances the next formation |
| Reservation identity | Add unpredictable nonzero `MatchReservationId(u128)` generated by the lobby; one reservation also owns one nonzero monotonic supervisor `RequestId` and later the returned `AllocationId`/`MatchId` |
| Ticket state | A ticket is exactly one of queued, reserved, or recovery-pending. A queued ticket is owned by one FIFO pool and live lobby session. A reserved ticket is owned by one live reservation and may become handoff-detached. A recovery-pending ticket is detached after dissolution, absent from public counts, ineligible for formation, and retains its original FIFO key until explicit authenticated reconcile or expiry. The three ownership sets are disjoint and exhaustive; no ticket can join another reservation |
| Detached handoff lease | Issuing `BeginMatchConnect` marks the live participant binding `BeginIssued`; it does not detach it. The first matching lobby unlink/disconnect observed after Begin converts that binding to a bounded handoff lease keyed authoritatively by manifest Netcode client ID and correlated by ticket/reservation IDs. Ordinary disconnect before Begin removes the culprit. A participant that remains on the lobby after Begin remains live until it unlinks or times out, so a lost Begin cannot create a fictitious detach. On dissolution, a detached retained ticket becomes recovery-pending for a 15-second terminal reconciliation window. Only an explicit `ReservationReconcile` on a fresh lobby session authenticated as the same Netcode identity may atomically reinsert it by original `(admission_order, ticket_id)`; authentication alone does nothing. Duplicate/stale rebinds fail closed; active leases expire at the lobby-owned loading deadline and terminal recovery state expires at its separate reconciliation deadline |
| Host admission | The supervisor keeps sole process/worker/route capacity authority. It admits against existing hard ceilings (one lobby plus at most four match workers under `MAX_WORKERS = 5`, route/capability bounds, pending-allocation bound) before spawning |
| M05 product reservation capacity | M05 admits at most one product reservation or already-Active product match for the logical server. Reaching Active releases per-ticket reservation recovery but does not release this temporary product slot; M06 replaces it with concurrent lifecycle/result ownership. If multiple game types become eligible together, catalog order deterministically selects one and all other pools remain queued only while the first reservation is pre-Active. When it reaches Active, the lobby atomically removes all remaining queued tickets with a bounded `ServerMatchCapacityOccupied` outcome, terminalizes every recovery-pending ticket from an earlier dissolved reservation with the same disposition, pauses new queue admission, and publishes zero queued counts with `FormationAvailability::ProductMatchOccupied` for every game type. A later same-identity reconciliation of a terminalized detached ticket reports removed/capacity-occupied and can never restore queue membership. Game Select disables Build & Join from that fresh authority fact; a racing/stale Join still receives the same rejection. This deliberately bounded one-match M05 behavior is honest rather than leaving an exact roster waiting forever. The supervisor retains its existing four-match infrastructure ceiling, but M06 owns enabling and proving simultaneous product reservations/matches |
| Gameplay selection across IPC | The lobby resolves and sends game type revision, mode, selected map preset/revision, exact two-team topology, rules profile, and stable participant rows. The supervisor validates structural bounds and copies these values; it does not infer them from mode or parse the operator catalog |
| Custom build transfer | Each participant carries a length-bounded opaque `build_snapshot` (maximum 256 bytes) in the routing contract. The lobby encodes one application `MatchBuildSnapshotV1` containing accepted canonical recipe, source preset if any, revision, fingerprint, and point total. The worker decodes, resolves against embedded catalogs, and byte/identity-checks it before Ready. Routing never imports Bevy or Brawler build types |
| Manifest evolution | Replace the one current match/allocation manifest/control schema rather than adding compatibility decoding. Bump the independent BRCT/manifest versions where their below-Lightyear framing changes; bump the one global application compatibility version for lobby/match messages. No message-level fallback versions |
| Grant protocol | After validated worker Ready, the lobby retains the complete secret grant set and sends one bounded capability-free `ReservationOffer` per live reserved session through one ordered matchmaking server envelope. Every participant must acknowledge the exact reservation/allocation/ticket before a roster-wide barrier broadcasts `BeginMatchConnect`; Begin contains only that recipient's grant. No participant can possess a match capability before Begin, and no participant leaves the lobby while another offer remains unacknowledged. The router/worker reject any connection lacking the Begin-delivered capability |
| Capability lifetime | Preserve the supervisor's 30-second pending activation and 10-minute hard lifetime. Product formation allows at most 10 seconds from reservation to validated grant set and at most 20 further seconds from grants to the activation commit, never exceeding the pending-capability window after Ready. Unused grants are revoked on dissolution, so the hard lifetime is never the product wait policy |
| Match admission | A worker accepts only the manifest's Netcode client ID paired with its routed peer and rejects duplicates. All manifests are validated before Ready; no partial roster is added later and join-in-progress remains absent |
| Client check-in | Add one idempotent Ready request in the ordered-reliable `MatchLoadingClientMessage` envelope, scoped to allocation, match, and request ID. The client sends it only after accepted Match Hello, map Ready, terrain Ready for the matching generation, assets are not `Loading` (`Ready` or a valid declared `Degraded` fallback state), and manifest-backed controlled state are all observed |
| Server check-in | The worker records each manifest participant once. Only an exact connected manifest roster with every check-in may set product participants ready and unlock the existing fixed-tick countdown |
| Match-side intent ordering | One ordered-reliable `MatchLoadingClientMessage` envelope carries `Ready`, `CancelMatchStart`, and exact `ServerOutcomeAck` variants on the match session. The worker collects all envelopes in channel order, validates them without mutation, applies every valid cancellation observed in that `Update` before committing any new check-in, and suppresses preparation for a terminal reservation. If preparation was already emitted in an earlier control turn, the supervisor's existing `Loading` versus `CommitGranted` arbitration decides the later cancel. The client flow arbiter likewise suppresses an automatic Ready send in any frame that commits local cancel intent |
| Match-loading server protocol | Worker-to-client authority outcomes use a distinct ordered-reliable `MatchLoadingServerMessage`; replaceable presentation progress uses a distinct sequenced `MatchLoadingStatus`. The client envelope never carries a worker outcome. Server outcomes are correlated to allocation/match/participant plus nonzero server sequence, retained one at a time per participant until exact acknowledgement, and limited to check-in accepted, cancellation accepted, cancellation TooLate, or terminal loading failure. Status contains only generation/revision, bounded phase, and expected/connected/checked-in counts; it may age or be lost and never changes flow, membership, readiness, cancellation disposition, or Countdown authority |
| Activation serialization | After exact check-in, the worker emits `ActivationPrepared` but remains Waiting. The supervisor is the single arbiter while the allocation is `Loading`: cancellation accepted first transitions to `Dissolved`; preparation accepted first transitions to the nonterminal `CommitGranted` phase and returns `CommitActivation`. `CommitGranted` closes the player-cancel window but remains able to reach `Active` or `Dissolved` after a Countdown departure. Only `CommitActivation` permits the worker's fixed-tick Waiting -> Countdown transition; cancellation received after `CommitGranted` returns an idempotent TooLate disposition |
| Countdown | The authoritative `MatchState` remains Waiting through loading. After `CommitActivation`, the existing server fixed-tick transition creates the only Countdown; Match Loading UI never runs an independent countdown. Reservation recovery state remains bounded through Countdown and is released only when the worker reports entry to Active or a terminal Countdown failure |
| Countdown-departure schedule | Product routed workers insert one manifest-loading terminal check after common connected-roster refresh and before `advance_waiting_and_countdown`. During `CommitGranted`/Countdown, a missing expected participant marks the product start terminal and emits `Dissolved(CountdownDeparture)` once; the common lifecycle observes that marker and skips its legacy Countdown -> Waiting/readiness-clear branch. The worker therefore retains Countdown only until supervisor-directed teardown and cannot replicate a false return to Waiting. Direct-UDP composition has no product loading gate and retains its existing reset behavior |
| Cancellation after reservation | Before the supervisor commits activation, a matching `CancelMatchStart` removes the cancelling ticket and dissolves the reservation. It travels over the live lobby session before Begin or over the match worker after Begin. If neither link is live, the client opens a fresh lobby session and sends `ReservationReconcile` with its retained cancel intent. For a still-live pre-commit reservation, successful authentication plus that explicit reconcile intent creates a control-only reconciliation binding and forwards the cancel without requeueing or resuming the failed match; for terminal recovery-pending state the explicit reconcile performs the normal same-identity ticket rebind. Authentication alone never requeues or cancels. Other compatible live-session tickets reinsert by original FIFO key; detached tickets become recovery-pending until explicit authenticated reconcile. Repeated Cancel and TooLate outcomes are idempotent |
| Disconnect/timeout culprit | Before commit, a disconnected or un-checked-in ticket is removed; during Countdown, a departed manifest participant is removed by the same terminal start-failure transaction. Live retained tickets reinsert by original FIFO key and detached retained tickets become recovery-pending. A failure with no attributable participant recovers every still-valid ticket through the same paths |
| Automatic recovery bound | One reservation makes one semantic allocation request; transport may retry only the identical request. Capacity refusal requeues everyone and applies `retry_after` clamped to 1–5 seconds without incrementing failure health. Worker spawn/Ready, route, or internal infrastructure failure increments a per-pool saturating counter: failures one and two apply the same cooldown; failure three pauses that pool for five seconds with an unavailable notice, then resets the counter for a later probe. A validated grant set resets it immediately. No case spends a player budget. A participant-attributable cancel, offer timeout, connection timeout, or check-in timeout removes that culprit immediately; innocent retained tickets are not charged |
| Loading deadlines and clocks | The lobby owns both the 30-second product deadline and its 10-second reservation-to-grant sub-deadline from reservation commit using process-local monotonic instants. The supervisor independently owns its 10-second allocation/Ready timer and, after Ready, a 20-second loading/activation timer; the worker independently caps check-in at 20 seconds after Ready. No monotonic instant crosses IPC or the network. Allocation/control messages carry bounded duration budgets, and offers/Begin carry only `remaining_loading_millis` derived and clamped by the sender for client presentation. The client starts a local presentation timer that never decides authority. The earliest correlated lobby, supervisor, or worker expiry enters the same serialized dissolution path. Measured p95/max evidence may tighten, but not silently lengthen, these review values |
| Worker-to-lobby terminal fact | Extend bounded control lifecycle with progress resolved by stable allocation/reservation IDs: `ActivationPrepared`, `CommitActivation`, `ActivationCommitted`, `Activated`, or `Dissolved { reason, missing participants }`. The supervisor validates correlation and serializes cancellation against commit; it does not decode gameplay state. `Activated` means the worker reached Active, not merely that Countdown began |
| Client failure path | A dissolved reservation returns retained clients to Queue after live/fresh lobby authentication with a bounded notice. A removed/cancelling client returns to Game Select. An unexpected match-session failure before Active starts a fresh lobby session and reconciles by reservation identity; it never resumes the failed match connection |
| Product/legacy boundary | Product routed workers use automatic loading check-in. The direct-UDP baseline retains explicit build/ready behavior. The M01 transition smoke is migrated to the product reservation contract before its old exact-two driver can be removed |
| Product flow/overlay | M05 adds both `ClientFlow::MatchLoading` and the minimal `ClientFlow::Match` required by authoritative Countdown/Active observation. It also adds one narrowly owned `ClientOverlay::Confirmation(CancelMatchStartConfirmation)` rather than assuming a pre-existing general confirmation framework. Countdown is the only transition from Match Loading to Match; current debug combat/HUD presentation remains until M07 |
| M05 post-Active behavior | Product-routed `SetReady` and `ReadyForRestart` are both unavailable. The existing authoritative match may progress through Active to Completed, but M05 keeps the client in minimal Match presentation, shows the replicated completed result using existing bounded debug facts, and exposes only **Disconnect** to Server Select after completion. The worker and `ActiveProductMatchOccupancy` remain allocated until operator/server shutdown; no restart, Results, Queue Again, Change Game, or silent return is fabricated. The M05 playtest explicitly states that one logical-server process hosts one product match and must be restarted for another |
| M05/M06 boundary | The supervisor activation commit closes the player-facing Cancel Match Start window and authorizes Countdown, but bounded reservation recovery remains until Active. A Countdown departure is an M05 terminal start failure: stop the worker, remove the culprit, and reconcile/requeue retained tickets. Once the worker reaches Active and emits `Activated`, disconnect/forfeit during Active, reusable worker completion cleanup, Results, return-to-lobby, Queue Again, Change Game, and concurrent admission belong to M06. M05 owns only the honest one-match terminal presentation and queue-admission pause needed to avoid stranded players or legacy restart behavior |

This specification uses three distinct terms deliberately: **pre-commit** is the cancelable loading
period while the supervisor allocation is `Loading`; **Countdown committed** begins at the
nonterminal `CommitGranted` control phase, is authoritative and no longer player-cancelable, but
still retains M05 start-failure recovery; **Activated** means the match reached
`MatchPhase::Active`, the supervisor observed `Activated`, and ownership crossed to M06. The
supervisor allocation lifecycle is therefore `Loading -> CommitGranted -> Active`, with
`Loading -> Dissolved` and `CommitGranted -> Dissolved` as the two failure exits. No implementation
or UI text may collapse these into one ambiguous "activated" or terminal flag.

## Scope

### Included

- exact FIFO formation for advertised 2v2 and 3v3 game types;
- application-acknowledged Join as the formation-eligibility barrier;
- atomic targeted `ReservationStarted` delivery and immediate Queue -> Match Loading presentation;
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
- minimal product Match flow entered only from replicated Countdown, plus one focused confirmed
  Cancel Match Start overlay owned by M05;
- capability-free roster-wide offer acknowledgement, one grant-bearing Begin barrier, explicit
  Begin-issued/live versus observed-unlink/detached state, bounded detached-ticket leases, and fresh
  lobby-to-match Lightyear handoff;
- exact manifest connection admission, client readiness predicate, idempotent check-in, loading
  deadline, supervisor-serialized activation commit, and one server-owned countdown;
- minimal Countdown-departure termination and retained-ticket recovery before Active;
- bounded dissolution, original-order recovery with terminal-acknowledgement formation barrier,
  retry/cooldown, route revocation, and process teardown;
- privacy-safe formation/allocation/loading telemetry and deterministic evidence;
- migration of the explicit routed smoke to exercise product formation and preservation of the
  named direct-UDP baseline;
- one product reservation or Active product match at a time; M06 enables concurrent product
  reservations and matches against the already-bounded supervisor capacity;
- bounded one-match post-Active behavior: pause further admission, remove already-queued overflow
  and terminalize older recovery-pending tickets with an honest capacity outcome, suppress product
  restart commands, retain the worker/occupancy, and expose only completed-match Disconnect until
  M06 supplies reusable lifecycle.

### Deferred

- product Results, Queue Again, Change Game, leave/forfeit, normal return-to-lobby, reusable worker
  completion cleanup, and reopening admission after the first Active match (M06); M05 may display
  only the existing replicated completed result in minimal Match presentation with Disconnect;
- Active-match disconnect semantics after the worker emits `Activated` (M06); M05 owns only the
  minimal terminal recovery for a departure during Countdown;
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
    flow.rs                    sole arbiter; gains MatchLoading, Match, and focused Confirmation
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
  -> Reserved(ReservationStarted retained; allocation not requested)
  -> AllocationPending
  -> OfferPending
  -> BeginIssued(live lobby binding)
  -> HandingOff(detached after observed unlink)
  -> CheckingIn
  -> ActivationPrepared
  -> CountdownCommitted
  -> Active(reservation recovery released to M06)
   \> any pre-Active failure -> Dissolving
        -> removed culprit
        -> live retained ticket Queued(terminal acknowledgement pending)
        -> detached retained ticket RecoveryPending -> authenticated reconcile
             -> Queued(terminal acknowledgement pending)
        -> exact terminal acknowledgement -> formation eligible
```

A reservation contains only bounded stable/application values:

- `MatchReservationId`, game/catalog/configuration identity, mode and rules profile;
- selected map preset/revision and offered-map ordinal;
- exact topology, process-local lobby deadline, and bounded duration budgets for other authorities;
- ordered participant rows containing ticket/optional-session/player/Netcode identity, assigned
  team, accepted build transfer, retained matchmaking phase, Begin-issued flag, handoff/check-in
  phase, and process-local detached-lease expiry;
- optional supervisor request/allocation/match/worker identity once learned;
- one terminal disposition flag so cancellation, timeout, disconnect, supervisor rejection, and
  worker failure cannot dissolve twice.

The queue invariant becomes `tickets == pooled union reserved union recovery_pending`, with all
three sets disjoint and every ticket present in the Netcode-client index. A queued ticket must have
one live session index, one pool position, and an explicit formation-eligibility bit. Fresh Joined
acknowledgement or exact recovered-terminal acknowledgement sets that bit; reinsertion/rebind clears
it. A reserved ticket has one reservation owner and either
one live membership session index (optionally marked Begin-issued) or one handoff-detached lease,
never both or neither. A separate authenticated reconciliation-control binding may refer to a
detached reserved ticket only to observe status or submit a retained cancel intent; it is not queue
membership and cannot receive another grant. A recovery-pending ticket has one detached
lease/terminal disposition, no membership session index, and no pool position. Session removal takes
an explicit ordinary-loss or Begin-issued handoff path; it may not infer intent merely from entity
disappearance. A recovered queued ticket whose terminal phase remains unacknowledged is publicly
counted but cannot form; session loss removes it, and terminal expiry removes it rather than silently
restoring eligibility and closes any still-live owning lobby session so the client cannot continue
presenting stale Queue membership.

When the supervisor reports `Activated`, the lobby removes the activated reservation tickets from
`QueueState` and installs one minimal `ActiveProductMatchOccupancy { match_id }` for the temporary M05
product slot. It contains no gameplay or participant state and blocks further formation. In the same
bounded transaction, every remaining queued ticket receives `ServerMatchCapacityOccupied`, is
removed, and returns to Game Select. Every recovery-pending ticket from an earlier dissolved
reservation is also removed from ticket ownership and its existing one-per-participant tombstone is
updated in place to `ServerMatchCapacityOccupied`; its terminal-reconciliation deadline restarts at
15 seconds from this capacity transaction. A later explicit same-identity `ReservationReconcile`
returns removed/capacity-occupied, never rebinds or inserts the ticket, and may acknowledge/release
that tombstone. Aggregate queued counts become zero and later Join attempts receive the same
fail-soft unavailable outcome. This is not a player penalty or infrastructure-health failure. M06
replaces that occupancy with concurrent result/return/requeue lifecycle; M05 does not free the slot
merely because per-ticket start recovery ended or the existing match reaches Completed.

M04 `QueueCommandMemory` remains session-scoped and is not carried across handoff. After Begin, the
old session memory may be removed only after its acknowledged membership has been copied into the
reservation. A successful fresh-session rebind initializes new command memory with that ticket as
the active membership, no inherited pending queue command, and a new request-ID sequence; old
session request IDs cannot mutate the rebound ticket. Offer acknowledgement and terminal-phase
acknowledgement are owned separately from queue-command idempotency.

The public queue snapshot still counts only queued tickets. Its revision changes once when an exact
roster leaves the pool, once for the dissolution's immediately requeued live tickets, once per later
atomic recovery-pending rebind transaction, and once for the Active-capacity transaction that both
removes queued/recovery-pending overflow and changes formation availability. No transient zero/re-add
snapshot is published inside one transaction. Recovered tickets are inserted by their original
`(admission_order, ticket_id)` relative to the current pool, preserving FIFO precedence without
reversing asynchronously reconnecting peers, but formation skips them until their exact `Requeued`
terminal phase is acknowledged. Targeted reservation messages carry no other
participant names, identities, builds, or check-in detail. A bounded terminal tombstone retains only
reservation/ticket/Netcode correlation and disposition long enough for fresh-session reconciliation,
then is released after an authenticated terminal-phase acknowledgement or expires 15 seconds after
the latest terminal transaction. Expiry removes any still-ineligible recovered ticket associated
with that tombstone and closes its still-live owning session with the recoverable expired
disposition.

The Active-capacity transaction uses ticket-only correlation for each removed live queued ticket. It
sends `QueueMembershipEnded { reason: ServerMatchCapacityOccupied }` and retains that exact sequence
until acknowledgement or lobby-session loss. It does not manufacture a queue request ID, enter
`QueueCommandMemory`, or pretend the player cancelled. A detached recovery-pending ticket instead
keeps its existing reservation/ticket correlation in the updated tombstone so a later authenticated
`ReservationReconcile` can receive the same removed/capacity-occupied disposition without creating
membership.
For a session whose Joined outcome is still application-unacknowledged, publication stages that
already-retained outcome first and the ticket-only termination second on `SessionChannel`; the
client therefore observes membership before its removal, and the later Joined acknowledgement may
clear command memory but cannot restore the removed ticket.
The client acknowledges it, clears only the matching membership, enters Game Select, and shows
**Server is hosting its current match. Try again after it restarts.** New Join receives the ordinary
request-correlated `QueueRejection::ServerMatchCapacityOccupied` while occupancy remains. The fresh
aggregate snapshot carries `FormationAvailability::ProductMatchOccupied` for every advertised game
type, so Game Select disables **Build & Join** without treating a stale snapshot as membership
authority.

### Deterministic formation transaction

In one lobby `Update`:

1. snapshot sessions authenticated at frame start;
2. collect ordered queue messages and outcome acknowledgements;
3. collect bounded supervisor reservation/activation facts without mutating queue authority;
4. reconcile routed disconnects using explicit ordinary-loss versus handoff intent;
5. apply Queue Join/Cancel transactions in the M04 stable order;
6. mark a fresh ticket formation-eligible only after its exact `Joined` outcome acknowledgement
   commits; mark a recovered ticket eligible only after its exact `Requeued` terminal-phase
   acknowledgement commits, and remove an ineligible recovered ticket whose terminal phase expires;
7. apply client/supervisor facts to existing reservations and stage any dissolution;
8. commit dissolutions/requeues;
9. if the one M05 product slot has no reservation or Active match, scan advertised game types in catalog order and
   form the first eligible exact roster whose pool is not in cooldown; tickets reinserted or rebound
   by steps 7–8 remain ineligible in this update because their terminal phase cannot yet have been
   observed and acknowledged;
10. retain and stage one targeted `ReservationStarted` phase for every newly reserved participant;
11. stage those reliable targeted phases before the aggregate snapshot publication intent from the
    final state; delivery channels may reorder, so the client continues to derive its own membership
    and flow only from targeted phases;
12. stage bounded allocation controls for the worker control owner.

At most eight game types and 32 authenticated sessions make a full ordered scan sufficient. No
priority queue or general matchmaking framework is warranted. M05 forms at most one product
reservation globally; later eligible pools remain ordered and visible while that reservation is
pre-Active. If it dissolves, retained tickets recover normally. If it reaches Active, the bounded
capacity transaction above clears overflow and pauses admission instead of leaving a complete roster
visibly waiting forever. M06 may raise that product bound only with simultaneous lifecycle and
isolation evidence, while the supervisor's existing infrastructure ceilings remain unchanged.

The M04 lobby chain is extended visibly rather than hidden inside one system:

```text
BeginLobbyFrame
  -> AuthenticateLobbyHellos
  -> CollectQueueAndMatchmakingClientMessages
  -> CollectSupervisorReservationFacts
  -> ReconcileDisconnectedSessions
  -> ApplyQueueTransactionsAndAcknowledgements
  -> ResolveReservationIntents
  -> CommitDissolutionsAndRequeues
  -> FormExactReservations
  -> PublishTargetedPhases
  -> PublishAggregateSnapshot
  -> StageAllocationControls
```

Each phase coordinates a bounded transaction owned by `QueueState`/the reservation resource; it does
not duplicate authority in frame scratch state. Requeued tickets retain their original order. A
pool cooldown or unavailable window is checked at formation, and a final snapshot always reflects
the state committed by every earlier set in that update.

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
6. waits for manifest digest/version/identity Ready under its process-local lifecycle timer;
7. registers all routes/capabilities atomically or rolls the worker back;
8. returns the complete grant set to the lobby.

The worker decodes `MatchBuildSnapshotV1`, resolves the canonical recipe with embedded catalogs,
and proves revision, preset identity, recipe fingerprint, point total, topology, map capacity,
mode/rules compatibility, protocol registry, and content fingerprint before Ready. A malformed row
fails the complete manifest; no participant receives a grant.

### Two-phase client handoff

The earlier `ReservationStarted` phase contains the receiving player's ticket/reservation IDs,
game/map/topology summary, its own accepted build summary, and the lobby-derived bounded remaining
loading duration. It contains no allocation or capability. The client accepts it only if the ticket
matches its current acknowledged Queue membership and connection generation, then immediately
commits flow to Match Loading. It may coexist with an older aggregate snapshot, but it may not
precede acknowledgement of that ticket's `Joined` outcome.

After worker Ready, `ReservationOffer` adds the receiving player's allocation/match IDs and refreshed
`remaining_loading_millis`, but still contains no route grant or capability bytes. The client accepts
it only for the current reservation/ticket/generation and sends `ReservationOfferAck` on the existing
one ordered client envelope.

The lobby retains one current semantic matchmaking phase per participant. It waits until every live
reserved participant has acknowledged the offer, then broadcasts one sequenced
`BeginMatchConnect` to the complete roster. Each targeted Begin adds only that recipient's redacted
in-memory route grant and refreshed `remaining_loading_millis`. A participant cannot learn any
capability before the complete-roster barrier and never leaves the lobby while another offer remains
unacknowledged. Reliable transport may retransmit bytes, while exact reservation/phase identity
makes duplicate application observation idempotent. Begin does not require a completed lobby
round-trip acknowledgement before the client acts: observed intentional lobby unlink is the server
handoff fact, and subsequent worker admission/check-in or an authoritative local deadline resolves
delivery.

On Begin, the client:

1. validates and moves the Begin-delivered grant into the existing Match Loading context;
2. intentionally disconnects and unlinks the lobby entity;
3. waits through the existing explicit deferred despawn boundary;
4. clears lobby-generation queue/editor/snapshot resources but retains one bounded loading context
   containing only the offered public summary, redacted in-memory grant, identity correlation,
   locally measured remaining duration, and pending cancel intent;
5. creates exactly one fresh routed match entity with the Begin-delivered capability;
6. performs normal Netcode and Brawler Match Hello authentication.

Duplicate/stale reservation phases, offers, acknowledgements, Begin messages, or old-generation
network data are ignored by exact identity. The client never logs or persists the capability. A
lobby disconnect before Begin is unexpected and recovers through a fresh lobby session; the
lobby's process-local reservation deadline remains authoritative. A client cannot attempt a routed
match connection from `ReservationStarted` or `ReservationOffer` because neither carries the route
secret.

Issuing Begin marks the still-live server participant binding `BeginIssued`. Only the later observed
unlink/disconnect converts it to a detached handoff lease. The ordinary M04 disconnect observer
removes a reserved ticket when no matching Begin was issued and preserves it when Begin was issued;
a client that never observes Begin remains live until unlink or timeout rather than becoming
fictitiously detached.

After losing the match link or learning of a pre-Active failure, the client authenticates a fresh
lobby session and sends one bounded `ReservationReconcile { reservation_id, ticket_id,
cancel_requested }` intent. The lobby authorizes it with the new session's Netcode client ID, never
with the correlation IDs alone. For a still-live pre-commit reservation, it installs one
control-only reconciliation binding, reports the current public phase, and atomically forwards a
retained cancel intent when requested; it does not requeue the ticket, issue another grant, or resume
the failed match. For terminal recovery-pending state, it rebinds the retained ticket and reports
Queue plus the exact reliable `Requeued` terminal phase; the ticket remains formation-ineligible
until that phase is acknowledged. If Active-product occupancy terminalized the record,
reconciliation instead returns removed/capacity-occupied and cannot rebind; otherwise it returns
removed/expired/TooLate. Concurrent duplicate reconnects admit at most one control or membership
binding for the identity; stale or wrong-identity reconciliation fails closed.

Fresh authentication by itself never mutates reservation or queue membership. A same-identity
ordinary Queue Join received while a recovery-pending ticket still exists returns one bounded
`RecoveryPending { retry_after_millis }` rejection and leaves both states unchanged; the client may
complete its retained reconciliation or wait for the 15-second terminal expiry before a new Join.
Choosing **Disconnect** while returning to queue abandons only local recovery, retains no automatic
reconcile action, and therefore cannot silently requeue on a quick same-server reconnect. The server
continues to expire the bounded recovery-pending record authoritatively.

### Match Loading interaction contract

Match Loading presents **Cancel Match Start** while the supervisor allocation remains pre-commit.
Activating it opens the new narrowly scoped
`ClientOverlay::Confirmation(CancelMatchStartConfirmation)`; the context retains only the current
reservation/ticket/generation and Match Loading return flow. Focus defaults to the non-destructive
**Keep Loading** action, and only explicit **Cancel Match Start** confirmation sends intent. Before
Begin the confirmed intent uses the lobby envelope. After Begin it uses the authenticated match
connection; while neither connection is live, the client retains one cancel intent, establishes a
fresh lobby session, and sends it with reservation reconciliation. The supervisor resolves the
arrival race: accepted cancellation returns the player to Game Select and retained peers to Queue;
`TooLate` closes the confirmation and removes the action. If the match link is live, replicated
authoritative Countdown takes over. If the match link was already lost, Match Loading shows
**Match starting…** without pretending to resume it and waits for the worker's inevitable
pre-Active departure/dissolution fact, after which normal removed/retained reconciliation applies.
The action is not presented during Countdown; M06 later supplies normal Leave Match behavior after
Active.

Controller South/Confirm or keyboard Enter on the loading action opens confirmation, and pointer
click uses the same `FlowUiAction::RequestCancelMatchStart`. The overlay's destructive confirmation
uses a distinct `FlowUiAction::ConfirmCancelMatchStart`. Controller East/Cancel or Escape dismisses
confirmation or moves focus back to **Cancel Match Start**, but never tears down a socket implicitly.
Focus survives phase-text updates. No network or UI system bypasses the existing flow arbiter.
Closing the overlay restores focus to **Cancel Match Start** only if that exact reservation remains
pre-commit; flow change, generation change, terminal outcome, or commit clears it exactly once.

After dissolution, a detached client remains in Match Loading with **Returning to queue…** while it
establishes a fresh lobby session and reconciles. During this bounded state the only action is
**Disconnect**, which abandons local recovery and returns to Server Select without claiming that the
server-side ticket was synchronously removed. Successful retained reconciliation enters Queue with
fresh membership/population and a notice; removed/expired disposition enters Game Select with the
last accepted build available through the ordinary Build Editor. Failure to authenticate before the
15-second terminal reconciliation expiry follows that same expired disposition rather than leaving
an indefinite loading screen.

Terminal presentation maps authority outcomes consistently:

| Outcome | Notice | Primary destination/action |
|---|---|---|
| Player cancelled or was the missing participant | Match start cancelled / could not complete | Game Select; ordinary **Build & Join** remains available |
| Retained ticket requeued | Match start could not complete; place retained | Queue; **Cancel Queue** remains available |
| Infrastructure/capacity unavailable | Server could not start the match; place retained after cooldown | Queue; no manual retry loop |
| Ticket expired/removed during reconciliation | Match start expired | Game Select; **Build & Join** opens the ordinary editor with the last accepted build |
| Activation already committed | Match starting | No cancel action; live match waits for authoritative Countdown, lost match waits for terminal start-failure reconciliation |
| Unreserved queue ticket removed when the first match becomes Active | Server is hosting its current match | Game Select; **Build & Join** disabled by fresh occupied availability, Disconnect remains available |

Presentation may show only the bounded authoritative phase and aggregate `connected / expected` or
`checked in / expected` counts. It never shows peer names, builds, failure attribution, capability,
or percentage/estimated progress. Replaceable `MatchLoadingStatus` supplies those counts and phase;
its freshness may change copy to a neutral **Waiting for server…**, but it cannot change flow or any
authority disposition. Reliable `MatchLoadingServerMessage` outcomes supply check-in acceptance,
accepted cancellation, TooLate, or terminal failure and are acknowledged by exact server sequence.

When replicated authoritative Countdown is observed for the loading match/generation, the sole flow
arbiter clears Confirmation/loading-only focus state and commits `ClientFlow::Match`. The existing
combat presentation remains in use. If the match later reaches Completed, minimal M05 Match
presentation shows the replicated result and **Disconnect** only; neither manual nor headless
product clients send `ReadyForRestart`. Active-match leave/forfeit and a reusable Results flow remain
M06 work.

### Match loading and check-in

The match worker creates a `MatchLoadingState` from the validated manifest before accepting
connections. Match Hello may create the participant fighter and replicated state as today, but the
participant remains non-ready and input-gated. Product routed sessions do not expose or send
`SetReady` or `ReadyForRestart`.

The client readiness predicate requires all of:

- current routed phase is Match and connection generation matches loading context;
- `ClientJoinPhase::Active` is accepted for the manifest-backed participant;
- the replicated `MatchState.match_id` equals the offer;
- `ClientMapReadiness::Ready` and the presented map identity equals the selected manifest map;
- `ClientTerrainReadiness::Ready` refers to that map/terrain generation;
- retained product client assets are not `Loading`: either `Ready` or `Degraded` with every required
  asset covered by its declared deterministic fallback;
- exactly one controlled fighter exists with the accepted player/build identity;
- no disconnect, invalid convergence, or local loading error is active.

It then sends one idempotent Ready variant through `MatchLoadingClientMessage` and waits. If the flow
arbiter commits cancel intent in the same client frame, it suppresses that Ready send. The worker
validates request identity, manifest membership, current connected link, map/match identity, and
duplicate/stale semantics. It records check-in only after applying all valid same-`Update` cancels,
retains one correlated `MatchLoadingServerMessage::CheckInAccepted` until exact acknowledgement, and
does not trust the client for map content, build data, team, or gameplay state. Observing or
acknowledging CheckInAccepted does not move the client to Match; only replicated Countdown does.

Once every manifest participant is still connected and checked in, one fixed-tick preparation
system emits `ActivationPrepared` once but leaves participants non-ready and `MatchState` Waiting.
The supervisor serializes this fact against `CancelActivation`; only `CommitActivation` may arm one
fixed-tick commit system. That system marks the exact expected manifest roster ready before common
roster refresh, and the existing authoritative lifecycle transitions Waiting -> Countdown. The
worker reports `ActivationCommitted` after observing Countdown and reports `Activated` only after
the lifecycle reaches Active. Client flow enters Match only from replicated authoritative
Countdown, not from its sent check-in, a lobby prediction, or `ActivationPrepared`.

The worker installs an `ExpectedManifestRoster` separately from `MatchLifecycleRules`. The expected
roster requires exactly the manifest's four or six participants for loading and activation. The
worker derives maximum participants per team from the validated manifest topology, while the common
lifecycle minimum remains a mode/lifecycle concern for later Active-match forfeit handling. The
direct-UDP composition retains its existing default two-player-per-team rules and explicit Ready
path. One checked-in advertised Hot Zone 3v3 operator fixture is required for product/process tests.

If any manifest participant departs after commit but before Active, the worker does not return to an
unrecoverable Waiting state. The product loading gate observes the refreshed roster while
`MatchState` is still Countdown, marks the start terminal, and emits one
`Dissolved(CountdownDeparture)` before the common lifecycle may evaluate its legacy reset branch.
`advance_waiting_and_countdown` skips that branch when the exact product start carries the terminal
marker, so Waiting/readiness-clear cannot be replicated. The supervisor revokes routes and stops the
worker, and the lobby removes the culprit and reconciles/requeues the retained detached tickets. Rich
forfeit or result semantics remain deferred because gameplay never became Active.

### Clock and activation ownership

Every process stores only its own monotonic instants:

- the lobby starts the authoritative 30-second reservation timer and a 10-second
  reservation-to-complete-grant-set sub-timer when formation commits and may cancel the correlated
  allocation when either applicable timer expires;
- the supervisor starts a 10-second spawn/Ready timer when it accepts the allocation and a separate
  20-second prepare/commit timer when it validates Ready;
- the worker starts a locally measured check-in timer capped at 20 seconds after Ready and reports
  expiry rather than comparing a foreign timestamp;
- the client converts received `remaining_loading_millis` into a local presentation timer, clamps it
  downward on newer phases, and never sends a timeout as authority.

IPC/application messages carry duration budgets or bounded remaining milliseconds, never an
`Instant`, wall-clock timestamp, or assumption of synchronized epochs. Transport time consumes the
recipient's apparent remaining budget. Any authority may observe expiry first, but the supervisor's
correlated allocation state serializes the result: `Loading` accepts either cancellation/dissolution
or one preparation and becomes `Dissolved` or `CommitGranted`; `CommitGranted` rejects player cancel
as TooLate but may still become `Dissolved` on start failure or `Active` after the worker reports
Active. Duplicate and late expiry/progress facts receive the already-committed disposition.

### Dissolution and requeue policy

All pre-Active start failures, including Countdown departure, converge on one transaction:

1. mark the reservation terminal exactly once;
2. revoke every allocation route and stop the incomplete worker under existing bounded process
   deadlines;
3. identify explicit cancelling/disconnected/timed-out participants when evidence exists;
4. remove those tickets and retain a bounded terminal outcome keyed to their authoritative Netcode
   identity for delivery after current/fresh lobby authentication;
5. retain tickets whose authoritative Netcode identities remain catalog-compatible; immediately
   reinsert live-session tickets into their original pool by `(admission_order, ticket_id)` as
   formation-ineligible and move detached tickets to recovery-pending without allocating new ticket
   IDs or admission revisions;
6. on an explicit `ReservationReconcile` from a matching freshly authenticated session, atomically
   rebind and insert a recovery-pending ticket into its original pool by the same FIFO key as
   formation-ineligible; authentication alone does nothing and expiry removes it;
7. publish one final aggregate revision and apply a per-pool cooldown when requested;
8. deliver an immediate or newly rebound retained ticket's current Queue membership plus reliable
   `Requeued` terminal notice to at most one authenticated lobby session;
9. accept one exact `MatchmakingTerminalAck` from that authenticated session to atomically release
   delivered terminal state and make that recovered queued ticket formation-eligible; detached or
   undelivered tombstones remain only until reconciliation or expiry;
10. on terminal expiry, remove any associated ineligible recovered ticket and close its still-live
    owning lobby session with the recoverable match-start-expired disposition rather than silently
    enabling it or leaving invisible membership; discard capability bytes immediately and other
    reservation/tombstone state after bounded terminal acknowledgement or expiry.

Original-key reinsertion preserves the retained players' FIFO precedence over newer overflow
tickets even when detached peers reconnect in a different order. Recovery-pending tickets are not
publicly counted and cannot form before rebind. Rebound and immediately requeued tickets are publicly
counted but cannot form before exact terminal acknowledgement, so the Queue recovery and notice are
observable and no terminal phase can be overwritten by an immediate second reservation. The policy
does not promise that retained players remain together or retain prior teams/map on the next exact
formation.

Cancellation removes only the canceller. A capacity refusal has no culprit and requeues everyone;
its cooldown prevents a busy-loop. Host-wide and worker infrastructure failure never spends an
individual ticket budget. A participant-attributable offer, connection, or check-in timeout removes
that participant immediately, so innocent retained tickets do not inherit another player's failure.
Repeated infrastructure failure increases only bounded pool/server health state and may temporarily
stop formation with a recoverable unavailable notice; it cannot create a hot allocation loop or
silently eject queued players.

### Schedule and role boundaries

Lobby application systems continue to observe typed Lightyear messages after receive in `Update`.
The formation/reservation sets extend M04's chain while keeping the queue mutation and snapshot
publication order visible. Worker control IPC still has exactly one stream reader/writer owner;
Bevy resources provide bounded inbox/outbox handoff rather than reading Unix streams from gameplay
systems.

Match loading client envelopes are collected in ordered channel order in `Update`. One transaction
validates the batch, commits every valid cancel before any new Ready/check-in, and forwards terminal
cancel control before a later fixed-tick preparation can inspect the roster. A cancelled reservation
cannot prepare in that turn. A valid cancellation from any participant wins over a last check-in
observed in the same worker `Update`; cancellation racing with preparation already emitted in an
earlier control turn is resolved only by the supervisor's allocation state. Fixed-tick preparation
can emit `ActivationPrepared` but cannot mutate lifecycle readiness. The supervisor control owner
serializes that request with cancellation and returns `CommitActivation` in a later control turn. A
worker fixed-tick commit set consumes that grant exactly once before common roster refresh and
lifecycle evaluation. An explicit deferred boundary makes readiness mutation visible before roster
refresh. Countdown creation stays in the existing lifecycle set. Product workers extend its visible
fixed-tick chain as follows:

```text
CommitProductActivation
  -> ApplyDeferred
  -> RefreshConnectedMatchRoster
  -> DetectProductCountdownDeparture
  -> AdvanceWaitingAndCountdown
  -> ObserveProductCountdownOrActive
```

`DetectProductCountdownDeparture` reads the refreshed exact manifest roster. When it marks the start
terminal, `AdvanceWaitingAndCountdown` must skip the common Countdown -> Waiting/readiness-clear
branch for that match; the terminal marker and dissolution outbox write are immediate resource
mutations and need no deferred visibility. Direct-UDP composition omits the product detector/marker
and retains existing behavior. The following observer reports Countdown commit or Active, while a
terminal marker reports only the already-staged dissolution; network sends remain buffered for
Lightyear `PostUpdate`.

The dedicated-server feature graph remains free of rendering, windowing, audio, device input, and
client assets. The routing package remains Bevy-free. Product UI never enters supervisor or server
features.

## Protocol and bounds

### Application protocol

Register the smallest bounded current-schema messages in `protocol.rs`:

- one `LobbyMatchmakingServerMessage` envelope carrying nonzero server sequence, reservation/ticket
  correlation and exactly one current phase: reservation started, capability-free offer,
  grant-bearing Begin, returning/requeued, removed, or expired; or ticket-only correlation for the
  single `QueueMembershipEnded(ServerMatchCapacityOccupied)` phase;
- client offer acknowledgement, `CancelMatchStart`, `ReservationReconcile { cancel_requested }`, and
  `MatchmakingTerminalAck { correlation, server_sequence }` using the existing one ordered lobby
  client envelope. Correlation is a bounded enum containing either reservation plus ticket or the
  ticket-only Active-capacity termination; invalid phase/correlation combinations fail decoding;
- one client-to-worker ordered-reliable `MatchLoadingClientMessage` envelope carrying nonzero client
  sequence/request identity, allocation/match/participant correlation, and exactly one of Ready,
  Cancel Match Start, or `ServerOutcomeAck { server_sequence }`;
- one worker-to-client ordered-reliable `MatchLoadingServerMessage` carrying the same stable
  allocation/match/participant correlation, nonzero server sequence, and exactly one retained
  authority outcome: check-in accepted, cancellation accepted, cancellation TooLate, or terminal
  loading failure;
- one worker-to-client replaceable sequenced `MatchLoadingStatus` containing connection generation,
  monotonically increasing status revision, bounded presentation phase, and only
  expected/connected/checked-in aggregate counts.

`protocol.rs` registers `MatchLoadingClientMessage` only client-to-server and
`MatchLoadingServerMessage`/`MatchLoadingStatus` only server-to-client. The two reliable types use
`SessionChannel`; status uses a dedicated `MatchLoadingStatusChannel` configured sequenced-unreliable
with unsent retry disabled. The worker publishes status on semantic phase/count mutation and refreshes
the byte-equivalent current revision once per second; the client treats it as stale after three
seconds and then presents neutral **Waiting for server…** copy without mutating authority. A client
may acknowledge only the exact current reliable server sequence. Each worker participant retains at
most one application-unacknowledged server outcome and at most one latest status snapshot; a later
authority outcome is staged until the earlier outcome is acknowledged, while status revisions may
supersede one another. Status loss or aging affects presentation only. Client flow changes only from
reliable targeted authority outcomes or replicated `MatchState`; status cannot accept
check-in/cancellation, move flow, or start Countdown.

Extend each complete `QueuePoolSnapshot` row with bounded `FormationAvailability::Open` or
`ProductMatchOccupied`. Only the lobby derives it from product occupancy. It is presentation/admission
availability, not client membership authority; stale availability ages under M04's existing snapshot
freshness policy and cannot create or remove a ticket.

The lobby retains at most one current semantic matchmaking server phase per live session or detached
lease and at most one terminal tombstone per reservation participant or Active-capacity-removed
ticket. A newer phase supersedes the
older retained phase only through the specified state transition; byte retries do not allocate
history. Queue-command outcomes remain M04's separate one-pending-outcome contract. Formation cannot
start until `Joined` is acknowledged, so `ReservationStarted` never competes with an unacknowledged
Join outcome. The lobby stages `ReservationStarted` before the aggregate snapshot publication intent
that removes the formed roster, but delivery channels remain independently ordered and may arrive in
either order without changing client membership. `ReservationOfferAck` is the roster barrier;
`MatchmakingTerminalAck` releases only the exact delivered terminal sequence/correlation; for a
recovered queued ticket it also commits formation eligibility. Begin is idempotent by
reservation and phase sequence; observed unlink, worker admission/check-in, or an authoritative
process-local deadline is its completion fact. Begin is the only application message containing a
route capability.

Every ID is nonzero and every collection has a custom bounded deserializer. Canonical encoded-size
tests cover maximum 3v3/8-participant shapes. Route capability Debug remains redacted. The one global
compatibility handshake changes once with the application registry; old clients fail cleanly rather
than decoding fallback forms.

### Routing/control protocol

Evolve the one current routing contract with:

- product allocation request fields and maximum 256-byte opaque build snapshot per participant;
- allocation cancellation/revocation correlated by request/allocation identity;
- worker/supervisor/lobby loading progress, activation prepare/commit, cancellation arbitration, and
  terminal activation/dissolution facts;
- manifest topology/game/map/build fields;
- explicit rejection categories for malformed, capacity, incompatible, spawn/Ready, cancelled,
  expired, and internal failures.

Keep control records below 64 KiB and match manifests below 4 KiB. With at most eight participant
rows and 256 build bytes each, the semantic 4 KiB manifest bound remains feasible and must be proven
by a maximum-shape test rather than assumed. Control queues retain their existing frame/byte bounds;
new progress bodies cannot create unbounded history.

The supervisor allocation state is explicitly `Loading`, `CommitGranted`, `Active`, or `Dissolved`.
Preparation is an input observed while `Loading`, not a terminal state. Capability bytes may be
returned only to the lobby control owner, are withheld from `ReservationOffer`, and enter the
application protocol only in the recipient-specific Begin phase. No control or application record
serializes a process-local monotonic instant.

### Operational bounds

| Bound | M05 value |
|---|---|
| Authenticated lobby sessions | 32 |
| Total tickets | 32 across queued, reserved, and recovery-pending states |
| Advertised game types | 8 |
| Teams | exactly 2 |
| Players per team | 2 or 3 |
| Formed roster | exactly 4 or 6; routing structural maximum remains 8 |
| Match workers | at most 4 plus one lobby under current `MAX_WORKERS = 5` |
| Tracked allocations | current supervisor maximum 8; live process capacity is stricter |
| M05 product slot | exactly 0 or 1 reservation/Active match; reaching Active removes queued overflow, terminalizes recovery-pending tickets, pauses Join admission, and does not free the slot before M06/server restart |
| Build transfer | at most 256 encoded bytes per participant |
| Pending capability activation | 30 seconds infrastructure maximum |
| Lobby product deadline | 30 process-local monotonic seconds from reservation commit |
| Lobby allocation/grant sub-deadline | 10 process-local monotonic seconds from reservation commit |
| Supervisor allocation/Ready timer | 10 process-local monotonic seconds from accepted request |
| Supervisor Ready-to-commit timer | 20 process-local monotonic seconds from validated Ready |
| Worker check-in timer | at most 20 process-local monotonic seconds from Ready |
| Client loading timer | presentation-only remaining duration, clamped downward on newer phases |
| Capacity/infrastructure cooldown | clamp retry hint to 1–5 seconds; third consecutive infrastructure failure pauses pool 5 seconds |
| Infrastructure health counter | one saturating value per pool, capped at 3 and reset by grant success or after the 5-second probe pause |
| Detached lease | bounded by the lobby's 30-second loading deadline; at most one per participant |
| Terminal reconciliation | 15 seconds after the latest dissolution/Active-capacity terminal transaction; at most one recovery-pending ticket/tombstone per participant; expiry removes an associated formation-ineligible recovered ticket and closes its still-live owning lobby session |
| Recovered formation barrier | requeued/rebound tickets are publicly counted but formation-ineligible until exact `Requeued` terminal acknowledgement; no same-update reformation |
| Retained matchmaking messages | one current phase per live/detached participant plus one bounded terminal tombstone; exact authenticated terminal acknowledgement releases delivered state and makes a recovered queued ticket eligible |
| Match-loading messages | at most one application-unacknowledged reliable worker outcome and one latest replaceable status snapshot per manifest participant; client and server directions are distinct |
| Recovery-pending Join delay | 1..=15,000 presentation milliseconds derived from the local terminal expiry; no automatic Join retry |
| Active-capacity removal | at most the 28 non-reserved tickets left by a 4-player formation across queued/recovery-pending ownership; one retained ticket-only terminal phase per live queued session, or one updated reservation-correlated tombstone per detached recovery-pending identity, until acknowledgement/loss/expiry |
| Player failure budget | none for capacity/infrastructure/other-player failure; attributable culprit removed once |

## Diagnostics and evidence

All diagnostics are bounded aggregates or redacted stable correlations. Record:

- formation attempts/successes by game type and topology;
- FIFO age at reservation, overflow left queued, map rotation selection, and cancellation race
  disposition;
- current/high-water reservations, pending allocations, loading workers, and requeued tickets;
- allocation accepted/rejected reason, spawn-to-Ready and Ready-to-grant latency;
- Joined-ack eligibility, Reservation Started, offer/ack/all-ack barrier/Begin, active reconcile,
  terminal rebind/ack, recovered-eligibility release/expiry, and check-in counts and phase latencies;
- loading activation, timeout, missing participant, disconnect, route expiry, worker failure, and
  dissolution reason;
- automatic requeue count, infrastructure cooldown/unavailable state, attributable removals, detached
  lease rebind/expiry, Active-capacity ticket removals/rejections, routes revoked, and workers stopped;
- manifest/build/topology validation failures without recipe, identity, capability, address, or
  player-name bytes;
- each authority's local timeout/elapsed observations without raw clock values, plus time from exact
  roster availability to authoritative Countdown.

Process evidence correlates reservation -> allocation -> worker -> activation with redacted IDs in
structured test records, not ordinary logs. Existing routing packet/queue/process metrics remain the
transport source of truth.

## Implementation plan

Implementation begins only after M04 completes, this document is reconciled against its actual
public/private seams, and the user validates the resulting specification.

### Slice 1 — Reconcile M04 and establish pure reservation rules

- [ ] Record M04's final ticket, pool, command-memory, schedule, client-flow, snapshot, telemetry,
  and teardown seams; update this specification where they differ.
- [ ] Add bounded reservation/loading shared identities, `ReservationStarted`, capability-free offer,
  grant-bearing Begin, terminal acknowledgement, and targeted outcomes.
- [ ] Extend `QueueState` with disjoint queued/reserved/recovery-pending ownership, Joined-ack
  eligibility, Begin-issued live binding versus observed-unlink detach, Netcode-indexed detached
  leases, control-only active-reservation reconciliation, original-key terminal rebind, terminal
  tombstones/acknowledgements, acknowledgement-gated recovered eligibility, Active-capacity
  terminalization of recovery-pending tickets, temporary Active-product occupancy, and revised index
  validation without weakening M04 idempotency.
- [ ] Implement/test exact oldest-roster extraction, deterministic alternating teams, catalog-order
  offered-map rotation, overflow preservation, one-product-reservation catalog-order selection,
  same-frame cancel/disconnect precedence, original-key requeue without same-update reformation,
  terminal-ack eligibility release, and fair failure attribution/cooldown as pure state transactions.
- [ ] Expand advertisement/catalog/runtime validation from exact 2v2 to exact 2v2 or 3v3 and prove
  every advertised map satisfies resolved capacity.
- [ ] Add one checked-in advertised Hot Zone 3v3 game type used by unit and process evidence.

### Slice 2 — Evolve allocation/manifest contracts before UI

- [ ] Define/measure `MatchBuildSnapshotV1`; prove preset and custom recipes round-trip below 256
  bytes and re-resolve to the M04 accepted identity/loadout.
- [ ] Evolve allocation request, match manifest, control versions, codecs, bounds, fixtures, and
  unknown-version behavior in `brawler-routing`.
- [ ] Move game/map/topology selection from supervisor mode policy to validated lobby request;
  retain only infrastructure policy in the supervisor.
- [ ] Generalize exact-two allocation validation to exact manifest topology and preserve duplicate,
  conflict, capacity, Ready, and rollback safety.
- [ ] Add supervisor-serialized cancellation versus activation prepare/commit, terminal loading
  progress, the explicit `Loading -> CommitGranted -> Active|Dissolved` state machine, correlation,
  redaction, bounded queues, and route/worker cleanup.
- [ ] Define process-local lobby/supervisor/worker timers and duration-budget fields; reject any
  serialized monotonic instant or foreign clock comparison.
- [ ] Update match-worker manifest validation and admission for 2v2/3v3 plus custom builds before
  any manifest can report Ready.

### Slice 3 — Connect production formation to real worker allocation

- [ ] Add lobby formation/allocation plugins and ordered schedule sets on the M04 queue.
- [ ] Publish retained targeted `ReservationStarted` phases before the aggregate snapshot that
  removes the roster; keep all other eligible pools queued while the reservation is pre-Active, then
  atomically remove queued overflow and terminalize recovery-pending tickets with
  `ServerMatchCapacityOccupied`, publish occupied availability, and pause admission at Active.
- [ ] Stage one stable request per reservation through the existing sole worker-control owner.
- [ ] Validate complete grant sets and map each grant to the exact current reservation participant.
- [ ] Implement allocation rejection, timeout, cancellation, disconnect, worker failure, cooldown,
  detached-session reconciliation, and original-order live/rebound reinsertion as one terminal
  transaction.
- [ ] Migrate the routed process smoke from automatic exact-two session allocation to explicit
  product queue formation; retain a narrow compatibility fixture only until equivalent evidence
  passes.

### Slice 4 — Deliver product Match Loading, minimal Match, and fresh connection handoff

- [ ] Add `MatchLoading` and minimal `Match` to the existing client flow model without creating a
  second transition arbiter; only replicated Countdown may commit Match.
- [ ] Add the narrowly scoped `Confirmation(CancelMatchStartConfirmation)` overlay with exact
  reservation/generation context, non-destructive default focus, restoration, and teardown.
- [ ] Implement Joined Ack eligibility -> Reservation Started -> capability-free Reservation Offer ->
  complete-roster Ack barrier -> grant-bearing Begin ordering with one sequenced server envelope,
  exact generation/identity checks, bounded retained phases, one loading context, and secret-free
  diagnostics.
- [ ] Reuse the delivered intentional disconnect/unlink/deferred-despawn/fresh-entity path in the
  product shell.
- [ ] Present mode/map/topology/own build and honest phase/error text with controller, keyboard,
  pointer, focus, and 960x540 scrolling behavior.
- [ ] Add confirmed Cancel Match Start through the flow arbiter with non-destructive default focus,
  preserve the intent across handoff gaps, remove it after authoritative commit, and implement the
  returning-to-queue and terminal destination matrix.
- [ ] Recover a pre-Active start failure through a fresh lobby connection and authoritative
  reservation reconciliation; distinguish active control-only reconciliation from terminal ticket
  rebind, require explicit reconcile after authentication, reject ordinary Join while recovery is
  pending, and never resume a failed match session.

### Slice 5 — Add exact worker check-in and authoritative countdown gate

- [ ] Install manifest-scoped `MatchLoadingState`, exact `ExpectedManifestRoster`, process-local
  check-in timer, and
  participant check-in state before accepting clients; derive routed 2v2/3v3 maximums without
  changing the direct-UDP default.
- [ ] Compose the client readiness predicate from accepted Hello, map, terrain, assets, controlled
  fighter/build, and generation facts.
- [ ] Add one ordered match-loading client envelope for idempotent Ready/Ack and Cancel; reject stale,
  duplicate-conflicting, non-manifest, wrong-map, or disconnected requests, make same-worker-Update
  cancel win before new check-in/preparation, and suppress client Ready when cancel commits locally.
- [ ] Add the separate ordered-reliable worker-to-client outcome envelope and replaceable sequenced
  presentation-status message with exact direction registration, correlation, acknowledgement,
  retention, supersession, freshness, and authority boundaries.
- [ ] Gate product routed participant readiness on the exact full manifest; accept asset `Ready` or
  valid declared `Degraded` fallback state and preserve the direct-UDP ready baseline.
- [ ] Prepare once, wait for supervisor `CommitActivation`, commit once in fixed tick, and prove the
  existing authoritative lifecycle produces exactly one Countdown and no client-side substitute.
- [ ] Emit ActivationCommitted/Activated/Dissolved lifecycle facts, terminate a Countdown departure,
  insert product departure detection after roster refresh and before common lifecycle advancement,
  suppress the legacy Countdown-to-Waiting/readiness-clear mutation, and stop/revoke incomplete
  workers under deadlines.
- [ ] Suppress product-routed `SetReady` and `ReadyForRestart`; retain existing replicated Completed
  facts in minimal Match presentation with Disconnect only and no legacy restart.

### Slice 6 — Verify and hand off

- [ ] Add focused pure, codec, ECS/schedule, in-memory routing, real process/UDP/IPC, UI/controller,
  impairment, failure, and capacity evidence below.
- [ ] Run canonical role checks/tests/clippy/format plus routed product and direct-UDP baselines.
- [ ] Measure maximum manifest/control/application message sizes and handoff latency phase bounds.
- [ ] Update canonical `just`/README commands only for validated product formation/loading paths.
- [ ] Set M05 to `User playtest` with a four- or six-client launch path, controls, expected loading
  phases, the explicit one-product-match-per-server-process/restart limitation, known M06
  limitations, and requested observations.

## Verification contract

### Pure queue/formation/build tests

- fewer than exact topology does not reserve; unacknowledged Joined tickets remain ineligible; exact
  N acknowledged memberships remove the N oldest eligible tickets; N+k leaves all other tickets in
  stable order;
- simultaneous eligible game types select only the first catalog-ordered roster; every other ticket
  remains queued in stable order while that reservation is pre-Active, then Active removes those
  tickets once with `ServerMatchCapacityOccupied` and rejects later Join without mutation;
- alternating assignment gives exact balanced teams for 2v2 and 3v3 and is stable across runs;
- map rotation follows operator order, binds once per reservation, wraps, does not change on request
  retry/capacity/failure before Ready, and advances exactly once when a complete grant set becomes
  offerable;
- same-frame valid Cancel/disconnect wins before formation; post-reservation Cancel dissolves once;
- live retained tickets and asynchronously rebound recovery-pending tickets reinsert by original FIFO
  key without new ticket/admission identity or reconnect-order reversal; both remain
  formation-ineligible until their exact `Requeued` terminal phase is acknowledged, cannot reform in
  the dissolution/rebind update, become eligible on exact acknowledgement, and are removed with any
  still-live owning session closed rather than silently enabled on terminal expiry;
- queued/reserved/recovery-pending sets remain disjoint and exhaustive; intentional handoff detaches
  only after Begin-issued unlink, lost Begin leaves a live binding until timeout, ordinary loss
  removes, one explicit reconcile on a matching fresh session rebinds, and
  auth-only/wrong/duplicate/expired rebind attempts fail;
- active-reservation reconciliation creates only one same-identity control binding, can forward a
  retained cancel, and cannot requeue, mint another grant, or resume the failed match;
- fresh authentication alone cannot rebind or cancel; only explicit same-identity reconcile mutates
  recovery, ordinary Join receives bounded `RecoveryPending`, and local Disconnect cannot cause an
  automatic rebind on a later connection;
- rebind creates fresh session command memory with the retained active ticket; old-session request
  IDs and pending commands cannot replay against it;
- explicit culprit removal, innocent-ticket original-order recovery, infrastructure-wide recovery
  without player penalty, and capacity/infrastructure cooldown/unavailable policy match the decision
  table;
- when a later reservation reaches Active, every older recovery-pending ticket is removed and its
  one tombstone becomes capacity-occupied; matching reconciliation returns removed without rebind,
  while wrong/stale/expired reconciliation cannot mutate state;
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
- `ReservationStarted` and `ReservationOffer` contain no capability bytes; only the recipient's
  Begin carries its grant, and no client can construct an accepted pre-Begin match connection;
- cancel/reject/progress/Activated/Dissolved correlation ignores stale or wrong worker generations;
- `ActivationPrepared`, `CancelActivation`, and `CommitActivation` are serialized once; both arrival
  orders produce one terminal disposition and a late cancel returns stable TooLate;
- lobby server phases preserve Joined Ack -> Reservation Started -> capability-free offer -> all-ack
  barrier -> grant-bearing Begin -> terminal ordering in one envelope, retain no history beyond
  declared bounds, and never overtake an unacknowledged Joined outcome;
- exact terminal acknowledgement releases only the delivered terminal sequence; stale/wrong
  acknowledgement cannot release a current tombstone or restore recovered-ticket eligibility;
- a recovered-ticket terminal acknowledgement atomically releases its tombstone and enables only
  that queued ticket; a valid acknowledgement collected at the expiry boundary wins before expiry,
  while expiry removes the ineligible ticket, closes its live owning session, and duplicate
  acknowledgement is idempotent;
- ticket-only Active-capacity termination cannot carry a reservation, releases only on its exact
  correlation/sequence acknowledgement, and never enters queue-command request memory;
- an unacknowledged Joined outcome is sent before its same-ticket Active-capacity termination; the
  late Joined acknowledgement clears only command memory and cannot recreate membership;
- duration fields round-trip within bounds while monotonic instants and wall-clock deadlines have no
  wire representation;
- match-loading client, reliable server-outcome, and replaceable server-status messages register only
  in their specified directions; maximum shapes round-trip, wrong direction/correlation/sequence and
  invalid outcome/status variants fail, and status contains no authority-changing variant;
- Debug/log output redacts build bytes, capability, nonce, digest, network identity, and source.

### ECS and schedule tests

- queue messages, Joined acknowledgements, and disconnect reconciliation precede
  cancellation/formation; a newly retained `ReservationStarted` phase is staged before the final
  snapshot that observes the committed result in the same update;
- at most one formation per game type per update and no pool/reservation owns one ticket twice;
- full control inbox/outbox retries a stable request without allocating another identity;
- worker app cannot report Ready until topology/map/build/registry/content validation passes;
- one participant's offer acknowledgement cannot emit Begin; only the complete-roster barrier emits
  recipient-specific grant-bearing Begin, and reservation/offer observation cannot spawn a match
  entity before Begin and the old lobby entity's deferred unlink/despawn;
- Begin issuance keeps the live membership binding; only the matching observed unlink installs a
  detached lease, while Begin loss plus a still-live lobby reaches one bounded timeout disposition;
- loading readiness remains false for each missing prerequisite independently and resets on session
  generation change;
- each worker participant retains at most one unacknowledged reliable loading outcome and one latest
  replaceable status; exact client acknowledgement releases only the current outcome, status
  supersession cannot change check-in/cancel/flow authority, and stale status cannot regress display;
- check-in is idempotent and manifest-scoped; a nonparticipant cannot mark another participant;
- ordered match-loading batches apply valid cancellation before any new Ready/check-in; cancel plus
  the last check-in in either envelope order cannot prepare, while preparation already emitted in a
  prior control turn remains subject to supervisor arbitration;
- four/six exact connected check-ins prepare once but remain Waiting; only supervisor commit sets the
  product roster ready once, while a partial roster never prepares;
- preparation alone cannot set readiness or create Countdown; supervisor commit is visible to exact
  roster refresh before lifecycle evaluation and produces one Countdown with the configured future
  start tick;
- supervisor `CommitGranted` rejects player cancellation as TooLate but accepts a correlated
  Countdown departure as Dissolved; only worker Active observation reaches terminal Active;
- a Countdown departure emits one dissolution instead of returning to an unrecoverable Waiting
  worker: product detection runs after roster refresh and before common lifecycle advancement, sets
  the terminal marker once, suppresses Countdown-to-Waiting/readiness-clear, and exposes no replicated
  Waiting phase; direct UDP retains its existing reset behavior. Active releases M05 reservation
  recovery state exactly once, removes its tickets, installs one non-gameplay Active occupancy,
  removes queued overflow and terminalizes older recovery-pending tickets with the bounded capacity
  outcome, publishes zero queued counts, and blocks later admission/formation until M06;
- replicated Countdown commits `ClientFlow::Match` exactly once; product-routed clients never send
  `SetReady` or `ReadyForRestart`, and Completed retains minimal Match presentation with Disconnect;
- headless product automation bypasses presentation only: it consumes the same reservation/handoff
  envelopes, sends the same automatic Ready envelope, never sends legacy ready/restart commands, and
  reaches its configured Active/terminal exit deterministically;
- no product presentation system mutates match/queue authority; server feature graph remains
  client-render/audio/input free.

### In-memory routed/network tests

- connect lobby -> catalog -> Join/Joined-ack exact roster -> Reservation Started/Match Loading ->
  capability-free offers -> complete-roster ack barrier -> grant-bearing Begin -> fresh match
  connection -> map/terrain convergence -> check-in -> supervisor activation commit -> Countdown for
  Wipeout 2v2 and the checked-in Hot Zone 3v3 fixture;
- overflow remains queued and its aggregate count is correct while the first roster loads;
- Cancel before formation, Cancel after reservation through lobby/match/reconnect paths, both
  cancel-versus-prepare orderings, ordinary versus intentional lobby disconnect, offer loss, one
  missing offer acknowledgement, Begin loss, duplicate offer/ack/begin, stale generation, route
  expiry, and one missing check-in have exact ticket/worker/route dispositions;
- a premature client has no capability before Begin; active-reservation reconciliation forwards a
  retained cancel without requeue/resume, while terminal reconciliation alone restores membership;
- failure after Begin reauthenticates a fresh lobby session, rebinds only the matching Netcode
  identity, restores retained Queue membership as formation-ineligible until exact terminal
  acknowledgement, and never resumes the failed match connection or reforms before recovery is
  visible;
- transport loss/retry within one allocation preserves one semantic request and one worker;
  sequenced queue snapshots remain independent of lossless targeted reservation outcomes;
- a custom M04 build reaches the worker and controls the spawned loadout without fallback to preset;
- wrong capability, peer, Netcode ID, allocation, match, map, build, or check-in identity fails
  without admitting a fighter or unlocking countdown;
- capacity rejection requeues with cooldown and no busy-loop; a later capacity opening forms the
  retained oldest roster only after the recovered terminal phases are acknowledged;
- simultaneous eligible pools form only the catalog-first reservation in M05; the other pool remains
  queued and receives no allocation, route, progress, or loading phase before Active, then receives
  one acknowledged ticket-only `ServerMatchCapacityOccupied` removal and zero-count snapshot rather
  than waiting indefinitely; fresh occupied availability disables Build & Join and a racing/stale
  Join receives the request-correlated rejection;
- a recovery-pending ticket left by an earlier dissolved reservation cannot survive the later Active
  capacity transaction: same-identity reconciliation receives removed/capacity-occupied and no route,
  grant, membership, or new formation is created.

### Real process/UDP/IPC tests

- canonical routed product smoke forms and loads an exact 2v2 through the public endpoint using the
  real lobby worker, supervisor, match child, Unix IPC, immediate targeted reservation phase, and
  fresh client sockets; headless clients use the product protocol and exit deterministically after
  the configured Active observation without invoking legacy restart;
- a six-client Hot Zone 3v3 process scenario reaches exactly one authoritative Countdown with all
  manifest teams/builds/map identities correct, then observes Active and the temporary occupied
  product slot, capacity-removes any overflow roster, and rejects new Join without forming another;
- cold worker spawn/Ready completes inside the 10-second allocation bound and full formation reaches
  activation commit inside the lobby's 30-second product deadline under the accepted development
  environment; evidence records each authority's local elapsed duration rather than comparing raw
  clock values;
- kill before Ready, kill after grants, malformed manifest, stalled control stream, route
  activation expiry, client loss during each loading phase, and supervisor capacity refusal dissolve
  once, revoke routes, reap incomplete child processes, and apply exact requeue policy;
- departure during authoritative Countdown dissolves once, reaps the worker, and restores every
  retained participant through fresh-lobby reconciliation without mutating or replicating a false
  Waiting state; the named direct-UDP reset behavior remains unchanged;
- a second queued roster cannot receive the first reservation's routes, packets, grants, progress,
  or replicated world state;
- the named direct-UDP smoke still reaches its current explicit ready/countdown behavior;
- routed packet MTU, control queue, manifest, and capability bounds remain unchanged or are updated
  with measured evidence.

### Visual/controller checks

- controller, keyboard/mouse, and pointer observe Match Loading immediately from Reservation Started,
  including reserving/starting-server phases, without an unexplained Queue-count drop, focus loss, or
  duplicate activation;
- Cancel Match Start is reachable through controller, keyboard, and pointer before commit, opens one
  confirmation overlay through `RequestCancelMatchStart`, defaults focus to **Keep Loading**, sends
  only through `ConfirmCancelMatchStart`, survives phase-text changes, never triggers from implicit
  socket teardown, and disappears after TooLate or authoritative commit;
- returning-to-queue presentation remains bounded and honest during fresh lobby authentication,
  exposes Disconnect, reaches Queue on retained rebind, and reaches Game Select with ordinary
  **Build & Join** after removed/expired disposition;
- recovered Queue and its notice are observed before that ticket can enter another Match Loading
  flow; delayed terminal acknowledgement cannot let a newer reservation overwrite recovery
  presentation;
- quick same-server authentication after choosing returning-state Disconnect does not silently
  requeue; explicit reconcile restores Queue, while ordinary Join reports bounded recovery pending;
- Match Loading remains legible at 960x540, 1280x720, and 1920x1080 with minimum/default/maximum M02
  UI scale and long valid server/game/map names;
- phases and aggregate roster counts do not imply percentages or queue estimates; every terminal
  outcome gives the specified plain notice and Queue/Game Select action;
- `Ready` and valid declared `Degraded` asset states may check in, while `Loading` or an undeclared
  required-asset failure cannot;
- only the player's own build is shown; secret/internal IDs and opponent builds never render;
- map/terrain sync cannot accept gameplay input before check-in and Countdown;
- the authoritative Countdown appears once after the last participant checks in and commits minimal
  Match flow once;
- Completed product presentation shows the replicated result and Disconnect only, with no ready-up,
  restart prompt, Queue Again, or hidden second match; queued overflow receives the capacity notice,
  and fresh occupied availability disables Build & Join for every advertised game type.

### Performance and bounds

- record p50/p95/max exact-roster-to-Ready, Ready-to-offer, offer-to-match-connect,
  match-connect-to-check-in, and exact-roster-to-Countdown latency;
- prove 32 tickets across queued/reserved/recovery-pending ownership, eight pool rows, M05's one live
  product reservation/allocation/Active worker, the unchanged supervisor ceiling of four workers/24
  match routes, one current matchmaking phase per participant, control-only reconciliation bindings,
  detached leases, and terminal tombstones stay within declared memory/queue/time bounds;
- run representative 3v3 fixed-tick/performance gates for both modes and terrain admission without
  regressing existing thresholds;
- repeated pre-Active start failure cannot grow request history, reservation memory, child handles,
  routes, capabilities, control frames, or client entities.

## Exit criteria

- [ ] M04 is complete and its actual delivered seams are reconciled here.
- [ ] The user validates the reconciled M05 specification before implementation begins.
- [ ] Exact acknowledged-membership 2v2 and 3v3 formation, deterministic teams/offered-map rotation,
  cancellation race, overflow, original-order recovery, terminal-acknowledgement eligibility, and
  fair bounded failure policies match pure and network evidence; a recovered ticket cannot reform in
  its dissolution/rebind update or before recovery is observed.
- [ ] Formation stages `ReservationStarted` before the removing aggregate snapshot, immediately enters
  Match Loading, leaves other pools ordered while pre-Active, and on Active removes queued overflow
  and terminalizes older recovery-pending tickets with an honest bounded capacity outcome before
  pausing admission rather than stranding or later restoring a ticket.
- [ ] Preset and custom M04 accepted builds cross the manifest boundary and revalidate before Ready.
- [ ] The supervisor remains queue/gameplay-opaque and the lobby remains simulation-free.
- [ ] Every retained participant establishes a distinct manifest-authenticated worker session
  through the same public endpoint and checks in against the correct map/terrain/build generation.
- [ ] Reservation Started and Offer disclose no capability; recipient-specific Begin is the first
  phase carrying a grant, and no pre-Begin connection can reach worker admission.
- [ ] Begin issuance retains a live binding until observed unlink creates one bounded detached lease;
  ordinary loss removes the culprit; recovery-pending state is not counted or formed; active
  reconciliation is control-only; terminal fresh-session reconciliation rebinds only the same
  authoritative Netcode identity, restores its original FIFO key as formation-ineligible, and exact
  terminal acknowledgement is the only recovery path that makes it eligible.
- [ ] Partial rosters never enter Countdown; a complete roster produces exactly one authoritative
  Countdown only after supervisor commit, commits minimal Match flow exactly once, and exposes no
  product ready-up, legacy restart command, or client countdown.
- [ ] Cancellation and activation preparation are serialized once, late cancellation returns
  TooLate after nonterminal CommitGranted, and a Countdown departure can still transition to
  Dissolved and reconcile; product departure detection runs after roster refresh and before common
  lifecycle advancement, so it cannot mutate or replicate the legacy Waiting reset.
- [ ] Match-side ordered envelopes and schedule precedence make same-Update cancellation win over new
  check-in/preparation; only an earlier supervisor-serialized preparation may make cancel TooLate.
- [ ] Match-loading client intents, reliable worker authority outcomes, and replaceable worker status
  have distinct registered directions, bounded correlation/sequence/retention, exact outcome
  acknowledgement, and tests proving status cannot mutate authority or flow.
- [ ] Lobby, supervisor, worker, and client use only their own monotonic clocks; wire records carry
  bounded duration/remaining values, and the client timer never decides timeout authority.
- [ ] Every pre-Active cancel/disconnect/refusal/failure/timeout has one route, worker, ticket, UI,
  and diagnostic disposition with no leaks, invisible membership, or unfair infrastructure penalty.
- [ ] Match Loading exposes one confirmed Cancel Match Start path with non-destructive default focus,
  honest bounded phases/counts, a bounded returning-to-queue state, correct terminal destinations,
  and the existing declared degraded-asset policy.
- [ ] M05 adds the concrete Confirmation overlay and minimal Match state missing from M04; explicit
  reconcile—not authentication—restores a ticket, quick reconnect cannot silently requeue, and
  Completed shows only existing replicated result facts plus Disconnect under the documented
  one-product-match-per-server-process limitation.
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
  authenticated sessions, operator catalog, delivered M04 tickets/pools, wire messages, and 2v2
  validation.
- `src/server/{admission,worker,routed_worker}.rs` and `src/server/mod.rs` — worker bootstrap,
  manifest admission, sole IPC owner, match Hello, loadout install, and current explicit ready path.
- `src/client/{mod,session,flow,queue}.rs` — delivered sequential routed transition, one client
  entity, product flow arbiter, and delivered queue state.
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

The 2026-08-19 completeness/quality/UX review identified and this revision addresses: Joined-outcome
formation eligibility; ticket survival across intentional lobby teardown; single-authority
cancellation-versus-activation ordering; Countdown-departure recovery; roster-wide Begin; bounded
ordered matchmaking messages; degraded-asset compatibility; manifest-specific 3v3 installation;
and fair infrastructure retry policy. A second coherence pass additionally addresses immediate
reservation visibility; capability withholding until Begin; Begin-issued versus observed-unlink
ownership; process-local deadline authority; active-reservation cancel reconciliation; nonterminal
`CommitGranted`; terminal acknowledgement; the M05 one-product-slot boundary; confirmed cancellation;
and bounded returning-to-queue presentation. A subsequent implementation-readiness review adds the
missing minimal Match flow and concrete Confirmation overlay, deterministic same-worker-turn cancel
precedence, explicit reconcile-only recovery, and honest one-product-match post-Active behavior that
suppresses legacy restart and capacity-removes overflow instead of stranding it. M05 remains in
Specification review. The follow-up repair pass makes recovered-ticket formation eligibility depend
on exact terminal acknowledgement, terminalizes older recovery-pending tickets when Active occupies
the temporary product slot, orders product Countdown-departure detection before the legacy lifecycle
reset, and separates client intents, reliable worker outcomes, and replaceable worker status into
directionally explicit application contracts. By user direction, this pass does not add a new
measurement/impairment profile; the existing verification scope remains unchanged.

Before implementation, reconcile this document after M04 completion and confirm that the revised
decisions, plan, verification contract, and exit criteria still match its final delivered seams. A
user direction to implement that reconciled M05 specification is the approval event that moves
status to `Implementing`.
