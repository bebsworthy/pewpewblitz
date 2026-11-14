# Outcome

The render evidence script is safe to run beside a developer's live server and with a scratch `CARGO_TARGET_DIR`.

# Context (from BRL-0176)

- `scripts/v3-render-evidence.sh` builds with `cargo build`, which honors `CARGO_TARGET_DIR`, but then executes the literal `target/debug/brawler-server`, `target/debug/brawler-supervisor`, and `target/release/brawler-client`. With a scratch target dir it silently runs stale binaries, or fails.
- Its supervisor gets no `--data-directory`, so it falls back to the default `data/server` (`packages/brawler-routing/src/bin/supervisor.rs`), the tracked directory that a developer server may also use.
- BRL-0176 worked around both with a patched scratch copy (scratch target, `--data-directory` under the scratchpad, `BRAWLER_RENDER_BIND` on an ephemeral port).
- BRL-0170 recorded the same literal-`target/` defect in `_client-features`.

# Scope

- Resolve binaries from `${CARGO_TARGET_DIR:-target}`.
- Give the supervisor a per-run temporary data directory (a `mktemp -d` beside the report), removed on cleanup.

# Acceptance criteria

- `CARGO_TARGET_DIR=/tmp/x just v3-render-evidence /tmp/x/report.txt` runs the freshly built binaries and leaves `data/server/` untouched.
- The default invocation still passes.
