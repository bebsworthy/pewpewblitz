# V2 milestone 04 — Product build editor and authoritative queue admission

## Tracking

| Field | Value |
|---|---|
| Status | Specification review |
| Prepared | 2026-08-19; reconciled after M03 completion and user-approved review fixes |
| Objective | Let an authenticated lobby player compose a bounded build, submit it with one selected advertised game type, receive an authoritative immutable queue ticket, inspect honest aggregate pool state, and cancel without leaving the lobby |
| Entry dependency | Satisfied 2026-08-19: M03 is complete; its delivered lobby catalog, client flow, overlay, persistence-error, and session-loss seams are reconciled below |
| Scope authority | Specification review only; production implementation remains unauthorized until the user validates this specification |

M04 research began while M03 was implementing. M03 is now complete and M04 is the current
specification-review milestone. The decisions below incorporate the user-approved plan review, but
they do not authorize production code changes until the user validates implementation.

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
   counts as current.
8. Ordered-reliable transport retains an outcome until transport acknowledgement and exposes no
   application queue-cap setting. M04 therefore allows only one application-unacknowledged queue
   outcome per session, requires a small client acknowledgement after consuming it, suppresses a
   second wire copy for an unacknowledged identical retry, and rate-limits accepted commands. The
   first over-rate command receives one bounded fail-soft notice; continued commands inside that
   notice window are the protocol-abuse boundary.

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
| Delivery | One client-to-server `QueueClientMessage` envelope carries commands and outcome acknowledgements in their actual ordered-reliable `SessionChannel` order; outcomes use the same channel server-to-client. Each session may have at most one application-unacknowledged queue outcome and at most 512 encoded outcome bytes retained for it. Complete snapshots use one server-to-client `QueueSnapshotChannel` with sequenced-unreliable delivery, local unsent retry disabled, immediate publication after visible mutation, and a one-second refresh; no polling or HTTP side path |
| Command abuse bound | Per authenticated session, a token bucket admits a burst of four new semantic queue commands and refills one token per second. An identical same-ID retry while its outcome remains application-unacknowledged is idempotent recovery: it consumes no token, causes no second wire copy, and leaves the retained outcome unchanged. The first new semantic command beyond the bucket, when no outcome is pending, receives one bounded `RateLimited` outcome with a retry delay and starts one notice window; the client retains its context, disables **Try Again** until that delay expires, then uses a new request ID. Any further command during that notice window, or any different command while an outcome remains application-unacknowledged, is protocol abuse: emit no additional reliable outcome, disconnect, and use normal queue-aware teardown |
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
  on disconnect, one application-unacknowledged outcome per session, bounded command rate, and no
  hidden auto-requeue;
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
  admission_order (not displayed as a global rank)
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

The adapter decodes at most four `QueueClientMessage` inputs—commands plus acknowledgements—per
authenticated session in one application update, or 128 across the 32-session lobby. A per-session
token bucket also admits a burst of four new semantic commands and refills one token per elapsed
real second. After structural/request-identity validation, an identical same-ID retry for the
application-unacknowledged outcome is recognized before token consumption: it consumes no token,
does not enqueue another wire copy, and relies on Lightyear's retained reliable send. Every other
command consumes a token before admission/idempotency mutation. Exceeding
the per-update decode bound is a protocol-abuse failure and disconnects the sender without generating
one reliable outcome per excess input. The first over-token command, when no outcome is pending,
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
one semantic command in flight, while same-update identical duplicates remain testable.

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

1. observe authenticated session loss and drain each session's ordered `QueueClientMessage` stream
   after Lightyear receive;
2. discard all queue inputs for sessions lost in that same update and remove their existing tickets;
3. preserve the envelope order directly, apply a matching acknowledgement before a following
   command, recognize a pending identical retry before token consumption, enforce the per-update and
   token bounds, then group admitted commands by stable authenticated player ID instead of Bevy
   query order;
4. reject a different command while an outcome remains unacknowledged, validate joins completely
   before mutation, and apply cancels exactly once;
5. commit each successful mutation with its pre-reserved public state revision;
6. queue at most one exact reliable outcome per session, then publish the newest complete sequenced
   snapshot if aggregate state changed.

Equal request ID plus identical command returns the cached semantic outcome without mutation; the
adapter emits it only when no copy for that request remains application-unacknowledged. Equal ID plus
different command is a protocol violation. Lower IDs are stale. A newer Join while already queued
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
`state_revision` replaces the complete snapshot. An older snapshot is ignored; an identical replay
is ignored; the same revision with different content is a protocol failure. Revision gaps and loss
are safe because every message is complete and refreshes repeat the newest state. The client records
local monotonic receipt time for the newest valid snapshot. If none has arrived or that receipt is
more than three seconds old, Game Select and Queue show `Updating queue` rather than zero or an old
count. The next valid current-generation snapshot restores counts immediately. The three-second age
is presentation freshness only and never changes membership or authority.

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
disconnects, and initial/mutation/refresh snapshot publications requested. It
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
ObserveLobbyLifecycle
  -> CollectQueueClientMessages
  -> ReconcileDisconnectedSessions
  -> ApplyQueueTransactions
  -> PublishQueueOutcomesAndSnapshot
```

This runs after Lightyear's pinned `PreUpdate` receive work. Replace M03's direct session mutation in
both `lobby_cleanup_disconnected` and `On<Remove, LobbyClient>` with emission of one bounded,
deduplicated `LobbySessionLost` record carrying the stable lobby session and authenticated Netcode
identity. `ReconcileDisconnectedSessions` consumes those records, removes each owned ticket first,
then removes the lobby session through one helper that mutably owns both pure models. The lost-
session set remains visible to `ApplyQueueTransactions`, so a command collected in the same update
cannot recreate membership. Observer timing, deferred entity removal, system registration, and query
order therefore cannot bypass queue cleanup. Tests exercise both the polled marker and component-
removal observer paths.

Outcome acknowledgements clear only the matching per-session pending-output marker and never mutate
queue membership. The unified receiver is drained in order, so an acknowledgement followed by the
next command on ordered-reliable `SessionChannel` is valid in one update while the reverse order is
not silently rearranged. Queue mutation is immediate
resource mutation inside one pure transaction; deferred entity commands
are not used as the ticket commit boundary. Snapshot refresh timing uses elapsed real time for
presentation delivery only and never affects FIFO or authority.

The server feature graph may use authored build catalogs, weapon/fighter definitions, lobby wire
types, and the pure resolver. It must not acquire UI, clipboard, client persistence, rendering,
audio, device input, or client assets. The supervisor and routing package remain Bevy-, Lightyear-,
gameplay-, build-, and queue-free in M04.

## Planning slices

Implementation remains blocked on user validation of this specification.

### Slice 1 — Establish queue/build contracts on the reconciled M03 seams

- [x] Record M03's delivered catalog, selected-game, flow-action, session-generation, error,
  persistence, and dual session-loss seams in this specification.
- [x] Keep M04 queue-only and move reservation plus cancellation-versus-reservation races to M05.
- [ ] Define bounded shared build candidate/accepted summary and queue IDs/messages without
  duplicating authored definitions or moving authority client-side.
- [ ] Increment the one global compatibility version and register current messages in exact
  directions on ordered-reliable `SessionChannel` and sequenced-unreliable
  `QueueSnapshotChannel`; retain no old decoder.
- [ ] Add pure serialization, decode-bound, identity, revision, and resolver-agreement tests.

### Slice 2 — Deliver authoritative admission before product UI expansion

- [ ] Add the pure bounded lobby queue model with one ticket per session, deterministic FIFO,
  immutable resolved loadout, exact indexes, pre-reserved public revisions, idempotent Join/Cancel,
  admission-revision ownership, one pending outcome per session, the four-token/one-per-second
  limiter with one fail-soft notice window, and queue-aware disconnect cleanup.
- [ ] Add explicit lobby schedule sets and one deduplicated session-loss seam over real
  authenticated M03 sessions and both existing teardown paths.
- [ ] Publish initial, post-mutation, and one-second refresh snapshots with bounded sequenced
  delivery, three-second client freshness aging, and required-revision barriers on membership
  outcomes.
- [ ] Record bounded privacy-safe queue counters/high-water marks and include their final aggregate
  in headless process evidence without identity-bearing history.
- [ ] Add an in-memory authenticated flow proving welcome → snapshot → join accepted → queued →
  cancel accepted → Game Select state data, with no allocation request.
- [ ] Prove duplicate, stale, malformed, ID/revision exhaustion, maximum-capacity, outcome-ack,
  rate-abuse, stalled-client, and same-frame disconnect cases before UI work.

### Slice 3 — Replace the debug editor in the product lobby flow

- [ ] Add BuildEditor as the one overlay over Game Select and Queue as the one new flow state.
- [ ] Reuse M02/M03 focus, pointer, controller, scrolling, style, and action-arbitration seams.
- [ ] Present all four presets, six custom fields, per-choice costs, used/remaining budget, invalid
  reason, focused before/after changed lines, and meaningful family-specific resolved preview
  through the specified pure presentation/comparison helpers without raw debug formatting or
  aggregate DPS.
- [ ] Submit one frozen draft with selected catalog/game identity and map exact server rejection
  reasons to deterministic product copy and focus behavior in retained editor state.
- [ ] Preserve card Confirm as selection, add explicit **Build & Join**, and present aggregate pool
  state as `N waiting · M players per match` plus exact accepted membership on Queue.
- [ ] Add immediately reachable Join/Cancel Disconnect, ten-second pending deadlines, same-request
  Retry without duplicate wire outcomes, outcome acknowledgement, and late-outcome precedence.

### Slice 4 — Add last-used persistence and recovery

- [ ] Add bounded `BuildFileV1`, missing/default/stale/malformed handling, atomic save, and focused
  temporary-directory tests.
- [ ] Load valid last-used state into a fresh editor and save only an authoritative acceptance.
- [ ] Reconcile the single Error overlay with three local persistence sources and one dirty build
  save context.
- [ ] Prove save failure cannot cancel or alter an accepted ticket and Retry Save is repeatable.

### Slice 5 — Verify and hand off

- [ ] Run focused pure/ECS/client/server/protocol/network/process tests, role-feature checks,
  formatting, and Clippy through canonical `justfile` commands.
- [ ] Preserve the M01 transition smoke and direct-UDP match build-selection baseline on the current
  global protocol.
- [ ] Add `just network-product-queue-smoke` as the canonical bounded headless product path for
  welcome → snapshot → admission → refreshed count → cancellation → refreshed count → exit.
- [ ] Capture representative Game Select population, Build Editor, rejection, Queue, and save-error
  layouts.
- [ ] Provide keyboard/mouse and physical-controller playtest steps and record feedback.

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
  enqueues no second copy, including five or more duplicates across bounded receive batches without
  advancing refill time; more than four total client envelopes in one update still crosses the
  independent decode-abuse bound; the first over-rate new semantic command with no pending outcome
  returns one bounded `RateLimited` notice without queue mutation or disconnect,
  same-ID replay remains the same rejection, a compliant new-ID attempt after its delay succeeds,
  and another command during the notice window disconnects without an outcome flood;
- polled `Disconnected` and `On<Remove, LobbyClient>` each pass through the same loss record and
  remove one ticket before its session; duplicate observation removes nothing else; disconnect plus
  Join in one update creates none;
- every mutation leaves pool/session/ticket indexes bijective and within the 32-session/eight-pool
  bounds; all 32 sessions may queue in one pool and no unreachable `QueueFull` outcome exists;
- complete snapshot revision increments only for public count changes and preserves catalog order,
  exact count, and formation size; fresh Joined membership records its admission revision, a no-op
  equivalent re-Join reuses that revision, and Cancelled names the revision created by removal;
- every queue outcome encodes within 512 bytes; at most one unacknowledged outcome per session and
  32/16 KiB process-wide queue outcomes/payload can be retained by the application adapter;
- queue diagnostics saturate safely, current/high-water ticket and pending-outcome values converge
  through admission/cancellation/disconnect, rejection/snapshot counters follow exact typed facts,
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
  over-bound input during decode;
- compatible welcome schedules one valid full snapshot; several consecutive dropped refreshes age
  counts to `Updating queue`, the next delivered full refresh restores them, older/duplicate
  snapshots are ignored, and conflicting same-revision content fails;
- outcome and snapshot may arrive in either order; Joined/Cancelled transitions use only the
  outcome, and affected counts remain `Updating queue` until a snapshot at or above the outcome's
  required revision arrives;
- two clients join the same pool in deterministic order, one cancels, the other remains, and both
  observe the same next aggregate revision;
- clients in different pools never receive each other's membership/build details but observe the
  same aggregate rows;
- 32-session/ticket bounds, malicious duplicate commands, missing outcome acknowledgement, a
  fail-soft first rate-limit notice, continued rate abuse, a stalled client, sustained queue churn,
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
- [ ] The user validates the final M04 specification before implementation begins.
- [ ] Build Editor supports every current preset/custom choice with honest budget, preview, precise
  invalid feedback, visible option costs and meaningful tradeoffs, and controller/keyboard/mouse
  operation.
- [ ] Last server-accepted build survives restart when valid and missing/malformed/stale/save-failed
  local data fails safely without altering queue authority.
- [ ] The lobby atomically validates selected game/catalog/build identity and creates at most one
  immutable ticket per authenticated session.
- [ ] Queue requests/cancellation are idempotent and safe under duplicates, stale IDs, overflow,
  same-frame disconnect, timeout/retry, and ID/order/revision exhaustion; reservation races remain
  explicitly owned by M05. A newer equivalent Join reuses the existing ticket and admission revision
  without consuming another public revision.
- [ ] Game Select and Queue show revisioned real aggregate counts and exact formation size without
  revealing private membership or inventing an estimate; membership-changing outcomes provide an
  explicit freshness barrier, stale snapshots age out after three seconds, and any later delivered
  current snapshot restores the display.
- [ ] Cancel Queue returns to Game Select without disconnecting; unexpected loss removes server
  membership and follows fresh-session recovery; Disconnect remains immediately reachable while
  either Join or Cancel is pending.
- [ ] Command and snapshot delivery remain bounded under a stalled client and sustained churn;
  at most one application queue outcome of at most 512 bytes is unacknowledged per session, abuse
  cannot create an outcome flood, the first over-rate new semantic command fails softly with one
  bounded notice, stale snapshots age out, and complete snapshots never accumulate in an ordered-
  reliable history.
- [ ] Privacy-safe bounded diagnostics expose queue current/high-water state and typed aggregate
  outcomes needed to diagnose admission, cleanup, abuse, and snapshot publication without retaining
  player, ticket, request, build, capability, or address identity.
- [ ] M04 product composition never allocates a worker; M01 transition and direct-match behavioral
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
  state while retaining the limiter for new semantic commands;
- replaced the nonexistent in-session catalog refresh with explicit incompatible-session recovery;
- added bounded privacy-safe queue diagnostics and headless process evidence;
- made Disconnect immediately reachable during Join as well as Cancel;
- expanded build presentation and playtest evidence so costs and gameplay tradeoffs are understandable
  through focused before/after changes without misleading aggregate DPS;
- removed duplicate preset-origin identity from the accepted summary, defined admission-revision
  reuse for a newer equivalent Join, and mapped correctable build rejections to deterministic product
  copy and focus behavior.
