# Found (BRL-01c8, 2026-11-20)
During BRL-01c8 the first full `just test` at load average ~9 had 8 network test failures (`lobby_takeover::*`, `map_publishing::*`, `queue::a_handed_off…`), each a bare `run_main` panic with no message in the captured output. All passed alone, then the full network suite passed 136/136 and a second `just test` passed. Same class as BRL-xbhb (a system failing validation when wall-clock time yields zero or several fixed steps).

# Plan
Reproduce under load (`yes` processes on every core) with `RUST_BACKTRACE=1 --nocapture`, capture the real error, and fix the cause (harness time stepping or a missing resource), not the symptom.

# Acceptance
Root cause recorded; the network suite passes 5 consecutive full runs under heavy load.


# Root cause (2026-11-20)
Host load was not the cause. Load and the failures share a cause: many processes running, so PIDs get reused.

- Captured with `RUST_BACKTRACE=1 -- --nocapture --test-threads=1` (the `just test` thread count), from the one test whose Compute Task Pool thread output libtest captured:
  `thread 'Compute Task Pool (4)' panicked at src/server/lobby/mod.rs:1436:29: profile storage failed to start: Database("profile schema predates published maps; delete this test database")`, then `Encountered a panic in system brawler::server::lobby::initialize_lobby_state!`, then `Encountered a panic in system bevy_app::main_schedule::Main::run_main!`. The other seven tests showed only the bare `run_main` line. The real panic message prints on a shared Bevy task-pool thread, and libtest captures that thread's output for at most one test. That is why the original log had no message.
- Mechanism: `Harness::new_product_lobby` (`tests/network/harness.rs`) pointed the real lobby worker at `$TMPDIR/brawler-network-profiles-{pid}-{seq}.sqlite3` and never deleted the file. macOS recycles PIDs. A later test process with a reused PID reopened an earlier run's databases. This machine had 1,524 leftover files from 217 PIDs: 1,182 at schema 2, 208 at schema 3, 170 at schema 4. A schema-2 file makes `ProfileStorage::migrate` refuse to open, and `initialize_lobby_state` panics in `Startup`, inside `Main::run_main`.
- Why it matched the report exactly: with `--test-threads=1`, product-lobby tests take sequence numbers alphabetically. lobby_takeover takes 1-3, map_publishing 4-7 and queue 8-12. A stale PID whose earlier run left sequences 1-8 fails exactly the 3 lobby_takeover tests, the 4 map_publishing tests and `queue::a_handed_off_lobby_session_closing_first_does_not_remove_the_other_participant`. That is the BRL-01c8 list. A rerun gets a new PID and passes.
- Second effect of the same cause: a stale file at the current schema opens fine but keeps the earlier run's accounts and published maps. Reproduced: all 4 `map_publishing` tests fail with `a valid recipe is accepted` (for example `Rejected(CoolingDown { retry_after_seconds: 23 })`), because an earlier author's publish cooldown and quotas carry over.
- Not a production bug. Production gives the lobby its own data directory. Refusing a too-old schema is intended.

## Reproduction (deterministic)
`scratchpad/run_seeded.sh` copies seed databases to `$TMPDIR/brawler-network-profiles-$$-N.sqlite3`, then `exec`s the test binary, so the PID matches. Before the fix: 8 schema-2 seeds with `--test-threads=1 lobby_takeover map_publishing queue` gave 8 failed / 5 passed, the exact BRL-01c8 set. Seeds from a fresh current-schema run made the 4 map_publishing tests fail. Load alone did not reproduce it: 3 full parallel runs with 24 `yes` processes on 12 cores, load about 36, passed 136/136 before the fix.

# Change
`tests/network/harness.rs`: a new `ProfileDatabaseFile` guard. It removes any existing database and its `-wal`/`-shm` files before the lobby app is built. It panics with the path if removal fails for any reason other than NotFound. `Drop` removes the files again. It is the last field of `Harness`, so it drops after the server `App`, whose storage executor joins and closes the connection. It also runs while a failed test unwinds. No production code changed, so there is no new production test. The reproduction above is the regression check.

# Verification
- After the fix, both seeded reproductions (schema-2 and current-schema stale rows) pass 13/13, and no `brawler-network-profiles-{pid}-*` file is left afterwards.
- 5 consecutive full runs, `cargo test --locked -p brawler-client --features network-test --test network -- --nocapture` with 24 `yes` processes on 12 cores (load 31.8 / 35.7 / 36.3 / 36.4 / 37.7): 136/136 each. The load processes were killed afterwards.
- `just lint` / `just test`: see below.

# Related / deferred
- `tests/routed_process.rs` has the same PID-only path, `brawler-routed-profiles-{pid}.sqlite3`. Two concurrent tests share it, so it needs a per-test path, not just removal. It is not in `just test`. Filed as BRL-shwv (backlog).
- The 1,500+ stale `brawler-network-profiles-*` files already in `$TMPDIR` are harmless now, because each harness clears its path before use. They were left in place rather than deleted.

# Learning
- A bare `Main::run_main` panic in a test only says that something under Main panicked. The real message prints on a Bevy task-pool thread, and libtest assigns that thread's output to at most one test. Run the failing set with `--test-threads=1 --nocapture` and look for the first `panicked at` on a `Compute Task Pool` thread.
- "Fails under load" can mean "fails while many processes run". Look for state keyed by PID or by time before blaming scheduling.


## Reproduction script (the scratchpad copy is temporary)
```sh
#!/bin/sh
# run_seeded.sh SEED_DIR TEST_BINARY ARGS...: exec keeps the PID, so the seeds land at the harness's paths.
seed=$1; shift
for f in "$seed"/*.sqlite3; do
  n=$(basename "$f" | sed -E 's/.*-([0-9]+)\.sqlite3/\1/')
  cp "$f" "$TMPDIR/brawler-network-profiles-$$-$n.sqlite3"
done
exec "$@"
```
Seed `x-1.sqlite3` … `x-8.sqlite3` with a `user_version=2` profile database, then run `run_seeded.sh seeds target/debug/deps/network-<hash> --test-threads=1 lobby_takeover map_publishing queue`.

## Final verification (commits a6674c92, 5f48033f on `worktree-agent-a828c0e4dcde8976c`)
- 5f48033f routes all three `Harness` constructors through one `from_server`. That fixes the Clippy `too_many_lines` finding the guard field caused on `new_mode_with_options`, plus two doc/format findings.
- `just lint`: pass.
- `just test`: pass. Network 136/136, performance gates included.
- After the final commit, the schema-2 seeded reproduction passes 13/13 and leaves no files behind.

## Merge verification (main, 2026-11-20)

Cherry-picked both commits onto main (test harness only). `just lint` pass; full network suite 136/136. The agent's loaded runs and stale-database reproductions are recorded above.
