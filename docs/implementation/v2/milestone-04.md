# V2 milestone 04 — Product build editor and authoritative queue admission

## Tracking

| Field | Value |
|---|---|
| Status | User playtest |
| Prepared | 2026-08-19; reconciled after M03 completion and user-approved review fixes |
| Objective | Let an authenticated lobby player compose a bounded build, submit it with one selected advertised game type, receive an authoritative immutable queue ticket, inspect honest aggregate pool state, and cancel without leaving the lobby |
| Entry dependency | Satisfied 2026-08-19: M03 is complete; its delivered lobby catalog, client flow, overlay, persistence-error, and session-loss seams are reconciled below |
| Scope authority | User validated the specification and directed implementation on 2026-08-19 |

M04 research began while M03 was implementing. M03 is now complete and the user validated this
specification by directing implementation on 2026-08-19.

## Player-visible outcome

Game Select shows the newest fresh population for every advertised game-type pool as
`N waiting · M players per match`, never as progress or an estimate. A snapshot older than three
seconds is no longer presented as current and becomes `Updating queue`. The player selects a game card
with the existing Confirm behavior, then activates one explicit **Build & Join** action to open
Build Editor. There the player chooses one of the four embedded presets or
edits the current six-field custom recipe, sees each option's cost, the used and remaining 12-point
budget, and an honest locally resolvable combat summary that explains the tradeoffs being changed,
then chooses **Join Queue**. The lobby validates the selected game and complete build in one
transaction. Acceptance closes the editor and enters Queue with the player's server-accepted build,
ticket identity, game type, and an honest population that is either current through the ticket's
required admission revision or visibly `Updating queue`.

An identical periodic refresh of the current snapshot renews its freshness even though it does not
replace the client's pool data. Only an older revision is ignored without renewing freshness.

A correctable rejection stays in Build Editor with the draft intact and the exact problem shown.
Cancel Queue is acknowledged by the lobby, returns to Game Select, and keeps the lobby connection
open. Join and Cancel show bounded pending states, and a secondary Disconnect action remains
reachable immediately if the player does not want to wait for the acknowledgement. Duplicate
activation, duplicate messages, stale revisions, queue overflow beyond a future formation, and
disconnect cleanup cannot create two
tickets or leave an invisible ticket behind.

M04 ends with players waiting in real bounded pools. It does not select a roster, reserve tickets,
assign teams, choose a map instance, ask the supervisor for capacity, start a worker, or issue route
grants. Those are M05 responsibilities.

## Reconciled research findings

### Existing reusable behavior

- `BuildCatalog`, `BrawlerBuildRecipe`, `BuildRevision`, `BuildPresetId`, and
  `resolve_build_recipe` already provide the embedded four-preset catalog, six custom fields,
  duplicate/incompatible-passive checks, 12-point budget, deterministic recipe fingerprint, and
  bounded resolved loadout.
- The current `BuildSelectionRequest` path validates a waiting match fighter and immediately
  mutates gameplay ECS state. Its editor is a debug text overlay. M04 can reuse the shared recipe
  and resolver, but lobby admission must not reuse that match-owned mutation system.
- M03's `LobbyPlugin` owns authenticated product sessions and immutable advertisements. Product
  sessions remain idle; `LobbyTransitionDriverPlugin` separately preserves automatic allocation
  only in the explicit M01 transition-smoke composition.
- M03 selection stores `(CatalogRevision, GameTypeId, configuration_revision)` locally without
  sending queue intent. Those three values are the M04 game-selection input.
- `MAX_AUTHENTICATED_LOBBY_SESSIONS` is 32, the M03 catalog bound is eight game types, the shared
  build candidate bound is 128 bytes, and routing currently bounds match participants at eight.
  One ticket per authenticated session therefore gives an immediate process-wide ticket bound.
- M02 settings and M03 connection persistence establish platform-local paths, bounded versioned
  RON, same-directory atomic replacement, safe defaults, and recoverable save failure. M04 adds one
  small build file and extends the closed load-failure aggregate from two booleans to three named
  sources; it does not add a generic persistence framework.
- M03's client composition is concrete: `ClientFlow` currently ends at `GameSelect`,
  `ClientOverlay` has one mutually exclusive overlay, and the chained `ClientFlowSet` order is
  `BeginFlowFrame -> ObserveSession -> CollectFlowInput -> ResolveFlowAction -> TeardownSession ->
  CommitFlow -> PresentFlow`, with an explicit deferred boundary between teardown and commit.
- M03 currently removes lobby membership through both `lobby_cleanup_disconnected` and the
  `On<Remove, LobbyClient>` observer. M04 must route both through one queue-aware teardown operation;
  neither path may erase the session identity before its ticket is removed.
- Lightyear receives packets and typed messages in `PreUpdate`; application systems may observe
  them in ordered `Update` sets. `SessionChannel` is already bidirectional ordered-reliable. This is
  suitable for small lobby commands/outcomes, while application request/ticket IDs remain necessary
  for semantic idempotency and stale-response rejection. Pinned Lightyear retains every
  unacknowledged reliable send and exposes no application capacity setting, so replaceable full
  aggregate snapshots require different delivery semantics.

### Consequences

1. Queue authority belongs to the long-lived lobby worker, not the supervisor or match worker.
2. Admission must resolve the build without spawning or mutating a fighter. The ticket keeps the
   complete server-resolved loadout internally; the client receives only its bounded accepted
   public summary.
3. The queue cannot key membership by a UI selection, connection entity, display name, or process-
   local `Entity`. It keys by authenticated lobby session and stable typed ticket identity.
4. A full pool snapshot is preferable to deltas: at most eight small rows are needed, gaps can be
   replaced directly, and the client never needs a resynchronization protocol.
5. FIFO across different network connections has no meaningful packet-level total order. The
   lobby assigns one monotonic admission order. Requests observed in the same application update
   receive a documented stable tie-break instead of depending on Bevy query iteration.
6. A local preview is advisory. The server repeats every catalog, size, combination, budget, and
   resolution check atomically before creating a ticket.
7. Full snapshots eliminate resynchronization deltas but do not by themselves bound an ordered-
   reliable sender. Outcomes remain lossless; snapshots use sequenced-unreliable delivery with local
   retry disabled and repeat the newest complete state once per second. Consecutive loss has no
   delivery deadline, so clients age a snapshot out after three seconds instead of presenting stale
   counts as current. A byte-equivalent refresh at the current revision renews receipt freshness;
   otherwise healthy unchanged pools would age out despite receiving every refresh.
8. Ordered-reliable transport retains an outcome until transport acknowledgement and exposes no
   application queue-cap setting. M04 therefore allows only one application-unacknowledged queue
   outcome per session, requires a small client acknowledgement after consuming it, suppresses a
   second wire copy for an unacknowledged identical retry, and rate-limits accepted commands.
   Identical application retries remain token-neutral but are admitted no more often than the
   product's ten-second Retry deadline; repeated early duplicates cross a separate bounded abuse
   threshold. The first over-rate new command receives one bounded fail-soft notice; continued new
   commands inside that notice window are the protocol-abuse boundary.

## Decisions for specification review

These decisions close the plan-review findings. They remain specification, not implementation
authority, until the user validates the milestone.

| Concern | M04 decision |
|---|---|
| Build-editor entry | Confirm retains M03's card-selection meaning; a separate `Build & Join` action for the selected card opens `BuildEditor`; Cancel restores focus to that action |
| Draft source | Load the last server-accepted local selection when valid for the current embedded build revision; otherwise use preset 1 and report malformed/stale local data recoverably |
| Persistence | Separate `build.ron`, schema version 1, storing build revision plus preset identity or complete custom recipe; save only after authoritative admission acceptance |
| Admission | One lobby transaction validates the authenticated session, selected catalog/game revisions, one-ticket rule, capacity, build revision, and complete build before mutation |
| Membership | At most one active ticket per lobby session across all game types; changing game or build requires acknowledged cancellation and a new admission |
| Request identity | Nonzero monotonic `QueueRequestId(u64)` scoped to one authenticated lobby session |
| Ticket identity | Server-generated unpredictable nonzero `QueueTicketId(u128)`, unique for the lobby-worker generation and never used as authentication |
| FIFO | Server monotonic `admission_order`; requests first observed in one update are tied by stable authenticated `PlayerId`, then request ID |
| Pool state | Complete revisioned aggregate snapshot for all advertised pools: stable game identity, queued count, and exact formation size; no names, builds, or wait estimate |
| Capacity | At most 32 tickets process-wide and therefore at most 32 in one pool, derived from one ticket per authenticated session; M04 has no smaller arbitrary per-pool limit or unreachable `Queue Full` state |
| Delivery | One client-to-server `QueueClientMessage` envelope carries commands and outcome acknowledgements in their actual ordered-reliable `SessionChannel` order; outcomes use the same channel server-to-client. Each session may have at most one application-unacknowledged queue outcome and at most 512 encoded outcome bytes retained for it. Complete snapshots use one server-to-client `QueueSnapshotChannel` with sequenced-unreliable delivery, local unsent retry disabled, immediate publication after visible mutation, and a one-second refresh; a byte-equivalent current-revision refresh renews client freshness without replacing pool data; no polling or HTTP side path |
| Command abuse bound | Per authenticated session, a token bucket admits a burst of four new semantic queue commands and refills one token per second. An identical same-ID retry while its outcome remains application-unacknowledged is idempotent recovery: it consumes no semantic-command token, causes no second wire copy, and leaves the retained outcome unchanged. It is accepted at most once per ten-second pending-command retry window. Earlier identical duplicates are dropped without an outcome and counted; the fourth early duplicate in one window is protocol abuse and disconnects. The first new semantic command beyond the semantic token bucket, when no outcome is pending, receives one bounded `RateLimited` outcome with a retry delay and starts one notice window; the client retains its context, disables **Try Again** until that delay expires, then uses a new request ID. Any further new command during that notice window, or any different command while an outcome remains application-unacknowledged, is protocol abuse: emit no additional reliable outcome, disconnect, and use normal queue-aware teardown |
| Authentication eligibility | Capture the stable IDs of sessions authenticated at the start of the lobby application update before processing new Hellos. Only that captured set may issue queue envelopes in the update. Queue input before authentication or in the same update as the accepting Hello is a protocol failure and cannot gain membership; the product client additionally waits until it has observed Welcome before sending queue intent |
| Disconnect | Both M03 disconnect paths call one queue-aware teardown operation that removes the ticket before session identity; a same-frame disconnect prevents admission |
| Cancel | Cancel carries request and ticket identity, is idempotent, and returns to Game Select only after an authoritative acknowledgement |
| Pending command recovery | Join and Cancel expose secondary `Disconnect` immediately and wait ten seconds before replacing the primary pending presentation with `Retry`; Retry uses the identical request ID and frozen command, and the server does not enqueue a second reliable copy while that outcome is unacknowledged. Late exact outcomes still win over the timeout overlay while the session generation remains active |
| Formation boundary | M04 supplies ticket identity, immutable data, FIFO ordering, cancellation semantics, and bounded pool operations only; M05 atomically adds reservation state and cancellation-versus-reservation race handling |
| Protocol evolution | Extend the one current global application schema and increment the global compatibility version once for the release containing M04; no message-level versions or fallback decoder |

## Scope

### Included

- product Build Editor overlay using the existing bounded build catalog and resolver;
- preset selection plus all currently supported custom pulse, ultimate, and two-passive fields;
- visible point budget, invalid-combination feedback, and a concise resolved health, movement,
  weapon, ultimate, and passive summary, including option costs and enough exact/product-described
  behavior to explain each available tradeoff without inventing compound damage numbers;
- schema-versioned local last-used build, bounded load, atomic replacement, safe fallback, Retry
  Save, and Continue Without Saving;
- one authoritative queue-admission command carrying selected catalog/game revisions and the full
  bounded build candidate;
- immutable server-owned tickets, FIFO admission order, one-ticket-per-session enforcement, and
  bounded pools;
- exact acceptance/rejection/cancellation outcomes, duplicate and stale handling, ticket cleanup
  on disconnect, one application-unacknowledged outcome per session, bounded semantic-command and
  identical-retry rates, pre-authentication rejection, and no hidden auto-requeue;
- bounded privacy-safe queue diagnostics for active/high-water tickets, mutations, rejection
  classes, pending outcomes, rate/protocol abuse, cleanup, snapshot publication intent, and client
  freshness aging/restoration;
- revisioned full aggregate pool snapshots for all advertised game types, bounded sequenced
  delivery, one-second refresh while the lobby session is active, and three-second client freshness
  aging;
- Game Select population presentation and a Queue flow with selected game, accepted build,
  `N waiting · M players per match`, Cancel, and Disconnect;
- windowed recovery plus deterministic headless admission/cancellation evidence;
- compatibility preservation for the explicit M01 transition smoke and direct-match build
  selection until their owning migration removes those paths.

### Deferred

- roster formation, ticket reservation, cancellation-versus-reservation races, team assignment, map
  rotation/selection, host admission, worker allocation, route capabilities, match connection,
  synchronization, and check-in (M05);
- replacing the M01 allocation/control/manifest build identity with the complete ticket snapshot
  (M05, when the actual transfer is exercised end to end);
- Queue Again, results, return-to-lobby lifecycle, worker failure, and concurrent match completion
  (M06);
- named saved builds, multiple local build slots, account arsenals, entitlements, acquisition,
  currencies, cloud saves, sharing, import/export, and build codes;
- arbitrary weapon-graph editing, variable slot counts, future content migrations, runtime catalog
  hot reload, and compatibility decoders;
- queue-wide player identities/builds, wait-time estimates, skill/rank, parties, priorities,
  backfill, join-in-progress, bots in PvP, and cross-server matchmaking;
- a general widget/reducer/data-binding framework or a second gameplay build-mutation path.

## Architecture and ownership

M03 delivered large but cohesive shared lobby, client-flow, and server-lobby modules. Queue state,
build editing, persistence, and transition-smoke allocation now demonstrate distinct owners, so M04
uses focused submodules while preserving the existing public paths:

```text
src/
  lobby/
    mod.rs                     M03 names/advertisements plus intentional public re-exports
    queue.rs                   bounded shared queue IDs, commands, outcomes, snapshots
  builds/
    model.rs                   build candidate/accepted-summary shapes shared by lobby and match
    definitions.rs             unchanged authored catalog and pure resolution authority
    server.rs                  existing match-waiting mutation path only
  client/
    flow.rs                    M03 ordered arbitration and the small cross-flow intent/commit seam
    build_editor.rs            draft, local preview, product editor UI, focus
    build_persistence.rs       one bounded last-used-build file
    queue.rs                   queue client model, timeout/retry, message observation, Queue UI
    session.rs                 Lightyear entity/lifecycle only; no editor or queue policy
  server/lobby/
    mod.rs                     composition, intentional re-exports, and explicit schedule sets
    session.rs                 M03 authenticated membership/name lifecycle and teardown intent
    catalog.rs                 M03 operator catalog authority
    queue.rs                   tickets, pools, admission/cancel transaction, snapshots
    queue_telemetry.rs         bounded counters/high-water marks without player or ticket identity
    transition_driver.rs       explicit M01 compatibility smoke only
```

Converting `src/lobby.rs` to `src/lobby/mod.rs` is an organization-only move: existing public module
paths remain unchanged, and `queue.rs` does not turn shared presentation types into server policy.
The 2,000-line M03 `client/flow.rs` remains the one transition arbiter, but editor rendering,
navigation, preview algorithms, queue state, and timeout policy stay in their owned files and expose
only small typed intents/commit data. The 1,200-line `server/lobby/mod.rs` is split because session,
queue, and transition allocation now have different state owners and production compositions, not
to meet a line target.

The supervisor remains unaware of queues and builds in M04. No M04 queue state crosses IPC because
there is no allocation. The ticket's future transfer contract is documented for M05, not added to
the routing crate before it has a production consumer.

## Shared build candidate and accepted summary

The existing match-selection request and the new lobby admission are two real consumers. Replace
their protocol-local candidate duplication with one shared bounded application shape while keeping
match mutation in `builds/server.rs`:

```text
BuildCandidate
  build_revision
  selection
    Preset(BuildPresetId)
    Custom(BrawlerBuildRecipe)

AcceptedBuildSummary
  canonical_recipe
  identity: SelectedBuild
  total_points
```

The queue ticket additionally stores the server's `ResolvedMatchLoadout`. That internal resolved
value is what M05 must transfer or reproducibly reconstruct under an explicit manifest contract.
M04 does not send the full 4 KiB resolved loadout back over lobby messages merely to render the
editor. After the global content handshake matches, the client can resolve the accepted canonical
recipe locally for presentation and compare its identity/point total with the authoritative
summary. A disagreement is a protocol/content failure, not a reason to trust the preview.

`SelectedBuild.source_build_preset_id` is the sole preset-origin field in the accepted summary;
there is no second top-level copy that could disagree with it. Preset admission resolves the current
server preset and records both that identity and its canonical recipe. Custom admission records
`source_build_preset_id = None`. Passive order is
canonicalized only where the existing identity resolver already canonicalizes it; the accepted
recipe remains a complete reproducible input.

## Queue wire contract

All variable-length fields retain decode-time bounds. Current-schema messages are:

```text
QueueClientMessage                          # client -> server
  Command
    request_id: QueueRequestId
    command:
      Join
        catalog_revision
        game_type_id
        game_type_configuration_revision
        build: BuildCandidate
      Cancel
        ticket_id: QueueTicketId
  OutcomeAck
    request_id: QueueRequestId

QueueCommandOutcome                         # server -> client
  request_id
  decision:
    Joined
      membership
    Cancelled
      ticket_id
      resulting_pool_state_revision
    Rejected
      typed reason

QueueMembership
  ticket_id
  catalog/game-type identity and revisions
  accepted_build: AcceptedBuildSummary
  admitted_at_pool_state_revision

QueuePoolSnapshot
  catalog_revision
  state_revision: nonzero u64
  pools: 1..=8 rows in advertised operator order
    game_type_id
    game_type_configuration_revision
    queued: u16
    formation_size: u8
```

The single `QueueClientMessage` type preserves the actual client-to-server order of commands and
acknowledgements in one `MessageReceiver<QueueClientMessage>`; the application does not attempt to
reconstruct cross-type order from separate typed receivers. Client messages and outcomes stay on
ordered-reliable `SessionChannel` in their exact directions. `QueuePoolSnapshot` is registered
server-to-client only on a new `QueueSnapshotChannel` configured as sequenced-unreliable. A full
snapshot is sent immediately after a visible mutation and once per second while sessions exist.
Each refresh is another recovery opportunity, but consecutive loss has no delivery deadline. Its
`ChannelSettings.retry_unsent_messages` is false so a
locally bandwidth-rejected refresh is dropped and replaced by a later complete refresh. The global
registry fingerprint and compatibility version cover the additional channel and registrations.

Lightyear deserializes network messages into each typed receiver during its receive phase. The M04
adapter therefore does not claim to impose a pre-deserialization decode cap. Routed datagram size and
the existing per-peer transport/router packet budgets remain the lower-layer bounds. At the
application boundary, the adapter drains and interprets at most four `QueueClientMessage`
envelopes—commands plus acknowledgements—per authenticated session in one update, or 128 across the
32-session lobby. Observing a fifth ready envelope is a protocol-abuse failure: drop the remaining
ready envelopes without interpreting them, emit no per-input outcome, and disconnect the sender.

A per-session token bucket admits a burst of four new semantic commands and refills one token per
elapsed real second. After structural/request-identity validation, an identical same-ID retry for
the application-unacknowledged outcome is recognized before semantic-token consumption: it consumes
no semantic-command token, does not enqueue another wire copy, and relies on Lightyear's retained
reliable send. The initial command sets `identical_retry_not_before` to ten seconds later. One retry
at or after that deadline is accepted as suppressed recovery and advances the deadline by another
ten seconds. Earlier identical duplicates are dropped without an outcome and increment a saturating
per-session early-duplicate count; the fourth early duplicate before the deadline is protocol abuse
and disconnects. A deadline advance resets that count. Every other command consumes a semantic token
before admission/idempotency mutation. The first over-token new command, when no outcome is pending,
returns one bounded `RateLimited { retry_after_millis: u16 }` outcome and starts a notice window
through that delay. The delay is the remaining time until the next whole-token refill, clamped to
`1..=1000` milliseconds. A further command during that window, including one sent after
acknowledging the notice, is protocol abuse and disconnects without another outcome. Rate time
affects only abuse handling, never FIFO or queue authority. Refill uses whole elapsed seconds, caps
at four, carries no fractional credit, and clears the notice window once the advertised retry delay
expires. Because `RateLimited` is an authoritative rejection, an identical same-ID retry while its
outcome is pending remains the same suppressed recovery case. After the client acknowledges that
outcome and the delay expires, **Try Again** freezes the same command body under a new monotonic
request ID. Deterministic tests advance the supplied monotonic clock.

Each authenticated session may have at most one application-unacknowledged queue outcome. Its
canonical encoded size must be at most `MAX_QUEUE_OUTCOME_BYTES = 512`, so M04 can retain at most 32
queue outcomes and 16 KiB of queue-outcome payload process-wide, excluding fixed map/container
overhead and Lightyear's bounded-per-message transport metadata. After consuming an outcome, the
client immediately sends `QueueClientMessage::OutcomeAck` with its request ID. An acknowledgement
is idempotent for the last acknowledged outcome; an impossible future/mismatched acknowledgement
is a protocol failure. A different command while an outcome is unacknowledged is also a protocol failure. An
identical same-ID Retry returns the cached semantic outcome but does not enqueue another wire copy;
Lightyear already retransmits the pending reliable message. Normal product UI therefore has exactly
one semantic command in flight, while same-update identical duplicates remain testable through the
explicit early-duplicate threshold.

Rejection vocabulary and player behavior are closed and typed:

| Class | Reasons | Client behavior |
|---|---|---|
| Correctable build | incompatible passive pair; over budget with exact used/budget values | Retain the frozen draft and show product copy. Incompatible passives focus Passive 2. Over budget focuses the last edited cost-bearing control, falling back to Power when no such edit exists |
| Impossible game/content disagreement | stale catalog, stale game configuration, or unknown game type despite the immutable accepted advertisement and matching session handshake | Treat as incompatible session state, disconnect cleanly, and offer a fresh connection to the same server or Back to Server Select; M04 does not claim an in-session catalog refresh path |
| Membership conflict | already queued/must cancel first, ticket mismatch, stale request | Preserve authoritative local membership if known; otherwise show the context-valid retry/back action |
| Temporarily unavailable | lobby shutting down, no longer admitting, internal build resolution failure, or the first command beyond the token bucket | Retain the draft or membership as appropriate; `RateLimited` disables **Try Again** until its bounded server-provided delay expires and then uses the same frozen body under a new request ID, while other unavailable outcomes offer Retry or Disconnect without inventing queue membership |
| Protocol/content failure | zero/undecodable IDs, over-bound candidate, stale build revision after a matching immutable content handshake, unknown build content unavailable to the product editor, conflicting payload for one request ID, or impossible same-revision content | Disconnect the incompatible session; do not expose codec/debug vocabulary as a correctable build error |

An advertised game type that is temporarily no longer admitting remains the recoverable
`Temporarily unavailable` case; it is not represented as a catalog refresh. The wire reason remains
a bounded domain enum rather than carrying presentation text or a UI control
identifier. The client maps each reason to fixed product copy and the deterministic focus behavior in
this table. Local validation uses the same incompatible-pair and exact used/budget facts, so an
ordinary product client should not submit either correctable error; the authoritative mapping still
makes a server rejection recoverable and testable. A stale build revision or unknown build ID cannot
arise from the current immutable, fingerprint-matched editor and is therefore not misrepresented as
something the player can repair by changing a field.

M04 has no `QueueFull` rejection: with at most one ticket for each of 32 authenticated sessions,
every authenticated session can queue, including all 32 in one pool. M04 also has no reservation
outcome or state; M05 adds those atomically with formation.

The global handshake remains the compatibility boundary. M04 does not add `V1` message suffixes,
per-message versions, old decoders, or a fallback request dialect.

## Authoritative queue transaction

The lobby owns a pure bounded queue model adapted by Bevy systems. Each authenticated session owns:

```text
QueueCommandMemory
  last_request_id
  canonical last request
  exact last outcome
  pending_outcome_request_id (optional)
  last_acknowledged_outcome_request_id (optional)
  command_tokens: 0..=4
  command_token_refill_at: monotonic real time
  rate_limit_notice_until: monotonic real time (optional)
  identical_retry_not_before: monotonic real time (optional)
  early_identical_retry_count: 0..=3
  active_ticket_id (optional)
  exact last-cancelled outcome (optional bounded tombstone)
```

Each ticket owns:

```text
QueueTicket
  ticket_id
  lobby_session_id
  player_id
  Netcode client identity
  catalog and game-type identity/revisions
  accepted canonical recipe and SelectedBuild identity
  resolved immutable loadout
  admission_order
  admitted_at_pool_state_revision
```

Each pool is a bounded FIFO of ticket IDs; tickets are also indexed by lobby session, authenticated
Netcode identity, and ticket ID for constant-time membership, ownership validation, and cleanup.
There are at most 32 total tickets and 32 tickets in any one pool. Indexes are checked after every
mutation in focused tests. No process-local entity crosses the model boundary.

Admission order is assigned only after all validation succeeds and immediately before the ticket
and pool indexes mutate. Every successful mutating Join or Cancel reserves the next nonzero public
state revision before mutation. A new ticket records that revision as
`admitted_at_pool_state_revision`. Several commands in one update may therefore create consecutive
revisions while one final complete snapshot publishes the highest resulting revision; skipped
intermediate snapshots are safe. A failed request consumes no ticket ID, admission order, or
revision. Exhausted ticket, order, or revision sources stop new queue mutations and return a typed
unavailable/fatal server condition without partial indexes or an unpublished public change.
Production ticket IDs use OS-backed entropy; tests inject deterministic IDs through the same narrow
source pattern already used for lobby session IDs.

For one application update, server systems:

1. capture the stable IDs of sessions authenticated at frame start, observe authenticated session
   loss, and drain each session's ordered `QueueClientMessage` stream after Lightyear receive;
2. reject queue input from sessions outside that captured set, discard all inputs for sessions lost
   in that same update, and remove their existing tickets;
3. preserve the envelope order directly, apply a matching acknowledgement before a following
   command, recognize a pending identical retry before token consumption, enforce the per-update and
   token bounds, then group admitted commands by stable authenticated player ID instead of Bevy
   query order;
4. reject a different command while an outcome remains unacknowledged, validate joins completely
   before mutation, and apply cancels exactly once;
5. commit each successful mutation with its pre-reserved public state revision;
6. queue at most one exact reliable outcome per session, then publish the newest complete sequenced
   snapshot if aggregate state changed.

Equal request ID plus identical command returns the cached semantic outcome without mutation; while
that outcome remains application-unacknowledged, the bounded identical-retry cadence above suppresses
another wire copy. Equal ID plus different command is a protocol violation. Lower IDs are stale. A
newer Join while already queued
returns the current membership without changing FIFO order only if it describes that exact active
membership; it reuses the ticket's original `admitted_at_pool_state_revision`, consumes no public
revision, and therefore requires only a snapshot at or above that original admission revision. A
different game/build is rejected with Must Cancel First. Repeated Cancel of the last
cancelled ticket replays its exact cancellation outcome, including the original resulting revision.
A cancel for another ticket is rejected and cannot remove membership.

Ordered-reliable transport does not remove these rules: it delivers bytes reliably and in order,
while application acknowledgement bounds retained queue outcomes and the transaction rules make UI
double activation, duplicated application sends, stale state, and future retry logic safe.

## Aggregate pool state

The worker generation begins at public `state_revision = 1`. The lobby schedules that initial full
snapshot after the M03 welcome, after any batch that changes public
counts, and on a one-second refresh cadence while at least one session exists. Rows always match the
immutable advertised catalog order and revisions. `formation_size` is checked multiplication of
teams by players per team and must fit the routing participant bound.

The snapshot exposes only aggregate queued tickets. It does not reveal player names, builds,
ticket IDs, admission order, reservation state, or a wait estimate. Until M05 actually removes an
exact roster into a reservation, overflow tickets remain included in the queued count. The UI says
`6 waiting · 4 players per match`; it never formats the values as `6 / 4`, an ETA, progress, or a
promise which four players will form first.

The client accepts only the active session generation and current catalog revision. A higher
`state_revision` replaces the complete snapshot and records local monotonic receipt time. A lower
revision is ignored without renewing freshness. The same revision with byte-equivalent canonical
content leaves pool data unchanged but renews receipt time; the same revision with different content
is a protocol failure. Revision gaps and loss are safe because every message is complete and
refreshes repeat the newest state. If no valid snapshot has arrived or the last accepted/renewed
receipt is more than three seconds old, Game Select and Queue show `Updating queue` rather than zero
or an old count. The next higher or byte-equivalent current-revision snapshot restores counts
immediately. The three-second age is presentation freshness only and never changes membership or
authority.

Membership outcomes carry the public revision required to present their state honestly: Joined
membership carries the ticket's `admitted_at_pool_state_revision`, while Cancelled carries the
revision created by its removal. The outcome
and snapshot use different channel semantics and may be observed in either order. The client
transitions only on the reliable outcome, never by inferring membership from counts. It displays
`Updating queue` whenever its newest snapshot revision is lower than the outcome's required
revision; a snapshot already at or above that revision is immediately honest. A cancellation may
return to Game Select before its snapshot arrives, but the affected pool stays `Updating queue`
until the required revision is present.

## Queue diagnostics and evidence

The lobby owns one bounded, privacy-safe `QueueTelemetry` resource. It records current and high-water
active tickets and pending outcomes plus saturating counters for successful admissions,
cancellations, disconnect removals, each bounded rejection class, rate-limit notices, protocol-abuse
disconnects, suppressed identical retries, early identical retries, and
initial/mutation/refresh snapshot publications requested. It
contains no display name, player/session/ticket/request identity, build recipe, capability, address,
or unbounded per-event history.

The client queue model separately owns two saturating presentation counters for fresh-to-stale aging
and stale-to-fresh restoration. They observe the already-required three-second presentation rule and
do not report delivery, infer membership, or feed authority.

The pure transaction returns a small typed mutation/rejection fact to the Bevy adapter; telemetry
observes that fact and never becomes another admission or cleanup path. `MessageSender::send` has no
per-message delivery result in pinned Lightyear, so M04 records publication intent and client-visible
freshness aging; packet loss/drop remains existing transport diagnostics rather than an invented
application delivery acknowledgement.

Focused tests assert counter saturation, exact current/high-water convergence after cleanup, and no
identity-bearing fields. The headless queue smoke includes a final bounded aggregate in its process
evidence so admission, cancellation, cleanup, and snapshot activity are diagnosable without logs or
private membership disclosure.

## Build Editor and local persistence

`BuildEditor` becomes a `ClientOverlay` variant over Game Select. It owns one local draft and no
authority. Opening it from **Build & Join** copies the valid last-used selection or preset 1. Cancel
discards edits made during that opening and restores focus to **Build & Join** for the still-selected
card.

The overlay retains one Custom recipe independently of the currently highlighted preset. If the
loaded selection is Custom, that recipe initializes it. Otherwise it initializes to the explicit
legal default `Balanced / Standard / Standard / Dash / Lightweight Frame / Tenacity`. Switching to
a preset does not destroy the Custom fields; switching back restores them for the lifetime of that
opening. Closing without admission discards all opening-local changes. A server rejection retains
the frozen submitted values and focuses the first implicated field; admission acceptance replaces
the active last-used selection.

The first product layout should use the existing exact content rather than a general editor:

- four preset choices plus one Custom choice;
- Custom fields for power, reach, magazine, ultimate, passive 1, and passive 2;
- selected field/value controls usable by pointer, keyboard, D-pad, and stick;
- point cost beside every preset and the currently selected Custom value, with every alternative's
  cost visible when its selector is open; total used and remaining against 12, changed-value
  feedback, and the first invalid reason;
- while a Custom selector is focused or open, a bounded before/after panel comparing the current
  legal draft with the highlighted alternative: point delta plus only the health, movement, weapon,
  ultimate, or passive lines that would change; invalid alternatives show the resulting budget or
  compatibility problem instead of fabricated resolved stats;
- an exact maximum-health integer; movement speed as an integer when integral or an `approximately`
  labelled one-decimal value otherwise; weapon delivery label, magazine capacity, fire interval,
  and refill time; exact damage, range, and projectile speed for the simple Custom Pulse; concise
  family-specific behavior for preset straight/spread, lobbed/area, and melee weapons, including
  exact per-hit or per-projectile damage and product-labelled reach/area/effects where meaningful;
  ultimate and passive display names, costs, and their existing concise descriptions;
- selected game type and one primary `Join Queue` action;
- disabled Join while the request is in flight, while the local draft cannot resolve, or after
  lobby membership is lost.

One pure presentation helper derives the stable summary from `ResolvedMatchLoadout`; a second pure
comparison helper resolves the highlighted candidate through the same local resolver and computes
the bounded changed lines without mutating the draft. Tick durations render
as exact seconds when divisible by the fixed tick rate or as `approximately` one decimal otherwise.
Complex weapons use bounded family-specific lines rather than collapsed aggregate DPS: damage is
labelled per hit/projectile/impact, area and falloff are named when applicable, and knockback/slow
are described in player terms. Do not expose internal fingerprints, ticks, recipient-policy enums,
raw enum debug names, or unlabelled rounded stats. Product copy calls local output a preview until
the server accepts it.

`BuildFileV1` stores only:

```text
schema_version
build_revision
selection: Preset ID or complete custom recipe
```

It has a small explicit byte bound, platform-local `build.ron` path, missing-file default, full
validation on load, and the established atomic replacement policy. A stale revision is not
silently migrated. After admission, the accepted selection becomes active in memory and is saved.
A save failure never cancels or weakens the authoritative ticket: Queue remains the underlying
flow and Error offers Retry Save or Continue Without Saving.

Extend M03's closed `ClientLocalLoadFailures` aggregate with the third named `build_failed` source.
`local_load_error` composes the one-, two-, or three-source product message into the existing single
Error overlay. This remains three booleans plus deterministic copy, not a notification bus or
arbitrary error collection.

## Client flow and action ordering

Add only the now-rendered state/overlay:

```text
ClientFlow:    ... GameSelect, Queue
ClientOverlay: ... BuildEditor
```

The M03 ordered flow sets remain the composition point. Queue message observation joins
`ObserveSession`; editor and Queue controls join `CollectFlowInput`; one coordinator in
`ResolveFlowAction` selects at most one result; `CommitFlow` changes flow/overlay and persistence
intent; `PresentFlow` updates cards, editor, Queue, focus, and scroll.

Priority extends the M03 rules:

1. active-session loss clears draft request/membership/snapshot and uses M03 recovery;
2. explicit Disconnect tears down membership through session loss and does not pretend Cancel was
   acknowledged;
3. authoritative Join/Cancel outcomes outrank ordinary UI and stale local save feedback;
4. local save failure opens Error over the already-correct underlying flow;
5. ordinary editor/navigation actions cannot submit twice in one frame.

Join and Cancel each freeze one canonical command and start a ten-second presentation deadline.
Before the deadline the primary action reads `Joining...` or `Cancelling...` and cannot submit
again; a secondary **Disconnect** remains reachable immediately, and ordinary navigation remains
usable where it cannot alter the frozen request. There is no local Cancel Join because admission may
already have committed; Disconnect is the safe escape. At expiry, Error offers **Retry** and
**Disconnect**. Retry sends the identical frozen command with the same
request ID and restarts the presentation deadline; it never allocates a new semantic operation.
Disconnect remains directly available from Queue even while cancellation is pending and uses normal
session teardown. There is no automatic retry loop. A late matching authoritative outcome outranks
the timeout overlay and commits its real result; a later outcome from an older request or session
generation is discarded.

`RateLimited` is different from a local ten-second timeout because the server has authoritatively
rejected that operation. The client acknowledges it, retains the frozen command body and current
editor or Queue context, shows the bounded remaining delay, and disables **Try Again** until the
deadline. **Try Again** then allocates the next request ID for the same body. It never reuses the
rejected request ID or retries automatically.

Late outcomes from an old connection generation or non-current request are discarded. A pool
snapshot never creates or cancels local membership. Build rejection keeps Game Select under the
BuildEditor overlay with the draft and focus at the first correctable field. An impossible catalog,
game-configuration, or game-identity disagreement closes the editor and incompatible lobby session,
then offers a fresh connection to the same server or Back to Server Select; it does not claim to
refresh M03's immutable advertisement in place. A clean Cancel acknowledgement enters Game Select
without disconnecting. Unexpected lobby loss while
queued removes the server ticket through session teardown and follows M03's fresh-session recovery;
v2 does not resume the ticket.

Headless automation uses the same Join/Outcome/Snapshot/Cancel messages with configured preset or
custom candidate. It bypasses presentation, not authority.

## Server schedule and role boundaries

Lobby `Update` order:

```text
BeginLobbyFrame
  -> AuthenticateLobbyHellos
  -> CollectQueueClientMessages
  -> ReconcileDisconnectedSessions
  -> ApplyQueueTransactions
  -> PublishQueueOutcomesAndSnapshot
```

This runs after Lightyear's pinned `PreUpdate` receive work. `BeginLobbyFrame` first captures the
stable lobby-session IDs that were authenticated before this application update and observes known
losses. `AuthenticateLobbyHellos` then runs M03's existing Hello transaction. Any session accepted
there is deliberately ineligible for queue commands until the next update; client-side observation
of Welcome is the ordinary product gate. Queue input from a session absent from the frame-start
eligibility set is a protocol failure and cannot be reordered into post-authentication intent merely
because its Hello was accepted from a separate typed receiver in the same update. The eligibility
snapshot is stable-ID data, not a retained Bevy `Entity` handle, and is cleared/rebuilt every update.

Replace M03's direct session mutation in
both `lobby_cleanup_disconnected` and `On<Remove, LobbyClient>` with emission of one bounded,
deduplicated `LobbySessionLost` record carrying the stable lobby session and authenticated Netcode
identity. `ReconcileDisconnectedSessions` consumes those records, removes each owned ticket first,
then removes the lobby session through one helper that mutably owns both pure models. The lost-
session set remains visible to `ApplyQueueTransactions`, so a command collected in the same update
cannot recreate membership. Observer timing, deferred entity removal, system registration, and query
order therefore cannot bypass queue cleanup. Tests exercise both the polled marker and component-
removal observer paths.

Outcome acknowledgements clear only the matching per-session pending-output marker, identical-retry
deadline, and early-duplicate count; they never mutate queue membership. The unified receiver is
drained in order, so an acknowledgement followed by the next command on ordered-reliable
`SessionChannel` is valid in one update while the reverse order is not silently rearranged. Queue
mutation is immediate resource mutation inside one pure transaction; deferred entity commands
are not used as the ticket commit boundary. Snapshot refresh timing uses elapsed real time for
presentation delivery only and never affects FIFO or authority.

The server feature graph may use authored build catalogs, weapon/fighter definitions, lobby wire
types, and the pure resolver. It must not acquire UI, clipboard, client persistence, rendering,
audio, device input, or client assets. The supervisor and routing package remain Bevy-, Lightyear-,
gameplay-, build-, and queue-free in M04.

## Planning slices

Implementation was authorized by the user on 2026-08-19.

### Slice 1 — Establish queue/build contracts on the reconciled M03 seams

- [x] Record M03's delivered catalog, selected-game, flow-action, session-generation, error,
  persistence, and dual session-loss seams in this specification.
- [x] Keep M04 queue-only and move reservation plus cancellation-versus-reservation races to M05.
- [x] Define bounded shared build candidate/accepted summary and queue IDs/messages without
  duplicating authored definitions or moving authority client-side.
- [x] Increment the one global compatibility version and register current messages in exact
  directions on ordered-reliable `SessionChannel` and sequenced-unreliable
  `QueueSnapshotChannel`; retain no old decoder.
- [x] Add pure serialization, decode-bound, identity, revision, and resolver-agreement tests.

### Slice 2 — Deliver authoritative admission before product UI expansion

- [x] Add the pure bounded lobby queue model with one ticket per session, deterministic FIFO,
  immutable resolved loadout, exact indexes, pre-reserved public revisions, idempotent Join/Cancel,
  admission-revision ownership, one pending outcome per session, the four-token/one-per-second
  semantic limiter with one fail-soft notice window, the ten-second identical-retry cadence and
  early-duplicate abuse threshold, and queue-aware disconnect cleanup.
- [x] Add explicit lobby schedule sets and one deduplicated session-loss seam over real
  authenticated M03 sessions and both existing teardown paths; capture frame-start authentication
  eligibility before processing new Hellos.
- [x] Publish initial, post-mutation, and one-second refresh snapshots with bounded sequenced
  delivery, three-second client freshness aging, and required-revision barriers on membership
  outcomes.
- [x] Record bounded privacy-safe queue counters/high-water marks and include their final aggregate
  in headless process evidence without identity-bearing history.
- [x] Add an in-memory authenticated flow proving welcome → snapshot → join accepted → queued →
  cancel accepted → Game Select state data, with no allocation request.
- [x] Prove through focused pure, ECS-adapter, and separate-App scenarios that duplicate, stale,
  malformed, ID/revision exhaustion, maximum-capacity, outcome-ack,
  semantic-rate abuse, early-identical-retry abuse, pre-/same-frame authentication, stalled-client,
  and same-frame disconnect cases before UI work.

### Slice 3 — Replace the debug editor in the product lobby flow

- [x] Add BuildEditor as the one overlay over Game Select and Queue as the one new flow state.
- [x] Reuse M02/M03 focus, pointer, controller, scrolling, style, and action-arbitration seams.
- [x] Present all four presets, six custom fields, per-choice costs, used/remaining budget, invalid
  reason, focused before/after changed lines, and meaningful family-specific resolved preview
  through the specified pure presentation/comparison helpers without raw debug formatting or
  aggregate DPS.
- [x] Submit one frozen draft with selected catalog/game identity and map exact server rejection
  reasons to deterministic product copy and focus behavior in retained editor state.
- [x] Preserve card Confirm as selection, add explicit **Build & Join**, and present aggregate pool
  state as `N waiting · M players per match` plus exact accepted membership on Queue.
- [x] Add immediately reachable Join/Cancel Disconnect, ten-second pending deadlines, same-request
  Retry without duplicate wire outcomes, outcome acknowledgement, and late-outcome precedence.

### Slice 4 — Add last-used persistence and recovery

- [x] Add bounded `BuildFileV1`, missing/default/stale/malformed handling, atomic save, and focused
  temporary-directory tests.
- [x] Load valid last-used state into a fresh editor and save only an authoritative acceptance.
- [x] Reconcile the single Error overlay with three local persistence sources and one dirty build
  save context.
- [x] Prove save failure cannot cancel or alter an accepted ticket and Retry Save is repeatable.

### Slice 5 — Verify and hand off

- [x] Run focused pure/ECS/client/server/protocol/network/process tests, role-feature checks,
  formatting, and Clippy through canonical `justfile` commands.
- [x] Preserve the M01 transition smoke and direct-UDP match build-selection baseline on the current
  global protocol.
- [x] Add `just network-product-queue-smoke` as the canonical bounded headless product path for
  welcome → snapshot → admission → refreshed count → cancellation → refreshed count → exit.
- [ ] Capture representative Game Select population, Build Editor, rejection, Queue, and save-error
  layouts.
- [x] Provide keyboard/mouse and physical-controller playtest steps; feedback remains the explicit
  user-playtest exit gate below.

## Verification contract

### Pure queue and build tests

- all four presets and representative legal custom recipes resolve identically on preview and
  authority; duplicate passives, incompatible passives, unknown IDs, stale revision, over-budget,
  and oversize candidates reject without mutation;
- ticket IDs and request IDs reject zero; ticket-ID, admission-order, and pool-state-revision
  exhaustion fail without partial indexes, consumed visible order, or unpublished public mutation;
- one session cannot own two tickets or switch game/build without cancellation;
- FIFO order survives cancellation from head/middle/tail and overflow remains ordered for the
  future next formation;
- simultaneous first-observed joins use the documented player/request tie-break independent of
  insertion/query order;
- identical request replay returns the exact semantic outcome without queuing a second copy while it
  is unacknowledged; a newer identical Join returns the same ticket and original admission revision
  without mutation; conflicting same-ID payload, lower stale ID, different join while queued,
  repeated cancel, and wrong-ticket cancel have exact outcomes;
- one matching outcome acknowledgement permits the next ordered command; duplicate acknowledgement
  is harmless; future/mismatched acknowledgement or a different command before acknowledgement
  disconnects without another reliable outcome;
- a burst of four acknowledged new semantic commands is admitted and tokens refill one per supplied
  elapsed second; an identical same-ID retry while its outcome is pending consumes no token and
  enqueues no second copy; one retry at each supplied ten-second deadline remains token-neutral and
  advances its deadline, early identical retries are silently dropped and counted, and the fourth
  early retry in one window disconnects without another outcome; more than four total client
  envelopes in one update crosses the independent application-processing abuse bound even though
  Lightyear has already deserialized them; the first over-rate new semantic command with no pending
  outcome returns one bounded `RateLimited` notice without queue mutation or disconnect,
  same-ID replay remains the same rejection, a compliant new-ID attempt after its delay succeeds,
  and another command during the notice window disconnects without an outcome flood;
- polled `Disconnected` and `On<Remove, LobbyClient>` each pass through the same loss record and
  remove one ticket before its session; duplicate observation removes nothing else; disconnect plus
  Join in one update creates none;
- a queue envelope before Hello, in the same update as an accepted Hello, or before the authenticated
  session is frame-start eligible creates no ticket and closes as a protocol failure; an ordinary
  queue command on the update after authentication uses the captured stable session identity;
- every mutation leaves pool/session/ticket indexes bijective and within the 32-session/eight-pool
  bounds; all 32 sessions may queue in one pool and no unreachable `QueueFull` outcome exists;
- complete snapshot revision increments only for public count changes and preserves catalog order,
  exact count, and formation size; fresh Joined membership records its admission revision, a no-op
  equivalent re-Join reuses that revision, and Cancelled names the revision created by removal;
- every queue outcome encodes within 512 bytes; at most one unacknowledged outcome per session and
  32/16 KiB process-wide queue outcomes/payload can be retained by the application adapter;
- client-visible `QueueMembership` omits server-only `admission_order`; only the authoritative ticket
  and pool indexes retain FIFO order;
- queue diagnostics saturate safely, current/high-water ticket and pending-outcome values converge
  through admission/cancellation/disconnect, rejection/snapshot and suppressed/early-identical-retry
  counters follow exact typed facts,
  and the aggregate schema contains no identity, recipe, capability, address, or unbounded history;
- accepted summaries have one preset-origin field through `SelectedBuild`; canonical recipe,
  fingerprint, revision, preset origin, and total points agree for presets and custom builds.

### Persistence, ECS, and UI tests

- valid preset/custom round trip, missing file, unsupported schema, stale build revision, malformed,
  invalid recipe, oversize, and save failure preserving active memory/ticket;
- BuildEditor is the only overlay, traps focus, restores the selected game card, and does not leave
  descendants after close;
- every preset/field/value, **Build & Join**, Join/Cancel, Retry, and Disconnect is reachable by
  pointer, keyboard, and controller;
- every preset exposes its point cost, every Custom alternative exposes its cost while its selector
  is open, and the selected-value cost plus used/remaining points remain visible and reconcile to
  authority; each focused Custom alternative shows only its point and resolved-stat/behavior deltas
  from the current legal draft, invalid alternatives show the exact unresolved problem, Custom Pulse
  changes visibly update exact damage/range/speed/economy, and preset summaries describe their actual
  delivery/effect families without aggregate DPS;
- card Confirm remains selection-only; Custom initializes to the specified legal default, survives
  temporary preset selection within one opening, and is discarded on editor Cancel;
- invalid local draft disables Join and focuses/shows the exact issue; server incompatible-passive
  rejection focuses Passive 2, server over-budget rejection focuses the last edited cost-bearing
  control with the specified fallback, and neither mutates last-used state; stale/unknown build
  content after a matched immutable handshake follows protocol/content failure instead of pretending
  to be field-correctable;
- double activation and held input create one in-flight request; Disconnect is reachable immediately;
  ten-second expiry offers Retry and Disconnect; Retry preserves the request ID and frozen bytes; a
  late matching outcome closes the timeout overlay and wins exactly once;
- `RateLimited` retains the frozen body and current context, acknowledges the rejection, exposes the
  bounded remaining delay, disables **Try Again** until expiry, and then sends the same body under a
  new request ID without automatic retry;
- Join acceptance enters Queue with accepted summary; save failure opens Error over Queue without
  losing membership; Retry Save and Continue Without Saving are exact;
- Cancel enters Game Select only on acknowledgement; wrong/stale outcome and pool snapshot cannot
  synthesize membership;
- unexpected loss clears queue/editor/pool state once and follows M03 recovery; late old-generation
  messages cannot reopen Queue;
- an impossible same-session catalog/game disagreement closes the incompatible lobby session and
  offers fresh connection/Server Select actions without claiming an in-session catalog refresh;
- Game Select never shows zero before the initial pool snapshot, ages a snapshot to `Updating queue`
  after three seconds, restores the next valid snapshot, formats fresh counts as
  `N waiting · M players per match`, and never exposes a wait estimate or ratio-like progress;
- maximum eight cards, every legal custom value, long valid names, and maximum UI scale remain
  scrollable with deterministic focus.

### Protocol and network tests

- the unified client command/ack envelope and outcomes are registered only in their allowed
  directions on ordered-reliable `SessionChannel`; one typed receiver preserves `Ack -> Command`
  in one update, rejects a different/new `Command -> Ack` while an outcome is pending without
  reordering it, and snapshots remain only server-to-client on sequenced-unreliable
  `QueueSnapshotChannel` with local unsent retry disabled, and every variable field rejects
  over-bound input during decode; the membership wire shape contains no admission order or global
  queue position;
- compatible welcome schedules one valid full snapshot; several consecutive dropped refreshes age
  counts to `Updating queue`, the next delivered full refresh restores them, a byte-equivalent
  current-revision refresh renews freshness without replacing pool data, an older snapshot does not
  renew freshness, and conflicting same-revision content fails;
- outcome and snapshot may arrive in either order; Joined/Cancelled transitions use only the
  outcome, and affected counts remain `Updating queue` until a snapshot at or above the outcome's
  required revision arrives;
- two clients join the same pool in deterministic order, one cancels, the other remains, and both
  observe the same next aggregate revision;
- clients in different pools never receive each other's membership/build details but observe the
  same aggregate rows;
- 32-session/ticket bounds, malicious duplicate commands, missing outcome acknowledgement, a
  fail-soft first rate-limit notice, continued semantic-rate abuse, early identical-retry abuse, a
  stalled client, sustained queue churn,
  and disconnect cleanup remain within the explicit 32-outcome/16-KiB application payload bound
  without reliable snapshot accumulation or allocation;
- `just network-product-queue-smoke` connects a headless product client, selects an advertised game,
  submits a configured build, verifies acceptance/fresh snapshot, cancels, verifies the new snapshot,
  and exits deterministically;
- product queue activity emits no supervisor allocation/control request and starts no match worker;
- explicit M01 transition smoke and direct-match build selection retain their accepted behavior.

### Visual and manual checks

- inspect 960×540, 1280×720, and one 16:10/ultrawide layout at default and maximum UI scale;
- inspect all preset/custom states, cheapest/maximum legal budget, each invalid reason, in-flight
  admission, command-timeout Retry/Disconnect, rate-limit delay/Try Again, each rejection class,
  initial/stale updating state, 0/1/exact/overflow pool counts, Queue, cancellation, and build-save
  Error;
- keyboard/mouse: choose game, open/cancel editor, edit every field, join, correct rejection, cancel
  queue, re-edit, and disconnect;
- controller: complete the same ordinary flow without text input, with visible focus after every
  overlay/flow change and controller reconnect;
- unprompted usability: choose and join with a preset, explain one preset tradeoff and the queue
  count, predict the direction of change before committing representative power, reach, magazine,
  ultimate, and passive alternatives, create a valid Custom build, correct one incompatible or
  over-budget draft, and discover that changing the accepted queued build requires acknowledged
  cancellation; record incorrect predictions, where the player needed explanation, and lost focus;
- verify opponent names/builds, global queue position, and wait estimate never appear.

## Exit criteria

- [x] M03 is complete and its final delivered seams are reconciled here.
- [x] The user validated the final M04 specification by directing implementation on 2026-08-19.
- [ ] Build Editor supports every current preset/custom choice with honest budget, preview, precise
  invalid feedback, visible option costs and meaningful tradeoffs, and controller/keyboard/mouse
  operation.
- [x] Last server-accepted build survives restart when valid and missing/malformed/stale/save-failed
  local data fails safely without altering queue authority.
- [x] The lobby admits queue intent only from sessions authenticated at frame start, atomically
  validates selected game/catalog/build identity, and creates at most one immutable ticket per
  authenticated session.
- [x] Queue requests/cancellation are idempotent and safe under duplicates, stale IDs, overflow,
  same-frame disconnect, timeout/retry, and ID/order/revision exhaustion; reservation races remain
  explicitly owned by M05. A newer equivalent Join reuses the existing ticket and admission revision
  without consuming another public revision.
- [x] Game Select and Queue show revisioned real aggregate counts and exact formation size without
  revealing private membership or inventing an estimate; membership-changing outcomes provide an
  explicit freshness barrier, stale snapshots age out after three seconds, a byte-equivalent
  current-revision refresh renews freshness, and any later delivered current snapshot restores the
  display.
- [x] Cancel Queue returns to Game Select without disconnecting and preserves a still-advertised
  selected game; unexpected loss removes server
  membership and follows fresh-session recovery; Disconnect remains immediately reachable while
  either Join or Cancel is pending.
- [x] Command and snapshot delivery remain bounded under a stalled client and sustained churn;
  at most one application queue outcome of at most 512 bytes is unacknowledged per session, abuse
  cannot create an outcome flood, the first over-rate new semantic command fails softly with one
  bounded notice, identical recovery is token- and wire-copy-neutral only at the bounded ten-second
  cadence, repeated early duplicates disconnect, unchanged current-revision refreshes renew
  freshness, stale snapshots age out, and complete snapshots never accumulate in an ordered-reliable
  history.
- [x] Privacy-safe bounded diagnostics expose queue current/high-water state and typed aggregate
  outcomes needed to diagnose admission, cleanup, abuse, and snapshot publication without retaining
  player, ticket, request, build, capability, or address identity.
- [x] M04 product composition never allocates a worker; M01 transition and direct-match behavioral
  baselines remain explicit and green.
- [ ] Focused pure/ECS/protocol/network/process/role and representative visual/controller evidence
  pass.
- [ ] User playtest feedback is recorded and triaged before M04 is marked `Complete`.

## Research record

### Repository and version-pinned local sources

- `docs/00-product-direction.md` — meaningful bounded build tradeoffs and network-first authority.
- `docs/13-player-ux.md` — build selection lifecycle, exact FIFO pools, immutable tickets, privacy,
  cancellation, recovery, and product flow.
- `docs/14-multiplayer-server-architecture.md` — lobby ownership of catalog/build/queue/reservation
  and supervisor ownership of host admission only.
- `docs/08-network-architecture.md` — stable identity, candidate/selected/runtime separation, global
  compatibility gate, and client-intent authority boundary.
- `docs/implementation/v2/{roadmap,milestone-01,milestone-02,milestone-03}.md` — delivery ordering,
  routed lobby/process seams, shell/persistence behavior, and selected advertised game identity.
- `src/builds/{model.rs,definitions.rs,server.rs,tests.rs}` and `content/v1/builds.ron` — exact
  candidate, catalog, resolver, budget, fingerprint, current match mutation, and test behavior.
- `src/client/{mod.rs,flow.rs,session.rs,shell.rs}` — delivered M03 state/overlay, ordered action
  arbitration, deferred teardown boundary, message lifecycle, focus, and two-source persistence
  error behavior.
- `src/server/lobby/{mod.rs,catalog.rs}`, `src/lobby.rs`, `src/protocol.rs`, and
  `packages/brawler-routing/src/{control.rs,manifest.rs,limits.rs,runtime.rs}` — current lobby IDs,
  delivered server-only catalog resolution, shared catalog wire model, bounds, product/transition
  plugin split, dual disconnect paths, message channels, and future handoff constraints.
- `references/lightyear/book/src/concepts/reliability/channels.md` — ordered-reliable guarantees and
  their limits.
- `references/lightyear/book/src/tutorial/build_client_server.md` and
  `references/lightyear/book/src/concepts/bevy_integration/system_order.md` — entity-scoped message
  sender/receiver lifecycle and receive/send schedule placement.
- `references/lightyear/examples/simple_setup/` — exact pinned typed-message and ordered-reliable
  channel composition.
- installed Bevy 0.19.1 and Lightyear 0.29.0 crate sources — exact APIs remain the implementation
  authority because the checked-in Bevy snapshot is 0.20-dev and public Lightyear indexing lags
  the pinned local version.
- installed Lightyear 0.29.0 `lightyear_transport/src/channel/{builder,send_reliable}.rs` — reliable
  sends retain each unacknowledged message and expose no application capacity setting, motivating
  complete sequenced-unreliable queue snapshots with periodic refresh instead of reliable snapshot
  accumulation, plus one explicitly acknowledged pending queue outcome per session.
- checked-in Lightyear `crates/transport/messages/src/{receive,send}.rs` — each message type owns a
  separate `MessageReceiver<M>` and `MessageSender::send` returns no per-message delivery result,
  motivating one ordered client envelope plus publication-intent/client-freshness diagnostics rather
  than reconstructed cross-type order or invented snapshot delivery acknowledgement.

### Current primary cross-checks

- [Bevy 0.19 state documentation](https://docs.rs/bevy/0.19.0/bevy/state/index.html) confirms that
  `States` and state-scoped cleanup model large application-flow changes.
- [Bevy 0.19 `DespawnOnExit`](https://docs.rs/bevy/0.19.0/bevy/state/state_scoped/struct.DespawnOnExit.html)
  confirms complete Queue-flow root cleanup without manual descendant bookkeeping.
- [Bevy 0.19 UI documentation](https://docs.rs/bevy/0.19.0/bevy/ui/index.html) confirms native flex/grid
  layout, interaction, focus policy, disabled interaction, and scroll primitives are sufficient for
  this bounded editor.
- [Official Bevy directional-navigation example](https://bevy.org/examples/ui-user-interface/directional-navigation/)
  and [scroll example](https://bevy.org/examples/ui-user-interface/scroll/) confirm the native
  focus/scroll patterns already selected in M02/M03.
- [Lightyear typed-message documentation](https://docs.rs/lightyear/0.29.0/lightyear/) documents
  entity-owned `MessageSender`/`MessageReceiver`; the checked-in pinned source supplies the exact
  0.29 schedule and channel APIs where public indexing is incomplete.
- [`atomic-write-file` 0.3.0](https://docs.rs/atomic-write-file/0.3.0/atomic_write_file/) and
  [`directories` 6.0.0](https://docs.rs/directories/6.0.0/directories/) remain the accepted primary
  persistence references from M02/M03; M04 adds no persistence dependency.

## Plan-review resolution

Corrections applied 2026-08-19 after M03 approval:

- reconciled the exact M03 flow sets, single-overlay model, two-source persistence reducer,
  product/transition plugin split, and dual disconnect paths;
- preserved card Confirm as selection and chose one explicit **Build & Join** action;
- kept M04 queue-only and moved the first reservation plus cancellation-versus-reservation race to
  M05 and the roadmap gate where it can be exercised honestly;
- defined one pure, tested build-preview formatter and omitted misleading compound damage numbers;
- selected a closed three-source load-failure aggregate rather than a general error framework;
- named `just network-product-queue-smoke` as the production headless evidence path;
- added required-state freshness barriers and atomic state-revision exhaustion handling;
- replaced reliable full-snapshot accumulation with immediate plus one-second sequenced-unreliable
  complete snapshots while keeping commands and outcomes ordered-reliable;
- added three-second snapshot aging so consecutive unreliable loss cannot leave an old count looking
  current;
- bounded reliable queue outcomes through explicit client acknowledgement, one pending outcome per
  session, a 512-byte outcome ceiling, and a four-token/one-per-second limiter whose first over-rate
  command receives one fail-soft notice before continued abuse disconnects;
- unified acknowledgements and commands in one ordered client envelope so typed receivers cannot
  silently reconstruct a different application order;
- made pending identical retries token-neutral and wire-copy-neutral, closing the duplicate-burst
  state while retaining the limiter for new semantic commands; the final review bounded those
  retries to the product's ten-second cadence with an early-duplicate abuse threshold and corrected
  the application processing cap so it no longer claims to precede Lightyear deserialization;
- made authentication eligibility a frame-start stable-ID snapshot so separately typed Hello and
  queue receivers cannot reorder pre-authentication intent into an accepted command;
- replaced the nonexistent in-session catalog refresh with explicit incompatible-session recovery;
- added bounded privacy-safe queue diagnostics and headless process evidence;
- made Disconnect immediately reachable during Join as well as Cancel;
- expanded build presentation and playtest evidence so costs and gameplay tradeoffs are understandable
  through focused before/after changes without misleading aggregate DPS;
- removed duplicate preset-origin identity from the accepted summary, defined admission-revision
  reuse for a newer equivalent Join, and mapped correctable build rejections to deterministic product
  copy and focus behavior;
- kept FIFO admission order solely in the authoritative ticket instead of exposing an unnecessary
  approximation of queue position in client membership.

## Implementation and verification evidence

Completed 2026-08-19 before the user-playtest handoff:

- the product flow now owns one Build Editor overlay and Queue state, all four presets, every value
  for the six custom fields, authored option costs, exact budget/invalid feedback, focused resolved
  deltas, explicit **Build & Join**, authoritative acceptance/cancellation, pending Retry and
  Disconnect actions, and a visibly disabled rate-limit retry with a live remaining delay;
- `BuildFileV1` loads and atomically saves only the last server-accepted preset/custom selection;
  focused tests cover valid round trips, missing/default, malformed, stale, invalid, oversized, and
  save-failure behavior;
- the lobby owns immutable bounded tickets, FIFO pools and indexes, atomic build/game validation,
  one acknowledged outcome per session, sequenced-unreliable complete snapshots, revision/freshness
  barriers, semantic-command and identical-retry abuse bounds, and cleanup before session removal;
- queue wire registration advanced the one global protocol version and keeps the unified command/ack
  envelope and outcome on `SessionChannel`, with snapshots server-to-client only on
  `QueueSnapshotChannel` and local unsent retry disabled;
- final `just verify` passed: 329 client tests, 289 server tests, 78 serial separate-App/UDP network
  tests, 14 performance gates, 79 routing unit tests plus its process/runtime/isolation suites,
  formatting, all role Clippy checks with warnings denied, server feature isolation, and the routed
  lobby → match → fresh-lobby transition smoke;
- `just network-product-queue-smoke` passed welcome → initial snapshot → admission → admission
  snapshot → cancellation → cancellation snapshot → exit against the production lobby composition.
  The final privacy-safe server aggregate reported one admission, one cancellation, zero current
  tickets/outcomes, ticket/outcome high-water marks of one, and two mutation publications; client
  evidence reported one freshness restoration and no aging. No match worker or allocation request
  was emitted;
- `just network-direct-smoke` passed for both clients, preserving the direct-UDP match build-selection
  baseline on the current global protocol;
- the first full verification run exposed an existing queue-less lobby-observer test that no longer
  had the new loss resource. The observer now records the shared queue-aware loss when that resource
  exists and preserves immediate session cleanup for minimal queue-less compositions; the focused
  regression and final canonical run pass;
- a final bounds audit changed queue-envelope collection from an unbounded ready-message `Vec` to a
  five-item drain: four are the allowed application budget, the fifth proves abuse, and dropping the
  draining iterator discards the remainder without interpreting or retaining it.

Native visual captures and a physical-controller pass are not claimed. The windowed Bevy process
launched successfully, but it was not exposed as an addressable macOS accessibility application in
this environment, so the required layout/controller observations remain the user-playtest gate.

### Implementation review remediation — 2026-08-19

The first post-implementation review found that the ten-second Retry action was continually
preempted by a level-triggered timeout observation, the Build Editor rebuilt its complete UI tree
every update and therefore could not retain scroll/focus state, Queue copy did not identify the
advertised game or accepted build clearly, disconnect cleanup at pool-revision exhaustion could
leave stale public rows, the queue-smoke watchdog did not exit after handling its timeout signal,
and the new queue contract had no separate-App network scenario.

Remediation:

- timeout presentation is now edge-triggered once per pending attempt; Retry keeps the exact frozen
  request and late matching authority still wins;
- the editor keeps one bounded render key, retains an unchanged entity tree, preserves
  `ScrollPosition` across meaningful content rebuilds, scrolls from mouse-wheel input, and keeps the
  controller-selected control visible; invalid/active Join and Back actions are disabled while the
  frozen admission is pending, with Disconnect remaining active;
- Queue resolves the accepted advertised display name and build name, shows the accepted ultimate
  and passives, and exposes a disabled `CANCELLING…` state without hiding Disconnect;
- disconnect cleanup always removes internal membership, but revision exhaustion marks the lobby
  authority generation terminal, suppresses publication under the exhausted revision, and requests
  process failure/restart;
- protocol tests now assert all queue message registrations, the snapshot channel, and exact sender
  directions; focused client/authority tests cover timeout/late-outcome behavior, retained editor
  scroll, accepted copy, cancellation copy, cross-pool aggregates, middle-cancellation FIFO, and
  disconnect exhaustion; a separate-App Crossbeam scenario proves the unified `Ack -> Command`
  envelope order over the real registered `SessionChannel`;
- the smoke watchdog now captures the parent PID, stops the child blocked under the parent's
  `wait`, and lets the parent signal handler exit with status 124 after bounded cleanup instead of
  returning to the interrupted wait. A forced one-second timeout asserted status 124, and the
  subsequent normal `just network-product-queue-smoke` completed admission, fresh snapshot,
  cancellation, and cleanup successfully;
- post-remediation verification passed `just check`, `just lint`, `just test`,
  `just server-features`, shell syntax validation, the forced watchdog assertion, and the normal
  product queue smoke. The test totals are the updated counts recorded above.

The remaining open gate is still representative visual inspection and physical-controller playtest;
those observations are not inferred from automated coverage.

### Implementation review remediation — 2026-08-19, second pass

A second correctness and UX review found five remaining gaps: cancellation could expose an older
snapshot as current before the authoritative cancellation revision arrived; Retry/Try Again always
removed the overlay that owned the pending command; over-budget editor copy omitted the exact used
and excess points; the editor exposed the internal game ID instead of the advertised display name;
and privacy-safe queue aggregates were written on every ordinary lobby refresh instead of only in
an explicit evidence run.

Remediation:

- the client snapshot accessor now enforces both freshness age and the required authoritative
  revision, so Game Select and Queue consumers cannot render a pre-cancellation aggregate as
  current while waiting for the replacement snapshot;
- Retry and rate-limit Try Again restore the pending command's owning context: Join returns to the
  Build Editor and Cancel returns to Queue, while preserving the frozen request semantics;
- one shared pure build-point rule now owns editor accounting and authoritative rejection details;
  invalid over-budget drafts remain editable and show exact copy such as
  `14 used · 2 over the 12-point budget`;
- Build Editor resolves the selected game's advertised display name from lobby membership and
  includes that name in its retained render key, so a membership/catalog presentation change
  refreshes the copy without leaking internal identifiers;
- queue aggregate stderr output is disabled by default and enabled only by
  `BRAWLER_QUEUE_EVIDENCE=1`; the canonical queue smoke sets that gate explicitly, while the normal
  lobby smoke remains quiet.

Post-remediation verification passed `just check`, `just lint`, `just test`,
`just server-features`, `just network-product-queue-smoke`, and
`just network-product-lobby-smoke`. The full matrix contained 332 client tests, 289 server tests,
78 serial separate-App/UDP network tests, 14 performance gates, and 79 routing unit tests plus its
process/runtime/isolation suites. Focused regressions cover the cancellation revision barrier,
Join/Cancel recovery context, advertised game copy, exact over-budget feedback, and the existing
authoritative exact-budget rejection. The remaining gate is still representative visual inspection
and physical-controller playtest.

### Implementation review remediation — 2026-08-19, third pass

A third correctness, completeness, defect, and UX review found that closing Build Editor left
keyboard/controller navigation on an editor-only index, correctable authoritative rejections updated
the editor's logical field but not the actual focused control, several semantically impossible queue
wire values could deserialize, the separate-App queue coverage did not exercise real lobby authority
with multiple clients, and every error overlay was titled `CONNECTION ERROR` regardless of cause.

Remediation:

- cancelling Build Editor now restores navigation to the visible **Build & Join** control, while
  incompatible-passive and over-budget rejections focus the exact field button selected for
  correction; full ECS/input regressions cover both transitions;
- queue membership, outcomes, and pool rows reject zero authoritative revisions, noncanonical or
  unexceeded over-budget details, retry delays outside `1..=1000 ms`, more than 32 queued tickets,
  and formation sizes outside `1..=8` during deserialization;
- the separate-App harness can now compose the production lobby worker with routed authenticated
  peers. Two authoritative scenarios prove stable-ID FIFO ordering, two-client aggregate
  convergence, cancellation, cross-pool public snapshots, private membership, and disconnect
  cleanup; the existing registered-channel scenario continues to prove `Ack -> Command` ordering;
- error overlays now use typed `CONNECTION ERROR`, `QUEUE ERROR`, `SAVE ERROR`, or `CONTENT ERROR`
  headings, including content mismatch paths that disconnect and require a fresh catalog;
- the first lint pass caught one redundant match arm in the new decoder validation. It was removed
  rather than suppressed, and the full warnings-denied matrix was rerun.

Post-remediation verification passed `just check`, `just lint`, `just test`,
`just server-features`, `just network-product-queue-smoke`,
`just network-product-lobby-smoke`, `just network-routed-smoke`, and
`just network-direct-smoke`. The final matrix contained 337 client tests, 291 server tests,
80 serial separate-App/UDP network tests, 14 performance gates, and 79 routing unit tests plus its
process/runtime/isolation suites. The remaining gate is still representative visual inspection and
physical-controller playtest; automated focus regressions reduce that risk but do not replace the
hands-on observation.

### Implementation review remediation — 2026-08-19, fourth pass

A fourth correctness, completeness, and UX review found that an exact-request outcome was not
correlated with its frozen command, ticket identities were checked only against active tickets,
returning from Queue reset the selected game to the first advertisement, and the implementation
checklist still represented pre-implementation planning rather than delivered evidence.

Remediation:

- the client now validates the decision kind plus the frozen catalog, game, configuration, build
  revision, build origin, and cancellation ticket before acknowledging or mutating membership;
  mismatched authority becomes a content/protocol failure and cannot synthesize or replace queue
  membership;
- each lobby generation now creates one unpredictable 64-bit ticket namespace and combines it with
  the never-reused admission order, guaranteeing lifetime-unique 128-bit ticket IDs within that
  worker generation without an unbounded issued-ID history; focused churn tests prove retired
  identities are not reused and do not collide with idempotent cancellation memory;
- Game Select preserves and focuses the prior game when it is still advertised, falling back to the
  first card only when the selection is absent from the accepted catalog;
- the planning slices and exit criteria now distinguish delivered automated evidence from the two
  remaining manual gates. Added regressions cover wrong-game Join, Joined-during-Cancel, retained
  game selection, 32 stalled outcomes within the byte ceiling, 128 join/cancel churn cycles, and a
  queue command sent before lobby Welcome.

Post-remediation verification passed `just check`, `just lint`, `just server-features`, and
`just test`, followed by `just network-product-queue-smoke`,
`just network-product-lobby-smoke`, `just network-routed-smoke`, and
`just network-direct-smoke`. The final matrix contained 340 client tests, 293 server tests,
81 serial separate-App/UDP network tests, 14 performance gates, and 79 routing unit tests plus its
process/runtime/isolation suites. The routed smoke completed the two-client
lobby-to-match-to-fresh-lobby transition, while the direct-UDP baseline completed with both clients
exiting successfully.

The remaining open gates are unchanged: representative visual inspection, physical-controller
playtest, and feedback triage. M04 remains `User playtest` until those observations are recorded.

### Implementation review remediation — 2026-08-19, fifth pass

A fifth correctness, completeness, quality, and UX review found three remaining client-side
presentation gaps: an existing error overlay was retained when a late authority response replaced
the error in the same flow, reopening Build Editor could overwrite the authority-selected
corrective field, and an invalid current custom draft prevented legal alternatives from explaining
the tradeoff that would repair it.

Remediation:

- the error UI root now retains the complete rendered `FlowError` as its presentation key and is
  rebuilt whenever that value changes, so a timeout followed by a late rate-limit outcome replaces
  stale Retry copy with the authoritative countdown and **Try Again** action;
- Build Editor initializes focus to the choice row only when navigation does not already name an
  editor control, preserving the corrective weapon, ultimate, or passive field selected while
  committing an authoritative rejection;
- alternative comparison now evaluates the legal candidate independently from the current draft,
  computes the invalid draft's exact point total with the canonical build rule, and prioritizes the
  changed category within the bounded eight-line explanation;
- focused regressions cover in-place error replacement, corrective focus across editor
  reconstruction, and a legal ultimate correction from an over-budget current draft.

Post-remediation verification passed `just check`, `just lint`, `just server-features`, and
`just test`, followed by `just network-product-queue-smoke`. The resulting matrix contains 343
client tests, 293 server tests, 81 serial separate-App/UDP network tests, 14 performance gates, and
79 routing unit tests plus its process/runtime/isolation suites. The remaining open gates are
unchanged: representative visual inspection, physical-controller playtest, and feedback triage.
M04 remains `User playtest` until those observations are recorded.

## Open user playtest record

Run `just network-product-queue-smoke` once for the bounded terminal authority check, then run
`just network-product-lobby` for the normal windowed product client. At 960×540, the default window
size, and one 16:10 or ultrawide size:

1. From Title, choose Play, connect to `127.0.0.1:5000`, select each advertised game card, and
   confirm its population reads `N waiting · M players per match` rather than a wait estimate.
2. Open **Build & Join**. With mouse/keyboard, inspect all four presets, then Custom; open each of
   Power, Reach, Magazine, Ultimate, Passive 1, and Passive 2, and verify every option shows a cost
   plus understandable changed lines. Create one incompatible and one over-budget draft, then
   correct each without losing the draft.
3. Join with a preset. Confirm Queue shows the accepted point total and a fresh aggregate count,
   Cancel Queue returns to Game Select without disconnecting, and a second Build Editor opening
   starts from the accepted build. Close and relaunch once to confirm that accepted build persists.
4. Repeat the ordinary select → edit → join → cancel flow using only a physical controller: D-pad
   navigates, South activates, and East backs out. Confirm visible focus remains on every transition
   and after disconnecting/reconnecting the controller.
5. While Join or Cancel is pending, confirm Disconnect remains immediately reachable. If a ten-second
   timeout or rate-limit response is exercised, confirm Retry preserves the frozen request while
   **Try Again** remains disabled until its visible countdown expires.
6. Check that no opponent names/builds, global queue position, ratio-like progress, or wait estimate
   appears anywhere.

Please record any clipped/unclear layout, lost focus, unexpected draft reset, misleading tradeoff,
or recovery failure. Each item will be triaged as implemented now, deferred, rejected with rationale,
or awaiting evidence before M04 is marked `Complete`.
