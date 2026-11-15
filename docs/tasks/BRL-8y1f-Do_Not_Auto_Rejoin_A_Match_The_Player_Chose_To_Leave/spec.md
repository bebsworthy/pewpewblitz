# Found
Device check 2026-11-15, Pixel 8a, HEAD 6a6d6599 (protocol 48), scratch supervisor on 0.0.0.0:5001. The BRL-094c device walk (protocol 47) left the match normally, so BRL-yyeg introduced this.

# Repro
1. Start Practice Zombie (3 players). Any running match will do.
2. Match menu (Back or the HUD button) -> LEAVE MATCH -> LEAVE MATCH.

Expected: the Dashboard.
Actual: the client starts a fresh lobby session. The lobby still holds the seat (`seat_hold`: "match seat held; a server bot controls the fighter", hold 60 s), so it sends `Rejoining` and a grant. The client rejoins the same match and fighter about 1.5 s later (`brawler client rejoined its running match player_id=10`) and shows the match menu again.
The 1st, 2nd and 3rd leave all rejoined. Only the 4th reached the Dashboard, once `MAX_REJOINS_PER_SEAT = 3` was used up.

Server log, 09:22: seat held 27.32 -> rejoin requested 27.57 -> rebind accepted -> grant delivered -> "seat reclaimed by a rejoined session" 27.86. The same again at 41.2.

# Fix direction
- A deliberate leave must release the seat. For example, the match worker treats a client-initiated leave as a departure (today's forfeit/remove rules) rather than a lost link, and the supervisor/lobby record it as terminal, so no rejoin is offered. Alternatively the client sends a leave intent the lobby honours. Keep the BRL-yyeg behaviour for real connection loss and app kills.
- The 3-rejoin limit should not be the only thing that lets a player leave.

# Acceptance
- A routed/network test: leave via the product leave path -> no RejoinRequest and no rebind; the fighter is removed as before BRL-yyeg.
- Pixel 8a: Leave Match -> confirm reaches the Dashboard on the first try, in Practice and PvP.

Evidence: /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/dc/l02.jpg-l07.jpg, s-l04.jpg, s-l06.jpg, server.log (09:20-09:23).


# Design (2026-11-15, before implementation)

Root cause: since BRL-yyeg a routed fighter has a `Persistent` lifetime and `hold_released_seats`
holds every seat whose link is lost in `Active`. Leave Match only disconnects the match session
(`request_return_to_lobby`), and Netcode reports a requested disconnect and a timeout identically
on the server (`DisconnectedReason::ByPeer(None)` from the same `disconnections` drain), so the
worker cannot tell a leave from a loss. The lobby seat table then offers the held seat to the next
lobby admission of the same account, until `MAX_REJOINS_PER_SEAT` is used up.

Decision: the voluntary signal is a client intent to the authority that owns the seat (the match
worker), and the worker reports the release up the control plane.
- Wire: `MatchCommand::LeaveMatch` (existing `MatchCommandRequest`/`MatchCommandOutcome` on
  `SessionChannel`). Application protocol 48 -> 49.
- Client: confirming Leave Match sends `LeaveMatch` and disconnects when its outcome arrives, or
  after a bounded 2 s wait (a lost link then falls back to today's hold).
- Match worker: an accepted `LeaveMatch` marks the participant's routed seat as left. When its
  session then ends, the fighter is removed in every phase (today's departure/forfeit rules, as
  before BRL-yyeg); a left seat is never held and refuses rebinds (`SeatExpired`, terminal).
  The worker sends one `SeatReleased` fact per left seat.
- Control IPC 6 -> 7: `SeatReleased {request_id, allocation_id, match_id, player_id}` (48 bytes)
  match worker -> supervisor (validated against that worker's allocation) -> owning lobby. The
  supervisor then refuses rejoins of that seat (`SeatExpired`); the lobby drops the account's seat
  (an in-flight rejoin is answered `Unavailable`), so no `Rejoining` is announced.
- Forfeit: leaving is a departure. The fighter is removed at once; a two-team match whose team is
  now empty forfeits exactly as a pre-BRL-yyeg disconnect did; Practice teams keep their bots.
- Involuntary loss (no `LeaveMatch`) keeps the 60 s hold and automatic rejoin unchanged.


# Implementation (2026-11-15)
- Wire: `MatchCommand::LeaveMatch`; `SUPPORTED_PROTOCOL_VERSION` 48 -> 49. Registered types are
  unchanged, so the routing identity stays network_protocol 4778953997791089217,
  protocol_registry_fingerprint 15212284317352318216, content_fingerprint 7772007507332977560.
- Client: `RoutedClientLifecycle::request_leave_match` (ConfirmLeaveMatch);
  `session/match_commands.rs::drive_match_leave` sends `LeaveMatch` and calls
  `request_return_to_lobby` on its outcome or after `MATCH_LEAVE_ANSWER_TIMEOUT` (2 s).
- Match worker: `process_match_commands` accepts `LeaveMatch` in every phase and marks the fighter
  `seat_hold::LeftSeat`. `release_left_seats` records the leave in `ParticipantRebinds` and queues
  one `SeatReleasedBody`; `hold_released_seats` removes a left fighter when its session ends (no
  hold, any phase); `rebind_refusal` answers `SeatExpired` for a left seat; the worker control
  owner drains the release facts.
- Control IPC 6 -> 7: `ControlType::SeatReleased = 27`, `SeatReleasedBody` (48 bytes).
  Supervisor `runtime/rejoin.rs::release_seat` validates the worker/allocation/participant, marks
  `SeatRejoins::released` (later rejoins -> `SeatExpired`) and forwards once to the lobby. Lobby
  `MatchSeatTable::release_seat` forgets the seat and answers an in-flight rejoin `Unavailable`.
- Docs: docs/14 (Rejoining a running match), docs/08 (protocol 49 / control 7), docs/13 (Leave
  Match confirmation).

# Verification (2026-11-15)
- New tests, each shown failing with the fix disabled: `tests/network/match_rejoin.rs`
  `leaving_on_purpose_releases_the_seat_instead_of_holding_it` (product leave path via the client
  lifecycle: one release, no hold, fighter removed, 1v1 forfeit, rebind `SeatExpired`) and
  `a_seat_its_player_left_refuses_a_racing_rebind`; unit tests for the client leave step machine,
  `rebind_refusal`, supervisor `a_seat_released_by_a_voluntary_leave_is_never_granted_again`, lobby
  `a_voluntarily_released_seat_is_forgotten`, and the `SeatReleased` codec round trip.
- `just check`, `just lint`, full `just test`: pass. `just e2e 2`: pass. `just rejoin-e2e`: pass
  (involuntary loss still rejoins).
- Pixel 8a: not run. The phone was not attached to adb during this session (no USB/mDNS device);
  the fixed APK is built (`scripts/android.sh build 192.168.31.211:5000`) but not installed.
  Device check still owed: Leave Match -> confirm reaches Dashboard on the first try in Practice
  and PvP, against the :5000 server on this HEAD.

# Learn from errors
- BRL-yyeg changed what a disconnect means (Persistent lifetime + hold) without re-checking every
  caller that disconnects on purpose. Netcode cannot tell a requested disconnect from a timeout on
  the server, so any "this was deliberate" fact must be an explicit, acknowledged message.
  Prevention: when a change reinterprets a transport event, list every producer of that event
  (here: Leave Match, match completion, rejected rejoin, loading cancel) and test each one.
- The rejoin tests covered only the loss path; a negative test (a deliberate leave must NOT
  rejoin) would have caught this before the device run.
