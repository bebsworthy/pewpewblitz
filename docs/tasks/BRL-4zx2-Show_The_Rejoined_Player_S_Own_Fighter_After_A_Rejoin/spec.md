# Found
Device check 2026-11-15 (Pixel 8a), HEAD b97a8888 (protocol 49), server `just server` on 0.0.0.0:5000, during the BRL-8y1f involuntary-loss check. Reproduced 3 of 3 successful rejoins.

# Repro
1. Phone (player Brawler-42DD) queues Wipeout 1v1. A headless opponent (`brawler-client --auto-connect --headless --product-match-smoke-1v1 --product-game-type wipeout-1v1 --client-id 510N`) queues too. The headless client exits about 13 s into the match, so its seat is held by a server bot (hold 60 s).
2. In the match: `adb shell am force-stop com.pewpewblitz.client`, wait about 13-15 s, relaunch, then CONNECT.
3. The lobby sends a rejoin and the server logs "seat reclaimed by a rejoined session player_id=<phone's id>". The phone logs "rejoined its running match player_id=<same id>; network_entity_id = 2" with the same roster as before.

Expected: the same fighter, with its own name plate, the HUD scores from the phone's team, and the camera on the phone's fighter.
Actual: the camera-centred fighter with the green "self" ring is labelled with the opponent's name (Brawler-13EF / 13F0 / 13F2, the headless client's name). The kill feed shows the opponent's name in green killing Brawler-42DD in red. The HUD blue score is the opponent's score: blue 1 / red 0 in the match, but Results said Your team 0, Opponents 1. In r5-pre both fighters have green name plates.
Before the force-stop the same match showed Brawler-42DD correctly (f2-before, r3-before).

# Notes
- In all three repros the opponent's seat was held by a server bot at rejoin time. In the BRL-yyeg device check the phone rejoined with its own name plate, so the held opponent or the entity order may matter: the phone was network_entity_id 2 every time.
- Not confirmed whether touch input moves the phone's own fighter or the other one: the match ended by forfeit before the joystick check finished.

# Acceptance
- A rejoined client presents its own fighter (local-player marker, name plate, camera, team-relative HUD and kill feed) in a test that rejoins while the other seat is bot-held.
- Pixel 8a: after force-stop and rejoin, the name plate and HUD match the Results screen.

Evidence (/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/vc): f2-before.jpg, f2-rejoin.jpg, f2-label.png, l2-after.jpg (Results), r3-before.jpg, r3-b.jpg, r3-c.jpg, r3-e.jpg (Results), r5-c.png, r5-pre.jpg, r5-post.jpg; f2-logcat.txt, r3-logcat.txt, r5-logcat.txt; server.log (17:27:38, 17:29:47, 17:36:22).
