Note (2026-11-15, BRL-k0tr): the BRL-k0tr fix completes the formation once every participant has
its `BeginMatchConnect` (`complete_product_handoff`), so a Practice lobby session that closes at
its own handoff no longer cancels the activation. This ticket's defect should be resolved by that
change; status left for the owner to confirm.

# Device check 2026-11-15 (Pixel 8a), b97a8888
Result: PASS. The evidence is indirect, because the lobby does not log a CancelActivation itself.
Practice Wipeout 2v2 started from the phone at 17:31:02Z. The server log has "recorded a handed-off match seat" and "brawler lobby completed a match handoff" (the `complete_product_handoff` path that forgets the activation). The phone's lobby session then closed (17:31:02.397) with no further lobby or supervisor activity. The Practice worker ran to its own end (result-received, code 0, 17:32:54) with no stop request before that. The same holds for all six PvP handoffs in the BRL-k0tr check.
Evidence (/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/vc): server.log (17:31:02-17:32:54), p1.jpg, p2.jpg, p-logcat.txt.
