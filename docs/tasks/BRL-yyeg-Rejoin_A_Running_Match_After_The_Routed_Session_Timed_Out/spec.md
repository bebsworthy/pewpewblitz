# Outcome
A lost match session can be re-established while the match still runs.

# Context (from BRL-g57a, 2026-11-14)
BRL-g57a keeps the same Netcode session alive through gaps up to the 10 s routed session bound (capability idle revocation). Beyond that the client returns to a fresh lobby and Dashboard. Rejoin is not supported today: the match worker rejects hellos once the match left Waiting (MatchInProgress); admission allows one live connection per manifest participant; the capability/route is revoked on idle or PeerClose; the manifest binds each participant to one PeerId.

# Scope
- Lobby recognises an account whose match is still running and requests a fresh capability for that participant.
- Supervisor control IPC to mint a capability for an existing worker route/peer (or a re-bindable peer), with expiry and rate limits.
- Match worker re-admits an in-progress manifest participant and re-attaches control of the existing fighter (or a defined respawn).
- Client: on match-session loss, prefer the rejoin grant over Dashboard.

# Acceptance criteria
- A routed-process test drops a client longer than 10 s mid-match and rejoins the same match and fighter.
- Server authority and manifest admission remain the only identity checks.

# Notes
Needs control-IPC and manifest contract changes; validate the architecture decision first (docs/14).

# Research and design (2026-11-15, before implementation)

Implementation is paused. Three choices below exceed the spec and the owner's direction; they are
open Ticket questions. No code, wire, IPC or manifest change has been made yet.

## Findings (code at 741a1836)
- **The fighter is gone the moment the session ends.** `commit_fighter_join` gives the fighter
  `ControlledBy { lifetime: SessionBased }`, so Lightyear despawns it when the match connection
  disconnects. There is nothing to "re-attach" today.
- **A lone player's disconnect ends the match at once.** `refresh_match_roster` counts only
  fighters whose owner link is connected (bots always count). `resolve_pregame_outcomes` commits
  `Forfeit` as soon as a team's count is 0 (`forfeit_result`, two-team modes). 1v1 PvP, and any
  team whose other members already left, therefore forfeit on the same tick. Practice teams with
  bots, and 2v2/3v3 with teammates still connected, keep running.
- **The lobby forgets a match at activation.** `apply_product_activation` completes the
  reservation and clears `ProductFormationState`; `LobbyState` keeps no allocation, worker, seat or
  account record afterwards. The lobby receives no control frame when a match ends (only the
  supervisor sees `Result`/`Exit`).
- **The account is known only in the lobby.** `ProfileAuthority` maps `AccountId` to the lobby
  session. The manifest carries `lobby_session_id` (dead after handoff), `player_id`,
  `netcode_client_id` and `peer_id`, and by contract (docs/14) no account.
- **Match admission binds both IDs.** `admit_manifest_client` requires the authenticated Netcode
  client ID *and* the routed `PeerId` to equal one manifest participant. `validate_match_hello`
  rejects every hello outside `Waiting` (`MatchInProgress`). A relaunched app has a fresh random
  Netcode client ID; a same-process network loss keeps its ID.
- **Routes are not reusable.** Idle (10 s) or `PeerClose` revokes the capability and
  `teardown_route_with_reason` removes the route and its queues. Capabilities are minted only in
  `prepare_allocation`/allocation commit, at most 2 live per lobby session.
- The client flow assumes a match is joined through Loading/Countdown (`MatchLoadingGate`
  check-in). A mid-match join must skip the loading check-in and countdown.

## Proposed architecture (recommended; awaiting answers)
1. **Seat hold in the match worker.** Fighters of routed participants get a server-owned lifetime
   (not `SessionBased`). When the owner link disconnects in `Active`, the seat becomes *held*: the
   fighter stays, its input becomes neutral, and it still counts toward the team for forfeit until
   the hold expires. At expiry the fighter is removed and today's departure/forfeit rules apply.
   Waiting/Loading/Countdown keep today's behavior (a countdown departure still fails the start).
2. **Lobby seat table.** At grant delivery the lobby records, per account: allocation, match,
   match worker, participant `player_id`, and the seat's slot. It is bounded (one seat per account,
   capacity = match slots × max participants) and released on a new supervisor→lobby `MatchEnded`
   fact (worker Result/Exit/failure) or at hold expiry. The account never leaves the lobby.
3. **Rejoin request.** When an account with a held seat completes lobby admission, the lobby sends
   `RejoinRequest {seat, current netcode_client_id}` to the supervisor. The supervisor registers a
   fresh route and fresh `PeerId` for the same worker, mints a fresh 256-bit capability (pending
   30 s, idle 10 s as today) and sends `ParticipantRebind {player_id, new peer_id, new
   netcode_client_id}` to the worker. The worker accepts it only for a held seat, replies
   `ParticipantRebound`, and the supervisor sends `RejoinGranted` to the lobby. Old routes and
   capabilities stay revoked. The immutable manifest is unchanged; the worker keeps a runtime
   rebind overlay on top of it.
4. **Re-admission.** `validate_match_hello` accepts a hello from the rebound (Netcode ID, PeerId)
   pair in `Active` (and `Completed`, so the result screen still appears) and the commit re-attaches
   `ControlledBy` to the existing fighter. `MatchJoinOutcome::Accepted` returns the same
   `player_id`/`network_entity_id`; the loading gate is not required for a rejoin.
5. **Client.** After a match-session loss the client already starts a fresh lobby session. On
   `RejoinAvailable` (a new lobby phase carrying the grant) it connects to the match directly, in
   Active presentation, instead of committing Dashboard. If the grant fails, today's Dashboard
   message stays.
6. **Wire and IPC.** The lobby message is an application-protocol change:
   `SUPPORTED_PROTOCOL_VERSION` 47 → 48 through the one global handshake (registry fingerprint
   changes; every client and the user's server must be rebuilt and restarted). Control IPC gains
   `RejoinRequest`, `RejoinGranted`/`RejoinRejected`, `ParticipantRebind`/`ParticipantRebound` and
   `MatchEnded`; `CONTROL_VERSION_CURRENT` 5 → 6. The match-manifest schema is unchanged.
7. **Verification plan.** Pure tests for the seat hold, the rebind admission and the lobby seat
   table; supervisor tests for rejoin minting and rate limits; separate-App network scenarios
   (`tests/network/match_rejoin.rs`, harness): a client disconnected for more than 10 s mid-match
   rejoins the same fighter, a hold that expires removes the fighter, a different account cannot
   rejoin; a routed-process smoke in a scratch copy of `scripts/e2e-network-change.sh` with an
   outage longer than 10 s.

## Open decisions (Ticket questions)
- Q1 How long a seat is held, and what happens to a lone player's team meanwhile.
- Q2 What the fighter does while its player is disconnected.
- Q3 Security and rate limits of the rejoin grant, and whether a relaunched app may rejoin.
- Q4 Whether rejoin is automatic or offered as a button.

# Owner decisions and implementation design (2026-11-15)

The four questions are answered; this section supersedes the "Proposed architecture" where they
differ.

- **Q1 seat hold (q-7ay64v4):** a routed participant's seat is held 60 s
  (`SEAT_HOLD_DURATION`, code-owned session policy beside the 10 s routed bound) after its match
  session ends while the match is `Active`. The held fighter counts for its team, so a lone
  player's team does not forfeit while held. At expiry the fighter is removed and today's
  departure/forfeit rules apply. A departure in Waiting/Loading/Countdown keeps today's behavior
  (the fighter is removed and a countdown departure still fails the start).
- **Q2 held fighter (q-73cycrv), changes the earlier "stands idle" design:** while held, the
  existing server-hosted Practice bot policy (`src/bots/`, docs/10) controls the fighter, in PvP
  and Practice alike. The hold inserts a fresh `PracticeBotController` and neutral
  `ActionState<FighterInput>` and removes the stale human `NativeBuffer`; the bot therefore
  writes intent through the same authoritative `ActionState` + `InputFreshness` path that
  movement, combat and abilities already consume, with the same fair delayed perception (a fresh
  controller has no observation history, so it acts only after the profile's reaction delay).
  On rejoin the controller is removed, `ActionState` is reset to neutral and `ControlledBy` is
  re-attached to the new link, so the next player input is the only intent. The fighter keeps
  its identity, health, position, score credit and team throughout.
- **Q3 grant security (q-fggtj39):** only a lobby session authenticated with the same
  `AccountId` that held the seat at handoff may ask. The account stays in the lobby. The
  supervisor mints a fresh 256-bit capability on a fresh `RouteId`/`PeerId` (30 s pending, 10 s
  idle as today); earlier capabilities stay revoked. Supervisor limits per seat per match: at
  most one live grant (no live capability on any of the seat's routes and no rebind in flight)
  and at most 3 granted rejoins. The worker rebinds the seat to the new Netcode client ID, so a
  relaunched app (new Netcode ID, same account) may rejoin.
- **Q4 trigger (q-f2m2ptw):** automatic. The lobby tells the client `Rejoining` before the
  welcome, the client shows a short "Rejoining match" status, and on any failure it falls back
  to today's Dashboard message.

## Wire, IPC and manifest
- Application protocol 47 -> 48 through the one global handshake. New registered lobby->client
  message `MatchRejoinStatus { Rejoining, Unavailable }` on `SessionChannel` (registry
  fingerprint changes). The rejoin grant itself reuses `MatchRouteGrant`, so the existing client
  grant/unlink/match-connect path carries it. `MatchJoinOutcome` is unchanged.
- Control IPC 5 -> 6 (`CONTROL_VERSION_CURRENT`): `RejoinRequest` (lobby->supervisor),
  `ParticipantRebind` (supervisor->match), `ParticipantRebound` (match->supervisor, with an
  outcome code), `RejoinGranted`/`RejoinRejected` (supervisor->lobby) and `MatchEnded`
  (supervisor->lobby, `ActivationBody` shape, sent once when an activated allocation's Result
  arrives or its worker ends). The match manifest schema is unchanged; the worker keeps a
  runtime rebind overlay on top of the immutable manifest.

## Ownership
- Match worker: `src/server/seat_hold.rs` owns the hold, bot takeover, expiry and the rebind
  overlay; admission consults the overlay and admits a rebound held seat in `Active` only.
- Lobby: `src/server/lobby/rejoin.rs` owns the bounded seat table (one seat per account,
  released on `MatchEnded`, `ActivationDissolved` or a terminal rejection) and the request/grant
  forwarding.
- Supervisor: `packages/brawler-routing/src/runtime/rejoin.rs` owns validation, limits, minting
  and `MatchEnded`.
- Client: the session lifecycle owns the rejoin attempt; the flow keeps the Match screen with a
  "Rejoining match" status and otherwise reuses the existing grant path.

# Implementation (2026-11-15)

- **Routing crate (Bevy-free).** `control.rs`: `ControlType` 21-26 (`RejoinRequest`,
  `ParticipantRebind`, `ParticipantRebound`, `RejoinGranted`, `RejoinRejected`, `MatchEnded`)
  with fixed body lengths (80/40/34/152/10/40), a closed `RejoinReason` (terminal vs transient),
  shared grant encoding and redacted Debug. `CONTROL_VERSION_CURRENT` 5 -> 6;
  `MAX_REJOINS_PER_SEAT = 3` in `limits.rs`. `runtime/rejoin.rs` owns per-seat state on each
  `AllocationParticipant` (granted count, granted routes, one in-flight rebind), admission (owning
  lobby, activated and not ended, known player, no rebind in flight, < 3 grants, no live capability
  on any seat route), minting a fresh `Capability`/`RouteId`/`PeerId`, binding the route only after
  the worker accepts, and `MatchEnded` (once, at Result or at worker reclaim; refuses in-flight
  rejoins). `SupervisorCore::has_live_capability_for_route`. BRL-sqz0's capability registry is
  untouched: a rejoin capability is an ordinary new record bound to the new lobby session.
- **Match worker.** `src/server/seat_hold.rs`: routed fighters get `ControlledBy { Persistent }`
  plus `RoutedSeat`. Before `MatchSet::Lifecycle` a lost owner link in `Active` inserts `SeatHold`
  (60 s), a fresh `PracticeBotController`, neutral `ActionState` and removes the stale
  `NativeBuffer`; in any other phase the fighter is removed as before. Expiry removes the fighter
  (today's departure/forfeit). `ParticipantRebinds` is the runtime overlay on the immutable
  manifest and also enforces the per-seat limit; `SeatRebindQueue` is fed and drained by the
  worker control owner. The roster counts a held seat and treats a routed seat without a live
  link as departed. Admission (`admit_rebound_client`, `MatchHelloValidationContext::rejoinable`)
  admits mid-match only a rebound held seat under its fresh (Netcode client, peer) pair;
  `ServerHelloContent::join` reclaims the existing fighter (`reclaim_held_seat`).
- **Lobby.** `src/server/lobby/rejoin.rs`: bounded `MatchSeatTable` (one seat per `AccountId`,
  `MAX_WORKERS * MAX_PARTICIPANTS`), recorded at `BeginMatchConnect`, released on `MatchEnded` or
  a terminal refusal. At welcome the same account gets `MatchRejoinStatus::Rejoining` (before the
  welcome) and a `RejoinRequest`; a `SeatConnected`/`Busy` refusal is retried every 500 ms for up
  to 12 s (the earlier session is still inside its 10 s bound); an unanswered request is reported
  unavailable after the window. Grants are forwarded as an ordinary `MatchRouteGrant`.
  `ProfileAuthority::account_for_client`.
- **Protocol.** `SUPPORTED_PROTOCOL_VERSION` 47 -> 48; new registered `MatchRejoinStatus`
  message (registry fingerprint 18160765825630381060 -> 15212284317352318216; network protocol and
  content fingerprint unchanged). Manifest schema unchanged.
- **Client.** `RoutedClientLifecycle::rejoin` (`AwaitingGrant` -> `Connecting` -> cleared on the
  accepted match join, `rejoined` one-shot); `process_match_rejoin_status` runs before the join
  outcome. The flow keeps the Match screen with a "Rejoining match…" label
  (`flow/screens/rejoin_status.rs`), moves a lobby screen (relaunched app) to Match
  (`RejoinStarted`), clears the loss notice on `MatchRejoined`, and otherwise falls back to the
  existing "The connection to the match was lost, and the match could not be resumed." Dashboard
  message. A refused rejoin hello returns to a fresh lobby instead of exiting.
- **Automation.** `--product-rejoin-smoke`, `BRAWLER_PRODUCT_REJOIN_SMOKE`, `scripts/e2e-rejoin.sh`,
  `just rejoin-e2e`.
- **Docs.** docs/14 "Rejoining a running match" (and the continuity/relaunch paragraphs that said
  rejoin was out of scope); docs/08 protocol 48 / control 6.

Design refinement beyond the original sketch (within the answers): the lobby retries a
not-yet-released seat for up to 12 s. Without it, a client that notices the loss a few ms before
the supervisor/worker release the old session (all sides use the same 10 s bound) would always
fail; limits and security are unchanged.

# Verification (2026-11-15)

All builds used `CARGO_TARGET_DIR` under the session scratchpad; `target/debug/brawler-server`
and the user's server on UDP 5000 were not touched. E2E runs used patched scratch copies of the
scripts with an ephemeral `BRAWLER_ROUTED_BIND`, the scratch target binaries and a scratch
`--data-directory` (never `data/server/`).

- Focused: `cargo test -p brawler-routing` (control round-trip/lengths, `runtime::rejoin` seat
  admission, `runtime::tests::rejoin_minting` fresh route/peer/capability up to the limit,
  refusals, `MatchEnded` once); `cargo test -p brawler --no-default-features --features server
  --lib -- lobby::rejoin seat_hold admission` (seat table, retries, bounds; seat hold, rebind
  refusal and limit; rebound-seat admission) - pass.
- `tests/network/match_rejoin.rs` (4 separate-App scenarios on the production match-worker graph):
  a session lost beyond the timeout is bot-held (fresh bot intent, no forfeit) and a relaunched
  identity reclaims the same fighter and moves it; an expired hold removes the fighter and
  forfeits the lone team and refuses a later rebind; another identity (own route, the replaced
  manifest identity, or the rebound route with another Netcode ID) cannot reclaim, and a
  connected seat is not rebound; three rejoins succeed and the fourth is `LimitReached` - pass.
- Routed-process smoke: scratch copy of `scripts/e2e-network-change.sh` with a >10 s outage
  (`BRAWLER_PRODUCT_REJOIN_SMOKE=1`) - pass: seat held at loss, `Rejoining`, rebind accepted, grant
  delivered, same player_id rejoined the same match, ticks resumed (about 0.5 s from loss to
  rejoin).
- Scratch `e2e 2` - pass; scratch `practice-e2e wipeout-1v1` - pass; scratch short-outage
  `network-change-e2e wipeout-1v1` - pass.
- `just check` - pass; `just lint` (VS Code ripgrep first on PATH) - pass; full `just test` after
  the last edit - pass (routing 92, server lib 538, client lib 588 + 800, network 119 incl. 4 `match_rejoin`, balance-lab, performance; 0 failures).

# Playtest note

Needs a native check by the owner after rebuilding and restarting every role (protocol 48 /
control 6; old clients are rejected at the handshake). Scenario: start a 1v1 PvP or Practice
match, then cut the phone's network (airplane mode) or kill the app for 12-50 s and restore it /
relaunch. Expected: "Rejoining match…" for about a second (up to 12 s after a quick relaunch),
then the same fighter, under your control, in the same match; meanwhile the fighter was moved by
a bot and your team did not forfeit. After 60 s away: the fighter is gone and today's forfeit /
Dashboard message applies. Observe: banner readability, camera/HUD after the rejoin, and whether
the bot's behaviour while held feels fair. Known limitations: a match that completes while you
are away shows Dashboard, not Results (BRL-jxfy); nobody is told the seat is bot-controlled
(BRL-dwqx).

# Learn-from-errors

- *Mistake:* released seats on `ActivationDissolved`; the first routed smoke showed the seat
  released 60 ms after handoff. *Cause:* a Practice lobby session that closes at its own handoff
  cancels the activation while the match keeps running (pre-existing, filed as BRL-5vc1).
  *Prevention:* bind lifecycle bookkeeping to the authoritative end fact (`MatchEnded`), not to a
  cancellation echo; run the routed smoke before trusting unit tests of cross-process lifecycle.
- *Mistake:* the network scenarios assumed the 3 s harness timeout; the match-worker graph uses
  the 10 s routed bound. *Prevention:* derive waits from the graph under test, not from a sibling
  harness.
- *Risk found by design review:* all parties release a lost session at about the same 10 s, so a
  rejoin issued at the client's loss can race the release; the bounded lobby retry removes the
  race without loosening any limit.
- Reusable: when a wire change needs a fingerprint change, register a message rather than editing
  a variant, and record before/after `routing-identity` from the old and new server binaries.

# Device check 2026-11-15 (Pixel 8a)
Setup: Pixel 8a (45171JEKB03025, Android 17), release APK built from HEAD 6a6d6599 and a scratch protocol-48 supervisor on 0.0.0.0:5001 over Wi-Fi. Evidence is under /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/dc/.
- **Kill-and-relaunch rejoin: pass.** In a running PvP Wipeout 1v1 (phone vs a headless client whose seat was already bot-held):
  - 09:13:56 `am force-stop`.
  - 09:14:06 server "match seat held; a server bot controls the fighter player_id=4 hold_seconds=60" (10 s routed bound).
  - Waited about 15 s, relaunched with `am start`, tapped CONNECT.
  - 09:14:22 "lobby requested a match rejoin for a held seat" -> worker rebind `refusal=None` -> "delivered a match rejoin grant".
  - 09:14:23 "match seat reclaimed by a rejoined session player_id=4".
  - Client: "brawler client is rejoining its running match", then "rejoined its running match player_id=4".
  - The phone showed the same match (score 2-1 and timer 2:06 continuing), the same fighter with its own name plate, and touch control. Outage about 26 s, inside the 60 s hold. Screens: `s-r02.jpg`, `r04.jpg`. After the opponent's hold expired, Results showed "Victory by forfeit" (`r05.jpg`).
  - The "Rejoining match…" banner was not caught; screencap takes ~1-2 s and the rejoin took about 0.6 s.
- BRL-5vc1 did not interfere. The rejoin ran in PvP; a Practice rejoin also worked (below).
- **Defect: Leave Match is auto-rejoined.** In Practice Zombie, LEAVE MATCH -> confirm started a fresh lobby, and the lobby immediately re-granted the held seat. The 1st-3rd leaves all returned to the same match; only the 4th, after `MAX_REJOINS_PER_SEAT`, reached the Dashboard. Filed **BRL-8y1f**.
- **Defect, possibly related:** PvP queue -> match handoff leaves the phone on the Dashboard while it plays the match unseen. Filed **BRL-k0tr**.
- Not run: the Wi-Fi/airplane-mode variant (system network settings are left to the owner), and expiry after 60 s away for the phone's own seat (the opponent's seat expiry did forfeit correctly at 09:10:02 and 09:14:39).
