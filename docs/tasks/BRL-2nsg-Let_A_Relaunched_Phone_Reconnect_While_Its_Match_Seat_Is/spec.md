# Found
Device check 2026-11-15 (Pixel 8a), HEAD b97a8888, server on 0.0.0.0:5000. Seen once in 4 force-stop/relaunch cycles.

# What happened
- 17:32:36Z: phone in a Wipeout 1v1 (player 12), headless opponent seat held from 17:32:49. Force-stop about 17:32:45; the phone's seat was held at 17:32:55.
- The relaunched app, CONNECT at about 17:33:0x and RETRY at 17:34:39: "client connecting to server 192.168.31.211:5000" then TimedOut after 10 s (Connection Lost screen). The server log has no "New connection" from the phone in that time.
- During the outage, `adb shell ping 192.168.31.211` succeeded, and a headless client from the Mac (127.0.0.1:5000) connected at 17:33:59. The macOS firewall is off.
- RETRY at 17:35:15 connected at once. The held seat had expired in the meantime (17:33:55), so the rejoin was lost (match forfeit).

# Lead
Not diagnosed. Check whether the supervisor's `SourceIngressLimiter` suppressed or limited the phone's source (60 s suppression / 8 pre-auth datagrams per 10 s window), or whether a stale route for the phone's address absorbed the datagrams. No supervisor log line exists for SourceLimited/Suppressed, so first add a bounded diagnostic.

# Acceptance
- The cause is identified and recorded. A relaunch after a force-stop connects on the first attempt (or the ingress outcome is logged).

Evidence (/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/vc): r4-ss.jpg, r4-c.jpg, r4-logcat2.txt, r4-logcat3.txt, r4-logcat4.txt, probe.log, server.log (17:32:36-17:35:16).
