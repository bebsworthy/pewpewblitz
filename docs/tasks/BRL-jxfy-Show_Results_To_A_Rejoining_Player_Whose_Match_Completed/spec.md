# Outcome
A player whose match session was lost, and whose match completed while their seat was held,
sees that match's Results screen when they come back inside the 60 s hold. Today they see
Dashboard (with the "could not be resumed" message after a network gap, or nothing after a
relaunch).

# Current state (checked 2026-11-15 at HEAD 3437c0d8)
- **Worker ends the match the moment it completes.** `worker_control_emit_result`
  (`src/server/worker.rs:645-680`) sends `Result` on the first frame the root is `Completed`, and
  `worker_control_finish_shutdown` half-closes the packet stream as soon as it is drained
  (`src/server/worker.rs:1172-1193`). The supervisor tears down every route and capability and
  sends `Stop` when it has both Result and packet EOF (`maybe_complete_match_result`,
  `packages/brawler-routing/src/runtime/worker_lifecycle.rs:578-606`; drain cap
  `RESULT_PACKET_DRAIN_TIMEOUT` 2 s, `runtime/mod.rs:49`). Nothing replays the result after that.
- **Supervisor refuses rejoins after Result.** `admit_rejoin_request` returns
  `RejoinReason::MatchEnded` when `record.result` is set or the worker's `result_received` is set
  (`packages/brawler-routing/src/runtime/rejoin.rs:130-138`). `notify_match_ended`
  (`runtime/rejoin.rs:378-410`) refuses any in-flight rejoin and sends `MatchEnded` to the lobby;
  it is called from Result (`worker_lifecycle.rs:563-564`) and from worker reclaim
  (`worker_lifecycle.rs:375-378`).
- **Lobby forgets the seat on `MatchEnded`.** `MatchSeatTable::release_allocation`
  (`src/server/lobby/rejoin.rs:295-316`, dispatched at `rejoin.rs:358`) drops every seat of the
  allocation and answers a pending rejoin `Unavailable`. The next welcome of that account is
  therefore ordinary (`src/server/lobby/profile_transactions.rs:213-226` only sends `Rejoining`
  while a seat exists). docs/14 states the gap: "A seat whose match completes while its player is
  away is released; that player sees Dashboard, not Results."
- **Worker admits a rebound seat only while Active.** Holds start only in `Active`
  (`src/server/seat_hold.rs:285`); `answer_participant_rebinds` computes `phase_active` and
  `rebind_refusal` answers `SeatExpired` outside Active (`seat_hold.rs:343-370`);
  `validate_match_hello` admits a mid-match `MatchJoinSource::Rejoin` only in `Active`
  (`src/server/admission.rs:285-305`). A hold keeps counting in `Completed`
  (`expire_seat_holds`, `seat_hold.rs:318-333`), but the worker is gone within milliseconds.
- **Results are client-local, built from replication.** `observe_completed_match`
  (`packages/brawler-client/src/session/routing.rs:111-205`) copies the replicated Completed
  `MatchState` (+ `WipeoutState`, `HotZoneState`, `MatchClock`, `ObjectiveResult`, fighter teams)
  into `ClientMatchResultContext` (`packages/brawler-client/src/lib.rs:313-329`) and then asks the
  routed lifecycle to return to the lobby. `FreshLobbyReturn` goes to `ClientFlow::Results` when a
  context exists, otherwise Dashboard plus the "could not be resumed" error when `match_failure`
  is set (`packages/brawler-client/src/flow/reducer/match_flow.rs:71-104`). `MatchFailed` clears
  the context (`match_flow.rs:105-109`). There is no server message that carries a result to a
  client, and the supervisor's canonical Result bytes (`canonical_match_result`,
  `worker.rs:581-627`) hold only the outcome, not the score or Wipeout/objective details.
- **Client guards already exist.** A `Rejoining` status is ignored when a result context exists
  (`session/routing.rs:78-90`); `Unavailable` only abandons a rejoin in progress; a refused rejoin
  hello returns to a fresh lobby (`session/admission.rs:356-366`). A rejoined session reaches
  `ClientJoinPhase::Active` through the normal `MatchJoinOutcome::Accepted`
  (`admission.rs:344-354`), which `observe_completed_match` uses for the local player.
- Versions at HEAD: application protocol 49 (`src/protocol.rs:47`), control IPC 7
  (`packages/brawler-routing/src/limits.rs:7`). Tests: `tests/network/match_rejoin.rs` (7 scenarios,
  none reaches Completed); e2e `just rejoin-e2e` (`scripts/e2e-rejoin.sh`, one Practice client).

# Plan
Chosen design: **keep the match worker alive in `Completed` while a seat is still held, and admit
a rebound held seat in `Completed`** (the ticket's option). The rejoining client then receives the
real replicated completed state and the existing `observe_completed_match` -> Results path runs
unchanged, so Results keep full fidelity (score, Wipeout stats board, objective result).
Rejected alternative: retain a result summary in the lobby and send it in a new lobby message.
It needs an application-protocol bump and a new client path, and it can show only the outcome,
because the score and stats live only in the worker.

1. **Defer Result while a seat is held (match worker, `src/server/worker.rs` + `seat_hold.rs`).**
   `worker_control_emit_result` emits only when the root is `Completed` **and** no
   `ResultsHold` blocker remains:
   - a fighter with `SeatHold` (bounded by its own `expires_at_tick`, at most
     `SEAT_HOLD_DURATION_SECONDS` = 60 s after the loss; expiry keeps working in Completed);
   - a seat reclaimed while `Completed` whose link is still connected, bounded by a
     `COMPLETED_REJOIN_LINGER` of the routed session bound (10 s) after the reclaim, so the new
     session gets the full completed snapshot before the drain barrier closes the routes.
   With no held seat at completion (the normal case) Result goes out on the same frame as today.
   Worst case the worker lives about 70 s past completion; supervisor allocation/route bounds are
   unchanged. Add the worker log line "match completed; result deferred for held seats" with
   `held` and the deadline tick.
2. **Release seats that cannot rejoin, at completion (worker -> supervisor -> lobby).** When the
   root first becomes `Completed`, the worker queues one `SeatReleased` for every routed seat that
   is *not* held (already connected players have their Results; their lobby must not announce a
   rejoin while Result is deferred). When a held seat is reclaimed in `Completed`, it queues
   `SeatReleased` for it right away, so the player's next lobby welcome after Results is ordinary.
   Reuse `SeatRebindQueue::releases` and `ParticipantRebinds::leave` (no new control type;
   `release_seat` in `runtime/rejoin.rs:345-370` already validates and forwards once). Update
   the `SeatReleasedBody` doc and docs/14 wording from "left on purpose" to "can no longer be
   rejoined".
3. **Admit a held seat in Completed (worker).** `rebind_refusal` takes the phase (Active or
   Completed) instead of `phase_active`; `Completed` accepts only `SeatState::Held` that has not
   left, so the per-seat limit and identity rules are unchanged. `validate_match_hello`
   (`admission.rs:285-305`) admits `MatchJoinSource::Rejoin` in `Active | Completed`.
   `reclaim_held_seat` is unchanged (bot controller and hold removed; nothing simulates in
   Completed). A session that drops in `Completed` is still removed, never held
   (`seat_hold.rs:285`).
4. **Supervisor and lobby: no logic change.** Because Result is deferred, `admit_rejoin_request`
   still sees an unended allocation and the lobby still holds the seat, so the existing
   `RejoinRequest` -> `ParticipantRebind` -> `RejoinGranted` -> `MatchRouteGrant` path carries the
   rejoin. `MatchEnded` arrives once, later, and still releases any remaining seats. Confirm with
   a routing unit test that `SeatReleased` before `Result` followed by `MatchEnded` is accepted
   and forwarded once.
5. **Client: make sure the existing path reaches Results.** Expected sequence for both the
   same-process gap and a relaunch: `Rejoining` -> Match screen (`RejoinStarted`,
   `match_failure = true`) -> grant -> `Accepted` (`MatchRejoined` clears `match_failure`) ->
   replicated Completed -> `observe_completed_match` captures the context -> return to lobby ->
   `FreshLobbyReturn` -> Results. Add a flow test (`packages/brawler-client/src/flow/tests.rs`)
   for this order and for the case where the completed snapshot arrives in the same frame as
   `MatchRejoined`. Known degradation, accepted: after a relaunch `game_type_id` is unknown, so
   Results show no Play-again button (`results/view.rs:57`).
6. **Wire and versions.** No application-protocol change (no new message, no replicated
   component, `MatchJoinOutcome` unchanged): protocol stays 49 and the registry fingerprint stays
   unchanged. Verify with `routing-identity` before/after. Control IPC: no new type or body. The
   change is in when `Result`/`SeatReleased` are sent, and all roles ship in one binary, so
   `CONTROL_VERSION_CURRENT` stays 7. Note the timing contract in docs/14 and in the docs/08
   control table ("Result may follow completion by up to the seat hold while a seat is held").
7. **Docs.** docs/14 "Rejoining a running match": replace the "sees Dashboard, not Results"
   sentence with the deferred-Result rule, the Completed rejoin admission and the completion
   `SeatReleased`.

# Acceptance
- Network tests in `tests/network/match_rejoin.rs` (production match-worker graph, harness):
  - `a_match_that_completes_while_a_seat_is_held_defers_its_result_until_the_rejoin`: player one
    lost and held, player two leaves (forfeit) or the match is driven to Completed. No `Result` is
    queued while held. `SeatReleased` is queued for player two only. A rebind is accepted in
    Completed, the rebound identity is admitted, the client world sees `MatchPhase::Completed`
    with its own fighter/team, and `Result` is queued after that session disconnects (or after the
    10 s linger).
  - `a_held_seat_that_expires_after_completion_releases_the_result`: no rejoin, so `Result` is
    queued at hold expiry (60 s of ticks) and a later rebind is `SeatExpired`.
  - `a_match_with_no_held_seat_sends_its_result_at_completion`: regression, same frame as today.
  - Another identity still cannot reclaim in Completed (extend
    `another_identity_cannot_reclaim_a_held_seat`).
- Unit tests: `seat_hold` rebind refusal matrix includes Completed; admission accepts `Rejoin` in
  Completed only for a rebound held seat; result-deferral predicate (held / lingering / none);
  routing `SeatReleased`-then-`MatchEnded` ordering; client flow order from Plan 5.
- E2E: a `just rejoin-e2e` variant (e.g. `BRAWLER_PRODUCT_REJOIN_SMOKE=completed`, or a two-client
  PvP wipeout-1v1 script): client A blacks out beyond 10 s, client B sends Leave Match (the lone
  team forfeits, so the match completes while A's seat is held), and A rejoins. It passes only if
  A logs the rejoin, captures a result context with `Forfeit` and its own team as winner, and
  returns to a fresh lobby. The existing `just rejoin-e2e`, `e2e 2` and `practice-e2e
  wipeout-1v1` still pass.
- `just check`, `just lint`, `just test` pass.
- Device check (Pixel 8a): in PvP wipeout-1v1 against a headless client, cut the phone's network
  (or force-stop it) and make the opponent leave. Restore or relaunch inside 60 s. Expected:
  "Rejoining match…" briefly, then the Results screen for that match ("Victory by forfeit"),
  then Dashboard on continue. A second relaunch goes straight to Dashboard with no rejoin
  attempt. The relaunch variant depends on BRL-2nsg; use the network-gap variant if relaunch
  still times out.

# Out of scope
- A match that completes while the player's session is still inside the 10 s routed bound (not
  yet held). The worker cannot tell that loss from a normal post-Results disconnect, so it keeps
  today's Dashboard message.
- Results for a player who returns after their seat hold expired (> 60 s), or a better message
  than "could not be resumed" for that case.
- Restoring the Play-again target after a relaunch; telling others the seat is bot-held
  (BRL-dwqx); relaunch connectivity (BRL-2nsg).
- Persisting results or progression server-side.


# Resolution 2026-11-16 (commit 721f68cc)
Implemented as planned. Beyond the plan:
- The worker latches the first Completed `MatchState` and builds `Result` from it, because the root can return to Waiting once the roster empties.
- The client waits for its join outcome before capturing the result context, because the Completed snapshot can arrive before `Accepted`.

New automation flags: `--leave-match-cue`, `--product-completed-rejoin-smoke`, plus `just completed-rejoin-e2e`. No protocol or IPC change; routing identity is unchanged.

**Tests:**
- The network tests from the acceptance list, with the full network suite at 128/128.
- Unit tests for seat_hold, admission, routing ordering and the client flow.
- `just check`, `lint` and `test` pass, as do `rejoin-e2e`, `completed-rejoin-e2e` (3 consecutive runs), `e2e 2` and `practice-e2e`.

**Pixel 8a (server 0.0.0.0:5000, APK from 721f68cc):** Wipeout 1v1 against a headless opponent with `--leave-match-cue`. Force-stop the phone 5 s into Active. After its seat is held (10 s), cue the opponent to leave (forfeit). Then relaunch and CONNECT.
- Cycles 1, 3 and 4: Results "Victory by forfeit, Wipeout 1v1" with the score board reached 20 s after the kill.
- Server, each time: `seat held` → `match completed; result deferred for held seats held=1` → `seat reclaimed by a rejoined session` → `reclaimed after completion; its seat is released` → `worker stopped forced=false`.
- Play Again was present.
- After Results → Dashboard, a second relaunch went straight to Dashboard, with no rejoin.
- Cycle 2 is invalid: both CONNECT taps were swallowed right after launch, no flow was logged, and nothing reached the server. That is BRL-gv67.

**Found, filed separately:** `WorkerExitMismatch` after a match formation failure.
