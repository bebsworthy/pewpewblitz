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
