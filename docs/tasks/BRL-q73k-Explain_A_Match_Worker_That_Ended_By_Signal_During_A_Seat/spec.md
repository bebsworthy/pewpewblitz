# Found
Device check 2026-11-15 (Pixel 8a), HEAD b97a8888, server on 0.0.0.0:5000 with RUST_LOG=info. Seen once.

# What happened
- Wipeout 1v1 match worker pid 63370 (worker 88618531226783727793869621698450704611), started 17:24:29Z. The headless opponent left, and its seat was held at 17:24:43 (bot controls it).
- Phone force-stopped at about 17:25:01Z. At 17:25:11, about when the phone's link timeout would hold its seat too, the supervisor logged `worker reaped ... success=false code=None`. There was no result-received, exit-received, stop-requested or "failed" line, and the worker printed nothing (no panic). The lobby then released both seats, so the phone's rejoin found no match (Dashboard).
- No crash report in ~/Library/Logs/DiagnosticReports. The same steps worked at 17:27:25 and later: both seats held, rejoin OK.
- The adb daemon was crashing repeatedly on the Mac during the session (adb .ips reports), so an external kill cannot be ruled out.

# Acceptance
- Reproduce, or rule out an external cause, by running the 1v1 held-seat + second link-loss path with a supervised worker. If it is a worker crash, fix it with a test.

Evidence (/private/tmp/claude-501/-Users-boyd-wip-brawler/716520d5-661e-45a6-996e-1f74fe351a54/scratchpad/vc): server.log lines 36-50 (17:24:29-17:25:11), f-launch.jpg, f-rejoin.jpg, f-logcat.txt.
