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
