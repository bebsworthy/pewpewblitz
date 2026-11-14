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
