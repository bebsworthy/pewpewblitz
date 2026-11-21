# Found (BRL-1chc, 2026-11-20)
`tests/routed_process.rs` `lobby_spec()` points the real lobby worker at `$TMPDIR/brawler-routed-profiles-{pid}.sqlite3` and never removes it. That is the hazard BRL-1chc fixed in the network harness: macOS recycles PIDs, so a later run can reopen a stale file. An older schema makes the lobby worker panic at startup (`profile schema predates published maps`), and old rows change outcomes. Both tests in the binary (`real_bevy_workers_isolate_match_crash_and_cleanup_routes_peers`, `real_bevy_lobby_restarts_after_crash_and_cleans_exactly`) share the one path while they run in parallel, so simply deleting the file at start could remove a sibling's live database.

The target needs `brawler/network-test` and is not part of `just test`, which is why it was not fixed in BRL-1chc.

# Plan
Give each test its own path (a per-test tag plus PID), remove any stale file and its `-wal`/`-shm` side files before the supervisor starts, and remove them after shutdown. The lobby restart test must keep one path across its own worker restart.

# Acceptance
Seeding a schema-2 file at the test's path (exec wrapper keeping the PID) no longer fails either test, and no `brawler-routed-profiles-*` file is left after a run.


# Implementation (2026-11-21)
- `tests/support/profile_database_file.rs` (new): the `ProfileDatabaseFile` guard from BRL-1chc, moved out of `tests/network/harness.rs` so both test binaries share one copy (`#[path]` module in `tests/network.rs` and `tests/routed_process.rs`). Behavior unchanged: remove the database and `-wal`/`-shm` before use (panic with the path on any error but NotFound), remove them again on drop.
- `tests/routed_process.rs`: each test takes its own guard, `brawler-routed-profiles-{pid}-{isolate|restart}.sqlite3`, declared first so it drops after the supervisor stops the lobby. `lobby_spec(&database)` puts that path in the manifest; the supervisor relaunches a crashed lobby from the same spec, so the restart test keeps one path across its restart.
- Also fixed (found while verifying): `real_bevy_workers_isolate_match_crash_and_cleanup_routes_peers` failed on main before this change. Its allocation pinned `map_revision: 1` and `map_recipe_fingerprint: 1`, so both match workers exited with `MapRevisionMismatch` at admission. The request now reads preset 1's admission revision and recipe fingerprint from `MapContentCatalog::embedded()`, as the admission unit test fixture does.

# Verification
- `cargo test -p brawler --features network-test --test routed-process`: 2/2 pass (before: 1/2, the map-revision failure above).
- Seeded reproduction: a `user_version=2` database copied to the old path and both new paths by an exec wrapper that keeps the PID (`run_routed_seeded.sh`, below). Old test file: the lobby panics `profile storage failed to start: ... predates published maps` on a task-pool thread, yet the restart test still reports ok because it never sends a profile request. That is why this hazard was silent. New test file: 2/2 pass, no profile-storage panic in the lobby's stderr, and no `-isolate`/`-restart` file left in `$TMPDIR` afterwards.
- `cargo test -p brawler-client --features network-test --test network`: 136/136 (shared guard).
- `just lint`: pass. `cargo clippy -p brawler --features network-test --test routed-process -- -D warnings`: clean.

```sh
#!/bin/sh
# run_routed_seeded.sh SEED TEST_BINARY ARGS...: exec keeps the PID, so the seed lands at each test's path.
seed=$1; shift
for tag in isolate restart; do cp "$seed" "$TMPDIR/brawler-routed-profiles-$$-$tag.sqlite3"; done
cp "$seed" "$TMPDIR/brawler-routed-profiles-$$.sqlite3"
exec "$@"
```
Seed: `sqlite3 seed.sqlite3 'PRAGMA user_version=2; CREATE TABLE players(id INTEGER);'`. Set `StderrPolicy::Inherit` in the test temporarily to see the lobby's panic.

# Learning
- These routed tests check lifecycle events only, so a lobby that panics after Ready still passes. A worker's stderr is the only place the failure shows; inherit it when a routed test result looks too clean.
- The routed-process target is outside `just test`, which is how the stale map revision went unnoticed. Not changed here: adding it to `just test` is a separate decision.
