# Found (BRL-1chc, 2026-11-20)
`tests/routed_process.rs` `lobby_spec()` points the real lobby worker at `$TMPDIR/brawler-routed-profiles-{pid}.sqlite3` and never removes it. That is the hazard BRL-1chc fixed in the network harness: macOS recycles PIDs, so a later run can reopen a stale file. An older schema makes the lobby worker panic at startup (`profile schema predates published maps`), and old rows change outcomes. Both tests in the binary (`real_bevy_workers_isolate_match_crash_and_cleanup_routes_peers`, `real_bevy_lobby_restarts_after_crash_and_cleans_exactly`) share the one path while they run in parallel, so simply deleting the file at start could remove a sibling's live database.

The target needs `brawler/network-test` and is not part of `just test`, which is why it was not fixed in BRL-1chc.

# Plan
Give each test its own path (a per-test tag plus PID), remove any stale file and its `-wal`/`-shm` side files before the supervisor starts, and remove them after shutdown. The lobby restart test must keep one path across its own worker restart.

# Acceptance
Seeding a schema-2 file at the test's path (exec wrapper keeping the PID) no longer fails either test, and no `brawler-routed-profiles-*` file is left after a run.
