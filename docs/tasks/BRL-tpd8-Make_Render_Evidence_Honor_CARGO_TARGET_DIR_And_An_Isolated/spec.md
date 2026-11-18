Reviewed 2026-11-15 against HEAD 3437c0d8: valid, spec refreshed — the script still runs literal target/ binaries and gives the supervisor no data directory; the `_client-features` half is already fixed.

# Outcome

The render evidence script is safe to run beside a developer's live server and with a scratch `CARGO_TARGET_DIR`.

# Context (from BRL-farz, re-verified)

- `scripts/v3-render-evidence.sh:129-132` builds with `cargo build`, which honors `CARGO_TARGET_DIR`. It then runs literal paths: `target/debug/brawler-server` (line 134), `target/debug/brawler-supervisor` (175), `$project_dir/target/debug/brawler-server` as the worker (179), and `target/release/brawler-client` (187, 198, 215, 227). With a scratch target dir it silently runs stale binaries, or fails.
- The supervisor gets no `--data-directory`, so it uses the default `data/server` (`packages/brawler-routing/src/bin/supervisor.rs:386`). That directory is tracked (`data/server/logical-server-id`, `data/server/profiles.sqlite3`) and a developer server may also use it. Since BRL-1fam, `BRAWLER_RENDER_WEAPON` makes the measured client create and select saved brawlers (`packages/brawler-client/src/profile/automation.rs`). Those rows land in the tracked `profiles.sqlite3`.
- Clients get no `BRAWLER_CLIENT_DATA_DIR` (`packages/brawler-client/src/connection_persistence.rs:16`), so they share the developer's client data.
- Already done: BRL-qrb8's `_client-features` defect is fixed (`scripts/check-client-features.sh:93` uses `${CARGO_TARGET_DIR:-target}`).
- Existing pattern to copy: `scripts/network.sh:26` / `network-match.sh:6` (`target_dir="${CARGO_TARGET_DIR:-$repo_root/target}"`) and `scripts/dev.sh:8-9,71,145` (separate server and client data dirs).

# Plan

1. Add `target_dir=${CARGO_TARGET_DIR:-$project_dir/target}` after `cd "$project_dir"` (line 122) and replace every literal `target/debug/...` and `target/release/...` execution with `$target_dir/...`. Leave the default report path (line 5) unchanged.
2. Create `run_data_dir=$(mktemp -d "${TMPDIR:-/tmp}/brawler-render-evidence.XXXXXX")` before the supervisor starts. Pass `--data-directory "$run_data_dir/server"` to the supervisor and `BRAWLER_CLIENT_DATA_DIR="$run_data_dir/client-<id>"` to each client.
3. In `cleanup()` (line 146), remove `$run_data_dir` after the processes are reaped. Support `BRAWLER_RENDER_KEEP_DATA=1` to keep it, and print its path on failure for debugging.
4. Optionally, refuse to start if `BRAWLER_RENDER_BIND` is `*:5000`, to protect the developer's server.

# Acceptance

- `CARGO_TARGET_DIR=$PWD/target/scratch-tpd8 just v3-render-evidence target/scratch-tpd8/report.txt` builds into and runs from `target/scratch-tpd8`. `ps` or the log shows that path.
- After a `BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_WEAPON=7` run, `git status --porcelain data/server` is empty, and no `brawler-render-evidence.*` dir remains in `$TMPDIR`.
- The default `just v3-render-evidence` still passes; `bash -n scripts/v3-render-evidence.sh` and `just lint` pass.


# Result (2026-11-18)

Implemented in `scripts/v3-render-evidence.sh`:
- `target_dir=${CARGO_TARGET_DIR:-$project_dir/target}`. The routing-identity probe, the supervisor, the worker executable and every client now run from `$target_dir`. The default report path is unchanged.
- Server data was already isolated in `${report_path}.server-data` (BRL-qfby). I kept that report-adjacent layout instead of the `mktemp` plan, so the data stays inspectable next to the report and never lands in `$TMPDIR`.
- Each client gets `BRAWLER_CLIENT_DATA_DIR=${report_path}.client-data/<id>`. The overwrite guard also refuses an existing client-data dir. Automation clients never write `connections.ron`, so the directory normally stays absent; the env var keeps any write out of the developer's config directory.
- The script refuses `BRAWLER_RENDER_BIND=*:5000` with exit 2, which protects the developer server port.

Verification:
- `bash -n` passes.
- `BRAWLER_RENDER_BIND=0.0.0.0:5000 scripts/v3-render-evidence.sh target/x.txt` exits 2 with the refusal message.
- `CARGO_TARGET_DIR=$PWD/target/scratch-tpd8 BRAWLER_RENDER_PRACTICE=1 BRAWLER_RENDER_WEAPON=7 scripts/v3-render-evidence.sh target/tpd8/scratch.txt` exits 0. It built `release/brawler-client`, `debug/brawler-server` and `debug/brawler-supervisor` fresh into `target/scratch-tpd8`, which was the only place they existed. Profile rows landed in `target/tpd8/scratch.txt.server-data/`, and `data/server/` mtimes are unchanged (Nov 16). The client log has 3 "evidence capture saved" lines. I deleted the scratch target afterwards.
- `just v3-render-evidence target/tpd8/default.txt` exits 0.
- `just lint` exits 0.
