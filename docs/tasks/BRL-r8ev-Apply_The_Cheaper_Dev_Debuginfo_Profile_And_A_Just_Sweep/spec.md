Reviewed 2026-11-15 against HEAD 3437c0d8: still valid, not started; profile unchanged (no `debug` key, so full debuginfo), no `just sweep` recipe, and target/ has grown to ~404 GB, which makes the sweep more urgent.

## Context

BRL-qrb8 measured with the current `[profile.dev]` because the "cheaper dev profile" listed under its Related
section had not landed. Current Cargo.toml (root):
```
[profile.dev]
opt-level = 1
[profile.dev.package."*"]
opt-level = 3
```
No `debug` key, so workspace crates and dependencies get full debuginfo. After the BRL-qrb8 split, a client-only
function-body edit still costs about 27-30 s to rebuild (`brawler-client` codegen + link), so debuginfo volume and
link time are the next lever. Baseline tables: docs/tasks/BRL-qrb8-*/spec.md:61-91 (before) and :195-215 (after split;
`target/` 52.6 GB after check+test+lint).

Disk at review time: `target/` 404 GB (`target/debug/deps` 269 GB, `target/debug/incremental` 120 GB,
`target/aarch64-linux-android` 6.4 GB, `target/release` 4 GB, plus stale `target/brl01xx` evidence dirs).
The justfile has only `clean: cargo clean` (justfile:100-102).

## Scope

- `[profile.dev] debug = "line-tables-only"`; `[profile.dev.package."*"] debug = false`.
- `just sweep`: remove stale fingerprints/artifacts (for example `cargo sweep --time N` if installed, or a bounded
  script that prunes old `target/debug/incremental/*` sessions and unreferenced `deps` hashes) without touching
  `data/` or running processes' binaries (the agent test server on UDP 5000 runs `target/debug/brawler-server`; do not
  delete it). Document it in `just help`.
- Re-run the BRL-qrb8 incremental matrix (scratch copy, scratch `CARGO_TARGET_DIR`) and record
  before/after plus `target/` size after `just lint && just test`.

## Plan
1. Baseline matrix on HEAD in a scratch copy (same method as BRL-qrb8 spec :70-91).
2. Apply the profile change; re-run; confirm a panic backtrace (`RUST_BACKTRACE=1`) still shows file:line for workspace frames.
3. Add the `sweep` recipe; run it on the real `target/` and record reclaimed size.

## Acceptance criteria

- Backtraces still show file:line; `just test` and `just e2e 2` pass (and `just lint`).
- Measured incremental and size numbers recorded; revert the profile part if no measurable win.


# Result (2026-11-17)
Implemented: `[profile.dev] debug = "line-tables-only"`, `[profile.dev.package."*"] debug = false`; `just sweep [dry] [N]` (scripts/sweep.py, README row).

Measurement (paired, interleaved base/after builds on warm copies, same method and edits as BRL-qrb8; the machine was loaded, so sequential runs were discarded):
- Body edits 1.2–1.4× faster in every configuration, e.g. protocol-body build-client 26.3→20.0 s, testlib-server 19.0→13.6 s; client-presentation-body test-client-all 32.5→24.7 s; combat-rule-body testlib-server 17.7→12.4 s.
- Item edits ~1.0–1.2×. Clean builds not improved (after ran under higher load; not a fair comparison).
- Artifacts: libbrawler_client.rlib 790→392 MB, libbrawler.rlib 491→251 MB, brawler-client 268→240 MB, brawler-server 101→92 MB. Scratch target after clean+matrix+lint+test 61→38 GB.
- Backtraces keep file:line for workspace frames; frame names are short (`probe`, `{closure#0}`) instead of fully qualified paths.

Sweep: real target/ 513 GB → 187.5 GB (`just sweep`, 2-day default); removes non-newest deps groups and incremental dirs/sessions and stale `target/brl*`/`agent-*`; never touches dev/, e2e/, release/, Android outputs, files hard-linked to top-level debug outputs, or running executables (lsof). Synthetic-tree test covered keep-newest, dev/ protection, protected groups, dry run.

Gates (scratch copy identical to the working tree, own target/, so the live 5000 supervisor binaries were not relinked): `just lint` pass, `just test` pass (2490), `just e2e 2` pass. Dry run on the real tree afterwards: nothing left to remove at the 2-day bound.
