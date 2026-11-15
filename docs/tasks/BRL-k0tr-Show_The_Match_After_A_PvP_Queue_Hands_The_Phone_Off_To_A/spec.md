# Found
Device check 2026-11-15, Pixel 8a (Android 17), APK and scratch supervisor at HEAD 6a6d6599 (protocol 48), server 0.0.0.0:5001.

# Repro
1. Phone: connect, Choose Game -> Wipeout 1v1 -> SELECT GAME, tap PLAY. The QUEUE screen shows ("1 waiting · 2 players per match").
2. Start a second client that queues the same game type. Used here: a headless automation client
   (`brawler-client --auto-connect --headless --product-match-smoke-1v1 --product-game-type wipeout-1v1`).
3. The server forms the match and hands both clients off. The phone's log shows the match join
   (`client accepted authoritative map snapshot`, `brawler client accepted player_id=4`, roster with both players).

Expected: Match loading, then the match.
Actual: the phone shows the Dashboard (PLAY focused) for the whole match. Hit sounds play (`hit-taken` cues hit the one-shot cap), so the fighter is in the match and is taking damage unseen. PLAY does nothing. The flow only recovers when the match ends.

Reproduced twice: 09:08 (PLAY tapped about 1 s before the opponent queued) and 09:13 (phone waiting on the QUEUE screen when the opponent queued). Practice (Hot Zone, Heist, Zombie, Wipeout) reaches the match normally. A force-stop and relaunch during that invisible match did show it (the BRL-yyeg rejoin moves a lobby screen to Match).

# Notes for the fix
- Not yet known whether this is Android-only or also on desktop: the routed e2e uses headless clients and does not check the flow screen.
- Lead: `QueueDecision::Cancelled` is the reducer path that commits Dashboard with PLAY focus (`flow/reducer/match_flow.rs::resolve_queue_outcome`). The screen matches that path. Check whether the lobby or the client publishes a cancel at the PvP handoff (compare BRL-5vc1 for Practice), and whether BRL-yyeg's seat recording at `BeginMatchConnect` changed the order of queue outcome versus grant.

# Acceptance
- A flow test drives queue Joined -> match grant -> match connect and asserts MatchLoading/Match, never Dashboard.
- On the Pixel 8a a phone-versus-second-client 1v1 shows loading and then the match.

Evidence: /private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/dc/n05.jpg, n06.jpg (first repro), n11-play1.jpg (queue screen), n13.jpg, n14.jpg (Dashboard during match), app-02.txt, server.log, headless2.log.


# Root cause (2026-11-15)

Not caused by BRL-yyeg code; the defect is in the lobby's handoff cleanup (V7) and only shows when
the participants' lobby sessions close at different times, which a phone on Wi-Fi does reliably.
- `lobby_deliver_product_grants` sends `BeginMatchConnect` (+ grant) to every participant. Each
  client then closes its lobby session on purpose.
- `cleanup_disconnected_sessions` treats the first closed lobby session as a lost participant
  while its ticket is still in the reservation: it completes the reservation, pushes
  `CancelActivation` (the match runs on anyway), and sends `MatchmakingServerPhase::Removed
  {ParticipantLost}` to every other participant whose lobby session is still open.
- The phone closes its lobby session 40-100 ms after the headless client (server log 09:08:49.040
  vs .080, 09:13:25.782 vs .879), so it still receives `Removed` after it accepted the grant.
  `ClientMatchLoadingModel` sets `returned_observation`, the flow reduces `MatchStartReturned`
  and commits Dashboard with PLAY focus (no result context), while the match session it already
  started connects and plays unseen. Practice is unaffected because its only participant is the
  one whose session closed, so nobody receives `Removed` (same cleanup bug, BRL-5vc1).

# Fix
- Lobby: once every participant's `BeginMatchConnect` is sent, the lobby's formation is done. A
  lobby session that closes after its `BeginMatchConnect` is the handoff, not a participant loss:
  no reservation cancel, no `CancelActivation`, no `Removed` to the others.
- Client: a `Removed` that arrives after the client accepted that reservation's
  `BeginMatchConnect` grant is stale (the client is already on its way to the match) and is
  ignored, so the flow cannot drop to Dashboard under a running match.


# Implementation (2026-11-15)
- `src/server/lobby/mod.rs::complete_product_handoff`: when `begin_sent` covers every
  participant, complete the reservation (PvP), clear the formation and forget the activation
  (`complete_product_activation`) without a cancel. The later lobby closes then find nothing to
  cancel. This also removes the spurious Practice `CancelActivation` of BRL-5vc1.
- `packages/brawler-client/src/queue/loading.rs`: a `Removed` for the active reservation after the
  lifecycle accepted its grant is logged and ignored.
- Diagnostics: the flow now logs each session fact it reduces ("brawler client flow observed a
  session fact") and every screen change ("brawler client flow changed from/to"), so a device log
  explains navigation (`flow/reducer.rs`).
- No wire, IPC or manifest change of its own (ships with BRL-8y1f's protocol 49 / control 7).
- Docs: docs/14 (handoff completion paragraph).

# Verification (2026-11-15)
- Desktop reproduction: `tests/network/queue.rs`
  `a_handed_off_lobby_session_closing_first_does_not_remove_the_other_participant` (two product
  lobby clients, 1v1 queue, supervisor grant, first client closes its lobby session). With the old
  lobby and client code the second client's loading model is cleared by `Removed` (the Dashboard
  path) and one `CancelActivation` is queued; with the fix neither happens. So the defect is not
  phone-only: any client whose lobby close lags another participant's hits it. The headless e2e
  never showed it because headless clients have no flow and both close in about the same frame.
- Flow test `flow::tests::a_queue_handoff_reaches_the_match_even_after_a_late_reservation_removal`
  drives Joined -> ReservationStarted -> BeginMatchConnect (grant) -> late Removed -> countdown
  through the real loading model and `observe_session`: MatchLoading then Match. Without the
  client guard it ends on Dashboard.
- `just check`, `just lint`, full `just test`, `just e2e 2` (lobby logs "completed a match
  handoff", no activation cancel), `just rejoin-e2e`: pass.
- Pixel 8a: not run; the phone was not attached to adb this session. Device check still owed:
  phone vs headless `--product-match-smoke-1v1 --product-game-type wipeout-1v1` shows Match
  loading then the match. The fixed APK is built, not installed.

# Learn from errors
- The lobby treated "session closed" as "participant lost" even after it had itself told that
  participant to leave. A lifecycle owner must mark the point where it hands an entity to another
  owner and stop interpreting that entity's later events; BRL-5vc1 had seen the same symptom for
  Practice and filed it as harmless because nobody else received the message.
- Product-flow outcomes were only checked on devices. Driving the real loading model in a flow
  test, plus logging flow transitions, makes this class of bug visible on desktop.
